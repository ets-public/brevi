// Python's `r = rows[k]` (or `acct = accts[name]`) names the same object as the container holds, so
// `r[0] -= t` or `acct.bal += 5` changes what is in the container. Brevi values are copies, so after each
// statement that changes r in place the copy is written back: `rows[k] = r`.
// Where a change would still be lost (an item of `sorted(lots)` changed in a loop, a list changed after it
// was put in another), the program is refused with a way to write it, rather than giving wrong output.
use crate::ast::*;
use crate::diag::Diag;
use std::collections::HashSet;

struct Cx {
    errs: Vec<Diag>,
    // Class methods that change self (`def deposit(self, n): self.bal += n`).
    mutators: HashSet<String>,
}

pub fn writeback(items: &mut [Item]) -> Vec<Diag> {
    let classes: Vec<String> = items.iter().filter_map(|i| if let Item::Type { name, .. } = i { Some(format!("{name}_")) } else { None }).collect();
    let mut mutators = HashSet::new();
    for it in items.iter() {
        if let Item::Fn { name, params, body, .. } = it {
            if params.first().is_some_and(|p| p.name == "self_") {
                if let Some(c) = classes.iter().find(|c| name.starts_with(c.as_str())) {
                    let mut hit = false;
                    any_expr(body, &mut |st| {
                        if direct_strict("self_", st, &HashSet::new()) {
                            hit = true;
                        }
                        false
                    });
                    if hit {
                        mutators.insert(name[c.len()..].to_string());
                    }
                }
            }
        }
    }
    let mut cx = Cx { errs: vec![], mutators };
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } | Item::Test { body, .. } = it {
            loops(body);
            stored_then_changed(body, &mut cx);
            walk(body, &mut cx);
            derived_loops(body, &mut cx);
        }
    }
    let mut seen = HashSet::new();
    cx.errs.retain(|d| seen.insert((d.line, d.msg.clone())));
    cx.errs
}

fn err(cx: &mut Cx, line: usize, msg: String, fix: String) {
    let mut d = Diag::new("check", line, 1, &msg);
    d.fix = Some(fix);
    cx.errs.push(d);
}

fn show(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => n.trim_end_matches('_').to_string(),
        Expr::Field(b, f) => format!("{}.{f}", show(b)),
        Expr::Index(b, k) => format!("{}[{}]", show(b), show(k)),
        Expr::Int(n) => n.to_string(),
        Expr::Unary("-", x) => format!("-{}", show(x)),
        Expr::Str(p) => format!("{:?}", p.iter().map(|x| if let StrPart::Lit(t) = x { t.clone() } else { "...".into() }).collect::<String>()),
        Expr::Method(r, m, _, _) if m == "sort" => format!("sorted({})", show(r)),
        Expr::Method(r, m, _, _) if m == "pairs" => format!("{}.items()", show(r)),
        Expr::Method(r, m, _, _) => format!("{}.{m}(...)", show(r)),
        Expr::Call(f, _, _) => format!("{f}(...)"),
        _ => "...".into(),
    }
}

// `for a in m.values()`, `for k, a in m.items()` and `for i, x in enumerate(xs)` whose body changes the item:
// the loop goes over keys or indexes and names the item `a = m[k]`, which the write-back then covers.
fn loops(e: &mut Expr) {
    match e {
        Expr::Block(stmts, tail) => {
            for st in stmts.iter_mut() {
                match st {
                    Stmt::Expr { expr, .. } => loops(expr),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => loops(value),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                loops(t);
            }
        }
        Expr::If(_, a, b) => {
            loops(a);
            if let Some(b) = b {
                loops(b);
            }
        }
        Expr::While(_, b) => loops(b),
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| loops(a)),
        Expr::For(p, it, body) => {
            loops(body);
            let item = match &*p {
                Pat::Bind(a) => Some(a.clone()),
                Pat::Tuple(ps) if ps.len() == 2 => if let Pat::Bind(a) = &ps[1] { Some(a.clone()) } else { None },
                _ => None,
            };
            let Some(a) = item else { return };
            if !any_expr(body, &mut |st| direct(&a, st)) {
                return;
            }
            let Expr::Method(base, m, args, line) = it.as_ref() else { return };
            if !args.is_empty() || !matches!(base.as_ref(), Expr::Ident(_) | Expr::Field(..)) {
                return;
            }
            let (base, line) = (base.as_ref().clone(), *line);
            let (key, new_it) = match (m.as_str(), &*p) {
                ("values", Pat::Bind(_)) => (format!("ax_vk{line}_{a}"), Expr::Method(Box::new(base.clone()), "keys".into(), vec![], line)),
                ("pairs", Pat::Tuple(ps)) => match &ps[0] {
                    Pat::Bind(k) => (k.clone(), Expr::Method(Box::new(base.clone()), "keys".into(), vec![], line)),
                    _ => return,
                },
                ("enumerate", Pat::Tuple(ps)) => match &ps[0] {
                    Pat::Bind(i) => (i.clone(), Expr::Range(Box::new(Expr::Int(0)), Box::new(Expr::Method(Box::new(base.clone()), "len".into(), vec![], line)))),
                    _ => return,
                },
                _ => return,
            };
            let get = Stmt::Assign { target: Expr::Ident(a.clone()), op: "=", value: Expr::Index(Box::new(base), Box::new(Expr::Ident(key.clone()))), line };
            let old = std::mem::replace(body.as_mut(), Expr::Bool(false));
            let new_body = match old {
                Expr::Block(mut st, t) => {
                    st.insert(0, get);
                    Expr::Block(st, t)
                }
                other => Expr::Block(vec![get, Stmt::Expr { expr: other, line }], None),
            };
            **body = new_body;
            *p = Pat::Bind(key);
            **it = new_it;
        }
        _ => {}
    }
}

// `row = []`, `grid.append(row)`, then `row.append(1)`: grid holds a copy made before the change.
fn stored_then_changed(e: &Expr, cx: &mut Cx) {
    let mut blocks: Vec<&Expr> = vec![];
    collect_blocks(e, &mut blocks);
    for b in blocks {
        let Expr::Block(stmts, tail) = b else { continue };
        for (i, st) in stmts.iter().enumerate() {
            let Some((x, into, line)) = stored(st) else { continue };
            let rest = &stmts[i + 1..];
            // Only up to the next setting of x as a whole.
            let end = rest.iter().position(|s| matches!(s, Stmt::Assign { target: Expr::Ident(n), op: "=", .. } | Stmt::Let { name: Pat::Bind(n), .. } if *n == x)).unwrap_or(rest.len());
            let t = if end == rest.len() { tail.as_deref() } else { None };
            let muts = cx.mutators.clone();
            if any_stmt(&rest[..end], t, &mut |s| direct_strict(&x, s, &muts) && !rebinds_in(s, &x)) {
                let xs = x.trim_end_matches('_').to_string();
                err(
                    cx,
                    line,
                    format!("{into} gets a copy of {xs} here, so changing {xs} later does not change {into} (Brevi values are copies, not shared)"),
                    format!("finish {xs} before putting it in {into}, or change it through {into}"),
                );
            }
        }
    }
}

fn rebinds_in(s: &Stmt, x: &str) -> bool {
    any_stmt(std::slice::from_ref(s), None, &mut |t| matches!(t, Stmt::Assign { target: Expr::Ident(n), op: "=", .. } if n == x))
}

fn collect_blocks<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Block(stmts, tail) => {
            out.push(e);
            for st in stmts {
                match st {
                    Stmt::Expr { expr, .. } => collect_blocks(expr, out),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => collect_blocks(value, out),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                collect_blocks(t, out);
            }
        }
        Expr::If(_, a, b) => {
            collect_blocks(a, out);
            if let Some(b) = b {
                collect_blocks(b, out);
            }
        }
        Expr::For(_, _, b) | Expr::While(_, b) => collect_blocks(b, out),
        Expr::Match(_, arms) => arms.iter().for_each(|(_, a)| collect_blocks(a, out)),
        _ => {}
    }
}

// `xs.append(x)`, `m[k] = x`, `o.f = x`: x (a name) put into another value.
fn stored(st: &Stmt) -> Option<(String, String, usize)> {
    match st {
        Stmt::Expr { expr: Expr::Method(recv, m, args, _), line } if ["push", "insert", "heap_push", "append"].contains(&m.as_str()) => {
            let x = args.iter().rev().find_map(|a| if let Expr::Ident(n) = a { Some(n.clone()) } else { None })?;
            if root(recv) == Some(x.as_str()) {
                return None;
            }
            Some((x, show(recv), *line))
        }
        Stmt::Assign { target: target @ (Expr::Index(..) | Expr::Field(..)), op: "=", value: Expr::Ident(x), line } if root(target) != Some(x.as_str()) => {
            let into = match target {
                Expr::Index(b, _) | Expr::Field(b, _) => show(b),
                _ => unreachable!(),
            };
            Some((x.clone(), into, *line))
        }
        _ => None,
    }
}

// A loop over a new list made from another (`for l in sorted(lots)`) that changes its item: the change
// stays in the new list.
fn derived_loops(e: &Expr, cx: &mut Cx) {
    let mut found: Vec<(String, String, usize)> = vec![];
    let muts = cx.mutators.clone();
    any_expr(e, &mut |st| {
        if let Stmt::Expr { expr: Expr::For(p, it, body), line } = st {
            let names: Vec<String> = match p {
                Pat::Bind(a) => vec![a.clone()],
                Pat::Tuple(ps) => ps.iter().filter_map(|q| if let Pat::Bind(a) = q { Some(a.clone()) } else { None }).collect(),
                _ => vec![],
            };
            let derived = match it.as_ref() {
                Expr::Method(r, m, _, _) => !["keys", "lines", "words", "split", "chars", "range"].contains(&m.as_str()) && mentions_name(r),
                Expr::Call(f, args, _) => f != "range" && args.iter().any(|(_, a)| mentions_name(a)),
                _ => false,
            };
            if derived {
                for a in names {
                    if any_expr(body, &mut |s| direct_strict(&a, s, &muts)) {
                        found.push((a, show(it), *line));
                    }
                }
            }
        }
        false
    });
    for (a, from, line) in found {
        let a = a.trim_end_matches('_').to_string();
        err(
            cx,
            line,
            format!("{a} is a copy of an item of {from}, so changing {a} in this loop does not change the list or map it came from (Brevi values are copies, not shared)"),
            "loop over the indexes or keys of the original and change it there, e.g. for i in range(len(xs)): xs[i].n += 1".into(),
        );
    }
}

fn mentions_name(e: &Expr) -> bool {
    match e {
        Expr::Ident(_) | Expr::Field(..) => true,
        Expr::Index(b, _) | Expr::Method(b, _, _, _) => mentions_name(b),
        _ => false,
    }
}

// Like `direct`, but a method call counts only when it is known to change its value.
fn direct_strict(r: &str, st: &Stmt, mutators: &HashSet<String>) -> bool {
    match st {
        Stmt::Expr { expr: Expr::Method(recv, m, _, _), .. } => root(recv) == Some(r) && (CHANGING.contains(&m.as_str()) || mutators.contains(m)),
        _ => direct(r, st),
    }
}

fn walk(e: &mut Expr, cx: &mut Cx) {
    match e {
        Expr::Block(stmts, tail) => {
            for st in stmts.iter_mut() {
                match st {
                    Stmt::Expr { expr, .. } => walk(expr, cx),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => walk(value, cx),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                walk(t, cx);
            }
            let mut i = 0;
            while i < stmts.len() {
                if let Some((r, base, key, line)) = alias_of(&stmts[i]) {
                    let mut rest = stmts.split_off(i + 1);
                    if changed(&r, &rest, tail.as_deref()) {
                        if safe(&r, &base, &key, &rest, tail.as_deref()) {
                            let wb = Stmt::Assign { target: Expr::Index(Box::new(base), Box::new(key)), op: "=", value: Expr::Ident(r.clone()), line };
                            add(&r, &wb, &mut rest, tail);
                        } else if any_stmt(&rest, tail.as_deref(), &mut |s| direct_strict(&r, s, &cx.mutators)) {
                            let (rs, from) = (r.trim_end_matches('_').to_string(), show(&Expr::Index(Box::new(base.clone()), Box::new(key.clone()))));
                            err(
                                cx,
                                line,
                                format!("{rs} is a copy of {from}, and the code after changes {rs} in a way Brevi cannot copy back (the list, map or key changes too), so {from} would not change (Brevi values are copies, not shared)"),
                                format!("change {from} directly, or set {from} = {rs} after changing {rs}"),
                            );
                        }
                    }
                    stmts.extend(rest);
                } else if let Some((r, from, line)) = picked(&stmts[i]) {
                    let rest = &stmts[i + 1..];
                    if any_stmt(rest, tail.as_deref(), &mut |s| direct_strict(&r, s, &cx.mutators)) {
                        let rs = r.trim_end_matches('_').to_string();
                        err(
                            cx,
                            line,
                            format!("{rs} is a copy of the item {from} picks, so changing {rs} does not change the item in the list (Brevi values are copies, not shared)"),
                            "find the item's index or key, then change xs[i] directly".into(),
                        );
                    }
                }
                i += 1;
            }
        }
        Expr::If(_, a, b) => {
            walk(a, cx);
            if let Some(b) = b {
                walk(b, cx);
            }
        }
        Expr::For(_, _, b) | Expr::While(_, b) => walk(b, cx),
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| walk(a, cx)),
        _ => {}
    }
}

// `best = max(lots, key=...)`, `x = xs.first()`: an item picked out of a list.
fn picked(st: &Stmt) -> Option<(String, String, usize)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    match value {
        Expr::Method(b, m, _, _) if ["max", "min", "max_by", "min_by", "first", "last"].contains(&m.as_str()) && mentions_name(b) => Some((r.clone(), format!("{m}()"), line)),
        _ => None,
    }
}

fn root(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident(n) => Some(n),
        Expr::Field(b, _) | Expr::Index(b, _) | Expr::TupleField(b, _) => root(b),
        _ => None,
    }
}

fn plain_key(k: &Expr) -> bool {
    match k {
        Expr::Ident(_) | Expr::Int(_) => true,
        Expr::Str(parts) => parts.iter().all(|p| matches!(p, StrPart::Lit(_))),
        Expr::Unary("-", x) => matches!(x.as_ref(), Expr::Int(_)),
        Expr::Field(b, _) => matches!(b.as_ref(), Expr::Ident(_)),
        _ => false,
    }
}

// `r = base[key]` where base is a name or a field path and key a name or a constant.
fn alias_of(st: &Stmt) -> Option<(String, Expr, Expr, usize)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    let (base, key) = match value {
        Expr::Index(base, key) => (base.as_ref().clone(), key.as_ref().clone()),
        // `r = resting.get(rid)` (None when missing): changed only after a None check, so it is there.
        Expr::Method(base, m, args, _) if (m == "get" || m == "get_or") && (args.len() == 1 || matches!(args.get(1), Some(Expr::Call(f, a, _)) if f == "none" && a.is_empty())) => {
            (base.as_ref().clone(), args[0].clone())
        }
        _ => return None,
    };
    let b = root(&base)?;
    if b == r || !matches!(&base, Expr::Ident(_) | Expr::Field(..)) || !plain_key(&key) {
        return None;
    }
    if matches!(&key, Expr::Ident(k) if k == r) {
        return None;
    }
    Some((r.clone(), base, key, line))
}

// Methods that change the value they are called on, when the call is a value rather than a statement.
const CHANGING: &[&str] = &["push", "insert", "remove", "clear", "update", "pop", "heap_push", "heap_pop", "pop_at", "add", "extend", "append", "discard"];

// Does this statement itself change r in place (not counting statements nested inside it)?
fn direct(r: &str, st: &Stmt) -> bool {
    match st {
        Stmt::Assign { target, value, .. } => {
            (!matches!(target, Expr::Ident(_)) && root(target) == Some(r)) || matches!(value, Expr::Method(recv, m, _, _) if root(recv) == Some(r) && CHANGING.contains(&m.as_str()))
        }
        Stmt::Let { value, .. } => matches!(value, Expr::Method(recv, m, _, _) if root(recv) == Some(r) && CHANGING.contains(&m.as_str())),
        // `acct.deposit(5)`, `r.push(x)`: a method called for its effect.
        Stmt::Expr { expr: Expr::Method(recv, _, _, _), .. } => root(recv) == Some(r),
        _ => false,
    }
}

fn any_stmt(stmts: &[Stmt], tail: Option<&Expr>, f: &mut dyn FnMut(&Stmt) -> bool) -> bool {
    for st in stmts {
        if f(st) {
            return true;
        }
        let inner = match st {
            Stmt::Expr { expr, .. } => Some(expr),
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } => Some(value),
            _ => None,
        };
        if let Some(e) = inner {
            if any_expr(e, f) {
                return true;
            }
        }
    }
    tail.map_or(false, |t| any_expr(t, f))
}

fn any_expr(e: &Expr, f: &mut dyn FnMut(&Stmt) -> bool) -> bool {
    match e {
        Expr::Block(s, t) => any_stmt(s, t.as_deref(), f),
        Expr::If(_, a, b) => any_expr(a, f) || b.as_ref().map_or(false, |b| any_expr(b, f)),
        Expr::For(_, _, b) | Expr::While(_, b) => any_expr(b, f),
        Expr::Match(_, arms) => arms.iter().any(|(_, a)| any_expr(a, f)),
        Expr::Lambda(_, b) => any_expr(b, f),
        _ => false,
    }
}

fn changed(r: &str, rest: &[Stmt], tail: Option<&Expr>) -> bool {
    any_stmt(rest, tail, &mut |st| direct(r, st))
}

// Nothing later sets r, the key name or the container as a whole, or the same slot; and r is not a loop
// name further down.
fn safe(r: &str, base: &Expr, key: &Expr, rest: &[Stmt], tail: Option<&Expr>) -> bool {
    let b = root(base).unwrap_or("");
    let k = if let Expr::Ident(k) = key { k.as_str() } else { "" };
    let slot = format!("{:?}", Expr::Index(Box::new(base.clone()), Box::new(key.clone())));
    let bad = any_stmt(rest, tail, &mut |st| match st {
        Stmt::Assign { target: Expr::Ident(n), .. } => n == r || n == k || n == b,
        Stmt::Let { name: Pat::Bind(n), .. } => n == r || n == k || n == b,
        Stmt::Assign { target, .. } => format!("{target:?}") == slot,
        Stmt::Expr { expr: Expr::For(p, _, _), .. } => format!("{p:?}").contains(&format!("{r:?}")),
        _ => false,
    });
    // `cur = stack[-1]`, `stack.append(x)`, then `cur.append(y)`: the slot no longer holds r's item.
    let mut moved = false;
    let shifted = any_stmt(rest, tail, &mut |st| {
        if moved && direct(r, st) {
            return true;
        }
        let structural = |e: &Expr| matches!(e, Expr::Method(recv, m, _, _) if root(recv) == Some(b) && ["push", "insert", "pop", "pop_at", "remove", "clear", "heap_push", "heap_pop", "heapify", "sort", "sort_by", "rev", "append"].contains(&m.as_str()));
        match st {
            Stmt::Expr { expr, .. } | Stmt::Assign { value: expr, .. } | Stmt::Let { value: expr, .. } if structural(expr) => moved = true,
            _ => {}
        }
        false
    });
    !bad && !shifted
}

// Puts `base[key] = r` after each statement that changes r, at any depth (not inside lambdas).
fn add(r: &str, wb: &Stmt, stmts: &mut Vec<Stmt>, tail: &mut Option<Box<Expr>>) {
    let old = std::mem::take(stmts);
    for mut st in old {
        let d = direct(r, &st);
        match &mut st {
            Stmt::Expr { expr, .. } => add_expr(r, wb, expr),
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } => add_expr(r, wb, value),
            _ => {}
        }
        stmts.push(st);
        if d {
            stmts.push(wb.clone());
        }
    }
    if let Some(t) = tail {
        // A branch ending in `r.push(x)`: it is a statement, so the write-back can follow it.
        if matches!(t.as_ref(), Expr::Method(recv, m, _, _) if root(recv) == Some(r) && CHANGING.contains(&m.as_str()) && !m.starts_with("pop") && m != "heap_pop" && m != "remove") {
            let line = match wb {
                Stmt::Assign { line, .. } => *line,
                _ => 0,
            };
            let t = tail.take().unwrap();
            stmts.push(Stmt::Expr { expr: *t, line });
            stmts.push(wb.clone());
        } else {
            add_expr(r, wb, t);
        }
    }
}

fn add_expr(r: &str, wb: &Stmt, e: &mut Expr) {
    match e {
        Expr::Block(s, t) => add(r, wb, s, t),
        Expr::If(_, a, b) => {
            add_expr(r, wb, a);
            if let Some(b) = b {
                add_expr(r, wb, b);
            }
        }
        Expr::For(_, _, b) | Expr::While(_, b) => add_expr(r, wb, b),
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| add_expr(r, wb, a)),
        _ => {}
    }
}
