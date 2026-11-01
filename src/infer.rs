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
            "dec" => Dec,
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
        _ => Hole,
    }
}

// What map, filter, any and all pass to their lambda: a map's keys, as Python's `[k for k in d]` reads them.
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

fn empty_cx(items: &[Item]) -> Cx<'_> {
    let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: HashSet::new(), genv: Env::new(), param_look: false };
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
                let t = elem(&self.ty(it, env));
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
                                        let et = if let Tuple(ts) = &t { ts.get(k).cloned().unwrap_or(Hole) } else { Hole };
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
                let each = ["map", "filter", "any", "all", "count", "sort_by"].contains(&m.as_str());
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
                        (Expr::Index(b, k), _) if matches!(b.as_ref(), Expr::Ident(m) if m == n) => {
                            let kt = self.ty(k, env);
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
                            self.look(m, &stmts[i + 1..], tail, &mut later, &mut mt);
                            if let Some(tl) = tail {
                                self.look_expr(m, tl, &mut later, &mut mt);
                            }
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
                                        let et = if let Tuple(ts) = &vt { ts.get(k).cloned().unwrap_or(Hole) } else { Hole };
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
                        ("push", List(_) | Hole) => List(Box::new(a0)),
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
                let t = elem(&self.ty(it, env));
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
            Expr::Ident(n) | Expr::Move(n) => env.get(n).or_else(|| self.genv.get(n)).cloned().unwrap_or(Hole),
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
                    "csv" => List(Box::new(List(Box::new(Str)))),
                    "json" => Json,
                    "date" => Date,
                    "abs" => a0(),
                    "min" | "max" => args.iter().fold(Hole, |t, (_, a)| merge(t, self.ty(a, env))),
                    "heap" => Heap(Box::new(Hole)),
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
            Expr::Index(b, _) => match self.ty(b, env) {
                List(e) => *e,
                Map(_, v) => *v,
                Str => Str,
                Json => Json,
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
        match m {
            "len" | "count" | "find" | "find_from" | "index_of" | "digit" | "code" | "int" | "days" | "day" | "month" | "year" | "weekday" => Int,
            "float" | "sqrt" | "floor" | "ceil" => F64,
            "dec" => Dec,
            "big" => Big,
            "char" | "fmt" | "spec" | "trim" | "trim_of" | "trim_end" | "trim_end_of" | "trim_start" | "trim_start_of" | "lower" | "upper" | "replace" | "re_sub" | "text" => Str,
            "list" => List(Box::new(Json)),
            "repeat" | "slice" | "rev" if matches!(r, Str) => Str,
            "join" => Str,
            "words" | "lines" | "chars" | "split" | "re_full" | "re_find" | "re_split" | "re_list" => List(Box::new(Str)),
            "partition" => Tuple(vec![Str, Str, Str]),
            "re_all" => List(Box::new(List(Box::new(Str)))),
            "ints" => List(Box::new(Int)),
            "json" => Json,
            "is_null" | "is_none" | "bool" | "is_empty" | "is_digit" | "is_alpha" | "is_space" | "is_upper" | "is_lower" | "is_alnum" | "has" | "contains" | "any" | "all" | "starts_with" | "ends_with" => Bool,
            "first" | "last" | "pop" | "min" | "max" | "sum" | "heap_pop" | "heap_push_pop" | "heap_replace" => match &r {
                Map(_, v) if m == "pop" => *v.clone(),
                _ => elem(&r),
            },
            "remove" => match &r {
                Map(_, v) => *v.clone(),
                _ => elem(&r),
            },
            "slice" | "sort" | "sort_by" | "rev" | "take" | "skip" | "filter" | "unique" | "step" | "repeat" | "abs" | "round" => match &r {
                Hole if m == "repeat" || m == "slice" => Hole,
                Map(k, _) if m == "filter" => List(k.clone()),
                _ => r,
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
            "fold" => merge(a(0), Hole),
            "add_days" => Date,
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
                let et = if let Tuple(ts) = &t { ts.get(k).cloned().unwrap_or(Hole) } else { Hole };
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
        Expr::Method(_, m, _, _) => ["push", "insert", "remove", "pop", "heap_push", "heap_pop", "heapify"].contains(&m.as_str()),
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
                            self.look(n, &stmts[i + 1..], tail.as_deref(), &mut later, &mut mt);
                            if let Some(tl) = tail {
                                self.look_expr(n, tl, &mut later, &mut mt);
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
                            }
                        }
                        Stmt::Assign { target: Expr::Tuple(xs), op: "=", value, .. } => {
                            let t = self.ty(value, env);
                            for (k, x) in xs.iter().enumerate() {
                                if let (Expr::Ident(n), Tuple(ts)) = (x, &t) {
                                    env.entry(n.clone()).or_insert(ts.get(k).cloned().unwrap_or(Hole));
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
                let t = elem(&self.ty(it, env));
                bind(p, t, env);
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
pub fn returns(items: &mut [Item]) -> Result<(), Vec<crate::diag::Diag>> {
    let q = |t: &Ty| matches!(t, Ty::Named(n) if n == "?");
    let open = |items: &[Item]| items.iter().any(|i| matches!(i, Item::Fn { ret, params, .. } if q(ret) || params.iter().any(|p| q(&p.ty))));
    if !open(items) {
        return Ok(());
    }
    // Map literals whose values differ in type are Json objects, also as return values.
    let (_, mixed) = empty_types(items);
    for round in 0..6 {
        // Top-level names (`ts = {}` filled inside a function) are known by type where functions read them.
        let genv = empty_cx(items).genv;
        let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: mixed.clone(), genv, param_look: false };
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
        let last = round == 5;
        let mut errs = Vec::new();
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
            for (k, it) in items.iter().enumerate() {
                let Item::Fn { params, body, .. } = it else { continue };
                for (pi, p) in params.iter().enumerate() {
                    // Calls are the better evidence, so this waits until they have had two rounds.
                    if round < 2 || !q(&p.ty) || fills.iter().any(|f| f.0 == k && f.1 == pi) {
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
            for (k, pi, t) in fills {
                if let Item::Fn { params, .. } = &mut items[k] {
                    if q(&params[pi].ty) {
                        params[pi].ty = t;
                    }
                }
            }
            if last {
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
        let mut found = Vec::new();
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
            let t = ts.into_iter().fold(Hole, merge);
            match to_ast(&t) {
                Some(t) => found.push((k, t)),
                // No `return` and a last line of unclear type (`if c: print(x) else: m.remove(k)`): no value, as in Python.
                None if last && !has_return && matches!(tail, Some(Expr::If(..) | Expr::Match(..))) => found.push((k, Ty::Unit)),
                None if last => {
                    let mut d = crate::diag::Diag::new("check", *line, 1, &format!("cannot work out what {name} returns"));
                    d.fix = Some(format!("write the return type: fn {name}(...) -> TYPE"));
                    errs.push(d);
                }
                None => {}
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
}

impl Ck<'_, '_> {
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
        for st in stmts.iter_mut() {
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
                    }
                    bind(name, t, env);
                }
                Stmt::Assign { target, op, value, line } => {
                    self.line = *line;
                    self.expr(value, env);
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
                                        let et = if let Tuple(ts) = &t { ts.get(k).cloned().unwrap_or(Hole) } else { Hole };
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
        // t[0] on a tuple is t.0, and xs.0 on a list is xs[0].
        let fix = match &*e {
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
                let mut inner = env.clone();
                bind(p, elem(&self.cx.ty(it, env)), &mut inner);
                self.expr(body, &mut inner);
            }
            Expr::While(c, body) => {
                self.expr(c, env);
                self.expr(body, &mut env.clone());
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
                // Only these methods pass each item to their lambda.
                let each = ["map", "filter", "any", "all", "count", "sort_by"].contains(&m.as_str());
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

// Checks every function and fixes the forms that have one obvious meaning.
pub fn check(items: &mut [Item]) -> Vec<crate::diag::Diag> {
    let mut cx = Cx { fns: HashMap::new(), structs: HashMap::new(), variants: HashMap::new(), others: vec![], ret: Hole, hints: HashMap::new(), known_hints: HashMap::new(), hint_t: HashMap::new(), mixed: HashSet::new(), genv: Env::new(), param_look: false };
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
    let empties = empty_cx(items).hint_t;
    let mut errs = Vec::new();
    for it in items.iter_mut() {
        let (params, body, line): (&[Param], &mut Expr, usize) = match it {
            Item::Fn { params, body, line, .. } => (params.as_slice(), body, *line),
            Item::Test { body, line, .. } => (&[][..], body, *line),
            _ => continue,
        };
        let mut env: Env = params.iter().map(|p| (p.name.clone(), from_ast(&p.ty))).collect();
        let mut ck = Ck { cx: &cx, line, errs: vec![], set_at: HashMap::new(), empties: &empties };
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
        Expr::Method(recv, m, _, _) if ["push", "pop", "insert", "remove"].contains(&m.as_str()) && root_is(recv, n) => hit = true,
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
fn walk(e: &Expr, f: &mut dyn FnMut(&Expr)) {
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
