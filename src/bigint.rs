// Python ints have no size limit. In a Python-style program, a local number read from text with int(s)
// that is only added, subtracted, multiplied, compared and printed becomes a big integer, so
// `a, b = int(p[0]), int(p[1])` / `print(a + b)` works for 60-digit numbers as it does in Python.
// Any other use (an index, a call argument, a field, division, a range) keeps it an int.
use crate::ast::*;
use std::collections::{HashMap, HashSet};

// `2 ** 10`, `3 ** 50 % 1000000007`: powers of number literals worked out here, so one past 64 bits that is
// taken modulo a number still gives Python's answer.
fn fold_pows(e: &mut Expr) {
    fn lit(e: &Expr) -> Option<i64> {
        match e {
            Expr::Int(k) => Some(*k),
            Expr::Unary("-", x) => match x.as_ref() {
                Expr::Int(k) => k.checked_neg(),
                _ => None,
            },
            _ => None,
        }
    }
    crate::parser::walk_mut(e, &mut |x| {
        let folded = match x {
            Expr::Binary("%", p, m) => match (p.as_ref(), lit(m)) {
                (Expr::Binary("**", a, b), Some(m)) if m > 0 => match (lit(a), lit(b)) {
                    (Some(a), Some(b)) if b >= 0 => {
                        let (mut r, mut base, mut k) = (1i128 % m as i128, (a as i128).rem_euclid(m as i128), b);
                        while k > 0 {
                            if k & 1 == 1 {
                                r = r * base % m as i128;
                            }
                            base = base * base % m as i128;
                            k >>= 1;
                        }
                        Some(r as i64)
                    }
                    _ => None,
                },
                _ => None,
            },
            Expr::Binary("**", a, b) => match (lit(a), lit(b)) {
                (Some(a), Some(b)) if (0..=u32::MAX as i64).contains(&b) => a.checked_pow(b as u32),
                _ => None,
            },
            _ => None,
        };
        if let Some(k) = folded {
            *x = Expr::Int(k);
        }
    });
}

pub fn promote(items: &mut [Item]) {
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            fold_pows(body);
        }
    }
    int_consts(items);
    big_fns(items);
    // Names each function reads: a top-level name read by another function must stay an int.
    let names: Vec<HashSet<String>> = items.iter().map(|it| match it {
        Item::Fn { body, .. } => idents(&format!("{body:?}")),
        _ => HashSet::new(),
    }).collect();
    for (i, it) in items.iter_mut().enumerate() {
        if let Item::Fn { name, params, body, ret, script, .. } = it {
            let mut keep: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            // A function's own locals are its own unless it declares them global; the top-level code's
            // names may be read by every function.
            if *script || format!("{body:?}").contains("__global") {
                for (j, ns) in names.iter().enumerate() {
                    if j != i {
                        keep.extend(ns.iter().cloned());
                    }
                }
            }
            if BIGF.with(|b| b.borrow().contains(name.as_str())) {
                let ps: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
                promote_ret(body, &keep, &ps);
                *ret = Ty::Named("big".into());
            } else {
                promote_body(body, &keep);
            }
        }
    }
}

// Names the top-level code sets once to an int literal and that nothing else sets.
fn int_consts(items: &[Item]) {
    let mut sets: HashMap<String, (usize, bool)> = HashMap::new();
    for it in items {
        let Item::Fn { body, script, .. } = it else { continue };
        let mut b = body.clone();
        crate::parser::walk_mut(&mut b, &mut |e| {
            if let Expr::Block(ss, _) = e {
                for st in ss {
                    let (n, lit) = match st {
                        Stmt::Let { name: Pat::Bind(n), value, .. } => (n.clone(), matches!(value, Expr::Int(_)) && *script),
                        Stmt::Assign { target: Expr::Ident(n), op, value, .. } => (n.clone(), *op == "=" && matches!(value, Expr::Int(_)) && *script),
                        _ => continue,
                    };
                    let en = sets.entry(n).or_insert((0, true));
                    en.0 += 1;
                    en.1 &= lit;
                }
            }
        });
    }
    let cs = sets.into_iter().filter(|(_, (k, lit))| *k == 1 && *lit).map(|(n, _)| n).collect();
    CONSTS.with(|c| *c.borrow_mut() = cs);
}

// Functions that return an int that can outgrow 64 bits (`f *= i ... return f`, `return n * fact(n - 1)`,
// Fibonacci) return a big integer. Every call of one is first written `f(x).int()`: where the call meets other
// big values, or is printed, the `.int()` becomes `.big()` (see from_text and conv) and the value stays big;
// anywhere else it is turned back into an int, which raises OverflowError if it does not fit.
fn big_fns(items: &mut [Item]) {
    let mut cands: HashSet<String> = items
        .iter()
        .filter_map(|it| match it {
            Item::Fn { name, ret: Ty::Named(t), script: false, .. } if t == "?" || t == "int" => Some(name.clone()),
            _ => None,
        })
        .collect();
    if cands.is_empty() {
        return;
    }
    let wrapped = cands.clone();
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            wrap_calls(body, &cands, true);
        }
    }
    let names: Vec<HashSet<String>> = items.iter().map(|it| match it {
        Item::Fn { body, .. } => idents(&format!("{body:?}")),
        _ => HashSet::new(),
    }).collect();
    loop {
        BIGF.with(|b| *b.borrow_mut() = cands.clone());
        let mut next = HashSet::new();
        for (i, it) in items.iter().enumerate() {
            let Item::Fn { name, params, body, script, .. } = it else { continue };
            if !cands.contains(name) {
                continue;
            }
            let ps: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            let mut keep = ps.clone();
            // A function's own locals are its own unless it declares them global; the top-level code's
            // names may be read by every function.
            if *script || format!("{body:?}").contains("__global") {
                for (j, ns) in names.iter().enumerate() {
                    if j != i {
                        keep.extend(ns.iter().cloned());
                    }
                }
            }
            let mut b = body.clone();
            if promote_ret(&mut b, &keep, &ps) {
                next.insert(name.clone());
            }
        }
        if next == cands {
            break;
        }
        cands = next;
        if cands.is_empty() {
            break;
        }
    }
    BIGF.with(|b| *b.borrow_mut() = cands.clone());
    // Calls of functions that stay plain ints lose the `.int()`.
    let dropped: HashSet<String> = wrapped.difference(&cands).cloned().collect();
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            unwrap_calls(body, &dropped);
        }
    }
}

thread_local! {
    static BIGF: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
    static RET_OK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn big_call(e: &Expr) -> bool {
    matches!(e, Expr::Call(f, _, _) if BIGF.with(|b| b.borrow().contains(f)))
}

fn wrap_calls(e: &mut Expr, fs: &HashSet<String>, _top: bool) {
    crate::parser::walk_mut(e, &mut |x| {
        if let Expr::Call(f, _, l) = x {
            if fs.contains(f.as_str()) {
                let l = *l;
                // Marked so the walk, which goes on into the new node, does not wrap it again.
                *f = format!("\u{1}{f}");
                let c = std::mem::replace(x, Expr::Int(0));
                *x = Expr::Method(Box::new(c), "int".into(), vec![], l);
            }
        }
    });
    crate::parser::walk_mut(e, &mut |x| {
        if let Expr::Call(f, _, _) = x {
            if let Some(n) = f.strip_prefix('\u{1}') {
                *f = n.to_string();
            }
        }
    });
}

// drop: functions whose calls were wrapped and that stay plain ints. (A call that was written `f(x).int()` in
// the source is left alone only if f returns a big integer; `int(f(x))` of an int is the same int.)
fn unwrap_calls(e: &mut Expr, drop: &HashSet<String>) {
    crate::parser::walk_mut(e, &mut |x| {
        if let Expr::Method(r, m, a, _) = x {
            if m == "int" && a.is_empty() && matches!(r.as_ref(), Expr::Call(f, _, _) if drop.contains(f)) {
                let inner = std::mem::replace(r.as_mut(), Expr::Int(0));
                *x = inner;
            }
        }
    });
}

fn returns<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Block(ss, last) => {
            for s in ss {
                match s {
                    Stmt::Return { value: Some(v), .. } => out.push(v),
                    Stmt::Return { value: None, .. } => out.push(&NONE_RET),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => returns(value, out),
                    Stmt::Expr { expr, .. } => returns(expr, out),
                    _ => {}
                }
            }
            if let Some(l) = last {
                returns(l, out);
            }
        }
        Expr::If(_, a, b) => {
            returns(a, out);
            if let Some(b) = b {
                returns(b, out);
            }
        }
        Expr::While(_, b) | Expr::For(_, _, b) => returns(b, out),
        Expr::Match(_, arms) => arms.iter().for_each(|(_, a)| returns(a, out)),
        _ => {}
    }
}
// Stands for a bare `return` (None), which no big function has.
static NONE_RET: Expr = Expr::Bool(false);

// A product or sum that keeps a big call's growth: `n * fact(n - 1)`, `fib(n - 1) + fib(n - 2)`.
fn big_growth(e: &Expr, c: &HashSet<String>) -> bool {
    let call = |x: &Expr| matches!(x, Expr::Method(r, m, a, _) if m == "int" && a.is_empty() && big_call(r));
    match e {
        Expr::Binary("*", a, b) => call(a) || call(b) || big_growth(a, c) || big_growth(b, c),
        Expr::Binary("+", a, b) if call(a) && call(b) => true,
        Expr::Binary(_, a, b) => big_growth(a, c) || big_growth(b, c),
        Expr::Unary(_, a) => big_growth(a, c),
        _ => mentions_name(e, c),
    }
}

// promote_body for a function that may return a big integer, with its parameters next to big values taken as
// ints made big. True when every return is a big value and one of them grows.
fn promote_ret(body: &mut Expr, keep: &HashSet<String>, params: &HashSet<String>) -> bool {
    RET_OK.with(|r| r.set(true));
    let c = promote_names(body, keep, params);
    RET_OK.with(|r| r.set(false));
    let mut rs = vec![];
    returns(body, &mut rs);
    // A function whose end can be reached returns None.
    let ok = !rs.is_empty() && ends_in_return(body) && rs.iter().all(|v| bigable(v, &c)) && rs.iter().any(|v| big_growth(v, &c));
    if ok {
        rewrite_returns(body, &c);
        rewrite(body, &c, false);
    }
    ok
}

fn ends_in_return(body: &Expr) -> bool {
    match body {
        Expr::Block(ss, None) => matches!(ss.last(), Some(Stmt::Return { value: Some(_), .. })) || matches!(ss.last(), Some(Stmt::Expr { expr, .. }) if ends_in_return(expr)),
        Expr::Block(_, Some(l)) => ends_in_return(l),
        Expr::If(_, a, Some(b)) => ends_in_return(a) && ends_in_return(b),
        _ => false,
    }
}

fn rewrite_returns(e: &mut Expr, c: &HashSet<String>) {
    match e {
        Expr::Block(ss, last) => {
            for s in ss.iter_mut() {
                match s {
                    Stmt::Return { value: Some(v), .. } => conv(v, c),
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => rewrite_returns(value, c),
                    Stmt::Expr { expr, .. } => rewrite_returns(expr, c),
                    _ => {}
                }
            }
            if let Some(l) = last {
                rewrite_returns(l, c);
            }
        }
        Expr::If(_, a, b) => {
            rewrite_returns(a, c);
            if let Some(b) = b {
                rewrite_returns(b, c);
            }
        }
        Expr::While(_, b) | Expr::For(_, _, b) => rewrite_returns(b, c),
        Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| rewrite_returns(a, c)),
        _ => {}
    }
}

fn idents(dbg: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for pre in ["Ident(\"", "Move(\"", "Bind(\""] {
        for (i, _) in dbg.match_indices(pre) {
            let rest = &dbg[i + pre.len()..];
            if let Some(end) = rest.find('"') {
                out.insert(rest[..end].to_string());
            }
        }
    }
    out
}

const ARITH: &[&str] = &["+", "-", "*", "//", "%"];
const CMP: &[&str] = &["==", "!=", "<", ">", "<=", ">="];

// int(x) / int(x, base) on something that reads as text: a literal, parts[0], s.strip(), input().
fn from_text(e: &Expr) -> bool {
    match e {
        Expr::Method(r, m, a, _) if (m == "int" && a.is_empty()) || (m == "int_base" && a.len() == 1) => text_arg(r),
        _ => false,
    }
}

// A value int(...) reads as text: a literal, parts[0], input(), s.strip().
fn text_arg(e: &Expr) -> bool {
    match e {
        Expr::Str(_) | Expr::Index(..) => true,
        Expr::Method(_, t, _, _) => t.starts_with("trim") || t == "lower" || t == "casefold" || t == "upper",
        Expr::Call(f, _, _) if BIGF.with(|b| b.borrow().contains(f)) => true,
        Expr::Call(f, a, _) => f == "input" && a.len() <= 1,
        Expr::Ident(n) | Expr::Move(n) => TEXT.with(|t| t.borrow().contains(n)),
        _ => false,
    }
}

// A value that reads as text: a literal, parts[0], input(), s.strip(), as text_arg takes them inside int(...).
fn text_value(e: &Expr) -> bool {
    match e {
        Expr::Str(_) | Expr::Index(..) => true,
        Expr::Method(_, t, _, _) => t.starts_with("trim") || t == "lower" || t == "casefold" || t == "upper",
        Expr::Call(f, a, _) => f == "input" && a.len() <= 1,
        _ => false,
    }
}

// Names that only ever hold small ints (a `for i in range(...)` loop's name): fine next to a big integer.
thread_local! {
    static SMALL: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
    // Locals only ever set to text (`a, b = parts[0], parts[1]`, `s = input()`), so int(a) reads text too.
    static TEXT: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
    // Top-level names only ever set to one int literal (`MOD = 10 ** 9 + 7`).
    static CONSTS: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
    // The parameters of a function being made to return a big integer (ints, made big where they meet one).
    static RET_PARAMS: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
}

// An int that cannot be big itself: a range loop's name, ord(c), len(xs).
fn small(e: &Expr) -> bool {
    match e {
        Expr::Ident(n) | Expr::Move(n) => SMALL.with(|s| s.borrow().contains(n)) || RET_PARAMS.with(|s| s.borrow().contains(n)) || CONSTS.with(|s| s.borrow().contains(n)),
        Expr::Method(_, m, a, _) => (m == "code" || m == "len") && a.is_empty(),
        _ => false,
    }
}

fn range_names(e: &Expr, out: &mut HashSet<String>) {
    let mut f = |x: &Expr| range_names(x, out);
    match e {
        Expr::For(Pat::Bind(n), xs, b) => {
            if matches!(xs.as_ref(), Expr::Range(..)) || matches!(xs.as_ref(), Expr::Method(r, m, _, _) if m == "step" && matches!(r.as_ref(), Expr::Range(..))) {
                out.insert(n.clone());
            }
            range_names(b, out);
        }
        _ => {
            let dbg = format!("{e:?}");
            if dbg.contains("For(") {
                each_sub(e, &mut f);
            }
        }
    }
}

fn each_sub(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    match e {
        Expr::Block(ss, last) => {
            for s in ss {
                match s {
                    Stmt::Let { value, .. } | Stmt::Assign { value, .. } => f(value),
                    Stmt::Expr { expr, .. } => f(expr),
                    Stmt::Return { value: Some(v), .. } => f(v),
                    _ => {}
                }
            }
            if let Some(l) = last {
                f(l);
            }
        }
        Expr::If(c, a, b) => {
            f(c);
            f(a);
            if let Some(b) = b {
                f(b);
            }
        }
        Expr::While(a, b) | Expr::For(_, a, b) => {
            f(a);
            f(b);
        }
        Expr::Match(x, arms) => {
            f(x);
            arms.iter().for_each(|(_, a)| f(a));
        }
        _ => {}
    }
}

// `f *= i`, `h = h * 31 + ord(c)`: a name multiplied by something and kept, which can outgrow 64 bits.
fn grows(n: &str, v: &Expr) -> bool {
    match v {
        Expr::Binary("*", a, b) => matches!(a.as_ref(), Expr::Ident(m) | Expr::Move(m) if m == n) || matches!(b.as_ref(), Expr::Ident(m) | Expr::Move(m) if m == n) || grows(n, a) || grows(n, b),
        // `a, b = b, a + b`: a sum of two names that are not small ints (Fibonacci numbers grow past 64 bits).
        // Also `a + b + c` (Tribonacci).
        Expr::Binary("+", ..) if {
            let mut leaves = vec![];
            sum_leaves(v, &mut leaves);
            leaves.len() >= 2 && leaves.iter().all(|x| matches!(x, Expr::Ident(_) | Expr::Move(_)) && !small(x)) && leaves.iter().any(|x| mentions_one(x, n))
        } => true,
        Expr::Binary("+" | "-", a, b) => grows(n, a) || grows(n, b),
        _ => false,
    }
}

fn sum_leaves<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Binary("+", a, b) => {
            sum_leaves(a, out);
            sum_leaves(b, out);
        }
        e => out.push(e),
    }
}

fn mentions_one(e: &Expr, n: &str) -> bool {
    matches!(e, Expr::Ident(m) | Expr::Move(m) if m == n)
}

fn bigable(e: &Expr, c: &HashSet<String>) -> bool {
    match e {
        e if small(e) => true,
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
                } else if ["+=", "-=", "*=", "//=", "%="].contains(op) {
                    // n += v is n = n + v.
                    let bop: &'static str = match *op {
                        "+=" => "+",
                        "-=" => "-",
                        "*=" => "*",
                        "//=" => "//",
                        _ => "%",
                    };
                    let whole = Expr::Binary(bop, Box::new(Expr::Ident(n.clone())), Box::new(value.clone()));
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
            Stmt::Return { value: Some(v), .. } => self.expr(v, RET_OK.with(|r| r.get())),
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
    let c = promote_names(body, params, &HashSet::new());
    rewrite(body, &c, false);
}

// The locals of body that become big integers. keep: names that must stay ints; ret_params: the parameters
// of a function returning a big integer.
fn promote_names(body: &Expr, params: &HashSet<String>, ret_params: &HashSet<String>) -> HashSet<String> {
    RET_PARAMS.with(|s| *s.borrow_mut() = ret_params.clone());
    let mut rn = HashSet::new();
    range_names(body, &mut rn);
    SMALL.with(|s| *s.borrow_mut() = rn);
    // Start from every name set from int(text) somewhere, then drop names with a use or a value that
    // cannot be big, until nothing changes.
    let mut seed = Scan { c: &HashSet::new(), bad: HashSet::new(), defs: HashMap::new() };
    seed.expr(body, false);
    let text: HashSet<String> = seed.defs.iter().filter(|(n, vs)| !params.contains(*n) && !vs.is_empty() && vs.iter().all(text_value)).map(|(n, _)| n.clone()).collect();
    TEXT.with(|t| *t.borrow_mut() = text);
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
                if seed.defs[n].iter().any(|v| has_text(v) || grows(n, v) || matches!(v, Expr::Int(k) if k.unsigned_abs() >= 1 << 40) || mentions_name(v, &reach)) {
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
    c
}

// Turns int(text) into big(text) and number literals next to promoted names into big numbers.
fn conv(e: &mut Expr, c: &HashSet<String>) {
    match e {
        Expr::Int(_) => {
            let n = std::mem::replace(e, Expr::Int(0));
            *e = Expr::Method(Box::new(n), "big".into(), vec![], 0);
        }
        e if small(e) && !matches!(&*e, Expr::Ident(n) | Expr::Move(n) if c.contains(n)) => {
            let n = std::mem::replace(e, Expr::Int(0));
            *e = Expr::Method(Box::new(n), "big".into(), vec![], 0);
        }
        e if from_text(e) => {
            if let Expr::Method(_, m, _, _) = e {
                *m = if m == "int_base" { "big_base" } else { "big" }.into();
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
        Expr::Method(r, m, args, _) if m == "py_repr" && args.is_empty() => printed(r, c),
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

// print(int(p[0]) + int(p[1])): sums of int(text) printed straight away are big too, as is a lone
// int(text) (print(int(s))), including inside repr() (print(repr(int(s, 0)))).
fn printed(e: &mut Expr, c: &HashSet<String>) {
    if from_text(e) || matches!(e, Expr::Binary(op, ..) if ARITH.contains(op)) && bigable(e, c) && mentions(e, c) {
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
