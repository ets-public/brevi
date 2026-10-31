// Ownership inference: a variable's last read moves the value instead of copying it.
// Reads that are safe to move become Expr::Move; everything else keeps copying.
use crate::ast::*;
use std::collections::{HashMap, HashSet};

struct Occ {
    name: String,
    value: bool,   // read in a spot where a copy would be made
    movable: bool, // not in a lambda, and not in a loop that the variable outlives
    depth: Option<usize>, // loop depth, or None inside a lambda
}

struct Walk<'a> {
    occs: Vec<Occ>,
    decl_depth: HashMap<String, usize>,
    loop_depth: usize,
    lambda: usize,
    // Indexes of reads that can move because the statement sets the variable again (`s = s + x`).
    reassign: Vec<usize>,
    mut_fns: &'a HashSet<String>,
    apply: Option<HashSet<usize>>,
}

pub fn mark_moves(items: &mut [Item]) {
    let mut_fns: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            Item::Fn { name, params, .. } if params.iter().any(|p| p.is_mut) => Some(name.clone()),
            _ => None,
        })
        .collect();
    for item in items.iter_mut() {
        let (params, body): (Vec<String>, &mut Expr) = match item {
            Item::Fn { params, body, .. } => (params.iter().filter(|p| !p.is_mut).map(|p| p.name.clone()).collect(), body),
            Item::Test { body, .. } => (vec![], body),
            _ => continue,
        };
        let mut w = Walk { occs: vec![], decl_depth: HashMap::new(), loop_depth: 0, lambda: 0, reassign: vec![], mut_fns: &mut_fns, apply: None };
        for p in &params {
            w.decl_depth.insert(p.clone(), 0);
        }
        w.expr(body, true);
        let mut last: HashMap<&str, usize> = HashMap::new();
        for (i, o) in w.occs.iter().enumerate() {
            last.insert(&o.name, i);
        }
        let mut chosen: HashSet<usize> = w.occs.iter().enumerate().filter(|(i, o)| o.value && o.movable && last[o.name.as_str()] == *i).map(|(i, _)| i).collect();
        chosen.extend(w.reassign.iter().copied().filter(|&i| w.occs[i].value));
        if chosen.is_empty() {
            continue;
        }
        let mut w2 = Walk { occs: vec![], decl_depth: HashMap::new(), loop_depth: 0, lambda: 0, reassign: vec![], mut_fns: &mut_fns, apply: Some(chosen) };
        for p in &params {
            w2.decl_depth.insert(p.clone(), 0);
        }
        w2.expr(body, true);
    }
}

impl Walk<'_> {
    fn declare(&mut self, n: &str) {
        let d = self.loop_depth;
        let e = self.decl_depth.entry(n.to_string()).or_insert(d);
        *e = (*e).min(d);
    }

    fn declare_pat(&mut self, p: &Pat) {
        match p {
            Pat::Bind(n) => self.declare(n),
            Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| self.declare_pat(p)),
            _ => {}
        }
    }

    fn ident(&mut self, e: &mut Expr, value: bool) {
        let Expr::Ident(n) = e else { return };
        let movable = self.lambda == 0 && self.decl_depth.get(n.as_str()).map_or(false, |&d| self.loop_depth <= d);
        let i = self.occs.len();
        let depth = if self.lambda == 0 { Some(self.loop_depth) } else { None };
        self.occs.push(Occ { name: n.clone(), value, movable, depth });
        if self.apply.as_ref().map_or(false, |s| s.contains(&i)) {
            *e = Expr::Move(n.clone());
        }
    }

    fn stmts(&mut self, stmts: &mut [Stmt]) {
        for st in stmts {
            match st {
                Stmt::Let { name, value, .. } => {
                    self.expr(value, true);
                    self.declare_pat(name);
                }
                Stmt::Assign { target, op, value, .. } => {
                    let start = self.occs.len();
                    self.expr(value, true);
                    if let (Expr::Ident(n), "=") = (&*target, *op) {
                        // The variable is set right after, so its last read in the value can move.
                        let (n, depth) = (n.clone(), Some(self.loop_depth));
                        if let Some(i) = (start..self.occs.len()).rev().find(|&i| self.occs[i].name == n) {
                            if self.lambda == 0 && self.occs[i].depth == depth {
                                self.reassign.push(i);
                            }
                        }
                        if !self.decl_depth.contains_key(&n) {
                            self.declare(&n);
                        }
                    }
                    if let Expr::Tuple(xs) = target {
                        for x in xs.iter() {
                            if let Expr::Ident(n) = x {
                                if !self.decl_depth.contains_key(n) {
                                    let n = n.clone();
                                    self.declare(&n);
                                }
                            }
                        }
                    }
                    match target {
                        Expr::Tuple(xs) => xs.iter_mut().for_each(|x| self.expr(x, false)),
                        t => self.expr(t, false),
                    }
                }
                Stmt::Expr { expr, .. } => self.expr(expr, true),
                Stmt::Return { value: Some(v), .. } => self.expr(v, true),
                Stmt::Assert { cond, .. } => self.expr(cond, true),
                _ => {}
            }
        }
    }

    fn expr(&mut self, e: &mut Expr, value: bool) {
        match e {
            Expr::Ident(_) => self.ident(e, value),
            Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Move(_) => {}
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.expr(x, true);
                    }
                }
            }
            Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| self.expr(x, true)),
            Expr::Map(ps) => {
                for (k, v) in ps {
                    self.expr(k, true);
                    self.expr(v, true);
                }
            }
            Expr::Unary(_, a) | Expr::Try(a) => self.expr(a, true),
            Expr::Binary(op, a, b) => {
                let v = !["==", "!=", "<", ">", "<=", ">=", "in"].contains(op);
                self.expr(a, v);
                self.expr(b, v);
            }
            Expr::Call(name, args, _) => {
                let v = !self.mut_fns.contains(name.as_str());
                for (_, a) in args {
                    self.expr(a, v);
                }
            }
            Expr::Method(recv, name, args, _) => {
                self.expr(recv, false);
                let ref_arg = ["has", "get_or", "contains", "index_of"].contains(&name.as_str());
                for (k, a) in args.iter_mut().enumerate() {
                    self.expr(a, !(k == 0 && ref_arg));
                }
            }
            Expr::Field(b, _) | Expr::TupleField(b, _) => self.expr(b, false),
            Expr::Index(b, i) => {
                self.expr(b, false);
                self.expr(i, false);
            }
            Expr::Lambda(ps, body) => {
                self.lambda += 1;
                for p in ps.clone() {
                    self.declare(&p);
                }
                self.expr(body, true);
                self.lambda -= 1;
            }
            Expr::If(c, t, f) => {
                self.expr(c, true);
                self.expr(t, true);
                if let Some(f) = f {
                    self.expr(f, true);
                }
            }
            Expr::Match(s, arms) => {
                self.expr(s, true);
                for (p, body) in arms {
                    self.declare_pat(p);
                    self.expr(body, true);
                }
            }
            Expr::For(p, iter, body) => {
                // A loop that assigns to its variable edits the list in place, so the list is not moved.
                let edits = matches!(p, Pat::Bind(n) if crate::emit::assigns_to(body, n));
                self.expr(iter, !edits);
                self.loop_depth += 1;
                self.declare_pat(p);
                self.expr(body, true);
                self.loop_depth -= 1;
            }
            Expr::While(c, body) => {
                self.loop_depth += 1;
                self.expr(c, true);
                self.expr(body, true);
                self.loop_depth -= 1;
            }
            Expr::Range(a, b) => {
                self.expr(a, true);
                self.expr(b, true);
            }
            Expr::Block(stmts, tail) => {
                self.stmts(stmts);
                if let Some(t) = tail {
                    self.expr(t, true);
                }
            }
        }
    }
}
