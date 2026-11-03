// Python the typed build would compile to a different answer, found on the Python syntax tree before the
// typed build runs. Such a file goes straight to the dynamic build (src/dynpy), which keeps CPython's
// behaviour, instead of being built typed and printing something else.
//
// - Sets: the typed build keeps a set as a list in sorted order, while CPython walks a set in its hash
//   table's order (`{-1, 5}` prints `{5, -1}`), and prints it in braces even inside a list or dict. Any
//   use where that order or form shows (printing, str(), iterating, list(s), s.pop(), joining) is refused.
//   Order-free uses stay typed: `x in s`, len, sorted, min, max, sum, any, all and set algebra.
// - print(q, q.popleft()): CPython reads every argument before printing; the typed build writes each one
//   as it goes, so a later argument that changes an earlier one shows the old value.
use crate::dynpy::parse::{Arg, CompFor, Ex, FPart, St, Stmt};
use std::collections::HashSet;

#[derive(Default)]
struct G {
    sets: HashSet<String>,       // names (and `.attr` names) holding a set
    holders: HashSet<String>,    // names holding sets inside a list, dict or tuple
    set_fns: HashSet<String>,    // functions returning a set
    holder_fns: HashSet<String>, // functions returning a container of sets
    params: Vec<(String, Vec<String>)>,
    matches: HashSet<String>,    // names holding a re match (a Match object or None)
    dicts: HashSet<String>,      // names holding a dict
}

// Why the file needs the dynamic build, with the line: None when the typed build gives CPython's answer.
pub fn check(src: &str) -> Option<(usize, String)> {
    let body = crate::dynpy::parse::parse(src).ok()?;
    let mut g = G::default();
    collect_params(&body, &mut g);
    for _ in 0..8 {
        let before = (g.sets.len(), g.holders.len(), g.set_fns.len(), g.holder_fns.len());
        learn(&body, None, &mut g);
        if before == (g.sets.len(), g.holders.len(), g.set_fns.len(), g.holder_fns.len()) {
            break;
        }
    }
    let mut out = None;
    uses(&body, &g, &mut out);
    out
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

fn each_block(st: &St, f: &mut dyn FnMut(&[Stmt])) {
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

// re.match(...), pat.search(...): the typed build keeps a match as its list of groups, so printing one
// shows `[]` or `['12']` where CPython shows None or `<re.Match object; ...>`.
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
        Ex::Index(b, _) => {
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
            St::AnnAssign(t, Some(v)) => bind(t, v, g),
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

fn walk_st(st: &St, f: &mut dyn FnMut(&Ex)) {
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
        St::AnnAssign(t, v) => {
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

fn walk_ex(e: &Ex, f: &mut dyn FnMut(&Ex)) {
    f(e);
    let mut go = |x: &Ex| walk_ex(x, f);
    match e {
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(&mut go),
        Ex::Dict(items) => items.iter().for_each(|(k, v)| {
            if let Some(k) = k {
                go(k);
            }
            go(v);
        }),
        Ex::Star(x) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) => go(x),
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
            for CompFor { target, iter, ifs } in fors {
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

fn uses(body: &[Stmt], g: &G, out: &mut Option<(usize, String)>) {
    for (k, s) in body.iter().enumerate() {
        if out.is_some() {
            return;
        }
        let line = s.line;
        let mut hit: Option<String> = None;
        // `book = asks if buy else bids`, then `book.remove(o)`: in Python book is one of the two lists, so
        // the change shows in it; the typed build changes a copy.
        if let St::Assign(ts, Ex::IfExp(_, a, b)) = &s.st {
            if let ([Ex::Name(x)], true) = (ts.as_slice(), matches!(a.as_ref(), Ex::Name(_) | Ex::Attr(..)) && matches!(b.as_ref(), Ex::Name(_) | Ex::Attr(..))) {
                if body[k + 1..].iter().any(|st| changes_name(&st.st, x)) {
                    hit = Some(format!("{x} is one of two lists picked by a condition and is changed later; Python changes that list itself"));
                }
            }
        }
        if let St::For(_, it, _, _) = &s.st {
            if is_set(it, g) {
                hit = Some("a loop over a set walks it in CPython's hash order".into());
            }
        }
        // The typed build stops at these errors (a Rust panic) instead of raising them where an except can
        // catch them; only errors from failing calls (int("x"): ValueError) are caught there.
        if let St::Try(_, hs, _, _) = &s.st {
            const UNCAUGHT: &[&str] = &["ZeroDivisionError", "ArithmeticError", "AssertionError", "EOFError", "IndexError", "KeyError", "LookupError", "OverflowError", "RecursionError"];
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
        if let St::For(t, it, body, _) = &s.st {
            if hit.is_none() && grows_while_walked(t, it, body, g) {
                hit = Some("changing a dict or set while a loop walks it stops CPython with a RuntimeError".into());
            }
        }
        walk_st(&s.st, &mut |e| {
            if hit.is_some() {
                return;
            }
            hit = shown(e, g);
        });
        if let Some(why) = hit {
            *out = Some((line, why));
            return;
        }
        match &s.st {
            St::Def(d) => uses(&d.body, g, out),
            St::Class(c) => uses(&c.body, g, out),
            st => each_block(st, &mut |b| uses(b, g, out)),
        }
    }
}

// An order-dependent use of a set at this expression (not looking inside it: walk_ex does that).
fn shown(e: &Ex, g: &G) -> Option<String> {
    let set_or_holder = |x: &Ex| is_set(x, g) || is_holder(x, g);
    // `m.group(3) is None`: a group that took no part in the match is None in Python; the typed build
    // reads it as "".
    if let Ex::Cmp(l, ops) = e {
        let group = |x: &Ex| matches!(x, Ex::Call(f, _) if matches!(f.as_ref(), Ex::Attr(_, m) if m == "group"));
        let mut sides = vec![l.as_ref()];
        sides.extend(ops.iter().map(|(_, x)| x));
        if ops.iter().any(|(op, _)| matches!(*op, "is" | "is not" | "==" | "!=")) && sides.iter().any(|x| group(x)) && sides.iter().any(|x| matches!(x, Ex::NoneV)) {
            return Some("a re group compared with None (None when the group did not match)".into());
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
                    if n == "print" {
                        return print_changes(&pos);
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
        Ex::FStr(parts) => {
            let mut bad = false;
            let mut m = false;
            fparts(parts, &mut |x| {
                bad |= set_or_holder(x);
                m |= is_match(x, g);
            });
            if m {
                return Some("an f-string of a re match shows a Match object or None".into());
            }
            bad.then(|| "an f-string of a set shows CPython's hash order".to_string())
        }
        Ex::Bin("%", l, r) if matches!(l.as_ref(), Ex::Str(_)) && (set_or_holder(r) || matches!(r.as_ref(), Ex::Tuple(v) if v.iter().any(|x| set_or_holder(x)))) => Some("% of a set shows CPython's hash order".into()),
        Ex::Comp(k, _, _, fors) if *k != 's' && fors.iter().any(|c| is_set(&c.iter, g)) => Some("a comprehension over a set walks it in CPython's hash order".into()),
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

// print(q, q.popleft()): an argument changes a name another argument shows.
fn print_changes(args: &[&Ex]) -> Option<String> {
    const CHANGING: &[&str] = &["pop", "popleft", "popitem", "append", "appendleft", "add", "remove", "insert", "extend", "update", "setdefault", "clear", "sort", "reverse", "discard", "rotate", "heappop", "heappush"];
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
                    Ex::Attr(_, m) | Ex::Name(m) if m == "heappop" || m == "heappush" => {
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

#[cfg(test)]
mod tests {
    use super::check;
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
    }
    #[test]
    fn order_free_set_use_stays_typed() {
        assert!(!needs("s = set()\ns.add(5)\nprint(5 in s, len(s), sorted(s), max(s), sum(s))\n"));
        assert!(!needs("seen = set()\nfor i in range(9):\n    if i % 3 not in seen:\n        seen.add(i % 3)\nprint(len(seen))\n"));
        assert!(!needs("xs = [3, 1]\nprint(xs, sorted(set(xs)))\n"));
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
        assert!(needs("try:\n    x = [1][3]\nexcept IndexError:\n    pass\n"));
        assert!(!needs("try:\n    x = int('q')\nexcept ValueError:\n    pass\n"));
        assert!(needs("d = {1: 2}\nfor k in d:\n    d[k + 1] = 0\n"));
        assert!(needs("import copy\nb = copy.copy([[1]])\n"));
        assert!(needs("a = []\nb = []\nbook = a if len(a) else b\nbook.append(1)\nprint(b)\n"));
        assert!(needs("a = [1]\nb = []\ndef f(c):\n    book = a if c else b\n    if book:\n        book.remove(1)\n"));
        assert!(!needs("a = [1]\nb = [2]\nbook = a if len(a) else b\nprint(len(book))\n"));
        assert!(needs("import re\nm = re.match('(a)(b)?', 'a')\nprint(m.group(2) is None)\n"));
        assert!(!needs("d = {1: 2}\nfor k in d:\n    d[k] = 0\nxs = [1]\nfor x in xs:\n    xs[0] = 2\n"));
    }
}
