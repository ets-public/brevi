// Python's `r = rows[k]` (or `acct = accts[name]`) names the same object as the container holds, so
// `r[0] -= t` or `acct.bal += 5` changes what is in the container. Brevi values are copies, so after each
// statement that changes r in place the copy is written back: `rows[k] = r`.
use crate::ast::*;

pub fn writeback(items: &mut [Item]) {
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } | Item::Test { body, .. } = it {
            walk(body);
        }
    }
}

fn walk(e: &mut Expr) {
    match e {
        Expr::Block(stmts, tail) => {
            for st in stmts.iter_mut() {
                match st {
                    Stmt::Expr { expr, .. } => walk(expr),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => walk(value),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                walk(t);
            }
            let mut i = 0;
            while i < stmts.len() {
                if let Some((r, base, key, line)) = alias_of(&stmts[i]) {
                    let mut rest = stmts.split_off(i + 1);
                    if safe(&r, &base, &key, &rest, tail.as_deref()) && changed(&r, &rest, tail.as_deref()) {
                        let wb = Stmt::Assign { target: Expr::Index(Box::new(base), Box::new(key)), op: "=", value: Expr::Ident(r.clone()), line };
                        add(&r, &wb, &mut rest, tail);
                    }
                    stmts.extend(rest);
                }
                i += 1;
            }
        }
        Expr::If(_, a, b) => {
            walk(a);
            if let Some(b) = b {
                walk(b);
            }
        }
        Expr::For(_, _, b) | Expr::While(_, b) => walk(b),
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| walk(a)),
        _ => {}
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
const CHANGING: &[&str] = &["push", "insert", "remove", "clear", "update", "pop", "heap_push", "heap_pop", "pop_at", "sort", "sort_by", "add", "extend", "append", "discard"];

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
    !bad
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
