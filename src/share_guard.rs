// Python names objects; the typed build copies lists, dicts and sets (a Rust Vec or map is a value). The two
// agree exactly when no container is changed through one name while another name that Python would see the
// same object through is still read afterwards. This pass proves that on the Python tree, before the typed
// build runs, and sends every file it cannot prove to the dynamic build (src/dynpy), which shares as CPython
// does. Class instances are handles in the typed build, so only containers matter.
//
// The proof is an abstract interpretation over each function:
// - kinds: how deep a name's containers nest (0: never a container: a number, string, object or function;
//   UNK: unknown), so an int read out of a list is not tracked;
// - edges: pairs "object at depth a of place P may be the object at depth b of place Q", made wherever the
//   typed build would copy what Python shares (`y = x`, `rows.append(row)`, `self.items = items`, `x[i]`,
//   a loop variable, a call's argument kept by the callee, a returned parameter or field);
// - liveness: whether a name may still be read;
// - a change through P at depth d conflicts with an edge (P, a, Q, b), a <= d, when Q may still be read.
// Function summaries (which parameters, fields and globals a function changes, and which it keeps or hands
// back) carry this across calls, iterated to a fixpoint.
//
// Where the typed build itself shares (a `mut` parameter passed `&mut place`, a loop over a list that changes
// each item in place through `iter_mut`, a lifted closure's captured list), the proof assumes the borrow and
// records it as required (see REQUIRED); main.rs (typed) and emit.rs check that the typed build made each one,
// and a file where it did not goes to the dynamic build too. Nothing is assumed that is not checked.
use crate::dynpy::parse::{Arg, Ex, FPart, FuncDef, Params, St, Stmt};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const UNK: u8 = 8;

// Borrows the proof relied on: (typed function name, parameter name) that must be a `mut` (`&mut`) parameter,
// and (line, loop variable) loops that must walk their list in place.
#[derive(Default, Clone, Debug)]
pub struct Required {
    pub muts: HashSet<(String, String)>,
    pub iters: HashSet<(usize, String)>,
    // (line, name, depth): the object at that depth of the name must be a handle in the typed build (a class
    // instance, a dict record, a row), whose copies are the same object; rustc checks it (see alias::share_marks).
    pub marks: BTreeSet<(usize, String, u8)>,
    // Module-level names a function uses and the module binds again: the typed build must keep one variable
    // for each (a rebinding of another type starts a new variable there, which the function would not see).
    pub globals: BTreeMap<String, usize>,
}

// By file: each file of a program is proved on its own (main.rs guard_files), at its own line numbers.
static REQUIRED: std::sync::Mutex<Vec<(std::path::PathBuf, Required)>> = std::sync::Mutex::new(vec![]);

// What the proof requires, at the lines of the joined source the typed build reads (modules.rs puts each
// module after the main file): a requirement of a file that is not part of it is dropped with that file.
pub fn required() -> Required {
    let mut out = Required::default();
    for (path, r) in REQUIRED.lock().unwrap().iter() {
        let Some(off) = crate::seg_offset(path) else { continue };
        out.muts.extend(r.muts.iter().cloned());
        out.iters.extend(r.iters.iter().map(|(l, n)| (l + off, n.clone())));
        out.marks.extend(r.marks.iter().map(|(l, n, d)| (l + off, n.clone(), *d)));
        out.globals.extend(r.globals.iter().map(|(n, l)| (n.clone(), l + off)));
    }
    out
}

fn add_required(path: &std::path::Path, r: Required) {
    let mut g = REQUIRED.lock().unwrap();
    match g.iter_mut().find(|(p, _)| p == path) {
        Some((_, have)) => {
            have.muts.extend(r.muts);
            have.iters.extend(r.iters);
            have.marks.extend(r.marks);
            have.globals.extend(r.globals);
        }
        None => g.push((path.to_path_buf(), r)),
    }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Pl {
    L(String), // a local of the function analysed (a parameter, a captured name, a comprehension's name)
    G(String), // a module-level name
    F(String), // an object field, by name, over every class
    Ret,       // the function's value
    Ext,       // somewhere the proof does not follow (an unknown function's value)
}

type Edge = (Pl, u8, Pl, u8);
// (place, depth in the place, depth in the value): the value's object at that depth is the place's.
type Src = (Pl, u8, u8);

fn add_d(a: u8, b: u8) -> u8 {
    a.saturating_add(b).min(UNK)
}

fn norm(a: Pl, da: u8, b: Pl, db: u8) -> Edge {
    if (&a, da) <= (&b, db) {
        (a, da, b, db)
    } else {
        (b, db, a, da)
    }
}

// The value's items: its object at depth k is the source's at depth k+1.
fn down(s: &[Src]) -> Vec<Src> {
    s.iter().map(|(p, dp, dv)| if *dv > 0 { (p.clone(), *dp, dv - 1) } else { (p.clone(), add_d(*dp, 1), 0) }).collect()
}
// A container holding the value.
fn up(s: &[Src]) -> Vec<Src> {
    s.iter().map(|(p, dp, dv)| (p.clone(), *dp, add_d(*dv, 1))).collect()
}
// A shallow copy (`list(x)`, `x[:]`, `sorted(x)`): a new object holding the same items.
fn shallow(s: &[Src]) -> Vec<Src> {
    s.iter().map(|(p, dp, dv)| if *dv == 0 { (p.clone(), add_d(*dp, 1), 1) } else { (p.clone(), *dp, *dv) }).collect()
}
// Pairs of (index, item) or tuples of items (`enumerate(x)`, `zip(x, y)`, `d.items()`, itertools).
fn paired(s: &[Src]) -> Vec<Src> {
    s.iter().map(|(p, dp, dv)| if *dv == 0 { (p.clone(), add_d(*dp, 1), 2) } else { (p.clone(), *dp, add_d(*dv, 1)) }).collect()
}

#[derive(Default, Clone, PartialEq, Debug)]
struct Sum {
    mutp: BTreeMap<String, u8>,    // parameter (or captured name) -> least depth changed
    esc: BTreeSet<(X, u8, X, u8)>, // edges between the function's outside places at its end
    mutf: BTreeMap<String, u8>,
    mutg: BTreeMap<String, u8>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum X {
    P(String),
    F(String),
    G(String),
    Ret,
}

struct FnInfo<'a> {
    def: &'a FuncDef,
    typed: String,
    class: Option<String>,
    parent: Option<usize>,
    params: Vec<String>, // positional, *star, keyword-only, **kw, then captured names
    npos: usize,
    star: Option<String>,
    kwstar: Option<String>,
    captured: Vec<String>,
    locals: HashSet<String>,
    globals: HashSet<String>,
    nested: HashMap<String, usize>, // defs inside it
    eligible: bool,                 // the typed build makes it a function whose `mut` parameters borrow
}

struct Prog<'a> {
    fns: Vec<FnInfo<'a>>,
    top: HashMap<String, usize>,               // module-level defs
    classes: HashMap<String, Option<usize>>,   // class -> its __init__
    class_fields: HashMap<String, Vec<String>>, // dataclass-style fields, for a class with no __init__
    methods: HashMap<String, Vec<usize>>,      // method name -> defs, over every class
    modules: HashSet<String>,                  // imported module names
    fromimp: HashMap<String, (String, String)>, // `from m import n as a`: a -> (m's first part, n)
    module_names: HashSet<String>,             // module-level names
    bound_other: HashSet<String>,
    dicts: HashSet<String>, // see dict_only
    regex: HashSet<String>, // see regex_only
    lambdas: HashMap<(usize, String), &'a Ex>, // see lambda_names              // module-level names some statement other than an import binds
    fn_globals: HashSet<String>,               // module-level names some function mentions
    // kinds
    kl: HashMap<(usize, String), K>, // (function, name) -> shape; function usize::MAX is the module
    kf: HashMap<String, K>,
    kret: HashMap<usize, K>,
    as_value: HashSet<usize>, // functions used as a value (a callback), whose calls the proof does not see
    sums: Vec<Sum>,
}

const MODULE: usize = usize::MAX;

// The folder and module name of the file being checked (set by main.rs guard_files), so `from util import f`
// of a sibling file can be followed instead of taken as an unknown call (see inline_imports).
static CONTEXT: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

// Each file's text and its parse, so the files of one program are parsed once (guard_files reads each one,
// and the proof of a file reads the modules it imports).
static PARSES: std::sync::Mutex<Vec<(std::path::PathBuf, String, Result<Vec<Stmt>, (usize, String)>)>> = std::sync::Mutex::new(vec![]);

pub fn parsed(path: &std::path::Path, src: &str) -> Result<Vec<Stmt>, (usize, String)> {
    let mut g = PARSES.lock().unwrap();
    if let Some((_, _, r)) = g.iter().find(|(p, t, _)| p == path && t == src) {
        return r.clone();
    }
    let r = crate::dynpy::parse::parse(src);
    g.push((path.to_path_buf(), src.to_string(), r.clone()));
    r
}

// Modules whose whole body a passing proof of an importing file took in (see check).
static COVERED: std::sync::Mutex<BTreeSet<std::path::PathBuf>> = std::sync::Mutex::new(BTreeSet::new());

pub fn set_context(file: &std::path::Path) {
    *CONTEXT.lock().unwrap() = Some(file.to_path_buf());
}

// Lines of statements taken from the k-th followed module are k * SHIFT + their own line, so a conflict found
// in them is reported at the import that brought them in, and what the proof requires of them (borrows, loops,
// marks) is kept at their own lines, as the module's own check keeps them.
const SHIFT: usize = 10_000_000;

// Why the file needs the dynamic build (with the line), and the borrows to check when it does not.
pub fn check(body: &[Stmt]) -> Option<(usize, String)> {
    let file = CONTEXT.lock().unwrap().clone().unwrap_or_default();
    // Every statement of this module was already proved, joined into a file that imports it.
    if !file.as_os_str().is_empty() && COVERED.lock().unwrap().contains(&file) {
        return None;
    }
    let dir = file.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let inlined = if file.as_os_str().is_empty() { None } else { inline_imports(body, &dir, &stem) };
    let (merged, imports) = inlined.unwrap_or_else(|| (body.to_vec(), vec![]));
    let req = match check_body(&merged) {
        Ok(r) => r,
        Err((line, why)) => {
            return Some(match line / SHIFT {
                0 => (line, why),
                k => {
                    let (at, m) = &imports[k - 1];
                    (*at, format!("{why} (in {m}.py line {})", line % SHIFT))
                }
            })
        }
    };
    // Each requirement goes to the file its line is in.
    let mut parts: Vec<Required> = vec![Required::default(); imports.len() + 1];
    for (l, n) in req.iters {
        parts[l / SHIFT].iters.insert((l % SHIFT, n));
    }
    for (l, n, d) in req.marks {
        parts[l / SHIFT].marks.insert((l % SHIFT, n, d));
    }
    for (n, l) in req.globals {
        parts[l / SHIFT].globals.insert(n, l % SHIFT);
    }
    parts[0].muts = req.muts;
    for (k, r) in parts.into_iter().enumerate() {
        let path = if k == 0 { file.clone() } else { dir.join(format!("{}.py", imports[k - 1].1)) };
        if k > 0 {
            COVERED.lock().unwrap().insert(path.clone());
        }
        add_required(&path, r);
    }
    None
}

// Module-scope names a file binds, and how: `import x` and `from m import n` bind the same object in every
// file, so two files may both bind them.
fn bound_names(body: &[Stmt]) -> (HashSet<String>, HashMap<String, String>) {
    let mut all = HashSet::new();
    let mut g = HashSet::new();
    assigned(body, &mut all, &mut g);
    all.extend(g);
    let mut how = HashMap::new();
    for s in body {
        match &s.st {
            St::Import(v) => v.iter().filter(|(_, a)| a.is_none()).for_each(|(m, _)| {
                let root = m.split('.').next().unwrap_or(m).to_string();
                how.insert(root.clone(), format!("import {root}"));
            }),
            St::ImportFrom(m, v) if !m.starts_with('.') => v.iter().filter(|(_, a)| a.is_none()).for_each(|(n, _)| {
                how.insert(n.clone(), format!("from {m} import {n}"));
            }),
            _ => {}
        }
    }
    (all, how)
}

// Names a file may read from module scope or builtins: everything it mentions at module level, and in each
// function what is not its own local (a nested function's locals count as free here, which only refuses more).
fn free_names(body: &[Stmt]) -> HashSet<String> {
    fn walk(b: &[Stmt], out: &mut HashSet<String>) {
        for s in b {
            match &s.st {
                St::Def(d) => {
                    let mut used = HashSet::new();
                    mentioned_stmt(s, &mut used);
                    let (params, ..) = param_names(&d.params);
                    let mut locals: HashSet<String> = params.into_iter().collect();
                    let mut globals = HashSet::new();
                    assigned(&d.body, &mut locals, &mut globals);
                    for g in &globals {
                        locals.remove(g);
                    }
                    out.extend(used.into_iter().filter(|n| !locals.contains(n)));
                }
                St::Class(c) => {
                    c.bases.iter().chain(c.decorators.iter()).for_each(|x| mentioned(x, out));
                    walk(&c.body, out);
                }
                _ => {
                    mentioned_stmt(s, out);
                }
            }
        }
    }
    let mut out = HashSet::new();
    walk(body, &mut out);
    out
}

fn shift_lines(b: &mut [Stmt], by: usize) {
    for s in b {
        s.line += by;
        match &mut s.st {
            St::Def(d) => {
                d.line += by;
                shift_lines(&mut d.body, by);
            }
            St::Class(c) => {
                c.line += by;
                shift_lines(&mut c.body, by);
            }
            St::If(arms, els) => {
                arms.iter_mut().for_each(|(_, b)| shift_lines(b, by));
                if let Some(e) = els {
                    shift_lines(e, by);
                }
            }
            St::While(_, b, e) | St::For(_, _, b, e) => {
                shift_lines(b, by);
                if let Some(e) = e {
                    shift_lines(e, by);
                }
            }
            St::Try(b, hs, e, fin) => {
                shift_lines(b, by);
                for h in hs.iter_mut() {
                    h.line += by;
                    shift_lines(&mut h.body, by);
                }
                for x in [e, fin].into_iter().flatten() {
                    shift_lines(x, by);
                }
            }
            St::With(_, b) => shift_lines(b, by),
            _ => {}
        }
    }
}

// The program with each module-level `from util import f, g` of a sibling file replaced by util.py's own
// statements, so the proof follows f and g as it follows the file's own functions instead of taking them as
// unknown calls that may change what they are given. Followed only when that is exact: every name imported
// is a function util.py defines once at module level, the import has no `as`, and no name one file binds at
// module level is one the other may read or bind (shared `import x` and `from m import n` lines aside), so the
// joined namespace means what the two files' own namespaces mean. Anything else stays an unknown call, which
// refuses. Also returns, for each followed module, the import's line and the module's name.
fn inline_imports(body: &[Stmt], dir: &std::path::Path, stem: &str) -> Option<(Vec<Stmt>, Vec<(usize, String)>)> {
    let mut out: Vec<Stmt> = body.to_vec();
    let mut imports: Vec<(usize, String)> = vec![];
    let mut done: HashMap<String, HashSet<String>> = HashMap::new();
    let mut i = 0;
    while i < out.len() {
        let St::ImportFrom(m, names) = &out[i].st else {
            i += 1;
            continue;
        };
        let (m, names) = (m.clone(), names.clone());
        let plain = !m.is_empty() && m.chars().all(|c| c.is_alphanumeric() || c == '_') && m != stem;
        let wanted: HashSet<String> = names.iter().map(|(n, _)| n.clone()).collect();
        if !plain || names.iter().any(|(n, a)| a.as_ref().is_some_and(|a| a != n)) {
            i += 1;
            continue;
        }
        // Already followed earlier in the program: its functions are bound, the same objects.
        if let Some(defs) = done.get(&m) {
            if wanted.is_subset(defs) {
                out[i].st = St::Pass;
            }
            i += 1;
            continue;
        }
        let path = dir.join(format!("{m}.py"));
        // As guard_files reads it (without `allow` lines), so the parse is shared.
        let Some(mut mbody) = crate::read_src(&path).ok().and_then(|src| parsed(&path, &crate::caps::split(&src).0).ok()) else {
            i += 1;
            continue;
        };
        let defs: HashSet<String> = mbody.iter().filter_map(|s| if let St::Def(d) = &s.st { Some(d.name.clone()) } else { None }).collect();
        let once = |n: &String| mbody.iter().filter(|s| matches!(&s.st, St::Def(d) if &d.name == n)).count() == 1;
        let (mb, mhow) = bound_names(&mbody);
        let mf = free_names(&mbody);
        let mut rest = out.clone();
        rest.remove(i);
        let (pb, phow) = bound_names(&rest);
        let pf = free_names(&rest);
        let same = |n: &String| mhow.get(n).is_some_and(|h| phow.get(n) == Some(h));
        let clash = mb.iter().any(|n| !wanted.contains(n) && (pf.contains(n) || pb.contains(n)) && !same(n))
            || pb.iter().any(|n| !wanted.contains(n) && mf.contains(n) && !same(n))
            || wanted.iter().any(|n| pb.contains(n));
        if !wanted.iter().all(|n| defs.contains(n) && once(n)) || clash || imports.len() >= 16 {
            i += 1;
            continue;
        }
        imports.push((out[i].line, m.clone()));
        shift_lines(&mut mbody, imports.len() * SHIFT);
        done.insert(m, defs);
        // The module's own imports are looked at next, in place.
        out.splice(i..i + 1, mbody);
    }
    (!imports.is_empty()).then_some((out, imports))
}

fn check_body(body: &[Stmt]) -> Result<Required, (usize, String)> {
    let mut p = Prog {
        fns: vec![],
        top: HashMap::new(),
        classes: HashMap::new(),
        class_fields: HashMap::new(),
        methods: HashMap::new(),
        modules: HashSet::new(),
        fromimp: HashMap::new(),
        module_names: HashSet::new(),
        bound_other: HashSet::new(),
        dicts: HashSet::new(),
        regex: HashSet::new(),
        lambdas: HashMap::new(),
        fn_globals: HashSet::new(),
        kl: HashMap::new(),
        kf: HashMap::new(),
        kret: HashMap::new(),
        as_value: HashSet::new(),
        sums: vec![],
    };
    collect(body, None, None, &mut p);
    close_captures(&mut p);
    p.dicts = dict_only(body);
    p.regex = regex_only(body, p.modules.contains("re"));
    p.lambdas = lambda_names(body, &p);
    let rebound = rebound_globals(body, &p);
    kinds(body, &mut p);
    p.sums = vec![Sum::default(); p.fns.len()];
    // Summaries to a fixpoint (they only grow), then one pass that reports.
    for _ in 0..30 {
        let mut changed = false;
        for f in 0..p.fns.len() {
            let mut cx = Cx::new(&p, f, false);
            cx.run_fn();
            let s = cx.summary();
            if s != p.sums[f] {
                p.sums[f] = s;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut req = Required::default();
    let mut first: Option<(usize, String)> = None;
    let mut keep = |c: Option<(usize, String)>, first: &mut Option<(usize, String)>| {
        if let Some(c) = c {
            if first.as_ref().map_or(true, |f| c.0 < f.0) {
                *first = Some(c);
            }
        }
    };
    let mut global_edges: BTreeSet<Edge> = BTreeSet::new();
    let mut unseen_muts: Vec<(Pl, u8)> = vec![];
    for f in 0..p.fns.len() {
        let mut cx = Cx::new(&p, f, true);
        cx.run_fn();
        keep(cx.conflict.clone(), &mut first);
        req.muts.extend(cx.req_muts.drain());
        req.iters.extend(cx.req_iters.drain());
        req.marks.extend(cx.req_marks.iter().cloned());
        global_edges.extend(cx.global_edges.iter().cloned());
        if p.as_value.contains(&f) {
            let s = &p.sums[f];
            unseen_muts.extend(s.mutf.iter().map(|(k, d)| (Pl::F(k.clone()), *d)));
            unseen_muts.extend(s.mutg.iter().map(|(k, d)| (Pl::G(k.clone()), *d)));
        }
    }
    {
        let mut cx = Cx::new(&p, MODULE, true);
        cx.run_module(body);
        if std::env::var("BREVI_SHARE_DEBUG").is_ok() {
            eprintln!("module edges {}: {:?}", cx.st.len(), cx.st.iter().take(40).collect::<Vec<_>>());
            eprintln!("kinds {:?}", p.kl);
        }
        keep(cx.conflict.clone(), &mut first);
        req.muts.extend(cx.req_muts.drain());
        req.iters.extend(cx.req_iters.drain());
        req.marks.extend(cx.req_marks.iter().cloned());
        global_edges.extend(cx.global_edges.iter().cloned());
    }
    // A field or global changed by a callback (at times the proof cannot see) must share with nothing.
    for (pl, d) in &unseen_muts {
        if let Some(e) = global_edges.iter().find(|(a, da, b, db)| (a == pl && d >= da) || (b == pl && d >= db)) {
            let n = match pl {
                Pl::F(n) => format!(".{n}"),
                Pl::G(n) | Pl::L(n) => n.clone(),
                _ => "a value".into(),
            };
            let _ = e;
            keep(Some((1, format!("{n} shares a list with another name and is changed by a function passed on as a value"))), &mut first);
        }
    }
    if let Some(c) = first {
        return Err(c);
    }
    req.globals.extend(rebound);
    Ok(req)
}

// ---------------------------------------------------------------- program tables

fn collect<'a>(body: &'a [Stmt], parent: Option<usize>, class: Option<&str>, p: &mut Prog<'a>) {
    for s in body {
        match &s.st {
            St::Def(d) => {
                let id = p.fns.len();
                let typed = match (parent, class) {
                    (Some(pa), _) => format!("{}_{}", p.fns[pa].typed, d.name),
                    (None, Some(c)) => format!("{c}_{}", d.name),
                    (None, None) => d.name.clone(),
                };
                let (mut params, npos, star, kwstar) = param_names(&d.params);
                let mut globals = HashSet::new();
                let mut locals: HashSet<String> = params.iter().cloned().collect();
                assigned(&d.body, &mut locals, &mut globals);
                for g in &globals {
                    locals.remove(g);
                }
                // Names it reads from an enclosing function.
                let mut captured = vec![];
                if let Some(pa) = parent {
                    let mut used = HashSet::new();
                    mentioned_block(&d.body, &mut used);
                    let mut c: Vec<String> = used.into_iter().filter(|n| !locals.contains(n) && !globals.contains(n) && in_enclosing(p, pa, n)).collect();
                    c.sort();
                    captured = c;
                }
                params.extend(captured.iter().cloned());
                p.fns.push(FnInfo {
                    def: d,
                    typed,
                    class: if parent.is_none() { class.map(|c| c.to_string()) } else { None },
                    parent,
                    params,
                    npos,
                    star,
                    kwstar,
                    captured,
                    locals,
                    globals,
                    nested: HashMap::new(),
                    eligible: !d.is_async && d.decorators.is_empty(),
                });
                match (parent, class) {
                    (Some(pa), _) => {
                        p.fns[pa].nested.insert(d.name.clone(), id);
                    }
                    (None, Some(c)) => {
                        p.methods.entry(d.name.clone()).or_default().push(id);
                        if d.name == "__init__" {
                            p.classes.insert(c.to_string(), Some(id));
                        }
                    }
                    (None, None) => {
                        p.top.insert(d.name.clone(), id);
                    }
                }
                collect(&d.body, Some(id), None, p);
            }
            St::Class(c) if parent.is_none() && class.is_none() => {
                p.classes.entry(c.name.clone()).or_insert(None);
                let mut fields = vec![];
                for st in &c.body {
                    if let St::AnnAssign(Ex::Name(n), _, _, _) = &st.st {
                        fields.push(n.clone());
                    }
                }
                p.class_fields.insert(c.name.clone(), fields);
                collect(&c.body, None, Some(&c.name), p);
            }
            St::Class(c) => {
                // A class inside a function or class: its methods are followed as nested functions are not.
                p.classes.entry(c.name.clone()).or_insert(None);
                collect(&c.body, parent, Some(&c.name), p);
            }
            St::Import(v) => v.iter().for_each(|(m, a)| {
                p.modules.insert(a.clone().unwrap_or_else(|| m.split('.').next().unwrap_or(m).to_string()));
            }),
            St::ImportFrom(m, v) => v.iter().for_each(|(n, a)| {
                let root = m.trim_start_matches('.').split('.').next().unwrap_or(m).to_string();
                p.fromimp.insert(a.clone().unwrap_or_else(|| n.clone()), (if m.starts_with('.') { String::new() } else { root }, n.clone()));
            }),
            _ => {}
        }
        if parent.is_none() && class.is_none() {
            let mut l = HashSet::new();
            let mut g = HashSet::new();
            assigned(std::slice::from_ref(s), &mut l, &mut g);
            if !matches!(s.st, St::Import(_) | St::ImportFrom(..)) {
                p.bound_other.extend(l.iter().cloned());
            }
            p.module_names.extend(l);
        }
        if !matches!(s.st, St::Def(_) | St::Class(_)) {
            for b in child_blocks(s) {
                collect_inner(b, parent, class, p);
            }
        }
    }
    if parent.is_none() && class.is_none() {
        let mut used = HashSet::new();
        for f in &p.fns {
            let mut u = HashSet::new();
            mentioned_block(&f.def.body, &mut u);
            let mut own = HashSet::new();
            each_expr_block(&f.def.body, &mut |e| {
                walk_ex(e, &mut |x| match x {
                    Ex::Comp(_, _, _, fors) => fors.iter().for_each(|c| target_names(&c.target, &mut own)),
                    Ex::Lambda(ps, _) => ps.pos.iter().chain(ps.kwonly.iter()).for_each(|(n, _)| {
                        own.insert(n.clone());
                    }),
                    _ => {}
                })
            });
            for n in u {
                if own.contains(&n) && !f.globals.contains(&n) {
                    continue;
                }
                if !f.locals.contains(&n) && !f.params.contains(&n) || f.globals.contains(&n) {
                    used.insert(n);
                }
            }
            for g in &f.globals {
                used.insert(g.clone());
            }
        }
        p.fn_globals = used.into_iter().filter(|n| p.module_names.contains(n)).collect();
    }
}

// Defs inside if/for/try blocks of the same scope.
fn collect_inner<'a>(body: &'a [Stmt], parent: Option<usize>, class: Option<&str>, p: &mut Prog<'a>) {
    let has = body.iter().any(|s| matches!(s.st, St::Def(_) | St::Class(_) | St::Import(_)) || child_blocks_has_def(s));
    if has {
        // collect() also adds module names; the outer call already did that for this statement.
        let save = p.module_names.clone();
        let fg = p.fn_globals.clone();
        collect(body, parent, class, p);
        p.module_names.extend(save);
        p.fn_globals = fg;
    }
}

fn child_blocks_has_def(s: &Stmt) -> bool {
    let mut hit = false;
    for_child_blocks(s, &mut |b| {
        if b.iter().any(|s| matches!(s.st, St::Def(_) | St::Class(_) | St::Import(_)) || child_blocks_has_def(s)) {
            hit = true;
        }
    });
    hit
}

// A nested def that calls another (`expr()` calling `eat()`, which changes the enclosing `pos`) captures what
// that one captures, as the typed build passes it on.
fn close_captures(p: &mut Prog) {
    let mentions: Vec<HashSet<String>> = p.fns.iter().map(|f| {
        let mut u = HashSet::new();
        mentioned_block(&f.def.body, &mut u);
        u
    }).collect();
    loop {
        let mut add: Vec<(usize, String)> = vec![];
        for (i, f) in p.fns.iter().enumerate() {
            let Some(pa) = f.parent else { continue };
            for (j, g) in p.fns.iter().enumerate() {
                if i == j || !mentions[i].contains(&g.def.name) || !(g.parent == f.parent || g.parent == Some(i)) {
                    continue;
                }
                for c in &g.captured {
                    if !f.captured.contains(c) && !f.locals.contains(c) && !f.globals.contains(c) && in_enclosing(p, pa, c) {
                        add.push((i, c.clone()));
                    }
                }
            }
        }
        if add.is_empty() {
            return;
        }
        for (i, c) in add {
            if !p.fns[i].captured.contains(&c) {
                p.fns[i].captured.push(c.clone());
                p.fns[i].params.push(c);
            }
        }
    }
}

fn in_enclosing(p: &Prog, mut f: usize, n: &str) -> bool {
    loop {
        if p.fns[f].locals.contains(n) {
            return true;
        }
        match p.fns[f].parent {
            Some(pa) => f = pa,
            None => return false,
        }
    }
}

fn param_names(ps: &Params) -> (Vec<String>, usize, Option<String>, Option<String>) {
    let mut v: Vec<String> = ps.pos.iter().map(|(n, _)| n.clone()).collect();
    let npos = v.len();
    if let Some(s) = &ps.star {
        v.push(s.clone());
    }
    v.extend(ps.kwonly.iter().map(|(n, _)| n.clone()));
    if let Some(k) = &ps.kwstar {
        v.push(k.clone());
    }
    (v, npos, ps.star.clone(), ps.kwstar.clone())
}

// Names bound only to dicts anywhere in the file (`d = {}`, `d = dict(...)`, `d = {k: v for ...}`): a loop over one
// walks its keys, and setting the current key (`d[k] = v`) leaves the keys as they are.
fn dict_only(body: &[Stmt]) -> HashSet<String> {
    fn is_dict(v: &Ex) -> bool {
        match v {
            Ex::Dict(_) => true,
            Ex::Comp(c, ..) => *c == 'd',
            Ex::Call(f, _) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "dict" | "defaultdict" | "Counter" | "OrderedDict")),
            _ => false,
        }
    }
    only_bound_to(body, &is_dict)
}

// Names bound only to compiled patterns and their matches (`R = re.compile(...)`, `m = R.match(s)`,
// `m = re.search(...)`): their methods change nothing and keep nothing of the program's.
fn regex_only(body: &[Stmt], has_re: bool) -> HashSet<String> {
    if !has_re {
        return HashSet::new();
    }
    let mut known: HashSet<String> = HashSet::new();
    for _ in 0..4 {
        let k = known.clone();
        let is_re = move |v: &Ex| match v {
            Ex::Call(f, _) => match f.as_ref() {
                Ex::Attr(o, m) => {
                    (matches!(o.as_ref(), Ex::Name(r) if r == "re") && matches!(m.as_str(), "compile" | "match" | "fullmatch" | "search"))
                        || (matches!(o.as_ref(), Ex::Name(r) if k.contains(r)) && matches!(m.as_str(), "match" | "fullmatch" | "search"))
                }
                _ => false,
            },
            Ex::NoneV => true,
            _ => false,
        };
        let next = only_bound_to(body, &is_re);
        if next == known {
            break;
        }
        known = next;
    }
    known
}

fn only_bound_to(body: &[Stmt], is_dict: &dyn Fn(&Ex) -> bool) -> HashSet<String> {
    fn walk(b: &[Stmt], yes: &mut HashSet<String>, no: &mut HashSet<String>, is_dict: &dyn Fn(&Ex) -> bool) {
        for st in b {
            match &st.st {
                St::Assign(_, v) | St::AnnAssign(_, Some(v), _, _) => {
                    let ts: Vec<&Ex> = match &st.st {
                        St::Assign(ts, _) => ts.iter().collect(),
                        St::AnnAssign(t, _, _, _) => vec![t],
                        _ => vec![],
                    };
                    for t in ts {
                        match t {
                            Ex::Name(n) if is_dict(v) => {
                                yes.insert(n.clone());
                            }
                            t => target_names(t, no),
                        }
                    }
                }
                St::Def(d) => {
                    for (n, _) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                        no.insert(n.clone());
                    }
                    no.extend(d.params.star.clone());
                    no.extend(d.params.kwstar.clone());
                    no.insert(d.name.clone());
                    walk(&d.body, yes, no, is_dict);
                }
                St::Class(c) => {
                    no.insert(c.name.clone());
                    walk(&c.body, yes, no, is_dict);
                }
                St::Expr(_) | St::Return(_) | St::Raise(..) | St::Assert(..) | St::Pass | St::Break | St::Continue | St::Global(_) | St::Nonlocal(_) | St::Del(_) => {}
                _ => {
                    let mut g = HashSet::new();
                    assigned(std::slice::from_ref(st), no, &mut g);
                    for c in child_blocks(st) {
                        walk(c, yes, no, is_dict);
                    }
                }
            }
        }
    }
    let (mut yes, mut no) = (HashSet::new(), HashSet::new());
    walk(body, &mut yes, &mut no, is_dict);
    each_expr_block(body, &mut |e| {
        walk_ex(e, &mut |x| match x {
            Ex::Walrus(n, _) => {
                no.insert(n.clone());
            }
            Ex::Comp(_, _, _, fors) => fors.iter().for_each(|c| target_names(&c.target, &mut no)),
            Ex::Lambda(ps, _) => ps.pos.iter().chain(ps.kwonly.iter()).for_each(|(n, _)| {
                no.insert(n.clone());
            }),
            _ => {}
        })
    });
    yes.retain(|n| !no.contains(n));
    yes
}

// Per scope (a function's index, or MODULE), the names whose one binding there is `name = lambda ...`: a call of
// one is followed into the lambda's body (see Cx::call_in).
fn lambda_names<'a>(body: &'a [Stmt], p: &Prog<'a>) -> HashMap<(usize, String), &'a Ex> {
    fn walk<'a>(b: &'a [Stmt], lam: &mut HashMap<String, Vec<&'a Ex>>, other: &mut HashSet<String>) {
        for st in b {
            match &st.st {
                St::Assign(ts, v) if ts.len() == 1 && matches!(v, Ex::Lambda(..)) && matches!(ts[0], Ex::Name(_)) => {
                    let Ex::Name(n) = &ts[0] else { continue };
                    lam.entry(n.clone()).or_default().push(v);
                }
                St::Def(d) => {
                    other.insert(d.name.clone());
                }
                St::Class(c) => {
                    other.insert(c.name.clone());
                }
                St::If(..) | St::While(..) | St::For(..) | St::Try(..) | St::With(..) => {
                    // the statement's own bindings (loop targets, `as` names, walruses in its heads)
                    let mut l = HashSet::new();
                    let mut g = HashSet::new();
                    assigned(std::slice::from_ref(st), &mut l, &mut g);
                    let mut inner = HashMap::new();
                    for c in child_blocks(st) {
                        walk(c, &mut inner, other);
                    }
                    for (n, v) in inner {
                        l.remove(&n);
                        lam.entry(n).or_default().extend(v);
                    }
                    // what assigned() found that is not a lambda binding inside
                    other.extend(l);
                }
                _ => {
                    let mut g = HashSet::new();
                    assigned(std::slice::from_ref(st), other, &mut g);
                    other.extend(g);
                }
            }
        }
    }
    let mut out = HashMap::new();
    let mut scopes: Vec<(usize, &'a [Stmt], Vec<String>)> = vec![(MODULE, body, vec![])];
    for (i, f) in p.fns.iter().enumerate() {
        scopes.push((i, &f.def.body, f.params.clone()));
    }
    // a name some function sets as global or nonlocal is bound in more places than its own scope
    let mut shared = HashSet::new();
    for f in &p.fns {
        shared.extend(f.globals.iter().cloned());
        fn nonlocals(b: &[Stmt], out: &mut HashSet<String>) {
            for st in b {
                if let St::Nonlocal(v) = &st.st {
                    out.extend(v.iter().cloned());
                }
                for c in child_blocks(st) {
                    nonlocals(c, out);
                }
            }
        }
        nonlocals(&f.def.body, &mut shared);
    }
    for (id, b, params) in scopes {
        let mut lam = HashMap::new();
        let mut other: HashSet<String> = params.into_iter().collect();
        walk(b, &mut lam, &mut other);
        for (n, v) in lam {
            if v.len() == 1 && !other.contains(&n) && !shared.contains(&n) {
                out.insert((id, n), v[0]);
            }
        }
    }
    out
}

fn child_blocks(s: &Stmt) -> Vec<&[Stmt]> {
    let mut v: Vec<&[Stmt]> = vec![];
    match &s.st {
        St::If(arms, els) => {
            arms.iter().for_each(|(_, b)| v.push(b));
            if let Some(e) = els {
                v.push(e);
            }
        }
        St::While(_, b, e) | St::For(_, _, b, e) => {
            v.push(b);
            if let Some(e) = e {
                v.push(e);
            }
        }
        St::Try(b, hs, e, fin) => {
            v.push(b);
            hs.iter().for_each(|h| v.push(&h.body));
            if let Some(e) = e {
                v.push(e);
            }
            if let Some(x) = fin {
                v.push(x);
            }
        }
        St::With(_, b) => v.push(b),
        _ => {}
    }
    v
}

fn for_child_blocks(s: &Stmt, f: &mut dyn FnMut(&[Stmt])) {
    match &s.st {
        St::If(arms, els) => {
            arms.iter().for_each(|(_, b)| f(b));
            if let Some(e) = els {
                f(e);
            }
        }
        St::While(_, b, e) | St::For(_, _, b, e) => {
            f(b);
            if let Some(e) = e {
                f(e);
            }
        }
        St::Try(b, hs, e, fin) => {
            f(b);
            hs.iter().for_each(|h| f(&h.body));
            if let Some(e) = e {
                f(e);
            }
            if let Some(x) = fin {
                f(x);
            }
        }
        St::With(_, b) => f(b),
        _ => {}
    }
}

fn target_names(t: &Ex, out: &mut HashSet<String>) {
    match t {
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Tuple(xs) | Ex::List(xs) => xs.iter().for_each(|x| target_names(x, out)),
        Ex::Star(x) => target_names(x, out),
        _ => {}
    }
}

// Names a block binds (not inside nested defs), and its `global` names.
fn assigned(body: &[Stmt], out: &mut HashSet<String>, globals: &mut HashSet<String>) {
    for s in body {
        match &s.st {
            St::Assign(ts, v) => {
                ts.iter().for_each(|t| target_names(t, out));
                walrus(v, out);
            }
            St::AugAssign(t, _, v) | St::AnnAssign(t, Some(v), _, _) => {
                target_names(t, out);
                walrus(v, out);
            }
            St::AnnAssign(t, None, _, _) => target_names(t, out),
            St::For(t, it, _, _) => {
                target_names(t, out);
                walrus(it, out);
            }
            St::With(items, _) => items.iter().for_each(|(_, a)| {
                if let Some(a) = a {
                    target_names(a, out)
                }
            }),
            St::Try(_, hs, _, _) => hs.iter().for_each(|h| {
                if let Some(n) = &h.name {
                    out.insert(n.clone());
                }
            }),
            St::Def(d) => {
                out.insert(d.name.clone());
            }
            St::Class(c) => {
                out.insert(c.name.clone());
            }
            St::Import(v) => v.iter().for_each(|(m, a)| {
                out.insert(a.clone().unwrap_or_else(|| m.split('.').next().unwrap_or(m).to_string()));
            }),
            St::ImportFrom(_, v) => v.iter().for_each(|(n, a)| {
                out.insert(a.clone().unwrap_or_else(|| n.clone()));
            }),
            St::Global(v) | St::Nonlocal(v) => globals.extend(v.iter().cloned()),
            St::Del(ts) => ts.iter().for_each(|t| {
                if let Ex::Name(n) = t {
                    out.insert(n.clone());
                }
            }),
            St::Expr(e) | St::Return(Some(e)) => walrus(e, out),
            St::If(arms, _) => arms.iter().for_each(|(c, _)| walrus(c, out)),
            St::While(c, _, _) => walrus(c, out),
            _ => {}
        }
        for_child_blocks(s, &mut |b| assigned(b, out, globals));
    }
}

fn walrus(e: &Ex, out: &mut HashSet<String>) {
    walk_ex(e, &mut |x| {
        if let Ex::Walrus(n, _) = x {
            out.insert(n.clone());
        }
    });
}

// Every sub-expression (lambdas and comprehensions included).
fn walk_ex(e: &Ex, f: &mut dyn FnMut(&Ex)) {
    f(e);
    match e {
        Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => xs.iter().for_each(|x| walk_ex(x, f)),
        Ex::Dict(kv) => kv.iter().for_each(|(k, v)| {
            if let Some(k) = k {
                walk_ex(k, f);
            }
            walk_ex(v, f);
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::YieldFrom(x) | Ex::Await(x) | Ex::Walrus(_, x) => walk_ex(x, f),
        Ex::Yield(x) => {
            if let Some(x) = x {
                walk_ex(x, f)
            }
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            walk_ex(a, f);
            walk_ex(b, f);
        }
        Ex::Slice(a, b, c) => [a, b, c].iter().for_each(|x| {
            if let Some(x) = x {
                walk_ex(x, f)
            }
        }),
        Ex::Call(g, args) => {
            walk_ex(g, f);
            args.iter().for_each(|a| match a {
                Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => walk_ex(x, f),
            });
        }
        Ex::Cmp(a, rest) => {
            walk_ex(a, f);
            rest.iter().for_each(|(_, x)| walk_ex(x, f));
        }
        Ex::IfExp(a, b, c) => {
            walk_ex(a, f);
            walk_ex(b, f);
            walk_ex(c, f);
        }
        Ex::Lambda(ps, b) => {
            ps.pos.iter().chain(ps.kwonly.iter()).for_each(|(_, d)| {
                if let Some(d) = d {
                    walk_ex(d, f)
                }
            });
            walk_ex(b, f);
        }
        Ex::Comp(_, a, b, fors) => {
            walk_ex(a, f);
            if let Some(b) = b {
                walk_ex(b, f);
            }
            fors.iter().for_each(|c| {
                walk_ex(&c.target, f);
                walk_ex(&c.iter, f);
                c.ifs.iter().for_each(|x| walk_ex(x, f));
            });
        }
        Ex::FStr(parts) => fparts(parts, f),
        _ => {}
    }
}

fn fparts(parts: &[FPart], f: &mut dyn FnMut(&Ex)) {
    for p in parts {
        if let FPart::Expr(e, _, spec, _) = p {
            walk_ex(e, f);
            fparts(spec, f);
        }
    }
}

fn mentioned(e: &Ex, out: &mut HashSet<String>) {
    walk_ex(e, &mut |x| match x {
        Ex::Name(n) | Ex::Walrus(n, _) => {
            out.insert(n.clone());
        }
        _ => {}
    });
}

// Names a statement mentions, nested defs and classes included (their reads happen when they run).
fn mentioned_stmt(s: &Stmt, out: &mut HashSet<String>) {
    match &s.st {
        St::Expr(e) | St::Return(Some(e)) => mentioned(e, out),
        St::Assign(ts, v) => {
            ts.iter().for_each(|t| mentioned(t, out));
            mentioned(v, out);
        }
        St::AugAssign(t, _, v) => {
            mentioned(t, out);
            mentioned(v, out);
        }
        St::AnnAssign(t, v, _, _) => {
            mentioned(t, out);
            if let Some(v) = v {
                mentioned(v, out);
            }
        }
        St::Raise(a, b) => [a, b].iter().for_each(|x| {
            if let Some(x) = x {
                mentioned(x, out)
            }
        }),
        St::Del(ts) => ts.iter().for_each(|t| mentioned(t, out)),
        St::Assert(a, b) => {
            mentioned(a, out);
            if let Some(b) = b {
                mentioned(b, out);
            }
        }
        St::If(arms, _) => arms.iter().for_each(|(c, _)| mentioned(c, out)),
        St::While(c, _, _) => mentioned(c, out),
        St::For(t, it, _, _) => {
            mentioned(t, out);
            mentioned(it, out);
        }
        St::With(items, _) => items.iter().for_each(|(c, a)| {
            mentioned(c, out);
            if let Some(a) = a {
                mentioned(a, out);
            }
        }),
        St::Try(_, hs, _, _) => hs.iter().for_each(|h| {
            if let Some(t) = &h.types {
                mentioned(t, out);
            }
        }),
        St::Def(d) => {
            mentioned_block(&d.body, out);
            d.params.pos.iter().chain(d.params.kwonly.iter()).for_each(|(_, x)| {
                if let Some(x) = x {
                    mentioned(x, out)
                }
            });
            d.decorators.iter().for_each(|x| mentioned(x, out));
        }
        St::Class(c) => {
            mentioned_block(&c.body, out);
            c.bases.iter().for_each(|x| mentioned(x, out));
        }
        _ => {}
    }
    for_child_blocks(s, &mut |b| mentioned_block(b, out));
}

fn mentioned_block(b: &[Stmt], out: &mut HashSet<String>) {
    b.iter().for_each(|s| mentioned_stmt(s, out));
}

// A module-level name a function reads or sets, bound again at module level: the typed build keeps the
// function's own copy (c05).
fn rebound_globals(body: &[Stmt], p: &Prog) -> BTreeMap<String, usize> {
    let mut count: HashMap<String, (usize, usize)> = HashMap::new();
    fn bump(s: &Stmt, count: &mut HashMap<String, (usize, usize)>) {
        let mut l = HashSet::new();
        let mut g = HashSet::new();
        match &s.st {
            St::Def(_) | St::Class(_) | St::Import(_) | St::ImportFrom(..) => {}
            St::For(..) | St::While(..) | St::If(..) | St::Try(..) | St::With(..) => {
                assigned(std::slice::from_ref(s), &mut l, &mut g);
                // a binding inside a loop runs more than once
                for n in l {
                    let e = count.entry(n).or_insert((0, s.line));
                    e.0 += 2;
                }
                return;
            }
            St::AugAssign(..) => {
                assigned(std::slice::from_ref(s), &mut l, &mut g);
                for n in l {
                    let e = count.entry(n).or_insert((0, s.line));
                    e.0 += 2;
                    e.1 = s.line;
                }
                return;
            }
            _ => assigned(std::slice::from_ref(s), &mut l, &mut g),
        }
        for n in l {
            let e = count.entry(n).or_insert((0, s.line));
            e.0 += 1;
            if e.0 > 1 {
                e.1 = s.line;
            }
        }
    }
    for s in body {
        bump(s, &mut count);
    }
    count
        .into_iter()
        .filter(|(n, (c, _))| *c > 1 && p.fn_globals.contains(n) && !p.top.contains_key(n) && !p.classes.contains_key(n))
        .map(|(n, (_, l))| (n, l))
        .collect()
}

// ---------------------------------------------------------------- kinds

fn kinds(body: &[Stmt], p: &mut Prog) {
    // Functions used other than as a call's function: their parameters' kinds are unknown.
    let mut value_uses: HashSet<String> = HashSet::new();
    let mut visit = |b: &[Stmt]| {
        let mut all = vec![];
        each_expr_block(b, &mut |e| all.push(e.clone()));
        for e in all {
            value_names(&e, &mut value_uses);
        }
    };
    visit(body);
    for f in &p.fns {
        visit(&f.def.body);
    }
    for (i, f) in p.fns.iter().enumerate() {
        // methods: `obj.m` read without a call
        if value_uses.contains(&f.def.name) {
            p.as_value.insert(i);
        }
    }
    // Kinds grow from nothing (0) to a fixpoint; what no part of the program sets is unknown from the start:
    // a field never set by the program, a parameter of a function nothing calls directly.
    let mut called: HashSet<String> = HashSet::new();
    let mut set_fields: HashSet<String> = HashSet::new();
    let mut scan = |b: &[Stmt]| {
        let mut all = vec![];
        each_expr_block(b, &mut |e| all.push(e.clone()));
        for st in b {
            fn targets(s: &Stmt, out: &mut HashSet<String>) {
                let mut put = |t: &Ex| {
                    walk_ex(t, &mut |x| {
                        if let Ex::Attr(_, f) = x {
                            out.insert(f.clone());
                        }
                    })
                };
                match &s.st {
                    St::Assign(ts, _) => ts.iter().for_each(|t| put(t)),
                    St::AugAssign(t, _, _) | St::AnnAssign(t, _, _, _) => put(t),
                    St::For(t, _, _, _) => put(t),
                    _ => {}
                }
                for_child_blocks(s, &mut |c| c.iter().for_each(|s| targets(s, out)));
            }
            targets(st, &mut set_fields);
        }
        for e in all {
            walk_ex(&e, &mut |x| {
                if let Ex::Call(f, _) = x {
                    match f.as_ref() {
                        Ex::Name(n) => {
                            called.insert(n.clone());
                        }
                        Ex::Attr(_, m) => {
                            called.insert(m.clone());
                        }
                        _ => {}
                    }
                }
            });
        }
    };
    scan(body);
    for f in &p.fns {
        scan(&f.def.body);
    }
    fn class_fields(b: &[Stmt], out: &mut HashSet<String>) {
        for s in b {
            if let St::Class(c) = &s.st {
                for st in &c.body {
                    match &st.st {
                        St::Assign(ts, _) => ts.iter().for_each(|t| {
                            if let Ex::Name(n) = t {
                                out.insert(n.clone());
                            }
                        }),
                        St::AnnAssign(Ex::Name(n), _, _, _) => {
                            out.insert(n.clone());
                        }
                        _ => {}
                    }
                }
                class_fields(&c.body, out);
            }
            for_child_blocks(s, &mut |c| class_fields(c, out));
        }
    }
    class_fields(body, &mut set_fields);
    for f in &p.fns {
        class_fields(&f.def.body, &mut set_fields);
    }
    for fl in set_fields {
        p.kf.insert(fl, K::S);
    }
    for (i, f) in p.fns.iter().enumerate() {
        let direct = called.contains(&f.def.name) || (f.def.name == "__init__" && f.class.as_ref().map_or(false, |c| called.contains(c)));
        let start = if direct && !p.as_value.contains(&i) { K::S } else { K::U };
        for n in &f.params {
            p.kl.insert((i, n.clone()), start.clone());
        }
        // a method's self is an object
        if f.class.is_some() {
            if let Some(s0) = f.params.first() {
                p.kl.insert((i, s0.clone()), K::S);
            }
        }
    }
    for _ in 0..40 {
        let before = (p.kl.clone(), p.kf.clone(), p.kret.clone());
        let mut k = KCx { p, f: MODULE };
        k.block(body);
        for f in 0..k.p.fns.len() {
            k.f = f;
            let d = k.p.fns[f].def;
            // self of a method is an object

            for (n, dflt) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                if let Some(x) = dflt {
                    let kd = k.kind(x);
                    k.set_local(f, n, kd);
                }
            }
            if let Some(s) = &d.params.star {
                k.set_local(f, s, K::list(K::S));
            }
            if let Some(s) = &d.params.kwstar {
                k.set_local(f, s, K::list(K::S));
            }
            k.block(&d.body);
        }
        if before == (k.p.kl.clone(), k.p.kf.clone(), k.p.kret.clone()) {
            return;
        }
    }
    // No fixpoint: every kind becomes unknown.
    for v in p.kl.values_mut() {
        *v = K::U;
    }
    for v in p.kf.values_mut() {
        *v = K::U;
    }
    for v in p.kret.values_mut() {
        *v = K::U;
    }
    p.as_value.extend(0..p.fns.len());
}

fn value_names(e: &Ex, out: &mut HashSet<String>) {
    match e {
        Ex::Call(f, args) => {
            match f.as_ref() {
                Ex::Name(_) => {}
                Ex::Attr(o, _) => value_names(o, out),
                g => value_names(g, out),
            }
            args.iter().for_each(|a| match a {
                Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => value_names(x, out),
            });
        }
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Attr(o, n) => {
            out.insert(n.clone());
            value_names(o, out);
        }
        _ => {
            // children
            let mut kids: Vec<&Ex> = vec![];
            children(e, &mut kids);
            kids.into_iter().for_each(|k| value_names(k, out));
        }
    }
}

fn children<'a>(e: &'a Ex, out: &mut Vec<&'a Ex>) {
    match e {
        Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => out.extend(xs.iter()),
        Ex::Dict(kv) => kv.iter().for_each(|(k, v)| {
            if let Some(k) = k {
                out.push(k);
            }
            out.push(v);
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::YieldFrom(x) | Ex::Await(x) | Ex::Walrus(_, x) => out.push(x),
        Ex::Yield(Some(x)) => out.push(x),
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            out.push(a);
            out.push(b);
        }
        Ex::Slice(a, b, c) => [a, b, c].into_iter().flatten().for_each(|x| out.push(x)),
        Ex::Call(g, args) => {
            out.push(g);
            args.iter().for_each(|a| match a {
                Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => out.push(x),
            });
        }
        Ex::Cmp(a, rest) => {
            out.push(a);
            rest.iter().for_each(|(_, x)| out.push(x));
        }
        Ex::IfExp(a, b, c) => {
            out.push(a);
            out.push(b);
            out.push(c);
        }
        Ex::Lambda(_, b) => out.push(b),
        Ex::Comp(_, a, b, fors) => {
            out.push(a);
            if let Some(b) = b {
                out.push(b);
            }
            fors.iter().for_each(|c| {
                out.push(&c.iter);
                c.ifs.iter().for_each(|x| out.push(x));
            });
        }
        Ex::FStr(parts) => {
            fn fp<'a>(parts: &'a [FPart], out: &mut Vec<&'a Ex>) {
                for p in parts {
                    if let FPart::Expr(e, _, spec, _) = p {
                        out.push(e);
                        fp(spec, out);
                    }
                }
            }
            fp(parts, out)
        }
        _ => {}
    }
}

// Every top expression of a block's statements (not inside nested defs).
fn each_expr_block(b: &[Stmt], f: &mut dyn FnMut(&Ex)) {
    for s in b {
        match &s.st {
            St::Expr(e) | St::Return(Some(e)) => f(e),
            St::Assign(ts, v) => {
                ts.iter().for_each(|t| f(t));
                f(v);
            }
            St::AugAssign(t, _, v) => {
                f(t);
                f(v);
            }
            St::AnnAssign(t, v, _, _) => {
                f(t);
                if let Some(v) = v {
                    f(v);
                }
            }
            St::If(arms, _) => arms.iter().for_each(|(c, _)| f(c)),
            St::While(c, _, _) => f(c),
            St::For(t, it, _, _) => {
                f(t);
                f(it);
            }
            St::With(items, _) => items.iter().for_each(|(c, _)| f(c)),
            St::Raise(Some(e), _) => f(e),
            St::Del(ts) => ts.iter().for_each(|t| f(t)),
            St::Assert(a, _) => f(a),
            St::Def(d) => {
                d.decorators.iter().for_each(|x| f(x));
                d.params.pos.iter().chain(d.params.kwonly.iter()).for_each(|(_, x)| {
                    if let Some(x) = x {
                        f(x)
                    }
                });
            }
            St::Class(c) => {
                c.bases.iter().for_each(|x| f(x));
                c.decorators.iter().for_each(|x| f(x));
                // class-level attributes
                each_expr_block(&c.body.iter().filter(|s| !matches!(s.st, St::Def(_))).cloned().collect::<Vec<_>>(), f);
            }
            _ => {}
        }
        if !matches!(s.st, St::Def(_) | St::Class(_)) {
            for_child_blocks(s, &mut |b| each_expr_block(b, f));
        }
    }
}

// A value's shape, as far as sharing goes: scalars (numbers, strings, objects, functions: no list inside
// that a copy would lose), lists, sets and dicts by their items' shape, tuples item by item, or unknown.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum K {
    S,
    U,
    L(Box<K>),
    T(Vec<K>),
}

const KMAX: u8 = 6;

impl K {
    fn depth(&self) -> u8 {
        match self {
            K::S => 0,
            K::U => UNK,
            K::L(x) => add_d(x.depth(), 1),
            K::T(xs) => add_d(xs.iter().map(|x| x.depth()).max().unwrap_or(0), 1),
        }
    }
    fn elem(&self) -> K {
        match self {
            K::S => K::S,
            K::U => K::U,
            K::L(x) => (**x).clone(),
            K::T(xs) => xs.iter().fold(K::S, |a, b| a.join(b)),
        }
    }
    fn item(&self, i: Option<i64>) -> K {
        match (self, i) {
            (K::T(xs), Some(i)) => {
                let i = if i < 0 { xs.len() as i64 + i } else { i };
                xs.get(i as usize).cloned().unwrap_or(K::S)
            }
            _ => self.elem(),
        }
    }
    fn list(x: K) -> K {
        K::L(Box::new(x)).capped()
    }
    fn capped(self) -> K {
        if self.depth() > KMAX && self.depth() != UNK {
            K::U
        } else {
            self
        }
    }
    fn join(&self, o: &K) -> K {
        match (self, o) {
            (K::S, x) | (x, K::S) => x.clone(),
            (K::U, _) | (_, K::U) => K::U,
            (K::L(a), K::L(b)) => K::list(a.join(b)),
            (K::T(a), K::T(b)) if a.len() == b.len() => K::T(a.iter().zip(b).map(|(x, y)| x.join(y)).collect()).capped(),
            (K::T(_), _) | (_, K::T(_)) | (K::L(_), _) => K::list(self.elem().join(&o.elem())),
        }
    }
}

fn dec(k: u8) -> u8 {
    if k == UNK {
        UNK
    } else {
        k.saturating_sub(1)
    }
}

fn lit_int(e: &Ex) -> Option<i64> {
    match e {
        Ex::Int(s) => s.parse().ok(),
        Ex::Unary("-", x) => lit_int(x).map(|v| -v),
        _ => None,
    }
}

struct KCx<'p, 'a> {
    p: &'p mut Prog<'a>,
    f: usize,
}

const BUILTIN_SCALAR: &[&str] = &[
    "len", "print", "str", "int", "float", "bool", "abs", "round", "ord", "chr", "hash", "id", "repr", "format", "isinstance", "issubclass", "callable", "sum", "any", "all", "hex", "bin", "oct", "divmod", "pow", "input", "type", "hasattr", "open", "ascii", "bytes", "bytearray", "complex", "super", "object", "Exception", "ValueError", "KeyError", "IndexError", "TypeError", "RuntimeError", "StopIteration", "ZeroDivisionError", "AttributeError", "NotImplementedError", "AssertionError", "OSError", "IOError", "ArithmeticError", "LookupError", "OverflowError", "exit", "quit", "vars", "globals", "locals", "setattr", "delattr",
];
const BUILTIN_COPY: &[&str] = &["list", "tuple", "sorted", "set", "frozenset", "reversed", "dict"];

fn put(m: &mut HashMap<(usize, String), K>, key: (usize, String), k: K) {
    let cur = m.get(&key).cloned().unwrap_or(K::S);
    let j = cur.join(&k);
    if j != cur || !m.contains_key(&key) {
        m.insert(key, j);
    }
}

impl<'p, 'a> KCx<'p, 'a> {
    fn scope_of(&self, n: &str) -> usize {
        let mut f = self.f;
        while f != MODULE {
            let fi = &self.p.fns[f];
            if fi.globals.contains(n) {
                return MODULE;
            }
            if fi.locals.contains(n) || fi.params.iter().any(|x| x == n) {
                return f;
            }
            match fi.parent {
                Some(pa) => f = pa,
                None => return MODULE,
            }
        }
        MODULE
    }
    fn set_local(&mut self, f: usize, n: &str, k: K) {
        put(&mut self.p.kl, (f, n.to_string()), k);
    }
    fn set_field(&mut self, n: &str, k: K) {
        let cur = self.p.kf.get(n).cloned().unwrap_or(K::S);
        self.p.kf.insert(n.to_string(), cur.join(&k));
    }
    fn set_name(&mut self, n: &str, k: K) {
        let s = self.scope_of(n);
        self.set_local(s, n, k);
    }
    fn name_kind(&self, n: &str) -> K {
        let s = self.scope_of(n);
        if let Some(k) = self.p.kl.get(&(s, n.to_string())) {
            return k.clone();
        }
        K::S
    }
    fn assign(&mut self, t: &Ex, k: K) {
        match t {
            Ex::Name(n) => self.set_name(n, k),
            Ex::Tuple(xs) | Ex::List(xs) => {
                let star = xs.iter().any(|x| matches!(x, Ex::Star(_)));
                for (i, x) in xs.iter().enumerate() {
                    match x {
                        Ex::Star(y) => self.assign(y, K::list(k.elem())),
                        _ => {
                            let e = if star { k.elem() } else { k.item(Some(i as i64)) };
                            self.assign(x, e)
                        }
                    }
                }
            }
            Ex::Attr(o, f) if !self.is_module(o) => self.set_field(f, k),
            Ex::Index(c, _) => self.deepen(c, K::list(k)),
            Ex::Star(y) => self.assign(y, K::list(k)),
            _ => {}
        }
    }
    // The container c holds a value of shape k (as its whole shape: `x[i] = v` gives x the shape [v]).
    fn deepen(&mut self, c: &Ex, k: K) {
        match c {
            Ex::Name(n) => self.set_name(n, k),
            Ex::Attr(o, f) if !self.is_module(o) => self.set_field(f, k),
            Ex::Index(cc, _) => self.deepen(cc, K::list(k)),
            _ => {}
        }
    }
    fn is_module(&self, e: &Ex) -> bool {
        matches!(e, Ex::Name(n) if self.p.modules.contains(n) && self.scope_of(n) == MODULE)
    }
    fn block(&mut self, b: &[Stmt]) {
        for s in b {
            match &s.st {
                St::Assign(ts, v) => {
                    let k = self.kind(v);
                    ts.iter().for_each(|t| self.assign(t, k.clone()));
                }
                St::AnnAssign(t, Some(v), _, _) => {
                    let k = self.kind(v);
                    self.assign(t, k);
                }
                St::AugAssign(t, op, v) => {
                    let k = self.kind(v);
                    let kt = self.kind(t);
                    if matches!(op.trim_end_matches('='), "+" | "|" | "*") {
                        self.assign(t, k.join(&kt));
                    }
                }
                St::For(t, it, _, _) => {
                    let k = self.kind(it);
                    self.assign(t, k.elem());
                }
                St::With(items, _) => {
                    for (c, a) in items {
                        let k = self.kind(c);
                        if let Some(a) = a {
                            self.assign(a, k);
                        }
                    }
                }
                St::Expr(e) => {
                    self.kind(e);
                }
                St::Return(Some(e)) => {
                    let k = self.kind(e);
                    if self.f != MODULE {
                        let cur = self.p.kret.get(&self.f).cloned().unwrap_or(K::S);
                        self.p.kret.insert(self.f, cur.join(&k));
                    }
                }
                St::If(arms, _) => arms.iter().for_each(|(c, _)| {
                    self.kind(c);
                }),
                St::While(c, _, _) => {
                    self.kind(c);
                }
                St::Del(ts) => ts.iter().for_each(|t| {
                    self.kind(t);
                }),
                St::Class(c) => {
                    for st in &c.body {
                        match &st.st {
                            St::Assign(ts, v) => {
                                let k = self.kind(v);
                                for t in ts {
                                    if let Ex::Name(n) = t {
                                        self.set_field(n, k.clone());
                                    }
                                }
                            }
                            St::AnnAssign(Ex::Name(n), v, _, _) => {
                                // a dataclass field: its value comes from the constructor's arguments (see call)
                                let k = v.as_ref().map_or(K::S, |v| self.kind(v));
                                self.set_field(n, k);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            if !matches!(s.st, St::Def(_) | St::Class(_)) {
                for_child_blocks(s, &mut |b| self.block(b));
            }
            if let St::Try(_, hs, _, _) = &s.st {
                for h in hs {
                    if let Some(n) = &h.name {
                        self.set_name(n, K::S);
                    }
                }
            }
        }
    }
    fn bind_args(&mut self, f: usize, args: &[Arg], skip_self: bool) {
        let fi_params = self.p.fns[f].params.clone();
        let npos = self.p.fns[f].npos;
        let mut i = if skip_self { 1 } else { 0 };
        for a in args {
            match a {
                Arg::Pos(x) => {
                    let k = self.kind(x);
                    if i < npos {
                        self.set_local(f, &fi_params[i], k);
                        // and back: a parameter the function deepens (`store.append(xs)`) deepens the argument
                        let kp = self.p.kl.get(&(f, fi_params[i].clone())).cloned().unwrap_or(K::S);
                        self.deepen(x, kp);
                    } else if let Some(s) = self.p.fns[f].star.clone() {
                        self.set_local(f, &s, K::list(k));
                    }
                    i += 1;
                }
                Arg::Kw(n, x) => {
                    let k = self.kind(x);
                    if fi_params.iter().any(|p| p == n) {
                        self.set_local(f, n, k);
                        let kp = self.p.kl.get(&(f, n.clone())).cloned().unwrap_or(K::S);
                        self.deepen(x, kp);
                    } else if let Some(s) = self.p.fns[f].kwstar.clone() {
                        self.set_local(f, &s, K::list(k));
                    }
                }
                Arg::Star(x) | Arg::KwStar(x) => {
                    let k = self.kind(x).elem();
                    for p in fi_params.iter().skip(if skip_self { 1 } else { 0 }) {
                        self.set_local(f, p, K::list(k.clone()).join(&k));
                    }
                }
            }
        }
    }
    fn kind(&mut self, e: &Ex) -> K {
        match e {
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False | Ex::Ellipsis => K::S,
            Ex::FStr(parts) => {
                let mut v = vec![];
                fparts(parts, &mut |x| v.push(x.clone()));
                v.iter().for_each(|x| {
                    self.kind(x);
                });
                K::S
            }
            Ex::Tuple(xs) if !xs.iter().any(|x| matches!(x, Ex::Star(_))) => K::T(xs.iter().map(|x| self.kind(x)).collect()).capped(),
            Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => {
                let mut m = K::S;
                for x in xs {
                    let k = match x {
                        Ex::Star(y) => self.kind(y).elem(),
                        _ => self.kind(x),
                    };
                    m = m.join(&k);
                }
                K::list(m)
            }
            Ex::Dict(kv) => {
                let mut m = K::S;
                for (k, v) in kv {
                    if let Some(k) = k {
                        self.kind(k);
                        m = m.join(&self.kind(v));
                    } else {
                        m = m.join(&self.kind(v).elem());
                    }
                }
                K::list(m)
            }
            Ex::Name(n) => self.name_kind(n),
            Ex::Attr(o, f) => {
                if self.is_module(o) {
                    return K::S;
                }
                self.kind(o);
                self.p.kf.get(f).cloned().unwrap_or(K::U)
            }
            Ex::Index(x, k) => {
                let kx = self.kind(x);
                if let Ex::Slice(..) = k.as_ref() {
                    self.kind(k);
                    return match kx {
                        K::T(_) => K::list(kx.elem()),
                        _ => kx,
                    };
                }
                self.kind(k);
                kx.item(lit_int(k))
            }
            Ex::Slice(a, b, c) => {
                [a, b, c].into_iter().flatten().for_each(|x| {
                    self.kind(x);
                });
                K::S
            }
            Ex::Star(x) => self.kind(x),
            Ex::Bin(op, a, b) => {
                let (ka, kb) = (self.kind(a), self.kind(b));
                match *op {
                    "+" => match (&ka, &kb) {
                        (K::T(x), K::T(y)) => K::T(x.iter().chain(y.iter()).cloned().collect()).capped(),
                        _ => ka.join(&kb),
                    },
                    // `[0] * n`: n is an int (see eval)
                    "*" if !self.p.methods.contains_key("__mul__") && !self.p.methods.contains_key("__rmul__") && matches!(a.as_ref(), Ex::List(_) | Ex::Tuple(_) | Ex::Comp('l', ..)) => ka,
                    "*" if !self.p.methods.contains_key("__mul__") && !self.p.methods.contains_key("__rmul__") && matches!(b.as_ref(), Ex::List(_) | Ex::Tuple(_) | Ex::Comp('l', ..)) => kb,
                    "*" | "|" | "&" | "-" | "^" => match (&ka, &kb) {
                        (K::T(_), _) | (_, K::T(_)) => K::list(ka.elem().join(&kb.elem())),
                        _ => ka.join(&kb),
                    },
                    _ => K::S,
                }
            }
            Ex::Unary(_, x) => {
                self.kind(x);
                K::S
            }
            Ex::And(a, b) | Ex::Or(a, b) => {
                let ka = self.kind(a);
                ka.join(&self.kind(b))
            }
            Ex::Cmp(a, rest) => {
                self.kind(a);
                rest.iter().for_each(|(_, x)| {
                    self.kind(x);
                });
                K::S
            }
            Ex::IfExp(c, a, b) => {
                self.kind(c);
                let ka = self.kind(a);
                ka.join(&self.kind(b))
            }
            Ex::Lambda(_, b) => {
                self.kind(b);
                K::S
            }
            Ex::Comp(c, elt, val, fors) => {
                for cf in fors {
                    let k = self.kind(&cf.iter);
                    self.assign(&cf.target, k.elem());
                    cf.ifs.iter().for_each(|x| {
                        self.kind(x);
                    });
                }
                let ke = self.kind(elt);
                let kv = val.as_ref().map_or(K::S, |v| self.kind(v));
                if *c == 'd' {
                    K::list(kv)
                } else {
                    K::list(ke)
                }
            }
            Ex::Walrus(n, v) => {
                let k = self.kind(v);
                self.set_name(n, k.clone());
                k
            }
            Ex::Yield(x) => {
                if let Some(x) = x {
                    let k = self.kind(x);
                    if self.f != MODULE {
                        let cur = self.p.kret.get(&self.f).cloned().unwrap_or(K::S);
                        self.p.kret.insert(self.f, cur.join(&K::list(k)));
                    }
                }
                K::U
            }
            Ex::YieldFrom(x) => {
                let k = self.kind(x);
                if self.f != MODULE {
                    let cur = self.p.kret.get(&self.f).cloned().unwrap_or(K::S);
                    self.p.kret.insert(self.f, cur.join(&k));
                }
                K::U
            }
            Ex::Await(x) => {
                self.kind(x);
                K::U
            }
            Ex::Call(f, args) => self.call_kind(f, args),
        }
    }
    fn arg_kinds(&mut self, args: &[Arg]) -> Vec<K> {
        args.iter()
            .map(|a| match a {
                Arg::Pos(x) | Arg::Kw(_, x) => self.kind(x),
                Arg::Star(x) | Arg::KwStar(x) => self.kind(x).elem(),
            })
            .collect()
    }
    fn resolve(&self, n: &str) -> Option<usize> {
        let mut f = self.f;
        while f != MODULE {
            if let Some(id) = self.p.fns[f].nested.get(n) {
                return Some(*id);
            }
            if self.p.fns[f].locals.contains(n) {
                return None;
            }
            match self.p.fns[f].parent {
                Some(pa) => f = pa,
                None => break,
            }
        }
        self.p.top.get(n).copied()
    }
    fn call_kind(&mut self, f: &Ex, args: &[Arg]) -> K {
        match f {
            Ex::Name(n) => {
                if let Some(id) = self.resolve(n) {
                    self.bind_args(id, args, false);
                    return self.p.kret.get(&id).cloned().unwrap_or(K::S);
                }
                if let Some(init) = self.p.classes.get(n).cloned() {
                    if let Some(id) = init {
                        self.bind_args(id, args, true);
                    } else {
                        let fields = self.p.class_fields.get(n).cloned().unwrap_or_default();
                        let ks = self.arg_kinds(args);
                        for (i, a) in args.iter().enumerate() {
                            let fname = match a {
                                Arg::Pos(_) => fields.get(i).cloned(),
                                Arg::Kw(k, _) => Some(k.clone()),
                                _ => None,
                            };
                            if let Some(fname) = fname {
                                self.set_field(&fname, ks[i].clone());
                            } else {
                                for fl in &fields {
                                    self.set_field(fl, ks[i].clone());
                                }
                            }
                        }
                    }
                    return K::S;
                }
                let ks = self.arg_kinds(args);
                let k0 = ks.first().cloned().unwrap_or(K::S);
                let s = self.scope_of(n);
                if s == MODULE {
                    if let Some((mo, nm)) = self.p.fromimp.get(n).cloned() {
                        return module_fn_kind(&mo, &nm, &ks, &k0);
                    }
                }
                if s != MODULE || self.p.module_names.contains(n.as_str()) {
                    return K::U; // a local holding a function, or a module-level name
                }
                match n.as_str() {
                    "range" => K::list(K::S),
                    "enumerate" => K::list(K::T(vec![K::S, k0.elem()])),
                    "zip" => K::list(K::T(ks.iter().map(|k| k.elem()).collect())),
                    "min" | "max" | "next" => {
                        let pos: Vec<K> = args.iter().zip(ks.iter()).filter(|(a, _)| matches!(a, Arg::Pos(_))).map(|(_, k)| k.clone()).collect();
                        if pos.len() == 1 {
                            pos[0].elem()
                        } else {
                            pos.iter().fold(K::S, |a, b| a.join(b))
                        }
                    }
                    "iter" => k0,
                    "map" => match args.first() {
                        Some(Arg::Pos(Ex::Name(f))) if BUILTIN_SCALAR.contains(&f.as_str()) && self.scope_of(f) == MODULE && !self.p.module_names.contains(f) => K::list(K::S),
                        _ => K::U,
                    },
                    "filter" => ks.get(1).cloned().unwrap_or(K::U),
                    "getattr" => K::U,
                    "divmod" => K::T(vec![K::S, K::S]),
                    "dict" => K::list(ks.iter().fold(K::S, |a, b| a.join(&b.elem()))),
                    _ if BUILTIN_COPY.contains(&n.as_str()) => K::list(ks.iter().fold(K::S, |a, b| a.join(&b.elem()))),
                    _ if BUILTIN_SCALAR.contains(&n.as_str()) => K::S,
                    _ => K::U,
                }
            }
            Ex::Attr(o, m) => {
                if let Ex::Name(mo) = o.as_ref() {
                    if self.is_module(o) {
                        let ks = self.arg_kinds(args);
                        let k0 = ks.first().cloned().unwrap_or(K::S);
                        return module_fn_kind(mo, m, &ks, &k0);
                    }
                }
                let ko = self.kind(o);
                let ks = self.arg_kinds(args);
                let mut k = K::S;
                let mut known = false;
                if let Some(ids) = self.p.methods.get(m).cloned() {
                    known = true;
                    for id in ids {
                        self.bind_args(id, args, true);
                        k = k.join(&self.p.kret.get(&id).cloned().unwrap_or(K::S));
                    }
                }
                if ko != K::S || !known {
                    let k0 = ks.first().cloned().unwrap_or(K::S);
                    let last = ks.last().cloned().unwrap_or(K::S);
                    let str_like = matches!(o.as_ref(), Ex::Str(_) | Ex::FStr(_)) || (ko == K::S && !known);
                    let c = match m.as_str() {
                        "split" | "rsplit" | "splitlines" | "keys" | "findall" => K::list(K::S),
                        "partition" | "rpartition" => K::T(vec![K::S, K::S, K::S]),
                        "pop" | "popleft" | "get" | "setdefault" => ko.elem().join(ks.get(1).unwrap_or(&K::S)),
                        "popitem" => K::T(vec![K::S, ko.elem()]),
                        "copy" => ko.clone(),
                        "values" => K::list(ko.elem()),
                        "items" => K::list(K::T(vec![K::S, ko.elem()])),
                        "append" | "add" | "appendleft" | "insert" => {
                            self.deepen(o, K::list(last));
                            K::S
                        }
                        "extend" | "update" | "extendleft" => {
                            self.deepen(o, K::list(k0.elem()));
                            K::S
                        }
                        "most_common" => K::list(K::T(vec![K::S, K::S])),
                        "union" | "intersection" | "difference" | "symmetric_difference" => K::list(K::S),
                        "count" | "index" | "find" | "rfind" | "join" | "strip" | "lstrip" | "rstrip" | "lower" | "upper" | "replace" | "startswith" | "endswith" | "format" | "isdigit" | "isalpha" | "issubset" | "issuperset" | "isdisjoint" | "sort" | "reverse" | "clear" | "remove" | "discard" | "elements" | "total" | "title" | "capitalize" | "encode" | "decode" | "zfill" | "center" | "ljust" | "rjust" | "isalnum" | "isspace" | "isupper" | "islower" | "casefold" | "swapcase" | "isnumeric" | "isdecimal" | "removeprefix" | "removesuffix" | "bit_length" | "is_integer" | "hex" | "group" | "start" | "end" | "span" | "write" | "read" | "readline" | "close" | "strftime" | "isoformat" | "toordinal" | "weekday" | "days" | "total_seconds" => K::S,
                        "groups" | "readlines" => K::list(K::S),
                        _ if str_like => K::U,
                        _ if ko == K::S && known => K::S,
                        _ => K::U,
                    };
                    k = k.join(&c);
                }
                k
            }
            g => {
                self.kind(g);
                self.arg_kinds(args);
                K::U
            }
        }
    }
}

fn module_fn_kind(mo: &str, m: &str, ks: &[K], k0: &K) -> K {
    match (mo, m) {
        ("json", "loads" | "load") => K::U,
        ("re", "findall" | "split") => K::list(K::U.join(&K::S)),
        ("math" | "statistics" | "time" | "json" | "string" | "operator" | "os" | "sys" | "datetime" | "decimal" | "fractions" | "struct" | "hashlib" | "base64" | "re", _) => K::S,
        ("random", "choice") => k0.elem(),
        ("random", "sample" | "choices") => K::list(k0.elem()),
        ("random", _) => K::S,
        ("heapq", "heappop" | "heapreplace" | "heappushpop") => k0.elem(),
        ("heapq", "nsmallest" | "nlargest") => K::list(ks.get(1).map_or(K::U, |k| k.elem())),
        ("heapq", "merge") => K::list(ks.iter().fold(K::S, |a, b| a.join(&b.elem()))),
        ("heapq" | "bisect", _) => K::S,
        ("copy", _) => k0.clone(),
        ("collections", "deque" | "Counter" | "OrderedDict") => K::list(k0.elem()),
        ("collections", "defaultdict") => K::U,
        ("itertools", "count" | "repeat" | "cycle") => K::U,
        ("itertools", _) => K::list(K::T(ks.iter().map(|k| k.elem()).collect())).join(&K::list(ks.iter().fold(K::S, |a, b| a.join(&b.elem())))),
        _ => K::U,
    }
}

// ---------------------------------------------------------------- liveness

struct Live {
    out: HashMap<usize, HashSet<String>>, // statement address -> names that may be read after it
}

fn sid(s: &Stmt) -> usize {
    s as *const Stmt as usize
}

impl Live {
    fn block(&mut self, b: &[Stmt], out: &HashSet<String>) -> HashSet<String> {
        let mut cur = out.clone();
        for s in b.iter().rev() {
            cur = self.stmt(s, &cur);
        }
        cur
    }
    fn stmt(&mut self, s: &Stmt, out: &HashSet<String>) -> HashSet<String> {
        {
            let e = self.out.entry(sid(s)).or_default();
            e.extend(out.iter().cloned());
        }
        let mut uses = HashSet::new();
        match &s.st {
            St::Assign(ts, v) => {
                let mut killed = HashSet::new();
                ts.iter().for_each(|t| {
                    if let Ex::Name(n) = t {
                        killed.insert(n.clone());
                    } else {
                        mentioned(t, &mut uses);
                    }
                });
                mentioned(v, &mut uses);
                let mut r: HashSet<String> = out.difference(&killed).cloned().collect();
                r.extend(uses);
                r
            }
            St::Return(v) => {
                if let Some(v) = v {
                    mentioned(v, &mut uses);
                }
                uses
            }
            St::For(t, it, b, e) => {
                mentioned(it, &mut uses);
                let e_in = e.as_ref().map_or(out.clone(), |e| self.block(e, out));
                let mut body_out: HashSet<String> = out.union(&e_in).cloned().collect();
                body_out.extend(uses.iter().cloned());
                loop {
                    let b_in = self.block(b, &body_out);
                    let n0 = body_out.len();
                    body_out.extend(b_in);
                    if body_out.len() == n0 {
                        break;
                    }
                }
                let _ = t;
                body_out
            }
            St::While(c, b, e) => {
                mentioned(c, &mut uses);
                let e_in = e.as_ref().map_or(out.clone(), |e| self.block(e, out));
                let mut body_out: HashSet<String> = out.union(&e_in).cloned().collect();
                body_out.extend(uses.iter().cloned());
                loop {
                    let b_in = self.block(b, &body_out);
                    let n0 = body_out.len();
                    body_out.extend(b_in);
                    if body_out.len() == n0 {
                        break;
                    }
                }
                body_out
            }
            St::If(arms, els) => {
                let mut r = HashSet::new();
                for (c, b) in arms {
                    mentioned(c, &mut r);
                    r.extend(self.block(b, out));
                }
                match els {
                    Some(e) => r.extend(self.block(e, out)),
                    None => r.extend(out.iter().cloned()),
                }
                r
            }
            St::Try(b, hs, e, fin) => {
                let fin_in = fin.as_ref().map_or(out.clone(), |f| self.block(f, out));
                let mut all: HashSet<String> = out.union(&fin_in).cloned().collect();
                for h in hs {
                    all.extend(self.block(&h.body, &fin_in));
                    if let Some(t) = &h.types {
                        mentioned(t, &mut all);
                    }
                }
                if let Some(e) = e {
                    all.extend(self.block(e, &fin_in));
                }
                // any statement of the body may jump to a handler
                let mut body_out = all.clone();
                loop {
                    let b_in = self.block(b, &body_out);
                    let n0 = body_out.len();
                    body_out.extend(b_in);
                    if body_out.len() == n0 {
                        break;
                    }
                }
                body_out
            }
            St::With(items, b) => {
                let mut r = self.block(b, out);
                items.iter().for_each(|(c, _)| mentioned(c, &mut r));
                r
            }
            _ => {
                mentioned_stmt(s, &mut uses);
                let mut r = out.clone();
                r.extend(uses);
                r
            }
        }
    }
}

// ---------------------------------------------------------------- the abstract interpretation

#[derive(Clone)]
struct Iterating {
    root: Pl,
    depth: u8,
    keys: Vec<String>, // the loop's own index or key names: `xs[i] = v` with i one of them changes only the current item
}

struct Cx<'p, 'a> {
    p: &'p Prog<'a>,
    f: usize,
    report: bool,
    st: BTreeSet<Edge>,
    live: Live,
    cur_live: HashSet<String>,
    always_live: HashSet<String>,
    borrowed: HashMap<String, (Pl, u8)>, // loop names that are the list's own item (iter_mut)
    comp: Vec<HashMap<String, String>>,  // comprehension names -> their hidden local names
    iterating: Vec<Iterating>,
    conflict: Option<(usize, String)>,
    line: usize,
    req_muts: HashSet<(String, String)>,
    req_iters: HashSet<(usize, String)>,
    req_marks: BTreeSet<(usize, String, u8)>,
    global_edges: BTreeSet<Edge>,
    // summary
    mutp: BTreeMap<String, u8>,
    mutf: BTreeMap<String, u8>,
    mutg: BTreeMap<String, u8>,
    esc: BTreeSet<(X, u8, X, u8)>,
    tmp: usize,
    kloc: HashMap<String, u8>, // kinds of hidden comprehension names
    key_ok: bool, // evaluating the arguments of a call that only compares what a lambda hands back
    dead: bool,                 // after return, raise, break or continue: no path reaches here
    loop_acc: Vec<BTreeSet<Edge>>, // states at each enclosing loop's break and continue
    try_acc: Vec<BTreeSet<Edge>>,  // states anywhere in each enclosing try body (a handler may start from any)
}

impl<'p, 'a> Cx<'p, 'a> {
    fn new(p: &'p Prog<'a>, f: usize, report: bool) -> Self {
        Cx {
            p,
            f,
            report,
            st: BTreeSet::new(),
            live: Live { out: HashMap::new() },
            cur_live: HashSet::new(),
            always_live: HashSet::new(),
            borrowed: HashMap::new(),
            comp: vec![],
            iterating: vec![],
            conflict: None,
            line: 0,
            req_muts: HashSet::new(),
            req_iters: HashSet::new(),
            req_marks: BTreeSet::new(),
            global_edges: BTreeSet::new(),
            mutp: BTreeMap::new(),
            mutf: BTreeMap::new(),
            mutg: BTreeMap::new(),
            esc: BTreeSet::new(),
            tmp: 0,
            kloc: HashMap::new(),
            key_ok: false,
            dead: false,
            loop_acc: vec![],
            try_acc: vec![],
        }
    }

    fn fi(&self) -> Option<&FnInfo<'a>> {
        if self.f == MODULE {
            None
        } else {
            Some(&self.p.fns[self.f])
        }
    }

    fn callback_keeps(&mut self) {
        for id in self.p.as_value.clone() {
            for (a, da, b, db) in self.p.sums[id].esc.clone() {
                for (x, d, y) in [(&a, da, &b), (&b, db, &a)] {
                    if matches!(y, X::P(_)) {
                        let pl = match x {
                            X::F(n) => Pl::F(n.clone()),
                            X::G(n) => Pl::G(n.clone()),
                            _ => continue,
                        };
                        self.add_edge(&pl, d, &Pl::Ext, 0);
                    }
                }
            }
        }
    }

    fn run_fn(&mut self) {
        self.callback_keeps();
        let fi = &self.p.fns[self.f];
        let body = &fi.def.body;
        // parameters and captured names belong to the caller: always read again
        self.always_live.extend(fi.params.iter().cloned());
        // names nested defs capture are read whenever those run
        for s in body {
            if let St::Def(d) = &s.st {
                let mut u = HashSet::new();
                mentioned_block(&d.body, &mut u);
                self.always_live.extend(u);
            }
        }
        let mut lam = HashSet::new();
        each_expr_block(body, &mut |e| {
            walk_ex(e, &mut |x| {
                if let Ex::Lambda(_, b) = x {
                    mentioned(b, &mut lam);
                }
            })
        });
        self.always_live.extend(lam);
        self.live.block(body, &HashSet::new());
        self.block(body);
        self.at_exit();
    }

    fn run_module(&mut self, body: &[Stmt]) {
        self.callback_keeps();
        self.always_live.extend(self.p.fn_globals.iter().cloned());
        let mut lam = HashSet::new();
        each_expr_block(body, &mut |e| {
            walk_ex(e, &mut |x| {
                if let Ex::Lambda(_, b) = x {
                    mentioned(b, &mut lam);
                }
            })
        });
        self.always_live.extend(lam);
        self.live.block(body, &HashSet::new());
        self.block(body);
    }

    fn summary(&self) -> Sum {
        Sum { mutp: self.mutp.clone(), esc: self.esc.clone(), mutf: self.mutf.clone(), mutg: self.mutg.clone() }
    }

    fn fail(&mut self, why: String) {
        if self.report && self.conflict.as_ref().map_or(true, |c| self.line < c.0) {
            self.conflict = Some((self.line, why));
        }
    }

    // Where a name lives.
    fn place(&self, n: &str) -> Pl {
        for m in self.comp.iter().rev() {
            if let Some(h) = m.get(n) {
                return Pl::L(h.clone());
            }
        }
        let mut f = self.f;
        while f != MODULE {
            let fi = &self.p.fns[f];
            if fi.globals.contains(n) {
                return Pl::G(n.to_string());
            }
            if fi.locals.contains(n) || fi.params.iter().any(|x| x == n) {
                return if f == self.f { Pl::L(n.to_string()) } else { Pl::Ext };
            }
            match fi.parent {
                Some(pa) => f = pa,
                None => break,
            }
        }
        // A captured name is one of this function's parameters (see collect).
        if let Some(fi) = self.fi() {
            if fi.captured.iter().any(|c| c == n) {
                return Pl::L(n.to_string());
            }
        }
        Pl::G(n.to_string())
    }

    fn kind_pl(&self, p: &Pl) -> u8 {
        match p {
            Pl::L(n) => {
                if let Some(k) = self.kloc.get(n) {
                    return *k;
                }
                let mut f = self.f;
                loop {
                    if let Some(k) = self.p.kl.get(&(f, n.clone())) {
                        return k.depth();
                    }
                    if f == MODULE {
                        return 0;
                    }
                    match self.p.fns[f].parent {
                        Some(pa) => f = pa,
                        None => {
                            // captured names take their enclosing function's kind
                            if self.p.fns[self.f].captured.iter().any(|c| c == n) {
                                return UNK;
                            }
                            return if self.p.fns[self.f].params.iter().any(|x| x == n) { UNK } else { 0 };
                        }
                    }
                }
            }
            Pl::G(n) => self.p.kl.get(&(MODULE, n.clone())).map_or(if self.p.module_names.contains(n) { UNK } else { 0 }, |k| k.depth()),
            Pl::F(n) => self.p.kf.get(n).map_or(UNK, |k| k.depth()),
            Pl::Ret | Pl::Ext => UNK,
        }
    }

    fn is_live(&self, p: &Pl) -> bool {
        match p {
            Pl::L(n) => self.always_live.contains(n) || self.cur_live.contains(n) || self.borrowed.values().any(|_| false),
            Pl::G(n) => self.f != MODULE || self.always_live.contains(n) || self.cur_live.contains(n),
            Pl::F(_) | Pl::Ret | Pl::Ext => true,
        }
    }

    fn partners(&self, p: &Pl) -> Vec<(Pl, u8, u8)> {
        // (other place, its depth, this place's depth)
        let mut v = vec![];
        for (a, da, b, db) in self.st.iter() {
            if a == p {
                v.push((b.clone(), *db, *da));
            }
            if b == p {
                v.push((a.clone(), *da, *db));
            }
        }
        v
    }

    fn name_srcs(&self, n: &str) -> Vec<Src> {
        if let Some((r, d)) = self.borrowed.get(n) {
            return self.path_srcs(r, *d);
        }
        let p = self.place(n);
        if self.kind_pl(&p) == 0 {
            return vec![];
        }
        // A standard-library constant or class (`from decimal import ROUND_HALF_UP, Decimal`) is no list.
        if let Pl::G(g) = &p {
            if let Some((mo, nm)) = self.p.fromimp.get(g) {
                if STDLIB.contains(&mo.as_str()) && nm.starts_with(|c: char| c.is_ascii_uppercase()) && !self.p.bound_other.contains(g) && !self.p.fns.iter().any(|f| f.globals.contains(g)) {
                    return vec![];
                }
            }
        }
        self.path_srcs(&p, 0)
    }

    // The value at depth d of place p, as sources.
    fn path_srcs(&self, p: &Pl, d: u8) -> Vec<Src> {
        let mut v = vec![(p.clone(), d, 0)];
        if matches!(p, Pl::Ext) {
            return v;
        }
        for (q, dq, dp) in self.partners(p) {
            // q@dq is p@dp; the value (p@d) at depth k is p@(d+k): with d+k >= dp, q@(dq + d + k - dp)
            if d >= dp {
                v.push((q, add_d(dq, d - dp), 0));
            } else {
                v.push((q, dq, dp - d));
            }
        }
        v
    }

    fn add_edge(&mut self, a: &Pl, da: u8, b: &Pl, db: u8) {
        // A place against itself is no sharing between two names (`q.append(q[0])` copies an item; the one
        // case of items sharing among themselves, `[row] * n`, is marked against Ext instead).
        if a == b {
            return;
        }
        if self.kind_pl(a) <= da && !matches!(a, Pl::Ret | Pl::Ext) {
            return;
        }
        if self.kind_pl(b) <= db && !matches!(b, Pl::Ret | Pl::Ext) {
            return;
        }
        let e = norm(a.clone(), da, b.clone(), db);
        // (a, da-k, b, db-k) already says the deeper objects are shared too
        for k in 1..=e.1.min(e.3) {
            if self.st.contains(&(e.0.clone(), e.1 - k, e.2.clone(), e.3 - k)) {
                return;
            }
        }
        let local = |p: &Pl| matches!(p, Pl::L(_));
        if !local(a) && !local(b) {
            self.global_edges.insert(e.clone());
            // A field or global sharing with another (or with the unknown) is checked on every change of either.
        }
        self.st.insert(e);
    }

    // Store sources into place `dst` at depth `dd`: dst@(dd + k) is src's value at depth k.
    fn store(&mut self, dst: &Pl, dd: u8, srcs: &[Src]) {
        for (p, dp, dv) in srcs {
            // the value at depth dv is p@dp; that is dst@(dd+dv)
            self.add_edge(dst, add_d(dd, *dv), p, *dp);
            // Ext is no object: two values from the unknown are not one list
            if matches!(p, Pl::Ext) {
                continue;
            }
            // dst now shares with everything p shares with (closure)
            for (q, dq, dpp) in self.partners(p) {
                if *dp >= dpp {
                    self.add_edge(dst, add_d(add_d(dd, *dv), 0), &q, add_d(dq, dp - dpp));
                } else {
                    self.add_edge(dst, add_d(add_d(dd, *dv), dpp - dp), &q, dq);
                }
            }
        }
    }

    fn kill(&mut self, p: &Pl) {
        self.st.retain(|(a, _, b, _)| a != p && b != p);
    }

    fn note_mut(&mut self, p: &Pl, d: u8) {
        if let Some(fi) = self.fi() {
            if let Pl::L(n) = p {
                if fi.params.iter().any(|x| x == n) {
                    let e = self.mutp.entry(n.clone()).or_insert(d);
                    *e = (*e).min(d);
                }
            }
        }
        match p {
            Pl::F(n) => {
                let e = self.mutf.entry(n.clone()).or_insert(d);
                *e = (*e).min(d);
            }
            Pl::G(n) if self.f != MODULE => {
                let e = self.mutg.entry(n.clone()).or_insert(d);
                *e = (*e).min(d);
            }
            _ => {}
        }
    }

    // The object at depth d of place p (its real storage) changes; any other name holding a copy of it, still
    // read later, would see Python's change in CPython and not in the typed build.
    fn mutate_place(&mut self, p: &Pl, d: u8, what: &str, own_key: bool) {
        self.note_mut(p, d);
        for it in self.iterating.clone() {
            if &it.root == p && it.depth == d && !own_key {
                let n = pl_name(p);
                self.fail(format!("{n} is changed ({what}) while a loop walks over it; the typed build walks a copy"));
            }
        }
        if matches!(p, Pl::Ext) {
            return;
        }
        for (q, dq, dp) in self.partners(p) {
            if d >= dp && self.is_live(&q) {
                // Both names hold a handle there (an object, a dict record): the typed build shares it too.
                let named = |x: &Pl| match x {
                    Pl::L(n) | Pl::G(n) if !n.contains('#') => Some(n.clone()),
                    _ => None,
                };
                if let (Some(a), Some(b)) = (named(p), named(&q)) {
                    if dp <= 1 && dq <= 1 {
                        if self.report {
                            self.req_marks.insert((self.line, a, dp));
                            self.req_marks.insert((self.line, b, dq));
                        }
                        continue;
                    }
                }
                let (a, b) = (pl_name(p), pl_name(&q));
                self.fail(format!("{a} and {b} name the same list in Python, and {a} is changed ({what}) while {b} is still used; the typed build would change only a copy"));
                return;
            }
        }
    }

    // A copy whose items come from srcs changes at depth d: each source changes in CPython, not in the typed build.
    fn mutate_copy(&mut self, srcs: &[Src], d: u8, what: &str) {
        for (p, dp, dv) in srcs {
            if d >= *dv && self.is_live(p) && self.kind_pl(p) > *dp {
                let a = pl_name(p);
                self.fail(format!("a copy of {a} is changed ({what}) while {a} is still used; Python changes {a} itself"));
                return;
            }
        }
    }

    // A place expression: (root, depth), for names, items, fields and `d.setdefault(k, v)`.
    fn place_of(&self, e: &Ex) -> Option<(Pl, u8)> {
        match e {
            Ex::Name(n) => {
                if let Some((r, d)) = self.borrowed.get(n) {
                    return Some((r.clone(), *d));
                }
                Some((self.place(n), 0))
            }
            Ex::Index(x, k) if !matches!(k.as_ref(), Ex::Slice(..)) => self.place_of(x).map(|(r, d)| (r, add_d(d, 1))),
            Ex::Attr(o, f) if !self.is_module(o) => Some((Pl::F(f.clone()), 0)),
            _ => None,
        }
    }

    fn is_module(&self, e: &Ex) -> bool {
        matches!(e, Ex::Name(n) if self.p.modules.contains(n) && matches!(self.place(n), Pl::G(_)))
    }

    fn mutate_ex(&mut self, e: &Ex, d: u8, what: &str, own_key: bool) {
        match self.place_of(e) {
            Some((r, rd)) => self.mutate_place(&r, add_d(rd, d), what, own_key),
            None => {
                let s = self.eval(e);
                self.mutate_copy(&s, d, what);
            }
        }
    }

    fn block(&mut self, b: &[Stmt]) {
        for s in b {
            if self.dead {
                return;
            }
            self.stmt(s);
            if let Some(acc) = self.try_acc.last_mut() {
                acc.extend(self.st.iter().cloned());
            }
        }
    }

    fn set_live(&mut self, s: &Stmt) {
        let mut l = self.live.out.get(&sid(s)).cloned().unwrap_or_default();
        mentioned_stmt(s, &mut l);
        self.cur_live = l;
        self.line = s.line;
    }

    fn assign_target(&mut self, t: &Ex, srcs: Vec<Src>, rebound: &HashSet<String>) {
        match t {
            Ex::Name(n) => {
                if self.borrowed.contains_key(n) {
                    self.fail(format!("the loop variable {n} is set while the loop changes the list's items through it"));
                    return;
                }
                let p = self.place(n);
                let srcs: Vec<Src> = srcs.into_iter().filter(|(q, _, _)| !matches!(q, Pl::L(m) if rebound.contains(m))).collect();
                if matches!(p, Pl::G(_) | Pl::Ext) {
                    // a global set inside a function (`global x`) or from module code: the old value's sharing goes
                    self.kill(&p);
                    self.store(&p, 0, &srcs);
                } else {
                    self.kill(&p);
                    self.store(&p, 0, &srcs);
                }
            }
            Ex::Tuple(xs) | Ex::List(xs) => {
                let item = down(&srcs);
                for x in xs {
                    match x {
                        Ex::Star(y) => {
                            let s = shallow(&srcs);
                            self.assign_target(y, s, rebound);
                        }
                        _ => self.assign_target(x, item.clone(), rebound),
                    }
                }
            }
            Ex::Index(c, k) => {
                let own = matches!(k.as_ref(), Ex::Name(kn) if self.iterating.iter().any(|it| it.keys.contains(kn)));
                self.eval(k);
                let what = if matches!(k.as_ref(), Ex::Slice(..)) { "a slice set" } else { "an item set" };
                match self.place_of(c) {
                    Some((r, d)) => {
                        self.mutate_place(&r, d, what, own);
                        let sv = if matches!(k.as_ref(), Ex::Slice(..)) { down(&srcs) } else { srcs };
                        self.store(&r, add_d(d, 1), &sv);
                    }
                    None => {
                        let s = self.eval(c);
                        self.mutate_copy(&s, 0, what);
                    }
                }
            }
            Ex::Attr(o, f) => {
                if !self.is_module(o) {
                    self.eval(o);
                    let p = Pl::F(f.clone());
                    // another object's field may hold the old value: no kill
                    self.store(&p, 0, &srcs);
                }
            }
            Ex::Star(y) => self.assign_target(y, shallow(&srcs), rebound),
            _ => {}
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        self.set_live(s);
        match &s.st {
            St::Expr(e) => {
                self.eval(e);
            }
            St::Assign(ts, v) => {
                // `a, b = b, a`: each target gets its own item's sources
                let mut rebound = HashSet::new();
                ts.iter().for_each(|t| target_names(t, &mut rebound));
                if let ([Ex::Tuple(tx) | Ex::List(tx)], Ex::Tuple(vx) | Ex::List(vx)) = (ts.as_slice(), v) {
                    if tx.len() == vx.len() && !tx.iter().chain(vx.iter()).any(|x| matches!(x, Ex::Star(_))) {
                        let all: Vec<Vec<Src>> = vx.iter().map(|x| self.eval(x)).collect();
                        for (t, s) in tx.iter().zip(all) {
                            self.assign_target(t, s, &rebound);
                        }
                        return;
                    }
                }
                let srcs = self.eval(v);
                if ts.len() > 1 {
                    // `a = b = []`: one list under every name
                    self.tmp += 1;
                    let h = Pl::L(format!("=#{}", self.tmp));
                    self.kloc.insert(format!("=#{}", self.tmp), UNK);
                    self.store(&h, 0, &srcs);
                    let mut all = srcs.clone();
                    all.push((h.clone(), 0, 0));
                    for t in ts {
                        let mut s2 = self.path_srcs(&h, 0);
                        s2.retain(|(p, _, _)| !matches!(p, Pl::L(m) if rebound.contains(m)));
                        self.assign_target(t, s2, &rebound);
                    }
                    self.kill(&h);
                    let _ = all;
                    return;
                }
                for t in ts {
                    self.assign_target(t, srcs.clone(), &rebound);
                }
            }
            St::AnnAssign(t, Some(v), _, _) => {
                let srcs = self.eval(v);
                let mut rebound = HashSet::new();
                target_names(t, &mut rebound);
                self.assign_target(t, srcs, &rebound);
            }
            St::AugAssign(t, op, v) => {
                let srcs = self.eval(v);
                let kt = match t {
                    Ex::Name(n) => self.borrowed.get(n).map(|(r, d)| dec_n(self.kind_pl(r), *d)).unwrap_or_else(|| self.kind_pl(&self.place(n))),
                    _ => self.ex_kind(t),
                };
                if kt > 0 && matches!(op.trim_end_matches('='), "+" | "|" | "&" | "-" | "^" | "*") {
                    // a list, dict or set changes in place (a tuple or string is rebound: treating it as a change
                    // is the safe side)
                    match self.place_of(t) {
                        Some((r, d)) => {
                            self.mutate_place(&r, d, &format!("{}=", op.trim_end_matches('=')), false);
                            self.store(&r, add_d(d, 1), &down(&srcs));
                        }
                        None => {
                            let s = self.eval(t);
                            self.mutate_copy(&s, 0, op);
                        }
                    }
                } else {
                    match t {
                        Ex::Name(n) => {
                            if self.borrowed.contains_key(n) {
                                self.fail(format!("the loop variable {n} is set while the loop changes the list's items through it"));
                            }
                            // a number: rebound; nothing shared
                            let p = self.place(n);
                            self.kill(&p);
                        }
                        Ex::Index(c, k) => {
                            let own = matches!(k.as_ref(), Ex::Name(kn) if self.iterating.iter().any(|it| it.keys.contains(kn)));
                            self.eval(k);
                            match self.place_of(c) {
                                Some((r, d)) => self.mutate_place(&r, d, op, own),
                                None => {
                                    let s = self.eval(c);
                                    self.mutate_copy(&s, 0, op);
                                }
                            }
                        }
                        Ex::Attr(o, _) => {
                            self.eval(o);
                        }
                        _ => {}
                    }
                }
            }
            St::Return(v) => {
                if let Some(v) = v {
                    let s = self.eval(v);
                    self.store(&Pl::Ret, 0, &s);
                }
                self.at_exit();
                self.dead = true;
            }
            St::Raise(a, b) => {
                if let Some(a) = a {
                    self.eval(a);
                }
                if let Some(b) = b {
                    self.eval(b);
                }
                if let Some(acc) = self.try_acc.last_mut() {
                    acc.extend(self.st.iter().cloned());
                }
                self.dead = true;
            }
            St::Break | St::Continue => {
                if let Some(acc) = self.loop_acc.last_mut() {
                    acc.extend(self.st.iter().cloned());
                }
                self.dead = true;
            }
            St::Del(ts) => {
                for t in ts {
                    match t {
                        Ex::Name(n) => {
                            let p = self.place(n);
                            self.kill(&p);
                        }
                        Ex::Index(c, k) => {
                            self.eval(k);
                            self.mutate_ex(c, 0, "del", false);
                        }
                        _ => {}
                    }
                }
            }
            St::Assert(a, b) => {
                self.eval(a);
                if let Some(b) = b {
                    self.eval(b);
                }
            }
            St::If(arms, els) => {
                let mut out = BTreeSet::new();
                let mut all_dead = true;
                for (c, b) in arms {
                    self.eval(c);
                    let at = self.st.clone();
                    self.dead = false;
                    self.block(b);
                    if !self.dead {
                        out.extend(self.st.iter().cloned());
                        all_dead = false;
                    }
                    self.st = at;
                }
                self.dead = false;
                match els {
                    Some(e) => {
                        self.block(e);
                        if !self.dead {
                            out.extend(self.st.iter().cloned());
                            all_dead = false;
                        }
                    }
                    None => {
                        out.extend(self.st.iter().cloned());
                        all_dead = false;
                    }
                }
                self.dead = all_dead;
                self.st = out;
            }
            St::While(c, b, e) => {
                self.loop_acc.push(BTreeSet::new());
                for _ in 0..12 {
                    let before = self.st.clone();
                    self.set_live(s);
                    self.eval(c);
                    self.dead = false;
                    self.block(b);
                    if self.dead {
                        self.st.clear();
                    }
                    self.dead = false;
                    let acc = self.loop_acc.last().cloned().unwrap_or_default();
                    self.st.extend(acc);
                    self.st.extend(before.iter().cloned());
                    if self.st == before {
                        break;
                    }
                }
                self.loop_acc.pop();
                if let Some(e) = e {
                    self.block(e);
                }
            }
            St::For(t, it, b, e) => self.for_loop(s, t, it, b, e.as_deref()),
            St::Try(b, hs, e, fin) => {
                let start = self.st.clone();
                self.try_acc.push(start.clone());
                self.block(b);
                let anywhere = self.try_acc.pop().unwrap_or_default();
                let mut out = BTreeSet::new();
                let body_dead = self.dead;
                if !body_dead {
                    if let Some(e) = e {
                        self.block(e);
                    }
                    if !self.dead {
                        out.extend(self.st.iter().cloned());
                    }
                }
                for h in hs {
                    self.st = anywhere.clone();
                    self.dead = false;
                    self.block(&h.body);
                    if !self.dead {
                        out.extend(self.st.iter().cloned());
                    }
                }
                // finally runs on every path, returning ones too: from everything seen
                self.dead = false;
                if let Some(f) = fin {
                    let mut all = out.clone();
                    all.extend(anywhere);
                    self.st = all;
                    self.block(f);
                    out.extend(self.st.iter().cloned());
                }
                if let Some(acc) = self.try_acc.last_mut() {
                    acc.extend(out.iter().cloned());
                }
                self.st = out;
            }
            St::With(items, b) => {
                let mut rebound = HashSet::new();
                for (c, a) in items {
                    let s = self.eval(c);
                    if let Some(a) = a {
                        target_names(a, &mut rebound);
                        self.assign_target(a, s, &rebound);
                    }
                }
                self.block(b);
            }
            St::Nonlocal(ns) => {
                if let Some(pa) = self.fi().and_then(|f| f.parent) {
                    for n in ns {
                        if self.p.kl.get(&(pa, n.clone())).map_or(UNK, |k| k.depth()) > 0 {
                            self.fail(format!("`nonlocal {n}` sets a list or dict of the enclosing function"));
                        }
                    }
                }
            }
            St::Def(d) => {
                // default values are made here; a nested def is followed at its calls
                for (_, x) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                    if let Some(x) = x {
                        self.eval(x);
                    }
                }
            }
            St::Class(c) => {
                // A class attribute holding a list is one list for every instance.
                for st in &c.body {
                    if let St::Assign(ts, v) = &st.st {
                        let s = self.eval(v);
                        for t in ts {
                            if let Ex::Name(n) = t {
                                if self.ex_kind(v) > 0 {
                                    let p = Pl::F(n.clone());
                                    self.add_edge(&p, 0, &Pl::Ext, 0);
                                }
                                self.store(&Pl::F(n.clone()), 0, &s);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn for_loop(&mut self, s: &Stmt, t: &Ex, it: &Ex, b: &[Stmt], e: Option<&[Stmt]>) {
        let srcs = self.eval(it);
        // What the loop walks: a place (the list itself) or a new list (sorted(xs), xs[:]).
        let (walked, item_srcs, keys): (Option<(Pl, u8)>, Vec<Src>, Vec<String>) = match it {
            Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(n) if n == "enumerate" && self.place(n) == Pl::G(n.clone()) && !self.p.module_names.contains(n)) && args.len() == 1 => {
                let inner = match &args[0] {
                    Arg::Pos(x) => x,
                    _ => it,
                };
                let keys = match t {
                    Ex::Tuple(xs) if xs.len() == 2 => match &xs[0] {
                        Ex::Name(i) => vec![i.clone()],
                        _ => vec![],
                    },
                    _ => vec![],
                };
                (self.place_of(inner), srcs.clone(), keys)
            }
            Ex::Call(f, args) if matches!(f.as_ref(), Ex::Attr(_, m) if m == "items" || m == "keys" || m == "values") && args.is_empty() => {
                let Ex::Call(f, _) = it else { unreachable!() };
                let Ex::Attr(o, m) = f.as_ref() else { unreachable!() };
                let keys = match (m.as_str(), t) {
                    ("items", Ex::Tuple(xs)) if xs.len() == 2 => match &xs[0] {
                        Ex::Name(k) => vec![k.clone()],
                        _ => vec![],
                    },
                    ("keys", Ex::Name(k)) => vec![k.clone()],
                    _ => vec![],
                };
                (self.place_of(o), srcs.clone(), keys)
            }
            _ => {
                let keys = match (t, self.ex_kind(it)) {
                    // `for k in d`: walking a dict's keys
                    (Ex::Name(k), _) => vec![k.clone()],
                    _ => vec![],
                };
                let pl = self.place_of(it);
                // The key form only holds for a dict: on a list `xs[x] = v` sets the item at the value x,
                // which may be one the loop has not reached
                let is_dict = matches!(it, Ex::Name(n) if self.p.dicts.contains(n));
                let keys = if is_dict { keys } else { vec![] };
                (pl, srcs.clone(), keys)
            }
        };
        let item = down(&item_srcs);
        // The loop variable as the list's own item (iter_mut): when the body changes it in place.
        let mut borrow: Option<String> = None;
        if let (Ex::Name(n), Some((r, d))) = (t, self.place_of(it)) {
            if !matches!(it, Ex::Call(..)) && changes_in_place(b, n) && self.kind_pl(&r) > add_d(d, 1) {
                borrow = Some(n.clone());
                self.borrowed.insert(n.clone(), (r.clone(), add_d(d, 1)));
                self.req_iters.insert((s.line, n.clone()));
            }
        }
        if let Some((r, d)) = walked.clone() {
            self.iterating.push(Iterating { root: r, depth: d, keys });
        }
        self.loop_acc.push(BTreeSet::new());
        for _ in 0..12 {
            let before = self.st.clone();
            self.set_live(s);
            if borrow.is_none() {
                let rebound: HashSet<String> = HashSet::new();
                self.assign_target(t, item.clone(), &rebound);
            }
            self.dead = false;
            self.block(b);
            if self.dead {
                self.st.clear();
            }
            self.dead = false;
            let acc = self.loop_acc.last().cloned().unwrap_or_default();
            self.st.extend(acc);
            self.st.extend(before.iter().cloned());
            if self.st == before {
                break;
            }
        }
        self.loop_acc.pop();
        if walked.is_some() {
            self.iterating.pop();
        }
        if let Some(n) = borrow {
            self.borrowed.remove(&n);
        }
        if let Some(e) = e {
            self.block(e);
        }
    }

    fn at_exit(&mut self) {
        let Some(fi) = self.fi() else { return };
        let params: Vec<String> = fi.params.clone();
        let x_of = |p: &Pl| -> Option<X> {
            match p {
                Pl::L(n) if params.contains(n) => Some(X::P(n.clone())),
                Pl::F(n) => Some(X::F(n.clone())),
                Pl::G(n) => Some(X::G(n.clone())),
                Pl::Ret => Some(X::Ret),
                Pl::Ext => Some(X::Ret).filter(|_| false),
                _ => None,
            }
        };
        let mut esc = vec![];
        for (a, da, b, db) in self.st.iter() {
            if let (Some(xa), Some(xb)) = (x_of(a), x_of(b)) {
                esc.push((xa, *da, xb, *db));
            }
            // A value from the unknown handed back or kept: the caller treats it as unknown too.
            if matches!(a, Pl::Ext) || matches!(b, Pl::Ext) {
                let (o, od, ed) = if matches!(a, Pl::Ext) { (b, *db, *da) } else { (a, *da, *db) };
                if let Some(xo) = x_of(o) {
                    esc.push((xo, od, X::G("<unknown>".into()), ed));
                }
            }
        }
        self.esc.extend(esc);
    }

    fn ex_kind(&self, e: &Ex) -> u8 {
        // a light kind of an expression, from the program's kinds
        match e {
            Ex::Name(n) => match self.borrowed.get(n) {
                Some((r, d)) => dec_n(self.kind_pl(r), *d),
                None => self.kind_pl(&self.place(n)),
            },
            Ex::Attr(o, f) if !self.is_module(o) => self.kind_pl(&Pl::F(f.clone())),
            Ex::Index(x, k) if !matches!(k.as_ref(), Ex::Slice(..)) => dec(self.ex_kind(x)),
            Ex::Index(x, _) => self.ex_kind(x),
            Ex::List(xs) | Ex::Set(xs) | Ex::Tuple(xs) => add_d(xs.iter().map(|x| if let Ex::Star(y) = x { dec(self.ex_kind(y)) } else { self.ex_kind(x) }).max().unwrap_or(0), 1),
            Ex::Dict(kv) => add_d(kv.iter().map(|(k, v)| if k.is_some() { self.ex_kind(v) } else { dec(self.ex_kind(v)) }).max().unwrap_or(0), 1),
            Ex::Comp(c, elt, val, _) => add_d(if *c == 'd' { val.as_ref().map_or(UNK, |v| self.ex_kind(v)) } else { self.ex_kind(elt) }, 1),
            Ex::Bin(op, a, b) if matches!(*op, "+" | "*" | "|") => self.ex_kind(a).max(self.ex_kind(b)),
            Ex::Bin(..) | Ex::Cmp(..) | Ex::Unary(..) => 0,
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::FStr(_) | Ex::NoneV | Ex::True | Ex::False => 0,
            _ => UNK,
        }
    }

    fn eval_args(&mut self, args: &[Arg]) -> Vec<(Option<String>, bool, Option<Ex>, Vec<Src>)> {
        // (keyword, is a * or ** argument, the expression, its sources)
        args.iter()
            .map(|a| match a {
                Arg::Pos(x) => (None, false, Some(x.clone()), self.eval(x)),
                Arg::Kw(k, x) => (Some(k.clone()), false, Some(x.clone()), self.eval(x)),
                Arg::Star(x) | Arg::KwStar(x) => (None, true, None, down(&self.eval(x))),
            })
            .collect()
    }

    fn eval(&mut self, e: &Ex) -> Vec<Src> {
        match e {
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False | Ex::Ellipsis => vec![],
            Ex::FStr(parts) => {
                let mut v = vec![];
                fparts(parts, &mut |x| v.push(x.clone()));
                for x in v {
                    self.eval(&x);
                }
                vec![]
            }
            Ex::Name(n) => self.name_srcs(n),
            Ex::Attr(o, f) => {
                if self.is_module(o) {
                    return vec![];
                }
                self.eval(o);
                let p = Pl::F(f.clone());
                if self.kind_pl(&p) == 0 {
                    return vec![];
                }
                self.path_srcs(&p, 0)
            }
            Ex::Index(x, k) => {
                self.eval(k);
                let sx = self.eval(x);
                if matches!(k.as_ref(), Ex::Slice(..)) {
                    shallow(&sx)
                } else {
                    down(&sx)
                }
            }
            Ex::Slice(a, b, c) => {
                for x in [a, b, c].into_iter().flatten() {
                    self.eval(x);
                }
                vec![]
            }
            Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => {
                let mut v = vec![];
                for x in xs {
                    match x {
                        Ex::Star(y) => {
                            let s = self.eval(y);
                            v.extend(shallow(&s));
                        }
                        _ => {
                            let s = self.eval(x);
                            v.extend(up(&s));
                        }
                    }
                }
                v
            }
            Ex::Dict(kv) => {
                let mut v = vec![];
                for (k, x) in kv {
                    match k {
                        Some(k) => {
                            self.eval(k);
                            let s = self.eval(x);
                            v.extend(up(&s));
                        }
                        None => {
                            let s = self.eval(x);
                            v.extend(shallow(&s));
                        }
                    }
                }
                v
            }
            Ex::Star(x) => self.eval(x),
            Ex::Bin(op, a, b) => {
                let (mut sa, mut sb) = (self.eval(a), self.eval(b));
                // `[0] * n`, `n * [0]`: a list display repeated, so the other side is an int (anything else
                // is a TypeError) unless the program defines its own __mul__ or __rmul__; its value holds
                // nothing of n's.
                let display = |x: &Ex| matches!(x, Ex::List(_) | Ex::Tuple(_) | Ex::Comp('l', ..));
                let own_mul = self.p.methods.contains_key("__mul__") || self.p.methods.contains_key("__rmul__");
                let (mut ka, mut kb) = (self.ex_kind(a), self.ex_kind(b));
                if *op == "*" && !own_mul {
                    if display(a) {
                        sb.clear();
                        kb = 0;
                    } else if display(b) {
                        sa.clear();
                        ka = 0;
                    }
                }
                if matches!(*op, "+" | "*" | "|") {
                    let mut v = shallow(&sa);
                    v.extend(shallow(&sb));
                    // `[row] * n`, `[[0] * m] * n`: the items are one list, many times
                    if *op == "*" && (ka >= 2 || kb >= 2) {
                        v.push((Pl::Ext, 0, 1));
                    }
                    v
                } else {
                    vec![]
                }
            }
            Ex::Unary(_, x) => {
                self.eval(x);
                vec![]
            }
            Ex::And(a, b) | Ex::Or(a, b) => {
                let mut v = self.eval(a);
                v.extend(self.eval(b));
                v
            }
            Ex::Cmp(a, rest) => {
                self.eval(a);
                for (_, x) in rest {
                    self.eval(x);
                }
                vec![]
            }
            Ex::IfExp(c, a, b) => {
                self.eval(c);
                let mut v = self.eval(a);
                v.extend(self.eval(b));
                v
            }
            Ex::Lambda(ps, body) => {
                // a lambda that changes something changes it whenever it is called, which the proof does not see
                if has_change(body) {
                    self.fail("a lambda changes a list or dict".into());
                }
                for (_, d) in ps.pos.iter().chain(ps.kwonly.iter()) {
                    if let Some(d) = d {
                        self.eval(d);
                    }
                }
                // Nor does it follow what a call of one hands back: its body may hand back only what it is
                // given or makes, never a list or dict it reaches by name.
                let mut map = HashMap::new();
                let names = ps.pos.iter().chain(ps.kwonly.iter()).map(|(n, _)| n.clone()).chain(ps.star.clone()).chain(ps.kwstar.clone());
                for n in names {
                    let h = format!("{n}#{}", ps as *const _ as usize);
                    self.kloc.insert(h.clone(), UNK);
                    map.insert(n, h);
                }
                let own: HashSet<String> = map.values().cloned().collect();
                self.comp.push(map);
                let v = self.eval(body);
                self.comp.pop();
                if !self.key_ok && v.iter().any(|(p, _, _)| !matches!(p, Pl::L(h) if own.contains(h))) {
                    self.fail("a lambda hands back a list or dict it reaches by name, which the typed build would copy".into());
                }
                vec![]
            }
            Ex::Comp(_, elt, val, fors) => {
                let mut map = HashMap::new();
                self.comp.push(HashMap::new());
                for cf in fors {
                    let s = self.eval(&cf.iter);
                    let mut names = HashSet::new();
                    target_names(&cf.target, &mut names);
                    for n in &names {
                        // one hidden name per comprehension (its place in the tree), however often it is walked
                        let h = format!("{n}#{}", cf as *const _ as usize);
                        map.insert(n.clone(), h.clone());
                        self.comp.last_mut().unwrap().insert(n.clone(), h.clone());
                        self.kloc.insert(h, UNK);
                    }
                    let rebound = HashSet::new();
                    self.assign_target(&cf.target, down(&s), &rebound);
                    for x in &cf.ifs {
                        self.eval(x);
                    }
                }
                let v = match val {
                    Some(v) => {
                        self.eval(elt);
                        let s = self.eval(v);
                        up(&s)
                    }
                    None => {
                        let s = self.eval(elt);
                        up(&s)
                    }
                };
                self.comp.pop();
                // the hidden names die with the comprehension, but what was built from them stays in v
                let v = self.resolve_hidden(v, &map);
                for h in map.values() {
                    self.kill(&Pl::L(h.clone()));
                }
                v
            }
            Ex::Walrus(n, v) => {
                let s = self.eval(v);
                let rebound = [n.clone()].into_iter().collect();
                self.assign_target(&Ex::Name(n.clone()), s.clone(), &rebound);
                self.name_srcs(n)
            }
            Ex::Yield(x) => {
                if let Some(x) = x {
                    let s = self.eval(x);
                    self.store(&Pl::Ret, 1, &s);
                }
                vec![(Pl::Ext, 0, 0)]
            }
            Ex::YieldFrom(x) => {
                let s = self.eval(x);
                self.store(&Pl::Ret, 0, &s);
                vec![(Pl::Ext, 0, 0)]
            }
            Ex::Await(x) => {
                let mut s = self.eval(x);
                s.push((Pl::Ext, 0, 0));
                s
            }
            Ex::Call(f, args) => self.call(f, args),
        }
    }

    // Sources through a comprehension's hidden names become what those names shared with.
    fn resolve_hidden(&self, v: Vec<Src>, map: &HashMap<String, String>) -> Vec<Src> {
        let hidden: HashSet<&String> = map.values().collect();
        let mut out = vec![];
        for (p, dp, dv) in v {
            if let Pl::L(n) = &p {
                if hidden.contains(n) {
                    for (q, dq, dpp) in self.partners(&p) {
                        if dp >= dpp {
                            out.push((q, add_d(dq, dp - dpp), dv));
                        } else {
                            out.push((q, dq, add_d(dv, dpp - dp)));
                        }
                    }
                    continue;
                }
            }
            out.push((p, dp, dv));
        }
        out
    }

    // A call of a local lambda: its body with the parameters bound to the arguments (a lambda changes nothing
    // itself; a lambda that would is refused where it is made).
    fn inline_lambda(&mut self, ps: &crate::dynpy::parse::Params, body: &Ex, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)]) -> Vec<Src> {
        let mut map = HashMap::new();
        let names: Vec<String> = ps.pos.iter().chain(ps.kwonly.iter()).map(|(n, _)| n.clone()).chain(ps.star.clone()).chain(ps.kwstar.clone()).collect();
        let star = a.iter().any(|x| x.1);
        let mut bind: HashMap<String, Vec<Src>> = HashMap::new();
        let mut i = 0;
        for x in a {
            match (&x.0, x.1) {
                (_, true) => {}
                (Some(k), false) => bind.entry(k.clone()).or_default().extend(x.3.iter().cloned()),
                (None, false) => {
                    match ps.pos.get(i) {
                        Some((n, _)) => bind.entry(n.clone()).or_default().extend(x.3.iter().cloned()),
                        // past the positional parameters: into *args, one level down
                        None => {
                            if let Some(st) = &ps.star {
                                bind.entry(st.clone()).or_default().extend(x.3.iter().map(|(p, dp, dv)| (p.clone(), *dp, add_d(*dv, 1))));
                            }
                        }
                    }
                    i += 1;
                }
            }
        }
        let all: Vec<Src> = a.iter().flat_map(|x| x.3.iter().map(|(p, dp, dv)| (p.clone(), *dp, if x.1 { *dv } else { add_d(*dv, 1) }))).collect();
        for n in &names {
            let h = format!("{n}#{}", ps as *const _ as usize);
            self.kloc.insert(h.clone(), UNK);
            // a * or ** argument may fill any parameter, at any depth: everything it holds goes to each
            let mut v = bind.get(n).cloned().unwrap_or_default();
            if star || ps.kwstar.as_ref() == Some(n) {
                v.extend(all.iter().cloned());
            }
            self.store(&Pl::L(h.clone()), 0, &v);
            map.insert(n.clone(), h);
        }
        self.comp.push(map);
        let v = self.eval(body);
        self.comp.pop();
        v
    }

    fn call(&mut self, f: &Ex, args: &[Arg]) -> Vec<Src> {
        // sorted/min/max/sort only compare what a key function hands back; re.sub's must be a str
        let key_ok = match f {
            Ex::Name(n) => matches!(n.as_str(), "sorted" | "min" | "max") && self.resolve(n).is_none() && matches!(self.place(n), Pl::G(_)) && !self.p.module_names.contains(n.as_str()),
            Ex::Attr(o, m) => m == "sort" || matches!(o.as_ref(), Ex::Name(r) if r == "re" && self.p.modules.contains("re")) && matches!(m.as_str(), "sub" | "subn"),
            _ => false,
        };
        let save = std::mem::replace(&mut self.key_ok, key_ok);
        let v = self.call_in(f, args);
        self.key_ok = save;
        v
    }

    fn call_in(&mut self, f: &Ex, args: &[Arg]) -> Vec<Src> {
        match f {
            Ex::Name(n) => {
                if let Some(id) = self.resolve(n) {
                    let a = self.eval_args(args);
                    return self.apply(id, &a, None);
                }
                if let Some(init) = self.p.classes.get(n).cloned() {
                    let a = self.eval_args(args);
                    match init {
                        Some(id) => {
                            self.apply(id, &a, Some(vec![]));
                        }
                        None => {
                            let fields = self.p.class_fields.get(n).cloned().unwrap_or_default();
                            for (i, (kw, star, _, s)) in a.iter().enumerate() {
                                let fname = if *star { None } else { kw.clone().or_else(|| fields.get(i).cloned()) };
                                match fname {
                                    Some(fl) => self.store(&Pl::F(fl), 0, s),
                                    None => {
                                        for fl in &fields {
                                            self.store(&Pl::F(fl.clone()), 0, s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return vec![];
                }
                if matches!(self.place(n), Pl::G(_)) {
                    if let Some((mo, nm)) = self.p.fromimp.get(n).cloned() {
                        let a = self.eval_args(args);
                        return self.module_call(&mo, &nm, args, &a);
                    }
                }
                let local = !matches!(self.place(n), Pl::G(_)) || self.p.module_names.contains(n.as_str());
                let lam = if matches!(self.place(n), Pl::L(_)) || self.f == MODULE { self.p.lambdas.get(&(self.f, n.clone())).copied() } else { None };
                let a = self.eval_args(args);
                if let Some(Ex::Lambda(ps, body)) = lam {
                    return self.inline_lambda(ps, body, &a);
                }
                let cbs = self.callbacks_in(&a);
                if !cbs.is_empty() {
                    self.callback_effects(&a, cbs);
                }
                if local {
                    return self.unknown_call(&a);
                }
                let s0 = a.first().map(|x| x.3.clone()).unwrap_or_default();
                match n.as_str() {
                    "range" | "len" | "print" | "sum" | "any" | "all" | "isinstance" => vec![],
                    _ if BUILTIN_SCALAR.contains(&n.as_str()) => vec![],
                    "enumerate" | "zip" => a.iter().flat_map(|x| paired(&x.3)).collect(),
                    "min" | "max" => {
                        let pos: Vec<&Vec<Src>> = a.iter().filter(|x| x.0.is_none()).map(|x| &x.3).collect();
                        let mut v = vec![];
                        if pos.len() == 1 {
                            v.extend(down(pos[0]));
                        } else {
                            pos.into_iter().for_each(|s| v.extend(s.iter().cloned()));
                        }
                        // default=
                        a.iter().filter(|x| x.0.as_deref() == Some("default")).for_each(|x| v.extend(x.3.iter().cloned()));
                        v
                    }
                    "next" => {
                        let mut v = down(&s0);
                        if let Some(x) = a.get(1) {
                            v.extend(x.3.iter().cloned());
                        }
                        v
                    }
                    "iter" => s0,
                    "map" if matches!(args.first(), Some(Arg::Pos(Ex::Name(f))) if BUILTIN_SCALAR.contains(&f.as_str()) && matches!(self.place(f), Pl::G(_)) && !self.p.module_names.contains(f)) => vec![],
                    "map" => {
                        // map(f, xs): f's value for each item, which may be the item
                        let mut v = vec![(Pl::Ext, 0, 0)];
                        for x in a.iter().skip(1) {
                            v.extend(shallow(&x.3));
                        }
                        v
                    }
                    "filter" => a.get(1).map(|x| shallow(&x.3)).unwrap_or_default(),
                    _ if BUILTIN_COPY.contains(&n.as_str()) => a.iter().flat_map(|x| shallow(&x.3)).collect(),
                    _ => self.unknown_call(&a),
                }
            }
            Ex::Attr(o, m) => {
                if let Ex::Name(mo) = o.as_ref() {
                    if self.is_module(o) {
                        let a = self.eval_args(args);
                        let cbs = self.callbacks_in(&a);
                        if !cbs.is_empty() {
                            self.callback_effects(&a, cbs);
                        }
                        return self.module_call(mo, m, args, &a);
                    }
                }
                // `datetime.datetime.strptime(...)`, `os.path.join(...)`: a function of a standard-library
                // module's class or submodule changes none of its arguments and keeps none
                let mut root = o.as_ref();
                while let Ex::Attr(r, _) = root {
                    root = r;
                }
                if !std::ptr::eq(root, o.as_ref()) && self.is_module(root) && matches!(root, Ex::Name(r) if STDLIB.contains(&r.as_str())) {
                    let a = self.eval_args(args);
                    let cbs = self.callbacks_in(&a);
                    if !cbs.is_empty() {
                        self.callback_effects(&a, cbs);
                    }
                    return a.iter().flat_map(|x| shallow(&x.3)).collect();
                }
                let so = self.eval(o);
                let ko = self.ex_kind(o);
                let a = self.eval_args(args);
                let mut out = vec![];
                let user = self.p.methods.get(m).cloned();
                if let Some(ids) = &user {
                    for id in ids.clone() {
                        out.extend(self.apply(id, &a, Some(so.clone())));
                    }
                }
                if ko > 0 || user.is_none() {
                    out.extend(self.container_method(o, &so, m, &a, user.is_some()));
                }
                out
            }
            g => {
                self.eval(g);
                let a = self.eval_args(args);
                self.unknown_call(&a)
            }
        }
    }

    fn resolve(&self, n: &str) -> Option<usize> {
        if !self.comp.is_empty() && self.comp.iter().any(|m| m.contains_key(n)) {
            return None;
        }
        let mut f = self.f;
        while f != MODULE {
            if let Some(id) = self.p.fns[f].nested.get(n) {
                return Some(*id);
            }
            if self.p.fns[f].locals.contains(n) {
                return None;
            }
            match self.p.fns[f].parent {
                Some(pa) => f = pa,
                None => break,
            }
        }
        self.p.top.get(n).copied()
    }

    // Functions passed as values may run inside any call the proof does not follow (`handlers[c](x)`,
    // `sorted(xs, key=f)`, a callback kept and called later): what they change of fields and globals changes
    // there, and what they change of their parameters is taken as a change of each argument.
    fn callback_effects(&mut self, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)], ids: Vec<usize>) {
        let mut mutp = false;
        for id in ids {
            let s = self.p.sums[id].clone();
            // A captured name is the enclosing function's own (`codes.append(...)` in a callback): that one
            // changes; the others are the callback's parameters, which may be given anything passed here.
            let captured = self.p.fns[id].captured.clone();
            for (pn, d) in &s.mutp {
                if captured.contains(pn) {
                    let pl = self.place(pn);
                    self.mutate_place(&pl, *d, "by a function passed on as a value", false);
                } else {
                    mutp = true;
                }
            }
            for (fl, d) in &s.mutf {
                self.mutate_place(&Pl::F(fl.clone()), *d, "by a function passed on as a value", false);
            }
            for (g, d) in &s.mutg {
                self.mutate_place(&Pl::G(g.clone()), *d, "by a function passed on as a value", false);
            }
        }
        if mutp {
            for x in a {
                if let Some(e) = &x.2 {
                    if let Some((r, d)) = self.place_of(e) {
                        self.mutate_place(&r, d, "by a function passed on as a value", false);
                    }
                }
            }
        }
    }

    fn callbacks_in(&self, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)]) -> Vec<usize> {
        a.iter().filter_map(|x| match &x.2 {
            Some(Ex::Name(n)) => self.resolve(n),
            Some(Ex::Attr(_, m)) => self.p.methods.get(m).and_then(|v| v.first().copied()),
            _ => None,
        }).collect()
    }

    // A function the proof does not know: it may keep or hand back what it is given (it is assumed not to
    // change it; the typed build has no such function anyway unless it is Python's, and those are listed).
    fn unknown_call(&mut self, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)]) -> Vec<Src> {
        let all: Vec<usize> = self.p.as_value.iter().copied().collect();
        self.callback_effects(a, all);
        // Code outside the program holds no list of the program's but what it is given: its value is new, or
        // one of its arguments (or in one).
        let mut v = vec![];
        for x in a {
            // it may change what it is given (a function from the program's own modules, a callback)
            if let Some(e) = &x.2 {
                if !x.3.is_empty() {
                    if let Some((r, d)) = self.place_of(e) {
                        self.mutate_place(&r, d, "by a function the proof does not follow", false);
                    }
                }
            }
            v.extend(x.3.iter().cloned());
            // kept somewhere the proof does not follow
            for (p, dp, dv) in &x.3 {
                self.add_edge(p, *dp, &Pl::Ext, *dv);
            }
        }
        v
    }

    fn module_call(&mut self, mo: &str, m: &str, args: &[Arg], a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)]) -> Vec<Src> {
        let s = |i: usize| a.get(i).map(|x| x.3.clone()).unwrap_or_default();
        let ex = |i: usize| match args.get(i) {
            Some(Arg::Pos(x)) => Some(x.clone()),
            _ => None,
        };
        let mutate0 = |cx: &mut Self, what: &str| {
            if let Some(x) = ex(0) {
                cx.mutate_ex(&x, 0, what, false);
            }
        };
        let store0 = |cx: &mut Self, v: &[Src]| {
            if let Some(x) = ex(0) {
                if let Some((r, d)) = cx.place_of(&x) {
                    cx.store(&r, add_d(d, 1), v);
                }
            }
        };
        match (mo, m) {
            ("heapq", "heappush") | ("bisect", "insort" | "insort_left" | "insort_right") => {
                mutate0(self, m);
                store0(self, &s(1));
                vec![]
            }
            ("heapq", "heappop" | "heapify") => {
                mutate0(self, m);
                down(&s(0))
            }
            ("heapq", "heapreplace" | "heappushpop") => {
                mutate0(self, m);
                store0(self, &s(1));
                let mut v = down(&s(0));
                v.extend(s(1));
                v
            }
            ("random", "shuffle") => {
                mutate0(self, m);
                vec![]
            }
            ("random", "choice") => down(&s(0)),
            ("random", "sample" | "choices") => shallow(&s(0)),
            ("heapq", "nsmallest" | "nlargest") => shallow(&s(1)),
            ("heapq", "merge") => a.iter().flat_map(|x| shallow(&x.3)).collect(),
            ("copy", "copy") => shallow(&s(0)),
            ("copy", "deepcopy") => vec![],
            ("collections", "deque" | "Counter" | "OrderedDict" | "defaultdict" | "ChainMap") => a.iter().flat_map(|x| shallow(&x.3)).collect(),
            ("functools", "reduce") => {
                let mut v = down(&s(1));
                v.extend(s(2));
                v.push((Pl::Ext, 0, 0));
                v
            }
            ("itertools", _) => a.iter().flat_map(|x| {
                let mut v = shallow(&x.3);
                v.extend(paired(&x.3));
                v
            }).collect(),
            _ if STDLIB.contains(&mo) => a.iter().flat_map(|x| shallow(&x.3)).collect(),
            _ => self.unknown_call(a),
        }
    }

    fn container_method(&mut self, o: &Ex, so: &[Src], m: &str, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)], user: bool) -> Vec<Src> {
        let s = |i: usize| a.get(i).map(|x| x.3.clone()).unwrap_or_default();
        let place = self.place_of(o);
        let store_items = |cx: &mut Self, v: &[Src]| {
            if let Some((r, d)) = &place {
                cx.store(r, add_d(*d, 1), v);
            }
        };
        match m {
            "append" | "appendleft" | "add" => {
                self.mutate_ex(o, 0, m, false);
                store_items(self, &s(0));
                vec![]
            }
            "insert" => {
                self.mutate_ex(o, 0, m, false);
                store_items(self, &s(1));
                vec![]
            }
            "extend" | "extendleft" | "update" => {
                self.mutate_ex(o, 0, m, false);
                for x in a {
                    let d = down(&x.3);
                    store_items(self, &d);
                }
                vec![]
            }
            "setdefault" => {
                self.mutate_ex(o, 0, m, false);
                store_items(self, &s(1));
                // `d.setdefault(k, []).append(v)`: the value is the dict's own item
                let mut v = down(so);
                v.extend(s(1));
                v
            }
            "pop" | "popleft" | "popitem" => {
                self.mutate_ex(o, 0, m, false);
                let mut v = down(so);
                v.extend(s(1));
                v
            }
            "remove" | "discard" | "clear" | "sort" | "reverse" | "rotate" | "difference_update" | "intersection_update" | "symmetric_difference_update" | "move_to_end" => {
                self.mutate_ex(o, 0, m, false);
                vec![]
            }
            "get" => {
                let mut v = down(so);
                v.extend(s(1));
                v
            }
            "copy" => shallow(so),
            "values" => shallow(so),
            "items" => paired(so),
            "keys" | "count" | "index" | "find" | "rfind" | "join" | "split" | "rsplit" | "splitlines" | "partition" | "rpartition" | "strip" | "lstrip" | "rstrip" | "lower" | "upper" | "title" | "capitalize" | "replace" | "startswith" | "endswith" | "format" | "isdigit" | "isalpha" | "isalnum" | "isspace" | "isupper" | "islower" | "issubset" | "issuperset" | "isdisjoint" | "union" | "intersection" | "difference" | "symmetric_difference" | "most_common" | "elements" | "total" | "encode" | "decode" | "zfill" | "center" | "ljust" | "rjust" | "casefold" | "swapcase" | "expandtabs" | "translate" | "maketrans" | "isnumeric" | "isdecimal" | "isidentifier" | "isprintable" | "istitle" | "removeprefix" | "removesuffix" | "hex" | "bit_length" | "conjugate" | "is_integer" | "as_integer_ratio" => {
                if m == "most_common" || m == "elements" {
                    paired(so)
                } else {
                    vec![]
                }
            }
            _ if user => vec![],
            // a compiled pattern's or a match's method: new strings, lists and matches; only a callback it is
            // given (`R.sub(repl, s)`) may change something
            _ if matches!(o, Ex::Name(n) if self.p.regex.contains(n)) => {
                let cbs = self.callbacks_in(a);
                if !cbs.is_empty() {
                    self.callback_effects(a, cbs);
                }
                vec![]
            }
            _ => {
                // A method no list, dict, set or deque has cannot change one (it would raise); a library
                // object's method may still change or keep its arguments (unknown_call).
                if self.ex_kind(o) > 0 && MUTATORS.contains(&m) {
                    self.mutate_ex(o, 0, m, false);
                }
                let mut v = self.unknown_call(a);
                v.extend(down(so));
                v
            }
        }
    }

    // A call of a function the program defines (`selfs`: the object's sources for a method or constructor).
    fn apply(&mut self, id: usize, a: &[(Option<String>, bool, Option<Ex>, Vec<Src>)], selfs: Option<Vec<Src>>) -> Vec<Src> {
        let fi = &self.p.fns[id];
        let sum = self.p.sums[id].clone();
        // Bind arguments to parameter names.
        let mut bound: HashMap<String, Vec<(Option<Ex>, Vec<Src>)>> = HashMap::new();
        let skip = if selfs.is_some() { 1 } else { 0 };
        if let (Some(s), Some(p0)) = (&selfs, fi.params.first()) {
            if fi.class.is_some() || selfs.as_ref().map_or(false, |v| v.is_empty()) {
                bound.entry(p0.clone()).or_default().push((None, s.clone()));
            }
        }
        let mut i = skip;
        let npos = fi.npos;
        let ordinary: Vec<String> = fi.params.iter().filter(|p| !fi.captured.contains(p)).cloned().collect();
        for (kw, star, ex, s) in a {
            if *star {
                for p in ordinary.iter().skip(skip) {
                    bound.entry(p.clone()).or_default().push((None, s.clone()));
                }
                continue;
            }
            match kw {
                Some(k) if ordinary.contains(k) => bound.entry(k.clone()).or_default().push((ex.clone(), s.clone())),
                Some(_) => {
                    if let Some(k) = &fi.kwstar {
                        bound.entry(k.clone()).or_default().push((None, up(s)));
                    }
                }
                None => {
                    if i < npos {
                        bound.entry(ordinary[i].clone()).or_default().push((ex.clone(), s.clone()));
                    } else if let Some(st) = &fi.star {
                        bound.entry(st.clone()).or_default().push((None, up(s)));
                    }
                    i += 1;
                }
            }
        }
        // captured names: this function's own names (a nested def is called from its enclosing scope)
        for c in &fi.captured {
            let ex = Ex::Name(c.clone());
            let s = self.name_srcs(c);
            bound.entry(c.clone()).or_default().push((Some(ex), s));
        }
        let typed = fi.typed.clone();
        let eligible = fi.eligible && !self.p.as_value.contains(&id);
        // Changes of parameters.
        for (pn, m) in &sum.mutp {
            let Some(args) = bound.get(pn).cloned() else { continue };
            for (ex, s) in args {
                let place = ex.as_ref().and_then(|x| self.place_of(x));
                match place {
                    Some((r, d)) if eligible && !matches!(r, Pl::Ext) => {
                        // passed `&mut place`: the change is the caller's own; the typed build must borrow
                        let needed = self.is_live(&r) || self.partners(&r).iter().any(|(q, _, _)| self.is_live(q)) || !matches!(r, Pl::L(_));
                        if needed {
                            let tp = if fi.class.is_some() && Some(pn) == fi.params.first() { "self_".to_string() } else { pn.clone() };
                            self.req_muts.insert((typed.clone(), tp));
                        }
                        self.mutate_place(&r, add_d(d, *m), &format!("by {}()", fi.def.name), false);
                    }
                    _ => {
                        let what = format!("by {}()", fi.def.name);
                        self.mutate_copy(&s, *m, &what);
                    }
                }
            }
        }
        // Edges between outside places.
        let mut ret = vec![];
        let resolve_x = |cx: &Self, x: &X| -> Vec<(Pl, u8, u8)> {
            // (place, its depth, the X's depth)
            match x {
                X::F(n) => vec![(Pl::F(n.clone()), 0, 0)],
                X::G(n) if n == "<unknown>" => vec![(Pl::Ext, 0, 0)],
                X::G(n) => vec![(Pl::G(n.clone()), 0, 0)],
                X::Ret => vec![],
                X::P(p) => bound.get(p).map(|v| v.iter().flat_map(|(_, s)| s.clone()).collect()).unwrap_or_default(),
            }
            .into_iter()
            .filter(|_| {
                let _ = cx;
                true
            })
            .collect()
        };
        for (xa, da, xb, db) in &sum.esc {
            let (xa, da, xb, db) = if matches!(xa, X::Ret) { (xb, *db, xa, *da) } else { (xa, *da, xb, *db) };
            let pa = resolve_x(self, xa);
            if matches!(xb, X::Ret) {
                // the value handed back is xa's
                for (p, dp, dv) in pa {
                    let k = da.max(dv);
                    ret.push((p, add_d(dp, k - dv), add_d(db, k - da)));
                }
                continue;
            }
            let pb = resolve_x(self, xb);
            for (p, dp, dv) in &pa {
                for (q, dq, dw) in &pb {
                    // p@(dp + k - dv) is xa@k... xa@da == xb@db
                    let ka = da.max(*dv);
                    let pa_d = add_d(*dp, ka - dv);
                    let xb_d = add_d(db, ka - da);
                    if xb_d >= *dw {
                        self.add_edge(p, pa_d, q, add_d(*dq, xb_d - dw));
                    } else {
                        self.add_edge(p, add_d(pa_d, dw - xb_d), q, *dq);
                    }
                }
            }
        }
        // Fields and globals it changes.
        for (fl, d) in &sum.mutf {
            self.mutate_place(&Pl::F(fl.clone()), *d, &format!("by {}()", fi.def.name), false);
        }
        for (g, d) in &sum.mutg {
            self.mutate_place(&Pl::G(g.clone()), *d, &format!("by {}()", fi.def.name), false);
        }
        ret
    }
}

fn dec_n(k: u8, n: u8) -> u8 {
    let mut k = k;
    for _ in 0..n {
        k = dec(k);
    }
    k
}

fn pl_name(p: &Pl) -> String {
    match p {
        Pl::L(n) | Pl::G(n) => format!("`{}`", n.split('#').next().unwrap_or(n)),
        Pl::F(n) => format!("`.{n}`"),
        Pl::Ret => "the value handed back".into(),
        Pl::Ext => "a value kept elsewhere".into(),
    }
}


// Does the loop body change name n in place (`n.append(v)`, `n[i] = v`, `n += [v]`, `n.sort()`, `del n[i]`)?
fn changes_in_place(b: &[Stmt], n: &str) -> bool {
    let is_n = |e: &Ex| matches!(e, Ex::Name(m) if m == n);
    let roots_n = |e: &Ex| {
        let mut e = e;
        loop {
            match e {
                Ex::Index(x, _) => e = x,
                Ex::Attr(x, _) => e = x,
                Ex::Name(m) => return m == n,
                _ => return false,
            }
        }
    };
    let mut hit = false;
    let mut check_ex = |e: &Ex| {
        walk_ex(e, &mut |x| {
            if let Ex::Call(f, _) = x {
                if let Ex::Attr(o, m) = f.as_ref() {
                    if roots_n(o) && MUTATORS.contains(&m.as_str()) {
                        hit = true;
                    }
                }
            }
        })
    };
    fn each(b: &[Stmt], f: &mut dyn FnMut(&Stmt)) {
        for s in b {
            f(s);
            for_child_blocks(s, &mut |c| each(c, f));
        }
    }
    let mut direct = false;
    each(b, &mut |s| match &s.st {
        St::Assign(ts, _) => {
            for t in ts {
                if let Ex::Index(..) = t {
                    if roots_n(t) {
                        direct = true;
                    }
                }
            }
        }
        St::AugAssign(t, _, _) => {
            if roots_n(t) && !is_n(t) || is_n(t) {
                if !is_n(t) {
                    direct = true;
                }
            }
        }
        St::Del(ts) => {
            if ts.iter().any(|t| matches!(t, Ex::Index(..)) && roots_n(t)) {
                direct = true;
            }
        }
        _ => {}
    });
    let mut exprs = vec![];
    each_expr_block(b, &mut |e| exprs.push(e.clone()));
    for e in &exprs {
        check_ex(e);
    }
    hit || direct
}

const MUTATORS: &[&str] = &[
    "append", "extend", "insert", "remove", "pop", "clear", "sort", "reverse", "update", "setdefault", "add", "discard", "popitem", "appendleft", "popleft", "extendleft", "rotate", "difference_update", "intersection_update", "symmetric_difference_update", "move_to_end", "subtract",
];
// Standard-library modules whose functions change none of their arguments and keep none (the ones that do,
// heapq, bisect, random.shuffle, are handled one by one in module_call).
const STDLIB: &[&str] = &[
    "math", "cmath", "statistics", "time", "json", "string", "re", "operator", "datetime", "decimal", "fractions", "struct", "hashlib", "base64", "bisect", "heapq", "random", "os", "sys", "textwrap", "unicodedata", "calendar", "itertools", "csv", "io", "dataclasses", "enum", "typing", "typing_extensions", "functools", "collections", "argparse", "subprocess", "pathlib", "shutil", "glob", "tempfile", "logging", "traceback", "inspect", "ast", "pprint", "uuid", "secrets", "zlib", "gzip", "pickle", "urllib", "html", "xml", "contextlib", "abc", "numbers", "platform", "locale", "difflib", "fnmatch", "shlex", "codecs", "binascii", "errno", "signal", "zoneinfo", "copy", "keyword", "token", "tokenize", "hmac", "warnings", "timeit", "array", "graphlib", "ipaddress", "email", "http", "socket", "select", "sqlite3", "threading", "queue", "asyncio", "concurrent", "multiprocessing", "weakref", "gc", "atexit", "builtins", "getpass", "colorsys", "wave", "zipfile", "tarfile", "mimetypes", "quopri", "uu", "pydoc", "unittest", "doctest", "linecache", "reprlib", "numbers",
];

// A change of a container anywhere in the expression (a method that changes it, or a walrus).
fn has_change(e: &Ex) -> bool {
    let mut hit = false;
    walk_ex(e, &mut |x| {
        if let Ex::Call(f, _) = x {
            if let Ex::Attr(_, m) = f.as_ref() {
                if MUTATORS.contains(&m.as_str()) {
                    hit = true;
                }
            }
        }
    });
    hit
}
