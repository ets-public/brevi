// Python the typed build would compile to a different answer, found on the Python syntax tree before the
// typed build runs. Such a file goes straight to the dynamic build (src/dynpy), which keeps CPython's
// behaviour, instead of being built typed and printing something else.
//
// - Sets: the typed build keeps a set as a list in sorted order, while CPython walks a set in its hash
//   table's order (`{-1, 5}` prints `{5, -1}`), and prints it in braces even inside a list or dict. Any
//   use where that order or form shows (printing, str(), iterating, list(s), s.pop(), joining) is refused.
//   Order-free uses stay typed: `x in s`, len, sorted, min, max, sum, any, all and set algebra.
// - print(q, q.popleft()), print(xs, grow(xs)), (q, q.pop()): CPython reads every argument before printing,
//   and a tuple or list holds the object itself; the typed build copies each one as it goes, so a later
//   argument that changes an earlier one shows the old value.
use crate::dynpy::parse::{Arg, CompFor, Ex, FPart, FuncDef, St, Stmt};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct G {
    sets: HashSet<String>,       // names (and `.attr` names) holding a set
    holders: HashSet<String>,    // names holding sets inside a list, dict or tuple
    set_fns: HashSet<String>,    // functions returning a set
    holder_fns: HashSet<String>, // functions returning a container of sets
    params: Vec<(String, Vec<String>)>,
    matches: HashSet<String>,    // names holding a re match (a Match object or None)
    dicts: HashSet<String>,      // names holding a dict
    pure_gens: HashSet<String>,  // generator functions the typed build may run eagerly
    tups: HashSet<String>,       // names holding tuple(...) of a list or iterator, or containers of them
    tups1: HashSet<String>,      // names holding a one-item tuple `(x,)` ("e:name": a list of them; "f()": returned)
    pure_fns: HashSet<String>,   // functions with no effect but their value (see pure_fns)
    fn_changes: HashMap<String, Changes>, // what each function changes in place (see program_changes)
}

// Why the file needs the dynamic build, with the line: None when the typed build gives CPython's answer.
pub fn check(src: &str) -> Option<(usize, String)> {
    check_soft(src).unwrap_or_else(|(l, m)| Some((l, m, vec![]))).map(|(l, w, _)| (l, w))
}

// As check, with the names the typed build may still take the file on (empty: it may not). Every reason
// found is a set walked or shown in CPython's hash order, on sets named there; when rustc finds that each of
// those holds strings (or tuples led by one), whose hashes CPython draws afresh each run (PYTHONHASHSEED), no
// order is CPython's own, and the typed build's is as good as any run's.
// Err (line, why): the dynamic build's parser cannot read the file, so none of this was checked.
pub fn check_soft(src: &str) -> Result<Option<(usize, String, Vec<String>)>, (usize, String)> {
    Ok(check_body(&crate::dynpy::parse::parse(src)?))
}

// json.dumps(x, ...): the typed build writes indent, separators (a pair), sort_keys and ensure_ascii; any other
// keyword (allow_nan, default, cls, ...) it would leave out.
fn json_kwargs(body: &[Stmt]) -> Option<(usize, String)> {
    let mut hit = None;
    crate::dynpy::typing::walk_all(body, &mut |e, line| {
        if hit.is_some() {
            return;
        }
        if let Ex::Call(f, args) = e {
            if matches!(f.as_ref(), Ex::Attr(m, n) if n == "dumps" && matches!(m.as_ref(), Ex::Name(j) if j == "json")) {
                for a in args {
                    let odd = match a {
                        Arg::Kw(k, v) => match k.as_str() {
                            "indent" | "sort_keys" | "ensure_ascii" => false,
                            "separators" => !matches!(v, Ex::Tuple(t) if t.len() == 2),
                            _ => true,
                        },
                        Arg::Pos(_) => false,
                        _ => true,
                    };
                    if odd {
                        let k = match a {
                            Arg::Kw(k, _) => format!("{k}="),
                            _ => "*".into(),
                        };
                        hit = Some((line, format!("json.dumps(..., {k}) the typed build would leave out")));
                        return;
                    }
                }
            }
        }
    });
    hit
}
fn check_body(body: &[Stmt]) -> Option<(usize, String, Vec<String>)> {
    let mut g = G::default();
    collect_params(body, &mut g);
    for _ in 0..8 {
        let before = (g.sets.len(), g.holders.len(), g.set_fns.len(), g.holder_fns.len(), g.tups.len(), g.tups1.len());
        learn(body, None, &mut g);
        if before == (g.sets.len(), g.holders.len(), g.set_fns.len(), g.holder_fns.len(), g.tups.len(), g.tups1.len()) {
            break;
        }
    }
    for _ in 0..8 {
        let before = g.pure_gens.len();
        find_pure_gens(body, &mut g);
        if before == g.pure_gens.len() {
            break;
        }
    }
    // Top-level code: a growing name that a function reads.
    {
        let mut gr = HashSet::new();
        growing(body, &mut gr);
        let mut used = HashSet::new();
        nested_names(body, &mut used);
        if let Some(n) = gr.iter().find(|n| used.contains(*n)) {
            let line = body.iter().find(|s| matches!(&s.st, St::Assign(..) | St::AugAssign(..))).map_or(1, |s| s.line);
            return Some((line, format!("`{n}` can grow past 64 bits and is read by a function, where the typed build keeps it a 64-bit int"), vec![]));
        }
    }
    if let Some(line) = crate::dynpy::typing::typed_isinstance(body) {
        return Some((line, "isinstance with a typing name".into(), vec![]));
    }
    if let Some((line, why)) = odd_decorator(body).or_else(|| json_kwargs(body)).or_else(|| findall_groups(body)).or_else(|| pattern_uses(body)).or_else(|| default_reads(body)).or_else(|| crate::share_guard::check(body)) {
        return Some((line, why, vec![]));
    }
    let mut hits = vec![];
    g.pure_fns = pure_fns(body);
    g.fn_changes = program_changes(body);
    uses(body, &g, &mut hits);
    let (line, why, _) = hits.first().cloned()?;
    if let Some(h) = hits.iter().find(|h| h.2.is_none()) {
        return Some((h.0, h.1.clone(), vec![]));
    }
    let mut names: Vec<String> = hits.iter().flat_map(|h| h.2.clone().unwrap_or_default()).collect();
    names.sort();
    names.dedup();
    // Each name must be set only by `name = ...` (or be a def's parameter), where the typed build checks it.
    if names.iter().any(|k| bound_oddly(body, k.split_once(':').map_or(k.as_str(), |x| x.1))) {
        return Some((line, why, vec![]));
    }
    Some((line, why, names))
}

// A decorator the typed build would drop although it changes what the def or class does (`@retry` from
// tenacity, `@atexit.register`, `@cached_property`, a program's own): the typed build drops every decorator
// but these, which it builds itself or which change nothing it prints.
fn odd_decorator(body: &[Stmt]) -> Option<(usize, String)> {
    const KNOWN: &[(&str, &str)] = &[
        ("functools", "lru_cache"),
        ("functools", "cache"),
        ("functools", "total_ordering"),
        ("dataclasses", "dataclass"),
        ("abc", "abstractmethod"),
        ("typing", "final"),
        ("typing", "override"),
        ("typing", "overload"),
        ("typing", "no_type_check"),
        ("typing", "runtime_checkable"),
        ("typing_extensions", "final"),
        ("typing_extensions", "override"),
        ("typing_extensions", "overload"),
        ("typing_extensions", "runtime_checkable"),
    ];
    fn imports(body: &[Stmt], mods: &mut HashMap<String, String>, names: &mut HashMap<String, (String, String)>, defined: &mut HashSet<String>) {
        for s in body {
            match &s.st {
                St::Import(v) => v.iter().for_each(|(m, a)| {
                    mods.insert(a.clone().unwrap_or_else(|| m.clone()), m.clone());
                }),
                St::ImportFrom(m, v) => v.iter().for_each(|(n, a)| {
                    names.insert(a.clone().unwrap_or_else(|| n.clone()), (m.clone(), n.clone()));
                }),
                St::Def(d) => {
                    defined.insert(d.name.clone());
                }
                St::Class(c) => {
                    defined.insert(c.name.clone());
                }
                St::Assign(ts, _) => ts.iter().for_each(|t| {
                    if let Ex::Name(n) = t {
                        defined.insert(n.clone());
                    }
                }),
                _ => {}
            }
            each_block(&s.st, &mut |b| imports(b, mods, names, defined));
        }
    }
    let (mut mods, mut names, mut defined) = (HashMap::new(), HashMap::new(), HashSet::new());
    imports(body, &mut mods, &mut names, &mut defined);
    let known = |d: &Ex, props: &HashSet<String>| -> bool {
        let d = match d {
            Ex::Call(f, _) => f.as_ref(),
            d => d,
        };
        match d {
            Ex::Name(n) if matches!(n.as_str(), "staticmethod" | "classmethod" | "property") => !defined.contains(n) && !names.contains_key(n),
            Ex::Name(n) => names.get(n).is_some_and(|(m, o)| KNOWN.contains(&(m.as_str(), o.as_str()))),
            Ex::Attr(r, a) => match r.as_ref() {
                Ex::Name(p) if props.contains(p) => matches!(a.as_str(), "setter" | "getter" | "deleter"),
                Ex::Name(m) => mods.get(m).is_some_and(|m| KNOWN.contains(&(m.as_str(), a.as_str()))),
                _ => false,
            },
            _ => false,
        }
    };
    fn walk(body: &[Stmt], known: &dyn Fn(&Ex, &HashSet<String>) -> bool) -> Option<(usize, String)> {
        // names of properties in this class body, for `@x.setter`
        let mut props = HashSet::new();
        for s in body {
            let (decs, line, name) = match &s.st {
                St::Def(d) => (&d.decorators, d.line, &d.name),
                St::Class(c) => (&c.decorators, c.line, &c.name),
                _ => {
                    if let Some(r) = { let mut r = None; each_block(&s.st, &mut |b| r = r.take().or_else(|| walk(b, known))); r } {
                        return Some(r);
                    }
                    continue;
                }
            };
            for d in decs {
                if !known(d, &props) {
                    return Some((line, format!("`{name}` has a decorator the typed build would leave out")));
                }
            }
            if let St::Def(d) = &s.st {
                if d.decorators.iter().any(|x| matches!(x, Ex::Name(n) if n == "property")) {
                    props.insert(d.name.clone());
                }
            }
            let mut r = None;
            each_block(&s.st, &mut |b| r = r.take().or_else(|| walk(b, known)));
            if r.is_some() {
                return r;
            }
        }
        None
    }
    walk(body, &known)
}

// Is `n` bound other than by `n = ...` or as a plain def parameter (a loop variable, a tuple target,
// `with ... as n`, `except E as n`, a comprehension's variable, `n :=`, a lambda's or a starred parameter)?
fn bound_oddly(body: &[Stmt], n: &str) -> bool {
    fn names_in(t: &Ex, n: &str) -> bool {
        let mut f = false;
        walk_ex(t, &mut |e| f |= matches!(e, Ex::Name(m) if m == n));
        f
    }
    let mut odd = false;
    for s in body {
        match &s.st {
            St::For(t, ..) => odd |= names_in(t, n),
            St::Assign(ts, _) => odd |= ts.iter().any(|t| !matches!(t, Ex::Name(_)) && matches!(t, Ex::Tuple(_) | Ex::List(_) | Ex::Star(_)) && names_in(t, n)),
            St::With(items, _) => odd |= items.iter().any(|(_, t)| t.as_ref().is_some_and(|t| names_in(t, n))),
            St::Try(_, hs, _, _) => odd |= hs.iter().any(|h| h.name.as_deref() == Some(n)),
            St::Import(v) | St::ImportFrom(_, v) => odd |= v.iter().any(|(m, a)| a.as_deref().unwrap_or(m) == n),
            St::Def(d) => odd |= d.params.star.as_deref() == Some(n) || d.params.kwstar.as_deref() == Some(n) || d.params.kwonly.iter().any(|p| p.0 == n),
            _ => {}
        }
        walk_st(&s.st, &mut |e| match e {
            Ex::Comp(_, _, _, fors) => odd |= fors.iter().any(|c| names_in(&c.target, n)),
            Ex::Walrus(m, _) => odd |= m == n,
            Ex::Lambda(p, _) => odd |= p.pos.iter().any(|x| x.0 == n) || p.star.as_deref() == Some(n),
            _ => {}
        });
        each_block(&s.st, &mut |b| odd |= bound_oddly(b, n));
    }
    odd
}

// The names whose sets `e` (a set, or a container of sets) walks, tagged `s:` (a set, or what set(...) was
// made of) or `h:` (a list or dict of sets): None when it is not just such names.
fn soft_keys(e: &Ex, g: &G) -> Option<Vec<String>> {
    match e {
        Ex::Name(n) if g.sets.contains(n) && !g.holders.contains(n) => Some(vec![format!("s:{n}")]),
        Ex::Name(n) if g.holders.contains(n) && !g.sets.contains(n) => Some(vec![format!("h:{n}")]),
        Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(n) if n == "set" || n == "frozenset") => match args.as_slice() {
            [Arg::Pos(Ex::Name(m))] => Some(vec![format!("s:{m}")]),
            _ => None,
        },
        Ex::Call(f, args) if args.is_empty() => match f.as_ref() {
            Ex::Attr(r, m) if m == "copy" => soft_keys(r, g),
            _ => None,
        },
        // `adj.get(u, ())` on a dict of sets: one of its sets (or an empty tuple).
        Ex::Call(f, args) if matches!(args.as_slice(), [Arg::Pos(_)] | [Arg::Pos(_), Arg::Pos(Ex::Tuple(_) | Ex::List(_) | Ex::Set(_))] | [Arg::Pos(_), Arg::Pos(Ex::Call(..))]) => match f.as_ref() {
            Ex::Attr(r, m) if m == "get" => match r.as_ref() {
                Ex::Name(h) if g.holders.contains(h) && !g.sets.contains(h) => Some(vec![format!("h:{h}")]),
                _ => None,
            },
            _ => None,
        },
        Ex::Index(b, _) => match b.as_ref() {
            Ex::Name(h) if g.holders.contains(h) && !g.sets.contains(h) => Some(vec![format!("h:{h}")]),
            _ => None,
        },
        Ex::Bin("|" | "&" | "-" | "^", a, b) => {
            let mut v = soft_keys(a, g).or_else(|| (!is_set(a, g) && !is_holder(a, g)).then(Vec::new))?;
            v.extend(soft_keys(b, g).or_else(|| (!is_set(b, g) && !is_holder(b, g)).then(Vec::new))?);
            (!v.is_empty()).then_some(v)
        }
        _ => None,
    }
}

// soft_keys over every set (or container of sets) among xs; None if one of them is not just names.
fn soft_all<'a>(xs: impl IntoIterator<Item = &'a Ex>, g: &G) -> Option<Vec<String>> {
    let mut v = vec![];
    for x in xs {
        if is_set(x, g) || is_holder(x, g) {
            v.extend(soft_keys(x, g)?);
        }
    }
    (!v.is_empty()).then_some(v)
}

// Calls whose answer does not depend on the order their argument is walked in (no key=: ties would show it).
fn order_free_call(e: &Ex) -> Option<&Ex> {
    let Ex::Call(f, args) = e else { return None };
    let Ex::Name(n) = f.as_ref() else { return None };
    if !matches!(n.as_str(), "max" | "min" | "sorted" | "any" | "all" | "len" | "set" | "frozenset" | "sum") {
        return None;
    }
    if args.iter().any(|a| !matches!(a, Arg::Pos(_) | Arg::Kw(..)) || matches!(a, Arg::Kw(k, _) if k != "reverse" && k != "default")) {
        return None;
    }
    let [Arg::Pos(x), ..] = args.as_slice() else { return None };
    // sum: only counting (`sum(1 for ...)`); float sums depend on the order.
    if n == "sum" && !matches!(x, Ex::Comp(_, elt, None, _) if matches!(elt.as_ref(), Ex::Int(_))) {
        return None;
    }
    matches!(x, Ex::Comp(..)).then_some(x)
}

fn collect_params(body: &[Stmt], g: &mut G) {
    for s in body {
        match &s.st {
            St::Def(f) => {
                g.params.push((f.name.clone(), f.params.pos.iter().map(|p| p.0.clone()).collect()));
                collect_params(&f.body, g);
            }
            St::Class(c) => collect_params(&c.body, g),
            _ => each_block(&s.st, &mut |b| collect_params(b, g)),
        }
    }
}

pub(crate) fn each_block(st: &St, f: &mut dyn FnMut(&[Stmt])) {
    match st {
        St::If(arms, els) => {
            arms.iter().for_each(|(_, b)| f(b));
            if let Some(b) = els {
                f(b);
            }
        }
        St::While(_, b, e) | St::For(_, _, b, e) => {
            f(b);
            if let Some(b) = e {
                f(b);
            }
        }
        St::Try(b, hs, e, fin) => {
            f(b);
            hs.iter().for_each(|h| f(&h.body));
            for b in [e, fin].into_iter().flatten() {
                f(b);
            }
        }
        St::With(_, b) => f(b),
        St::Def(d) => f(&d.body),
        St::Class(c) => f(&c.body),
        _ => {}
    }
}

fn key(e: &Ex) -> Option<String> {
    match e {
        Ex::Name(n) => Some(n.clone()),
        Ex::Attr(_, a) => Some(format!(".{a}")),
        _ => None,
    }
}

fn root(e: &Ex) -> Option<String> {
    match e {
        Ex::Name(_) | Ex::Attr(..) => key(e),
        Ex::Index(b, _) => root(b),
        _ => None,
    }
}

fn is_set(e: &Ex, g: &G) -> bool {
    match e {
        Ex::Set(_) => true,
        Ex::Comp('s', ..) => true,
        Ex::Call(f, _) => match f.as_ref() {
            Ex::Name(n) if n == "set" || n == "frozenset" => true,
            Ex::Name(n) => g.set_fns.contains(n),
            Ex::Attr(r, m) => {
                (matches!(m.as_str(), "union" | "intersection" | "difference" | "symmetric_difference") && is_set(r, g))
                    || (m == "copy" && is_set(r, g))
                    || (matches!(m.as_str(), "get" | "pop" | "setdefault") && is_holder(r, g))
                    || g.set_fns.contains(m.as_str())
            }
            _ => false,
        },
        Ex::Bin(op, a, b) => matches!(*op, "|" | "&" | "-" | "^") && (is_set(a, g) || is_set(b, g)),
        Ex::Name(_) | Ex::Attr(..) => key(e).is_some_and(|k| g.sets.contains(&k)),
        Ex::Index(b, _) => is_holder(b, g),
        Ex::IfExp(_, a, b) => is_set(a, g) || is_set(b, g),
        Ex::Or(a, b) | Ex::And(a, b) => is_set(a, g) || is_set(b, g),
        Ex::Walrus(_, v) => is_set(v, g),
        _ => false,
    }
}

// A return, or a break or continue not inside a loop of its own, in body (not in nested functions).
fn jumps_out(body: &[Stmt], in_loop: bool) -> bool {
    body.iter().any(|s| match &s.st {
        St::Return(_) => true,
        St::Break | St::Continue => !in_loop,
        St::If(arms, e) => arms.iter().any(|(_, b)| jumps_out(b, in_loop)) || e.as_deref().is_some_and(|e| jumps_out(e, in_loop)),
        St::While(_, b, e) | St::For(_, _, b, e) => jumps_out(b, true) || e.as_deref().is_some_and(|e| jumps_out(e, in_loop)),
        St::Try(b, hs, e, f) => jumps_out(b, in_loop) || hs.iter().any(|h| jumps_out(&h.body, in_loop)) || e.as_deref().is_some_and(|e| jumps_out(e, in_loop)) || f.as_deref().is_some_and(|f| jumps_out(f, in_loop)),
        St::With(_, b) => jumps_out(b, in_loop),
        _ => false,
    })
}

// `(x,)`: the typed build keeps a one-item tuple as a one-item list, so showing it whole gives [x] where CPython
// gives (x,). Only the one-item tuple itself is followed (names, lists appended to, returns, arguments), not the
// containers it is put in at other places: `c = (n, path + (u,))` then `best[k] = c` is a real tuple whose second
// item the typed build joins as a tuple, and `ans[2]` read back is not a one-item tuple.
fn is_tup1(e: &Ex, g: &G) -> bool {
    match e {
        Ex::Tuple(v) if v.len() == 1 => true,
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().any(|x| is_tup1(x, g)),
        Ex::Dict(items) => items.iter().any(|(k, v)| k.as_ref().is_some_and(|k| is_tup1(k, g)) || is_tup1(v, g)),
        // `(a,) + (b,)`: two lists joined is a list; `(a,) + t` with t a real tuple is joined as a tuple.
        Ex::Bin("+", a, b) => is_tup1(a, g) && is_tup1(b, g),
        Ex::Bin("*", a, b) => is_tup1(a, g) || is_tup1(b, g),
        Ex::Comp(_, elt, val, _) => is_tup1(elt, g) || val.as_deref().is_some_and(|v| is_tup1(v, g)),
        Ex::Name(_) | Ex::Attr(..) => key(e).is_some_and(|k| g.tups1.contains(&k) || g.tups1.contains(&format!("e:{k}"))),
        Ex::Index(b, i) if !matches!(i.as_ref(), Ex::Slice(..)) => key(b).is_some_and(|k| g.tups1.contains(&format!("e:{k}"))),
        Ex::Call(f, args) => match f.as_ref() {
            Ex::Name(n) if g.tups1.contains(&format!("{n}()")) => true,
            Ex::Name(n) if matches!(n.as_str(), "sorted" | "list" | "reversed") => args.iter().any(|a| matches!(a, Arg::Pos(x) if is_tup1(x, g))),
            _ => false,
        },
        Ex::IfExp(_, a, b) | Ex::Or(a, b) => is_tup1(a, g) || is_tup1(b, g),
        _ => false,
    }
}

// tuple(xs): the typed build keeps it as a list, so printing it shows [1, 2] where CPython shows (1, 2).
fn is_tup(e: &Ex, g: &G) -> bool {
    match e {
        Ex::Call(f, args) => match f.as_ref() {
            Ex::Name(n) if n == "tuple" => !args.is_empty(),
            // A function that returns tuple(...) values (or a list of them).
            Ex::Name(n) if g.tups.contains(&format!("{n}()")) => true,
            Ex::Attr(_, m) if g.tups.contains(&format!("{m}()")) => true,
            Ex::Name(n) if matches!(n.as_str(), "sorted" | "list" | "reversed" | "set" | "frozenset" | "dict") => args.iter().any(|a| matches!(a, Arg::Pos(x) if is_tup(x, g))),
            Ex::Attr(r, m) if matches!(m.as_str(), "copy" | "items" | "keys" | "values" | "get" | "pop") => is_tup(r, g),
            // m.groups() is a tuple.
            Ex::Attr(_, m) if m == "groups" => true,
            _ => false,
        },
        Ex::Comp(_, elt, val, _) => is_tup(elt, g) || val.as_deref().is_some_and(|v| is_tup(v, g)),
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().any(|x| is_tup(x, g)),
        Ex::Dict(items) => items.iter().any(|(k, v)| k.as_ref().is_some_and(|k| is_tup(k, g)) || is_tup(v, g)),
        // ("k:name": a dict with tuple(...) keys only; shown whole it shows them, but its values are not tuples.)
        Ex::Name(_) | Ex::Attr(..) => key(e).is_some_and(|k| g.tups.contains(&k) || g.tups.contains(&format!("k:{k}"))),
        Ex::Index(b, _) => match b.as_ref() {
            Ex::Name(_) | Ex::Attr(..) => key(b).is_some_and(|k| g.tups.contains(&k)),
            b => is_tup(b, g),
        },
        Ex::IfExp(_, a, b) | Ex::Or(a, b) => is_tup(a, g) || is_tup(b, g),
        // `(5,) + tuple(ranks)` written so is a real (JSON) tuple in the typed build; `tuple(a) + b` and the
        // like, joined onto a list kept for tuple(...), are lists too.
        Ex::Bin("+", _, _) if tuple_join(e, g) => false,
        Ex::Bin("+", a, b) => is_tup(a, g) || is_tup(b, g),
        _ => false,
    }
}

// A tuple written out joined with a tuple(...) value, which the typed build makes a JSON tuple:
// `(x,) + tuple(xs)` as written (parser.rs, `tup_join`), and `(3, trip) + kickers` with kickers a
// tuple(...) of like items (infer.rs). (Other item types do not build typed, so they never print wrong.)
fn tuple_join(e: &Ex, g: &G) -> bool {
    let Ex::Bin("+", a, b) = e else { return false };
    match a.as_ref() {
        Ex::Tuple(v) if v.len() == 1 => matches!(b.as_ref(), Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(n) if n == "tuple") && args.len() == 1),
        Ex::Tuple(v) if v.len() >= 2 => is_tup(b, g),
        _ => false,
    }
}

// A function that returns a real tuple on one path, `return (8, high)`, and on another one the typed build
// keeps as a list, `return (5,) + tuple(ranks)`: one value then holds a tuple or a list, which print
// differently and do not compare item by item, so sorting by it gives another order than CPython's.
// Return values that are tuples of one length are looked at item by item (`return score, cat`).
fn mixed_tuple_returns(d: &FuncDef, g: &G) -> bool {
    fn rets(body: &[Stmt], out: &mut Vec<Ex>) {
        for s in body {
            match &s.st {
                St::Return(Some(v)) => out.push(v.clone()),
                St::Def(_) | St::Class(_) => {}
                st => each_block(st, &mut |b| rets(b, out)),
            }
        }
    }
    fn mixed(vs: &[&Ex], g: &G) -> bool {
        if let Some(Ex::Tuple(first)) = vs.first() {
            let n = first.len();
            if vs.iter().all(|v| matches!(v, Ex::Tuple(t) if t.len() == n)) {
                return (0..n).any(|i| {
                    let col: Vec<&Ex> = vs.iter().map(|v| if let Ex::Tuple(t) = v { &t[i] } else { unreachable!() }).collect();
                    mixed(&col, g)
                });
            }
        }
        let listed = |v: &Ex| !tuple_join(v, g) && (is_tup(v, g) || is_tup1(v, g) || matches!(v, Ex::Bin("+", a, b) if is_tup1(a, g) || is_tup1(b, g)));
        let real = |v: &Ex| tuple_join(v, g) || matches!(v, Ex::Tuple(t) if t.len() >= 2) && !listed(v);
        vs.iter().any(|v| real(v)) && vs.iter().any(|v| listed(v))
    }
    let mut vs = vec![];
    rets(&d.body, &mut vs);
    vs.len() >= 2 && mixed(&vs.iter().collect::<Vec<_>>(), g)
}

fn is_holder(e: &Ex, g: &G) -> bool {
    match e {
        Ex::List(v) | Ex::Tuple(v) => v.iter().any(|x| is_set(x, g) || is_holder(x, g)),
        Ex::Dict(items) => items.iter().any(|(_, v)| is_set(v, g) || is_holder(v, g)),
        Ex::Comp(k, elt, val, _) => *k != 's' && (is_set(val.as_deref().unwrap_or(elt), g) || is_holder(val.as_deref().unwrap_or(elt), g)),
        Ex::Call(f, args) => match f.as_ref() {
            Ex::Name(n) if n == "defaultdict" => args.iter().any(|a| match a {
                Arg::Pos(Ex::Name(t)) => t == "set" || t == "frozenset",
                Arg::Pos(Ex::Lambda(_, b)) => is_set(b, g),
                _ => false,
            }),
            Ex::Name(n) if matches!(n.as_str(), "dict" | "list" | "tuple" | "sorted" | "reversed" | "zip" | "enumerate") => args.iter().any(|a| matches!(a, Arg::Pos(x) | Arg::Star(x) if is_holder(x, g) || (n != "sorted" && n != "list" && n != "tuple" && is_set(x, g)))),
            Ex::Name(n) => g.holder_fns.contains(n),
            Ex::Attr(r, m) => (matches!(m.as_str(), "items" | "values" | "copy") && is_holder(r, g)) || g.holder_fns.contains(m.as_str()),
            _ => false,
        },
        Ex::Name(_) | Ex::Attr(..) => key(e).is_some_and(|k| g.holders.contains(&k)),
        Ex::Index(b, _) => is_holder(b, g),
        Ex::IfExp(_, a, b) | Ex::Or(a, b) => is_holder(a, g) || is_holder(b, g),
        _ => false,
    }
}

// re.match(...), pat.search(...): a match, which a program that prints one builds over dynamic values.
fn is_match(e: &Ex, g: &G) -> bool {
    match e {
        Ex::Call(f, _) => matches!(f.as_ref(), Ex::Attr(_, m) if matches!(m.as_str(), "match" | "search" | "fullmatch")),
        Ex::Name(_) | Ex::Attr(..) => key(e).is_some_and(|k| g.matches.contains(&k)),
        Ex::Walrus(_, v) => is_match(v, g),
        _ => false,
    }
}

fn is_dict(e: &Ex) -> bool {
    match e {
        Ex::Dict(_) | Ex::Comp('d', ..) => true,
        Ex::Call(f, _) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "dict" | "defaultdict" | "Counter" | "OrderedDict")),
        _ => false,
    }
}

// `for k in d: d[k2] = v`: CPython stops with "dictionary changed size during iteration" (and for a set,
// "Set changed size"); the typed build walks a copy and goes on.
fn grows_while_walked(target: &Ex, it: &Ex, body: &[Stmt], g: &G) -> bool {
    // `for k in d: d[k] = v` sets keys that are there: no change of size.
    let mut own: Vec<String> = vec![];
    walk_ex(target, &mut |e| {
        if let Ex::Name(n) = e {
            own.push(n.clone());
        }
    });
    let base = match it {
        Ex::Call(f, a) if a.is_empty() => match f.as_ref() {
            Ex::Attr(r, m) if matches!(m.as_str(), "keys" | "items" | "values") => r.as_ref(),
            _ => return false,
        },
        x => x,
    };
    let Some(k) = key(base) else { return false };
    if !g.dicts.contains(&k) && !g.sets.contains(&k) {
        return false;
    }
    let mut hit = false;
    fn go(body: &[Stmt], k: &str, own: &[String], hit: &mut bool) {
        for s in body {
            let on_k = |e: &Ex| key(e).as_deref() == Some(k);
            match &s.st {
                St::Assign(ts, _) => {
                    if ts.iter().any(|t| matches!(t, Ex::Index(b, i) if on_k(b) && !matches!(i.as_ref(), Ex::Name(n) if own.contains(n)))) {
                        *hit = true;
                    }
                }
                St::Del(xs) => {
                    if xs.iter().any(|t| matches!(t, Ex::Index(b, _) if on_k(b))) {
                        *hit = true;
                    }
                }
                _ => {}
            }
            walk_st(&s.st, &mut |e| {
                if let Ex::Call(f, _) = e {
                    if let Ex::Attr(r, m) = f.as_ref() {
                        if on_k(r) && matches!(m.as_str(), "pop" | "popitem" | "setdefault" | "update" | "clear" | "add" | "discard" | "remove") {
                            *hit = true;
                        }
                    }
                }
            });
            if let St::Def(_) | St::Class(_) = &s.st {
                continue;
            }
            each_block(&s.st, &mut |b| go(b, k, own, hit));
        }
    }
    go(body, &k, &own, &mut hit);
    hit
}

fn bind(t: &Ex, v: &Ex, g: &mut G) {
    match t {
        Ex::Name(_) | Ex::Attr(..) => {
            let k = key(t).unwrap();
            let keys_only = matches!(v, Ex::Dict(items) if !items.iter().any(|(_, x)| is_tup(x, g)));
            if is_tup(v, g) {
                g.tups.insert(if keys_only { format!("k:{k}") } else { k.clone() });
            }
            if is_tup1(v, g) {
                g.tups1.insert(k.clone());
            }
            if is_match(v, g) {
                g.matches.insert(k.clone());
            }
            if is_dict(v) {
                g.dicts.insert(k.clone());
            }
            if is_set(v, g) {
                g.sets.insert(k);
            } else if is_holder(v, g) {
                g.holders.insert(k);
            }
        }
        Ex::Index(b, i) => {
            if is_tup(v, g) || is_tup(i, g) {
                if let Some(r) = root(b) {
                    g.tups.insert(if is_tup(v, g) { r } else { format!("k:{r}") });
                }
            }
            if is_set(v, g) || is_holder(v, g) {
                if let Some(r) = root(b) {
                    g.holders.insert(r);
                }
            }
        }
        // `a, b = s1, s2` and `for k, v in holder.items()`: every name may hold a set.
        Ex::Tuple(ts) | Ex::List(ts) => {
            if let Ex::Tuple(vs) | Ex::List(vs) = v {
                if vs.len() == ts.len() {
                    ts.iter().zip(vs).for_each(|(t, v)| bind(t, v, g));
                    return;
                }
            }
            if is_holder(v, g) {
                for t in ts {
                    if let Some(k) = key(t) {
                        g.sets.insert(k.clone());
                        g.holders.insert(k);
                    }
                }
            }
        }
        _ => {}
    }
}

fn learn(body: &[Stmt], func: Option<&str>, g: &mut G) {
    for s in body {
        match &s.st {
            St::Assign(ts, v) => ts.iter().for_each(|t| bind(t, v, g)),
            St::AnnAssign(t, Some(v), _) => bind(t, v, g),
            St::For(t, it, _, _) => {
                if is_holder(it, g) {
                    match t {
                        Ex::Name(_) | Ex::Attr(..) => {
                            let k = key(t).unwrap();
                            g.sets.insert(k.clone());
                            g.holders.insert(k);
                        }
                        _ => bind(t, it, g),
                    }
                }
            }
            St::Return(Some(v)) => {
                if let Some(f) = func {
                    if is_tup(v, g) {
                        g.tups.insert(format!("{f}()"));
                    }
                    if is_tup1(v, g) {
                        g.tups1.insert(format!("{f}()"));
                    }
                    if is_set(v, g) {
                        g.set_fns.insert(f.to_string());
                    } else if is_holder(v, g) {
                        g.holder_fns.insert(f.to_string());
                    }
                }
            }
            _ => {}
        }
        // Sets put into containers (`groups.append(s)`, `d.setdefault(k, set())`) and passed to functions.
        walk_st(&s.st, &mut |e| {
            if let Ex::Call(f, args) = e {
                let pos: Vec<&Ex> = args.iter().filter_map(|a| if let Arg::Pos(x) = a { Some(x) } else { None }).collect();
                if let Ex::Attr(r, m) = f.as_ref() {
                    if matches!(m.as_str(), "append" | "appendleft" | "insert" | "add") && pos.last().is_some_and(|x| is_tup1(x, g)) {
                        if let Some(k) = key(r) {
                            g.tups1.insert(format!("e:{k}"));
                        }
                    }
                    if matches!(m.as_str(), "append" | "appendleft" | "insert" | "extend" | "add" | "setdefault" | "update" | "put" | "push") && pos.iter().any(|x| is_tup(x, g)) {
                        if let Some(k) = root(r) {
                            g.tups.insert(k);
                        }
                    }
                    if matches!(m.as_str(), "append" | "appendleft" | "insert" | "extend" | "add" | "setdefault" | "update" | "put" | "push") && pos.iter().any(|x| is_set(x, g) || is_holder(x, g)) {
                        if let Some(k) = root(r) {
                            g.holders.insert(k);
                        }
                    }
                }
                if let Ex::Name(n) = f.as_ref() {
                    if let Some((_, ps)) = g.params.iter().find(|(f, _)| f == n).cloned() {
                        for (p, a) in ps.iter().zip(pos.iter()) {
                            if is_set(a, g) {
                                g.sets.insert(p.clone());
                            } else if is_holder(a, g) {
                                g.holders.insert(p.clone());
                            }
                            // `perms(xs, cur, out)` whose body does `out.append(tuple(cur))`: the caller's list holds tuples.
                            if is_tup1(a, g) {
                                g.tups1.insert(p.clone());
                            }
                            if g.tups.contains(p) {
                                if let Some(r) = root(a) {
                                    g.tups.insert(r);
                                }
                            }
                        }
                    }
                }
            }
        });
        match &s.st {
            St::Def(d) => learn(&d.body, Some(&d.name), g),
            St::Class(c) => learn(&c.body, None, g),
            st => each_block(st, &mut |b| learn(b, func, g)),
        }
    }
}

// Every expression in a statement (not in nested blocks' statements, which each_block visits).
// Does this statement (or a block inside it) change the list, dict or set named x in place?
fn changes_name(st: &St, x: &str) -> bool {
    const MUT: &[&str] = &["append", "extend", "insert", "remove", "pop", "clear", "sort", "reverse", "add", "discard", "update", "setdefault", "popitem", "appendleft", "popleft", "difference_update", "intersection_update"];
    let is_x = |e: &Ex| matches!(e, Ex::Name(n) if n == x);
    let target_in = |e: &Ex| matches!(e, Ex::Index(b, _) if is_x(b));
    let hit = match st {
        St::Assign(ts, _) => ts.iter().any(|t| target_in(t)),
        St::AugAssign(t, _, _) => is_x(t) || target_in(t),
        St::Del(xs) => xs.iter().any(|t| target_in(t)),
        _ => false,
    };
    if hit {
        return true;
    }
    let mut found = false;
    walk_st(st, &mut |e| {
        if let Ex::Call(f, _) = e {
            if let Ex::Attr(r, m) = f.as_ref() {
                if is_x(r) && MUT.contains(&m.as_str()) {
                    found = true;
                }
            }
        }
    });
    if found {
        return true;
    }
    if matches!(st, St::Def(_) | St::Class(_)) {
        return false;
    }
    let mut inner = false;
    each_block(st, &mut |b| inner = inner || b.iter().any(|s| changes_name(&s.st, x)));
    inner
}

pub(crate) fn walk_st(st: &St, f: &mut dyn FnMut(&Ex)) {
    let mut go = |e: &Ex| walk_ex(e, f);
    match st {
        St::Expr(e) | St::Return(Some(e)) => go(e),
        St::Assign(ts, v) => {
            ts.iter().for_each(&mut go);
            go(v);
        }
        St::AugAssign(t, _, v) => {
            go(t);
            go(v);
        }
        St::AnnAssign(t, v, _) => {
            go(t);
            if let Some(v) = v {
                go(v);
            }
        }
        St::If(arms, _) => arms.iter().for_each(|(c, _)| go(c)),
        St::While(c, _, _) => go(c),
        St::For(t, it, _, _) => {
            go(t);
            go(it);
        }
        St::Assert(c, m) => {
            go(c);
            if let Some(m) = m {
                go(m);
            }
        }
        St::Raise(Some(e), _) => go(e),
        St::With(items, _) => items.iter().for_each(|(e, _)| go(e)),
        St::Del(xs) => xs.iter().for_each(&mut go),
        _ => {}
    }
}

pub(crate) fn walk_ex(e: &Ex, f: &mut dyn FnMut(&Ex)) {
    f(e);
    // `3 ** 50 % m` of number literals: one value, worked out when the typed build compiles.
    if let Ex::Bin("%", l, r) = e {
        if matches!(l.as_ref(), Ex::Bin("**", a, b) if lit_int(a).is_some() && lit_int(b).is_some()) && lit_int(r).is_some() {
            return;
        }
    }
    let mut go = |x: &Ex| walk_ex(x, f);
    match e {
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(&mut go),
        Ex::Dict(items) => items.iter().for_each(|(k, v)| {
            if let Some(k) = k {
                go(k);
            }
            go(v);
        }),
        Ex::Star(x) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => go(x),
        Ex::Attr(x, _) => go(x),
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            go(a);
            go(b);
        }
        Ex::Slice(a, b, c) => [a, b, c].into_iter().flatten().for_each(|x| go(x)),
        Ex::Call(fx, args) => {
            go(fx);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => go(x),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            go(a);
            rest.iter().for_each(|(_, x)| go(x));
        }
        Ex::IfExp(a, b, c) => {
            go(a);
            go(b);
            go(c);
        }
        Ex::Lambda(_, b) => go(b),
        Ex::Comp(_, elt, val, fors) => {
            go(elt);
            if let Some(v) = val {
                go(v);
            }
            for CompFor { target, iter, ifs, .. } in fors {
                go(target);
                go(iter);
                ifs.iter().for_each(&mut go);
            }
        }
        Ex::Yield(Some(x)) => go(x),
        Ex::FStr(parts) => fparts(parts, &mut go),
        _ => {}
    }
}

fn fparts(parts: &[FPart], go: &mut dyn FnMut(&Ex)) {
    for p in parts {
        if let FPart::Expr(x, _, spec, _) = p {
            go(x);
            fparts(spec, go);
        }
    }
}

// Each hit: (line, why, the names a set-order hit is on; None when the typed build cannot take it). Stops
// at the first hit that is not a set-order one.
type Hit = (usize, String, Option<Vec<String>>);

fn uses(body: &[Stmt], g: &G, out: &mut Vec<Hit>) {
    for (k, s) in body.iter().enumerate() {
        if out.iter().any(|h| h.2.is_none()) {
            return;
        }
        let line = s.line;
        let mut hit: Option<String> = None;
        let mut soft: Option<Vec<String>> = None;
        // `book = asks if buy else bids`, then `book.remove(o)`: in Python book is one of the two lists, so
        // the change shows in it; the typed build changes a copy.
        if let St::Assign(ts, Ex::IfExp(_, a, b)) = &s.st {
            if let ([Ex::Name(x)], true) = (ts.as_slice(), matches!(a.as_ref(), Ex::Name(_) | Ex::Attr(..)) && matches!(b.as_ref(), Ex::Name(_) | Ex::Attr(..))) {
                if body[k + 1..].iter().any(|st| changes_name(&st.st, x)) {
                    hit = Some(format!("{x} is one of two lists picked by a condition and is changed later; Python changes that list itself"));
                }
            }
        }
        if let St::For(t, it, lb, le) = &s.st {
            if is_set(it, g) && !order_free_loop(t, it, lb, le.as_deref(), g) {
                hit = Some("a loop over a set walks it in CPython's hash order".into());
                soft = soft_keys(it, g);
            }
        }
        // The typed build stops at these errors (a Rust panic) instead of raising them where an except can
        // catch them; only errors from failing calls (int("x"): ValueError) are caught there.
        // ZeroDivisionError and IndexError are raised catchably inside a try that names them (emit.rs).
        if let St::Try(_, hs, _, _) = &s.st {
            const UNCAUGHT: &[&str] = &["ArithmeticError", "AssertionError", "EOFError", "KeyError", "LookupError", "OverflowError", "RecursionError"];
            for h in hs {
                let mut names = vec![];
                if let Some(t) = &h.types {
                    walk_ex(t, &mut |e| {
                        if let Ex::Name(n) = e {
                            names.push(n.clone());
                        }
                    });
                }
                if let Some(n) = names.iter().find(|n| UNCAUGHT.contains(&n.as_str())) {
                    hit = Some(format!("except {n}: the typed build stops at this error instead of raising it"));
                }
            }
        }
        // `return`, `break` or `continue` leaving a try that has a finally: the typed build leaves without running
        // the finally (and a return there was lost: the function returned None).
        if let St::Try(b, hs, e, Some(fin)) = &s.st {
            if hit.is_none() && (jumps_out(b, false) || hs.iter().any(|h| jumps_out(&h.body, false)) || e.as_deref().is_some_and(|e| jumps_out(e, false)) || jumps_out(fin, false)) {
                hit = Some("return, break or continue leaves a try that has a finally, which the typed build does not run then".into());
            }
        }
        if let St::For(t, it, body, _) = &s.st {
            if hit.is_none() && grows_while_walked(t, it, body, g) {
                hit = Some("changing a dict or set while a loop walks it stops CPython with a RuntimeError".into());
            }
        }
        if let St::Class(c) = &s.st {
            if hit.is_none() && c.body.iter().any(|m| matches!(&m.st, St::Def(d) if d.name == "__del__")) {
                hit = Some("__del__ runs when CPython frees the object, which the typed build does not track".into());
            }
        }
        if let St::Def(d) = &s.st {
            if hit.is_none() {
                if let Some(n) = shared_growth(d) {
                    hit = Some(format!("`{n}` can grow past 64 bits and is shared with another function (a closure, nonlocal or global), where the typed build keeps it a 64-bit int"));
                }
            }
            if hit.is_none() && mixed_tuple_returns(d, g) {
                hit = Some(format!("{}() returns a tuple on one path and one built with + or tuple(...) on another, which the typed build keeps as a list", d.name));
            }
            if hit.is_none() && own_yield(&d.body) && !pure_gen(d, g) {
                hit = Some("the typed build runs a generator to the end at its first use, where CPython runs it step by step with its caller".into());
            }
        }
        // A comprehension handed straight to max/sorted/any/len...: its order does not show.
        let mut free: Vec<*const Ex> = vec![];
        walk_st(&s.st, &mut |e| {
            if hit.is_some() {
                return;
            }
            if let Some(x) = order_free_call(e) {
                free.push(x as *const Ex);
            }
            if free.contains(&(e as *const Ex)) {
                return;
            }
            if let Some((w, k)) = shown(e, g) {
                hit = Some(w);
                soft = k;
            }
        });
        if let Some(why) = hit {
            let set_order = why.contains("hash order");
            out.push((line, why, soft.filter(|_| set_order)));
            if out.last().unwrap().2.is_none() {
                return;
            }
        }
        match &s.st {
            St::Def(d) => uses(&d.body, g, out),
            St::Class(c) => uses(&c.body, g, out),
            st => each_block(st, &mut |b| uses(b, g, out)),
        }
    }
}

// Names this body (not a function defined inside it) makes grow without bound: `f *= i`, `h = h * 31 + c`,
// `a, b = b, a + b`.
fn growing(body: &[Stmt], out: &mut HashSet<String>) {
    fn has_mul_of(e: &Ex, n: &str) -> bool {
        let mut found = false;
        walk_ex(e, &mut |x| {
            if let Ex::Bin("*" | "**" | "<<", a, b) = x {
                if matches!(a.as_ref(), Ex::Name(m) if m == n) || matches!(b.as_ref(), Ex::Name(m) if m == n) {
                    found = true;
                }
            }
        });
        found
    }
    fn name_sum(e: &Ex, k: &mut usize) -> bool {
        match e {
            Ex::Bin("+", a, b) => name_sum(a, k) && name_sum(b, k),
            Ex::Name(_) => {
                *k += 1;
                true
            }
            _ => false,
        }
    }
    for s in body {
        match &s.st {
            St::AugAssign(Ex::Name(n), op, _) if matches!(*op, "*" | "**" | "<<") => {
                out.insert(n.clone());
            }
            St::AugAssign(Ex::Name(n), "+", v) if has_mul_of(v, n) => {
                out.insert(n.clone());
            }
            St::Assign(ts, v) => {
                for t in ts {
                    match (t, v) {
                        (Ex::Name(n), v) if has_mul_of(v, n) => {
                            out.insert(n.clone());
                        }
                        (Ex::Tuple(ns), Ex::Tuple(vs)) if ns.len() == vs.len() => {
                            for (n, v) in ns.iter().zip(vs) {
                                let mut k = 0;
                                if let Ex::Name(n) = n {
                                    if (name_sum(v, &mut k) && k >= 2) || has_mul_of(v, n) {
                                        out.insert(n.clone());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            St::Def(_) | St::Class(_) => {}
            st => each_block(st, &mut |b| growing(b, out)),
        }
    }
}

fn declared(body: &[Stmt], out: &mut HashSet<String>) {
    for s in body {
        match &s.st {
            St::Global(ns) | St::Nonlocal(ns) => out.extend(ns.iter().cloned()),
            St::Def(_) | St::Class(_) => {}
            st => each_block(st, &mut |b| declared(b, out)),
        }
    }
}

// Names read inside the functions (and classes) defined in body.
fn nested_names(body: &[Stmt], out: &mut HashSet<String>) {
    for s in body {
        match &s.st {
            St::Def(d) => names_in(&d.body, out),
            St::Class(c) => names_in(&c.body, out),
            st => each_block(st, &mut |b| nested_names(b, out)),
        }
    }
}

fn names_in(body: &[Stmt], out: &mut HashSet<String>) {
    for s in body {
        walk_st(&s.st, &mut |e| {
            if let Ex::Name(n) = e {
                out.insert(n.clone());
            }
        });
        match &s.st {
            St::Def(d) => names_in(&d.body, out),
            St::Class(c) => names_in(&c.body, out),
            st => each_block(st, &mut |b| names_in(b, out)),
        }
    }
}

// A growing int the typed build cannot make big because another function shares it: a name that grows here
// and that a function defined inside reads, or that this function declares global or nonlocal.
fn shared_growth(d: &crate::dynpy::parse::FuncDef) -> Option<String> {
    let mut g = HashSet::new();
    growing(&d.body, &mut g);
    if g.is_empty() {
        return None;
    }
    let mut decl = HashSet::new();
    declared(&d.body, &mut decl);
    if let Some(n) = g.iter().find(|n| decl.contains(*n)) {
        return Some(n.clone());
    }
    let mut used = HashSet::new();
    nested_names(&d.body, &mut used);
    g.iter().find(|n| used.contains(*n)).cloned()
}

// A yield in this body (not in a function defined inside it).
fn own_yield(body: &[Stmt]) -> bool {
    let mut found = false;
    for s in body {
        if let St::Def(_) | St::Class(_) = &s.st {
            continue;
        }
        walk_st(&s.st, &mut |e| {
            if matches!(e, Ex::Yield(_) | Ex::YieldFrom(_)) {
                found = true;
            }
        });
        each_block(&s.st, &mut |b| found |= own_yield(b));
    }
    found
}

fn find_pure_gens(body: &[Stmt], g: &mut G) {
    for s in body {
        match &s.st {
            St::Def(d) => {
                if own_yield(&d.body) && pure_gen(d, g) {
                    g.pure_gens.insert(d.name.clone());
                }
                find_pure_gens(&d.body, g);
            }
            St::Class(c) => find_pure_gens(&c.body, g),
            _ => {}
        }
    }
}

// The typed build makes a generator a function that returns the list of everything it yields. That gives
// CPython's answer only when running it early cannot be told apart: it ends, it reads nothing but its own
// arguments and locals (the caller cannot change what it sees part way), it prints and raises nothing, and
// no value is sent into it.
fn pure_gen(d: &crate::dynpy::parse::FuncDef, g: &G) -> bool {
    const PURE: &[&str] = &["range", "len", "abs", "min", "max", "sum", "str", "int", "float", "bool", "tuple", "enumerate", "zip", "reversed", "sorted", "ord", "chr", "divmod", "round", "isinstance", "True", "False", "None"];
    let mut locals: HashSet<String> = d.params.pos.iter().map(|(n, _)| n.clone()).collect();
    locals.extend(d.params.star.clone());
    locals.extend(d.params.kwonly.iter().map(|(n, _)| n.clone()));
    locals.extend(d.params.kwstar.clone());
    fn targets(e: &Ex, out: &mut HashSet<String>) {
        match e {
            Ex::Name(n) => {
                out.insert(n.clone());
            }
            Ex::Tuple(v) | Ex::List(v) => v.iter().for_each(|x| targets(x, out)),
            Ex::Star(x) => targets(x, out),
            _ => {}
        }
    }
    fn stmts_ok(body: &[Stmt], locals: &mut HashSet<String>) -> bool {
        body.iter().all(|s| match &s.st {
            St::Assign(ts, v) => {
                ts.iter().for_each(|t| targets(t, locals));
                ts.iter().all(|t| matches!(t, Ex::Name(_) | Ex::Tuple(_))) && !matches!(v, Ex::Yield(_) | Ex::YieldFrom(_))
            }
            St::AugAssign(t, _, _) => matches!(t, Ex::Name(_)),
            St::Expr(_) | St::Pass | St::Break | St::Continue | St::Return(_) => true,
            St::If(arms, els) => arms.iter().all(|(_, b)| stmts_ok(b, locals)) && els.as_ref().is_none_or(|b| stmts_ok(b, locals)),
            St::For(t, _, b, e) => {
                targets(t, locals);
                stmts_ok(b, locals) && e.as_ref().is_none_or(|b| stmts_ok(b, locals))
            }
            _ => false,
        })
    }
    if !stmts_ok(&d.body, &mut locals) {
        return false;
    }
    fn all_exprs(body: &[Stmt], f: &mut dyn FnMut(&Ex)) {
        for s in body {
            walk_st(&s.st, f);
            each_block(&s.st, &mut |b| all_exprs(b, f));
        }
    }
    all_exprs(&d.body, &mut |e| {
        if let Ex::Comp(_, _, _, fors) = e {
            fors.iter().for_each(|c| targets(&c.target, &mut locals));
        }
    });
    let mut ok = true;
    all_exprs(&d.body, &mut |e| match e {
        Ex::Name(n) if !locals.contains(n) && !PURE.contains(&n.as_str()) && !g.pure_gens.contains(n) && n != &d.name => ok = false,
        Ex::Attr(..) | Ex::Lambda(..) | Ex::Walrus(..) => ok = false,
        _ => {}
    });
    ok
}

fn lit_int(e: &Ex) -> Option<i64> {
    match e {
        Ex::Int(t) => t.replace('_', "").parse().ok(),
        Ex::Unary("-", x) => lit_int(x).and_then(|k| k.checked_neg()),
        _ => None,
    }
}

// An order-dependent use of a set at this expression (not looking inside it: walk_ex does that), with the
// names it is on (see soft_keys).
fn shown(e: &Ex, g: &G) -> Option<(String, Option<Vec<String>>)> {
    let why = shown_why(e, g)?;
    let keys = match e {
        Ex::Call(_, args) => soft_all(args.iter().filter_map(|a| if let Arg::Pos(x) | Arg::Star(x) = a { Some(x) } else { None }).chain(match e {
            Ex::Call(f, _) => match f.as_ref() {
                Ex::Attr(r, _) => Some(r.as_ref()),
                _ => None,
            },
            _ => None,
        }), g),
        Ex::FStr(parts) => {
            let mut xs = vec![];
            fparts(parts, &mut |x| xs.push(x.clone()));
            soft_all(xs.iter(), g)
        }
        Ex::Bin("%", _, r) => match r.as_ref() {
            Ex::Tuple(v) => soft_all(v.iter(), g),
            x => soft_all([x], g),
        },
        Ex::Comp(_, _, _, fors) => {
            let sets: Vec<&Ex> = fors.iter().map(|c| &c.iter).filter(|x| is_set(x, g)).collect();
            sets.iter().map(|x| soft_keys(x, g)).collect::<Option<Vec<_>>>().map(|v| v.concat())
        }
        _ => None,
    };
    Some((why, keys))
}

fn shown_why(e: &Ex, g: &G) -> Option<String> {
    let set_or_holder = |x: &Ex| is_set(x, g) || is_holder(x, g);
    // `d.items() <= e.items()`, `d.values() == e.values()`: views compare as sets (values by identity) in CPython.
    let view = |x: &Ex| matches!(x, Ex::Call(f, a) if a.is_empty() && matches!(f.as_ref(), Ex::Attr(_, m) if matches!(m.as_str(), "keys" | "items" | "values")));
    if let Ex::Cmp(first, rest) = e {
        if rest.iter().any(|(op, _)| matches!(*op, "<" | "<=" | ">" | ">=" | "==" | "!=")) && (view(first) || rest.iter().any(|(_, x)| view(x))) {
            return Some("comparing dict views (d.keys(), d.items(), d.values()) follows set rules the typed build does not".into());
        }
    }
    match e {
        Ex::Call(f, args) => {
            let pos: Vec<&Ex> = args.iter().filter_map(|a| if let Arg::Pos(x) | Arg::Star(x) = a { Some(x) } else { None }).collect();
            match f.as_ref() {
                Ex::Name(n) if matches!(n.as_str(), "print" | "str" | "repr" | "format" | "ascii") => {
                    if pos.iter().any(|x| set_or_holder(x)) {
                        return Some(format!("{n}() of a set shows CPython's hash order"));
                    }
                    if pos.iter().any(|x| is_match(x, g)) {
                        return Some(format!("{n}() of a re match shows a Match object or None"));
                    }
                    if pos.iter().any(|x| is_tup(x, g)) {
                        return Some(format!("{n}() of tuple(...) shows a tuple, which the typed build keeps as a list"));
                    }
                    if args.iter().any(|a| matches!(a, Arg::Pos(x) if is_tup1(x, g))) {
                        return Some(format!("{n}() of a one-item tuple shows (x,), which the typed build keeps as a list"));
                    }
                    if n == "print" {
                        return print_changes(&pos).or_else(|| later_changes(&pos, g).map(|k| format!("print() reads every argument before writing; a later one changes {k}")));
                    }
                }
                Ex::Name(n) if matches!(n.as_str(), "list" | "tuple" | "enumerate" | "zip" | "iter" | "next" | "map" | "filter" | "reversed") => {
                    if pos.iter().any(|x| is_set(x, g)) {
                        return Some(format!("{n}() of a set takes CPython's hash order"));
                    }
                }
                // copy.copy(rows) shares the inner lists in Python; the typed build copies them too.
                Ex::Attr(r, m) if m == "copy" && matches!(r.as_ref(), Ex::Name(n) if n == "copy") => {
                    return Some("copy.copy() shares the items it copies".into());
                }
                Ex::Attr(r, m) => {
                    if m == "pop" && is_set(r, g) && pos.is_empty() {
                        return Some("set.pop() takes the first item in CPython's hash order".into());
                    }
                    if matches!(m.as_str(), "join" | "format" | "extend" | "writerow" | "writelines" | "dumps") && pos.iter().any(|x| set_or_holder(x) || is_mapped_set(x, g)) {
                        return Some(format!(".{m}() of a set takes CPython's hash order"));
                    }
                }
                _ => {}
            }
            None
        }
        Ex::Tuple(v) | Ex::List(v) => later_changes(&v.iter().collect::<Vec<_>>(), g).map(|k| format!("a tuple or list holds the object itself; a later item changes {k}")),
        Ex::FStr(parts) => {
            let mut bad = false;
            let mut m = false;
            let mut tup = false;
            fparts(parts, &mut |x| {
                bad |= set_or_holder(x);
                m |= is_match(x, g);
                tup |= is_tup(x, g) || is_tup1(x, g);
            });
            if tup {
                return Some("an f-string of tuple(...) shows a tuple, which the typed build keeps as a list".into());
            }
            if m {
                return Some("an f-string of a re match shows a Match object or None".into());
            }
            bad.then(|| "an f-string of a set shows CPython's hash order".to_string())
        }
        // `"%s" % t` with t a one-item tuple held in a name: Python fills in its item, the typed build the list.
        Ex::Bin("%", l, r) if matches!(l.as_ref(), Ex::Str(_)) && !matches!(r.as_ref(), Ex::Tuple(_)) && is_tup1(r, g) => Some("% of a one-item tuple held in a name fills in its item, but the typed build keeps it as a list".into()),
        Ex::Bin("%", l, r) if matches!(l.as_ref(), Ex::Str(_)) && matches!(r.as_ref(), Ex::Tuple(v) if v.iter().any(|x| is_tup(x, g) || is_tup1(x, g))) => Some("% of tuple(...) shows a tuple, which the typed build keeps as a list".into()),
        Ex::Bin("%", l, r) if matches!(l.as_ref(), Ex::Str(_)) && (set_or_holder(r) || matches!(r.as_ref(), Ex::Tuple(v) if v.iter().any(|x| set_or_holder(x)))) => Some("% of a set shows CPython's hash order".into()),
        Ex::Comp(k, _, _, fors) if *k != 's' && fors.iter().any(|c| is_set(&c.iter, g)) => Some("a comprehension over a set walks it in CPython's hash order".into()),
        // `2 ** 100` is past 64 bits (`3 ** 50 % m`, taken modulo a number, is worked out by the typed build).
        Ex::Bin("**", a, b) => match (lit_int(a), lit_int(b)) {
            (Some(a), Some(b)) if b >= 0 && (b > u32::MAX as i64 || a.checked_pow(b as u32).is_none()) => Some("this power is past 64 bits, where Python's ints keep growing".into()),
            // `2 ** (F * K)`: many fields packed into one int (a bit-parallel trick), far past 64 bits.
            (Some(2), None) if matches!(b.as_ref(), Ex::Bin("*", ..)) => Some("2 ** (a * b) packs many fields into one int, past 64 bits".into()),
            _ => None,
        },
        Ex::Bin("<<", _, b) if matches!(b.as_ref(), Ex::Bin("*", ..)) => Some("x << (a * b) packs many fields into one int, past 64 bits".into()),
        _ => None,
    }
}

// `map(str, s)` handed to join.
fn is_mapped_set(x: &Ex, g: &G) -> bool {
    match x {
        Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(n) if n == "map" || n == "filter") => args.iter().any(|a| matches!(a, Arg::Pos(y) if is_set(y, g))),
        Ex::Comp(_, _, _, fors) => fors.iter().any(|c| is_set(&c.iter, g)),
        _ => false,
    }
}

// Methods that change their receiver, and standard-library functions that change their first argument.
const CHANGING: &[&str] = &["pop", "popleft", "popitem", "append", "appendleft", "add", "remove", "insert", "extend", "extendleft", "update", "setdefault", "clear", "sort", "reverse", "discard", "rotate", "move_to_end", "difference_update", "intersection_update", "symmetric_difference_update", "heappop", "heappush"];
const CHANGING_FNS: &[&str] = &["heappop", "heappush", "heapify", "heapreplace", "heappushpop", "insort", "insort_left", "insort_right", "shuffle"];

// print(q, q.popleft()): an argument changes a name another argument shows.
fn print_changes(args: &[&Ex]) -> Option<String> {
    let mut changed: Vec<(usize, String)> = vec![];
    for (i, a) in args.iter().enumerate() {
        walk_ex(a, &mut |e| {
            if let Ex::Call(f, cargs) = e {
                match f.as_ref() {
                    Ex::Attr(r, m) if CHANGING.contains(&m.as_str()) => {
                        if let Some(k) = root(r) {
                            changed.push((i, k));
                        }
                    }
                    // heapq.heappop(h)
                    Ex::Attr(_, m) | Ex::Name(m) if CHANGING_FNS.contains(&m.as_str()) => {
                        if let Some(Arg::Pos(x)) = cargs.first() {
                            if let Some(k) = root(x) {
                                changed.push((i, k));
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
    for (i, k) in &changed {
        for (j, a) in args.iter().enumerate() {
            if j == *i {
                continue;
            }
            let mut names = false;
            walk_ex(a, &mut |e| names |= key(e).as_ref() == Some(k));
            if names {
                return Some(format!("print() reads every argument before writing; one changes {}", k.trim_start_matches('.')));
            }
        }
    }
    None
}

// print(xs, grow(xs)) or (q, q.pop()): the name a later argument changes in place, when an earlier one holds
// that object. The typed build has already copied the earlier one by then, and a copy cannot show the change.
// `print(n, bump())` with an int n needs no check: both read n first, and Python's int does not change.
fn later_changes(args: &[&Ex], g: &G) -> Option<String> {
    for j in 1..args.len() {
        let mut changed = vec![];
        walk_ex(args[j], &mut |e| call_changes(e, &g.fn_changes, &mut changed));
        let mut shown = vec![];
        args[..j].iter().for_each(|a| held(a, &mut shown));
        if let Some(k) = changed.iter().find(|k| shown.contains(k)) {
            return Some(k.trim_start_matches('.').to_string());
        }
    }
    None
}

// The names (and `.attr` names) whose objects the value of `e` may be, or may hold. Values made fresh
// (`len(xs)`, `str(xs)`, an f-string, a comparison, what `xs.pop()` took out) hold none.
fn held(e: &Ex, out: &mut Vec<String>) {
    const FRESH: &[&str] = &["len", "sum", "str", "repr", "ascii", "format", "int", "float", "bool", "abs", "round", "any", "all", "ord", "chr", "hash", "id", "type", "isinstance", "bin", "hex", "oct"];
    // pygen::NEXT is `next(g)`: it holds what the generator gave, not the generator.
    const FRESH_METHODS: &[&str] = &[crate::pygen::NEXT, "join", "count", "index", "find", "rfind", "format", "startswith", "endswith", "upper", "lower", "casefold", "strip", "lstrip", "rstrip", "split", "replace", "title"];
    match e {
        Ex::Name(n) => out.push(n.clone()),
        Ex::Attr(b, a) => {
            out.push(format!(".{a}"));
            held(b, out);
        }
        Ex::Index(b, _) | Ex::Star(b) | Ex::Walrus(_, b) => held(b, out),
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| held(x, out)),
        Ex::Dict(items) => items.iter().for_each(|(_, v)| held(v, out)),
        Ex::IfExp(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) | Ex::Bin(_, a, b) => {
            held(a, out);
            held(b, out);
        }
        Ex::Comp(_, _, _, fors) => fors.iter().for_each(|c| held(&c.iter, out)),
        Ex::Call(f, args) => {
            match f.as_ref() {
                Ex::Name(n) if FRESH.contains(&n.as_str()) => return,
                Ex::Attr(_, m) if FRESH_METHODS.contains(&m.as_str()) || CHANGING.contains(&m.as_str()) => return,
                Ex::Attr(r, _) => held(r, out),
                _ => {}
            }
            for a in args {
                let (Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x)) = a;
                held(x, out);
            }
        }
        _ => {}
    }
}

// What a function changes in place: its parameters by position (ANY_PARAM: one of `*args`, a keyword-only
// parameter or `**kw`), and names from outside it (globals, a closure's names).
#[derive(Default, PartialEq)]
struct Changes {
    params: HashSet<usize>,
    names: HashSet<String>,
}
const ANY_PARAM: usize = usize::MAX;

// What each function changes, called directly or through the functions it calls. Keys: `f` for a function,
// `.m` for a method (self is parameter 0, and classes with the same method are joined), `C` for what
// `C(...)` changes through __init__.
fn program_changes(body: &[Stmt]) -> HashMap<String, Changes> {
    let mut all: HashMap<String, Changes> = HashMap::new();
    loop {
        let mut next: HashMap<String, Changes> = HashMap::new();
        each_def(body, false, &mut |k, d, skip| {
            let c = changes_of(d, &all);
            let e = next.entry(k).or_default();
            e.params.extend(c.params.iter().filter(|p| **p >= skip).map(|p| if *p == ANY_PARAM { *p } else { p - skip }));
            e.names.extend(c.names);
        });
        if next == all {
            return all;
        }
        all = next;
    }
}

// Every def in the program, with its key in program_changes and how many leading parameters a call does not
// pass (a class's __init__ does not get self from the arguments).
fn each_def(body: &[Stmt], in_class: bool, f: &mut dyn FnMut(String, &FuncDef, usize)) {
    for s in body {
        match &s.st {
            St::Def(d) => {
                f(if in_class { format!(".{}", d.name) } else { d.name.clone() }, d, 0);
                each_def(&d.body, false, f);
            }
            St::Class(c) => {
                for m in &c.body {
                    if let St::Def(d) = &m.st {
                        if d.name == "__init__" {
                            f(c.name.clone(), d, 1);
                        }
                    }
                }
                each_def(&c.body, true, f);
            }
            st => each_block(st, &mut |b| each_def(b, in_class, f)),
        }
    }
}

// What one function changes, given what the functions it calls change (`known`).
fn changes_of(d: &FuncDef, known: &HashMap<String, Changes>) -> Changes {
    #[derive(Default)]
    struct Scan {
        local: HashSet<String>,
        outer: HashSet<String>,                 // named by `global` or `nonlocal`
        from: HashMap<String, HashSet<String>>, // a local name -> what it may be the same object as
        changed: Vec<String>,
    }
    // `target = value`: a name is bound to what the value holds; an item or attribute is changed.
    fn bind(t: &Ex, v: &Ex, sc: &mut Scan) {
        match t {
            Ex::Index(..) | Ex::Attr(..) => held(t, &mut sc.changed),
            _ => {
                let mut objs = vec![];
                held(v, &mut objs);
                walk_ex(t, &mut |x| {
                    if let Ex::Name(n) = x {
                        sc.local.insert(n.clone());
                        sc.from.entry(n.clone()).or_default().extend(objs.iter().cloned());
                    }
                });
            }
        }
    }
    fn scan(b: &[Stmt], known: &HashMap<String, Changes>, sc: &mut Scan) {
        for s in b {
            match &s.st {
                St::Def(d) => {
                    sc.local.insert(d.name.clone());
                    continue;
                }
                St::Class(c) => {
                    sc.local.insert(c.name.clone());
                    continue;
                }
                St::Global(v) | St::Nonlocal(v) => sc.outer.extend(v.iter().cloned()),
                St::Assign(ts, v) => ts.iter().for_each(|t| bind(t, v, sc)),
                St::AnnAssign(t, Some(v), _) => bind(t, v, sc),
                St::For(t, it, ..) => bind(t, it, sc),
                St::With(items, _) => items.iter().for_each(|(e, t)| {
                    if let Some(t) = t {
                        bind(t, e, sc);
                    }
                }),
                // `count += 1` makes a new int; `xs += [1]` changes the list.
                St::AugAssign(Ex::Name(n), _, Ex::Int(_) | Ex::Float(_)) => {
                    sc.local.insert(n.clone());
                }
                St::AugAssign(t, ..) => {
                    if let Ex::Name(n) = t {
                        sc.local.insert(n.clone());
                    }
                    held(t, &mut sc.changed);
                }
                St::Del(ts) => ts.iter().filter(|t| matches!(t, Ex::Index(..) | Ex::Attr(..))).for_each(|t| held(t, &mut sc.changed)),
                _ => {}
            }
            walk_st(&s.st, &mut |e| call_changes(e, known, &mut sc.changed));
            each_block(&s.st, &mut |b| scan(b, known, sc));
        }
    }
    let mut sc = Scan::default();
    let p = &d.params;
    let pos: Vec<&String> = p.pos.iter().map(|x| &x.0).collect();
    let rest: Vec<&String> = p.star.iter().chain(&p.kwstar).chain(p.kwonly.iter().map(|x| &x.0)).collect();
    sc.local.extend(pos.iter().chain(&rest).map(|n| n.to_string()));
    scan(&d.body, known, &mut sc);
    let mut out = Changes::default();
    let mut seen = HashSet::new();
    while let Some(n) = sc.changed.pop() {
        if n.starts_with('.') || !seen.insert(n.clone()) {
            continue;
        }
        if sc.outer.contains(&n) || !sc.local.contains(&n) {
            out.names.insert(n.clone());
        } else if let Some(i) = pos.iter().position(|x| **x == n) {
            out.params.insert(i);
        } else if rest.iter().any(|x| **x == n) {
            out.params.insert(ANY_PARAM);
        }
        if let Some(objs) = sc.from.get(&n) {
            sc.changed.extend(objs.iter().cloned());
        }
    }
    out
}

// The names whose objects this call (not looking inside it: walk_ex does that) changes in place.
fn call_changes(e: &Ex, known: &HashMap<String, Changes>, out: &mut Vec<String>) {
    let Ex::Call(f, args) = e else { return };
    let (user, skip) = match f.as_ref() {
        Ex::Attr(r, m) => {
            if CHANGING.contains(&m.as_str()) {
                held(r, out);
            }
            (known.get(&format!(".{m}")), 1)
        }
        Ex::Name(n) => (known.get(n), 0),
        _ => (None, 0),
    };
    if matches!(f.as_ref(), Ex::Attr(_, m) | Ex::Name(m) if CHANGING_FNS.contains(&m.as_str())) {
        if let Some(Arg::Pos(x)) = args.first() {
            held(x, out);
        }
    }
    if let Some(c) = user {
        out.extend(c.names.iter().cloned());
        let any = c.params.contains(&ANY_PARAM);
        if let (Ex::Attr(r, _), true) = (f.as_ref(), any || c.params.contains(&0)) {
            held(r, out);
        }
        for (i, a) in args.iter().enumerate() {
            match a {
                Arg::Pos(x) if any || c.params.contains(&(i + skip)) => held(x, out),
                // Which parameter these reach is not followed here.
                Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) if !c.params.is_empty() => held(x, out),
                _ => {}
            }
        }
    }
    // A function handed over by name (`apply(grow, xs)`) may be called on anything given with it.
    for a in args {
        let Arg::Pos(Ex::Name(n)) = a else { continue };
        let Some(c) = known.get(n) else { continue };
        out.extend(c.names.iter().cloned());
        if !c.params.is_empty() {
            for b in args {
                let (Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x)) = b;
                held(x, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check;
    const GROW: &str = "def grow(v):\n    v.append(9)\n    return len(v)\nxs = [1]\n";
    const BOX: &str = "class Box:\n    def __init__(self):\n        self.items = [1]\n    def put(self, x):\n        self.items.append(x)\n        return x\n";
    const LOG: &str = "log = [0]\nhits = 0\ndef note(x):\n    global hits\n    hits += 1\n    log.append(x)\n    return hits\n";

    fn verdict(p: &str) -> Option<String> {
        check(p).map(|x| x.1)
    }

    // A later argument changes what an earlier one holds: not typed.
    #[test]
    fn later_change_leaves_the_typed_build() {
        for tail in ["print(xs, grow(xs))", "print(xs, grow(v=xs))", "print(xs, grow(xs), sep='|')", "print((xs, 1), grow(xs))", "print([xs], grow(xs))", "print({'k': xs}, grow(xs))", "u = (xs, grow(xs))", "u = [xs, grow(xs)]", "print(xs[0:1], grow(xs))"] {
            assert!(verdict(&format!("{GROW}{tail}\n")).is_some(), "{tail}");
        }
        let cases = [
            "def via(rows):\n    r = rows[0]\n    r.append(7)\n    return 0\nrows = [[1]]\nprint(rows[0], via(rows))\n".to_string(),
            "def walk(rows):\n    for r in rows:\n        r[0] = 7\n    return 0\nrows = [[1]]\nprint(rows, walk(rows))\n".to_string(),
            "def wipe(d):\n    del d['a']\n    return 0\nd = {'a': 1}\nprint(d, wipe(d))\n".to_string(),
            "def more(v):\n    v += [1]\n    return 0\nxs = [1]\nprint(xs, more(xs))\n".to_string(),
            "import heapq\ndef take(h):\n    return heapq.heappop(h)\nh = [1, 2]\nprint(h, take(h))\n".to_string(),
            format!("{GROW}def outer(v):\n    return grow(v)\nprint(xs, outer(xs))\n"),
            format!("{GROW}def apply(f, v):\n    return f(v)\nprint(xs, apply(grow, xs))\n"),
            format!("{LOG}print(log, note(1))\n"),
            format!("{LOG}def again():\n    return note(2)\nprint(log, again())\n"),
            "log = [0]\ndef alias():\n    out = log\n    out.append(1)\n    return 0\nprint(log, alias())\n".to_string(),
            format!("{BOX}b = Box()\nprint(b, b.put(2))\n"),
            format!("{BOX}def fill(b):\n    b.items.append(3)\n    return 0\nb = Box()\nprint(b.items, fill(b))\n"),
            "class K:\n    def __init__(self, v):\n        v.append(5)\nxs = [1]\nprint(xs, K(xs))\n".to_string(),
            "def outer():\n    acc = [1]\n    def add():\n        acc.append(2)\n        return 0\n    print(acc, add())\nouter()\n".to_string(),
            "q = [1, 2]\nr = (q, q.pop())\nprint(r)\n".to_string(),
        ];
        for p in &cases {
            assert!(verdict(p).is_some(), "{p}");
        }
    }

    // Nothing shown is changed after it was read: stays typed.
    #[test]
    fn no_later_change_stays_typed() {
        let cases = [
            format!("{GROW}print(grow(xs), xs)\n"),
            format!("{GROW}print(len(xs), grow(xs))\n"),
            format!("{GROW}print(str(xs), grow(xs))\n"),
            format!("{GROW}print(f'{{xs}}', grow(xs))\n"),
            format!("{GROW}ys = [2]\nprint(ys, grow(xs))\n"),
            crate::pygen::rewrite("def rng(s):\n    x = s\n    while True:\n        x += 1\n        yield x\nr = rng(1)\nprint(next(r), next(r) % 100)\n"),
            format!("{LOG}print(hits, note(1))\n"),
            format!("{LOG}ys = [2]\nprint(ys, note(1))\n"),
            "def fib(n, memo={}):\n    return n\nprint(5, fib(5))\n".to_string(),
            "def f(v):\n    out = []\n    out.append(v[0])\n    return out\nxs = [1]\nprint(xs, f(xs))\n".to_string(),
            "def f(v):\n    v = [2]\n    return v\nxs = [1]\nprint(xs, f(xs))\n".to_string(),
            "s = [1, 2, 3]\na, c = s.pop(), s.pop()\nprint(a, c, s)\n".to_string(),
            "s = [1, 2, 3]\nr = (s.pop(), s)\nprint(r)\n".to_string(),
        ];
        for p in &cases {
            assert_eq!(verdict(p), None, "{p}");
        }
    }
    fn needs(src: &str) -> bool {
        check(src).is_some()
    }
    #[test]
    fn sets_shown_or_walked() {
        assert!(needs("s = {3, -1}\nprint(s)\n"));
        assert!(needs("s = set()\ns.add(5)\nfor x in s:\n    print(x)\n"));
        assert!(needs("from collections import defaultdict\ng = defaultdict(set)\ng[1].add(2)\nprint(dict(g))\n"));
        assert!(needs("def f():\n    return {1, 2}\nprint(f())\n"));
        assert!(needs("s = {1, 2}\nprint(', '.join(map(str, s)))\n"));
        assert!(needs("s = {1, 2}\nxs = list(s)\n"));
        assert!(needs("s = {1, 2}\nprint(f'{s}')\n"));
        assert!(needs("s = {1, 2}\nprint([x for x in s])\n"));
        assert!(needs("t = tuple(map(int, '1 2'.split()))\nprint(t)\n"));
        assert!(needs("u = []\nu.append(tuple([1, 2]))\nprint(f'{u}')\n"));
        // Through a helper's parameter, and through a function's result.
        assert!(needs("def go(cur, out):\n    out.append(tuple(cur))\ndef main():\n    res = []\n    go([1, 2], res)\n    print(res)\nmain()\n"));
        assert!(needs("def f():\n    out = []\n    out.append(tuple([1, 2]))\n    return out\nprint(f())\n"));
        assert!(needs("seen = {tuple([1, 2]): 0}\nprint(seen)\n"));
        assert!(!needs("seen = {tuple([1, 2]): 0}\ns = seen[tuple([1, 2])]\nprint('%d' % (s,))\n"));
        // A one-item tuple shown whole (kept as a one-item list), but not one used as a key, a `%` argument
        // list, joined to a real tuple, or inside a real tuple read back by index.
        assert!(needs("t = ('a',)\nprint(t)\n"));
        assert!(needs("print([(3,), (4,)])\n"));
        assert!(needs("d = {(1,): 'x'}\nprint(d)\n"));
        assert!(needs("print((1,) + (2,))\n"));
        assert!(needs("out = []\nfor x in 'ab':\n    out.append((x,))\nprint(out[0])\n"));
        assert!(needs("def f(x):\n    return (x,)\nprint(f(1))\n"));
        assert!(needs("def show(t):\n    print(t)\nshow((1,))\n"));
        assert!(needs("t = (1,)\nprint(f'{t}')\n"));
        assert!(!needs("d = {('a',): 1}\nprint(d[('a',)], '%s' % ('x',))\n"));
        assert!(!needs("ids = ('a', 'b')\nprint(ids + ('c',))\n"));
        assert!(needs("t = (5,)\nprint('%s' % t)\n"));
        assert!(needs("d = {1: 1}\ne = {1: 2}\nprint(d.items() <= e.items())\n"));
        assert!(needs("class F:\n    def __del__(self):\n        print('bye')\nx = F()\n"));
        // A jump out of a try with a finally; a loop of its own inside the try is fine.
        assert!(needs("def f(x):\n    try:\n        return x\n    finally:\n        print('done')\nprint(f(1))\n"));
        assert!(needs("for i in range(3):\n    try:\n        continue\n    finally:\n        print(i)\n"));
        assert!(!needs("def f(xs):\n    try:\n        for x in xs:\n            if x:\n                break\n    finally:\n        print('done')\n    return 1\nprint(f([1]))\n"));
        assert!(!needs("n = 5\nprint('%d items' % (n,))\n"));
        assert!(!needs("best = {}\nv = (0, ('s',), 'x')\nc = (v[0] + 1, v[1] + ('u',), 'via')\nbest[1] = c\nans = best[1]\nprint('%s' % ans[2], len(ans[1]))\n"));
    }
    #[test]
    fn order_free_set_use_stays_typed() {
        assert!(!needs("s = set()\ns.add(5)\nprint(5 in s, len(s), sorted(s), max(s), sum(s))\n"));
        assert!(!needs("seen = set()\nfor i in range(9):\n    if i % 3 not in seen:\n        seen.add(i % 3)\nprint(len(seen))\n"));
        assert!(!needs("xs = [3, 1]\nprint(xs, sorted(set(xs)))\n"));
    }
    #[test]
    fn unread_file_is_not_safe() {
        let src = "x = 3\nmatch x:\n    case 3:\n        pass\nprint({3, -1, 5})\n";
        assert_eq!(super::check_soft(src).map_err(|e| e.0), Err(2));
        assert!(needs(src));
        // except* is read now, so the set print is found.
        let src = "try:\n    pass\nexcept* ValueError:\n    pass\nprint({3, -1, 5})\n";
        assert_eq!(super::check_soft(src).map(|x| x.map(|x| x.0)), Ok(Some(5)));
    }
    #[test]
    fn string_set_walks_wait_for_rustc() {
        use super::check_soft;
        // Named for the typed build to check (rustc proves the items are strings), not refused outright.
        assert_eq!(check_soft("ws = set('a b a'.split())\nfor w in ws:\n    print(w)\n").unwrap().map(|x| x.2), Some(vec!["s:ws".to_string()]));
        // A set made by a function, or bound as a loop variable, cannot be checked by name.
        assert_eq!(check_soft("def f():\n    return {'a'}\nfor w in f():\n    print(w)\n").unwrap().map(|x| x.2), Some(vec![]));
        assert_eq!(check_soft("for s in [{'a'}]:\n    for w in s:\n        print(w)\n").unwrap().map(|x| x.2.is_empty()), Some(true));
        // Order-free consumers of a comprehension over a set.
        assert!(!needs("s = {3, 1}\nprint(max(x * 2 for x in s), sorted(x for x in s), any(x > 2 for x in s))\n"));
        assert!(needs("s = {3, 1}\nprint(max((x for x in s), key=lambda v: v % 2))\n"));
    }
    #[test]
    fn print_argument_changed_later() {
        assert!(needs("from collections import deque\nq = deque([1, 2])\nprint(q, q.popleft())\n"));
        assert!(!needs("q = [1, 2]\nprint(q.pop(), len([]))\n"));
    }
    #[test]
    fn matches_errors_and_loops() {
        assert!(needs("import re\nprint(re.match('a', 'b'))\n"));
        assert!(!needs("import re\nm = re.match('a', 'ab')\nprint(m.group() if m else '')\n"));
        assert!(!needs("try:\n    x = [1][3]\nexcept IndexError:\n    pass\n"));
        assert!(!needs("try:\n    x = 1 // 0\nexcept ZeroDivisionError:\n    pass\n"));
        assert!(!needs("try:\n    x = int('q')\nexcept ValueError:\n    pass\n"));
        assert!(needs("d = {1: 2}\nfor k in d:\n    d[k + 1] = 0\n"));
        assert!(needs("import copy\nb = copy.copy([[1]])\n"));
        assert!(needs("a = []\nb = []\nbook = a if len(a) else b\nbook.append(1)\nprint(b)\n"));
        assert!(needs("a = [1]\nb = []\ndef f(c):\n    book = a if c else b\n    if book:\n        book.remove(1)\n"));
        assert!(!needs("a = [1]\nb = [2]\nbook = a if len(a) else b\nprint(len(book))\n"));
        assert!(!needs("import re\nm = re.match('(a)(b)?', 'a')\nprint(m.group(2) is None)\n"));
        assert!(needs("import re\nm = re.match('(a)(b)?', 'a')\nprint(m.groups())\n"));
        assert!(!needs("d = {1: 2}\nfor k in d:\n    d[k] = 0\n"));
        // an item set on the list being walked may change an item the loop has not reached (the sharing proof)
        assert!(needs("xs = [1]\nfor x in xs:\n    xs[0] = 2\n"));
    }
}

// Capturing groups in a regular expression: `(` not escaped, not in a class, not `(?:`, `(?=`... (`(?P<n>` counts).
pub(crate) fn re_groups(p: &str) -> usize {
    let b: Vec<char> = p.chars().collect();
    let (mut i, mut n, mut class) = (0, 0, false);
    while i < b.len() {
        match b[i] {
            '\\' => i += 1,
            '[' if !class => {
                class = true;
                // `[]...]` and `[^]...]`: a first `]` is a member.
                if b.get(i + 1) == Some(&'^') {
                    i += 1;
                }
                if b.get(i + 1) == Some(&']') {
                    i += 1;
                }
            }
            ']' if class => class = false,
            '(' if !class => {
                if b.get(i + 1) != Some(&'?') || (b.get(i + 2) == Some(&'P') && b.get(i + 3) == Some(&'<')) {
                    n += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    n
}

// `re.findall` with two or more groups gives a list of tuples, which the typed build (a list of strings)
// cannot hold; a pattern it cannot see (not a literal, or a name set once by re.compile of one) may have them.
fn findall_groups(body: &[Stmt]) -> Option<(usize, String)> {
    fn all(body: &[Stmt], f: &mut dyn FnMut(usize, &Ex)) {
        for s in body {
            let line = s.line;
            walk_st(&s.st, &mut |e| f(line, e));
            match &s.st {
                St::Def(d) => all(&d.body, f),
                St::Class(c) => all(&c.body, f),
                st => each_block(st, &mut |b| all(b, f)),
            }
        }
    }
    let lit = |e: &Ex| if let Ex::Str(p) = e { Some(p.clone()) } else { None };
    let pos = |a: &[Arg], k: usize| match a.get(k) {
        Some(Arg::Pos(x)) => Some(x.clone()),
        _ => None,
    };
    // name -> the pattern of each `name = re.compile("...")` (None: set some other way too).
    let mut pats: HashMap<String, Option<String>> = HashMap::new();
    fn sets(body: &[Stmt], f: &mut dyn FnMut(&St)) {
        for s in body {
            f(&s.st);
            match &s.st {
                St::Def(d) => sets(&d.body, f),
                St::Class(c) => sets(&c.body, f),
                st => each_block(st, &mut |b| sets(b, f)),
            }
        }
    }
    sets(body, &mut |st| {
        if let St::Assign(ts, v) = st {
            for t in ts {
                let n = match t {
                    Ex::Name(n) => n.clone(),
                    Ex::Attr(_, n) => format!(".{n}"),
                    _ => continue,
                };
                let p = match v {
                    Ex::Call(f, a) if matches!(f.as_ref(), Ex::Attr(m, c) if c == "compile" && matches!(m.as_ref(), Ex::Name(r) if r == "re")) => pos(a, 0).as_ref().and_then(lit),
                    _ => None,
                };
                let e = pats.entry(n).or_insert(p.clone());
                if *e != p {
                    *e = None;
                }
            }
        }
    });
    let mut hit = None;
    all(body, &mut |line, e| {
        if hit.is_some() {
            return;
        }
        let Ex::Call(f, a) = e else { return };
        let Ex::Attr(on, m) = f.as_ref() else { return };
        if m != "findall" {
            return;
        }
        let pat = match on.as_ref() {
            Ex::Name(r) if r == "re" => pos(a, 0).as_ref().and_then(lit),
            Ex::Name(n) => pats.get(n).cloned().flatten(),
            Ex::Attr(_, n) => pats.get(&format!(".{n}")).cloned().flatten(),
            _ => None,
        };
        if pat.as_deref().map_or(true, |p| re_groups(p) >= 2) {
            hit = Some((line, "re.findall with two or more groups gives tuples (or a pattern the typed build cannot see)".to_string()));
        }
    });
    hit
}

// The typed build keeps a compiled pattern as its pattern string, and reads `p.match(s)`, `p.search(s)`,
// `p.fullmatch(s)`, `p.findall(s)`, `p.finditer(s)` and `p.sub(r, s)` as the re functions. Any other use would
// see the string: `p.split(s)` is str.split, repr(p) and print(p) show the string, `p.pattern` and `p.flags` are
// not there. Those send the file to the dynamic build, whose patterns are re.Pattern objects.
fn pattern_uses(body: &[Stmt]) -> Option<(usize, String)> {
    fn all(body: &[Stmt], f: &mut dyn FnMut(usize, &Ex)) {
        for s in body {
            let line = s.line;
            walk_st(&s.st, &mut |e| f(line, e));
            match &s.st {
                St::Def(d) => all(&d.body, f),
                St::Class(c) => all(&c.body, f),
                st => each_block(st, &mut |b| all(b, f)),
            }
        }
    }
    fn sets(body: &[Stmt], f: &mut dyn FnMut(&St)) {
        for s in body {
            f(&s.st);
            match &s.st {
                St::Def(d) => sets(&d.body, f),
                St::Class(c) => sets(&c.body, f),
                st => each_block(st, &mut |b| sets(b, f)),
            }
        }
    }
    let compiled = |e: &Ex| matches!(e, Ex::Call(f, _) if matches!(f.as_ref(), Ex::Attr(m, c) if c == "compile" && matches!(m.as_ref(), Ex::Name(r) if r == "re")));
    // names (".attr" for attributes) set to re.compile(...) anywhere
    let mut names: HashSet<String> = HashSet::new();
    sets(body, &mut |st| {
        if let St::Assign(ts, v) = st {
            if compiled(v) {
                for t in ts {
                    match t {
                        Ex::Name(n) => names.insert(n.clone()),
                        Ex::Attr(_, n) => names.insert(format!(".{n}")),
                        _ => false,
                    };
                }
            }
        }
    });
    let is_pat = |e: &Ex| {
        compiled(e)
            || match e {
                Ex::Name(n) => names.contains(n),
                Ex::Attr(_, n) => names.contains(&format!(".{n}")),
                _ => false,
            }
    };
    let mut hit = None;
    all(body, &mut |line, e| {
        if hit.is_some() {
            return;
        }
        let why = match e {
            Ex::Attr(on, m) if is_pat(on) && !matches!(m.as_str(), "match" | "fullmatch" | "search" | "findall" | "finditer" | "sub") => Some(format!("a compiled pattern's .{m}")),
            Ex::Call(f, a) if matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "repr" | "str" | "print" | "format" | "type" | "hash" | "id" | "len" | "list" | "tuple" | "isinstance"))
                && a.iter().any(|a| matches!(a, Arg::Pos(x) if is_pat(x))) =>
            {
                Some("a compiled pattern shown or passed to a builtin".into())
            }
            Ex::FStr(parts) if parts.iter().any(|p| matches!(p, crate::dynpy::parse::FPart::Expr(x, ..) if is_pat(x))) => Some("a compiled pattern in an f-string".into()),
            _ => None,
        };
        if let Some(w) = why {
            hit = Some((line, format!("{w}, which the typed build keeps as a plain string")));
        }
    });
    hit
}

// Reading a missing key of a defaultdict puts it in (Python calls the factory), where the typed build reads
// the zero value and leaves the dict as it was. That shows only when the program looks at the dict's keys
// (len, a loop, `in`, print, passing it on): then a read that may miss sends the file to the dynamic build.
fn default_reads(body: &[Stmt]) -> Option<(usize, String)> {
    fn sts(body: &[Stmt], f: &mut dyn FnMut(&Stmt)) {
        for s in body {
            f(s);
            match &s.st {
                St::Def(d) => sts(&d.body, f),
                St::Class(c) => sts(&c.body, f),
                st => each_block(st, &mut |b| sts(b, f)),
            }
        }
    }
    let key = |e: &Ex| match e {
        Ex::Name(n) => Some(n.clone()),
        Ex::Attr(o, n) if matches!(o.as_ref(), Ex::Name(s) if s == "self") => Some(format!("self.{n}")),
        _ => None,
    };
    let is_dd = |v: &Ex| matches!(v, Ex::Call(f, _) if matches!(f.as_ref(), Ex::Name(n) if n == "defaultdict") || matches!(f.as_ref(), Ex::Attr(m, n) if n == "defaultdict" && matches!(m.as_ref(), Ex::Name(c) if c == "collections")));
    // An immutable value a name may hold while the typed build copies it for each missing key: a scalar,
    // or a tuple of them. Lists, sets and dicts are out: copying a mutable object per key differs from
    // CPython, which puts the one object in (`defaultdict(lambda: base)` with base = [1]).
    fn pure(e: &Ex) -> bool {
        match e {
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::True | Ex::False | Ex::NoneV | Ex::Ellipsis => true,
            Ex::Tuple(xs) => xs.iter().all(pure),
            Ex::Call(f, args) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "int" | "float" | "str" | "Decimal" | "Fraction" | "tuple")) && args.iter().all(|a| match a { Arg::Pos(e) => pure(e), _ => false }),
            _ => false,
        }
    }
    let mut consts: HashSet<String> = HashSet::new();
    let mut bad: HashSet<String> = HashSet::new();
    sts(body, &mut |s| match &s.st {
        St::Assign(ts, v) => {
            let p = pure(v);
            for t in ts {
                if let Ex::Name(n) = t {
                    if !p {
                        bad.insert(n.clone());
                    }
                    if !bad.contains(n) && p {
                        consts.insert(n.clone());
                    } else {
                        consts.remove(n);
                    }
                }
            }
        }
        St::AugAssign(t, _, _) => {
            if let Ex::Name(n) = t {
                bad.insert(n.clone());
            }
        }
        St::Del(ts) => {
            for t in ts {
                if let Ex::Name(n) = t {
                    bad.insert(n.clone());
                }
            }
        }
        St::For(t, _, _, _) => {
            if let Ex::Name(n) = t {
                bad.insert(n.clone());
            }
        }
        _ => {}
    });
    // What the typed build's `lax_def` can copy for each missing key: the parser's safe_default forms
    // (literals and containers of them, built-in conversions), with a name only when it is a constant.
    // Mutable containers are no longer copied: the parser stores a constructor that builds them afresh,
    // so their parts must be literals (a name's object cannot be rebuilt by that constructor).
    fn imm(e: &Ex, consts: &HashSet<String>) -> bool {
        match e {
            Ex::Name(n) => consts.contains(n),
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::True | Ex::False | Ex::NoneV | Ex::Ellipsis => true,
            Ex::Tuple(xs) => xs.iter().all(|x| imm(x, consts)),
            Ex::Call(f, args) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "int" | "float" | "str" | "Decimal" | "Fraction" | "tuple")) && args.iter().all(|a| matches!(a, Arg::Pos(e) if imm(e, consts))),
            _ => false,
        }
    }
    fn lit(e: &Ex) -> bool {
        match e {
            Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::True | Ex::False | Ex::NoneV | Ex::Ellipsis => true,
            Ex::Tuple(xs) => xs.iter().all(lit),
            Ex::Call(f, args) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "int" | "float" | "str" | "Decimal" | "Fraction" | "tuple")) && args.iter().all(|a| matches!(a, Arg::Pos(e) if lit(e))),
            _ => false,
        }
    }
    fn fresh(e: &Ex) -> bool {
        match e {
            Ex::List(xs) | Ex::Set(xs) | Ex::Tuple(xs) => xs.iter().all(|x| lit(x) || fresh(x)),
            Ex::Dict(items) => items.iter().all(|(k, v)| k.as_ref().map_or(true, |k| lit(k) || fresh(k)) && (lit(v) || fresh(v))),
            Ex::Call(f, args) => matches!(f.as_ref(), Ex::Name(n) if matches!(n.as_str(), "list" | "dict" | "set" | "tuple")) && args.iter().all(|a| matches!(a, Arg::Pos(e) if lit(e) || fresh(e))),
            _ => false,
        }
    }
    fn safe_template(e: &Ex, consts: &HashSet<String>) -> bool {
        imm(e, consts) || fresh(e)
    }
    // The typed build inserts the key for reads through these names; anything else still goes dynamic.
    let mut dds: HashMap<String, bool> = HashMap::new();
    let mut bad_at: Option<usize> = None;
    sts(body, &mut |s| match &s.st {
        St::Assign(ts, v) if is_dd(v) => {
            let ok = match v {
                Ex::Call(_, args) => match args.first() {
                    None => true,
                    Some(Arg::Pos(e)) => match e {
                        Ex::Lambda(p, body) => p.pos.is_empty() && safe_template(body, &consts),
                        e => matches!(e, Ex::Name(n) if matches!(n.as_str(), "int" | "float" | "str" | "list" | "set" | "dict" | "tuple" | "bool" | "Decimal" | "Fraction")),
                    },
                    Some(Arg::Kw(n, e)) if n == "lambda" => safe_template(e, &consts),
                    _ => false,
                },
                _ => false,
            };
            if !ok {
                bad_at.get_or_insert(s.line);
            }
            ts.iter().filter_map(key).for_each(|k| {
                dds.insert(k, ok);
            });
        }
        St::AnnAssign(t, Some(v), _) if is_dd(v) => {
            if let Some(k) = key(t) {
                let ok = match v {
                    Ex::Call(_, args) => match args.first() {
                        None => true,
                        Some(Arg::Pos(e)) => match e {
                            Ex::Lambda(p, body) => p.pos.is_empty() && safe_template(body, &consts),
                            e => matches!(e, Ex::Name(n) if matches!(n.as_str(), "int" | "float" | "str" | "list" | "set" | "dict" | "tuple" | "bool" | "Decimal" | "Fraction")),
                        },
                        Some(Arg::Kw(n, e)) if n == "lambda" => safe_template(e, &consts),
                        _ => false,
                    },
                    _ => false,
                };
                if !ok {
                    bad_at.get_or_insert(s.line);
                }
                dds.insert(k, ok);
            }
        }
        _ => {}
    });
    if dds.is_empty() {
        return None;
    }
    // A factory the typed build cannot copy per missing key (a name that may hold a mutable object, a
    // user function, ...): CPython puts one object in every missing key, so the typed build must not run.
    if let Some(line) = bad_at {
        return Some((line, "reading a missing key of a defaultdict puts the key in, which the typed build does not".into()));
    }
    // Methods on d[k] that change it (and so go through the typed build's insert-on-miss path).
    const CHANGES: &[&str] = &["append", "extend", "add", "update", "insert", "appendleft", "discard", "remove", "pop", "popleft", "clear", "setdefault", "sort"];
    // A loop over a defaultdict's keys (`for k in d`, `sorted(d)`, `d.items()`): reading `d2[k]` with that k
    // finds the key (in d; also taken as found in another defaultdict filled by the same loop, `inn[mo]` and
    // `out[mo]`), so it puts nothing in.
    fn over_keys(it: &Ex, dds: &HashMap<String, bool>, key: &dyn Fn(&Ex) -> Option<String>) -> bool {
        match it {
            Ex::Call(f, a) => match (f.as_ref(), a.first()) {
                (Ex::Attr(o, m), _) if matches!(m.as_str(), "keys" | "items") => key(o).is_some_and(|k| dds.contains_key(&k)),
                (Ex::Name(n), Some(Arg::Pos(x))) if matches!(n.as_str(), "sorted" | "list" | "reversed") => over_keys(x, dds, key),
                _ => false,
            },
            x => key(x).is_some_and(|k| dds.contains_key(&k)),
        }
    }
    struct Scan<'a> {
        dds: &'a HashMap<String, bool>,
        key: &'a dyn Fn(&Ex) -> Option<String>,
        read_at: Option<usize>,
        seen: bool,
        is_at: Option<usize>,
    }
    fn scan(body: &[Stmt], keys: &[String], sc: &mut Scan) {
        let mut keys = keys.to_vec();
        for s in body {
            // `a = min(d, key=...)`: a is one of d's keys from here on.
            if let St::Assign(ts, Ex::Call(f, a)) = &s.st {
                if let ([Ex::Name(n)], Ex::Name(m), Some(Arg::Pos(x))) = (ts.as_slice(), f.as_ref(), a.first()) {
                    if matches!(m.as_str(), "min" | "max") && over_keys(x, sc.dds, sc.key) {
                        keys.push(n.clone());
                    }
                }
            }
            let keys = keys.as_slice();
            let mut safe: Vec<*const Ex> = vec![];
            let mut inner: Vec<*const Ex> = vec![];
            match &s.st {
                St::Assign(ts, _) => ts.iter().for_each(|t| safe.push(t as *const Ex)),
                St::AugAssign(t, _, _) => safe.push(t as *const Ex),
                St::Del(xs) => xs.iter().for_each(|t| safe.push(t as *const Ex)),
                _ => {}
            }
            let (dds, key) = (sc.dds, sc.key);
            let (mut read_at, mut seen, mut is_at) = (sc.read_at, sc.seen, sc.is_at);
            // `d[k] is d[m]` (and `is not`) compares object identity; the typed build's values have none.
            let dd_index = |x: &Ex| matches!(x, Ex::Index(b, _) if matches!(b.as_ref(), Ex::Name(_)) && key(b).is_some_and(|k| dds.get(&k) == Some(&true)));
            // `max(d, key=lambda k: d[k])`: the lambda is given the keys.
            let mut lam: Vec<String> = vec![];
            walk_st(&s.st, &mut |e| {
                match e {
                    Ex::Call(f, a) => {
                        if let (Ex::Name(n), Some(Arg::Pos(x))) = (f.as_ref(), a.first()) {
                            if matches!(n.as_str(), "min" | "max" | "sorted") && over_keys(x, dds, key) {
                                for k in a {
                                    if let Arg::Kw(w, Ex::Lambda(p, _)) = k {
                                        if w == "key" {
                                            lam.extend(p.pos.first().map(|q| q.0.clone()));
                                        }
                                    }
                                }
                            }
                        }
                        if let Ex::Attr(o, m) = f.as_ref() {
                            if CHANGES.contains(&m.as_str()) && matches!(o.as_ref(), Ex::Index(..)) {
                                safe.push(o.as_ref() as *const Ex);
                            }
                            // `d.get(k)`, `d.setdefault(...)`, `d.pop(k, ...)`: the dict itself is not looked at.
                            if key(o).is_some_and(|k| dds.contains_key(&k)) && matches!(m.as_str(), "get" | "setdefault" | "pop") {
                                inner.push(o.as_ref() as *const Ex);
                            }
                        }
                    }
                    Ex::Index(b, i) if key(b).is_some_and(|k| dds.contains_key(&k)) => {
                        inner.push(b.as_ref() as *const Ex);
                        let found = matches!(i.as_ref(), Ex::Name(k) if keys.contains(k) || lam.contains(k));
                        // A read through a known defaultdict name now inserts the key in the typed build.
                        let handled = key(b).is_some_and(|k| dds.get(&k) == Some(&true)) && matches!(b.as_ref(), Ex::Name(_));
                        if !safe.contains(&(e as *const Ex)) && !found && !handled {
                            read_at.get_or_insert(s.line);
                        }
                    }
                    // `self.d` reached through its Attr: the Name `self` under it is not the dict.
                    Ex::Attr(o, _) if key(e).is_some_and(|k| dds.contains_key(&k)) => {
                        if !inner.contains(&(e as *const Ex)) {
                            seen = true;
                        }
                        inner.push(o.as_ref() as *const Ex);
                    }
                    Ex::Cmp(a, ops) => {
                        for (op, b) in ops {
                            let fixed = matches!(*b, Ex::NoneV | Ex::True | Ex::False | Ex::Ellipsis);
                            if matches!(*op, "is" | "is not") && !fixed && (dd_index(a) || dd_index(b)) {
                                is_at.get_or_insert(s.line);
                            }
                        }
                    }
                    Ex::Name(n) if dds.contains_key(n) && !inner.contains(&(e as *const Ex)) && !safe.contains(&(e as *const Ex)) => seen = true,
                    _ => {}
                }
            });
            sc.read_at = read_at;
            sc.seen = seen;
            sc.is_at = is_at;
            match &s.st {
                St::Def(d) => scan(&d.body, &[], sc),
                St::Class(c) => scan(&c.body, &[], sc),
                St::For(t, it, b, els) => {
                    let mut ks = keys.to_vec();
                    if over_keys(it, sc.dds, sc.key) {
                        match t {
                            Ex::Name(k) => ks.push(k.clone()),
                            Ex::Tuple(v) => {
                                if let Some(Ex::Name(k)) = v.first() {
                                    ks.push(k.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    scan(b, &ks, sc);
                    if let Some(e) = els {
                        scan(e, keys, sc);
                    }
                }
                st => each_block(st, &mut |b| scan(b, keys, sc)),
            }
        }
    }
    let mut sc = Scan { dds: &dds, key: &key, read_at: None, seen: false, is_at: None };
    scan(body, &[], &mut sc);
    let (read_at, seen) = (sc.read_at, sc.seen);
    if let Some(line) = sc.is_at {
        return Some((line, "comparing defaultdict entries with is needs object identity, which the typed build does not have".into()));
    }
    match (read_at, seen) {
        (Some(line), true) => Some((line, "reading a missing key of a defaultdict puts the key in, which the typed build does not".into())),
        _ => None,
    }
}

// Calls whose only effect is their value. Methods are by name, whatever the receiver.
const PURE_CALLS: &[&str] = &["len", "abs", "min", "max", "int", "str", "float", "bool", "sorted", "isinstance", "ord", "chr", "range", "tuple", "list", "set", "frozenset", "sum", "any", "all", "enumerate", "zip", "round", "divmod", "repr", "reversed", "map", "filter"];
const PURE_METHODS: &[&str] = &["get", "lower", "casefold", "upper", "strip", "lstrip", "rstrip", "startswith", "endswith", "isdigit", "isalpha", "isalnum", "isspace", "isupper", "islower", "split", "join", "find", "rfind", "count", "index", "replace", "keys", "values", "items", "copy", "issubset", "issuperset", "union", "intersection", "difference"];

fn pure_ex(e: &Ex, fns: &HashSet<String>) -> bool {
    let mut ok = true;
    walk_ex(e, &mut |x| match x {
        Ex::Call(f, args) => {
            ok &= args.iter().all(|a| matches!(a, Arg::Pos(_) | Arg::Kw(..) | Arg::Star(_)));
            ok &= match f.as_ref() {
                Ex::Name(n) => PURE_CALLS.contains(&n.as_str()) || fns.contains(n),
                Ex::Attr(_, m) => PURE_METHODS.contains(&m.as_str()),
                _ => false,
            };
        }
        Ex::Walrus(..) | Ex::Yield(_) | Ex::YieldFrom(_) | Ex::Await(_) | Ex::Lambda(..) => ok = false,
        _ => {}
    });
    ok
}

// Functions with no effect but their value: no output, no global or nonlocal, no yield, no change to
// anything but names the function itself made from a literal (`seen = set()`), and calls only to such
// functions.
fn pure_fns(body: &[Stmt]) -> HashSet<String> {
    let mut defs: Vec<&FuncDef> = vec![];
    fn collect<'a>(b: &'a [Stmt], out: &mut Vec<&'a FuncDef>) {
        for s in b {
            if let St::Def(d) = &s.st {
                out.push(d);
            }
        }
    }
    collect(body, &mut defs);
    let mut pure: HashSet<String> = HashSet::new();
    loop {
        let before = pure.len();
        for d in &defs {
            if pure.contains(&d.name) {
                continue;
            }
            if fn_pure(d, &pure) {
                pure.insert(d.name.clone());
            }
        }
        if pure.len() == before {
            break;
        }
    }
    pure
}

fn fn_pure(d: &FuncDef, pure: &HashSet<String>) -> bool {
    // Names made here from a literal or an empty constructor.
    let mut fresh: HashSet<String> = HashSet::new();
    let mut ok = true;
    let mut fns = pure.clone();
    fns.insert(d.name.clone());
    fn go(b: &[Stmt], fresh: &mut HashSet<String>, fns: &HashSet<String>, ok: &mut bool) {
        for s in b {
            if !*ok {
                return;
            }
            match &s.st {
                St::Assign(ts, v) => {
                    for t in ts {
                        match t {
                            Ex::Name(n) => {
                                let lit = matches!(v, Ex::List(_) | Ex::Set(_) | Ex::Dict(_) | Ex::Comp(..)) || matches!(v, Ex::Call(f, a) if a.is_empty() && matches!(f.as_ref(), Ex::Name(c) if matches!(c.as_str(), "list" | "set" | "dict")));
                                if lit {
                                    fresh.insert(n.clone());
                                } else {
                                    fresh.remove(n);
                                }
                            }
                            Ex::Tuple(xs) | Ex::List(xs) if xs.iter().all(|x| matches!(x, Ex::Name(_))) => xs.iter().for_each(|x| {
                                if let Ex::Name(n) = x {
                                    fresh.remove(n);
                                }
                            }),
                            Ex::Index(b, _) if matches!(b.as_ref(), Ex::Name(n) if fresh.contains(n)) => {}
                            _ => *ok = false,
                        }
                    }
                    *ok &= pure_or_fresh(v, fresh, fns);
                }
                St::AugAssign(t, _, v) => {
                    *ok &= matches!(t, Ex::Name(_)) || matches!(t, Ex::Index(b, _) if matches!(b.as_ref(), Ex::Name(n) if fresh.contains(n)));
                    *ok &= pure_or_fresh(v, fresh, fns);
                }
                St::Expr(e) => *ok &= pure_or_fresh(e, fresh, fns),
                St::Return(e) => *ok &= e.as_ref().map_or(true, |e| pure_or_fresh(e, fresh, fns)),
                St::Pass | St::Break | St::Continue => {}
                St::If(arms, els) => {
                    for (c, b) in arms {
                        *ok &= pure_or_fresh(c, fresh, fns);
                        go(b, fresh, fns, ok);
                    }
                    if let Some(b) = els {
                        go(b, fresh, fns, ok);
                    }
                }
                St::While(c, b, e) => {
                    *ok &= pure_or_fresh(c, fresh, fns);
                    go(b, fresh, fns, ok);
                    if let Some(b) = e {
                        go(b, fresh, fns, ok);
                    }
                }
                St::For(t, it, b, e) => {
                    walk_ex(t, &mut |x| {
                        if let Ex::Name(n) = x {
                            fresh.remove(n);
                        }
                    });
                    *ok &= pure_or_fresh(it, fresh, fns);
                    go(b, fresh, fns, ok);
                    if let Some(b) = e {
                        go(b, fresh, fns, ok);
                    }
                }
                _ => *ok = false,
            }
        }
    }
    go(&d.body, &mut fresh, &fns, &mut ok);
    ok && d.params.pos.iter().chain(&d.params.kwonly).all(|(_, dflt)| dflt.as_ref().map_or(true, |x| matches!(x, Ex::Int(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False | Ex::Float(_))))
}

// As pure_ex, with changes (`seen.add(x)`) allowed on names the function made itself.
fn pure_or_fresh(e: &Ex, fresh: &HashSet<String>, fns: &HashSet<String>) -> bool {
    let mut ok = true;
    walk_ex(e, &mut |x| match x {
        Ex::Call(f, args) => {
            ok &= args.iter().all(|a| matches!(a, Arg::Pos(_) | Arg::Kw(..) | Arg::Star(_)));
            ok &= match f.as_ref() {
                Ex::Name(n) => PURE_CALLS.contains(&n.as_str()) || fns.contains(n),
                Ex::Attr(r, m) => PURE_METHODS.contains(&m.as_str()) || (matches!(r.as_ref(), Ex::Name(n) if fresh.contains(n)) && matches!(m.as_str(), "add" | "append" | "extend" | "insert" | "pop" | "remove" | "discard" | "update" | "setdefault" | "sort" | "clear")),
                _ => false,
            };
        }
        Ex::Walrus(..) | Ex::Yield(_) | Ex::YieldFrom(_) | Ex::Await(_) | Ex::Lambda(..) => ok = false,
        _ => {}
    });
    ok
}

// A loop over a set whose passes do not depend on each other's order: each only adds to sets, appends to
// the entry of its own element (`children[d].append(n)`), counts by a constant (`indeg[n] += 1`) or sets
// a flag to one constant, under conditions that change nothing and read none of what the loop changes.
// Whatever order the passes run in, they leave the same values.
fn order_free_loop(target: &Ex, it: &Ex, body: &[Stmt], els: Option<&[Stmt]>, g: &G) -> bool {
    let lv: Option<&String> = match target {
        Ex::Name(n) => Some(n),
        _ => None,
    };
    let walked = key(it);
    // What the passes change (bases), and the one constant each flag is given.
    let mut changed: HashSet<String> = HashSet::new();
    let mut sets_to: HashMap<String, String> = HashMap::new();
    let mut reads: Vec<&Ex> = vec![];
    let mut ok = true;
    fn go<'a>(b: &'a [Stmt], lv: Option<&String>, walked: &Option<String>, g: &G, changed: &mut HashSet<String>, sets_to: &mut HashMap<String, String>, reads: &mut Vec<&'a Ex>, ok: &mut bool) {
        for s in b {
            match &s.st {
                St::Pass | St::Continue => {}
                St::Expr(Ex::Call(f, args)) => match (f.as_ref(), args.as_slice()) {
                    (Ex::Attr(r, m), [Arg::Pos(a)]) if matches!(m.as_str(), "add" | "discard") || m == "append" => {
                        match r.as_ref() {
                            // `nxt.add(q + 1)`: a set (not the one walked).
                            Ex::Name(n) if m != "append" && walked.as_deref() != Some(n.as_str()) => {
                                changed.insert(n.clone());
                            }
                            // `children[d].append(n)` with d the element: one entry per pass.
                            Ex::Index(b, i) if matches!(b.as_ref(), Ex::Name(_)) && lv.is_some_and(|v| matches!(i.as_ref(), Ex::Name(x) if x == v)) => {
                                if let Ex::Name(n) = b.as_ref() {
                                    changed.insert(n.clone());
                                }
                            }
                            _ => *ok = false,
                        }
                        reads.push(a);
                    }
                    _ => *ok = false,
                },
                // `indeg[n] += 1`, `count -= 1`: a constant int.
                St::AugAssign(t, op, v) if matches!(*op, "+" | "-") && matches!(v, Ex::Int(_)) => match t {
                    Ex::Name(n) => {
                        changed.insert(n.clone());
                    }
                    Ex::Index(b, i) if matches!(b.as_ref(), Ex::Name(_)) => {
                        if let Ex::Name(n) = b.as_ref() {
                            changed.insert(n.clone());
                        }
                        reads.push(i);
                    }
                    _ => *ok = false,
                },
                // `found = True`: every pass that sets it sets the same constant.
                St::Assign(ts, v @ (Ex::True | Ex::False | Ex::NoneV | Ex::Int(_))) if matches!(ts.as_slice(), [Ex::Name(_)]) => {
                    let Ex::Name(n) = &ts[0] else { unreachable!() };
                    let c = format!("{v:?}");
                    if sets_to.get(n).is_some_and(|x| *x != c) {
                        *ok = false;
                    }
                    sets_to.insert(n.clone(), c);
                    changed.insert(n.clone());
                }
                St::If(arms, e) => {
                    for (c, b) in arms {
                        reads.push(c);
                        go(b, lv, walked, g, changed, sets_to, reads, ok);
                    }
                    if let Some(b) = e {
                        go(b, lv, walked, g, changed, sets_to, reads, ok);
                    }
                }
                _ => *ok = false,
            }
        }
    }
    go(body, lv, &walked, g, &mut changed, &mut sets_to, &mut reads, &mut ok);
    let _ = els;
    if !ok {
        return false;
    }
    // What the passes read changes nothing, and reads none of what they change (a set being filled may be
    // asked `x in nxt`: adding is the same whatever came first).
    for r in reads {
        if !pure_ex(r, &g.pure_fns) {
            return false;
        }
        // Names read outside the right side of an `in` test.
        let mut all: HashMap<String, usize> = HashMap::new();
        let mut in_rhs: HashMap<String, usize> = HashMap::new();
        walk_ex(r, &mut |x| match x {
            Ex::Name(n) => *all.entry(n.clone()).or_default() += 1,
            Ex::Cmp(_, ops) => {
                for (op, rhs) in ops {
                    if let (true, Ex::Name(n)) = (matches!(*op, "in" | "not in"), rhs) {
                        *in_rhs.entry(n.clone()).or_default() += 1;
                    }
                }
            }
            _ => {}
        });
        let outside = |n: &String| all.get(n).copied().unwrap_or(0) > in_rhs.get(n).copied().unwrap_or(0);
        let mut bad = changed.iter().any(|n| outside(n));
        // `x in s` is the same whatever came first only on a set that the passes add to.
        bad |= in_rhs.keys().any(|n| changed.contains(n) && (!g.sets.contains(n) || sets_to.contains_key(n)));
        if bad {
            return false;
        }
    }
    true
}
