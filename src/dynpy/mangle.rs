// Private name mangling, as CPython's compiler does it (Python/compile.c _Py_Mangle): inside a class body, and
// in every function nested in it, an identifier `__x` (two leading underscores, not ending in two) is
// `_Class__x`, where Class is the innermost enclosing class's name without its leading underscores. That covers
// names (loads, stores, deletes, global/nonlocal), attribute names, parameter names, def and class names bound
// in the body, import bindings and `except ... as` names. Keyword argument names in calls and strings are left
// as written, as CPython leaves them. A def or class whose name is mangled keeps its own name in `disp`
// (its __name__ and __qualname__ stay unmangled, as in CPython). Runs once on every parsed module, so the
// names the emitter and the runtime see are the ones CPython's class namespaces and instance dicts hold.
use super::parse::*;

type PR<T> = Result<T, (usize, String)>;

// the mangled form of n inside class cls, or None when n is not a private name there
pub fn mangled(cls: &str, n: &str) -> Option<String> {
    if !n.starts_with("__") || n.ends_with("__") || n.contains('.') {
        return None;
    }
    // the parser's and emitter's own hidden names never change
    if n.starts_with("__brevi_") || n.starts_with("__bv_") || n == "__bytes_lit" {
        return None;
    }
    let c = cls.trim_start_matches('_');
    if c.is_empty() {
        return None;
    }
    Some(format!("_{c}{n}"))
}

fn m(cls: Option<&str>, n: &mut String) {
    if let Some(c) = cls {
        if let Some(x) = mangled(c, n) {
            *n = x;
        }
    }
}

pub fn module(body: &mut [Stmt]) -> PR<()> {
    block(body, None)
}

fn block(body: &mut [Stmt], cls: Option<&str>) -> PR<()> {
    for s in body.iter_mut() {
        stmt(s, cls)?;
    }
    Ok(())
}

fn params(p: &mut Params, cls: Option<&str>) {
    for (n, d) in p.pos.iter_mut().chain(p.kwonly.iter_mut()) {
        m(cls, n);
        if let Some(d) = d {
            ex(d, cls);
        }
    }
    if let Some(n) = &mut p.star {
        m(cls, n);
    }
    if let Some(n) = &mut p.kwstar {
        m(cls, n);
    }
    for (n, a) in p.ann.iter_mut() {
        m(cls, n);
        ex(a, cls);
    }
}

fn stmt(s: &mut Stmt, cls: Option<&str>) -> PR<()> {
    let line = s.line;
    match &mut s.st {
        St::Expr(e) => ex(e, cls),
        St::Assign(ts, v) => {
            for t in ts.iter_mut() {
                ex(t, cls);
            }
            ex(v, cls);
        }
        St::AugAssign(t, _, v) => {
            ex(t, cls);
            ex(v, cls);
        }
        St::AnnAssign(t, v, a, _) => {
            ex(t, cls);
            if let Some(v) = v {
                ex(v, cls);
            }
            ex(a, cls);
        }
        St::Pass | St::Break | St::Continue => {}
        St::Return(v) => {
            if let Some(v) = v {
                ex(v, cls);
            }
        }
        St::Raise(a, b) => {
            for x in [a, b].into_iter().flatten() {
                ex(x, cls);
            }
        }
        St::Global(ns) | St::Nonlocal(ns) => {
            for n in ns.iter_mut() {
                m(cls, n);
            }
        }
        St::Del(ts) => {
            for t in ts.iter_mut() {
                ex(t, cls);
            }
        }
        St::Assert(a, b) => {
            ex(a, cls);
            if let Some(b) = b {
                ex(b, cls);
            }
        }
        St::Import(names) => {
            for (n, alias) in names.iter_mut() {
                match alias {
                    Some(a) => m(cls, a),
                    None => {
                        let top = n.split('.').next().unwrap_or("").to_string();
                        if let Some(c) = cls {
                            if let Some(x) = mangled(c, &top) {
                                if n.contains('.') {
                                    return Err((line, format!("import {n} in a class body (a private module name bound mangled) is not supported in dynamic builds")));
                                }
                                *alias = Some(x);
                            }
                        }
                    }
                }
            }
        }
        St::ImportFrom(_, names) => {
            for (n, alias) in names.iter_mut() {
                match alias {
                    Some(a) => m(cls, a),
                    None => {
                        if let Some(c) = cls {
                            if let Some(x) = mangled(c, n) {
                                *alias = Some(x);
                            }
                        }
                    }
                }
            }
        }
        St::If(arms, els) => {
            for (c, b) in arms.iter_mut() {
                ex(c, cls);
                block(b, cls)?;
            }
            if let Some(b) = els {
                block(b, cls)?;
            }
        }
        St::While(c, b, e) => {
            ex(c, cls);
            block(b, cls)?;
            if let Some(e) = e {
                block(e, cls)?;
            }
        }
        St::For(t, it, b, e) => {
            ex(t, cls);
            ex(it, cls);
            block(b, cls)?;
            if let Some(e) = e {
                block(e, cls)?;
            }
        }
        St::Try(b, hs, e, f) => {
            block(b, cls)?;
            for h in hs.iter_mut() {
                if let Some(t) = &mut h.types {
                    ex(t, cls);
                }
                if let Some(n) = &mut h.name {
                    m(cls, n);
                }
                block(&mut h.body, cls)?;
            }
            if let Some(e) = e {
                block(e, cls)?;
            }
            if let Some(f) = f {
                block(f, cls)?;
            }
        }
        St::With(items, b) => {
            for (c, t) in items.iter_mut() {
                ex(c, cls);
                if let Some(t) = t {
                    ex(t, cls);
                }
            }
            block(b, cls)?;
        }
        St::Def(d) => {
            for x in d.decorators.iter_mut() {
                ex(x, cls);
            }
            if let Some(c) = cls {
                if let Some(x) = mangled(c, &d.name) {
                    d.disp = Some(std::mem::replace(&mut d.name, x));
                }
            }
            params(&mut d.params, cls);
            if let Some(r) = &mut d.ret {
                ex(r, cls);
            }
            block(&mut d.body, cls)?;
        }
        St::Class(c) => {
            for x in c.decorators.iter_mut().chain(c.bases.iter_mut()) {
                ex(x, cls);
            }
            for (_, v) in c.kwargs.iter_mut() {
                ex(v, cls);
            }
            c.static_attrs = static_attrs(&c.body);
            let own = c.name.clone();
            if let Some(k) = cls {
                if let Some(x) = mangled(k, &c.name) {
                    c.disp = Some(std::mem::replace(&mut c.name, x));
                }
            }
            block(&mut c.body, Some(&own))?;
        }
    }
    Ok(())
}

fn fparts(ps: &mut [FPart], cls: Option<&str>) {
    for p in ps.iter_mut() {
        if let FPart::Expr(e, _, spec, _) = p {
            ex(e, cls);
            fparts(spec, cls);
        }
    }
}

fn ex(e: &mut Ex, cls: Option<&str>) {
    if cls.is_none() {
        return;
    }
    match e {
        Ex::Name(n) => m(cls, n),
        Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False | Ex::Ellipsis => {}
        Ex::FStr(ps) => fparts(ps, cls),
        Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => {
            for x in xs.iter_mut() {
                ex(x, cls);
            }
        }
        Ex::Dict(kv) => {
            for (k, v) in kv.iter_mut() {
                if let Some(k) = k {
                    ex(k, cls);
                }
                ex(v, cls);
            }
        }
        Ex::Star(x) | Ex::Unary(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => ex(x, cls),
        Ex::Attr(o, n) => {
            ex(o, cls);
            m(cls, n);
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            ex(a, cls);
            ex(b, cls);
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                ex(x, cls);
            }
        }
        Ex::Call(f, args) => {
            ex(f, cls);
            for a in args.iter_mut() {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::KwStar(x) | Arg::Kw(_, x) => ex(x, cls),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            ex(a, cls);
            for (_, x) in rest.iter_mut() {
                ex(x, cls);
            }
        }
        Ex::IfExp(a, b, c) => {
            ex(a, cls);
            ex(b, cls);
            ex(c, cls);
        }
        Ex::Lambda(p, body) => {
            params(p, cls);
            ex(body, cls);
        }
        Ex::Comp(_, a, b, fors) => {
            ex(a, cls);
            if let Some(b) = b {
                ex(b, cls);
            }
            for f in fors.iter_mut() {
                ex(&mut f.target, cls);
                ex(&mut f.iter, cls);
                for i in f.ifs.iter_mut() {
                    ex(i, cls);
                }
            }
        }
        Ex::Walrus(n, v) => {
            m(cls, n);
            ex(v, cls);
        }
        Ex::Yield(v) => {
            if let Some(v) = v {
                ex(v, cls);
            }
        }
    }
}

// __static_attributes__ of a class with this body, as CPython 3.13's compiler collects it: every attribute
// stored on the name `self` (an assignment, an annotated assignment with a value, a for, with or comprehension
// target) in a scope whose nearest enclosing class, not counting the scope itself, is this class. So the body's
// functions (and classes nested in them) count, the class body's own statements and the functions of a nested
// class do not, and an augmented assignment does not (its store is no target).
pub fn static_attrs(body: &[Stmt]) -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    sa_block(body, Unit::Own, false, &mut out);
    out.into_iter().collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Own,
    OtherClass,
    Func,
}

// whether a scope opened inside `unit` (whose own flag is `flag`) has this class as its nearest class
fn inner_flag(unit: Unit, flag: bool) -> bool {
    match unit {
        Unit::Own => true,
        Unit::OtherClass => false,
        Unit::Func => flag,
    }
}

fn sa_target(t: &Ex, flag: bool, out: &mut std::collections::BTreeSet<String>) {
    match t {
        Ex::Attr(o, n) => {
            if flag && matches!(o.as_ref(), Ex::Name(s) if s == "self") {
                out.insert(n.clone());
            }
        }
        Ex::Tuple(xs) | Ex::List(xs) => {
            for x in xs {
                sa_target(x, flag, out);
            }
        }
        Ex::Star(x) => sa_target(x, flag, out),
        _ => {}
    }
}

fn sa_ex(e: &Ex, unit: Unit, flag: bool, out: &mut std::collections::BTreeSet<String>) {
    match e {
        Ex::Lambda(p, b) => {
            let f = inner_flag(unit, flag);
            for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
                if let Some(d) = d {
                    sa_ex(d, unit, flag, out);
                }
            }
            sa_ex(b, Unit::Func, f, out);
        }
        Ex::Comp(_, a, b, fors) => {
            for c in fors {
                sa_target(&c.target, flag, out);
                sa_ex(&c.iter, unit, flag, out);
                for i in &c.ifs {
                    sa_ex(i, unit, flag, out);
                }
            }
            sa_ex(a, unit, flag, out);
            if let Some(b) = b {
                sa_ex(b, unit, flag, out);
            }
        }
        Ex::FStr(ps) => {
            fn walk(ps: &[FPart], unit: Unit, flag: bool, out: &mut std::collections::BTreeSet<String>) {
                for p in ps {
                    if let FPart::Expr(e, _, spec, _) = p {
                        sa_ex(e, unit, flag, out);
                        walk(spec, unit, flag, out);
                    }
                }
            }
            walk(ps, unit, flag, out);
        }
        Ex::List(xs) | Ex::Tuple(xs) | Ex::Set(xs) => xs.iter().for_each(|x| sa_ex(x, unit, flag, out)),
        Ex::Dict(kv) => kv.iter().for_each(|(k, v)| {
            if let Some(k) = k {
                sa_ex(k, unit, flag, out);
            }
            sa_ex(v, unit, flag, out);
        }),
        Ex::Star(x) | Ex::Unary(_, x) | Ex::YieldFrom(x) | Ex::Await(x) | Ex::Attr(x, _) | Ex::Walrus(_, x) => sa_ex(x, unit, flag, out),
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            sa_ex(a, unit, flag, out);
            sa_ex(b, unit, flag, out);
        }
        Ex::Slice(a, b, c) => [a, b, c].into_iter().flatten().for_each(|x| sa_ex(x, unit, flag, out)),
        Ex::Call(f, args) => {
            sa_ex(f, unit, flag, out);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::KwStar(x) | Arg::Kw(_, x) => sa_ex(x, unit, flag, out),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            sa_ex(a, unit, flag, out);
            rest.iter().for_each(|(_, x)| sa_ex(x, unit, flag, out));
        }
        Ex::IfExp(a, b, c) => [a, b, c].into_iter().for_each(|x| sa_ex(x, unit, flag, out)),
        Ex::Yield(Some(x)) => sa_ex(x, unit, flag, out),
        _ => {}
    }
}

fn sa_block(body: &[Stmt], unit: Unit, flag: bool, out: &mut std::collections::BTreeSet<String>) {
    for s in body {
        match &s.st {
            St::Expr(e) => sa_ex(e, unit, flag, out),
            St::Assign(ts, v) => {
                sa_ex(v, unit, flag, out);
                for t in ts {
                    sa_target(t, flag, out);
                    sa_ex(t, unit, flag, out);
                }
            }
            St::AugAssign(t, _, v) => {
                sa_ex(t, unit, flag, out);
                sa_ex(v, unit, flag, out);
            }
            St::AnnAssign(t, v, a, _) => {
                if let Some(v) = v {
                    sa_ex(v, unit, flag, out);
                    sa_target(t, flag, out);
                }
                sa_ex(t, unit, flag, out);
                sa_ex(a, unit, flag, out);
            }
            St::Return(Some(v)) => sa_ex(v, unit, flag, out),
            St::Raise(a, b) => [a, b].into_iter().flatten().for_each(|x| sa_ex(x, unit, flag, out)),
            St::Del(ts) => ts.iter().for_each(|x| sa_ex(x, unit, flag, out)),
            St::Assert(a, b) => {
                sa_ex(a, unit, flag, out);
                if let Some(b) = b {
                    sa_ex(b, unit, flag, out);
                }
            }
            St::If(arms, els) => {
                for (c, b) in arms {
                    sa_ex(c, unit, flag, out);
                    sa_block(b, unit, flag, out);
                }
                if let Some(b) = els {
                    sa_block(b, unit, flag, out);
                }
            }
            St::While(c, b, e) => {
                sa_ex(c, unit, flag, out);
                sa_block(b, unit, flag, out);
                if let Some(e) = e {
                    sa_block(e, unit, flag, out);
                }
            }
            St::For(t, it, b, e) => {
                sa_target(t, flag, out);
                sa_ex(t, unit, flag, out);
                sa_ex(it, unit, flag, out);
                sa_block(b, unit, flag, out);
                if let Some(e) = e {
                    sa_block(e, unit, flag, out);
                }
            }
            St::Try(b, hs, e, f) => {
                sa_block(b, unit, flag, out);
                for h in hs {
                    if let Some(t) = &h.types {
                        sa_ex(t, unit, flag, out);
                    }
                    sa_block(&h.body, unit, flag, out);
                }
                for x in [e, f].into_iter().flatten() {
                    sa_block(x, unit, flag, out);
                }
            }
            St::With(items, b) => {
                for (c, t) in items {
                    sa_ex(c, unit, flag, out);
                    if let Some(t) = t {
                        sa_target(t, flag, out);
                        sa_ex(t, unit, flag, out);
                    }
                }
                sa_block(b, unit, flag, out);
            }
            St::Def(d) => {
                for x in &d.decorators {
                    sa_ex(x, unit, flag, out);
                }
                for (_, dv) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                    if let Some(dv) = dv {
                        sa_ex(dv, unit, flag, out);
                    }
                }
                sa_block(&d.body, Unit::Func, inner_flag(unit, flag), out);
            }
            St::Class(c) => {
                for x in c.decorators.iter().chain(c.bases.iter()) {
                    sa_ex(x, unit, flag, out);
                }
                sa_block(&c.body, Unit::OtherClass, inner_flag(unit, flag), out);
            }
            _ => {}
        }
    }
}
