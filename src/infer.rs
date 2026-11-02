// Types for empty values: `xs = []` and `m = {}` get their item types from how they are used later,
// so programs need no `xs: [str] = []`. The result is a Rust type with `_` where nothing is known.
use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
enum T {
    Hole,
    Int,
    F64,
    Bool,
    Str,
    Dec,
    Big,
    Date,
    Json,
    Named(String),
    List(Box<T>),
    Map(Box<T>, Box<T>),
    Tuple(Vec<T>),
    Heap(Box<T>),
    Unit,
}
use T::*;

fn from_ast(t: &Ty) -> T {
    match t {
        Ty::Named(n) => match n.as_str() {
            "int" => Int,
            "f64" | "float" => F64,
            "bool" => Bool,
            "str" => Str,
            "dec" | "quantize" => Dec,
            "big" => Big,
            "date" | "Date" => Date,
            "json" | "Json" => Json,
            n if n.starts_with("AxHeap<") || n == "?" => Hole,
            n => Named(n.into()),
        },
        Ty::List(t) => List(Box::new(from_ast(t))),
        Ty::Map(k, v) => Map(Box::new(from_ast(k)), Box::new(from_ast(v))),
        Ty::Tuple(ts) => Tuple(ts.iter().map(from_ast).collect()),
        Ty::Unit => Unit,
    }
}

fn rust(t: &T) -> String {
    match t {
        Hole => "_".into(),
        Int => "i64".into(),
        F64 => "f64".into(),
        Bool => "bool".into(),
        Str => "String".into(),
        Dec => "Dec".into(),
        Big => "Big".into(),
        Date => "Date".into(),
        Json => "Json".into(),
        Named(n) => n.clone(),
        List(e) => format!("Vec<{}>", rust(e)),
        Map(k, v) => format!("AxM<{}, {}>", rust(k), rust(v)),
        Tuple(ts) if is_row(t) => format!("AxRow<({},)>", ts.iter().map(rust).collect::<Vec<_>>().join(", ")),
        Tuple(ts) => format!("({},)", ts.iter().map(rust).collect::<Vec<_>>().join(", ")),
        Heap(e) => format!("AxHeap<{}>", rust(e)),
        Unit => "()".into(),
    }
}

fn known(t: &T) -> bool {
    match t {
        Hole => false,
        List(e) | Heap(e) => known(e),
        Map(k, v) => known(k) && known(v),
        Tuple(ts) => ts.iter().all(known),
        _ => true,
    }
}

// Anything learned below the outer container?
fn some_known(t: &T) -> bool {
    match t {
        List(e) | Heap(e) => *e.as_ref() != Hole,
        Map(k, v) => **k != Hole || **v != Hole,
        // `best = None`, later `best = v`: a plain type.
        other => *other != Hole && !matches!(other, Unit),
    }
}

fn merge(a: T, b: T) -> T {
    match (a, b) {
        (Hole, b) => b,
        (a, Hole) => a,
        (List(x), List(y)) => List(Box::new(merge(*x, *y))),
        (Heap(x), Heap(y)) => Heap(Box::new(merge(*x, *y))),
        (Map(k1, v1), Map(k2, v2)) => Map(Box::new(merge(*k1, *k2)), Box::new(merge(*v1, *v2))),
        (Tuple(x), Tuple(y)) if x.len() == y.len() => Tuple(x.into_iter().zip(y).map(|(a, b)| merge(a, b)).collect()),
        // An int next to a dec or float is a literal that mixes in.
        (Int, b @ (Dec | F64)) => b,
        (a, _) => a,
    }
}

// Unknown item types as int, for an empty value nothing reads.
fn fill_holes(t: &T) -> T {
    match t {
        Hole => Int,
        List(e) => List(Box::new(fill_holes(e))),
        Heap(e) => Heap(Box::new(fill_holes(e))),
        Map(k, v) => Map(Box::new(fill_holes(k)), Box::new(fill_holes(v))),
        other => other.clone(),
    }
}

fn elem(t: &T) -> T {
    match t {
        List(e) | Heap(e) => *e.clone(),
        Map(k, v) => Tuple(vec![*k.clone(), *v.clone()]),
        Str => Str,
        Json => Json,
        // A tuple of like items (`DAYS = ("Mon", "Tue")`) is read item by item, as Python reads it.
        Tuple(ts) if !ts.is_empty() && ts.iter().all(|t| *t == ts[0]) => ts[0].clone(),
        _ => Hole,
    }
}

// What map, filter, any and all pass to their lambda: a map's keys, as Python's `[k for k in d]` reads them.
// Python passes along parameters a function never reads (`def route(frm, to, stops: list)`), so their type
// cannot be worked out. Such a parameter is dropped, with its argument at each call, unless the function is
// also used as a value.
fn drop_unused_params(items: &mut [Item]) {
    let q = |t: &Ty| matches!(t, Ty::Named(n) if n == "?");
    let mut drops: Vec<(String, Vec<usize>)> = Vec::new();
    for it in items.iter() {
        let Item::Fn { name, params, body, .. } = it else { continue };
        let mut read: Vec<String> = Vec::new();
        walk(body, &mut |e| {
            if let Expr::Ident(n) | Expr::Move(n) = e {
                read.push(n.clone());
            }
        });
        let gone: Vec<usize> = params.iter().enumerate().filter(|(_, p)| q(&p.ty) && !read.contains(&p.name)).map(|(i, _)| i).collect();
        if !gone.is_empty() {
            drops.push((name.clone(), gone));
        }
    }
    if drops.is_empty() {
        return;
    }
    // A function named outside a call (`map(f, xs)`, `ops = [f]`) keeps its shape.
    let mut as_value: Vec<String> = Vec::new();
    for it in items.iter() {
        if let Item::Fn { body, .. } = it {
            walk(body, &mut |e| {
                if let Expr::Ident(n) | Expr::Move(n) = e {
                    as_value.push(n.clone());
                }
            });
        }
    }
    drops.retain(|(n, _)| !as_value.contains(n));
    for it in items.iter_mut() {
        if let Item::Fn { name, params, body, .. } = it {
            if let Some((_, gone)) = drops.iter().find(|(n, _)| n == name) {
                let mut i = 0;
                params.retain(|_| {
                    i += 1;
                    !gone.contains(&(i - 1))
                });
            }
            crate::parser::walk_mut(body, &mut |e| {
                if let Expr::Call(f, args, _) = e {
                    if let Some((_, gone)) = drops.iter().find(|(n, _)| n == f) {
                        if args.iter().all(|(k, _)| k.is_none()) {
                            let mut i = 0;
                            args.retain(|_| {
                                i += 1;
                                !gone.contains(&(i - 1))
                            });
                        }
                    }
                }
            });
        }
    }
}

// `a or b` / `a and b` on values that are not bools: their shared type (one side may still be unknown).
fn or_value(a: &T, b: &T) -> Option<T> {
    match (a, b) {
        (Bool, _) | (_, Bool) | (Hole, Hole) => None,
        (Hole, t) | (t, Hole) => Some(t.clone()),
        (x, y) if x == y => Some(x.clone()),
        _ => None,
    }
}

// What a `for p in xs` loop gives p: a map read by one name gives its keys, as in Python.
fn loop_elem(p: &Pat, t: &T) -> T {
    match (p, t) {
        (Pat::Bind(_), Map(k, _)) => *k.clone(),
        _ => elem(t),
    }
}

fn each_elem(t: &T) -> T {
    match t {
        Map(k, _) => *k.clone(),
        t => elem(t),
    }
}

fn empty_kind(e: &Expr) -> Option<T> {
    match e {
        Expr::List(xs) if xs.is_empty() => Some(List(Box::new(Hole))),
        Expr::Map(ps) if ps.is_empty() => Some(Map(Box::new(Hole), Box::new(Hole))),
        Expr::Call(n, a, _) if n == "heap" && a.is_empty() => Some(Heap(Box::new(Hole))),
        // `best = None`: typed by what is put there later.
        Expr::Call(n, a, _) if n == "none" && a.is_empty() => Some(Hole),
        // `board = [None] * 9`: a list typed by what is put in its slots later.
        Expr::Method(l, m, a, _) if m == "repeat" && a.len() == 1 && matches!(l.as_ref(), Expr::List(xs) if matches!(xs.as_slice(), [Expr::Call(n, a, _)] if n == "none" && a.is_empty())) => Some(List(Box::new(Hole))),
        // `inner = secs.get_or(k, {})`: typed by what is done with it, like a fresh `{}`.
        Expr::Method(_, m, a, _) if matches!(m.as_str(), "get_or" | "get") && a.len() == 2 => empty_kind(&a[1]),
        _ => None,
    }
}

type Env = HashMap<String, T>;

struct Cx<'a> {
    fns: HashMap<String, (Vec<(String, T)>, T)>,
    structs: HashMap<String, Vec<(String, T)>>,
    variants: HashMap<String, String>,
    others: Vec<(&'a [Param], &'a Expr)>,
    ret: T,
    hints: HashMap<(String, usize), String>,
    known_hints: HashMap<(String, usize), bool>,
    hint_t: HashMap<(String, usize), T>,
    // Keys of string-keyed map literals whose values differ in type (these become Json objects).
    mixed: HashSet<String>,
    // The script's top-level names, seen from inside functions.
    genv: Env,
    // Typing a parameter written without a type from how its own function uses it: every use counts.
    param_look: bool,
    // `t.append(c)` with c not known yet: what t holds is not settled (it may be a mixed list).
    unsure: bool,
    // Working out an entry read out of a map (`dd = nxt[s]`): `dd[t] = v` makes it a map.
    entry_look: bool,
}

pub fn map_key(ps: &[(Expr, Expr)]) -> String {
    ps.iter().map(|(k, _)| format!("{k:?}")).collect::<Vec<_>>().join(",")
}

// (name, line) of each untyped empty value -> its Rust type, and whether the type is complete.
pub fn empty_types(items: &[Item]) -> (HashMap<(String, usize), (String, bool)>, HashSet<String>) {
    let mut cx = empty_cx(items);
    let mixed = std::mem::take(&mut cx.mixed);
    (cx.hints.into_iter().map(|(k, v)| {
        let full = cx.known_hints[&k];
        (k, (v, full))
    }).collect(), mixed)
}

// Top-level names with the types functions see them with (`R = {c: i for i, c in enumerate(s)}`).
pub fn global_types(items: &[Item]) -> HashMap<String, Ty> {
    empty_cx(items).genv.iter().filter_map(|(n, t)| Some((n.clone(), to_ast(t)?))).collect()
}

fn empty_cx(items: &[Item]) -> Cx<'_> {
    let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: HashSet::new(), genv: Env::new(), param_look: false, unsure: false, entry_look: false };
    for it in items {
        match it {
            Item::Fn { name, params, ret, body, script, .. } => {
                cx.fns.insert(name.clone(), (params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect(), from_ast(ret)));
                if !script {
                    cx.others.push((params, body));
                }
            }
            Item::Type { name, fields, .. } => {
                cx.structs.insert(name.clone(), fields.iter().map(|f| (f.name.clone(), from_ast(&f.ty))).collect());
            }
            Item::Enum { name, variants, .. } => {
                for v in variants {
                    cx.variants.insert(v.name.clone(), name.clone());
                }
            }
            _ => {}
        }
    }
    // The script runs first, so its top-level names are known inside functions.
    let mut order: Vec<&Item> = items.iter().filter(|i| matches!(i, Item::Fn { script: true, .. })).collect();
    order.extend(items.iter().filter(|i| !matches!(i, Item::Fn { script: true, .. })));
    // Twice: the second pass knows the top-level types the first found (a name filled from another's items).
    for it in order.iter().chain(order.iter()) {
        let (params, ret, body, script) = match it {
            Item::Fn { params, ret, body, script, .. } => (params.as_slice(), from_ast(ret), body, *script),
            Item::Test { body, .. } => (&[][..], Unit, body, false),
            _ => continue,
        };
        cx.ret = ret;
        let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
        cx.walk(body, &mut env, script);
        if script {
            cx.genv = env;
        }
    }
    cx
}

impl Cx<'_> {
    // ---- forward walk: find empty declarations, and keep local types up to date ----
    fn walk(&mut self, e: &Expr, env: &mut Env, top: bool) {
        if !matches!(e, Expr::Block(..) | Expr::For(..) | Expr::While(..) | Expr::If(..) | Expr::Match(..)) {
            self.scan_maps(e, env);
        }
        match e {
            Expr::Block(stmts, tail) => {
                let mut inner = env.clone();
                self.stmts(stmts, tail.as_deref(), &mut inner, top);
                if let Some(t) = tail {
                    self.walk(t, &mut inner, false);
                }
                if top {
                    *env = inner;
                }
            }
            Expr::For(p, it, body) => {
                let t = loop_elem(p, &self.ty(it, env));
                let mut inner = env.clone();
                bind(p, t, &mut inner);
                self.walk(body, &mut inner, false);
            }
            Expr::While(c, body) => {
                self.walk(c, env, false);
                self.walk(body, &mut env.clone(), false);
            }
            Expr::If(c, a, b) => {
                self.walk(c, env, false);
                self.walk(a, &mut env.clone(), false);
                if let Some(b) = b {
                    self.walk(b, &mut env.clone(), false);
                }
            }
            Expr::Match(s, arms) => {
                let st = self.ty(s, env);
                for (p, a) in arms {
                    let mut inner = env.clone();
                    bind_arm(p, &st, &mut inner);
                    self.walk(a, &mut inner, false);
                }
                self.walk(s, env, false);
            }
            _ => {}
        }
    }

    fn stmts(&mut self, stmts: &[Stmt], tail: Option<&Expr>, env: &mut Env, top: bool) {
        // Top-level names with a plain value (`nextid = 1`) are known inside functions too.
        let mut globals = env.clone();
        if top {
            for st in stmts {
                if let Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(n), value, .. } = st {
                    let t = self.ty(value, &globals);
                    if known(&t) && !globals.contains_key(n) {
                        globals.insert(n.clone(), t);
                    }
                }
            }
        }
        for (i, st) in stmts.iter().enumerate() {
            let decl = match st {
                Stmt::Let { name: Pat::Bind(n), ty: None, value, line, .. } => Some((n, value, *line)),
                Stmt::Assign { target: Expr::Ident(n), op: "=", value, line } if !env.contains_key(n) => Some((n, value, *line)),
                _ => None,
            };
            if let Some((n, value, line)) = decl {
                if let Some(base) = empty_kind(value) {
                    let mut t = base.clone();
                    {
                        // The block's last expression (often a loop or an if) is part of what follows.
                        let mut later = env.clone();
                        self.look(n, &stmts[i + 1..], tail, &mut later, &mut t);
                        if let Some(tl) = tail {
                            self.look_expr(n, tl, &mut later, &mut t);
                        }
                    }
                    if top {
                        // A top-level name may be filled inside functions.
                        for (ps, body) in self.others.clone() {
                            if ps.iter().any(|p| &p.name == n) {
                                continue;
                            }
                            let mut fenv = globals.clone();
                            // A top-level name the function sets before reading is its own local there.
                            let text = format!("{body:?}");
                            fenv.retain(|g, _| g == n || !crate::emit::sets_first(&text, g));
                            fenv.extend(ps.iter().map(|p| (p.name.clone(), from_ast(&p.ty))));
                            fenv.insert(n.clone(), t.clone());
                            self.look_expr(n, body, &mut fenv, &mut t);
                        }
                    }
                    if some_known(&t) {
                        self.hints.insert((n.clone(), line), rust(&t));
                        self.known_hints.insert((n.clone(), line), known(&t));
                        self.hint_t.insert((n.clone(), line), t.clone());
                    } else {
                        // An empty value that is never used again can have any item type.
                        let id = format!("Ident({n:?})");
                        let mv = format!("Move({n:?})");
                        let mut rest = format!("{:?} {:?}", &stmts[i + 1..], tail);
                        if top {
                            for (_, body) in &self.others {
                                rest.push_str(&format!("{body:?}"));
                            }
                        }
                        if !rest.contains(&id) && !rest.contains(&mv) {
                            let filled = fill_holes(&t);
                            self.hints.insert((n.clone(), line), rust(&filled));
                            self.known_hints.insert((n.clone(), line), true);
                        }
                    }
                    // Functions read it with this type from here on (not an older guess).
                    if top && known(&t) {
                        globals.insert(n.clone(), t.clone());
                    }
                    env.insert(n.clone(), t);
                    continue;
                }
            }
            match st {
                Stmt::Let { name, ty, value, .. } => {
                    self.walk(value, env, false);
                    let t = ty.as_ref().map(from_ast).unwrap_or_else(|| self.ty(value, env));
                    bind(name, t, env);
                }
                Stmt::Assign { target, op, value, .. } => {
                    self.walk(value, env, false);
                    match (target, *op) {
                        (Expr::Ident(n), "=") if !env.contains_key(n) => {
                            let t = self.ty(value, env);
                            env.insert(n.clone(), t);
                        }
                        (Expr::Tuple(xs), "=") => {
                            let t = self.ty(value, env);
                            for (k, x) in xs.iter().enumerate() {
                                if let Expr::Ident(n) = x {
                                    if !env.contains_key(n) {
                                        // A list unpacks as well (`for a, b in rows`, `a, b = s.split()`).
                let et = match &t {
                    Tuple(ts) => ts.get(k).cloned().unwrap_or(Hole),
                    List(e) => *e.clone(),
                    Json => Json,
                    _ => Hole,
                };
                                        env.insert(n.clone(), et);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Stmt::Expr { expr, .. } => self.walk(expr, env, false),
                Stmt::Return { value: Some(v), .. } => self.walk(v, env, false),
                _ => {}
            }
        }
    }

    // Finds map literals with string keys whose values have different types.
    fn scan_maps(&mut self, e: &Expr, env: &Env) {
        match e {
            Expr::Map(ps) => {
                for (_, v) in ps {
                    self.scan_maps(v, env);
                }
                if ps.len() > 1 && ps.iter().all(|(k, _)| matches!(k, Expr::Str(_))) {
                    let ts: Vec<String> = ps.iter().map(|(_, v)| self.ty(v, env)).filter(known).map(|t| rust(&t)).collect();
                    if ts.iter().any(|t| t != &ts[0]) {
                        self.mixed.insert(map_key(ps));
                    }
                }
            }
            Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| self.scan_maps(x, env)),
            Expr::Call(_, args, _) => args.iter().for_each(|(_, a)| self.scan_maps(a, env)),
            Expr::Method(r, m, args, _) => {
                self.scan_maps(r, env);
                let each = ["map", "filter", "any", "all", "count", "sort_by", "group_runs"].contains(&m.as_str());
                let et = if each { each_elem(&self.ty(r, env)) } else { Hole };
                for a in args {
                    match a {
                        Expr::Lambda(ps, b) if ps.len() == 1 => {
                            let mut inner = env.clone();
                            inner.insert(ps[0].clone(), et.clone());
                            self.scan_maps(b, &inner);
                        }
                        a => self.scan_maps(a, env),
                    }
                }
            }
            Expr::Binary(_, a, b) => {
                self.scan_maps(a, env);
                self.scan_maps(b, env);
            }
            Expr::Unary(_, a) | Expr::Try(a) => self.scan_maps(a, env),
            Expr::Lambda(_, b) => self.scan_maps(b, env),
            Expr::If(..) | Expr::Match(..) | Expr::Block(..) => self.walk(e, &mut env.clone(), false),
            _ => {}
        }
    }

    // ---- look ahead: what do later statements put into `n`? ----
    fn look(&mut self, n: &str, stmts: &[Stmt], tail: Option<&Expr>, env: &mut Env, acc: &mut T) {
        for (i, st) in stmts.iter().enumerate() {
            match st {
                Stmt::Let { name, ty, value, .. } => {
                    self.look_expr(n, value, env, acc);
                    if matches!(name, Pat::Bind(m) if m == n) {
                        return;
                    }
                    let t = ty.as_ref().map(from_ast).unwrap_or_else(|| self.ty(value, env));
                    bind(name, t, env);
                }
                Stmt::Assign { target, op, value, .. } => {
                    self.look_expr(n, value, env, acc);
                    let vt = self.ty(value, env);
                    match (target, *op) {
                        (Expr::Ident(m), "=" | "+=") if m == n => *acc = merge(acc.clone(), vt),
                        // Python's `rec[3] -= n` on a list of unknown items may be a slot of a mixed record: no evidence.
                        (Expr::Index(b, k), _) if matches!(b.as_ref(), Expr::Ident(m) if m == n) && py_on() && matches!(k.as_ref(), Expr::Int(_)) && matches!(acc, List(e) if **e == Hole) => {}
                        // `st = stats[sku]` then `st[0] += q`: a list entry, not a dict keyed by ints.
                        (Expr::Index(b, k), _) if matches!(b.as_ref(), Expr::Ident(m) if m == n) && self.entry_look && matches!(k.as_ref(), Expr::Int(_)) && *acc == Hole => {}
                        (Expr::Index(b, k), _) if matches!(b.as_ref(), Expr::Ident(m) if m == n) => {
                            let kt = self.ty(k, env);
                            // `key in memo` (read as a list of keys) and then `memo[key] = v` with a key that is
                            // the same type: memo is a dict (a list indexed by its own items would be odd).
                            if py_on() && matches!(&*acc, List(e) if **e == kt) && known(&kt) {
                                *acc = Map(Box::new(kt), Box::new(vt));
                                continue;
                            }
                            *acc = merge(acc.clone(), self.slot(acc, kt, vt));
                        }
                        (Expr::Index(b, k), _) => {
                            // `m[a][b] = v`
                            if let Expr::Index(bb, k1) = b.as_ref() {
                                if matches!(bb.as_ref(), Expr::Ident(m) if m == n) {
                                    let (k1t, k2t) = (self.ty(k1, env), self.ty(k, env));
                                    let inner = match acc {
                                        Map(_, v) | List(v) => *v.clone(),
                                        _ => Hole,
                                    };
                                    // `m[a][b] = v` into a missing entry makes it a map (an empty list has no slot b).
                                    let inner = match inner {
                                        // `m[-1][1] = v` on a list: the item may as well be a list, so later lines decide.
                                        Hole if matches!(acc, List(_)) && matches!(k2t, Int) => Hole,
                                        Hole => Map(Box::new(k2t), Box::new(vt)),
                                        inner => merge(inner.clone(), self.slot(&inner, k2t, vt)),
                                    };
                                    *acc = merge(acc.clone(), self.slot(acc, k1t, inner));
                                }
                            }
                            self.look_expr(n, b, env, acc);
                            self.look_expr(n, k, env, acc);
                        }
                        // Another empty value declared on the way: work out its type from the lines after it first.
                        (Expr::Ident(m), "=") if !env.contains_key(m) && empty_kind(value).is_some() && m != n => {
                            let mut mt = empty_kind(value).unwrap();
                            let mut later = env.clone();
                            later.insert(m.clone(), mt.clone());
                            self.look(m, &stmts[i + 1..], tail, &mut later, &mut mt);
                            if let Some(tl) = tail {
                                self.look_expr(m, tl, &mut later, &mut mt);
                            }
                            env.insert(m.clone(), mt);
                        }
                        // `lq = m[k]` then `lq.push(x)`: what the copy is filled with is the entry's type.
                        (Expr::Ident(m), "=") if !env.contains_key(m) && m != n && matches!(value, Expr::Index(b, _) if matches!(b.as_ref(), Expr::Ident(x) if x == n)) => {
                            let Expr::Index(_, k) = value else { unreachable!() };
                            let kt = self.ty(k, env);
                            let mut mt = vt.clone();
                            let mut later = env.clone();
                            later.insert(m.clone(), mt.clone());
                            let was = self.entry_look;
                            self.entry_look = mt == Hole && matches!(acc, Map(..) | Hole);
                            self.look(m, &stmts[i + 1..], tail, &mut later, &mut mt);
                            if let Some(tl) = tail {
                                self.look_expr(m, tl, &mut later, &mut mt);
                            }
                            self.entry_look = was;
                            if some_known(&mt) {
                                *acc = merge(acc.clone(), self.slot(acc, kt, mt.clone()));
                            }
                            env.insert(m.clone(), mt);
                        }
                        (Expr::Ident(m), "=") if !env.contains_key(m) => {
                            env.insert(m.clone(), vt);
                        }
                        // A name set again to a value of another known type (a function's own `n` over a global `n`).
                        (Expr::Ident(m), "=") if m != n && known(&vt) && env.get(m).map_or(false, |t| known(t) && *t != vt) => {
                            env.insert(m.clone(), vt);
                        }
                        (Expr::Tuple(xs), "=") => {
                            for (k, x) in xs.iter().enumerate() {
                                if let Expr::Ident(m) = x {
                                    if !env.contains_key(m) {
                                        // `qid, ans = tok.split("=", 1)`: a list gives its items.
                                        let et = match &vt {
                                            Tuple(ts) => ts.get(k).cloned().unwrap_or(Hole),
                                            List(e) => (**e).clone(),
                                            Json => Json,
                                            _ => Hole,
                                        };
                                        env.insert(m.clone(), et);
                                    }
                                }
                            }
                        }
                        (t, _) => self.look_expr(n, t, env, acc),
                    }
                }
                Stmt::Expr { expr, .. } => {
                    self.look_expr(n, expr, env, acc);
                    // A name set in the branches of an if or match is known after it.
                    let mut sets = Vec::new();
                    self.branch_sets(expr, env, &mut sets);
                    for (m, t) in sets {
                        if m != n && !env.contains_key(&m) && known(&t) {
                            env.insert(m, t);
                        }
                    }
                }
                Stmt::Return { value: Some(v), .. } => {
                    if matches!(v, Expr::Ident(m) if m == n) {
                        *acc = merge(acc.clone(), self.ret.clone());
                    }
                    self.look_expr(n, v, env, acc);
                }
                Stmt::Assert { cond, .. } => self.look_expr(n, cond, env, acc),
                _ => {}
            }
            env.insert(n.to_string(), acc.clone());
        }
    }

    fn branch_sets(&self, e: &Expr, env: &Env, out: &mut Vec<(String, T)>) {
        let block = |b: &Expr, out: &mut Vec<(String, T)>| {
            if let Expr::Block(st, _) = b {
                for s in st {
                    if let Stmt::Assign { target: Expr::Ident(m), op: "=", value, .. } = s {
                        out.push((m.clone(), self.ty(value, env)));
                    }
                }
            }
        };
        match e {
            Expr::If(_, a, b) => {
                block(a, out);
                match b.as_deref() {
                    Some(i @ Expr::If(..)) => self.branch_sets(i, env, out),
                    Some(b) => block(b, out),
                    None => {}
                }
            }
            Expr::Match(_, arms) => arms.iter().for_each(|(_, a)| block(a, out)),
            _ => {}
        }
    }

    // The container type that holds `v` at key `k`, given the container's kind.
    fn slot(&self, acc: &T, k: T, v: T) -> T {
        match acc {
            Map(..) => Map(Box::new(k), Box::new(v)),
            List(_) => List(Box::new(v)),
            // A parameter of unknown kind indexed by a non-int key (`seen[oid] = true`) is a map.
            Hole if self.param_look && !matches!(k, Int | Hole) => Map(Box::new(k), Box::new(v)),
            Hole if self.entry_look && k != Hole => Map(Box::new(k), Box::new(v)),
            _ => Hole,
        }
    }

    fn is_n(e: &Expr, n: &str) -> bool {
        matches!(e, Expr::Ident(m) | Expr::Move(m) if m == n)
    }

    fn look_expr(&mut self, n: &str, e: &Expr, env: &mut Env, acc: &mut T) {
        match e {
            Expr::Method(recv, m, args, _) => {
                let a0 = args.first().map(|a| self.ty(a, env)).unwrap_or(Hole);
                if Self::is_n(recv, n) {
                    let ev = match (m.as_str(), &*acc) {
                        // Methods only text has.
                        ("split" | "words" | "lines" | "lower" | "upper" | "trim" | "starts_with" | "ends_with" | "replace" | "is_digit" | "is_alpha" | "ints" | "partition" | "find" | "chars", Hole) if self.param_look => Str,
                        ("has", Hole) if self.param_look => Map(Box::new(a0), Box::new(Hole)),
                        // Python lists that hold ints and strings (`t.append(int(s[i:j]))`, `t.append(c)`): JSON items.
                        ("push", List(x)) if scalar(x) && scalar(&a0) && clash(x, &a0) => {
                            *acc = List(Box::new(Json));
                            Hole
                        }
                        ("push", List(x)) if a0 == Hole && scalar(x) && matches!(args.first(), Some(Expr::Ident(_))) => {
                            self.unsure = true;
                            Hole
                        }
                        ("push" | "heap_push", List(_) | Hole) => List(Box::new(a0)),
                        // A set() list filled from another list: `dealt.update(cards)`.
                        ("update" | "extend", List(_)) if matches!(a0, List(_)) => a0.clone(),
                        ("push", Heap(_)) => Heap(Box::new(a0)),
                        ("insert", List(_)) => List(Box::new(args.get(1).map(|a| self.ty(a, env)).unwrap_or(Hole))),
                        ("has" | "remove", Map(..)) => Map(Box::new(a0), Box::new(Hole)),
                        ("get_or", Map(..)) => Map(Box::new(a0), Box::new(args.get(1).map(|a| self.ty(a, env)).unwrap_or(Hole))),
                        ("contains" | "index_of", List(_)) => List(Box::new(a0)),
                        _ => Hole,
                    };
                    *acc = merge(acc.clone(), ev);
                } else if let Expr::Index(b, k) = recv.as_ref() {
                    if Self::is_n(b, n) {
                        let kt = self.ty(k, env);
                        let inner = match m.as_str() {
                            "push" => List(Box::new(a0)),
                            _ => Hole,
                        };
                        *acc = merge(acc.clone(), self.slot(acc, kt, inner));
                    }
                }
                self.look_expr(n, recv, env, acc);
                let et = each_elem(&self.ty(recv, env));
                for a in args {
                    self.look_lambda(n, a, &et, env, acc);
                }
            }
            // `st[t]` with st a {str: ...} map makes t a str; `xs[i]` makes i an int.
            Expr::Index(b, k) if self.param_look && Self::is_n(k, n) => {
                let ev = match self.ty(b, env) {
                    Map(kt, _) => *kt,
                    List(_) | Str => Int,
                    _ => Hole,
                };
                *acc = merge(acc.clone(), ev);
                self.look_expr(n, b, env, acc);
            }
            // `t == "x"`, `n + 1`, `w in words`: the other side's type.
            Expr::Binary(op, a, b) if self.param_look && (Self::is_n(a, n) || Self::is_n(b, n)) && !matches!(*op, "and" | "or" | "&&" | "||") => {
                let other = if Self::is_n(a, n) { b } else { a };
                let ot = self.ty(other, env);
                let ev = match *op {
                    "in" if Self::is_n(a, n) => elem(&ot),
                    // `c in mem` with c a str: mem may be a list or a string, so the calls decide.
                    "in" if matches!(ot, Int | F64 | Bool | Tuple(_) | Named(_)) => List(Box::new(ot)),
                    "in" => Hole,
                    "*" | "/" | "//" | "%" if matches!(ot, Str) => Hole,
                    _ if matches!(ot, Int | F64 | Str | Bool | Dec | Date | Big | Named(_) | Tuple(_)) => ot,
                    _ => Hole,
                };
                *acc = merge(acc.clone(), ev);
                self.look_expr(n, other, env, acc);
            }
            // `p.qty` on a parameter: the one user type with that field.
            Expr::Field(a, f) if self.param_look && Self::is_n(a, n) => {
                let owners: Vec<&String> = self.structs.iter().filter(|(_, fs)| fs.iter().any(|(fname, _)| fname == f)).map(|(s, _)| s).collect();
                if owners.len() == 1 {
                    *acc = merge(acc.clone(), Named(owners[0].clone()));
                }
            }
            Expr::Index(b, k) => {
                if Self::is_n(b, n) {
                    if let Map(..) = acc {
                        let kt = self.ty(k, env);
                        *acc = merge(acc.clone(), Map(Box::new(kt), Box::new(Hole)));
                    }
                }
                self.look_expr(n, b, env, acc);
                self.look_expr(n, k, env, acc);
            }
            Expr::Call(f, args, _) => {
                if let Some((ps, _)) = self.fns.get(f).cloned() {
                    for (k, (_, a)) in args.iter().enumerate() {
                        if Self::is_n(a, n) {
                            if let Some((_, pt)) = ps.get(k) {
                                *acc = merge(acc.clone(), pt.clone());
                            }
                        }
                    }
                }
                for (_, a) in args {
                    self.look_expr(n, a, env, acc);
                }
            }
            Expr::Block(stmts, tail) => {
                let mut inner = env.clone();
                self.look(n, stmts, tail.as_deref(), &mut inner, acc);
                if let Some(t) = tail {
                    self.look_expr(n, t, &mut inner, acc);
                }
            }
            Expr::For(p, it, body) => {
                self.look_expr(n, it, env, acc);
                let t = loop_elem(p, &self.ty(it, env));
                let mut inner = env.clone();
                bind(p, t, &mut inner);
                self.look_expr(n, body, &mut inner, acc);
            }
            Expr::While(c, body) => {
                self.look_expr(n, c, env, acc);
                self.look_expr(n, body, &mut env.clone(), acc);
            }
            Expr::If(c, a, b) => {
                self.look_expr(n, c, env, acc);
                self.look_expr(n, a, &mut env.clone(), acc);
                if let Some(b) = b {
                    self.look_expr(n, b, &mut env.clone(), acc);
                }
            }
            Expr::Match(s, arms) => {
                self.look_expr(n, s, env, acc);
                let st = self.ty(s, env);
                for (p, a) in arms {
                    let mut inner = env.clone();
                    bind_arm(p, &st, &mut inner);
                    self.look_expr(n, a, &mut inner, acc);
                }
            }
            Expr::Binary(_, a, b) | Expr::Range(a, b) => {
                self.look_expr(n, a, env, acc);
                self.look_expr(n, b, env, acc);
            }
            Expr::Unary(_, a) | Expr::Try(a) | Expr::Field(a, _) | Expr::TupleField(a, _) => self.look_expr(n, a, env, acc),
            Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| self.look_expr(n, x, env, acc)),
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.look_expr(n, x, env, acc);
                    }
                }
            }
            Expr::Lambda(..) => self.look_lambda(n, e, &Hole, env, acc),
            _ => {}
        }
    }

    fn look_lambda(&mut self, n: &str, e: &Expr, et: &T, env: &mut Env, acc: &mut T) {
        if let Expr::Lambda(ps, body) = e {
            let mut inner = env.clone();
            bind_params(ps, et, &mut inner);
            self.look_expr(n, body, &mut inner, acc);
        } else {
            self.look_expr(n, e, env, acc);
        }
    }

    // ---- the type of an expression, Hole when unknown ----
    fn ty(&self, e: &Expr, env: &Env) -> T {
        match e {
            Expr::Int(_) => Int,
            Expr::Float(_) => F64,
            Expr::Bool(_) => Bool,
            Expr::Str(_) => Str,
            Expr::Ident(n) if n == "Null" && !env.contains_key(n) => Json,
            Expr::Call(f, a, _) if f == "ax_row" && a.len() == 1 => self.ty(&a[0].1, env),
            Expr::Ident(n) | Expr::Move(n) => env.get(n).or_else(|| self.genv.get(n)).cloned().unwrap_or(Hole),
            // Python's `[expiry, seq, sku, qty]`: a record of mixed types is a tuple (its slots stay settable).
            Expr::List(xs) if mixed_record(xs, &|x| self.ty(x, env)) => Tuple(xs.iter().map(|x| self.ty(x, env)).collect()),
            // ... and one whose items are not all known yet may still be such a record.
            Expr::List(xs) if py_on() && (2..=12).contains(&xs.len()) && xs.iter().any(|x| self.ty(x, env) == Hole) && xs.iter().any(|x| matches!(self.ty(x, env), Int | F64 | Str | Bool | Dec | Date)) => List(Box::new(Hole)),
            Expr::List(xs) => List(Box::new(xs.iter().fold(Hole, |a, x| merge(a, self.ty(x, env))))),
            Expr::Map(ps) if self.mixed.contains(&map_key(ps)) => Json,
            Expr::Map(ps) => {
                let (k, v) = ps.iter().fold((Hole, Hole), |(k, v), (a, b)| (merge(k, self.ty(a, env)), merge(v, self.ty(b, env))));
                Map(Box::new(k), Box::new(v))
            }
            Expr::Tuple(xs) => Tuple(xs.iter().map(|x| self.ty(x, env)).collect()),
            Expr::Unary(op, a) => {
                if *op == "!" || *op == "not" {
                    Bool
                } else {
                    self.ty(a, env)
                }
            }
            Expr::Binary(op, a, b) => match *op {
                // `s or ""` on values that are not bools is a value of their type (see Checker::expr).
                "and" | "or" if or_value(&self.ty(a, env), &self.ty(b, env)).is_some() => or_value(&self.ty(a, env), &self.ty(b, env)).unwrap(),
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "and" | "or" | "~=" | "&&" | "||" | "in" => Bool,
                _ => {
                    let (x, y) = (self.ty(a, env), self.ty(b, env));
                    match (&x, &y) {
                        (Str, _) | (_, Str) => Str,
                        (Dec, _) | (_, Dec) => Dec,
                        (F64, _) | (_, F64) => F64,
                        _ => merge(x, y),
                    }
                }
            },
            Expr::Call(n, args, _) => {
                let a0 = || args.first().map(|(_, a)| self.ty(a, env)).unwrap_or(Hole);
                match n.as_str() {
                    "lines" | "args" => List(Box::new(Str)),
                    "stdin" | "input" | "read" | "str" | "__err" => Str,
                    "__truediv" if args.iter().any(|(_, a)| self.ty(a, env) == Dec) => Dec,
                    "__truediv" => F64,
                    "csv" => List(Box::new(List(Box::new(Str)))),
                    "json" => Json,
                    "date" => Date,
                    "abs" => a0(),
                    "min" | "max" => args.iter().fold(Hole, |t, (_, a)| merge(t, self.ty(a, env))),
                    "heap" => Heap(Box::new(Hole)),
                    "ax_to_json" => Json,
                    "ax_json_dec" => Dec,
                    "ax_subset" => Bool,
                    "ax_json_strs" => List(Box::new(Str)),
                    "ax_json_ints" => List(Box::new(Int)),
                    "Str" | "Num" | "Bool" | "Arr" | "Obj" if !self.variants.contains_key(n.as_str()) => Json,
                    n if self.structs.contains_key(n) => Named(n.into()),
                    n if self.variants.contains_key(n) => {
                        let en = &self.variants[n];
                        if en == "Json" { Json } else { Named(en.clone()) }
                    }
                    n => self.fns.get(n).map(|f| f.1.clone()).unwrap_or(Hole),
                }
            }
            Expr::Method(recv, m, args, _) => self.method(recv, m, args, env),
            Expr::Field(b, f) => match self.ty(b, env) {
                Named(s) => self.structs.get(&s).and_then(|fs| fs.iter().find(|(n, _)| n == f).map(|(_, t)| t.clone())).unwrap_or(Hole),
                Date if matches!(f.as_str(), "days" | "day" | "month" | "year" | "weekday") => Int,
                _ => Hole,
            },
            Expr::TupleField(b, i) => match self.ty(b, env) {
                Tuple(ts) => ts.get(*i).cloned().unwrap_or(Hole),
                _ => Hole,
            },
            Expr::Index(b, i) => match self.ty(b, env) {
                // `tx['posts']` on a dict made a record: the field.
                Named(r) if crate::records::is_record(&r) => match rec_key(i) {
                    Some(k) => self.structs.get(&r).and_then(|fs| fs.iter().find(|(n, _)| *n == k).map(|(_, t)| t.clone())).unwrap_or(Hole),
                    None => Hole,
                },
                List(e) => *e,
                Map(_, v) => *v,
                Str => Str,
                Json => Json,
                // `r[1]` on a tuple is its field (the checker turns it into r.1).
                Tuple(ts) => match i.as_ref() {
                    Expr::Int(k) if *k >= 0 => ts.get(*k as usize).cloned().unwrap_or(Hole),
                    _ => Hole,
                },
                _ => Hole,
            },
            Expr::Try(a) => self.ty(a, env),
            Expr::If(_, a, b) => {
                let t = self.ty(a, env);
                match b {
                    Some(b) => merge(t, self.ty(b, env)),
                    None => t,
                }
            }
            Expr::Range(..) => List(Box::new(Int)),
            Expr::Match(_, arms) => arms.iter().fold(Hole, |t, (p, b)| {
                let mut inner = env.clone();
                bind(p, Hole, &mut inner);
                merge(t, self.ty(b, &inner))
            }),
            Expr::Block(stmts, Some(tail)) => {
                let mut inner = env.clone();
                for st in stmts {
                    match st {
                        Stmt::Let { name, ty, value, .. } => {
                            let t = ty.as_ref().map(from_ast).unwrap_or_else(|| self.ty(value, &inner));
                            bind(name, t, &mut inner);
                        }
                        Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if !inner.contains_key(n) => {
                            let t = self.ty(value, &inner);
                            inner.insert(n.clone(), t);
                        }
                        // `(r, d) = ax_cu` from a comprehension's `for k, (r, d) in ...`.
                        Stmt::Assign { target: Expr::Tuple(xs), op: "=", value, .. } if xs.iter().all(|x| matches!(x, Expr::Ident(_))) => {
                            let p = Pat::Tuple(xs.iter().map(|x| if let Expr::Ident(n) = x { Pat::Bind(n.clone()) } else { Pat::Wild }).collect());
                            let t = self.ty(value, &inner);
                            bind(&p, t, &mut inner);
                        }
                        _ => {}
                    }
                }
                self.ty(tail, &inner)
            }
            _ => Hole,
        }
    }

    fn method(&self, recv: &Expr, m: &str, args: &[Expr], env: &Env) -> T {
        let r = self.ty(recv, env);
        let a = |i: usize| args.get(i).map(|x| self.ty(x, env)).unwrap_or(Hole);
        let lam = |i: usize, et: &T| match args.get(i) {
            Some(Expr::Lambda(ps, body)) => {
                let mut inner = env.clone();
                bind_params(ps, et, &mut inner);
                self.ty(body, &inner)
            }
            // `xs.map(decode)`: a function passed by name gives its return type.
            Some(Expr::Ident(f)) if !env.contains_key(f) => self.fns.get(f).map(|(_, r)| r.clone()).unwrap_or(Hole),
            _ => Hole,
        };
        // A class's own method wins over the built-in of that name (`stock.take(sku, n)`).
        if let Named(c) = &r {
            if let Some((_, ret)) = self.fns.get(&format!("{c}_{m}")) {
                return ret.clone();
            }
        }
        match m {
            "len" | "count" | "count_sub" | "find" | "find_from" | "index_of" | "digit" | "code" | "int" | "days" | "day" | "month" | "year" | "weekday" => Int,
            "floor" | "ceil" if r == Dec => Int,
            "float" | "sqrt" | "floor" | "ceil" => F64,
            "dec" | "quantize" => Dec,
            "big" => Big,
            "char" | "fmt" | "spec" | "py_repr" | "trim" | "trim_of" | "trim_end" | "trim_end_of" | "trim_start" | "trim_start_of" | "lower" | "upper" | "replace" | "re_sub" | "text" | "zfill" | "title" | "center" | "center_with" | "capitalize" | "ljust" | "rjust" | "ljust_with" | "rjust_with" => Str,
            "list" => List(Box::new(Json)),
            "repeat" | "slice" | "rev" if matches!(r, Str) => Str,
            "join" => Str,
            "words" | "lines" | "chars" | "split" | "split_n" | "rsplit_n" | "re_full" | "re_find" | "re_split" | "re_list" | "re_match" | "re_findall" => List(Box::new(Str)),
            "partition" => Tuple(vec![Str, Str, Str]),
            "re_all" | "csv_rows" => List(Box::new(List(Box::new(Str)))),
            "ints" => List(Box::new(Int)),
            "json" => Json,
            "is_null" | "is_none" | "bool" | "is_empty" | "is_digit" | "is_alpha" | "is_space" | "is_upper" | "is_lower" | "is_alnum" | "is_ascii" | "is_leap" | "isinstance" | "has" | "contains" | "any" | "all" | "starts_with" | "ends_with" => Bool,
            "pop_item" => match &r {
                Map(k, v) => Tuple(vec![*k.clone(), *v.clone()]),
                _ => Hole,
            },
            "pop_or" => match &r {
                Map(_, v) => *v.clone(),
                _ => Hole,
            },
            "first" | "last" | "pop" | "min" | "max" | "sum" | "heap_pop" | "heap_push_pop" | "heap_replace" | "pop_at" | "next_or" | "next_or_empty" => match &r {
                Map(_, v) if m == "pop" || m == "pop_at" => *v.clone(),
                _ => elem(&r),
            },
            "remove" => match &r {
                Map(_, v) => *v.clone(),
                _ => elem(&r),
            },
            "slice" | "sort" | "sort_by" | "rev" | "take" | "skip" | "filter" | "unique" | "step" | "repeat" | "abs" | "round" => match &r {
                // list(s) and set(s) on a string: its characters.
                Str if m == "skip" || m == "unique" => List(Box::new(Str)),
                Hole if m == "repeat" || m == "slice" => Hole,
                Map(k, _) if m == "filter" || m == "sort" || m == "sort_by" || m == "sort_by_rev" || m == "skip" => List(k.clone()),
                _ => r,
            },
            // A tuple of one type (`(0, 1, 2)`) walked as a list.
            "tuple_items" => match &r {
                Tuple(ts) if !ts.is_empty() => List(Box::new(ts[0].clone())),
                _ => Hole,
            },
            "flatten" => match elem(&r) {
                List(t) => List(t),
                Str => List(Box::new(Str)),
                _ => Hole,
            },
            "enumerate" | "enumerate_from" => List(Box::new(Tuple(vec![Int, elem(&r)]))),
            "zip" => List(Box::new(Tuple(vec![elem(&r), elem(&a(0))]))),
            "keys" => match &r {
                Map(k, _) => List(k.clone()),
                Json => List(Box::new(Str)),
                _ => Hole,
            },
            "values" => match &r {
                Map(_, v) => List(v.clone()),
                Json => List(Box::new(Json)),
                _ => Hole,
            },
            "pairs" => match &r {
                Map(k, v) => List(Box::new(Tuple(vec![*k.clone(), *v.clone()]))),
                _ => Hole,
            },
            "get_or" => match &r {
                Map(_, v) => merge(*v.clone(), a(1)),
                _ => a(1),
            },
            "or" => merge(r, a(0)),
            "map" => List(Box::new(lam(0, &each_elem(&r)))),
            "group_runs" => List(Box::new(Tuple(vec![lam(0, &each_elem(&r)), List(Box::new(each_elem(&r)))]))),
            "fold" => merge(a(0), Hole),
            "add_days" | "delta_days" => Date,
            "month_range" => Tuple(vec![Int, Int]),
            "tally" => Map(Box::new(elem(&r)), Box::new(Int)),
            "tally_words" => Map(Box::new(Str), Box::new(Int)),
            "to_dict" => match elem(&r) {
                Tuple(kv) if kv.len() == 2 => Map(Box::new(kv[0].clone()), Box::new(kv[1].clone())),
                _ => match &r {
                    Map(..) => r,
                    _ => Hole,
                },
            },
            "most_common" | "most_common_all" => match &r {
                Map(k, _) => List(Box::new(Tuple(vec![*k.clone(), Int]))),
                _ => Hole,
            },
            _ => Hole,
        }
    }
}

fn bind(p: &Pat, t: T, env: &mut Env) {
    match p {
        Pat::Bind(n) => {
            env.insert(n.clone(), t);
        }
        Pat::Tuple(ps) => {
            for (k, p) in ps.iter().enumerate() {
                // A list unpacked into names (`for a, b, w in rows` with rows a list of lists) gives its items.
                let et = match &t {
                    Tuple(ts) => ts.get(k).cloned().unwrap_or(Hole),
                    List(e) => (**e).clone(),
                    Json => Json,
                    _ => Hole,
                };
                bind(p, et, env);
            }
        }
        _ => {}
    }
}

// Names bound by a match arm: Json variants and Ok(v) have known types.
fn bind_arm(p: &Pat, subj: &T, env: &mut Env) {
    match (p, subj) {
        (Pat::Variant(v, args), _) if args.len() == 1 => {
            let t = match (v.as_str(), subj) {
                ("Str", Json) => Str,
                ("Num", Json) => F64,
                ("Bool", Json) => Bool,
                ("Arr", Json) => List(Box::new(Json)),
                ("Obj", Json) => Map(Box::new(Str), Box::new(Json)),
                ("Ok", t) => t.clone(),
                ("Err", _) => Str,
                _ => Hole,
            };
            bind(&args[0], t, env);
        }
        // A user enum's fields: unknown here, but the names are the arm's own, not outer ones of the same name.
        (Pat::Variant(_, args), _) => args.iter().for_each(|a| bind(a, Hole, env)),
        (p, t) => bind(p, t.clone(), env),
    }
}

fn bind_params(ps: &[String], et: &T, env: &mut Env) {
    if ps.len() == 1 {
        env.insert(ps[0].clone(), et.clone());
    } else {
        for (k, p) in ps.iter().enumerate() {
            let t = if let Tuple(ts) = et { ts.get(k).cloned().unwrap_or(Hole) } else { Hole };
            env.insert(p.clone(), t);
        }
    }
}

// Two known types no value can have both of (an int next to a decimal mixes in).
// Only a list, set or tuple next to another kind counts: two scalars (str next to int) more often mean
// one of the guesses is off than a slot that really holds both.
fn ty_clash(a: &T, b: &T) -> bool {
    let coll = |t: &T| matches!(t, List(_) | Tuple(_) | Map(..));
    a != b && to_ast(a).is_some() && to_ast(b).is_some() && (coll(a) || coll(b))
}

// `n = v` as the last use of n in each branch of an if chain becomes `n = ax_to_json(v)`.
fn json_branch_sets(e: &mut Expr, n: &str) {
    let mentions = |s: &Stmt| format!("{s:?}").contains(&format!("Ident({n:?})"));
    let block = |b: &mut Expr| {
        if let Expr::Block(stmts, _) = b {
            let len = stmts.len();
            for i in 0..len {
                let last = !stmts[i + 1..].iter().any(|s| mentions(s));
                if let Stmt::Assign { target: Expr::Ident(t), op: "=", value, line } = &mut stmts[i] {
                    if t == n && last && !matches!(value, Expr::Call(f, _, _) if f == "ax_to_json") {
                        let v = std::mem::replace(value, Expr::Int(0));
                        *value = Expr::Call("ax_to_json".into(), vec![(None, v)], *line);
                    }
                }
            }
        }
    };
    if let Expr::If(_, a, b) = e {
        block(a);
        match b.as_deref_mut() {
            Some(i @ Expr::If(..)) => json_branch_sets(i, n),
            Some(b) => block(b),
            None => {}
        }
    }
}

// The slots of returned tuples that hold values of clashing types on different paths, as paths into
// nested tuples (`(qid, ('multi', pts, frozenset(s)))` gives [1, 2]).
fn json_slots(ts: &[T]) -> Vec<Vec<usize>> {
    let tuples: Vec<&Vec<T>> = ts.iter().filter_map(|t| if let Tuple(xs) = t { Some(xs) } else { None }).collect();
    let Some(n) = tuples.first().map(|x| x.len()) else { return vec![] };
    if tuples.len() < 2 || tuples.iter().any(|x| x.len() != n) {
        return vec![];
    }
    let mut out = vec![];
    for i in 0..n {
        let at: Vec<T> = tuples.iter().map(|x| x[i].clone()).collect();
        let known: Vec<&T> = at.iter().filter(|t| to_ast(t).is_some()).collect();
        let inner = json_slots(&at);
        // Tuples of one length at this slot: look inside them instead.
        if !inner.is_empty() && at.iter().all(|t| matches!(t, Tuple(x) if x.len() == inner_len(&at))) {
            out.extend(inner.into_iter().map(|mut p| {
                p.insert(0, i);
                p
            }));
        } else if known.iter().any(|a| known.iter().any(|b| ty_clash(a, b))) {
            out.push(vec![i]);
        }
    }
    out
}

fn inner_len(ts: &[T]) -> usize {
    match ts.first() {
        Some(Tuple(x)) => x.len(),
        _ => 0,
    }
}

fn set_path(t: &mut T, p: &[usize]) {
    match (t, p) {
        (t, []) => *t = Json,
        (Tuple(xs), [i, rest @ ..]) => {
            if let Some(x) = xs.get_mut(*i) {
                set_path(x, rest);
            }
        }
        _ => {}
    }
}

// `return (a, b, x)` with slot 2 holding JSON: `return (a, b, ax_to_json(x))`.
fn json_returns(body: &mut Expr, slots: &[Vec<usize>]) {
    fn wrap_at(v: &mut Expr, p: &[usize]) {
        match (v, p) {
            (Expr::Call(f, _, _), []) if f == "ax_to_json" => {}
            (x, []) => {
                let inner = std::mem::replace(x, Expr::Int(0));
                *x = Expr::Call("ax_to_json".into(), vec![(None, inner)], 0);
            }
            (Expr::Tuple(xs), [i, rest @ ..]) => {
                if let Some(x) = xs.get_mut(*i) {
                    wrap_at(x, rest);
                }
            }
            _ => {}
        }
    }
    let wrap = |v: &mut Expr| {
        if matches!(v, Expr::Tuple(_)) {
            for p in slots {
                wrap_at(v, p);
            }
        }
    };
    crate::parser::walk_mut(body, &mut |e| {
        if let Expr::Block(stmts, _) = e {
            for st in stmts.iter_mut() {
                if let Stmt::Return { value: Some(v), .. } = st {
                    wrap(v);
                }
            }
        }
    });
    if let Expr::Block(_, Some(t)) = body {
        wrap(t);
    }
}

fn to_ast(t: &T) -> Option<Ty> {
    let n = |s: &str| Some(Ty::Named(s.into()));
    match t {
        Int => n("int"),
        F64 => n("f64"),
        Bool => n("bool"),
        Str => n("str"),
        Dec => n("dec"),
        Big => n("big"),
        Date => n("Date"),
        Json => n("Json"),
        Named(s) => n(s),
        List(e) => Some(Ty::List(Box::new(to_ast(e)?))),
        Map(k, v) => Some(Ty::Map(Box::new(to_ast(k)?), Box::new(to_ast(v)?))),
        Tuple(ts) => Some(Ty::Tuple(ts.iter().map(to_ast).collect::<Option<Vec<_>>>()?)),
        Heap(e) => Some(Ty::Named(format!("AxHeap<{}>", rust(e)))).filter(|_| known(e)),
        Unit => Some(Ty::Unit),
        Hole => None,
    }
}

// Is this expression, as a function's last line, a statement rather than a value?
// The value is a print on every path (`if c: print(a) else: print(b)`).
fn ends_in_print(e: &Expr) -> bool {
    match e {
        Expr::Call(f, _, _) => f == "print" || f == "write",
        Expr::Block(_, Some(t)) => ends_in_print(t),
        Expr::If(_, a, Some(b)) => ends_in_print(a) && ends_in_print(b),
        Expr::Match(_, arms) => !arms.is_empty() && arms.iter().all(|(_, a)| ends_in_print(a)),
        _ => false,
    }
}

fn stmt_branch(e: &Expr) -> bool {
    match e {
        Expr::Block(_, Some(t)) => stmt_branch(t),
        Expr::Call(..) | Expr::Method(..) | Expr::For(..) | Expr::While(..) | Expr::If(..) => {
            !matches!(e, Expr::Call(n, _, _) if n == "fail") && no_value(e)
        }
        _ => false,
    }
}

fn no_value(e: &Expr) -> bool {
    match e {
        Expr::For(..) | Expr::While(..) => true,
        Expr::If(_, _, None) => true,
        // `if store.has(k): store.remove(k) else: print("NOT FOUND")`: a branch that only prints or
        // changes a collection makes the whole if a statement (a branch ending in fail still has a value).
        Expr::If(_, a, Some(b)) => stmt_branch(a) || stmt_branch(b),
        Expr::Call(n, _, _) => ["print", "write", "fail"].contains(&n.as_str()),
        // Changing a list or map is a statement even though remove and pop give a value.
        Expr::Method(_, m, _, _) => ["push", "pop_or", "insert", "remove", "clear", "update", "pop", "heap_push", "heap_pop", "heapify", "pop_at", "move_to_end", "pop_item", "next_or", "next_or_empty"].contains(&m.as_str()),
        _ => false,
    }
}

impl Cx<'_> {
    // Types of every name set in a body, in order (first setting wins), for reading return values.
    fn flat_env(&mut self, e: &Expr, env: &mut Env) {
        match e {
            Expr::Block(stmts, tail) => {
                for (i, st) in stmts.iter().enumerate() {
                    match st {
                        // `out = []` then `out.push(x)`: the type comes from the lines after it.
                        Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if empty_kind(value).is_some() && env.get(n).map_or(true, |t| !known(t)) => {
                            let mut mt = empty_kind(value).unwrap();
                            let mut later = env.clone();
                            later.insert(n.clone(), mt.clone());
                            self.unsure = false;
                            self.look(n, &stmts[i + 1..], tail.as_deref(), &mut later, &mut mt);
                            if let Some(tl) = tail {
                                self.look_expr(n, tl, &mut later, &mut mt);
                            }
                            if std::mem::take(&mut self.unsure) {
                                mt = List(Box::new(Hole));
                            }
                            env.insert(n.clone(), mt);
                        }
                        Stmt::Let { name, ty, value, .. } => {
                            self.flat_env(value, env);
                            let t = ty.as_ref().map(from_ast).unwrap_or_else(|| self.ty(value, env));
                            bind(name, t, env);
                        }
                        Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } => {
                            self.flat_env(value, env);
                            if !env.contains_key(n) || env[n] == Hole {
                                let t = self.ty(value, env);
                                env.insert(n.clone(), t);
                            } else if !known(&env[n]) && known(&self.ty(value, env)) {
                                // `xs = []` in one branch, `xs = [e for e in ...]` in the other.
                                let t = self.ty(value, env);
                                env.insert(n.clone(), t);
                            } else {
                                // Python's `st = int(st)`: the name holds the new type from here on.
                                let t = self.ty(value, env);
                                if clash(&env[n], &t) {
                                    env.insert(n.clone(), t);
                                }
                            }
                        }
                        Stmt::Assign { target: Expr::Tuple(xs), op: "=", value, .. } => {
                            let t = self.ty(value, env);
                            // A name already there but still unknown (an earlier loop's `for sku, q in ...`) takes this type.
                            let mut put = |n: &String, t: T| {
                                if env.get(n).map_or(true, |o| !known(o) && known(&t)) {
                                    env.insert(n.clone(), t);
                                }
                            };
                            for (k, x) in xs.iter().enumerate() {
                                if let (Expr::Ident(n), Tuple(ts)) = (x, &t) {
                                    put(n, ts.get(k).cloned().unwrap_or(Hole));
                                }
                                // `a, b = line.split()`: each name is an item.
                                if let (Expr::Ident(n), List(e)) = (x, &t) {
                                    put(n, *e.clone());
                                }
                            }
                        }
                        Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } => self.flat_env(expr, env),
                        _ => {}
                    }
                }
                if let Some(t) = tail {
                    self.flat_env(t, env);
                }
            }
            Expr::For(p, it, body) => {
                let t = loop_elem(p, &self.ty(it, env));
                // A loop name that is still unknown here does not hide what an earlier setting found
                // (`name = parts[0]` above, then `for name, total, _ in roster.students`).
                let mut tmp = Env::new();
                bind(p, t, &mut tmp);
                // (A single loop name over a list not known yet is not taken to be the earlier value:
                // `q = parse(s)` above, then `for q in g.qids`.)
                let single = matches!(p, Pat::Bind(_));
                for (n, t) in tmp {
                    if known(&t) || single || env.get(&n).map_or(true, |o| !known(o)) {
                        env.insert(n, t);
                    }
                }
                self.flat_env(body, env);
            }
            Expr::While(_, b) => self.flat_env(b, env),
            Expr::If(_, a, b) => {
                self.flat_env(a, env);
                if let Some(b) = b {
                    self.flat_env(b, env);
                }
            }
            Expr::Match(s, arms) => {
                let st = self.ty(s, env);
                for (p, a) in arms {
                    bind_arm(p, &st, env);
                    self.flat_env(a, env);
                }
            }
            _ => {}
        }
    }

    fn returned(&self, e: &Expr, env: &Env, out: &mut Vec<T>) {
        match e {
            Expr::Block(stmts, tail) => {
                for st in stmts {
                    match st {
                        // `return print(...)` gives no value.
                        Stmt::Return { value: Some(v), .. } if no_value(v) => out.push(Unit),
                        Stmt::Return { value: Some(v), .. } => out.push(self.ty(v, env)),
                        Stmt::Return { value: None, .. } => out.push(Unit),
                        Stmt::Expr { expr, .. } => self.returned(expr, env, out),
                        _ => {}
                    }
                }
                if let Some(t) = tail {
                    self.returned(t, env, out);
                }
            }
            Expr::For(_, _, b) | Expr::While(_, b) => self.returned(b, env, out),
            Expr::If(_, a, b) => {
                self.returned(a, env, out);
                if let Some(b) = b {
                    self.returned(b, env, out);
                }
            }
            Expr::Match(_, arms) => arms.iter().for_each(|(_, a)| self.returned(a, env, out)),
            _ => {}
        }
    }
}

// Every call to a named function in `e`, with its arguments.
fn calls_in<'a>(e: &'a Expr, out: &mut Vec<(&'a str, &'a [(Option<String>, Expr)])>) {
    let mut go = |x: &'a Expr| calls_in(x, out);
    match e {
        Expr::Call(f, args, _) => {
            for (_, a) in args {
                calls_in(a, out);
            }
            out.push((f.as_str(), args.as_slice()));
        }
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(go),
        Expr::Map(ps) => {
            for (k, v) in ps {
                calls_in(k, out);
                calls_in(v, out);
            }
        }
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    calls_in(x, out);
                }
            }
        }
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) | Expr::Try(a) => go(a),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) | Expr::For(_, a, b) => {
            go(a);
            calls_in(b, out);
        }
        Expr::Method(r, _, args, _) => {
            calls_in(r, out);
            args.iter().for_each(|a| calls_in(a, out));
        }
        Expr::If(c, a, b) => {
            calls_in(c, out);
            calls_in(a, out);
            if let Some(b) = b {
                calls_in(b, out);
            }
        }
        Expr::Match(s, arms) => {
            calls_in(s, out);
            arms.iter().for_each(|(_, a)| calls_in(a, out));
        }
        Expr::Block(stmts, tail) => {
            for st in stmts {
                match st {
                    Stmt::Let { value, .. } => calls_in(value, out),
                    Stmt::Assign { target, value, .. } => {
                        calls_in(target, out);
                        calls_in(value, out);
                    }
                    Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } | Stmt::Assert { cond: expr, .. } => calls_in(expr, out),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                calls_in(t, out);
            }
        }
        _ => {}
    }
}

// Fills in the return type of every function written without `->`, from what it returns.
// The type another function gives a name that also takes f's value, when it fills f's unknown parts.
fn partner_ret(items: &[Item], f: &str, fns: &HashMap<String, (Vec<(String, T)>, T)>, t: &T) -> Option<T> {
    let mut found = None;
    for it in items {
        let Item::Fn { body, .. } = it else { continue };
        let mut from: HashMap<String, Vec<String>> = HashMap::new();
        walk(body, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts {
                    match st {
                        Stmt::Let { name: Pat::Bind(n), value: Expr::Call(c, _, _), .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Call(c, _, _), .. } => {
                            from.entry(n.clone()).or_default().push(c.clone());
                        }
                        _ => {}
                    }
                }
            }
        });
        for cs in from.values().filter(|cs| cs.iter().any(|c| c == f)) {
            for g in cs.iter().filter(|c| *c != f) {
                if let Some((_, gt)) = fns.get(g) {
                    let m = merge(t.clone(), gt.clone());
                    if known(gt) && known(&m) {
                        found = Some(m);
                    }
                }
            }
        }
    }
    found
}

pub fn returns(items: &mut [Item]) -> Result<(), Vec<crate::diag::Diag>> {
    let q = |t: &Ty| matches!(t, Ty::Named(n) if n == "?");
    let open = |items: &[Item]| {
        items.iter().any(|i| matches!(i, Item::Fn { ret, params, .. } if q(ret) || params.iter().any(|p| q(&p.ty))) || matches!(i, Item::Type { fields, .. } if fields.iter().any(|f| q(&f.ty))))
    };
    if !open(items) {
        return Ok(());
    }
    // Map literals whose values differ in type are Json objects, also as return values.
    let (_, mixed) = empty_types(items);
    // Whether the last round still learned types from calls or returns (the evidence param_look waits for).
    let mut moving = true;
    for round in 0..12 {
        // Top-level names (`ts = {}` filled inside a function) are known by type where functions read them.
        let genv = empty_cx(items).genv;
        let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: mixed.clone(), genv, param_look: false, unsure: false, entry_look: false };
        for it in items.iter() {
            match it {
                Item::Fn { name, params, ret, .. } => {
                    let r = if matches!(ret, Ty::Named(n) if n == "?") { Hole } else { from_ast(ret) };
                    cx.fns.insert(name.clone(), (params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect(), r));
                }
                Item::Type { name, fields, .. } => {
                    cx.structs.insert(name.clone(), fields.iter().map(|f| (f.name.clone(), from_ast(&f.ty))).collect());
                }
                Item::Enum { name, variants, .. } => {
                    for v in variants {
                        cx.variants.insert(v.name.clone(), name.clone());
                    }
                }
                _ => {}
            }
        }
        let last = round == 11;
        // A field typed this round is seen by the functions only next round.
        let mut fields_moved = false;
        let mut errs = Vec::new();
        // Python: a name set to values of clashing types in the branches of an if (`spec = parts[3]` /
        // `spec = frozenset(s)` / `spec = (Decimal(v), tol)`) and used after it holds JSON values.
        if py_on() {
            for it in items.iter_mut() {
                let Item::Fn { params, body, .. } = it else { continue };
                let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
                cx.flat_env(body, &mut env);
                let mut wraps: Vec<(usize, String)> = vec![];
                let mut k = 0;
                walk(body, &mut |e| {
                    if let Expr::Block(stmts, _) = e {
                        for st in stmts {
                            k += 1;
                            let Stmt::Expr { expr: ife @ Expr::If(..), .. } = st else { continue };
                            let mut sets = Vec::new();
                            cx.branch_sets(ife, &env, &mut sets);
                            for (n, t) in &sets {
                                if sets.iter().any(|(m, u)| m == n && ty_clash(t, u)) && !wraps.iter().any(|(kk, m)| *kk == k && m == n) {
                                    wraps.push((k, n.clone()));
                                }
                            }
                        }
                    }
                });
                if wraps.is_empty() {
                    continue;
                }
                let mut k = 0;
                crate::parser::walk_mut(body, &mut |e| {
                    if let Expr::Block(stmts, _) = e {
                        for st in stmts.iter_mut() {
                            k += 1;
                            let Stmt::Expr { expr: ife @ Expr::If(..), .. } = st else { continue };
                            for (_, n) in wraps.iter().filter(|(kk, _)| *kk == k) {
                                json_branch_sets(ife, n);
                            }
                        }
                    }
                });
            }
        }
        // `stock.take(sku, n)` on a value now known to be a class with that method calls the method, also when
        // the name is a built-in one or the class comes from another file.
        for it in items.iter_mut() {
            let Item::Fn { params, body, .. } = it else { continue };
            let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
            cx.flat_env(body, &mut env);
            crate::parser::walk_mut(body, &mut |e| {
                rec_field(e, &|x| global_too(cx.ty(x, &env), x, &cx.genv), &cx.structs);
                if let Expr::Method(r, m, args, line) = e {
                    if let Named(c) = cx.ty(r, &env) {
                        let f = format!("{c}_{m}");
                        if cx.fns.contains_key(&f) {
                            let mut a = vec![(None, (**r).clone())];
                            a.extend(args.drain(..).map(|x| (None, x)));
                            *e = Expr::Call(f, a, *line);
                        }
                    }
                }
            });
        }
        // A parameter written without a type (`fn chk(t: str, cc):`) takes the type of what the calls pass.
        let untyped: HashMap<String, usize> = items
            .iter()
            .enumerate()
            .filter_map(|(k, i)| match i {
                Item::Fn { name, params, .. } if params.iter().any(|p| q(&p.ty)) => Some((name.clone(), k)),
                _ => None,
            })
            .collect();
        if !untyped.is_empty() {
            let mut fills: Vec<(usize, usize, Ty)> = Vec::new();
            for it in items.iter() {
                let (params, body): (&[Param], &Expr) = match it {
                    Item::Fn { params, body, .. } => (params, body),
                    Item::Test { body, .. } => (&[], body),
                    _ => continue,
                };
                let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
                cx.flat_env(body, &mut env);
                // A function passed by name to a method that calls it on each item (`sorted(rows, key=key)`,
                // `map(double, xs)`) takes the item type.
                walk(body, &mut |e| {
                    if let Expr::Method(r, m, args, _) = e {
                        if ["sort_by", "map", "filter", "any", "all", "count", "min_by", "max_by", "group_by", "group_runs"].contains(&m.as_str()) {
                            for a in args {
                                if let Expr::Ident(f) = a {
                                    if let Some(&k) = untyped.get(f) {
                                        if let Item::Fn { params: ps, .. } = &items[k] {
                                            if ps.len() == 1 && q(&ps[0].ty) {
                                                if let Some(t) = to_ast(&each_elem(&cx.ty(r, &env))) {
                                                    fills.push((k, 0, t));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                // Calls inside `all(valid(c) for c in t[1:])`: the lambda's names take the items' type.
                let mut lams: Vec<(Expr, Env)> = Vec::new();
                walk(body, &mut |e| {
                    if let Expr::Method(r, _, args, _) = e {
                        for a in args {
                            if let Expr::Lambda(ps, lb) = a {
                                let mut inner = env.clone();
                                bind_params(ps, &each_elem(&cx.ty(r, &env)), &mut inner);
                                lams.push((lb.as_ref().clone(), inner));
                            }
                        }
                    }
                });
                for (lb, lenv) in &lams {
                    let mut calls = Vec::new();
                    calls_in(lb, &mut calls);
                    for (f, args) in calls {
                        let Some(&k) = untyped.get(f) else { continue };
                        let Item::Fn { params: ps, .. } = &items[k] else { continue };
                        for (j, (nm, a)) in args.iter().enumerate() {
                            let pi = match nm {
                                Some(nm) => ps.iter().position(|p| &p.name == nm),
                                None => Some(j),
                            };
                            let Some(pi) = pi.filter(|&pi| pi < ps.len() && q(&ps[pi].ty)) else { continue };
                            if let Some(t) = to_ast(&cx.ty(a, lenv)) {
                                fills.push((k, pi, t));
                            }
                        }
                    }
                }
                let mut calls = Vec::new();
                calls_in(body, &mut calls);
                for (f, args) in calls {
                    let Some(&k) = untyped.get(f) else { continue };
                    let Item::Fn { params: ps, .. } = &items[k] else { continue };
                    for (j, (nm, a)) in args.iter().enumerate() {
                        let pi = match nm {
                            Some(nm) => ps.iter().position(|p| &p.name == nm),
                            None => Some(j),
                        };
                        let Some(pi) = pi.filter(|&pi| pi < ps.len() && q(&ps[pi].ty)) else { continue };
                        if let Some(t) = to_ast(&cx.ty(a, &env)) {
                            fills.push((k, pi, t));
                        }
                    }
                }
            }
            // A parameter no call pins down takes the type its own function's uses show
            // (`lots.push(Lot(...))`, `st[t]`, `p.qty`).
            let from_calls = !fills.is_empty();
            for (k, it) in items.iter().enumerate() {
                let Item::Fn { params, body, .. } = it else { continue };
                for (pi, p) in params.iter().enumerate() {
                    // Calls are the better evidence, so this waits until they have had two rounds and stopped
                    // teaching anything new.
                    if round < 2 || ((from_calls || moving) && round < 9) || !q(&p.ty) || fills.iter().any(|f| f.0 == k && f.1 == pi) {
                        continue;
                    }
                    let mut env: Env = params.iter().filter(|o| !q(&o.ty)).map(|o| (o.name.clone(), from_ast(&o.ty))).collect();
                    let mut acc = Hole;
                    cx.param_look = true;
                    match body {
                        Expr::Block(stmts, tail) => {
                            cx.look(&p.name, stmts, tail.as_deref(), &mut env, &mut acc);
                            if let Some(t) = tail {
                                cx.look_expr(&p.name, t, &mut env, &mut acc);
                            }
                        }
                        e => cx.look_expr(&p.name, e, &mut env, &mut acc),
                    }
                    cx.param_look = false;
                    if let Some(t) = to_ast(&acc) {
                        fills.push((k, pi, t));
                    }
                }
            }
            // Python: a parameter given values of clashing types by different calls (`Question(qid, kind, pts, t[3])`
            // next to `Question(qid, kind, pts, frozenset(s))`) holds JSON values.
            if py_on() {
                let mut clash: Vec<(usize, usize)> = vec![];
                for (i, (k, pi, t)) in fills.iter().enumerate() {
                    if fills[..i].iter().any(|(k2, p2, t2)| k2 == k && p2 == pi && ty_clash(&from_ast(t), &from_ast(t2))) && !clash.contains(&(*k, *pi)) {
                        clash.push((*k, *pi));
                    }
                }
                fills.retain(|(k, pi, _)| !clash.contains(&(*k, *pi)));
                for (k, pi) in clash {
                    fills.push((k, pi, Ty::Named("Json".into())));
                }
            }
            for (k, pi, t) in fills {
                if let Item::Fn { params, .. } = &mut items[k] {
                    if q(&params[pi].ty) {
                        params[pi].ty = t;
                    }
                }
            }
            if last {
                drop_unused_params(items);
                // A function nothing calls (a Python method whose one use was inlined) needs no types.
                let mut used: HashSet<String> = HashSet::new();
                for it in items.iter() {
                    if let Item::Fn { body, .. } | Item::Test { body, .. } = it {
                        walk(body, &mut |e| match e {
                            Expr::Call(f, _, _) | Expr::Ident(f) => {
                                used.insert(f.clone());
                            }
                            Expr::Method(_, m, _, _) => {
                                used.insert(m.clone());
                            }
                            _ => {}
                        });
                    }
                }
                for it in items.iter_mut() {
                    if let Item::Fn { name, params, ret, body, .. } = it {
                        // `Orders_add_backorder` is called as `.add_backorder`, `Stock__ordered` as `._ordered`.
                        let called = used.contains(name.as_str()) || name.match_indices('_').any(|(i, _)| used.contains(&name[i + 1..]) || used.contains(&name[i..]));
                        if name != "main" && !called && params.iter().any(|p| q(&p.ty)) {
                            params.clear();
                            *ret = Ty::Named("?".into());
                            *body = Expr::Block(vec![], None);
                        }
                    }
                }
                for it in items.iter() {
                    if let Item::Fn { name, params, line, .. } = it {
                        for p in params.iter().filter(|p| q(&p.ty)) {
                            let mut d = crate::diag::Diag::new("check", *line, 1, &format!("cannot work out the type of {} in {name}", p.name));
                            d.fix = Some(format!("write it: {}: TYPE", p.name));
                            errs.push(d);
                        }
                    }
                }
                if !errs.is_empty() {
                    return Err(errs);
                }
            }
        }
        // Fields of a Python class (`self.n = 0`, `self.items.append(x)`) take the type of what is put there.
        let open_types: Vec<(String, String)> = items
            .iter()
            .flat_map(|i| match i {
                Item::Type { name, fields, .. } => fields.iter().filter(|f| q(&f.ty)).map(|f| (name.clone(), f.name.clone())).collect::<Vec<_>>(),
                _ => vec![],
            })
            .collect();
        if !open_types.is_empty() {
            let mut ev: HashMap<(String, String), T> = HashMap::new();
            let mut idx_ev: Vec<((String, String), T, T)> = vec![];
            for it in items.iter() {
                let Item::Fn { params, body, .. } = it else { continue };
                let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
                cx.flat_env(body, &mut env);
                let owner = |e: &Expr, env: &Env| -> Option<(String, String)> {
                    let Expr::Field(r, f) = e else { return None };
                    let Expr::Ident(n) = r.as_ref() else { return None };
                    match env.get(n).filter(|t| **t != Hole).or_else(|| cx.genv.get(n)) {
                        Some(Named(c)) if open_types.iter().any(|(oc, of)| oc == c && of == f) => Some((c.clone(), f.clone())),
                        _ => None,
                    }
                };
                fn add(ev: &mut HashMap<(String, String), T>, k: (String, String), t: T) {
                    let cur = ev.remove(&k).unwrap_or(Hole);
                    // Python: a field set to a string on one path and a list on another holds JSON values.
                    if py_on() && known(&cur) && known(&t) && ty_clash(&cur, &t) {
                        ev.insert(k, Json);
                        return;
                    }
                    ev.insert(k, merge(cur, t));
                }
                walk(body, &mut |e| {
                    if let Expr::Block(stmts, _) = e {
                        for st in stmts {
                            let Stmt::Assign { target, op, value, .. } = st else { continue };
                            if let Some(k) = owner(target, &env) {
                                let t = match empty_kind(value) {
                                    Some(t) => t,
                                    None => cx.ty(value, &env),
                                };
                                if *op == "=" || *op == "+=" {
                                    add(&mut ev, k, t);
                                }
                            } else if let Expr::Index(b, ix) = target {
                                if let Some(k) = owner(b, &env) {
                                    // Whether the field is a list or a map shows from all functions: decided below.
                                    idx_ev.push((k, cx.ty(ix, &env), cx.ty(value, &env)));
                                } else if let Expr::Index(b2, ix2) = b.as_ref() {
                                    // `self.scores[name][qid] = score`: a map of maps (keyed by a non-int).
                                    let (t2, t1) = (cx.ty(ix2, &env), cx.ty(ix, &env));
                                    if let Some(k) = owner(b2, &env) {
                                        if *op == "=" && known(&t2) && t2 != Int && known(&t1) {
                                            add(&mut ev, k, Map(Box::new(t2), Box::new(Map(Box::new(t1), Box::new(cx.ty(value, &env))))));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Expr::Method(r, m, args, _) = e {
                        if let Some(k) = owner(r, &env) {
                            if m == "push" && args.len() == 1 {
                                add(&mut ev, k.clone(), List(Box::new(cx.ty(&args[0], &env))));
                            }
                            // `self.seen.update(cards)` / `xs.extend(ys)` on a field that starts as set() or [].
                            if matches!(m.as_str(), "update" | "extend") && args.len() == 1 {
                                if let t @ List(_) = cx.ty(&args[0], &env) {
                                    add(&mut ev, k, t);
                                }
                            }
                        } else if let Expr::Index(b, ix) = r.as_ref() {
                            // `self.lots.setdefault(sku, []).append(lot)` (read as `self_.lots[sku].push(lot)`).
                            if let Some(k) = owner(b, &env) {
                                if m == "push" && args.len() == 1 && !matches!(ev.get(&k), Some(List(_))) {
                                    let t = Map(Box::new(cx.ty(ix, &env)), Box::new(List(Box::new(cx.ty(&args[0], &env)))));
                                    add(&mut ev, k, t);
                                }
                            }
                        }
                    }
                });
            }
            for (k, kt, vt) in idx_ev {
                let t = match ev.get(&k) {
                    Some(List(_)) => List(Box::new(vt)),
                    _ => Map(Box::new(kt), Box::new(vt)),
                };
                let cur = ev.remove(&k).unwrap_or(Hole);
                ev.insert(k, merge(cur, t));
            }
            for it in items.iter_mut() {
                if let Item::Type { name, fields, .. } = it {
                    for f in fields.iter_mut().filter(|f| q(&f.ty)) {
                        if let Some(t) = ev.get(&(name.clone(), f.name.clone())).and_then(to_ast) {
                            f.ty = t;
                            fields_moved = true;
                        }
                    }
                }
            }
            if last {
                // Python: a field nothing pins down (`'players': {}` never filled, `self.ep = None` only
                // ever set from another file) holds JSON values where its type is not known.
                if py_on() {
                    for it in items.iter_mut() {
                        if let Item::Type { name, fields, .. } = it {
                            for f in fields.iter_mut().filter(|f| q(&f.ty)) {
                                let t = ev.get(&(name.clone(), f.name.clone())).cloned().unwrap_or(Hole);
                                if let Some(t) = to_ast(&holes_json(t)) {
                                    f.ty = t;
                                }
                            }
                        }
                    }
                }
                for it in items.iter() {
                    if let Item::Type { name, fields, line } = it {
                        for f in fields.iter().filter(|f| q(&f.ty)) {
                            let mut d = crate::diag::Diag::new("check", *line, 1, &format!("cannot work out the type of {name}.{}", f.name));
                            d.fix = Some(format!("set self.{} to a value of its type in __init__", f.name));
                            errs.push(d);
                        }
                    }
                }
                if !errs.is_empty() {
                    return Err(errs);
                }
            }
        }
        let mut found = Vec::new();
        let mut to_json: Vec<(usize, Vec<Vec<usize>>)> = Vec::new();
        for (k, it) in items.iter().enumerate() {
            let Item::Fn { name, params, ret: Ty::Named(q), body, line, .. } = it else { continue };
            if q != "?" {
                continue;
            }
            if name == "main" {
                found.push((k, Ty::Unit));
                continue;
            }
            let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
            cx.flat_env(body, &mut env);
            let mut ts = Vec::new();
            cx.returned(body, &env, &mut ts);
            let has_return = !ts.is_empty();
            let tail = match body {
                Expr::Block(_, None) => None,
                Expr::Block(_, Some(t)) => Some(t.as_ref()),
                e => Some(e),
            };
            match tail {
                Some(t) if !no_value(t) => ts.push(cx.ty(t, &env)),
                _ => {}
            }
            // A bare `return` means the function gives no value; so does ending on a print
            // (`return 0` on the early paths is then only a way out, as in Python).
            let bare = ts.contains(&Unit) || tail.map_or(false, ends_in_print);
            let ts: Vec<T> = ts.into_iter().filter(|t| *t != Unit).collect();
            if ts.is_empty() || bare {
                found.push((k, Ty::Unit));
                continue;
            }
            // `return 0` on some paths and `return v` (a JSON value) on others: the function gives JSON.
            let json = ts.contains(&Json) && ts.iter().all(|t| matches!(t, Json | Int | F64 | Bool | Str | Hole));
            // Python tuples whose slot holds a different type per path (`(qid, 'choice', pts, t[3])`,
            // `(qid, 'multi', pts, set(t[3]))`, `(qid, 'number', pts, (v, tol))`): that slot holds JSON values.
            let mut ts = ts;
            if py_on() {
                let slots = json_slots(&ts);
                if !slots.is_empty() {
                    for t in ts.iter_mut() {
                        for p in &slots {
                            set_path(t, p);
                        }
                    }
                    to_json.push((k, slots));
                }
            }
            let t = if json { Json } else { ts.into_iter().fold(Hole, merge) };
            match to_ast(&t) {
                Some(t) => found.push((k, t)),
                // No `return` and a last line of unclear type (`if c: print(x) else: m.remove(k)`): no value, as in Python.
                None if last && !has_return && matches!(tail, Some(Expr::If(..) | Expr::Match(..))) => found.push((k, Ty::Unit)),
                // `s = new_state()` and later `s = step(s, k)`: a None slot takes the type the other call gives it.
                None if last && py_on() && partner_ret(items, name, &cx.fns, &t).is_some() => {
                    if let Some(t) = partner_ret(items, name, &cx.fns, &t).and_then(|p| to_ast(&p)) {
                        found.push((k, t));
                    }
                }
                None if last => {
                    let mut d = crate::diag::Diag::new("check", *line, 1, &format!("cannot work out what {name} returns"));
                    d.fix = Some(format!("write the return type: fn {name}(...) -> TYPE"));
                    errs.push(d);
                }
                None => {}
            }
        }
        moving = !found.is_empty() || fields_moved;
        for (k, slots) in to_json {
            if let Item::Fn { body, .. } = &mut items[k] {
                json_returns(body, &slots);
            }
        }
        for (k, t) in found {
            if let Item::Fn { ret, .. } = &mut items[k] {
                *ret = t;
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        if !open(items) {
            break;
        }
    }
    Ok(())
}

// ---- Brevi's own type check: clear mistakes get a Brevi message before rustc runs ----

fn show_t(t: &T) -> String {
    match t {
        Hole => "_".into(),
        Int => "int".into(),
        F64 => "f64".into(),
        Bool => "bool".into(),
        Str => "str".into(),
        Dec => "dec".into(),
        Big => "big".into(),
        Date => "Date".into(),
        Json => "Json".into(),
        Named(n) => n.clone(),
        List(e) => format!("[{}]", show_t(e)),
        Map(k, v) => format!("{{{}: {}}}", show_t(k), show_t(v)),
        Tuple(ts) => format!("({})", ts.iter().map(show_t).collect::<Vec<_>>().join(", ")),
        Heap(e) => format!("heap[{}]", show_t(e)),
        Unit => "nothing".into(),
    }
}

// Two fully known types that cannot be the same value (int literals mix into dec and big).
fn clash(a: &T, b: &T) -> bool {
    if !known(a) || !known(b) || matches!(a, Unit) || matches!(b, Unit) {
        return false;
    }
    match (a, b) {
        (Int, Dec | Big) | (Dec | Big, Int) => false,
        (Named(x), Named(y)) => x != y,
        (List(x), List(y)) | (Heap(x), Heap(y)) => clash(x, y),
        (Map(k1, v1), Map(k2, v2)) => clash(k1, k2) || clash(v1, v2),
        (Tuple(x), Tuple(y)) => x.len() != y.len() || x.iter().zip(y).any(|(a, b)| clash(a, b)),
        (a, b) => std::mem::discriminant(a) != std::mem::discriminant(b),
    }
}

fn float_lit(e: &mut Expr) {
    match e {
        Expr::Int(n) => *e = Expr::Float(*n as f64),
        Expr::Unary("-", x) => float_lit(x),
        _ => {}
    }
}

struct Ck<'a, 'b> {
    cx: &'a Cx<'b>,
    line: usize,
    errs: Vec<crate::diag::Diag>,
    // Where each local was first set, for messages.
    set_at: HashMap<String, usize>,
    // The item types found for untyped empty values, by (name, line).
    empties: &'a HashMap<(String, usize), T>,
    // Python-style file: a name set again to another type in the block that made it is a new name there.
    py: bool,
    // Names bound by the loop or function whose body the next block is.
    next_own: Vec<String>,
    // Names set from a value that may be None (a `None` or a call to a function that returns one).
    maybe_none: HashSet<String>,
    // Functions that return None somewhere.
    none_fns: &'a HashSet<String>,
}

impl Ck<'_, '_> {
    fn may_be_none(&self, e: &Expr) -> bool {
        match e {
            Expr::Ident(n) => self.maybe_none.contains(n),
            e => self.none_src(e),
        }
    }

    // A value that is None, or may be: `None`, `[] if c else None`, or a call to a function that returns None.
    fn none_src(&self, e: &Expr) -> bool {
        // `m.get(k)` is left alone: a dict whose values are lists rarely stores an empty one.
        if matches!(e, Expr::Method(_, m, a, _) if m == "get_or" && matches!(a.get(1), Some(Expr::Call(n, _, _)) if n == "none")) {
            return false;
        }
        let d = format!("{e:?}");
        // A regex match is never an empty list, so None stays apart from it.
        if d.contains("\"re_") {
            return false;
        }
        none_or_empty(&d) || self.none_fns.iter().any(|f| d.contains(&format!("Call({f:?},")))
    }

    fn note_none(&mut self, n: &str, value: &Expr) {
        if self.none_src(value) {
            self.maybe_none.insert(n.to_string());
        }
    }

    fn empty_t(&self, n: &str, line: usize, value: &Expr) -> Option<T> {
        empty_kind(value)?;
        self.empties.get(&(n.to_string(), line)).cloned()
    }

    fn err(&mut self, msg: String, fix: String) {
        if self.errs.iter().any(|d| d.line == self.line) {
            return;
        }
        let mut d = crate::diag::Diag::new("type", self.line, 1, &msg);
        d.fix = Some(fix);
        self.errs.push(d);
    }

    fn block(&mut self, stmts: &mut [Stmt], tail: &mut Option<Box<Expr>>, env: &mut Env) {
        // Names made in this block (so `w = int(w)` here can shadow them, as Python's rebinding does).
        let mut own: HashSet<String> = std::mem::take(&mut self.next_own).into_iter().collect();
        for st in stmts.iter_mut() {
            if self.py {
                let shadow = match &*st {
                    Stmt::Assign { target: Expr::Ident(n), op: "=", value, line } if own.contains(n) && clash(&env.get(n).cloned().unwrap_or(Hole), &self.cx.ty(value, env)) => {
                        Some((n.clone(), value.clone(), *line))
                    }
                    _ => None,
                };
                if let Some((n, value, line)) = shadow {
                    env.remove(&n);
                    *st = Stmt::Let { name: Pat::Bind(n), ty: None, value, mutable: true, line };
                }
            }
            match &*st {
                Stmt::Let { name, .. } => own.extend(pat_names(name)),
                Stmt::Assign { target: Expr::Ident(n), op: "=", .. } if !env.contains_key(n) => {
                    own.insert(n.clone());
                }
                Stmt::Assign { target: Expr::Tuple(xs), op: "=", .. } => {
                    for x in xs {
                        if let Expr::Ident(n) = x {
                            if !env.contains_key(n) {
                                own.insert(n.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
            match st {
                Stmt::Let { name, ty, value, line, .. } => {
                    self.line = *line;
                    self.expr(value, env);
                    let t = ty.as_ref().map(from_ast).unwrap_or_else(|| {
                        let e = if let Pat::Bind(n) = &*name { self.empty_t(n, *line, value) } else { None };
                        e.unwrap_or_else(|| self.cx.ty(value, env))
                    });
                    if let Pat::Bind(n) = name {
                        self.set_at.insert(n.clone(), *line);
                        self.note_none(n, value);
                    }
                    bind(name, t, env);
                }
                Stmt::Assign { target, op, value, line } => {
                    self.line = *line;
                    self.expr(value, env);
                    if let Expr::Ident(n) = &*target {
                        self.note_none(n, value);
                    }
                    // `rec["done"] = True` on a JSON record: the value is put in as JSON.
                    if self.py && *op == "=" {
                        if let Expr::Index(b, _) = &*target {
                            let vt = self.cx.ty(value, env);
                            if self.cx.ty(b, env) == Json && vt != Json && (scalar(&vt) || matches!(vt, List(_) | Map(..))) {
                                let v = std::mem::replace(value, Expr::Int(0));
                                *value = Expr::Call("ax_to_json".into(), vec![(None, v)], *line);
                            }
                        }
                        // A field (or a dict record's key) that holds JSON values: `q['spec'] = set(s)`.
                        if matches!(&*target, Expr::Index(..) | Expr::Field(..)) && self.cx.ty(target, env) == Json {
                            let vt = self.cx.ty(value, env);
                            if vt != Json && vt != Hole && (scalar(&vt) || matches!(vt, List(_) | Map(..))) && !matches!(value, Expr::Call(f, _, _) if f == "ax_to_json") {
                                let v = std::mem::replace(value, Expr::Int(0));
                                *value = Expr::Call("ax_to_json".into(), vec![(None, v)], *line);
                            }
                        }
                    }
                    if !matches!(target, Expr::Ident(_)) {
                        self.expr(target, env);
                    }
                    match (target, *op) {
                        (Expr::Ident(n), "=") => {
                            let t = self.empty_t(n, *line, value).unwrap_or_else(|| self.cx.ty(value, env));
                            match env.get(n) {
                                Some(old) if clash(old, &t) => {
                                    let at = self.set_at.get(n).copied().unwrap_or(0);
                                    let msg = format!("{n} holds {} (set on line {at}) but is set to {} here", show_t(old), show_t(&t));
                                    self.err(msg, format!("use a new name for the {} value", show_t(&t)));
                                }
                                Some(Hole) | None => {
                                    self.set_at.entry(n.clone()).or_insert(*line);
                                    env.insert(n.clone(), t);
                                }
                                _ => {}
                            }
                        }
                        (Expr::Tuple(xs), "=") => {
                            let t = self.cx.ty(value, env);
                            for (k, x) in xs.iter().enumerate() {
                                if let Expr::Ident(n) = x {
                                    if !env.contains_key(n) {
                                        let et = match &t {
                                            Tuple(ts) => ts.get(k).cloned().unwrap_or(Hole),
                                            List(e) => (**e).clone(),
                                            Json => Json,
                                            _ => Hole,
                                        };
                                        env.insert(n.clone(), et);
                                    }
                                }
                            }
                        }
                        (Expr::Ident(n), "+=") => {
                            let (a, b) = (env.get(n).cloned().unwrap_or(Hole), self.cx.ty(value, env));
                            self.arith("+", &a, &b);
                        }
                        _ => {}
                    }
                }
                Stmt::Expr { expr, line } => {
                    self.line = *line;
                    self.expr(expr, env);
                }
                Stmt::Return { value: Some(v), line } => {
                    self.line = *line;
                    self.expr(v, env);
                }
                Stmt::Assert { cond, line } => {
                    self.line = *line;
                    self.expr(cond, env);
                }
                _ => {}
            }
        }
        if let Some(t) = tail {
            self.expr(t, env);
        }
    }

    fn arith(&mut self, op: &str, a: &T, b: &T) {
        let word = match op {
            "+" => "add",
            "-" => "subtract",
            "*" => "multiply",
            "/" | "//" => "divide",
            _ => "combine",
        };
        match (a, b) {
            (Str, Int | F64 | Bool | Dec) | (Int | F64 | Bool | Dec, Str) if op == "+" => {
                let msg = format!("cannot {word} {} and {}", show_t(a), show_t(b));
                self.err(msg, "turn the number into text with str(x), or write \"{x}\"".into())
            }
            (Int, F64) | (F64, Int) => {
                let msg = format!("cannot {word} int and f64");
                self.err(msg, "int and f64 never mix: convert with .float() or .int()".into())
            }
            _ => {}
        }
    }

    fn expr(&mut self, e: &mut Expr, env: &mut Env) {
        // `sum(e - s for s, e in m)` with m a list of lists: the lambda takes each list and unpacks it.
        if let Expr::Method(r, _, args, line) = &mut *e {
            if let List(inner) = self.cx.ty(r, env) {
                if matches!(*inner, List(_)) {
                    for a in args.iter_mut() {
                        if let Expr::Lambda(ps, body) = a {
                            if (2..=4).contains(&ps.len()) {
                                let names: Vec<Expr> = ps.iter().map(|p| Expr::Ident(p.clone())).collect();
                                let b = std::mem::replace(body.as_mut(), Expr::Int(0));
                                let un = Stmt::Assign { target: Expr::Tuple(names), op: "=", value: Expr::Ident("ax_row".into()), line: *line };
                                *a = Expr::Lambda(vec!["ax_row".into()], Box::new(Expr::Block(vec![un], Some(Box::new(b)))));
                            }
                        }
                    }
                }
            }
        }
        // A JSON value (a Python slot of a different type per row) next to a decimal or a list is read as one:
        // `abs(v - spec[0]) <= spec[1]`, `s == spec`, `s & spec`.
        if let (true, Expr::Binary(op, a, b)) = (self.py, &mut *e) {
            if matches!(*op, "+" | "-" | "*" | "/" | "//" | "%" | "<" | "<=" | ">" | ">=" | "==" | "!=" | "&" | "|" | "^") {
                let (ta, tb) = (self.cx.ty(a, env), self.cx.ty(b, env));
                let conv = |t: &T| match t {
                    Dec => Some("ax_json_dec"),
                    List(x) if **x == Str => Some("ax_json_strs"),
                    List(x) if **x == Int => Some("ax_json_ints"),
                    _ => None,
                };
                let side = if ta == Json && tb != Json { conv(&tb).map(|f| (f, a)) } else if tb == Json && ta != Json { conv(&ta).map(|f| (f, b)) } else { None };
                if let Some((f, x)) = side {
                    let v = std::mem::replace(x.as_mut(), Expr::Int(0));
                    **x = Expr::Call(f.into(), vec![(None, v)], 0);
                }
            }
        }
        // `Fraction(Decimal(s))`, `Decimal(d)`: a decimal already.
        if let Expr::Method(r, m, args, _) = &mut *e {
            if m == "dec" && args.is_empty() && self.cx.ty(r, env) == Dec {
                let inner = std::mem::replace(r.as_mut(), Expr::Int(0));
                *e = inner;
            }
        }
        // `Decimal(total) / Decimal(n)`: decimal division (an int side becomes a decimal).
        if let Expr::Call(f, args, _) = &mut *e {
            if f == "__truediv" && args.len() == 2 {
                let ts: Vec<T> = args.iter().map(|(_, a)| self.cx.ty(a, env)).collect();
                if ts.contains(&Dec) && ts.iter().all(|t| matches!(t, Dec | Int)) {
                    let mut v: Vec<Expr> = args.drain(..).map(|(_, a)| a).collect();
                    for (x, t) in v.iter_mut().zip(&ts) {
                        if *t == Int {
                            let inner = std::mem::replace(x, Expr::Int(0));
                            *x = Expr::Method(Box::new(inner), "dec".into(), vec![], 0);
                        }
                    }
                    let b = v.pop().unwrap();
                    let a = v.pop().unwrap();
                    *e = Expr::Binary("/", Box::new(a), Box::new(b));
                }
            }
        }
        // A list of JSON items (Python's mixed list) is given plain values as JSON.
        if let Expr::Method(r, m, args, line) = &mut *e {
            if matches!(m.as_str(), "push" | "insert") && !args.is_empty() {
                if let List(it) = self.cx.ty(r, env) {
                    let k = args.len() - 1;
                    if *it == Json && scalar(&self.cx.ty(&args[k], env)) && self.cx.ty(&args[k], env) != Json {
                        let v = std::mem::replace(&mut args[k], Expr::Int(0));
                        args[k] = Expr::Call("ax_to_json".into(), vec![(None, v)], *line);
                    }
                }
            }
        }
        // `dist.get(v, INF)` with int values and INF = float('inf'): the default is the biggest int.
        if let Expr::Method(r, m, args, _) = &mut *e {
            // Python's `list(d)` (a copy, `d.skip(0)`) of a dict is its keys.
            if m == "skip" && matches!(args.as_slice(), [Expr::Int(0)]) && matches!(self.cx.ty(r, env), Map(..)) {
                *m = "keys".into();
                args.clear();
            }
            if m == "get_or" && args.len() == 2 {
                if let Map(_, v) = self.cx.ty(r, env) {
                    if *v == Int && self.cx.ty(&args[1], env) == F64 {
                        let d = std::mem::replace(&mut args[1], Expr::Int(0));
                        args[1] = Expr::Method(Box::new(d), "int".into(), vec![], 0);
                    }
                }
            }
        }
        rec_field(e, &|x| global_too(self.cx.ty(x, env), x, &self.cx.genv), &self.cx.structs);
        // Python walks a tuple of one type like a list: `all(board[i] == p for i in line)`, `for i in line`.
        let same_tuple = |t: &T| matches!(t, Tuple(ts) if ts.len() >= 2 && known(&ts[0]) && ts.iter().all(|x| x == &ts[0]));
        match &mut *e {
            Expr::Method(r, m, _, _) if self.py && matches!(m.as_str(), "all" | "any" | "map" | "filter" | "sum" | "min" | "max" | "contains" | "index_of" | "count" | "sort" | "sort_by" | "rev" | "enumerate" | "zip" | "join" | "len" | "slice" | "skip") && same_tuple(&self.cx.ty(r, env)) => {
                let x = std::mem::replace(r.as_mut(), Expr::Int(0));
                **r = Expr::Method(Box::new(x), "tuple_items".into(), vec![], self.line);
            }
            Expr::Binary("in" | "not in", _, b) if self.py && same_tuple(&self.cx.ty(b, env)) => {
                let x = std::mem::replace(b.as_mut(), Expr::Int(0));
                **b = Expr::Method(Box::new(x), "tuple_items".into(), vec![], self.line);
            }
            Expr::For(_, it, _) if self.py && same_tuple(&self.cx.ty(it, env)) => {
                let x = std::mem::replace(it.as_mut(), Expr::Int(0));
                **it = Expr::Method(Box::new(x), "tuple_items".into(), vec![], self.line);
            }
            _ => {}
        }
        // t[0] on a tuple is t.0, and xs.0 on a list is xs[0].
        let fix = match &*e {
            Expr::List(xs) if mixed_record(xs, &|x| self.cx.ty(x, env)) => Some(Expr::Call("ax_row".into(), vec![(None, Expr::Tuple(xs.clone()))], 0)),
            Expr::Index(b, i) => match (self.cx.ty(b, env), i.as_ref()) {
                (Tuple(_), Expr::Int(k)) if *k >= 0 => Some(Expr::TupleField(b.clone(), *k as usize)),
                _ => None,
            },
            Expr::TupleField(b, k) => match self.cx.ty(b, env) {
                List(_) | Str => Some(Expr::Index(b.clone(), Box::new(Expr::Int(*k as i64)))),
                _ => None,
            },
            _ => None,
        };
        if let Some(f) = fix {
            *e = f;
        }
        // Count the tuple types made from mixed lists (rows) and those written as tuples: a type that is
        // both can't print as a list for rows and a tuple for tuples, so then rows print as tuples.
        match &*e {
            Expr::Call(f, a, _) if f == "ax_row" && a.len() == 1 => {
                let k = format!("{:?}", self.cx.ty(&a[0].1, env));
                ROW_TUPLES.with(|r| r.borrow_mut().entry(k).or_insert((0, 0)).0 += 1);
            }
            Expr::Tuple(_) => {
                let k = format!("{:?}", self.cx.ty(e, env));
                ROW_TUPLES.with(|r| r.borrow_mut().entry(k).or_insert((0, 0)).1 += 1);
            }
            _ => {}
        }
        match e {
            Expr::Block(stmts, tail) => {
                let mut inner = env.clone();
                self.block(stmts, tail, &mut inner);
            }
            Expr::If(c, a, b) => {
                self.expr(c, env);
                self.expr(a, &mut env.clone());
                if let Some(b) = b {
                    self.expr(b, &mut env.clone());
                }
            }
            Expr::Match(s, arms) => {
                self.expr(s, env);
                let st = self.cx.ty(s, env);
                for (p, a) in arms {
                    let mut inner = env.clone();
                    bind_arm(p, &st, &mut inner);
                    self.expr(a, &mut inner);
                }
            }
            Expr::For(p, it, body) => {
                self.expr(it, env);
                let mut it_t = self.cx.ty(it, env);
                // Python's `for k in m:` reads the keys (also `for (s, y) in m:` with tuple keys).
                if matches!(it_t, Map(..)) && (matches!(p, Pat::Bind(_)) || self.py) {
                    let m = std::mem::replace(it.as_mut(), Expr::Int(0));
                    **it = Expr::Method(Box::new(m), "keys".into(), vec![], self.line);
                    it_t = self.cx.ty(it, env);
                }
                let mut inner = env.clone();
                bind(p, loop_elem(p, &it_t), &mut inner);
                if matches!(**body, Expr::Block(..)) {
                    self.next_own = pat_names(p);
                }
                self.expr(body, &mut inner);
                self.next_own.clear();
            }
            Expr::While(c, body) => {
                self.expr(c, env);
                self.expr(body, &mut env.clone());
            }
            // Python's `m.group(3) or ""` is a value, not a test: the first operand when it is true, else the second.
            Expr::Binary(op @ ("or" | "and"), a, b) if or_value(&self.cx.ty(a, env), &self.cx.ty(b, env)).is_some() => {
                let (x, y) = ((**a).clone(), (**b).clone());
                let blk = |e: Expr| Box::new(Expr::Block(vec![], Some(Box::new(e))));
                *e = if *op == "or" { Expr::If(Box::new(x.clone()), blk(x), Some(blk(y))) } else { Expr::If(Box::new(x.clone()), blk(y), Some(blk(x))) };
                self.expr(e, env);
            }
            Expr::Binary(op, a, b) => {
                self.expr(a, env);
                self.expr(b, env);
                // A whole-number literal next to an f64 is that f64 (`x * 2`, `w > 0`).
                if ["+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">="].contains(op) {
                    if self.cx.ty(a, env) == F64 {
                        float_lit(b);
                    } else if self.cx.ty(b, env) == F64 {
                        float_lit(a);
                    }
                }
                // Python mixes int and float freely (`qty * price`): the int side is turned into a float.
                if ["+", "-", "*", "/", "//", "%", "==", "!=", "<", ">", "<=", ">=", "**"].contains(op) {
                    let (x, y) = (self.cx.ty(a, env), self.cx.ty(b, env));
                    let float = |e: &mut Box<Expr>| {
                        let inner = std::mem::replace(e.as_mut(), Expr::Int(0));
                        **e = Expr::Method(Box::new(inner), "float".into(), vec![], 0);
                    };
                    match (&x, &y) {
                        (Int, F64) if *op != "**" => float(a),
                        (F64, Int) if *op != "**" => float(b),
                        _ => {}
                    }
                }
                if ["+", "-", "*", "/", "//", "%"].contains(op) {
                    let (x, y) = (self.cx.ty(a, env), self.cx.ty(b, env));
                    self.arith(op, &x, &y);
                }
            }
            Expr::Call(n, args, line) => {
                self.line = *line;
                for (_, a) in args.iter_mut() {
                    self.expr(a, env);
                }
                if let Some((ps, _)) = self.cx.fns.get(n.as_str()) {
                    if args.iter().all(|(name, _)| name.is_none()) && args.len() == ps.len() {
                        for ((_, pt), (_, a)) in ps.iter().zip(args.iter_mut()) {
                            if *pt == F64 {
                                float_lit(a);
                            }
                            // A Python function given JSON values at some calls takes plain values as JSON too.
                            if self.py && *pt == Json {
                                let at = self.cx.ty(a, env);
                                if (scalar(&at) || matches!(at, List(_) | Tuple(_) | Map(..))) && at != Json && at != Hole {
                                    let v = std::mem::replace(a, Expr::Int(0));
                                    *a = Expr::Call("ax_to_json".into(), vec![(None, v)], *line);
                                }
                            }
                        }
                        for ((pn, pt), (_, a)) in ps.iter().zip(args.iter()) {
                            let at = self.cx.ty(a, env);
                            if clash(pt, &at) {
                                let msg = format!("{n}() takes {pn}: {}, but gets {} here", show_t(pt), show_t(&at));
                                self.err(msg, format!("pass {pn} as {}", show_t(pt)));
                            }
                        }
                    }
                }
            }
            Expr::Method(r, m, args, line) => {
                self.line = *line;
                self.expr(r, env);
                // A list or dict that may be None keeps None as an empty one, so `is None` can't tell them apart.
                if self.py && m == "is_none" && self.may_be_none(r) {
                    if let t @ (List(_) | Map(..)) = self.cx.ty(r, env) {
                        let what = if matches!(t, List(_)) { "list" } else { "dict" };
                        self.err(format!("this {what} may be None or empty, and Brevi can't tell those apart"), format!("use an empty {what} instead of None, and test it with `not x`"));
                    }
                }
                // Only these methods pass each item to their lambda.
                let each = ["map", "filter", "any", "all", "count", "sort_by", "group_runs"].contains(&m.as_str());
                let et = if each { each_elem(&self.cx.ty(r, env)) } else { Hole };
                for a in args.iter_mut() {
                    match a {
                        Expr::Lambda(ps, body) => {
                            let mut inner = env.clone();
                            bind_params(ps, &et, &mut inner);
                            self.expr(body, &mut inner);
                        }
                        a => self.expr(a, env),
                    }
                }
            }
            Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| self.expr(x, env)),
            Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
                self.expr(k, env);
                self.expr(v, env);
            }),
            Expr::Unary(_, a) | Expr::Try(a) | Expr::Field(a, _) | Expr::TupleField(a, _) => self.expr(a, env),
            Expr::Index(a, b) | Expr::Range(a, b) => {
                self.expr(a, env);
                self.expr(b, env);
            }
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.expr(x, env);
                    }
                }
            }
            Expr::Lambda(_, b) => self.expr(b, &mut env.clone()),
            _ => {}
        }
    }
}

// A value that may be None and may be an empty list literal: Brevi keeps None for a list as an empty
// one, so the two would look the same. Only values that hold both forms themselves are caught.
fn none_or_empty(d: &str) -> bool {
    d.contains("Call(\"none\", []") && (d.contains("List([])") || d.contains("Map([])"))
}

// The text of each `return` value in a function body's debug text.
fn returned_texts(d: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = d;
    while let Some(i) = rest.find("Return { value: Some(") {
        let s = &rest[i + "Return { value: Some(".len()..];
        let mut depth = 1;
        let mut end = s.len();
        for (j, c) in s.char_indices() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = j;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push(&s[..end]);
        rest = &s[end..];
    }
    out
}

// Checks every function and fixes the forms that have one obvious meaning.
pub fn check(items: &mut [Item], py: bool) -> Vec<crate::diag::Diag> {
    let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: HashSet::new(), genv: Env::new(), param_look: false, unsure: false, entry_look: false };
    for it in items.iter() {
        match it {
            Item::Fn { name, params, ret, .. } => {
                cx.fns.insert(name.clone(), (params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect(), from_ast(ret)));
            }
            Item::Type { name, fields, .. } => {
                cx.structs.insert(name.clone(), fields.iter().map(|f| (f.name.clone(), from_ast(&f.ty))).collect());
            }
            Item::Enum { name, variants, .. } => {
                for v in variants {
                    cx.variants.insert(v.name.clone(), name.clone());
                }
            }
            _ => {}
        }
    }
    // Functions that may return None.
    let none_fns: HashSet<String> = items
        .iter()
        .filter_map(|it| match it {
            Item::Fn { name, body, .. } if none_or_empty(&returned_texts(&format!("{body:?}")).join(" ")) => Some(name.clone()),
            _ => None,
        })
        .collect();
    // Names set to None in one place and to an empty list or dict in another (`players = None`, later
    // `players = {}`): Brevi keeps None for them as the empty value, so `is None` can't tell them apart.
    let none_or_empty_names: HashSet<String> = {
        let (mut nones, mut empties_set) = (HashSet::new(), HashSet::new());
        for it in items.iter() {
            let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
            walk(body, &mut |e| {
                if let Expr::Block(stmts, _) = e {
                    for st in stmts {
                        if let Stmt::Let { name: Pat::Bind(n), value, .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } = st {
                            match value {
                                Expr::Call(f, a, _) if f == "none" && a.is_empty() => {
                                    nones.insert(n.clone());
                                }
                                Expr::List(x) if x.is_empty() => {
                                    empties_set.insert(n.clone());
                                }
                                Expr::Map(x) if x.is_empty() => {
                                    empties_set.insert(n.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        }
        nones.intersection(&empties_set).cloned().collect()
    };
    // Top-level names, as functions see them (`for p in postings: s += p[1]` with postings a global list of pairs).
    let ecx = empty_cx(items);
    cx.genv = ecx.genv;
    cx.mixed = ecx.mixed;
    let empties = ecx.hint_t;
    let mut errs = Vec::new();
    for it in items.iter_mut() {
        let (params, body, line): (&[Param], &mut Expr, usize) = match it {
            Item::Fn { params, body, line, .. } => (params.as_slice(), body, *line),
            Item::Test { body, line, .. } => (&[][..], body, *line),
            _ => continue,
        };
        let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
        let mut ck = Ck { cx: &cx, line, errs: vec![], set_at: HashMap::new(), empties: &empties, py, next_own: params.iter().map(|p| p.name.clone()).collect(), maybe_none: none_or_empty_names.clone(), none_fns: &none_fns };
        if !matches!(body, Expr::Block(..)) {
            ck.next_own.clear();
        }
        ck.expr(body, &mut env);
        errs.extend(ck.errs);
    }
    errs.sort_by_key(|d| d.line);
    errs
}

// Python passes lists, maps and objects by reference, so models leave out `mut` on a parameter they
// change in place (`fn add(lots, x): lots.push(x)`). Such a parameter becomes `mut`: changed in place
// (push, pop, insert, remove, `p[k] = v`, `p.f = v`, `p[k] += 1`) or passed on to a `mut` parameter,
// and never set as a whole (`p = ...`, which in Python makes a new local).
pub fn auto_mut(items: &mut [Item]) {
    loop {
        let muts: HashMap<String, Vec<bool>> = items
            .iter()
            .filter_map(|i| match i {
                Item::Fn { name, params, .. } => Some((name.clone(), params.iter().map(|p| p.is_mut).collect())),
                _ => None,
            })
            .collect();
        let mut changed = false;
        for it in items.iter_mut() {
            let Item::Fn { params, body, script: false, .. } = it else { continue };
            for p in params.iter_mut() {
                // `r[ci] = v; return r` changes a copy and hands it back, so it stays a copy.
                if !p.is_mut && in_place(body, &p.name, &muts) && !crate::emit::assigns_to(body, &p.name) && !rebinds(body, &p.name) && !returns_name(body, &p.name) {
                    p.is_mut = true;
                    changed = true;
                }
            }
        }
        if !changed {
            return;
        }
    }
}

fn returns_name(body: &Expr, n: &str) -> bool {
    let is_n = |e: &Expr| matches!(e, Expr::Ident(m) | Expr::Move(m) if m == n);
    let mut hit = matches!(body, Expr::Block(_, Some(t)) if is_n(t));
    walk(body, &mut |x| {
        if let Expr::Block(stmts, _) = x {
            if stmts.iter().any(|st| matches!(st, Stmt::Return { value: Some(v), .. } if is_n(v))) {
                hit = true;
            }
        }
    });
    hit
}

fn root_is(e: &Expr, n: &str) -> bool {
    match e {
        Expr::Ident(m) | Expr::Move(m) => m == n,
        Expr::Index(b, _) | Expr::Field(b, _) | Expr::TupleField(b, _) => root_is(b, n),
        _ => false,
    }
}

// `p = ...` or `p += ...` anywhere, also as a `for p in` / pattern binding.
fn rebinds(e: &Expr, n: &str) -> bool {
    let mut hit = false;
    walk(e, &mut |x| {
        if let Expr::Block(stmts, _) = x {
            for st in stmts {
                match st {
                    Stmt::Assign { target: Expr::Ident(m), .. } if m == n => hit = true,
                    Stmt::Let { name: Pat::Bind(m), .. } if m == n => hit = true,
                    _ => {}
                }
            }
        }
        if let Expr::For(Pat::Bind(m), _, _) = x {
            if m == n {
                hit = true;
            }
        }
    });
    hit
}

fn in_place(e: &Expr, n: &str, muts: &HashMap<String, Vec<bool>>) -> bool {
    let mut hit = false;
    walk(e, &mut |x| match x {
        Expr::Block(stmts, _) => {
            for st in stmts {
                if let Stmt::Assign { target, .. } = st {
                    if root_is(target, n) && !matches!(target, Expr::Ident(_) | Expr::Move(_)) {
                        hit = true;
                    }
                }
            }
        }
        Expr::Method(recv, m, _, _) if ["push", "pop", "insert", "remove", "clear", "heap_push", "heap_pop", "heap_push_pop", "heap_replace", "heapify", "pop_at", "pop_item", "update", "move_to_end"].contains(&m.as_str()) && root_is(recv, n) => hit = true,
        // `for b in self.backs: b[2] -= 1` changes the items of n's list in place.
        Expr::For(p, it, body) => {
            let mut src = it.as_ref();
            while let Expr::Method(r, m, a, _) = src {
                if a.is_empty() && matches!(m.as_str(), "values" | "iter") {
                    src = r;
                } else {
                    break;
                }
            }
            let items: Vec<String> = match p {
                Pat::Bind(a) => vec![a.clone()],
                Pat::Tuple(ps) => ps.iter().filter_map(|q| if let Pat::Bind(a) = q { Some(a.clone()) } else { None }).collect(),
                _ => vec![],
            };
            if root_is(src, n) && !matches!(src, Expr::Ident(_)) && items.iter().any(|a| in_place(body, a, muts)) {
                hit = true;
            }
        }
        Expr::Call(f, args, _) => {
            if let Some(ms) = muts.get(f) {
                for (k, (_, a)) in args.iter().enumerate() {
                    if ms.get(k).copied().unwrap_or(false) && matches!(a, Expr::Ident(m) | Expr::Move(m) if m == n) {
                        hit = true;
                    }
                }
            }
        }
        _ => {}
    });
    hit
}

// Calls f on e and every expression inside it (lambda bodies too).
pub(crate) fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match e {
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| walk(x, f)),
        Expr::Map(ps) => ps.iter().for_each(|(k, v)| {
            walk(k, f);
            walk(v, f);
        }),
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    walk(x, f);
                }
            }
        }
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) | Expr::Try(a) => walk(a, f),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => {
            walk(a, f);
            walk(b, f);
        }
        Expr::For(_, a, b) => {
            walk(a, f);
            walk(b, f);
        }
        Expr::Call(_, args, _) => args.iter().for_each(|(_, a)| walk(a, f)),
        Expr::Method(r, _, args, _) => {
            walk(r, f);
            args.iter().for_each(|a| walk(a, f));
        }
        Expr::If(c, a, b) => {
            walk(c, f);
            walk(a, f);
            if let Some(b) = b {
                walk(b, f);
            }
        }
        Expr::Match(s, arms) => {
            walk(s, f);
            arms.iter().for_each(|(_, a)| walk(a, f));
        }
        Expr::Block(stmts, tail) => {
            for st in stmts {
                match st {
                    Stmt::Let { value, .. } => walk(value, f),
                    Stmt::Assign { target, value, .. } => {
                        walk(target, f);
                        walk(value, f);
                    }
                    Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } | Stmt::Assert { cond: expr, .. } => walk(expr, f),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                walk(t, f);
            }
        }
        _ => {}
    }
}

fn pat_names(p: &Pat) -> Vec<String> {
    let mut out = vec![];
    crate::parser::pat_names(p, &mut out);
    out
}

fn scalar(t: &T) -> bool {
    matches!(t, Int | F64 | Str | Bool | Json)
}

// Set for a Python-style program (see main.rs): some Python forms are typed the Python way.
pub static PY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn py_on() -> bool {
    PY.load(std::sync::atomic::Ordering::Relaxed)
}

// Every unknown part of a type made a JSON value.
fn holes_json(t: T) -> T {
    match t {
        Hole => Json,
        List(e) => List(Box::new(holes_json(*e))),
        Map(k, v) => Map(Box::new(holes_json(*k)), Box::new(holes_json(*v))),
        Tuple(ts) => Tuple(ts.into_iter().map(holes_json).collect()),
        other => other,
    }
}

thread_local! {
    // Per tuple type: (rows made from mixed lists, tuples seen, which counts the rows' own tuples too).
    static ROW_TUPLES: std::cell::RefCell<HashMap<String, (usize, usize)>> = std::cell::RefCell::new(HashMap::new());
}
pub fn rows_reset() {
    ROW_TUPLES.with(|r| r.borrow_mut().clear());
}
// A tuple type held only by mixed-list rows: in Rust a shared row (AxRow).
fn is_row(t: &T) -> bool {
    // When any row type clashes, every row stays a plain tuple (main.rs drops all the ax_row calls).
    !rows_clash() && ROW_TUPLES.with(|r| r.borrow().get(&format!("{t:?}")).is_some_and(|(rows, all)| *rows > 0 && all <= rows))
}
pub fn is_row_ast(t: &Ty) -> bool {
    is_row(&from_ast(t))
}
// Some tuple type is used both for mixed-list rows and for real tuples.
pub fn rows_clash() -> bool {
    ROW_TUPLES.with(|r| r.borrow().values().any(|(rows, all)| *rows > 0 && all > rows))
}

fn mixed_record(xs: &[Expr], ty: &dyn Fn(&Expr) -> T) -> bool {
    if !py_on() || !(2..=12).contains(&xs.len()) {
        return false;
    }
    let ts: Vec<T> = xs.iter().map(ty).collect();
    ts.iter().all(|t| matches!(t, Int | F64 | Str | Bool | Dec | Date)) && ts.iter().any(|t| clash(t, &ts[0]) && !matches!((t, &ts[0]), (Int, F64) | (F64, Int)))
}

fn rec_key(i: &Expr) -> Option<String> {
    match i {
        Expr::Str(parts) => match parts.as_slice() {
            [StrPart::Lit(k)] => Some(crate::records::field_of(k)),
            _ => None,
        },
        _ => None,
    }
}

// `tx['posts']`, `tx.get('posts')`, `tx.get('posts', [])` and `'posts' in tx` on a record.
fn rec_field(e: &mut Expr, ty: &dyn Fn(&Expr) -> T, structs: &HashMap<String, Vec<(String, T)>>) {
    let has = |b: &Expr, k: &str| -> Option<bool> {
        match ty(b) {
            // A key only some of the dicts have: not known from the type.
            Named(r) if crate::records::is_record(&r) && crate::records::is_optional(&r, k) => None,
            Named(r) if crate::records::is_record(&r) => Some(structs.get(&r).is_some_and(|fs| fs.iter().any(|(n, _)| n == k))),
            _ => None,
        }
    };
    let field = |b: &Expr, k: &str| matches!(ty(b), Named(r) if crate::records::is_record(&r) && structs.get(&r).is_some_and(|fs| fs.iter().any(|(n, _)| n == k)));
    let new = match &*e {
        Expr::Index(b, i) => rec_key(i).and_then(|k| (field(b, &k)).then(|| Expr::Field(b.clone(), k))),
        Expr::Method(b, m, args, _) if matches!(m.as_str(), "get" | "get_or") && (1..=2).contains(&args.len()) => {
            rec_key(&args[0]).and_then(|k| match has(b, &k) {
                Some(true) => Some(Expr::Field(b.clone(), k)),
                Some(false) => args.get(1).cloned(),
                None => None,
            })
        }
        Expr::Binary(op @ ("in" | "not in"), a, b) => rec_key(a).and_then(|k| has(b, &k).map(|h| Expr::Bool(h == (*op == "in")))),
        _ => None,
    };
    if let Some(n) = new {
        *e = n;
    }
}

// A top-level name still unknown where it is read (`cur = None` above) has the type functions gave it.
fn global_too(t: T, x: &Expr, genv: &Env) -> T {
    match (&t, x) {
        (Hole, Expr::Ident(n)) => genv.get(n).cloned().unwrap_or(Hole),
        _ => t,
    }
}
