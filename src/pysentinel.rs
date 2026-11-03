// A module-level sentinel, `MISSING = object()`, used only to mark "no value" (`d.get(k, MISSING)`,
// `if old is MISSING:`), is rewritten to None for the typed build, which has no object() values:
//
//     MISSING = object()                    pass
//     log.append((k, d.get(k, MISSING)))    log.append((k, d.get(k, None)))
//     if old is MISSING:            ->      if old is None:
//
// That keeps CPython's answers only when no value the program tests could be either one, so the rewrite is
// refused unless:
// - the name is set once, at the top level, and is only compared with `is`, `is not`, `==` or `!=`, passed
//   on and stored (never printed, called, indexed or read an attribute of);
// - every None test either pairs with the sentinel's own test on the same name (`v is MISSING or v is None`,
//   `v is not MISSING and v is not None`), where the two read the same after the rewrite, or tests a re
//   call (`re.match(...) is not None`), which never gives the sentinel;
// - no name the sentinel may reach (one tested against it, set from an expression naming it, or a
//   parameter defaulting to it) is tested for truth, where object() is true and None is false.
// Lines keep their numbers, so errors name the same lines.
use crate::dynpy::parse::{self, Arg, Ex, St, Stmt};
use crate::typed_guard::{each_block, walk_ex, walk_st};
use std::collections::HashSet;

pub fn rewrite(src: &str) -> String {
    let Ok(body) = parse::parse(src) else { return src.to_string() };
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut changed = false;
    for s in &body {
        let St::Assign(ts, Ex::Call(f, args)) = &s.st else { continue };
        let ([Ex::Name(n)], Ex::Name(o)) = (ts.as_slice(), f.as_ref()) else { continue };
        if o != "object" || !args.is_empty() || !safe(&body, n) {
            continue;
        }
        let Some(new) = replace(&lines, s.line, n) else { continue };
        lines = new;
        changed = true;
    }
    if !changed {
        return src.to_string();
    }
    let mut out = lines.join("\n");
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

// Every statement, nested blocks and function bodies included.
fn each_stmt(body: &[Stmt], f: &mut dyn FnMut(&Stmt)) {
    for s in body {
        f(s);
        each_block(&s.st, &mut |b| each_stmt(b, f));
    }
}

fn is_name(e: &Ex, n: &str) -> bool {
    matches!(e, Ex::Name(m) if m == n)
}

fn mentions(e: &Ex, n: &str) -> bool {
    let mut f = false;
    walk_ex(e, &mut |x| f |= is_name(x, n));
    f
}

// `x is None`, `x is not None`, `x == None`, `x != None`: (x, the op).
fn none_test(e: &Ex) -> Option<(&Ex, &'static str)> {
    let Ex::Cmp(l, ops) = e else { return None };
    let [(op, r)] = ops.as_slice() else { return None };
    if !matches!(*op, "is" | "is not" | "==" | "!=") {
        return None;
    }
    match (l.as_ref(), r) {
        (x, Ex::NoneV) | (Ex::NoneV, x) => Some((x, op)),
        _ => None,
    }
}

// `x is N` (or `x is not N`, ==, !=): (x, the op).
fn sentinel_test<'a>(e: &'a Ex, n: &str) -> Option<(&'a Ex, &'static str)> {
    let Ex::Cmp(l, ops) = e else { return None };
    let [(op, r)] = ops.as_slice() else { return None };
    match (l.as_ref(), r) {
        (x, y) | (y, x) if is_name(y, n) && !is_name(x, n) => Some((x, op)),
        _ => None,
    }
}

fn negated(op: &str) -> bool {
    op == "is not" || op == "!="
}

fn same(a: &Ex, b: &Ex) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

// Leaves of an `or` (or `and`) chain.
fn chain<'a>(e: &'a Ex, or: bool, out: &mut Vec<&'a Ex>) {
    match e {
        Ex::Or(a, b) if or => {
            chain(a, or, out);
            chain(b, or, out);
        }
        Ex::And(a, b) if !or => {
            chain(a, or, out);
            chain(b, or, out);
        }
        x => out.push(x),
    }
}

fn re_call(e: &Ex) -> bool {
    match e {
        Ex::Call(f, _) => match f.as_ref() {
            Ex::Attr(r, m) => is_name(r, "re") || matches!(m.as_str(), "match" | "search" | "fullmatch"),
            _ => false,
        },
        _ => false,
    }
}

fn safe(body: &[Stmt], n: &str) -> bool {
    let mut ok = true;
    let mut sets = 0;
    // Names the sentinel may reach.
    let mut carriers: HashSet<String> = HashSet::new();
    // None tests paired with the sentinel's own test (by address).
    let mut paired: HashSet<*const Ex> = HashSet::new();
    let mut tests: Vec<*const Ex> = vec![];
    // Sentinel tests, and those paired with a None test of the same value (`v is MISSING or v is None`).
    let mut s_tests: HashSet<*const Ex> = HashSet::new();
    let mut s_paired: HashSet<*const Ex> = HashSet::new();
    each_stmt(body, &mut |s| {
        match &s.st {
            St::Assign(ts, v) => {
                for t in ts {
                    if mentions(t, n) {
                        sets += 1;
                    }
                    if let Ex::Name(m) = t {
                        if m != n && mentions(v, n) {
                            carriers.insert(m.clone());
                        }
                    }
                }
            }
            St::AugAssign(t, _, _) | St::AnnAssign(t, _) | St::For(t, ..) if mentions(t, n) => ok = false,
            St::Global(v) | St::Nonlocal(v) if v.iter().any(|x| x == n) => ok = false,
            St::Def(d) => {
                if d.name == n || d.params.pos.iter().chain(&d.params.kwonly).any(|p| p.0 == n) {
                    ok = false;
                }
                for (p, dflt) in d.params.pos.iter().chain(&d.params.kwonly) {
                    if dflt.as_ref().is_some_and(|x| mentions(x, n)) {
                        carriers.insert(p.clone());
                    }
                }
            }
            St::Class(c) if c.name == n => ok = false,
            St::Import(v) | St::ImportFrom(_, v) if v.iter().any(|(m, a)| a.as_deref().unwrap_or(m) == n) => ok = false,
            _ => {}
        }
        if let St::Try(_, hs, _, _) = &s.st {
            if hs.iter().any(|h| h.name.as_deref() == Some(n)) {
                ok = false;
            }
        }
        walk_st(&s.st, &mut |e| {
            match e {
                Ex::Or(..) | Ex::And(..) => {
                    let or = matches!(e, Ex::Or(..));
                    let mut leaves = vec![];
                    chain(e, or, &mut leaves);
                    for l in &leaves {
                        if let Some((x, op)) = sentinel_test(l, n) {
                            if negated(op) != or && leaves.iter().any(|m| none_test(m).is_some_and(|(y, op2)| same(x, y) && negated(op2) == negated(op))) {
                                s_paired.insert(*l as *const Ex);
                            }
                        }
                        if let Some((x, op)) = none_test(l) {
                            if negated(op) != or && leaves.iter().any(|m| sentinel_test(m, n).is_some_and(|(y, op2)| same(x, y) && negated(op2) == negated(op))) {
                                paired.insert(*l as *const Ex);
                            }
                        }
                    }
                }
                Ex::Cmp(l, ops) => {
                    let sides: Vec<&Ex> = std::iter::once(l.as_ref()).chain(ops.iter().map(|(_, x)| x)).collect();
                    if sides.iter().any(|x| is_name(x, n)) {
                        match sentinel_test(e, n) {
                            Some((x, op)) if matches!(op, "is" | "is not" | "==" | "!=") => {
                                s_tests.insert(e as *const Ex);
                                if let Ex::Name(m) = x {
                                    carriers.insert(m.clone());
                                }
                            }
                            _ => ok = false,
                        }
                    }
                    if none_test(e).is_some() {
                        tests.push(e as *const Ex);
                    }
                }
                // Read only as a whole value: not called, indexed, iterated or read an attribute of.
                Ex::Attr(b, _) | Ex::Index(b, _) if is_name(b, n) => ok = false,
                Ex::Call(f, args) => {
                    if is_name(f, n) {
                        ok = false;
                    }
                    let shows = matches!(f.as_ref(), Ex::Name(p) if matches!(p.as_str(), "print" | "str" | "repr" | "format" | "id" | "hash" | "type" | "isinstance" | "bool" | "len" | "list" | "iter"));
                    if shows && args.iter().any(|a| matches!(a, Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) if is_name(x, n))) {
                        ok = false;
                    }
                }
                Ex::FStr(_) | Ex::Bin("%", ..) if mentions(e, n) => ok = false,
                Ex::Lambda(p, _) if p.pos.iter().any(|x| x.0 == n || x.1.as_ref().is_some_and(|d| mentions(d, n))) => ok = false,
                Ex::Walrus(m, _) if m == n => ok = false,
                Ex::Comp(_, _, _, fors) if fors.iter().any(|c| mentions(&c.target, n)) => ok = false,
                _ => {}
            }
        });
    });
    if !ok || sets != 1 {
        return false;
    }
    // Every None test is paired, or on a re call.
    let mut none_ok = true;
    each_stmt(body, &mut |s| {
        walk_st(&s.st, &mut |e| {
            if let Some((x, _)) = none_test(e) {
                if !paired.contains(&(e as *const Ex)) && !re_call(x) {
                    none_ok = false;
                }
            }
        })
    });
    // When every sentinel test also tests None the same way, the program never tells the two apart, so a
    // None it makes cannot be mistaken for the sentinel.
    let never_apart = s_tests.iter().all(|t| s_paired.contains(t));
    if !none_ok || (!never_apart && makes_none(body, &paired)) {
        return false;
    }
    // No carrier is tested for truth.
    let truth = |e: &Ex| matches!(e, Ex::Name(m) if carriers.contains(m) || m == n);
    let mut tested = false;
    each_stmt(body, &mut |s| {
        match &s.st {
            St::If(arms, _) => tested |= arms.iter().any(|(c, _)| truth(c)),
            St::While(c, ..) | St::Assert(c, _) => tested |= truth(c),
            _ => {}
        }
        walk_st(&s.st, &mut |e| match e {
            Ex::Unary("not", x) => tested |= truth(x),
            Ex::And(a, b) | Ex::Or(a, b) => tested |= truth(a) || truth(b),
            Ex::IfExp(c, _, _) => tested |= truth(c),
            Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(p) if p == "bool" || p == "filter" || p == "any" || p == "all") => {
                tested |= args.iter().any(|a| matches!(a, Arg::Pos(x) if truth(x)))
            }
            Ex::Comp(_, _, _, fors) => tested |= fors.iter().any(|c| c.ifs.iter().any(&truth)),
            _ => {}
        });
    });
    !tested
}

// Could a value the program passes around be None, which the sentinel would then be mistaken for
// (`get(k, None)` against `if default is MISSING`)? Yes for a None literal anywhere but a paired or re test
// (or `d.pop(k, None)` as a statement, whose value is dropped), a parameter defaulting to None, a one-argument
// `.get(k)` or `.setdefault(k)`, and the value of a call to a function that may end without a value.
fn makes_none(body: &[Stmt], paired: &HashSet<*const Ex>) -> bool {
    // Functions (and methods) that may give None: a bare `return`, or a last statement that is not a return
    // or raise.
    let mut gives_none: HashSet<String> = HashSet::new();
    each_stmt(body, &mut |s| {
        if let St::Def(d) = &s.st {
            let mut bare = !matches!(d.body.last().map(|s| &s.st), Some(St::Return(Some(_)) | St::Raise(..)));
            each_stmt(&d.body, &mut |x| bare |= matches!(x.st, St::Return(None)));
            if bare {
                gives_none.insert(d.name.clone());
            }
        }
    });
    let mut found = false;
    // Allowed None literals, by address: in paired or re tests, and `d.pop(k, None)` statements.
    let mut ok_none: HashSet<*const Ex> = HashSet::new();
    let mut dropped: HashSet<*const Ex> = HashSet::new();
    each_stmt(body, &mut |s| {
        if let St::Expr(e) = &s.st {
            dropped.insert(e as *const Ex);
            if let Ex::Call(f, args) = e {
                if let (Ex::Attr(_, m), [Arg::Pos(_), Arg::Pos(x @ Ex::NoneV)]) = (f.as_ref(), args.as_slice()) {
                    if m == "pop" {
                        ok_none.insert(x as *const Ex);
                    }
                }
            }
        }
        if let St::Def(d) = &s.st {
            found |= d.params.pos.iter().chain(&d.params.kwonly).any(|p| matches!(p.1, Some(Ex::NoneV)));
        }
        walk_st(&s.st, &mut |e| {
            if let Ex::Cmp(l, ops) = e {
                if paired.contains(&(e as *const Ex)) || none_test(e).is_some_and(|(x, _)| re_call(x)) {
                    ok_none.insert(l.as_ref() as *const Ex);
                    ops.iter().for_each(|(_, x)| {
                        ok_none.insert(x as *const Ex);
                    });
                }
            }
        });
    });
    each_stmt(body, &mut |s| {
        walk_st(&s.st, &mut |e| match e {
            Ex::NoneV => found |= !ok_none.contains(&(e as *const Ex)),
            Ex::Lambda(p, _) => found |= p.pos.iter().any(|x| matches!(x.1, Some(Ex::NoneV))),
            Ex::Call(f, args) => {
                let used = !dropped.contains(&(e as *const Ex));
                match f.as_ref() {
                    Ex::Attr(_, m) if args.len() == 1 && matches!(m.as_str(), "get" | "setdefault") => found = true,
                    Ex::Attr(_, m) | Ex::Name(m) if used && gives_none.contains(m) => found = true,
                    _ => {}
                }
            }
            _ => {}
        })
    });
    found
}

// The lines with `n = object()` made `pass` and every other `n` in code made None (strings and comments
// are left as written; an f-string naming n was refused by safe()).
fn replace(lines: &[String], def_line: usize, n: &str) -> Option<Vec<String>> {
    let mut out = lines.to_vec();
    let d = out.get_mut(def_line.checked_sub(1)?)?;
    if d.trim() != format!("{n} = object()") && !d.trim().starts_with(&format!("{n} = object()  #")) && !d.trim().starts_with(&format!("{n} = object() #")) {
        return None;
    }
    *d = "pass".into();
    for (i, l) in out.iter_mut().enumerate() {
        if i + 1 == def_line || !l.contains(n) {
            continue;
        }
        *l = swap_words(l, n)?;
    }
    Some(out)
}

// l with each code word n (outside strings and comments) made None.
fn swap_words(l: &str, n: &str) -> Option<String> {
    let b: Vec<char> = l.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let nc: Vec<char> = n.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '#' {
            out.extend(&b[i..]);
            break;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if b[i..].starts_with(&nc) && (i == 0 || !word(b[i - 1]) && b[i - 1] != '.') && b.get(i + nc.len()).map_or(true, |&c| !word(c)) {
            out.push_str("None");
            i += nc.len();
            continue;
        }
        out.push(c);
        i += 1;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::rewrite;
    fn swapped(src: &str) -> bool {
        rewrite(src) != src
    }
    #[test]
    fn sentinel_made_none() {
        let src = "store = {}\nlog = []\nMISSING = object()\ndef setk(k, v):\n    log.append((k, store.get(k, MISSING)))\n    if v is MISSING:\n        store.pop(k, None)\n    else:\n        store[k] = v\nsetk('a', '1')\nfor k, old in log:\n    if old is MISSING:\n        print(k)\n";
        let out = rewrite(src);
        assert!(out.contains("store.get(k, None)") && out.contains("if old is None:") && out.contains("\npass\n"), "{out}");
        assert!(swapped("M = object()\ndef f(v=M):\n    return v is M or v is None\nprint(f())\n"));
        assert!(swapped("import re\nM = object()\nx = M\nprint(x is M, re.match('a', 'a') is not None)\n"));
    }
    #[test]
    fn sentinel_kept_when_none_or_truth_could_tell() {
        // An unpaired None test: the value may be either.
        assert!(!swapped("M = object()\nd = {}\nv = d.get('k', M)\nprint(v is None)\n"));
        // Tested for truth: object() is true, None is false.
        assert!(!swapped("M = object()\nd = {}\nv = d.get('k', M)\nif v:\n    print(1)\n"));
        // Shown.
        assert!(!swapped("M = object()\nprint(M)\n"));
        assert!(!swapped("M = object()\nprint(f'{M}')\n"));
        assert_eq!(rewrite("M = object()  # M marks none\nx = 'M'\nprint(x is M)\n"), "pass\nx = 'M'\nprint(x is None)\n");
        // None passed on as a value, told apart from the sentinel: rewriting would make the two one.
        assert!(!swapped("M = object()\ndef get(d, k, default=M):\n    if k in d:\n        return d[k]\n    if default is M:\n        raise KeyError(k)\n    return default\nprint(get({}, 'a', None))\n"));
        assert!(!swapped("M = object()\ndef f(x=M):\n    if x is M:\n        return 'unset'\n    return 'set'\nprint(f(None), f())\n"));
        assert!(!swapped("M = object()\nd = {}\nv = d.get('k')\nprint(v is M)\n"));
        assert!(!swapped("M = object()\ndef g():\n    pass\nv = g()\nprint(v is M)\n"));
        // Set twice.
        assert!(!swapped("M = object()\nM = object()\nprint(1 is M)\n"));
    }
}
