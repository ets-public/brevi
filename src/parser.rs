use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{lex, Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    // The function being parsed returns a map, so an arm `_ => {}` is an empty map.
    ret_map: bool,
    // Functions the program defines, which keep their own meaning over Python's len, sum, etc.
    user_fns: Vec<String>,
    // Functions written inside the function being parsed (Python's nested `def`), lifted to the top level.
    children: Vec<Item>,
    hoisted: Vec<Item>,
}

type R<T> = Result<T, Diag>;

pub fn parse_program(src: &str) -> R<Vec<Item>> {
    let toks = crate::lexer::indent_blocks(lex(src)?);
    let user_fns = toks.windows(2).filter_map(|w| match (&w[0].tok, &w[1].tok) {
        (Tok::Kw("fn"), Tok::Ident(n)) => Some(n.clone()),
        _ => None,
    }).collect();
    let mut p = Parser { toks, pos: 0, ret_map: false, user_fns, children: vec![], hoisted: vec![] };
    let mut items = Vec::new();
    // Statements outside any function form the program's main (script mode).
    let mut script = Vec::new();
    let mut script_line = 0;
    loop {
        p.skip_newlines();
        if p.peek() == &Tok::Eof {
            break;
        }
        if matches!(p.peek(), Tok::Kw("fn" | "type" | "enum" | "test")) {
            items.push(p.item()?);
        } else if p.error_class() {
            continue;
        } else {
            if script.is_empty() {
                script_line = p.line();
            }
            script.push(p.stmt()?);
            if p.peek() != &Tok::Newline {
                return Err(p.err(&format!("expected end of line, found {}", tok_text(p.peek()))));
            }
        }
    }
    items.append(&mut p.hoisted);
    if !script.is_empty() {
        // Top-level statements beside fn main run first, then main, as in a Python file that ends with main().
        let mut script = script;
        if let Some(Item::Fn { name, params, .. }) = items.iter_mut().find(|i| matches!(i, Item::Fn { name, .. } if name == "main")) {
            if !params.is_empty() {
                return Err(Diag::new("parse", script_line, 1, "main takes no parameters").fix_hint("use args() to read command line arguments"));
            }
            *name = "main_".into();
            // `main()`, also inside `if __name__ == "__main__":`.
            let mut called = false;
            for st in script.iter_mut() {
                if let Stmt::Expr { expr, .. } = st {
                    called |= rename_main(expr);
                }
            }
            if !called {
                let line = script.last().map_or(script_line, |_| script_line);
                script.push(Stmt::Expr { expr: Expr::Call("main_".into(), vec![], line), line });
            }
        }
        items.push(Item::Fn {
            name: "main".into(),
            params: vec![],
            ret: Ty::Unit,
            fallible: true,
            body: Expr::Block(script, None),
            line: script_line,
            script: true,
        });
    }
    Ok(items)
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Int(n) => n.to_string(),
        Tok::Float(f) => f.to_string(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Ident(s) => s.clone(),
        Tok::Kw(k) => k.to_string(),
        Tok::Sym(s) => s.to_string(),
        Tok::Newline => "end of line".into(),
        Tok::Eof => "end of file".into(),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }
    fn line(&self) -> usize {
        self.toks[self.pos].line
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn err(&self, msg: &str) -> Diag {
        let t = &self.toks[self.pos];
        Diag::new("parse", t.line, t.col, msg)
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }
    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Kw(x) if *x == s)
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.next();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, s: &str) -> bool {
        if self.is_kw(s) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(self.err(&format!("expected '{s}', found {}", tok_text(self.peek()))).fix_hint(&format!("insert '{s}'")))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected a name, found {}", tok_text(&t))))
            }
        }
    }
    fn skip_newlines(&mut self) {
        while self.peek() == &Tok::Newline {
            self.next();
        }
    }
    // Commas and newlines both separate list entries inside braces.
    fn skip_seps(&mut self) {
        while self.peek() == &Tok::Newline || self.is_sym(",") {
            self.next();
        }
    }

    fn item(&mut self) -> R<Item> {
        let line = self.line();
        if self.eat_kw("fn") {
            let name = self.ident()?;
            let name = if SHADOWABLE.contains(&name.as_str()) { format!("{name}_") } else { name };
            self.expect_sym("(")?;
            let mut params = Vec::new();
            while !self.is_sym(")") {
                let is_mut = self.eat_kw("mut");
                let pname = self.ident()?;
                // Without a type, the parameter takes the type of what the calls pass (see infer::returns).
                let ty = if self.eat_sym(":") { self.ty()? } else { Ty::Named("?".into()) };
                params.push(Param { name: pname, ty, is_mut });
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            // Without `->` the return type is worked out from the body (see infer::returns).
            let ret = if self.eat_sym("->") { self.ty()? } else { Ty::Named("?".into()) };
            let fallible = self.eat_sym("!");
            self.ret_map = matches!(ret, Ty::Map(..));
            let saved_children = std::mem::take(&mut self.children);
            let body = if self.eat_sym("=") {
                self.skip_newlines();
                self.expr()?
            } else if self.is_sym("{") || self.is_sym(":") {
                self.block()?
            } else {
                return Err(self.err("expected '=', ':' or '{' to start the function body"));
            };
            self.ret_map = false;
            let kids = std::mem::replace(&mut self.children, saved_children);
            let mut body = body;
            if !kids.is_empty() {
                let lifted = lift_nested(&name, &params, &mut body, kids);
                self.hoisted.extend(lifted);
            }
            return Ok(Item::Fn { name, params, ret, fallible, body, line, script: false });
        }
        if self.eat_kw("type") {
            let name = self.ident()?;
            self.expect_sym("{")?;
            let fields = self.fields("}")?;
            self.expect_sym("}")?;
            return Ok(Item::Type { name, fields, line });
        }
        if self.eat_kw("enum") {
            let name = self.ident()?;
            self.expect_sym("{")?;
            let mut variants = Vec::new();
            self.skip_seps();
            while !self.is_sym("}") {
                let vname = self.ident()?;
                let fields = if self.eat_sym("(") {
                    let f = self.fields(")")?;
                    self.expect_sym(")")?;
                    f
                } else {
                    Vec::new()
                };
                variants.push(Variant { name: vname, fields });
                self.skip_seps();
            }
            self.expect_sym("}")?;
            return Ok(Item::Enum { name, variants, line });
        }
        if self.eat_kw("test") {
            let name = match self.next() {
                Tok::Str(s) => s.replace("{{", "{").replace("}}", "}"),
                _ => return Err(self.err("expected a test name in quotes")),
            };
            let body = self.block()?;
            return Ok(Item::Test { name, body, line });
        }
        Err(self.err(&format!("expected fn, type, enum or test, found {}", tok_text(self.peek()))))
    }

    fn fields(&mut self, close: &str) -> R<Vec<Field>> {
        let mut fields = Vec::new();
        self.skip_seps();
        while !self.is_sym(close) {
            let name = self.ident()?;
            self.expect_sym(":")?;
            let ty = self.ty()?;
            let default = if self.eat_sym("=") { Some(self.expr()?) } else { None };
            fields.push(Field { name, ty, default });
            self.skip_seps();
        }
        Ok(fields)
    }

    fn ty(&mut self) -> R<Ty> {
        if self.eat_sym("[") {
            let t = self.ty()?;
            self.expect_sym("]")?;
            return Ok(Ty::List(Box::new(t)));
        }
        if self.eat_sym("{") {
            let k = self.ty()?;
            self.expect_sym(":")?;
            let v = self.ty()?;
            self.expect_sym("}")?;
            return Ok(Ty::Map(Box::new(k), Box::new(v)));
        }
        if self.eat_sym("(") {
            let mut ts = Vec::new();
            while !self.is_sym(")") {
                ts.push(self.ty()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            return Ok(if ts.is_empty() { Ty::Unit } else { Ty::Tuple(ts) });
        }
        let name = self.ident()?;
        if name == "heap" && self.eat_sym("[") {
            let t = self.ty()?;
            self.expect_sym("]")?;
            return Ok(Ty::Named(format!("AxHeap<{}>", crate::emit::rust_ty(&t))));
        }
        Ok(Ty::Named(name))
    }

    fn block(&mut self) -> R<Expr> {
        // `if c: stmt` on one line.
        if self.eat_sym(":") {
            let st = self.stmt()?;
            return Ok(match st {
                Stmt::Expr { expr, .. } => Expr::Block(vec![], Some(Box::new(expr))),
                st => Expr::Block(vec![st], None),
            });
        }
        self.expect_sym("{")?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat_sym("}") {
                break;
            }
            if self.peek() == &Tok::Eof {
                return Err(self.err("block is missing its closing '}'").fix_hint("insert '}'"));
            }
            stmts.push(self.stmt()?);
            if self.is_sym("!") {
                return Err(self.err("unexpected '!' after an expression")
                    .fix_hint("use ? to pass an error up; ! only goes after a function's return type"));
            }
            if !(self.peek() == &Tok::Newline || self.is_sym("}")) {
                return Err(self.err(&format!("expected end of line, found {}", tok_text(self.peek())))
                    .fix_hint("put one statement per line"));
            }
        }
        // The last expression statement is the block's value.
        let tail = match stmts.last() {
            Some(Stmt::Expr { .. }) => match stmts.pop() {
                Some(Stmt::Expr { expr, .. }) => Some(Box::new(expr)),
                _ => None,
            },
            _ => None,
        };
        Ok(Expr::Block(stmts, tail))
    }

    // Python's `class BadInput(Exception): pass`: Brevi errors are messages, so an error class needs nothing.
    // Skipped (true) when the class body holds only `pass`, `...` or a docstring.
    fn error_class(&mut self) -> bool {
        if !matches!(self.peek(), Tok::Ident(w) if w == "class") || !matches!(self.peek_at(1), Tok::Ident(_)) {
            return false;
        }
        let save = self.pos;
        self.pos += 2;
        if self.eat_sym("(") {
            let mut depth = 1;
            while depth > 0 && self.peek() != &Tok::Eof {
                match self.next() {
                    Tok::Sym("(") => depth += 1,
                    Tok::Sym(")") => depth -= 1,
                    _ => {}
                }
            }
        }
        let kids = self.children.len();
        let trivial = match self.block() {
            Ok(Expr::Block(st, t)) => {
                st.iter().all(|s| matches!(s, Stmt::Expr { expr: Expr::Block(b, None), .. } if b.is_empty()) || matches!(s, Stmt::Expr { expr: Expr::Str(_), .. }))
                    && t.map_or(true, |t| matches!(*t, Expr::Str(_) | Expr::Block(..) | Expr::Range(..)))
            }
            _ => false,
        };
        self.children.truncate(kids);
        if !trivial {
            self.pos = save;
        }
        trivial
    }

    fn stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        // Python's `del m[k]` and `del xs[i]`.
        if matches!(self.peek(), Tok::Ident(w) if w == "del") && !matches!(self.peek_at(1), Tok::Newline | Tok::Sym("=" | "(" | "." | "[")) {
            self.next();
            let e = self.expr()?;
            return match e {
                Expr::Index(b, k) => Ok(Stmt::Expr { expr: Expr::Method(b, "remove".into(), vec![*k], line), line }),
                _ => Err(self.err("del takes m[k] or xs[i]").fix_hint("write del m[k]")),
            };
        }
        // Python's `pass`.
        if matches!(self.peek(), Tok::Ident(w) if w == "pass") && matches!(self.peek_at(1), Tok::Newline | Tok::Sym("}")) {
            self.next();
            return Ok(Stmt::Expr { expr: Expr::Block(vec![], None), line });
        }
        // Python's nested `def`: lifted out of the function (see lift_nested).
        if self.is_kw("fn") && matches!(self.peek_at(1), Tok::Ident(_)) {
            let f = self.item()?;
            self.children.push(f);
            return Ok(Stmt::Expr { expr: Expr::Block(vec![], None), line });
        }
        // Python's `global count` / `nonlocal count`. A program that uses them gets Python's scope rule
        // exactly: a function that sets a top-level name without declaring it global has its own local.
        if matches!(self.peek(), Tok::Ident(w) if w == "global" || w == "nonlocal") && matches!(self.peek_at(1), Tok::Ident(_)) {
            self.next();
            let mut names = Vec::new();
            while let Tok::Ident(n) = self.peek().clone() {
                self.next();
                names.push((None, Expr::Ident(n)));
                if !self.eat_sym(",") {
                    break;
                }
            }
            return Ok(Stmt::Expr { expr: Expr::Call("__global".into(), names, line), line });
        }
        // Python's try/except/else/finally: `__try(body, handler, name)`. An error raised in body (a failing call or
        // `raise`) runs handler with `name` set to its message; else runs after a body without error, finally after both.
        if matches!(self.peek(), Tok::Ident(w) if w == "try_") && matches!(self.peek_at(1), Tok::Sym("{" | ":")) {
            self.next();
            let body = self.block()?;
            let mut handler: Option<Expr> = None;
            let mut err_name = String::from("_");
            let mut els: Option<Expr> = None;
            let mut fin: Option<Expr> = None;
            loop {
                let save = self.pos;
                self.skip_newlines();
                if matches!(self.peek(), Tok::Ident(w) if w == "except") {
                    self.next();
                    let mut depth = 0;
                    let mut name = None;
                    while !(depth == 0 && matches!(self.peek(), Tok::Sym("{" | ":"))) && self.peek() != &Tok::Eof && self.peek() != &Tok::Newline {
                        match self.next() {
                            Tok::Sym("(" | "[") => depth += 1,
                            Tok::Sym(")" | "]") => depth -= 1,
                            Tok::Ident(w) if w == "as_" => {
                                if let Tok::Ident(n) = self.peek().clone() {
                                    name = Some(n);
                                }
                            }
                            _ => {}
                        }
                    }
                    let h = self.block()?;
                    // Brevi errors carry only a message, so the first handler serves every error.
                    if handler.is_none() {
                        handler = Some(h);
                        if let Some(n) = name {
                            err_name = n;
                        }
                    }
                } else if self.is_kw("else") && handler.is_some() {
                    self.next();
                    els = Some(self.block()?);
                } else if matches!(self.peek(), Tok::Ident(w) if w == "finally") {
                    self.next();
                    fin = Some(self.block()?);
                } else {
                    self.pos = save;
                    break;
                }
            }
            let mut body = body;
            if let Some(Expr::Block(es, et)) = els {
                if let Expr::Block(bs, bt) = &mut body {
                    if let Some(t) = bt.take() {
                        bs.push(Stmt::Expr { expr: *t, line });
                    }
                    bs.extend(es);
                    *bt = et;
                }
            }
            // An if whose condition is `__try()`: every pass over branches (types, names set on some paths) applies as is.
            let mut handler = match handler {
                Some(Expr::Block(st, t)) => (st, t),
                Some(other) => (vec![], Some(Box::new(other))),
                None => (vec![], None),
            };
            if err_name != "_" {
                handler.0.insert(0, Stmt::Let { name: Pat::Bind(err_name), ty: None, value: Expr::Call("__err".into(), vec![], line), mutable: false, line });
            }
            let handler = Expr::Block(handler.0, handler.1);
            let t = Expr::If(Box::new(Expr::Call("__try".into(), vec![], line)), Box::new(body), Some(Box::new(handler)));
            return Ok(match fin {
                Some(Expr::Block(mut fs, ft)) => {
                    fs.insert(0, Stmt::Expr { expr: t, line });
                    Stmt::Expr { expr: Expr::Block(fs, ft), line }
                }
                _ => Stmt::Expr { expr: t, line },
            });
        }
        // Python's `raise ValueError("bad")` is fail("bad"); a bare `raise` or `raise KeyError` fails with the name.
        if matches!(self.peek(), Tok::Ident(w) if w == "raise") {
            self.next();
            let msg = if self.peek() == &Tok::Newline || self.is_sym("}") {
                Expr::Str(vec![StrPart::Lit("error".into())])
            } else {
                let e = self.expr()?;
                if let Tok::Ident(w) = self.peek().clone() {
                    if w == "from" {
                        self.next();
                        self.expr()?;
                    }
                }
                match e {
                    Expr::Call(n, mut args, _) if n.chars().next().map_or(false, |c| c.is_uppercase()) => {
                        if args.is_empty() { Expr::Str(vec![StrPart::Lit(n)]) } else { Expr::Call("str".into(), vec![(None, args.remove(0).1)], line) }
                    }
                    Expr::Ident(n) => Expr::Str(vec![StrPart::Lit(n)]),
                    other => Expr::Call("str".into(), vec![(None, other)], line),
                }
            };
            return Ok(Stmt::Expr { expr: Expr::Call("fail".into(), vec![(None, msg)], line), line });
        }
        if self.is_kw("let") || self.is_kw("var") {
            let mutable = self.next() == Tok::Kw("var");
            let name = bind_upper(self.pat()?);
            let ty = if self.eat_sym(":") { Some(self.ty()?) } else { None };
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Let { name, ty, value, mutable, line });
        }
        // `x: T = v` declares a typed variable, like `var x: T = v`.
        if matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym(":")) {
            let name = bind_upper(self.pat()?);
            self.expect_sym(":")?;
            let ty = Some(self.ty()?);
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Let { name, ty, value, mutable: true, line });
        }
        if self.eat_kw("return") {
            let mut value = if self.peek() == &Tok::Newline || self.is_sym("}") { None } else { Some(self.expr()?) };
            // Python's `return a, b` returns a tuple.
            if value.is_some() && self.is_sym(",") {
                let mut xs = vec![value.take().unwrap()];
                while self.eat_sym(",") {
                    xs.push(self.expr()?);
                }
                value = Some(Expr::Tuple(xs));
            }
            return Ok(Stmt::Return { value, line });
        }
        if self.eat_kw("break") {
            return Ok(Stmt::Break { line });
        }
        if self.eat_kw("continue") {
            return Ok(Stmt::Continue { line });
        }
        if self.eat_kw("assert") {
            return Ok(Stmt::Assert { cond: self.expr()?, line });
        }
        let mut e = self.expr()?;
        // `a, b = ...` assigns a tuple, like `(a, b) = ...`.
        let mut bare = false;
        if matches!(e, Expr::Ident(_)) && self.is_sym(",") && self.bare_targets() {
            let mut xs = vec![e];
            while self.eat_sym(",") {
                xs.push(self.postfix()?);
            }
            e = Expr::Tuple(xs);
            bare = true;
        }
        // Python's `x |= m`, `x //= 2`, `x <<= 1` and the like: `x = x | m`.
        for op in ["//=", "%=", "|=", "&=", "^=", "<<=", ">>="] {
            if self.eat_sym(op) {
                self.skip_newlines();
                let value = self.expr()?;
                let bin: &'static str = match op {
                    "//=" => "//",
                    "%=" => "%",
                    "|=" => "|",
                    "&=" => "&",
                    "^=" => "^",
                    "<<=" => "<<",
                    _ => ">>",
                };
                return Ok(Stmt::Assign { target: e.clone(), op: "=", value: Expr::Binary(bin, Box::new(e), Box::new(value)), line });
            }
        }
        for op in ["=", "+=", "-=", "*=", "/="] {
            if self.eat_sym(op) {
                let op: &'static str = match op {
                    "=" => "=",
                    "+=" => "+=",
                    "-=" => "-=",
                    "*=" => "*=",
                    _ => "/=",
                };
                self.skip_newlines();
                let mut value = self.expr()?;
                // Python's `a = b = 0` sets each name.
                if op == "=" && matches!(e, Expr::Ident(_)) && matches!(value, Expr::Ident(_)) && self.is_sym("=") {
                    let mut names = vec![e.clone(), value];
                    while self.eat_sym("=") {
                        names.push(self.expr()?);
                    }
                    let v = names.pop().unwrap();
                    if names.iter().all(|n| matches!(n, Expr::Ident(_))) {
                        let vs = vec![v; names.len()];
                        return Ok(Stmt::Assign { target: Expr::Tuple(names), op, value: Expr::Tuple(vs), line });
                    }
                    return Err(self.err("expected names before each ="));
                }
                if bare && self.is_sym(",") {
                    let mut vs = vec![value];
                    while self.eat_sym(",") {
                        vs.push(self.expr()?);
                    }
                    value = Expr::Tuple(vs);
                }
                return Ok(Stmt::Assign { target: e, op, value, line });
            }
        }
        // Python's `xs.extend(ys)` is `xs += ys`; `s.add(x)` on a set() list adds x unless it is there.
        match e {
            Expr::Method(recv, m, mut a, _) if m == "extend" && a.len() == 1 => Ok(Stmt::Assign { target: *recv, op: "+=", value: a.remove(0), line }),
            Expr::Method(recv, m, mut a, l) if m == "add" && a.len() == 1 => {
                let x = a.remove(0);
                let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "contains".into(), vec![x.clone()], l)));
                let push = Expr::Method(recv, "push".into(), vec![x], l);
                Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![Stmt::Expr { expr: push, line }], None)), None), line })
            }
            e => Ok(Stmt::Expr { expr: e, line }),
        }
    }

    // After a name: is this `, b, c =`?
    fn bare_targets(&self) -> bool {
        let mut k = 0;
        loop {
            match (self.peek_at(k), self.peek_at(k + 1)) {
                (Tok::Sym(","), Tok::Ident(_)) => k += 2,
                (Tok::Sym("="), _) => return k > 0,
                _ => return false,
            }
        }
    }

    fn pat(&mut self) -> R<Pat> {
        match self.next() {
            Tok::Ident(s) if s == "_" => Ok(Pat::Wild),
            Tok::Ident(s) => {
                if s.chars().next().unwrap().is_uppercase() {
                    let mut args = Vec::new();
                    if self.eat_sym("(") {
                        while !self.is_sym(")") {
                            args.push(self.pat()?);
                            if !self.eat_sym(",") {
                                break;
                            }
                        }
                        self.expect_sym(")")?;
                    }
                    Ok(Pat::Variant(s, args))
                } else {
                    Ok(Pat::Bind(s))
                }
            }
            Tok::Int(n) => Ok(Pat::Int(n)),
            Tok::Sym("-") => match self.next() {
                Tok::Int(n) => Ok(Pat::Int(-n)),
                _ => Err(self.err("expected a number after '-'")),
            },
            Tok::Str(s) => Ok(Pat::Str(s.replace("{{", "{").replace("}}", "}"))),
            Tok::Kw("true") => Ok(Pat::Bool(true)),
            Tok::Kw("false") => Ok(Pat::Bool(false)),
            Tok::Sym("(") => {
                let mut ps = Vec::new();
                while !self.is_sym(")") {
                    ps.push(self.pat()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
                Ok(Pat::Tuple(ps))
            }
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected a pattern, found {}", tok_text(&t))))
            }
        }
    }

    pub fn expr(&mut self) -> R<Expr> {
        let e = self.binary(0)?;
        // Python's `a if c else b`; without the `else`, the `if` belongs to a comprehension.
        if self.is_kw("if") {
            let save = self.pos;
            self.next();
            if let Ok(c) = self.binary(0) {
                if self.eat_kw("else") {
                    let other = self.expr()?;
                    return Ok(Expr::If(Box::new(c), Box::new(Expr::Block(vec![], Some(Box::new(e)))), Some(Box::new(Expr::Block(vec![], Some(Box::new(other)))))));
                }
            }
            self.pos = save;
        }
        Ok(e)
    }

    fn binary(&mut self, level: usize) -> R<Expr> {
        const LEVELS: &[&[&str]] = &[
            &["or"],
            &["and"],
            &["==", "!=", "<", ">", "<=", ">=", "~=", "in"],
            &[".."],
            &["|"],
            &["^"],
            &["&"],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "%", "//"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
            // `x not in xs` is `!(x in xs)`.
            let negate = level == 2 && self.peek() == &Tok::Kw("not") && self.peek_at(1) == &Tok::Kw("in");
            if negate {
                self.next();
            }
            let op = match self.peek() {
                Tok::Sym(s) if LEVELS[level].contains(s) => *s,
                Tok::Kw(k) if LEVELS[level].contains(k) => *k,
                _ => break,
            };
            self.next();
            self.skip_newlines();
            let rhs = self.binary(level + 1)?;
            let empty = |e: &Expr| matches!(e, Expr::List(xs) if xs.is_empty()) || matches!(e, Expr::Map(m) if m.is_empty());
            lhs = if op == ".." {
                Expr::Range(Box::new(lhs), Box::new(rhs))
            } else if (op == "==" || op == "!=") && (empty(&lhs) || empty(&rhs)) {
                // `xs == []` and `m != {}` test for empty.
                let x = if empty(&rhs) { lhs } else { rhs };
                let t = Expr::Method(Box::new(x), "is_empty".into(), vec![], self.line());
                if op == "==" { t } else { Expr::Unary("!", Box::new(t)) }
            } else if (op == "==" || op == "!=") && [&lhs, &rhs].iter().any(|e| matches!(e, Expr::Call(n, a, _) if n == "none" && a.is_empty())) {
                // `x is None` / `x == None`.
                let x = if matches!(&rhs, Expr::Call(n, _, _) if n == "none") { lhs } else { rhs };
                let t = Expr::Method(Box::new(x), "is_none".into(), vec![], self.line());
                if op == "==" { t } else { Expr::Unary("!", Box::new(t)) }
            } else if let (true, Expr::Str(parts)) = (op == "%", &lhs) {
                // Python's `"%d of %s" % (a, b)` fills the string.
                let args = match rhs.clone() {
                    Expr::Tuple(xs) => xs,
                    x => vec![x],
                };
                match fill_format(parts, args, true, self.line()) {
                    Some(p) => Expr::Str(p),
                    None => Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                }
            } else if op == "*" && matches!(lhs, Expr::List(_)) {
                // Python's `[0] * n` repeats the list.
                Expr::Method(Box::new(lhs), "repeat".into(), vec![rhs], self.line())
            } else if op == "*" && matches!(rhs, Expr::List(_)) {
                Expr::Method(Box::new(rhs), "repeat".into(), vec![lhs], self.line())
            } else {
                Expr::Binary(op, Box::new(lhs), Box::new(rhs))
            };
            if negate {
                lhs = Expr::Unary("!", Box::new(lhs));
            }
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> R<Expr> {
        if self.eat_sym("-") {
            return Ok(Expr::Unary("-", Box::new(self.unary()?)));
        }
        // `not` is accepted as a spelling of `!`.
        if self.eat_sym("!") || self.eat_kw("not") {
            return Ok(Expr::Unary("!", Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            // A line starting with '.' continues a method chain.
            if self.peek() == &Tok::Newline {
                let mut k = 0;
                while self.peek_at(k) == &Tok::Newline {
                    k += 1;
                }
                if self.peek_at(k) == &Tok::Sym(".") {
                    self.skip_newlines();
                } else {
                    break;
                }
            }
            let line = self.line();
            if self.eat_sym(".") {
                match self.next() {
                    Tok::Int(n) => e = Expr::TupleField(Box::new(e), n as usize),
                    t @ (Tok::Ident(_) | Tok::Kw("or" | "match" | "in" | "not" | "and" | "type" | "test")) => {
                        let name = match t {
                            Tok::Ident(n) => n,
                            Tok::Kw(k) => k.to_string(),
                            _ => unreachable!(),
                        };
                        if self.eat_sym("(") {
                            let raw = self.args()?;
                            // Python's d.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP): half to even unless ROUND_HALF_UP.
                            if name == "quantize" && !raw.is_empty() {
                                let up = raw.iter().any(|a| a.0.as_deref() == Some("rounding") && matches!(&a.1, Expr::Ident(r) if r == "ROUND_HALF_UP"))
                                    || raw.get(1).is_some_and(|a| a.0.is_none() && matches!(&a.1, Expr::Ident(r) if r == "ROUND_HALF_UP"));
                                e = Expr::Method(Box::new(e), name, vec![raw[0].1.clone(), Expr::Bool(up)], line);
                                continue;
                            }
                            if matches!(name.as_str(), "sort" | "sort_by" | "sorted") {
                                if let Some(s) = sort_kw(e.clone(), &raw, line) {
                                    e = s;
                                    continue;
                                }
                            }
                            let mut args: Vec<Expr> = raw.into_iter().map(|(_, a)| a).collect();
                            let name = python_method(&name, args.len()).unwrap_or(name);
                            // Python's `"{} of {:>5}".format(a, b)` fills the string.
                            if name == "format" {
                                if let Expr::Str(parts) = &e {
                                    if let Some(p) = fill_format(parts, args.clone(), false, line) {
                                        e = Expr::Str(p);
                                        continue;
                                    }
                                }
                            }
                            // csv.reader(sys.stdin), csv.reader(lines) or csv.reader(text.splitlines()) give the rows.
                            if name == "reader" && args.len() == 1 && matches!(&e, Expr::Ident(m) if m == "csv") {
                                let src = match args.remove(0) {
                                    Expr::Call(f, a, l) if f == "lines" && a.is_empty() => Expr::Call("stdin".into(), vec![], l),
                                    other => other,
                                };
                                e = Expr::Method(Box::new(src), "csv_rows".into(), vec![], line);
                                continue;
                            }
                            // date.fromisoformat(s) and datetime.date.fromisoformat(s) are date(s).
                            if name == "fromisoformat" && args.len() == 1 && matches!(&e, Expr::Ident(m) if m == "date") || matches!(&e, Expr::Field(m, f) if f == "date" && matches!(m.as_ref(), Expr::Ident(d) if d == "datetime")) && name == "fromisoformat" {
                                e = Expr::Call("date".into(), vec![(None, args.remove(0))], line);
                                continue;
                            }
                            // sys.stdout.write(s) prints s as it is.
                            if name == "write" && args.len() == 1 && matches!(&e, Expr::Field(m, f) if f == "stdout" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys")) {
                                e = Expr::Call("print".into(), vec![(None, args.remove(0)), (Some("end".into()), Expr::Str(vec![]))], line);
                                continue;
                            }
                            // A deque is a list: popleft() takes the first item, appendleft(x) puts one in front.
                            if name == "popleft" && args.is_empty() {
                                e = Expr::Method(Box::new(e), "remove".into(), vec![Expr::Int(0)], line);
                                continue;
                            }
                            if name == "appendleft" && args.len() == 1 {
                                e = Expr::Method(Box::new(e), "insert".into(), vec![Expr::Int(0), args.remove(0)], line);
                                continue;
                            }
                            // heapq.heappush(h, x), heapq.heappop(h), heapq.heapify(h) work on a plain list, as in Python.
                            if matches!(&e, Expr::Ident(m) if m == "heapq") && !args.is_empty() {
                                if let Some(m) = heapq_method(&name, args.len()) {
                                    let h = args.remove(0);
                                    e = Expr::Method(Box::new(h), m.into(), args, line);
                                    continue;
                                }
                            }
                            // math.sqrt(x), math.floor(x), math.ceil(x); sys.stdin.read() is stdin().
                            if matches!(&e, Expr::Ident(m) if m == "math") && args.len() == 1 && matches!(name.as_str(), "sqrt" | "floor" | "ceil") {
                                let x = args.remove(0);
                                let f = Expr::Method(Box::new(Expr::Method(Box::new(x), "float".into(), vec![], line)), name.clone(), vec![], line);
                                e = if name == "sqrt" { f } else { Expr::Method(Box::new(f), "int".into(), vec![], line) };
                                continue;
                            }
                            if matches!(&e, Expr::Call(f, a, _) if f == "lines" && a.is_empty()) && name == "read" && args.is_empty() {
                                e = Expr::Call("stdin".into(), vec![], line);
                                continue;
                            }
                            // `m.setdefault(k, [])` is `m[k]`: a missing key reads as the empty value.
                            if name == "setdefault" && args.len() == 2 && empty_literal(&args[1]) {
                                e = Expr::Index(Box::new(e), Box::new(args.remove(0)));
                                continue;
                            }
                            // `xs.to_list()` / `xs.copy()`: a list already is one, so this is a copy.
                            let name = if matches!(name.as_str(), "to_list" | "to_vec" | "copy" | "collect") && args.is_empty() {
                                args.push(Expr::Int(0));
                                "skip".to_string()
                            } else {
                                name
                            };
                            // `n.range()` counts 0..n.
                            if name == "range" && args.is_empty() {
                                e = Expr::Range(Box::new(Expr::Int(0)), Box::new(e));
                                continue;
                            }
                            // Python's `", ".join(xs)` is `xs.join(", ")`.
                            e = if name == "join" && args.len() == 1 && matches!(e, Expr::Str(_)) && !matches!(args[0], Expr::Str(_)) {
                                Expr::Method(Box::new(args.remove(0)), name, vec![e], line)
                            } else {
                                Expr::Method(Box::new(e), name, args, line)
                            };
                        } else if name == "stdin" && matches!(&e, Expr::Ident(m) if m == "sys") {
                            e = Expr::Call("lines".into(), vec![], line);
                        } else {
                            e = Expr::Field(Box::new(e), name);
                        }
                    }
                    _ => return Err(self.err("expected a field or method name after '.'")),
                }
            } else if self.eat_sym("[") {
                // `xs[a:b]`, `xs[a:]`, `xs[:b]` slice like `slice(a, b)`.
                let line = self.line();
                let start = if self.is_sym(":") { None } else { Some(self.expr()?) };
                if self.eat_sym(":") {
                    let mut args = vec![start.clone().unwrap_or(Expr::Int(0))];
                    if !self.is_sym("]") && !self.is_sym(":") {
                        args.push(self.expr()?);
                    }
                    // Python's step: `xs[::-1]` reverses, `xs[::2]` takes every second item.
                    let step = if self.eat_sym(":") && !self.is_sym("]") { Some(self.expr()?) } else { None };
                    self.expect_sym("]")?;
                    if start.is_some() || args.len() > 1 || step.is_none() {
                        e = Expr::Method(Box::new(e), "slice".into(), args, line);
                    }
                    match step {
                        Some(Expr::Unary("-", n)) if matches!(*n, Expr::Int(1)) => e = Expr::Method(Box::new(e), "rev".into(), vec![], line),
                        Some(Expr::Int(-1)) => e = Expr::Method(Box::new(e), "rev".into(), vec![], line),
                        Some(Expr::Int(1)) | None => {}
                        Some(k) => e = Expr::Method(Box::new(e), "step".into(), vec![k], line),
                    }
                } else {
                    self.expect_sym("]")?;
                    let Some(idx) = start else { return Err(self.err("expected an index")) };
                    e = Expr::Index(Box::new(e), Box::new(idx));
                }
            } else if self.is_sym("(") && matches!(e, Expr::Index(..) | Expr::Field(..) | Expr::TupleField(..)) {
                // `ops[1](3, 4)`: call a function value taken from a list, tuple or field.
                self.eat_sym("(");
                let args = self.args()?.into_iter().map(|(_, a)| a).collect();
                e = Expr::Method(Box::new(e), "__call".into(), args, line);
            } else if self.eat_sym("?") {
                e = Expr::Try(Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    // Arguments after '(' up to and including ')'. Supports name: value.
    fn args(&mut self) -> R<Vec<(Option<String>, Expr)>> {
        let mut args = Vec::new();
        while !self.is_sym(")") {
            let name = match (self.peek().clone(), self.peek_at(1).clone()) {
                // Python keywords: `name=value` (`key=f`, `reverse=True`).
                (Tok::Ident(n), Tok::Sym(":")) | (Tok::Ident(n), Tok::Sym("=")) => {
                    self.next();
                    self.next();
                    Some(n)
                }
                _ => None,
            };
            // `sum(*row for row in grid)` (Python 3.15) passes the joined lists.
            let flat = self.eat_sym("*");
            let e = self.expr()?;
            // `min(x for x in xs)` passes the list the loop builds.
            let e = if self.eat_kw("for") {
                self.flat_comprehension(e, flat)?
            } else if flat {
                // Python's `print(*xs)`: each item is its own argument.
                args.push((Some("__star".into()), e));
                if !self.eat_sym(",") {
                    break;
                }
                continue;
            } else {
                e
            };
            args.push((name, e));
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(args)
    }

    // The rest of `elem for x in xs if c`, as xs.filter(x => c).map(x => elem).
    fn comprehension(&mut self, elem: Expr) -> R<Expr> {
        let line = self.line();
        let paren = self.eat_sym("(");
        let mut names = vec![self.ident()?];
        while self.eat_sym(",") {
            names.push(self.ident()?);
        }
        if paren {
            self.expect_sym(")")?;
        }
        if !self.eat_kw("in") {
            return Err(self.err("expected in").fix_hint("write [x * 2 for x in xs]"));
        }
        let mut e = self.expr()?;
        // `x for a in A for x in a`: each a gives a list; they are joined.
        if self.eat_kw("for") {
            let inner = self.comprehension(elem)?;
            return Ok(Expr::Method(Box::new(Expr::Method(Box::new(e), "map".into(), vec![Expr::Lambda(names, Box::new(inner))], line)), "flatten".into(), vec![], line));
        }
        if self.eat_kw("if") {
            // The condition uses Python truth (`if s.trim()`): `not not c` reads it that way.
            let c = self.expr()?;
            let c = if matches!(c, Expr::Binary(..) | Expr::Bool(_)) { c } else { Expr::Unary("!", Box::new(Expr::Unary("!", Box::new(c)))) };
            e = Expr::Method(Box::new(e), "filter".into(), vec![Expr::Lambda(names.clone(), Box::new(c))], line);
        }
        if !matches!(&elem, Expr::Ident(n) if names.len() == 1 && n == &names[0]) {
            e = Expr::Method(Box::new(e), "map".into(), vec![Expr::Lambda(names, Box::new(elem))], line);
        }
        Ok(e)
    }

    // `[*xs for xs in xss]` (Python 3.15) joins the lists, or the characters of strings.
    fn flat_comprehension(&mut self, elem: Expr, flat: bool) -> R<Expr> {
        let line = self.line();
        let e = self.comprehension(elem)?;
        Ok(if flat { Expr::Method(Box::new(e), "flatten".into(), vec![], line) } else { e })
    }

    // Is the '(' at the cursor the start of a lambda's parameter list?
    fn paren_is_lambda(&self) -> bool {
        let mut k = 1;
        loop {
            match self.peek_at(k) {
                Tok::Ident(_) | Tok::Sym(",") => k += 1,
                Tok::Sym(")") => return self.peek_at(k + 1) == &Tok::Sym("=>"),
                _ => return false,
            }
        }
    }

    fn primary(&mut self) -> R<Expr> {
        let line = self.line();
        if let (Tok::Ident(name), Tok::Sym("=>")) = (self.peek().clone(), self.peek_at(1).clone()) {
            self.next();
            self.next();
            let body = self.expr()?;
            return Ok(Expr::Lambda(vec![name], Box::new(body)));
        }
        if self.is_sym("(") && self.paren_is_lambda() {
            self.next();
            let mut names = Vec::new();
            while !self.is_sym(")") {
                names.push(self.ident()?);
                self.eat_sym(",");
            }
            self.next();
            self.expect_sym("=>")?;
            let body = self.expr()?;
            return Ok(Expr::Lambda(names, Box::new(body)));
        }
        match self.next() {
            Tok::Int(n) => Ok(Expr::Int(n)),
            Tok::Float(f) => Ok(Expr::Float(f)),
            Tok::Kw("true") => Ok(Expr::Bool(true)),
            Tok::Kw("false") => Ok(Expr::Bool(false)),
            Tok::Str(s) => Ok(Expr::Str(self.interp(&s, line)?)),
            // Python's `lambda x, y: expr`.
            Tok::Ident(name) if name == "lambda" && matches!(self.peek(), Tok::Ident(_) | Tok::Sym(":")) => {
                let mut names = Vec::new();
                while let Tok::Ident(n) = self.peek().clone() {
                    self.next();
                    names.push(n);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(":")?;
                let body = self.expr()?;
                Ok(Expr::Lambda(names, Box::new(body)))
            }
            // Python's `if __name__ == "__main__":` is always true here.
            Tok::Ident(name) if name == "__name__" => Ok(Expr::Str(vec![StrPart::Lit("__main__".into())])),
            // Python's None: an int, float, str or tuple slot that is not set yet.
            Tok::Ident(name) if name == "None" && !self.is_sym("(") => Ok(Expr::Call("none".into(), vec![], line)),
            Tok::Ident(name) => {
                if self.eat_sym("(") {
                    let mut args = self.args()?;
                    // Python's len(xs), sum(xs), sorted(xs), min(xs), max(xs) are the methods.
                    let m = match name.as_str() {
                        "len" | "sum" | "min" | "max" | "enumerate" | "int" | "float" => Some(name.as_str()),
                        "sorted" => Some("sort"),
                        "reversed" => Some("rev"),
                        "ord" => Some("code"),
                        "chr" => Some("char"),
                        "unique" => Some("unique"),
                        _ => None,
                    };
                    // min(xs, key=f) and max(xs, key=f): the first item with the smallest or largest key.
                    if (name == "min" || name == "max") && !self.user_fns.contains(&name) && args.len() == 2 && args[0].0.is_none() && args[1].0.as_deref() == Some("key") {
                        let f = args.remove(1).1;
                        let xs = args.remove(0).1;
                        let mut a = vec![f];
                        if name == "max" {
                            a.push(Expr::Bool(true));
                        }
                        return Ok(Expr::Index(Box::new(Expr::Method(Box::new(xs), "sort_by".into(), a, line)), Box::new(Expr::Int(0))));
                    }
                    // sorted(xs, key=f, reverse=r)
                    if name == "sorted" && !self.user_fns.contains(&name) && args.first().is_some_and(|a| a.0.is_none()) {
                        let recv = args[0].1.clone();
                        if let Some(s) = sort_kw(recv, &args[1..], line) {
                            return Ok(s);
                        }
                    }
                    // range(n), range(a, b) and range(a, b, step) are a..b.
                    if name == "range" && (1..=3).contains(&args.len()) && args.iter().all(|a| a.0.is_none()) && !self.user_fns.contains(&name) {
                        let mut a: Vec<Expr> = args.into_iter().map(|a| a.1).collect();
                        let (lo, hi) = if a.len() == 1 { (Expr::Int(0), a.remove(0)) } else { (a.remove(0), a.remove(0)) };
                        let r = Expr::Range(Box::new(lo), Box::new(hi));
                        return Ok(if a.is_empty() { r } else { Expr::Method(Box::new(r), "step".into(), a, line) });
                    }
                    // any(c for x in xs) and all(...) test each item.
                    if (name == "any" || name == "all") && args.len() == 1 && !self.user_fns.contains(&name) {
                        let arg = args.remove(0).1;
                        return Ok(match arg {
                            Expr::Method(base, mm, mut la, _) if mm == "map" && la.len() == 1 => Expr::Method(base, name, vec![la.remove(0)], line),
                            other => Expr::Method(Box::new(other), name, vec![Expr::Lambda(vec!["b_".into()], Box::new(Expr::Ident("b_".into())))], line),
                        });
                    }
                    let star = args.iter().any(|a| a.0.as_deref() == Some("__star"));
                    if !self.user_fns.contains(&name) && (star || args.iter().all(|a| a.0.is_none())) {
                        if let Some(d) = python_call(&name, &mut args, line) {
                            return Ok(d);
                        }
                    }
                    match m {
                        Some(m) if args.len() == 1 && args[0].0.is_none() && !self.user_fns.contains(&name) => Ok(Expr::Method(Box::new(args.remove(0).1), m.into(), vec![], line)),
                        // A program's own function named like a built-in (`fn args(...)`) is its own.
                        _ if self.user_fns.contains(&name) && SHADOWABLE.contains(&name.as_str()) => Ok(Expr::Call(format!("{name}_"), args, line)),
                        _ => Ok(Expr::Call(name, args, line)),
                    }
                } else if matches!(name.as_str(), "int" | "float" | "str" | "len") && !self.is_sym("=") {
                    // `map(int, xs)`, `key=len`: a built-in conversion used as a function value.
                    let v = Expr::Ident("v_".into());
                    Ok(Expr::Lambda(vec!["v_".into()], Box::new(Expr::Method(Box::new(v), name, vec![], line))))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Tok::Sym("(") => {
                let mut items = Vec::new();
                while !self.is_sym(")") {
                    items.push(self.expr()?);
                    // `(x * 2 for x in xs)`: a generator is the list it makes.
                    if items.len() == 1 && self.eat_kw("for") {
                        let c = self.comprehension(items.pop().unwrap())?;
                        self.expect_sym(")")?;
                        return Ok(c);
                    }
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
                if items.len() == 1 {
                    Ok(items.pop().unwrap())
                } else {
                    Ok(Expr::Tuple(items))
                }
            }
            Tok::Sym("[") => {
                let mut items = Vec::new();
                while !self.is_sym("]") {
                    let flat = self.eat_sym("*");
                    let e = self.expr()?;
                    // `[f(x) for x in xs if c]`
                    if self.eat_kw("for") {
                        let c = self.flat_comprehension(e, flat)?;
                        self.expect_sym("]")?;
                        return Ok(c);
                    }
                    if flat {
                        return Err(self.err("expected for after *x").fix_hint("join lists with +: a + b"));
                    }
                    items.push(e);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                Ok(Expr::List(items))
            }
            Tok::Sym("{") => {
                // In expression position braces are always a map literal.
                let mut pairs = Vec::new();
                self.skip_seps();
                while !self.is_sym("}") {
                    let k = self.expr()?;
                    // Python's sets: `{a, b}` and `{w for w in ws}` are lists without repeats.
                    if pairs.is_empty() && !self.is_sym(":") {
                        let line = self.line();
                        let list = if self.eat_kw("for") {
                            self.comprehension(k)?
                        } else {
                            let mut xs = vec![k];
                            while self.eat_sym(",") {
                                self.skip_newlines();
                                if self.is_sym("}") {
                                    break;
                                }
                                xs.push(self.expr()?);
                            }
                            Expr::List(xs)
                        };
                        self.skip_newlines();
                        self.expect_sym("}")?;
                        return Ok(Expr::Method(Box::new(list), "unique".into(), vec![], line));
                    }
                    self.expect_sym(":")?;
                    let v = self.expr()?;
                    // Python's `{k: v for x in xs}`: a map filled by a loop over the pairs.
                    if pairs.is_empty() && self.is_kw("for") {
                        let line = self.line();
                        self.next();
                        let list = self.comprehension(Expr::Tuple(vec![k, v]))?;
                        self.skip_newlines();
                        self.expect_sym("}")?;
                        let d = format!("ax_d{}", self.pos);
                        let fill = Stmt::Assign {
                            target: Expr::Index(Box::new(Expr::Ident(d.clone())), Box::new(Expr::Ident("ax_k".into()))),
                            op: "=",
                            value: Expr::Ident("ax_v".into()),
                            line,
                        };
                        let pat = Pat::Tuple(vec![Pat::Bind("ax_k".into()), Pat::Bind("ax_v".into())]);
                        return Ok(Expr::Block(
                            vec![
                                Stmt::Assign { target: Expr::Ident(d.clone()), op: "=", value: Expr::Map(vec![]), line },
                                Stmt::Expr { expr: Expr::For(pat, Box::new(list), Box::new(Expr::Block(vec![fill], None))), line },
                            ],
                            Some(Box::new(Expr::Ident(d))),
                        ));
                    }
                    pairs.push((k, v));
                    self.skip_seps();
                }
                self.expect_sym("}")?;
                Ok(Expr::Map(pairs))
            }
            Tok::Kw("if") => {
                let cond = self.expr()?;
                let then = self.block()?;
                let save = self.pos;
                self.skip_newlines();
                if self.eat_kw("else") {
                    let other = if self.is_kw("if") { self.primary()? } else { self.block()? };
                    Ok(Expr::If(Box::new(cond), Box::new(then), Some(Box::new(other))))
                } else {
                    self.pos = save;
                    Ok(Expr::If(Box::new(cond), Box::new(then), None))
                }
            }
            Tok::Kw("match") => {
                let subject = self.expr()?;
                // Inside brackets the arms are not indented into a block: `(match v: A => 1  _ => 0)`.
                if self.is_sym(":") {
                    self.pos += 1;
                    let mut arms = Vec::new();
                    self.skip_seps();
                    while !self.is_sym(")") && !self.is_sym("]") && self.pos < self.toks.len() {
                        let p = self.pat()?;
                        self.expect_sym("=>")?;
                        let body = self.expr()?;
                        arms.push((p, body));
                        if self.is_sym(",") {
                            self.pos += 1;
                        }
                        self.skip_seps();
                    }
                    return Ok(Expr::Match(Box::new(subject), arms));
                }
                self.expect_sym("{")?;
                let mut arms = Vec::new();
                self.skip_seps();
                while !self.is_sym("}") {
                    let p = self.pat()?;
                    // `Arr(xs):` as in Python's `case`, for `Arr(xs) =>`.
                    if !self.eat_sym(":") && !self.is_sym("{") {
                        self.expect_sym("=>")?;
                    }
                    self.skip_newlines();
                    let next = self.toks.get(self.pos + 1).map(|t| t.tok.clone());
                    let after = self.toks.get(self.pos + 2).map(|t| t.tok.clone());
                    let map = self.is_sym("{")
                        && match (&next, &after) {
                            (Some(Tok::Sym("}")), _) => self.ret_map,
                            (Some(Tok::Str(_) | Tok::Int(_) | Tok::Ident(_)), Some(Tok::Sym(":"))) => true,
                            _ => false,
                        };
                    let body = if map {
                        self.expr()?
                    } else if self.is_sym("{") {
                        self.block()?
                    } else {
                        // An arm can also be one statement, such as `Num(n) => total += n`.
                        match self.stmt()? {
                            Stmt::Expr { expr, .. } => expr,
                            st => Expr::Block(vec![st], None),
                        }
                    };
                    arms.push((p, body));
                    self.skip_seps();
                }
                self.expect_sym("}")?;
                // `match run(x):` with only an `Err(m)` arm: success needs nothing done.
                let has = |n: &str| arms.iter().any(|(p, _)| matches!(p, Pat::Variant(v, _) if v == n));
                if has("Err") && !has("Ok") && !arms.iter().any(|(p, _)| matches!(p, Pat::Wild | Pat::Bind(_))) {
                    arms.push((Pat::Variant("Ok".into(), vec![Pat::Wild]), Expr::Block(vec![], None)));
                }
                Ok(Expr::Match(Box::new(subject), arms))
            }
            Tok::Kw("for") => {
                let mut p = bind_upper(self.pat()?);
                // `for k, v in ...` unpacks like `for (k, v) in ...`.
                if self.is_sym(",") {
                    let mut ps = vec![p];
                    while self.eat_sym(",") {
                        ps.push(bind_upper(self.pat()?));
                    }
                    p = Pat::Tuple(ps);
                }
                if !self.eat_kw("in") {
                    return Err(self.err("expected 'in' after the loop variable"));
                }
                let iter = self.expr()?;
                let body = self.block()?;
                Ok(Expr::For(p, Box::new(iter), Box::new(body)))
            }
            Tok::Kw("while") => {
                let cond = self.expr()?;
                let body = self.block()?;
                Ok(Expr::While(Box::new(cond), Box::new(body)))
            }
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected an expression, found {}", tok_text(&t))))
            }
        }
    }

    // Splits "a {x} b" into literal and interpolated parts. "{{" is a literal brace.
    fn interp(&self, s: &str, line: usize) -> R<Vec<StrPart>> {
        let mut parts = Vec::new();
        let mut lit = String::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '{' if chars.get(i + 1) == Some(&'{') => {
                    lit.push('{');
                    i += 2;
                }
                '}' if chars.get(i + 1) == Some(&'}') => {
                    lit.push('}');
                    i += 2;
                }
                '{' if !chars.get(i + 1).map_or(false, |c| c.is_alphabetic() || matches!(c, '_' | '(' | '[' | '-' | '!' | '"' | '\'')) => {
                    lit.push('{');
                    i += 1;
                }
                '{' => {
                    let mut depth = 1;
                    let start = i + 1;
                    i += 1;
                    while i < chars.len() && depth > 0 {
                        match chars[i] {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        i += 1;
                    }
                    if depth > 0 {
                        return Err(Diag::new("parse", line, 1, "unclosed '{' in string").fix_hint("use {{ for a literal brace"));
                    }
                    let inner: String = chars[start..i - 1].iter().collect();
                    // Python's format spec: `{x:.2f}`, `{n:03d}`, `{s:<10}`, `{v:>8}`.
                    let (inner, spec) = split_spec(&inner);
                    let toks = lex(&inner).map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    let mut sub = Parser { toks, pos: 0, ret_map: false, user_fns: self.user_fns.clone(), children: vec![], hoisted: vec![] };
                    let e = sub.expr().map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    if !lit.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    let e = match spec {
                        Some(sp) => Expr::Method(Box::new(e), "spec".into(), vec![Expr::Str(vec![StrPart::Lit(sp)])], line),
                        None => e,
                    };
                    parts.push(StrPart::Interp(e));
                }
                c => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        if !lit.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(lit));
        }
        Ok(parts)
    }
}

// A declared name like `MAX = 10` is a variable even though it is capitalised.
// Python method names models write by habit, as the Brevi method they mean.
// Built-in functions a program may define for itself; its own version gets a suffix in the Rust.
const SHADOWABLE: &[&str] = &["args", "input", "stdin", "lines", "read", "write", "json", "csv", "date", "heap", "abs", "min", "max"];

// `x:.2f` -> ("x", Some(".2f")): a colon outside brackets and quotes followed by a Python format spec.
fn split_spec(inner: &str) -> (String, Option<String>) {
    let (mut depth, mut quote) = (0i32, None::<char>);
    for (i, c) in inner.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ':') if depth == 0 => {
                let spec = &inner[i + 1..];
                let ok = !spec.is_empty() && {
                    let b = spec.trim_start_matches(['<', '>', '^']).trim_start_matches('+');
                    let b = b.trim_start_matches(|c: char| c.is_ascii_digit() || c == ',');
                    let b = b.strip_prefix('.').map_or(b, |r| r.trim_start_matches(|c: char| c.is_ascii_digit()));
                    b.is_empty() || matches!(b, "d" | "f" | "s" | "x" | "%" | "e")
                };
                return if ok { (inner[..i].to_string(), Some(spec.to_string())) } else { (inner.to_string(), None) };
            }
            _ => {}
        }
    }
    (inner.to_string(), None)
}

// Python's sort keywords: `key=f` and `reverse=r` (also `sort_by(f, reverse=r)`) become `sort_by(f, r)`.
fn sort_kw(recv: Expr, args: &[(Option<String>, Expr)], line: usize) -> Option<Expr> {
    let get = |k: &str| args.iter().find(|a| a.0.as_deref() == Some(k)).map(|a| a.1.clone());
    let (key, rev) = (get("key"), get("reverse"));
    if key.is_none() && rev.is_none() {
        return None;
    }
    let f = key
        .or_else(|| args.iter().find(|a| a.0.is_none()).map(|a| a.1.clone()))
        .unwrap_or_else(|| Expr::Lambda(vec!["x_".into()], Box::new(Expr::Ident("x_".into()))));
    let mut a = vec![f];
    a.extend(rev);
    Some(Expr::Method(Box::new(recv), "sort_by".into(), a, line))
}

// Renames a top-level `main()` call (plain, or in an if's blocks) to `main_()`; true when there was one.
fn rename_main(e: &mut Expr) -> bool {
    match e {
        Expr::Call(f, a, _) if f == "main" && a.is_empty() => {
            *f = "main_".into();
            true
        }
        Expr::If(_, a, b) => {
            let x = rename_main(a);
            b.as_mut().map_or(false, |b| rename_main(b)) || x
        }
        Expr::Block(stmts, tail) => {
            let mut hit = false;
            for st in stmts.iter_mut() {
                if let Stmt::Expr { expr, .. } = st {
                    hit |= rename_main(expr);
                }
            }
            tail.as_mut().map_or(false, |t| rename_main(t)) || hit
        }
        _ => false,
    }
}

// Python's heapq functions (`from heapq import heappush` or `heapq.heappush`) as list methods.
fn heapq_method(name: &str, nargs: usize) -> Option<&'static str> {
    Some(match (name, nargs) {
        ("heappush", 2) => "heap_push",
        ("heappop", 1) => "heap_pop",
        ("heapify", 1) => "heapify",
        ("heappushpop", 2) => "heap_push_pop",
        ("heapreplace", 2) => "heap_replace",
        _ => return None,
    })
}

fn python_method(name: &str, nargs: usize) -> Option<String> {
    Some(match (name, nargs) {
        ("append", 1) => "push",
        ("sorted", 0) => "sort",
        ("reversed", 0) => "rev",
        ("strip", 0) => "trim",
        ("strip", 1) => "trim_of",
        ("rstrip", 0) => "trim_end",
        ("rstrip", 1) => "trim_end_of",
        ("lstrip", 0) => "trim_start",
        ("lstrip", 1) => "trim_start_of",
        ("splitlines", 0) => "lines",
        ("isupper", 0) => "is_upper",
        ("islower", 0) => "is_lower",
        ("isalnum", 0) => "is_alnum",
        ("split", 0) => "words",
        ("startswith", 1) => "starts_with",
        ("endswith", 1) => "ends_with",
        ("isdigit", 0) => "is_digit",
        ("isalpha", 0) => "is_alpha",
        ("isspace", 0) => "is_space",
        ("items", 0) => "pairs",
        ("index", 1) => "index_of",
        ("get", 2) => "get_or",
        ("char_at" | "to_char" | "chr", 0) => "char",
        ("find", 2) => "find_from",
        _ => return None,
    }
    .into())
}

// Python built-ins spelled as Brevi: set(xs), divmod(a, b), round(x, n), enumerate(xs, 1), defaultdict(int).
fn python_call(name: &str, args: &mut Vec<(Option<String>, Expr)>, line: usize) -> Option<Expr> {
    let m = |e: Expr, n: &str, a: Vec<Expr>| Expr::Method(Box::new(e), n.into(), a, line);
    if args.iter().any(|(n, _)| n.as_deref() == Some("__star")) {
        if name != "print" {
            return None;
        }
        // `print(*xs, 1)`: the items written with the separator between them.
        let mut items: Option<Expr> = None;
        let mut rest = Vec::new();
        for (n, e) in args.drain(..) {
            let part = match n.as_deref() {
                Some("__star") => m(e, "map", vec![Expr::Lambda(vec!["ax_x".into()], Box::new(Expr::Call("str".into(), vec![(None, Expr::Ident("ax_x".into()))], line)))]),
                None => Expr::List(vec![Expr::Call("str".into(), vec![(None, e)], line)]),
                Some(_) => {
                    rest.push((n, e));
                    continue;
                }
            };
            items = Some(match items {
                Some(a) => Expr::Binary("+", Box::new(a), Box::new(part)),
                None => part,
            });
        }
        let sep = rest.iter().position(|(n, _)| n.as_deref() == Some("sep")).map(|i| rest.remove(i).1).unwrap_or(Expr::Str(vec![StrPart::Lit(" ".into())]));
        let joined = m(items.unwrap_or(Expr::List(vec![])), "join", vec![sep]);
        let mut a = vec![(None, joined)];
        a.extend(rest);
        return Some(Expr::Call("print".into(), a, line));
    }
    if let Some(hm) = heapq_method(name, args.len()) {
        let h = args.remove(0).1;
        return Some(m(h, hm, args.drain(..).map(|a| a.1).collect()));
    }
    Some(match (name, args.len()) {
        ("set" | "list" | "deque", 0) => Expr::List(vec![]),
        ("Decimal", 1) => m(args.remove(0).1, "dec", vec![]),
        ("deque" | "list", 1) => m(args.remove(0).1, "skip", vec![Expr::Int(0)]),
        ("map" | "filter", 2) => {
            let f = args.remove(0).1;
            m(args.remove(0).1, name, vec![f])
        }
        ("dict" | "Counter", 0) | ("defaultdict", 1) => Expr::Map(vec![]),
        ("set", 1) => m(args.remove(0).1, "unique", vec![]),
        ("divmod", 2) => {
            let (a, b) = (args[0].1.clone(), args[1].1.clone());
            Expr::Tuple(vec![Expr::Binary("//", Box::new(a.clone()), Box::new(b.clone())), Expr::Binary("%", Box::new(a), Box::new(b))])
        }
        ("round", 1) => m(m(args.remove(0).1, "round", vec![]), "int", vec![]),
        ("round", 2) => {
            let n = args.remove(1).1;
            m(args.remove(0).1, "round", vec![n])
        }
        ("enumerate", 2) => {
            let n = args.remove(1).1;
            m(args.remove(0).1, "enumerate_from", vec![n])
        }
        _ => return None,
    })
}

// Fills `{}`/`{:spec}`/`{0}` (format) or `%s`/`%5.2f`/`%-8s` (percent) slots in a string's text
// with the arguments; None when the slots and arguments do not match up.
fn fill_format(parts: &[StrPart], args: Vec<Expr>, percent: bool, line: usize) -> Option<Vec<StrPart>> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut next = 0;
    let mut used = vec![false; args.len()];
    for part in parts {
        let text = match part {
            StrPart::Lit(t) => t,
            other => {
                if !lit.is_empty() {
                    out.push(StrPart::Lit(std::mem::take(&mut lit)));
                }
                out.push(other.clone());
                continue;
            }
        };
        let cs: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            let slot = if percent && cs[i] == '%' {
                if cs.get(i + 1) == Some(&'%') {
                    lit.push('%');
                    i += 2;
                    continue;
                }
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_ascii_digit() || matches!(cs[j], '-' | '.' | '+')) {
                    j += 1;
                }
                let ty = *cs.get(j)?;
                if !matches!(ty, 'd' | 'i' | 's' | 'f' | 'r' | 'x' | 'e') {
                    return None;
                }
                let flags: String = cs[i + 1..j].iter().collect();
                let spec = match (ty, flags.strip_prefix('-')) {
                    (_, Some(w)) => format!("<{w}"),
                    ('s' | 'r', None) if !flags.is_empty() => format!(">{flags}"),
                    ('f' | 'x' | 'e', None) => format!("{flags}{ty}"),
                    (_, None) => flags,
                };
                i = j + 1;
                let k = next;
                next += 1;
                Some((k, spec))
            } else if !percent && cs[i] == '{' {
                let j = i + cs[i..].iter().position(|&c| c == '}')?;
                let inner: String = cs[i + 1..j].iter().collect();
                let (idx, spec) = inner.split_once(':').unwrap_or((&inner, ""));
                if !idx.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let k = if idx.is_empty() {
                    next += 1;
                    next - 1
                } else {
                    idx.parse().ok()?
                };
                i = j + 1;
                Some((k, spec.to_string()))
            } else {
                None
            };
            match slot {
                Some((k, spec)) => {
                    let a = args.get(k)?.clone();
                    *used.get_mut(k)? = true;
                    if !lit.is_empty() {
                        out.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    out.push(StrPart::Interp(if spec.is_empty() {
                        a
                    } else {
                        Expr::Method(Box::new(a), "spec".into(), vec![Expr::Str(vec![StrPart::Lit(spec)])], line)
                    }));
                }
                None => {
                    lit.push(cs[i]);
                    i += 1;
                }
            }
        }
    }
    if !lit.is_empty() {
        out.push(StrPart::Lit(lit));
    }
    used.iter().all(|u| *u).then_some(out)
}

// `[]`, `{}`, `0`, `0.0`, `""` and `set()`/`list()`/`dict()`: the value a missing map key reads as.
fn empty_literal(e: &Expr) -> bool {
    match e {
        Expr::List(v) => v.is_empty(),
        Expr::Map(v) => v.is_empty(),
        Expr::Int(0) => true,
        Expr::Float(f) => *f == 0.0,
        Expr::Str(p) => p.is_empty(),
        _ => false,
    }
}

fn bind_upper(p: Pat) -> Pat {
    match p {
        Pat::Variant(n, args) if args.is_empty() => Pat::Bind(n),
        Pat::Tuple(ps) => Pat::Tuple(ps.into_iter().map(bind_upper).collect()),
        p => p,
    }
}

// Python's nested `def inner(a):` inside `def outer(...)` becomes the top-level `outer_inner(a, c1, c2)`,
// where c1, c2 are the outer function's locals it reads; calls inside outer pass them along.
fn lift_nested(outer: &str, outer_params: &[Param], body: &mut Expr, kids: Vec<Item>) -> Vec<Item> {
    let mut outer_locals: Vec<String> = outer_params.iter().map(|p| p.name.clone()).collect();
    walk_mut(body, &mut |e| {
        if let Expr::Block(stmts, _) = e {
            for st in stmts.iter() {
                match st {
                    Stmt::Assign { target, .. } => pat_names_expr(target, &mut outer_locals),
                    Stmt::Let { name, .. } => pat_names(name, &mut outer_locals),
                    _ => {}
                }
            }
        }
        if let Expr::For(p, _, _) = e {
            pat_names(p, &mut outer_locals);
        }
    });
    let names: Vec<String> = kids.iter().filter_map(|k| match k { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
    // What each child reads from the outer function.
    let mut caps: Vec<Vec<String>> = Vec::new();
    for k in &kids {
        let Item::Fn { params, body: kb, .. } = k else {
            caps.push(vec![]);
            continue;
        };
        let mut own: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut kb2 = kb.clone();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts.iter() {
                    if let Stmt::Assign { target, .. } = st {
                        pat_names_expr(target, &mut own);
                    }
                }
            }
            if let Expr::For(p, _, _) = e {
                pat_names(p, &mut own);
            }
            if let Expr::Lambda(ps, _) = e {
                own.extend(ps.iter().cloned());
            }
        });
        let mut used = Vec::new();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Ident(n) | Expr::Move(n) = e {
                if outer_locals.contains(n) && !own.contains(n) && !used.contains(n) {
                    used.push(n.clone());
                }
            }
        });
        caps.push(used);
    }
    let mangled = |n: &str| format!("{outer}_{n}");
    let rename = |e: &mut Expr| {
        walk_mut(e, &mut |x| {
            match x {
                Expr::Call(f, args, line) => {
                    if let Some(i) = names.iter().position(|n| n == f) {
                        *f = mangled(&names[i]);
                        for c in &caps[i] {
                            args.push((None, Expr::Ident(c.clone())));
                        }
                        let _ = line;
                    }
                }
                Expr::Ident(n) => {
                    if let Some(i) = names.iter().position(|m| m == n) {
                        if caps[i].is_empty() {
                            *n = mangled(&names[i]);
                        }
                    }
                }
                _ => {}
            }
        });
    };
    rename(body);
    let mut out = Vec::new();
    for (k, cap) in kids.into_iter().zip(caps.iter()) {
        if let Item::Fn { name, mut params, ret, fallible, mut body, line, script } = k {
            for c in cap {
                params.push(Param { name: c.clone(), ty: Ty::Named("?".into()), is_mut: false });
            }
            rename(&mut body);
            out.push(Item::Fn { name: mangled(&name), params, ret, fallible, body, line, script });
        } else {
            out.push(k);
        }
    }
    out
}

fn pat_names(p: &Pat, out: &mut Vec<String>) {
    match p {
        Pat::Bind(n) => {
            if !out.contains(n) {
                out.push(n.clone())
            }
        }
        Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| pat_names(p, out)),
        _ => {}
    }
}

fn pat_names_expr(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident(n) => {
            if !out.contains(n) {
                out.push(n.clone())
            }
        }
        Expr::Tuple(xs) => xs.iter().for_each(|x| pat_names_expr(x, out)),
        _ => {}
    }
}

// Calls f on every expression inside e (children first is not needed; f sees each node once).
fn walk_mut(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
    f(e);
    match e {
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| walk_mut(x, f)),
        Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
            walk_mut(k, f);
            walk_mut(v, f);
        }),
        Expr::Str(parts) => {
            for p in parts.iter_mut() {
                if let StrPart::Interp(x) = p {
                    walk_mut(x, f);
                }
            }
        }
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) | Expr::Try(a) => walk_mut(a, f),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) | Expr::For(_, a, b) => {
            walk_mut(a, f);
            walk_mut(b, f);
        }
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| walk_mut(a, f)),
        Expr::Method(r, _, args, _) => {
            walk_mut(r, f);
            args.iter_mut().for_each(|a| walk_mut(a, f));
        }
        Expr::If(c, a, b) => {
            walk_mut(c, f);
            walk_mut(a, f);
            if let Some(b) = b {
                walk_mut(b, f);
            }
        }
        Expr::Match(sub, arms) => {
            walk_mut(sub, f);
            arms.iter_mut().for_each(|(_, a)| walk_mut(a, f));
        }
        Expr::Block(stmts, tail) => {
            for st in stmts.iter_mut() {
                match st {
                    Stmt::Let { value, .. } => walk_mut(value, f),
                    Stmt::Assign { target, value, .. } => {
                        walk_mut(target, f);
                        walk_mut(value, f);
                    }
                    Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } | Stmt::Assert { cond: expr, .. } => walk_mut(expr, f),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                walk_mut(t, f);
            }
        }
        _ => {}
    }
}
