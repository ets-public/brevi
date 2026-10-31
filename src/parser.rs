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
}

type R<T> = Result<T, Diag>;

pub fn parse_program(src: &str) -> R<Vec<Item>> {
    let toks = crate::lexer::indent_blocks(lex(src)?);
    let user_fns = toks.windows(2).filter_map(|w| match (&w[0].tok, &w[1].tok) {
        (Tok::Kw("fn"), Tok::Ident(n)) => Some(n.clone()),
        _ => None,
    }).collect();
    let mut p = Parser { toks, pos: 0, ret_map: false, user_fns };
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
    if !script.is_empty() {
        // Top-level statements beside fn main run first, then main, as in a Python file that ends with main().
        let mut script = script;
        if let Some(Item::Fn { name, params, .. }) = items.iter_mut().find(|i| matches!(i, Item::Fn { name, .. } if name == "main")) {
            if !params.is_empty() {
                return Err(Diag::new("parse", script_line, 1, "main takes no parameters").fix_hint("use args() to read command line arguments"));
            }
            *name = "main_".into();
            let calls = |st: &Stmt| matches!(st, Stmt::Expr { expr: Expr::Call(f, a, _), .. } if f == "main" && a.is_empty());
            if script.iter().any(|st| calls(st)) {
                for st in script.iter_mut() {
                    if let Stmt::Expr { expr: Expr::Call(f, a, _), .. } = st {
                        if f == "main" && a.is_empty() {
                            *f = "main_".into();
                        }
                    }
                }
            } else {
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
            let body = if self.eat_sym("=") {
                self.skip_newlines();
                self.expr()?
            } else if self.is_sym("{") || self.is_sym(":") {
                self.block()?
            } else {
                return Err(self.err("expected '=', ':' or '{' to start the function body"));
            };
            self.ret_map = false;
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

    fn stmt(&mut self) -> R<Stmt> {
        let line = self.line();
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
            let value = if self.peek() == &Tok::Newline || self.is_sym("}") { None } else { Some(self.expr()?) };
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
        Ok(Stmt::Expr { expr: e, line })
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
            lhs = if op == ".." {
                Expr::Range(Box::new(lhs), Box::new(rhs))
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
                    t @ (Tok::Ident(_) | Tok::Kw("or")) => {
                        let name = match t {
                            Tok::Ident(n) => n,
                            _ => "or".to_string(),
                        };
                        if self.eat_sym("(") {
                            let raw = self.args()?;
                            if matches!(name.as_str(), "sort" | "sort_by" | "sorted") {
                                if let Some(s) = sort_kw(e.clone(), &raw, line) {
                                    e = s;
                                    continue;
                                }
                            }
                            let mut args: Vec<Expr> = raw.into_iter().map(|(_, a)| a).collect();
                            let name = python_method(&name, args.len()).unwrap_or(name);
                            // `xs.to_list()` / `xs.copy()`: a list already is one, so this is a copy.
                            let name = if matches!(name.as_str(), "to_list" | "to_vec" | "copy" | "collect") && args.is_empty() {
                                args.push(Expr::Int(0));
                                "skip".to_string()
                            } else {
                                name
                            };
                            // Python's `", ".join(xs)` is `xs.join(", ")`.
                            e = if name == "join" && args.len() == 1 && matches!(e, Expr::Str(_)) && !matches!(args[0], Expr::Str(_)) {
                                Expr::Method(Box::new(args.remove(0)), name, vec![e], line)
                            } else {
                                Expr::Method(Box::new(e), name, args, line)
                            };
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
                    let mut args = vec![start.unwrap_or(Expr::Int(0))];
                    if !self.is_sym("]") {
                        args.push(self.expr()?);
                    }
                    self.expect_sym("]")?;
                    e = Expr::Method(Box::new(e), "slice".into(), args, line);
                } else {
                    self.expect_sym("]")?;
                    let Some(idx) = start else { return Err(self.err("expected an index")) };
                    e = Expr::Index(Box::new(e), Box::new(idx));
                }
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
            let e = self.expr()?;
            // `min(x for x in xs)` passes the list the loop builds.
            let e = if self.eat_kw("for") { self.comprehension(e)? } else { e };
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
                    match m {
                        Some(m) if args.len() == 1 && args[0].0.is_none() && !self.user_fns.contains(&name) => Ok(Expr::Method(Box::new(args.remove(0).1), m.into(), vec![], line)),
                        // A program's own function named like a built-in (`fn args(...)`) is its own.
                        _ if self.user_fns.contains(&name) && SHADOWABLE.contains(&name.as_str()) => Ok(Expr::Call(format!("{name}_"), args, line)),
                        _ => Ok(Expr::Call(name, args, line)),
                    }
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Tok::Sym("(") => {
                let mut items = Vec::new();
                while !self.is_sym(")") {
                    items.push(self.expr()?);
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
                    let e = self.expr()?;
                    // `[f(x) for x in xs if c]`
                    if self.eat_kw("for") {
                        let c = self.comprehension(e)?;
                        self.expect_sym("]")?;
                        return Ok(c);
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
                    self.expect_sym(":")?;
                    let v = self.expr()?;
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
                    let mut sub = Parser { toks, pos: 0, ret_map: false, user_fns: self.user_fns.clone() };
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

fn python_method(name: &str, nargs: usize) -> Option<String> {
    Some(match (name, nargs) {
        ("append", 1) => "push",
        ("sorted", 0) => "sort",
        ("reversed", 0) => "rev",
        ("strip", 0) => "trim",
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

fn bind_upper(p: Pat) -> Pat {
    match p {
        Pat::Variant(n, args) if args.is_empty() => Pat::Bind(n),
        Pat::Tuple(ps) => Pat::Tuple(ps.into_iter().map(bind_upper).collect()),
        p => p,
    }
}
