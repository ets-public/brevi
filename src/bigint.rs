// Python ints have no size limit. In a Python-style program, a local number read from text with int(s)
// that is only added, subtracted, multiplied, compared and printed becomes a big integer, so
// `a, b = int(p[0]), int(p[1])` / `print(a + b)` works for 60-digit numbers as it does in Python.
// Any other use (an index, a call argument, a field, division, a range) keeps it an int.
use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub fn promote(items: &mut [Item]) {
    for it in items.iter_mut() {
        if let Item::Fn { params, body, .. } = it {
            let params: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            promote_body(body, &params);
        }
    }
}

const ARITH: &[&str] = &["+", "-", "*"];
const CMP: &[&str] = &["==", "!=", "<", ">", "<=", ">="];

// int(x) on something that reads as text: parts[0], s.strip(), input().
fn from_text(e: &Expr) -> bool {
    match e {
        Expr::Method(r, m, a, _) if m == "int" && a.is_empty() => match r.as_ref() {
            Expr::Index(..) => true,
            Expr::Method(_, t, _, _) => t.starts_with("trim") || t == "lower" || t == "upper",
            Expr::Call(f, a, _) => f == "input" && a.len() <= 1,
            _ => false,
        },
        _ => false,
    }
}

fn bigable(e: &Expr, c: &HashSet<String>) -> bool {
    match e {
        Expr::Ident(n) | Expr::Move(n) => c.contains(n),
        Expr::Int(_) => true,
        Expr::Binary(op, a, b) if ARITH.contains(op) => bigable(a, c) && bigable(b, c),
        Expr::Unary("-", a) => bigable(a, c),
        e => from_text(e),
    }
}

fn mentions(e: &Expr, c: &HashSet<String>) -> bool {
    match e {
        Expr::Ident(n) | Expr::Move(n) => c.contains(n),
        Expr::Binary(_, a, b) => mentions(a, c) || mentions(b, c),
        Expr::Unary(_, a) => mentions(a, c),
        e => from_text(e),
    }
}

struct Scan<'a> {
    c: &'a HashSet<String>,
    bad: HashSet<String>,
    defs: HashMap<String, Vec<Expr>>,
}

impl Scan<'_> {
    fn def(&mut self, target: &Pat, value: Option<&Expr>) {
        match (target, value) {
            (Pat::Bind(n), Some(v)) => {
                self.defs.entry(n.clone()).or_default().push(v.clone());
                let ok = self.c.contains(n);
                self.expr(v, ok);
            }
            (Pat::Tuple(ps), Some(Expr::Tuple(vs))) if ps.len() == vs.len() => {
                for (p, v) in ps.iter().zip(vs) {
                    self.def(p, Some(v));
                }
            }
            (p, v) => {
                self.bind_bad(p);
                if let Some(v) = v {
                    self.expr(v, false);
                }
            }
        }
    }
    fn bind_bad(&mut self, p: &Pat) {
        match p {
            Pat::Bind(n) => {
                self.bad.insert(n.clone());
            }
            Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| self.bind_bad(p)),
            _ => {}
        }
    }
    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { name, value, .. } => self.def(name, Some(value)),
            Stmt::Assign { target: Expr::Ident(n), op, value, .. } => {
                if *op == "=" {
                    self.def(&Pat::Bind(n.clone()), Some(value));
                } else if ["+=", "-=", "*="].contains(op) {
                    // n += v is n = n + v.
                    let whole = Expr::Binary(&op[..1], Box::new(Expr::Ident(n.clone())), Box::new(value.clone()));
                    self.defs.entry(n.clone()).or_default().push(whole);
                    let ok = self.c.contains(n);
                    self.expr(value, ok);
                } else {
                    self.bad.insert(n.clone());
                    self.expr(value, false);
                }
            }
            Stmt::Assign { target: Expr::Tuple(ts), op: "=", value, .. } if ts.iter().all(|t| matches!(t, Expr::Ident(_))) => {
                self.def(&tuple_pat(ts), Some(value));
            }
            Stmt::Assign { target, value, .. } => {
                self.expr(target, false);
                self.expr(value, false);
            }
            Stmt::Expr { expr, .. } => self.expr(expr, false),
            Stmt::Return { value: Some(v), .. } => self.expr(v, false),
            Stmt::Assert { cond, .. } => self.expr(cond, false),
            _ => {}
        }
    }
    // ok: this spot may hold a big integer.
    fn expr(&mut self, e: &Expr, ok: bool) {
        match e {
            Expr::Ident(n) | Expr::Move(n) => {
                if !ok {
                    self.bad.insert(n.clone());
                }
            }
            Expr::Binary(op, a, b) if ARITH.contains(op) => {
                let k = ok && bigable(a, self.c) && bigable(b, self.c);
                self.expr(a, k);
                self.expr(b, k);
            }
            Expr::Binary(op, a, b) if CMP.contains(op) => {
                let k = bigable(a, self.c) && bigable(b, self.c);
                self.expr(a, k);
                self.expr(b, k);
            }
            Expr::Unary("-", a) => self.expr(a, ok),
            Expr::Call(f, args, _) if f == "print" || f == "str" => {
                for (n, a) in args {
                    self.expr(a, n.is_none());
                }
            }
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.expr(x, true);
                    }
                }
            }
            Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Try(a) => self.expr(a, false),
            Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => {
                self.expr(a, false);
                self.expr(b, false);
            }
            Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| self.expr(x, false)),
            Expr::Map(kv) => kv.iter().for_each(|(k, v)| {
                self.expr(k, false);
                self.expr(v, false);
            }),
            Expr::Call(_, args, _) => args.iter().for_each(|(_, a)| self.expr(a, false)),
            Expr::Method(r, _, args, _) => {
                self.expr(r, false);
                args.iter().for_each(|a| self.expr(a, false));
            }
            Expr::Lambda(ps, b) => {
                ps.iter().for_each(|p| {
                    self.bad.insert(p.clone());
                });
                self.expr(b, false);
            }
            Expr::If(c, a, b) => {
                self.expr(c, false);
                self.expr(a, false);
                if let Some(b) = b {
                    self.expr(b, false);
                }
            }
            Expr::Match(x, arms) => {
                self.expr(x, false);
                for (p, a) in arms {
                    self.bind_bad(p);
                    self.expr(a, false);
                }
            }
            Expr::For(p, xs, b) => {
                self.bind_bad(p);
                self.expr(xs, false);
                self.expr(b, false);
            }
            Expr::Block(ss, last) => {
                ss.iter().for_each(|s| self.stmt(s));
                if let Some(l) = last {
                    self.expr(l, false);
                }
            }
            Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) => {}
        }
    }
}

fn promote_body(body: &mut Expr, params: &HashSet<String>) {
    // Start from every name set from int(text) somewhere, then drop names with a use or a value that
    // cannot be big, until nothing changes.
    let mut seed = Scan { c: &HashSet::new(), bad: HashSet::new(), defs: HashMap::new() };
    seed.expr(body, false);
    // Every local is a candidate at first; the fixed point keeps those whose every value and use can
    // be big, and then only the ones whose values come (through other kept names) from int(text).
    let mut c: HashSet<String> = seed.defs.keys().filter(|n| !params.contains(*n)).cloned().collect();
    loop {
        loop {
            if c.is_empty() {
                break;
            }
            let mut s = Scan { c: &c, bad: HashSet::new(), defs: HashMap::new() };
            s.expr(body, false);
            let mut next: HashSet<String> = c.iter().filter(|n| !s.bad.contains(*n)).cloned().collect();
            next.retain(|n| s.defs.get(n).is_some_and(|vs| vs.iter().all(|v| bigable(v, &c))));
            if next == c {
                break;
            }
            c = next;
        }
        let mut reach: HashSet<String> = HashSet::new();
        loop {
            let before = reach.len();
            for n in &c {
                if seed.defs[n].iter().any(|v| has_text(v) || mentions_name(v, &reach)) {
                    reach.insert(n.clone());
                }
            }
            if reach.len() == before {
                break;
            }
        }
        if reach == c {
            break;
        }
        c = reach;
    }
    rewrite(body, &c, false);
}

// Turns int(text) into big(text) and number literals next to promoted names into big numbers.
fn conv(e: &mut Expr, c: &HashSet<String>) {
    match e {
        Expr::Int(_) => {
            let n = std::mem::replace(e, Expr::Int(0));
            *e = Expr::Method(Box::new(n), "big".into(), vec![], 0);
        }
        e if from_text(e) => {
            if let Expr::Method(_, m, _, _) = e {
                *m = "big".into();
            }
        }
        Expr::Binary(_, a, b) => {
            conv(a, c);
            conv(b, c);
        }
        Expr::Unary(_, a) => conv(a, c),
        _ => {}
    }
}

fn rewrite_stmt(s: &mut Stmt, c: &HashSet<String>) {
    match s {
        Stmt::Let { name, value, .. } => rewrite_def(name, value, c),
        Stmt::Assign { target: Expr::Ident(n), value, .. } if c.contains(n.as_str()) => {
            conv(value, c);
        }
        Stmt::Assign { target: Expr::Tuple(ts), op: "=", value, .. } if ts.iter().all(|t| matches!(t, Expr::Ident(_))) => {
            rewrite_def(&tuple_pat(ts), value, c);
        }
        Stmt::Assign { target, value, .. } => {
            rewrite(target, c, false);
            rewrite(value, c, false);
        }
        Stmt::Expr { expr, .. } => rewrite(expr, c, false),
        Stmt::Return { value: Some(v), .. } => rewrite(v, c, false),
        Stmt::Assert { cond, .. } => rewrite(cond, c, false),
        _ => {}
    }
}

fn rewrite_def(p: &Pat, v: &mut Expr, c: &HashSet<String>) {
    match (p, v) {
        (Pat::Bind(n), v) if c.contains(n) => conv(v, c),
        (Pat::Tuple(ps), Expr::Tuple(vs)) if ps.len() == vs.len() => {
            for (p, v) in ps.iter().zip(vs.iter_mut()) {
                rewrite_def(p, v, c);
            }
        }
        (_, v) => rewrite(v, c, false),
    }
}

fn rewrite(e: &mut Expr, c: &HashSet<String>, _in: bool) {
    match e {
        Expr::Binary(op, a, b) if (ARITH.contains(op) || CMP.contains(op)) && bigable(a, c) && bigable(b, c) && (mentions(a, c) || mentions(b, c)) && (mentions_name(a, c) || mentions_name(b, c)) => {
            conv(a, c);
            conv(b, c);
        }
        Expr::Block(ss, last) => {
            ss.iter_mut().for_each(|s| rewrite_stmt(s, c));
            if let Some(l) = last {
                rewrite(l, c, false);
            }
        }
        Expr::Str(parts) => parts.iter_mut().for_each(|p| {
            if let StrPart::Interp(x) = p {
                printed(x, c);
            }
        }),
        Expr::Call(f, args, _) if f == "print" || f == "str" => args.iter_mut().for_each(|(n, a)| {
            if n.is_none() {
                printed(a, c)
            } else {
                rewrite(a, c, false)
            }
        }),
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Try(a) | Expr::Lambda(_, a) => rewrite(a, c, false),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => {
            rewrite(a, c, false);
            rewrite(b, c, false);
        }
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| rewrite(x, c, false)),
        Expr::Map(kv) => kv.iter_mut().for_each(|(k, v)| {
            rewrite(k, c, false);
            rewrite(v, c, false);
        }),
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| rewrite(a, c, false)),
        Expr::Method(r, _, args, _) => {
            rewrite(r, c, false);
            args.iter_mut().for_each(|a| rewrite(a, c, false));
        }
        Expr::If(x, a, b) => {
            rewrite(x, c, false);
            rewrite(a, c, false);
            if let Some(b) = b {
                rewrite(b, c, false);
            }
        }
        Expr::Match(x, arms) => {
            rewrite(x, c, false);
            arms.iter_mut().for_each(|(_, a)| rewrite(a, c, false));
        }
        Expr::For(_, xs, b) => {
            rewrite(xs, c, false);
            rewrite(b, c, false);
        }
        _ => {}
    }
}

fn mentions_name(e: &Expr, c: &HashSet<String>) -> bool {
    match e {
        Expr::Ident(n) | Expr::Move(n) => c.contains(n),
        Expr::Binary(_, a, b) => mentions_name(a, c) || mentions_name(b, c),
        Expr::Unary(_, a) => mentions_name(a, c),
        _ => false,
    }
}

fn tuple_pat(ts: &[Expr]) -> Pat {
    Pat::Tuple(ts.iter().map(|t| if let Expr::Ident(n) = t { Pat::Bind(n.clone()) } else { Pat::Wild }).collect())
}

// print(int(p[0]) + int(p[1])): sums of int(text) printed straight away are big too.
fn printed(e: &mut Expr, c: &HashSet<String>) {
    if matches!(e, Expr::Binary(op, ..) if ARITH.contains(op)) && bigable(e, c) && mentions(e, c) {
        conv(e, c);
    } else {
        rewrite(e, c, false);
    }
}

fn has_text(e: &Expr) -> bool {
    match e {
        Expr::Binary(_, a, b) => has_text(a) || has_text(b),
        Expr::Unary(_, a) => has_text(a),
        e => from_text(e),
    }
}
