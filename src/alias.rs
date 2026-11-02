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
    // Functions that hand back an item of a container (`def stat(sku): ...; return stats[sku]`):
    // name -> (the container: a parameter's index or a top-level name, the key parameter's index).
    getters: std::collections::HashMap<String, (Result<usize, String>, usize)>,
    // Functions (and class methods, by method name) that return a filtered, sorted or sliced copy of a
    // list they did not build (`return [t for t in tasks if not t[1]]`, `return list(tasks)`).
    copy_fns: HashSet<String>,
    // Class method name -> the fields of self its body names (`self.tx` in any way).
    method_fields: std::collections::HashMap<String, HashSet<String>>,
    // Copies that only matter when the value is not shared (a list of ints, not an object, record or row):
    // (line, name, message, fix), checked by the Rust compiler through a `__shared(name)` marker.
    pending: Vec<(usize, String, String, String)>,
}

thread_local! {
    // (function, parameter index) pairs whose function changes that parameter in place (`dq.popleft()`, `lot[0] -= t`).
    static CHANGES_PARAM: std::cell::RefCell<HashSet<(String, usize)>> = std::cell::RefCell::new(HashSet::new());
}

pub fn writeback(items: &mut [Item]) -> Vec<Diag> {
    CHANGES_PARAM.with(|c| {
        let mut c = c.borrow_mut();
        c.clear();
        for it in items.iter() {
            if let Item::Fn { name, params, body, .. } = it {
                for (i, p) in params.iter().enumerate() {
                    // Directly, or through an item taken from it (`lot = dq[0]`, then `lot[0] -= t`).
                    let mut hit = any_expr(body, &mut |st| direct(&p.name, st));
                    if !hit {
                        let mut items_of: Vec<String> = vec![];
                        any_expr(body, &mut |st| {
                            if let Stmt::Assign { target: Expr::Ident(x), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(x), value, .. } = st {
                                if root(value) == Some(p.name.as_str()) && matches!(value, Expr::Index(..)) {
                                    items_of.push(x.clone());
                                }
                            }
                            false
                        });
                        hit = items_of.iter().any(|x| any_expr(body, &mut |st| direct(x, st)));
                    }
                    if hit {
                        c.insert((name.clone(), i));
                    }
                }
            }
        }
    });
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
    let mut getters = std::collections::HashMap::new();
    for it in items.iter() {
        if let Item::Fn { name, params, body, .. } = it {
            if let Some(g) = getter(params, body) {
                getters.insert(name.clone(), g);
            }
        }
    }
    let mut copy_fns = HashSet::new();
    for it in items.iter() {
        if let Item::Fn { name, body, .. } = it {
            let mut hit = false;
            let mut check = |v: &Expr| {
                if let Expr::Method(r, m, _, _) = v {
                    if COPYING.contains(&m.as_str()) && mentions_name(r) {
                        hit = true;
                    }
                }
            };
            any_expr(body, &mut |st| {
                if let Stmt::Return { value: Some(v), .. } = st {
                    check(v);
                }
                false
            });
            if let Expr::Block(_, Some(t)) = body {
                check(t);
            }
            if hit {
                copy_fns.insert(name.clone());
                if let Some(c) = classes.iter().find(|c| name.starts_with(c.as_str())) {
                    copy_fns.insert(name[c.len()..].to_string());
                }
            }
        }
    }
    let mut method_fields: std::collections::HashMap<String, HashSet<String>> = std::collections::HashMap::new();
    for it in items.iter() {
        if let Item::Fn { name, params, body, .. } = it {
            if params.first().is_some_and(|p| p.name == "self_") {
                if let Some(c) = classes.iter().find(|c| name.starts_with(c.as_str())) {
                    let mut fs = HashSet::new();
                    crate::infer::walk(body, &mut |e| {
                        if let Expr::Field(b, f) = e {
                            if matches!(b.as_ref(), Expr::Ident(n) if n == "self_") {
                                fs.insert(f.clone());
                            }
                        }
                    });
                    method_fields.entry(name[c.len()..].to_string()).or_default().extend(fs);
                }
            }
        }
    }
    let mut cx = Cx { errs: vec![], mutators, getters, copy_fns, method_fields, pending: vec![] };
    let inl: Inline = items
        .iter()
        .filter_map(|it| match it {
            Item::Fn { name, params, body: Expr::Block(st, t), .. } => {
                let e = match (st.as_slice(), t) {
                    ([Stmt::Return { value: Some(e), .. }], None) => e.clone(),
                    ([], Some(e)) => e.as_ref().clone(),
                    // `xs = [l for l in self.lots if ...]`, `xs.sort(key=...)`, `return xs`: the sorted list.
                    ([set, Stmt::Expr { expr: Expr::Method(r, m, a, l), .. }, rest @ ..], t) if matches!(r.as_ref(), Expr::Ident(_)) && matches!(m.as_str(), "sort" | "sort_by") => {
                        let Expr::Ident(x) = r.as_ref() else { return None };
                        let first = match set {
                            Stmt::Let { name: Pat::Bind(n), value, .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if n == x => value.clone(),
                            _ => return None,
                        };
                        let ret = match (rest, t) {
                            ([Stmt::Return { value: Some(v), .. }], None) => v,
                            ([], Some(v)) => v.as_ref(),
                            _ => return None,
                        };
                        if !matches!(ret, Expr::Ident(n) | Expr::Move(n) if n == x) {
                            return None;
                        }
                        Expr::Method(Box::new(first), m.clone(), a.clone(), *l)
                    }
                    _ => return None,
                };
                Some((name.clone(), (params.iter().map(|p| p.name.clone()).collect(), e)))
            }
            _ => None,
        })
        .collect();
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } | Item::Test { body, .. } = it {
            let pre = loops(body, &inl);
            if let Expr::Block(st, _) = body {
                st.splice(0..0, pre);
            }
            stored_then_changed(body, &mut cx);
            if crate::infer::py_on() {
                name_alias(body, &mut cx);
            }
            walk(body, &mut cx);
            derived_loops(body, &mut cx);
            for (line, var, msg, fix) in std::mem::take(&mut cx.pending) {
                let hex: String = format!("{msg}\u{1}{fix}").bytes().map(|b| format!("{b:02x}")).collect();
                // `@name`: the value itself must be shared (an object or a row), not just its items.
                let (var, whole) = match var.strip_prefix('@') { Some(v) => (v.to_string(), true), None => (var.clone(), false) };
                let mut margs = vec![(None, Expr::Ident(var.clone())), (None, Expr::Str(vec![StrPart::Lit(hex)]))];
                if whole {
                    margs.push((None, Expr::Bool(true)));
                }
                let mark = Stmt::Expr { expr: Expr::Call("__shared".into(), margs, line), line };
                if !place_mark(body, line, &var, &mark) {
                    err(&mut cx, line, msg, fix);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    cx.errs.retain(|d| seen.insert((d.line, d.msg.clone())));
    cx.errs
}

// A copy that is wrong only when `var`'s value is not shared: the Rust compiler decides (see `__shared`).
fn defer(cx: &mut Cx, line: usize, var: &str, msg: String, fix: String) {
    cx.pending.push((line, var.to_string(), msg, fix));
}

// Puts the marker where `var` is in scope: first in the body of the loop at `line` that names it, or right
// after the statement at `line`.
fn place_mark(e: &mut Expr, line: usize, var: &str, mark: &Stmt) -> bool {
    match e {
        Expr::Block(stmts, tail) => {
            for i in 0..stmts.len() {
                if stmt_line(&stmts[i]) == line {
                    if let Stmt::Expr { expr: Expr::For(p, _, body), .. } = &mut stmts[i] {
                        if format!("{p:?}").contains(&format!("{var:?}")) {
                            if let Expr::Block(b, _) = body.as_mut() {
                                b.insert(0, mark.clone());
                                return true;
                            }
                        }
                        // A loop made to walk indexes (`for i in ...: bo = xs[i]`): right after var is set.
                        if let Expr::Block(b, _) = body.as_mut() {
                            let at = b.iter().position(|st| matches!(st, Stmt::Let { name: Pat::Bind(n), .. } | Stmt::Assign { target: Expr::Ident(n), .. } if n == var));
                            if let Some(k) = at {
                                b.insert(k + 1, mark.clone());
                                return true;
                            }
                        }
                    }
                    if !matches!(&stmts[i], Stmt::Expr { expr: Expr::For(..) | Expr::If(..) | Expr::While(..), .. }) {
                        stmts.insert(i + 1, mark.clone());
                        return true;
                    }
                }
                let hit = match &mut stmts[i] {
                    Stmt::Expr { expr, .. } => place_mark(expr, line, var, mark),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => matches!(value, Expr::Block(..) | Expr::If(..)) && place_mark(value, line, var, mark),
                    _ => false,
                };
                if hit {
                    return true;
                }
            }
            if let Some(t) = tail {
                return place_mark(t, line, var, mark);
            }
            false
        }
        Expr::If(_, a, b) => place_mark(a, line, var, mark) || b.as_mut().is_some_and(|b| place_mark(b, line, var, mark)),
        // A loop that is the last thing in its block (no statement line of its own): the one binding var.
        Expr::For(p, it, b) if format!("{p:?}").contains(&format!("{var:?}")) && format!("{it:?}").contains(&format!(", {line})")) => {
            if let Expr::Block(st, _) = b.as_mut() {
                st.insert(0, mark.clone());
                return true;
            }
            place_mark(b, line, var, mark)
        }
        Expr::For(_, _, b) | Expr::While(_, b) => place_mark(b, line, var, mark),
        Expr::Match(_, arms) => arms.iter_mut().any(|(_, a)| place_mark(a, line, var, mark)),
        _ => false,
    }
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
fn loops(e: &mut Expr, inl: &Inline) -> Vec<Stmt> {
    match e {
        Expr::Block(stmts, tail) => {
            split_pairs(stmts);
            fold_local_copies(stmts, tail);
            let mut k = 0;
            while k < stmts.len() {
                let pre = match &mut stmts[k] {
                    Stmt::Expr { expr, .. } => loops(expr, inl),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => loops(value, inl),
                    _ => vec![],
                };
                let n = pre.len();
                for (j, p) in pre.into_iter().enumerate() {
                    stmts.insert(k + j, p);
                }
                k += n + 1;
            }
            if let Some(t) = tail {
                let pre = loops(t, inl);
                stmts.extend(pre);
            }
        }
        Expr::If(_, a, b) => {
            loops(a, inl);
            if let Some(b) = b {
                loops(b, inl);
            }
        }
        Expr::While(_, b) => {
            loops(b, inl);
        }
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| {
            loops(a, inl);
        }),
        Expr::For(p, it, body) => {
            loops(body, inl);
            let item = match &*p {
                Pat::Bind(a) => Some(a.clone()),
                Pat::Tuple(ps) if ps.len() == 2 => if let Pat::Bind(a) = &ps[1] { Some(a.clone()) } else { None },
                _ => None,
            };
            let Some(a) = item else { return vec![] };
            if !any_expr(body, &mut |st| direct(&a, st)) {
                return vec![];
            }
            if matches!(p, Pat::Bind(_)) {
                if let Some((i, pre)) = index_loop(&a, it, body) {
                    *p = Pat::Bind(i);
                    return pre;
                }
                // `for b in orders.pending_for(sku)` where the method only returns a list made from a field.
                if let Some(mut inlined) = inline_call(it, inl) {
                    let mut b2 = body.clone();
                    if let Some((i, pre)) = index_loop(&a, &mut inlined, &mut b2) {
                        *p = Pat::Bind(i);
                        *it = inlined;
                        *body = b2;
                        return pre;
                    }
                }
            }
            let Expr::Method(base, m, args, line) = it.as_ref() else { return vec![] };
            if !args.is_empty() || !matches!(base.as_ref(), Expr::Ident(_) | Expr::Field(..)) {
                return vec![];
            }
            let (base, line) = (base.as_ref().clone(), *line);
            let (key, new_it) = match (m.as_str(), &*p) {
                ("values", Pat::Bind(_)) => (format!("ax_vk{line}_{a}"), Expr::Method(Box::new(base.clone()), "keys".into(), vec![], line)),
                ("pairs", Pat::Tuple(ps)) => match &ps[0] {
                    Pat::Bind(k) => (k.clone(), Expr::Method(Box::new(base.clone()), "keys".into(), vec![], line)),
                    _ => return vec![],
                },
                ("enumerate", Pat::Tuple(ps)) => match &ps[0] {
                    Pat::Bind(i) => (i.clone(), Expr::Range(Box::new(Expr::Int(0)), Box::new(Expr::Method(Box::new(base.clone()), "len".into(), vec![], line)))),
                    _ => return vec![],
                },
                _ => return vec![],
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
    vec![]
}

// `h, a = teams[home], teams[away]` is two settings, so each name is an alias of its entry and the
// write-back below works on it as on `h = teams[home]`.
fn split_pairs(stmts: &mut Vec<Stmt>) {
    let mut k = 0;
    while k < stmts.len() {
        let split = match &stmts[k] {
            Stmt::Assign { target: Expr::Tuple(ts), op: "=", value: Expr::Tuple(vs), line } if ts.len() == vs.len() && ts.len() > 1 => {
                let ns: Option<Vec<String>> = ts.iter().map(|t| if let Expr::Ident(n) = t { Some(n.clone()) } else { None }).collect();
                ns.map(|ns| (ns, vs.clone(), *line, false))
            }
            Stmt::Let { name: Pat::Tuple(ps), value: Expr::Tuple(vs), line, .. } if ps.len() == vs.len() && ps.len() > 1 => {
                let ns: Option<Vec<String>> = ps.iter().map(|p| if let Pat::Bind(n) = p { Some(n.clone()) } else { None }).collect();
                ns.map(|ns| (ns, vs.clone(), *line, true))
            }
            _ => None,
        };
        if let Some((ns, vs, line, is_let)) = split {
            if vs.iter().all(|v| matches!(v, Expr::Index(..))) && !vs.iter().any(|v| ns.iter().any(|n| names(v, n))) {
                let new: Vec<Stmt> = ns
                    .into_iter()
                    .zip(vs)
                    .map(|(n, v)| if is_let { Stmt::Let { name: Pat::Bind(n), ty: None, value: v, mutable: true, line } } else { Stmt::Assign { target: Expr::Ident(n), op: "=", value: v, line } })
                    .collect();
                let c = new.len();
                stmts.splice(k..k + 1, new);
                k += c;
                continue;
            }
        }
        k += 1;
    }
}

// The names a statement's debug text mentions.
fn idents(d: &str) -> Vec<String> {
    d.split("Ident(\"").skip(1).filter_map(|r| r.split('"').next()).map(String::from).collect()
}

fn names(e: &Expr, n: &str) -> bool {
    let d = format!("{e:?}");
    d.contains(&format!("Ident({n:?})")) || d.contains(&format!("Move({n:?})"))
}

// `cands = [l for l in self.lots if ...]`, `cands.sort(key=...)`, then `for lot in cands:` changing lot:
// Python changes the dicts in self.lots. When cands is used nowhere else, the loop goes over the
// filtered, sorted expression itself, which the index loop below writes back.
fn fold_local_copies(stmts: &mut Vec<Stmt>, tail: &Option<Box<Expr>>) {
    let mut k = 0;
    while k < stmts.len() {
        let Stmt::Expr { expr: Expr::For(Pat::Bind(a), it, body), .. } = &stmts[k] else { k += 1; continue };
        let Expr::Ident(x) = it.as_ref() else { k += 1; continue };
        let (x, a) = (x.clone(), a.clone());
        if !any_expr(body, &mut |st| direct(&a, st)) || names(body, &x) {
            k += 1;
            continue;
        }
        // The setting of x, then at most an in-place sort, right before the loop.
        let mut def = None;
        let mut sort: Option<(String, Vec<Expr>, usize)> = None;
        // Statements between that touch nothing the setting reads (`taken = 0`) may stay before the loop.
        let mut between: Vec<Stmt> = vec![];
        let mut sort_at = None;
        for j in (0..k).rev() {
            match &stmts[j] {
                Stmt::Expr { expr: Expr::Method(r, m, args, l), .. } if matches!(r.as_ref(), Expr::Ident(n) if *n == x) && matches!(m.as_str(), "sort" | "sort_by") && sort.is_none() && between.is_empty() => {
                    sort = Some((m.clone(), args.clone(), *l));
                    sort_at = Some(j);
                }
                Stmt::Let { name: Pat::Bind(n), value, .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if *n == x => {
                    let next = if sort.is_some() { j + 2 } else { j + 1 };
                    let ok_between = between.iter().all(|b| {
                        let d = format!("{b:?}");
                        let reads = format!("{value:?}");
                        !names(&Expr::Block(vec![b.clone()], None), &x) && !d.contains("Call(") && !d.contains("Method(") && idents(&d).iter().all(|i| !reads.contains(&format!("Ident({i:?})")))
                    });
                    if next + between.len() == k && ok_between && matches!(value, Expr::Method(_, m, _, _) if m == "filter" || m == "sort_by" || m == "sort") {
                        def = Some((j, value.clone()));
                    }
                    break;
                }
                st if sort.is_none() && !names(&Expr::Block(vec![st.clone()], None), &x) => between.push(st.clone()),
                _ => break,
            }
        }
        let _ = sort_at;
        let Some((j, value)) = def else { k += 1; continue };
        // x must not be read after the loop.
        let later = stmts[k + 1..].iter().any(|st| names(&Expr::Block(vec![st.clone()], None), &x)) || tail.as_ref().is_some_and(|t| names(t, &x));
        if later {
            k += 1;
            continue;
        }
        let new_it = match sort {
            Some((m, args, l)) => Expr::Method(Box::new(value), m, args, l),
            None => value,
        };
        if let Stmt::Expr { expr: Expr::For(_, it, _), .. } = &mut stmts[k] {
            **it = new_it;
        }
        // Drop the setting (and its sort); the statements between stay, in order.
        let drop_to = k - between.len();
        stmts.drain(j..drop_to);
        k = j + between.len() + 1;
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
                defer(
                    cx,
                    line,
                    &x,
                    format!("{into} gets a copy of {xs} here, so changing {xs} later does not change {into} (Brevi values are copies, not shared)"),
                    format!("finish {xs} before putting it in {into}, or change it through {into}"),
                );
            }
        }
    }
}

// Python's `b = a` gives a second name for the same list or dict: `b.append(x)` changes a too. Brevi copies
// plain lists and dicts, so a change through one name that the other is read after is refused.
fn name_alias(e: &Expr, cx: &mut Cx) {
    let mut blocks: Vec<&Expr> = vec![];
    collect_blocks(e, &mut blocks);
    for b in blocks {
        let Expr::Block(stmts, tail) = b else { continue };
        for (i, st) in stmts.iter().enumerate() {
            let Stmt::Assign { target: Expr::Ident(bn), op: "=", value: Expr::Ident(an) | Expr::Move(an), line } = st else { continue };
            if bn == an {
                continue;
            }
            let rest = &stmts[i + 1..];
            // Up to the next setting of either name as a whole.
            let end = rest.iter().position(|s| rebinds_in(s, bn) || rebinds_in(s, an)).unwrap_or(rest.len());
            let t = if end == rest.len() { tail.as_deref() } else { None };
            let muts = cx.mutators.clone();
            for (x, y) in [(bn, an), (an, bn)] {
                // The first change through x, and a read of y after it.
                let Some(k) = rest[..end].iter().position(|s| any_stmt(std::slice::from_ref(s), None, &mut |s| direct_strict(x, s, &muts))) else { continue };
                let later = &rest[k + 1..end];
                let read = later.iter().any(|s| names(&Expr::Block(vec![s.clone()], None), y)) || t.is_some_and(|t| names(t, y)) || any_stmt(std::slice::from_ref(&rest[k]), None, &mut |s| matches!(s, Stmt::Expr { .. } | Stmt::Assign { .. }) && loops_back(s, y));
                if read {
                    let (bs, as_) = (bn.trim_end_matches('_'), an.trim_end_matches('_'));
                    defer(
                        cx,
                        *line,
                        &format!("@{bn}"),
                        format!("{bs} = {as_} makes {bs} a copy in Brevi, but in Python both names are the same list or dict, so changing one changes the other"),
                        format!("use one name, or write {bs} = {as_}.copy() if a separate copy is meant"),
                    );
                    break;
                }
            }
        }
    }
}

// A loop statement that changes one name and reads the other (`for ...: b.append(x); print(a)`).
fn loops_back(s: &Stmt, y: &str) -> bool {
    matches!(s, Stmt::Expr { expr: Expr::For(..) | Expr::While(..), .. }) && names(&Expr::Block(vec![s.clone()], None), y)
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
const COPYING: [&str; 8] = ["filter", "sort", "sort_by", "rev", "slice", "take", "skip", "unique"];

// How deep below the name `x` an expression reaches: `x` is 0, `x[i]` is 1, `x[i][1]` and `x[i].n` are 2.
fn depth_below(e: &Expr, x: &str) -> Option<usize> {
    match e {
        Expr::Ident(n) if n == x => Some(0),
        Expr::Field(b, _) | Expr::Index(b, _) | Expr::TupleField(b, _) => depth_below(b, x).map(|d| d + 1),
        _ => None,
    }
}

// A statement that changes an item inside `x` (`x[i][1] = v`, `x[i].n += 1`, `x[i].append(v)`).
fn item_change(x: &str, st: &Stmt) -> bool {
    match st {
        Stmt::Assign { target, .. } => depth_below(target, x).is_some_and(|d| d >= 2),
        Stmt::Expr { expr: Expr::Method(recv, m, _, _), .. } => depth_below(recv, x).is_some_and(|d| d >= 1) && CHANGING.contains(&m.as_str()),
        _ => false,
    }
}

fn derived_loops(e: &Expr, cx: &mut Cx) {
    let mut found: Vec<(String, String, usize)> = vec![];
    let muts = cx.mutators.clone();
    // Locals set from a helper that returns a copy of a list it did not build (`vis = visible_tasks()`):
    // changing an item of one (`vis[i][1] = ...`) changes the shared item in Python, a copy here.
    let mut from_copy: Vec<(String, String)> = vec![];
    any_expr(e, &mut |st| {
        if let Stmt::Let { name: Pat::Bind(x), value, .. } | Stmt::Assign { target: Expr::Ident(x), op: "=", value, .. } = st {
            let f = match value {
                Expr::Call(f, _, _) => Some(f),
                Expr::Method(_, m, _, _) => Some(m),
                _ => None,
            };
            if f.is_some_and(|f| cx.copy_fns.contains(f)) {
                from_copy.push((x.clone(), show(value)));
            }
        }
        false
    });
    let mut item_errs: Vec<(String, String, usize)> = vec![];
    for (x, from) in &from_copy {
        any_expr(e, &mut |st| {
            if item_change(x, st) {
                let line = match st {
                    Stmt::Assign { line, .. } | Stmt::Expr { line, .. } => *line,
                    _ => 0,
                };
                item_errs.push((x.clone(), from.clone(), line));
            }
            false
        });
    }
    for (raw, from, line) in item_errs {
        let x = raw.trim_end_matches('_').to_string();
        defer(
            cx,
            line,
            &raw,
            format!("the items of {x} are copies of the items {from} returned, so changing them does not change the original list (Brevi values are copies, not shared)"),
            "change the item in the original list, e.g. find its index there and set tasks[i][1] = v".into(),
        );
    }
    // Locals set to a filtered, sorted or sliced copy of a list (`cand = sorted(l for l in self.lots if ...)`):
    // their items are copies too.
    let mut copies: HashSet<String> = HashSet::new();
    any_expr(e, &mut |st| {
        if let Stmt::Let { name: Pat::Bind(x), value: Expr::Method(r, m, _, _), .. } | Stmt::Assign { target: Expr::Ident(x), op: "=", value: Expr::Method(r, m, _, _), .. } = st {
            if COPYING.contains(&m.as_str()) && mentions_name(r) {
                copies.insert(x.clone());
            }
        }
        false
    });
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
                Expr::Ident(x) => copies.contains(x),
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
    for (raw, from, line) in found {
        let a = raw.trim_end_matches('_').to_string();
        defer(
            cx,
            line,
            &raw,
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
                // `m = months[date[:7]]` then `m[0] += x`: the key is worked out once, into a name, so the
                // change can be written back under it.
                // `st = stat(sku)` where stat returns `stats[sku]`: the call is made, then st names stats[sku].
                if let Some((r, call, base, key, line)) = getter_call(&stmts[i], &cx.getters) {
                    if changed(&r, &stmts[i + 1..], tail.as_deref()) {
                        stmts[i] = Stmt::Assign { target: Expr::Ident(r), op: "=", value: Expr::Index(Box::new(base), Box::new(key)), line };
                        stmts.insert(i, Stmt::Expr { expr: call, line });
                        i += 1;
                        continue;
                    }
                }
                if let Some((r, key, line)) = keyed_alias(&stmts[i]) {
                    if changed(&r, &stmts[i + 1..], tail.as_deref()) {
                        let kn = format!("ax_ak{line}_{r}");
                        if let Stmt::Assign { value, .. } | Stmt::Let { value, .. } = &mut stmts[i] {
                            match value {
                                Expr::Index(_, k) => **k = Expr::Ident(kn.clone()),
                                Expr::Method(_, _, args, _) => args[0] = Expr::Ident(kn.clone()),
                                _ => {}
                            }
                        }
                        stmts.insert(i, Stmt::Assign { target: Expr::Ident(kn), op: "=", value: key, line });
                        i += 1;
                        continue;
                    }
                }
                if let Some((r, base, key, line, guarded)) = alias_of(&stmts[i]) {
                    let mut rest = stmts.split_off(i + 1);
                    // The code stores it back itself (`q = lots.get(sku, [])` ... `lots[sku] = q`) after the last change.
                    let slot = format!("{:?}", Expr::Index(Box::new(base.clone()), Box::new(key.clone())));
                    let stores = rest.iter().rposition(|st| matches!(st, Stmt::Assign { target, op: "=", value: Expr::Ident(v), .. } if *v == r && format!("{target:?}") == slot));
                    let stored_back = stores.is_some_and(|j| !changed(&r, &rest[j + 1..], tail.as_deref()));
                    if !stored_back && changed(&r, &rest, tail.as_deref()) {
                        if safe(&r, &base, &key, &rest, tail.as_deref()) {
                            let set = Stmt::Assign { target: Expr::Index(Box::new(base.clone()), Box::new(key.clone())), op: "=", value: Expr::Ident(r.clone()), line };
                            // `dq = lots.get(sku, [])`: when sku is missing, Python changes a new list nobody keeps.
                            let wb = if guarded {
                                let has = Expr::Method(Box::new(base), "has".into(), vec![key], line);
                                Stmt::Expr { expr: Expr::If(Box::new(has), Box::new(Expr::Block(vec![set], None)), None), line }
                            } else {
                                set
                            };
                            add(&r, &wb, &mut rest, tail);
                        } else if any_stmt(&rest, tail.as_deref(), &mut |s| direct_strict(&r, s, &cx.mutators)) {
                            let (rs, from) = (r.trim_end_matches('_').to_string(), show(&Expr::Index(Box::new(base.clone()), Box::new(key.clone()))));
                            defer(
                                cx,
                                line,
                                &r,
                                format!("{rs} is a copy of {from}, and the code after changes {rs} in a way Brevi cannot copy back (the list, map or key changes too), so {from} would not change (Brevi values are copies, not shared)"),
                                format!("change {from} directly, or set {from} = {rs} after changing {rs}"),
                            );
                        }
                    }
                    stmts.extend(rest);
                } else if let Some((r, b, f, line)) = field_alias(&stmts[i]) {
                    // `t = self.tx` then `t['postings'].append(p)`: in Python t is self.tx; here a copy, so it is
                    // written back after each change, unless later lines set self.tx or a method that uses it runs.
                    let mut rest = stmts.split_off(i + 1);
                    if changed(&r, &rest, tail.as_deref()) {
                        let target = Expr::Field(Box::new(Expr::Ident(b.clone())), f.clone());
                        let ts = format!("{target:?}");
                        let mf = cx.method_fields.clone();
                        let clobber = any_stmt(&rest, tail.as_deref(), &mut |st| {
                            let calls = |e: &Expr| {
                                let mut hit = false;
                                crate::infer::walk(e, &mut |x| {
                                    if let Expr::Method(recv, m, _, _) = x {
                                        if matches!(recv.as_ref(), Expr::Ident(n) if *n == b) && mf.get(m).is_some_and(|fs| fs.contains(&f)) {
                                            hit = true;
                                        }
                                    }
                                });
                                hit
                            };
                            match st {
                                Stmt::Assign { target: Expr::Ident(n), value, .. } => n == &r || n == &b || calls(value),
                                Stmt::Let { name: Pat::Bind(n), value, .. } => n == &r || n == &b || calls(value),
                                Stmt::Assign { target: t, value, .. } => format!("{t:?}") == ts || calls(value),
                                Stmt::Expr { expr: Expr::For(p, it, _), .. } => format!("{p:?}").contains(&format!("{r:?}")) || calls(it),
                                Stmt::Expr { expr, .. } if !matches!(expr, Expr::If(..) | Expr::For(..) | Expr::While(..) | Expr::Block(..) | Expr::Match(..)) => calls(expr),
                                _ => false,
                            }
                        });
                        if !clobber {
                            let wb = Stmt::Assign { target, op: "=", value: Expr::Ident(r.clone()), line };
                            add(&r, &wb, &mut rest, tail);
                        } else {
                            let (rs, from) = (r.trim_end_matches('_').to_string(), format!("{}.{}", b.trim_end_matches('_'), f));
                            defer(
                                cx,
                                line,
                                &r,
                                format!("{rs} is a copy of {from}, and the code after changes {rs} where Brevi cannot copy it back ({from} is also set or used there), so {from} would not change (Brevi values are copies, not shared)"),
                                format!("change {from} directly, or set {from} = {rs} after changing {rs}"),
                            );
                        }
                    }
                    stmts.extend(rest);
                } else if let Some((r, from, line)) = picked(&stmts[i]) {
                    let rest = &stmts[i + 1..];
                    if any_stmt(rest, tail.as_deref(), &mut |s| direct_strict(&r, s, &cx.mutators)) {
                        let rs = r.trim_end_matches('_').to_string();
                        defer(
                            cx,
                            line,
                            &r,
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

// `t = self.tx`: a local named after a field of a plain name.
fn field_alias(st: &Stmt) -> Option<(String, String, String, usize)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    match value {
        Expr::Field(b, f) => match b.as_ref() {
            Expr::Ident(b) if b != r => Some((r.clone(), b.clone(), f.clone(), line)),
            _ => None,
        },
        _ => None,
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
        // `max(xs, key=f)` is `xs.sort_by(f, true)[0]`; also `sorted(xs)[0]` and the like.
        Expr::Index(c, _) if matches!(c.as_ref(), Expr::Method(b, m, _, _) if COPYING.contains(&m.as_str()) && mentions_name(b)) => Some((r.clone(), "max() or min()".to_string(), line)),
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
        // `d.setdefault(l[2], [0, 0])`: an item of a record named by a constant.
        Expr::Index(b, i) => matches!(b.as_ref(), Expr::Ident(_)) && matches!(i.as_ref(), Expr::Int(_) | Expr::Str(_)) && plain_key(i),
        _ => false,
    }
}

// `r = base[key]` where base is a name or a field path and key a name or a constant.
fn alias_of(st: &Stmt) -> Option<(String, Expr, Expr, usize, bool)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    let (base, key, guarded) = match value {
        Expr::Index(base, key) => (base.as_ref().clone(), key.as_ref().clone(), false),
        // `r = resting.get(rid)` (None when missing): changed only after a None check, so it is there.
        Expr::Method(base, m, args, _) if (m == "get" || m == "get_or") && (args.len() == 1 || matches!(args.get(1), Some(Expr::Call(f, a, _)) if f == "none" && a.is_empty())) => {
            (base.as_ref().clone(), args[0].clone(), false)
        }
        // `dq = lots.get(sku, deque())`: the stored list when there is one.
        Expr::Method(base, m, args, _) if m == "get_or" && args.len() == 2 && (matches!(&args[1], Expr::List(x) if x.is_empty()) || matches!(&args[1], Expr::Map(x) if x.is_empty())) => {
            (base.as_ref().clone(), args[0].clone(), true)
        }
        _ => return None,
    };
    let b = root(&base)?;
    let nested = matches!(&base, Expr::Index(bb, bk) if matches!(bb.as_ref(), Expr::Ident(_) | Expr::Field(..)) && plain_key(bk) && !matches!(bk.as_ref(), Expr::Ident(k) if k == r));
    if b == r || !(matches!(&base, Expr::Ident(_) | Expr::Field(..)) || nested) || !plain_key(&key) {
        return None;
    }
    if matches!(&key, Expr::Ident(k) if k == r) {
        return None;
    }
    Some((r.clone(), base, key, line, guarded))
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
    // A loop that ends a block is its tail: give it to f as a statement too (on the line after the last one).
    if let Some(t) = tail {
        if matches!(t, Expr::For(..)) {
            let inside = match t {
                Expr::For(_, _, b) => match b.as_ref() {
                    Expr::Block(st, _) => st.first().map_or(0, stmt_line).saturating_sub(1),
                    _ => 0,
                },
                _ => 0,
            };
            let line = if inside > 0 { inside } else { stmts.last().map_or(0, stmt_line) + 1 };
            if f(&Stmt::Expr { expr: t.clone(), line }) {
                return true;
            }
        }
    }
    tail.map_or(false, |t| any_expr(t, f))
}

fn stmt_line(st: &Stmt) -> usize {
    match st {
        Stmt::Expr { line, .. } | Stmt::Let { line, .. } | Stmt::Assign { line, .. } => *line,
        _ => 0,
    }
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
    any_stmt(rest, tail, &mut |st| direct_wb(r, st))
}

// `direct`, or r handed to a function of the program (`cogs = take(dq, q)`), which may change it: the
// copy is written back after the call (unchanged, that changes nothing).
fn direct_wb(r: &str, st: &Stmt) -> bool {
    let passes = |e: &Expr| match e {
        Expr::Call(f, args, _) => args.iter().enumerate().any(|(i, (_, a))| matches!(a, Expr::Ident(n) if n == r) && CHANGES_PARAM.with(|c| c.borrow().contains(&(f.clone(), i)))),
        // `self.discard(t)` with discard changing its parameter (the method's own `self` is parameter 0).
        Expr::Method(_, m, args, _) => args.iter().enumerate().any(|(i, a)| matches!(a, Expr::Ident(n) if n == r) && CHANGES_PARAM.with(|c| c.borrow().iter().any(|(f, j)| *j == i + 1 && f.ends_with(&format!("_{m}"))))),
        _ => false,
    };
    direct(r, st)
        || match st {
            Stmt::Expr { expr, .. } => passes(expr),
            Stmt::Assign { value, .. } | Stmt::Let { value, .. } => passes(value),
            _ => false,
        }
}

// Nothing later sets r, the key name or the container as a whole, or the same slot; and r is not a loop
// name further down.
fn safe(r: &str, base: &Expr, key: &Expr, rest: &[Stmt], tail: Option<&Expr>) -> bool {
    let b = root(base).unwrap_or("");
    let k = root(key).unwrap_or("");
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
        let d = direct_wb(r, &st);
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

const SHAPE: [&str; 15] = ["push", "insert", "pop", "pop_at", "remove", "clear", "heap_push", "heap_pop", "heapify", "sort", "sort_by", "rev", "append", "extend", "discard"];

// The body changes the list as a whole (adds, removes, sorts or sets it): its indexes do not stay put.
// `base.remove(a)` of the loop's own item is allowed when `own` is given.
fn shape_changed(b: &str, body: &Expr, own: Option<&str>) -> bool {
    let mut hit = false;
    crate::infer::walk(body, &mut |e| {
        if let Expr::Method(r, m, args, _) = e {
            if root(r) == Some(b) && matches!(r.as_ref(), Expr::Ident(_) | Expr::Field(..)) && SHAPE.contains(&m.as_str()) {
                let mine = m == "remove" && own.is_some_and(|a| own_item(args, a));
                if !mine {
                    hit = true;
                }
            }
        }
    });
    hit || any_expr(body, &mut |st| matches!(st, Stmt::Assign { target, .. } if matches!(target, Expr::Ident(n) if n == b) || matches!(target, Expr::Method(r, ..) if root(r) == Some(b))))
}

fn subst(e: &mut Expr, name: &str, with: &Expr) {
    crate::parser::walk_mut(e, &mut |x| {
        if matches!(x, Expr::Ident(n) if n == name) {
            *x = with.clone();
        }
    });
}

// `for lot in sorted((l for l in lots if l.sku == s), key=...)` or `for b in list(backorders)` whose body
// changes the item: in Python the item is the list's own object. The loop goes over indexes of the list
// instead (`lot = lots[i]`), so the write-back puts each change back in place.
fn index_loop(a: &str, it: &mut Box<Expr>, body: &mut Box<Expr>) -> Option<(String, Vec<Stmt>)> {
    // Peel `.filter(f)`, `.sort_by(k)`, `.sort()`, `.rev()` and `.skip(0)` down to the list itself.
    let mut steps: Vec<(String, Option<(String, Expr)>)> = vec![];
    let mut cur: &Expr = it.as_ref();
    let line;
    // `m.get(k, [])` at the bottom: the list m[k].
    let mut got: Option<Expr> = None;
    loop {
        if steps.is_empty() && matches!(cur, Expr::Ident(_) | Expr::Field(..)) {
            // A plain list: name the index after the body's first line.
            line = match body.as_ref() {
                Expr::Block(st, _) => st.first().map(stmt_line).unwrap_or(0),
                _ => 0,
            };
            break;
        }
        match cur {
            Expr::Method(r, m, args, l) if m == "get_or" && matches!(args.as_slice(), [k, Expr::List(d)] if d.is_empty() && plain_key(k)) && matches!(r.as_ref(), Expr::Ident(_) | Expr::Field(..)) => {
                got = Some(Expr::Index(r.clone(), Box::new(args[0].clone())));
                line = *l;
                break;
            }
            Expr::Method(r, m, args, l) => {
                let step = match (m.as_str(), args.as_slice()) {
                    ("filter" | "sort_by", [Expr::Lambda(ps, lb)]) if ps.len() == 1 => (m.clone(), Some((ps[0].clone(), lb.as_ref().clone()))),
                    ("sort" | "rev", []) => (m.clone(), None),
                    ("skip", [Expr::Int(0)]) => (m.clone(), None),
                    _ => return None,
                };
                steps.push(step);
                cur = r;
                if matches!(cur, Expr::Ident(_) | Expr::Field(..)) {
                    line = *l;
                    break;
                }
                if let Expr::Method(r2, m2, a2, _) = cur {
                    if m2 == "get_or" && matches!(a2.as_slice(), [k, Expr::List(d)] if d.is_empty() && plain_key(k)) && matches!(r2.as_ref(), Expr::Ident(_) | Expr::Field(..)) {
                        got = Some(Expr::Index(r2.clone(), Box::new(a2[0].clone())));
                        line = *l;
                        break;
                    }
                }
            }
            _ => return None,
        }
    }
    let base = got.unwrap_or_else(|| cur.clone());
    let Some(b) = root(&base).map(String::from) else { return None };
    // A plain `for b in self.backs` changes the items in place, unless the body also reads the list's
    // owner (`self.shipped += got`): then it goes by index too, so each change is written back.
    if steps.is_empty() && !matches!(base, Expr::Index(..)) && !names(body, &b) {
        return None;
    }
    steps.reverse();
    let copy_only = steps.iter().all(|(m, _)| m == "skip");
    if shape_changed(&b, body, if copy_only { Some(a) } else { None }) {
        return None;
    }
    let i = format!("ax_li{line}");
    let at = |ix: &str| Expr::Index(Box::new(base.clone()), Box::new(Expr::Ident(ix.to_string())));
    let len = Expr::Method(Box::new(base.clone()), "len".into(), vec![], line);
    if copy_only {
        // `for b in list(xs)` that may remove b itself: later items move down one place each time.
        let removes = {
            let mut hit = false;
            crate::infer::walk(body, &mut |e| {
                if matches!(e, Expr::Method(r, m, _, _) if m == "remove" && root(r) == Some(b.as_str())) {
                    hit = true;
                }
            });
            hit
        };
        if !removes {
            **it = Expr::Range(Box::new(Expr::Int(0)), Box::new(len));
            prepend(body, vec![Stmt::Assign { target: Expr::Ident(a.into()), op: "=", value: at(&i), line }], line);
            return Some((i, vec![]));
        }
        let (d, k) = (format!("ax_ld{line}"), format!("ax_lk{line}"));
        // After `xs.remove(b)`: one more item gone below the next one.
        let bump = Stmt::Assign { target: Expr::Ident(d.clone()), op: "+=", value: Expr::Int(1), line };
        bump_after_remove(body, &b, a, &bump);
        prepend(
            body,
            vec![
                Stmt::Assign { target: Expr::Ident(k.clone()), op: "=", value: Expr::Binary("-", Box::new(Expr::Ident(i.clone())), Box::new(Expr::Ident(d.clone()))), line },
                Stmt::Assign { target: Expr::Ident(a.into()), op: "=", value: at(&k), line },
            ],
            line,
        );
        **it = Expr::Range(Box::new(Expr::Int(0)), Box::new(len));
        return Some((i, vec![Stmt::Assign { target: Expr::Ident(d), op: "=", value: Expr::Int(0), line }]));
    }
    // Indexes worked out before the loop go stale if the body takes items out of the list
    // (`orders.remove(b)`, also through a helper): leave the loop to the copy check.
    let d = format!("{body:?}");
    if d.contains("\"remove\"") || d.contains("\"pop\"") || d.contains("\"pop_at\"") || d.contains("remove") {
        return None;
    }
    // Indexes of the items, filtered and sorted by the same tests on `xs[i]`.
    let mut new_it = Expr::Method(Box::new(Expr::Range(Box::new(Expr::Int(0)), Box::new(len))), "skip".into(), vec![Expr::Int(0)], line);
    for (m, lam) in steps {
        new_it = match (m.as_str(), lam) {
            ("filter" | "sort_by", Some((p, mut lb))) => {
                subst(&mut lb, &p, &at(&i));
                Expr::Method(Box::new(new_it), m, vec![Expr::Lambda(vec![i.clone()], Box::new(lb))], line)
            }
            ("sort", None) => Expr::Method(Box::new(new_it), "sort_by".into(), vec![Expr::Lambda(vec![i.clone()], Box::new(at(&i)))], line),
            (m, _) => Expr::Method(Box::new(new_it), m.to_string(), vec![], line),
        };
    }
    **it = new_it;
    prepend(body, vec![Stmt::Assign { target: Expr::Ident(a.into()), op: "=", value: at(&i), line }], line);
    Some((i, vec![]))
}

fn prepend(body: &mut Box<Expr>, sts: Vec<Stmt>, line: usize) {
    let old = std::mem::replace(body.as_mut(), Expr::Bool(false));
    **body = match old {
        Expr::Block(mut st, t) => {
            for (k, s) in sts.into_iter().enumerate() {
                st.insert(k, s);
            }
            Expr::Block(st, t)
        }
        other => {
            let mut st = sts;
            st.push(Stmt::Expr { expr: other, line });
            Expr::Block(st, None)
        }
    };
}

fn bump_after_remove(e: &mut Expr, b: &str, a: &str, bump: &Stmt) {
    crate::parser::walk_mut(e, &mut |x| {
        if let Expr::Block(stmts, tail) = x {
            let is_rm = |e: &Expr| matches!(e, Expr::Method(r, m, args, _) if m == "remove" && root(r) == Some(b) && own_item(args, a));
            if tail.as_deref().is_some_and(is_rm) {
                let t = tail.take().unwrap();
                let line = if let Expr::Method(_, _, _, l) = t.as_ref() { *l } else { 0 };
                stmts.push(Stmt::Expr { expr: *t, line });
            }
            let mut k = 0;
            while k < stmts.len() {
                if matches!(&stmts[k], Stmt::Expr { expr: Expr::Method(r, m, args, _), .. } if m == "remove" && root(r) == Some(b) && own_item(args, a)) {
                    stmts.insert(k + 1, bump.clone());
                    k += 1;
                }
                k += 1;
            }
        }
    });
}

// `xs.remove(b)` (Python's, as `xs.remove(xs.index_of(b))`) of the loop's own item b.
fn own_item(args: &[Expr], a: &str) -> bool {
    match args {
        [Expr::Ident(x)] => x == a,
        [Expr::Method(_, m, xs, _)] if m == "index_of" => matches!(xs.as_slice(), [Expr::Ident(x)] if x == a),
        _ => false,
    }
}

// `r = base[key]` (or `base.get(key)`) whose key is an expression (`date[:7]`, `row["sku"]`), not a name.
fn keyed_alias(st: &Stmt) -> Option<(String, Expr, usize)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    let (base, key) = match value {
        Expr::Index(base, key) => (base.as_ref(), key.as_ref()),
        Expr::Method(base, m, args, _) if (m == "get" || m == "get_or") && (args.len() == 1 || matches!(args.get(1), Some(Expr::Call(f, a, _)) if f == "none" && a.is_empty())) => (base.as_ref(), &args[0]),
        _ => return None,
    };
    let b = root(base)?;
    if b == r || !matches!(base, Expr::Ident(_) | Expr::Field(..)) || plain_key(key) || matches!(key, Expr::Range(..)) {
        return None;
    }
    Some((r.clone(), key.clone(), line))
}

// A function whose only value is `base[k]`, with k a parameter and base a parameter or a top-level name.
fn getter(params: &[Param], body: &Expr) -> Option<(Result<usize, String>, usize)> {
    let mut rets: Vec<Expr> = vec![];
    let mut other = false;
    crate::infer::walk(body, &mut |e| {
        if let Expr::Block(stmts, _) = e {
            for st in stmts {
                if let Stmt::Return { value, .. } = st {
                    match value {
                        Some(v) => rets.push(v.clone()),
                        None => other = true,
                    }
                }
            }
        }
        if matches!(e, Expr::Lambda(..)) {
            other = true;
        }
    });
    if let Expr::Block(_, Some(t)) = body {
        rets.push(t.as_ref().clone());
    }
    if other || rets.len() != 1 {
        return None;
    }
    let Expr::Index(b, k) = &rets[0] else { return None };
    let (Expr::Ident(bn), Expr::Ident(kn)) = (b.as_ref(), k.as_ref()) else { return None };
    let ki = params.iter().position(|p| &p.name == kn)?;
    // The function must not set the key name to something else.
    let mut sets_key = false;
    crate::infer::walk(body, &mut |e| {
        if let Expr::Block(stmts, _) = e {
            if stmts.iter().any(|st| matches!(st, Stmt::Assign { target: Expr::Ident(n), .. } | Stmt::Let { name: Pat::Bind(n), .. } if n == kn || n == bn)) {
                sets_key = true;
            }
        }
    });
    if sets_key {
        return None;
    }
    let base = match params.iter().position(|p| &p.name == bn) {
        Some(i) => Ok(i),
        None => Err(bn.clone()),
    };
    Some((base, ki))
}

fn getter_call(st: &Stmt, getters: &std::collections::HashMap<String, (Result<usize, String>, usize)>) -> Option<(String, Expr, Expr, Expr, usize)> {
    let (r, value, line) = match st {
        Stmt::Assign { target: Expr::Ident(r), op: "=", value, line } => (r, value, *line),
        Stmt::Let { name: Pat::Bind(r), value, line, .. } => (r, value, *line),
        _ => return None,
    };
    let Expr::Call(f, args, _) = value else { return None };
    let (base, ki) = getters.get(f)?;
    if args.iter().any(|(n, _)| n.is_some()) {
        return None;
    }
    let base = match base {
        Ok(bi) => args.get(*bi)?.1.clone(),
        Err(g) => Expr::Ident(g.clone()),
    };
    if !matches!(base, Expr::Ident(_) | Expr::Field(..)) || root(&base) == Some(r.as_str()) {
        return None;
    }
    let key = args.get(*ki)?.1.clone();
    Some((r.clone(), value.clone(), base, key, line))
}

type Inline = std::collections::HashMap<String, (Vec<String>, Expr)>;

// A call of a function that only returns an expression, with the arguments put in (plain arguments only).
fn inline_call(it: &Expr, inl: &Inline) -> Option<Box<Expr>> {
    let Expr::Call(f, args, _) = it else { return None };
    let (ps, e) = inl.get(f)?;
    let simple = |a: &Expr| matches!(a, Expr::Ident(_) | Expr::Field(..) | Expr::Int(_) | Expr::Str(_));
    // `od.waiting(w[1])`: an item of a name is as plain as the name.
    let plain = |a: &Expr| simple(a) || matches!(a, Expr::Index(b, k) if simple(b) && simple(k));
    if ps.len() != args.len() || args.iter().any(|(n, a)| n.is_some() || !plain(a)) {
        return None;
    }
    let mut lams: Vec<String> = vec![];
    crate::infer::walk(e, &mut |x| {
        if let Expr::Lambda(ps, _) = x {
            lams.extend(ps.iter().cloned());
        }
    });
    if args.iter().any(|(_, a)| root(a).is_some_and(|n| lams.iter().any(|l| l == n))) {
        return None;
    }
    let mut e = e.clone();
    // Names first go to placeholders, so an argument named like another parameter is not replaced twice.
    for (k, p) in ps.iter().enumerate() {
        subst(&mut e, p, &Expr::Ident(format!("ax_inl{k}")));
    }
    for (k, (_, a)) in args.iter().enumerate() {
        subst(&mut e, &format!("ax_inl{k}"), a);
    }
    Some(Box::new(e))
}
