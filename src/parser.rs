use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{lex, Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

type R<T> = Result<T, Diag>;

pub fn parse_program(src: &str) -> R<Vec<Item>> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
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
        if items.iter().any(|i| matches!(i, Item::Fn { name, .. } if name == "main")) {
            return Err(Diag::new("parse", script_line, 1, "top-level statements and fn main cannot both be used")
                .fix_hint("move these statements into main, or delete fn main"));
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
            self.expect_sym("(")?;
            let mut params = Vec::new();
            while !self.is_sym(")") {
                let is_mut = self.eat_kw("mut");
                let pname = self.ident()?;
                self.expect_sym(":")?;
                let ty = self.ty()?;
                params.push(Param { name: pname, ty, is_mut });
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            let ret = if self.eat_sym("->") { self.ty()? } else { Ty::Unit };
            let fallible = self.eat_sym("!");
            let body = if self.eat_sym("=") {
                self.skip_newlines();
                self.expr()?
            } else if self.is_sym("{") {
                self.block()?
            } else {
                return Err(self.err("expected '=' or '{' to start the function body"));
            };
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
                Tok::Str(s) => s,
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
            let name = self.pat()?;
            let ty = if self.eat_sym(":") { Some(self.ty()?) } else { None };
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Let { name, ty, value, mutable, line });
        }
        // `x: T = v` declares a typed variable, like `var x: T = v`.
        if matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym(":")) {
            let name = self.pat()?;
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
        let e = self.expr()?;
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
                let value = self.expr()?;
                return Ok(Stmt::Assign { target: e, op, value, line });
            }
        }
        Ok(Stmt::Expr { expr: e, line })
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
            Tok::Str(s) => Ok(Pat::Str(s)),
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
        self.binary(0)
    }

    fn binary(&mut self, level: usize) -> R<Expr> {
        const LEVELS: &[&[&str]] = &[
            &["or"],
            &["and"],
            &["==", "!=", "<", ">", "<=", ">=", "~="],
            &[".."],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
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
                            let args = self.args()?.into_iter().map(|(_, a)| a).collect();
                            e = Expr::Method(Box::new(e), name, args, line);
                        } else {
                            e = Expr::Field(Box::new(e), name);
                        }
                    }
                    _ => return Err(self.err("expected a field or method name after '.'")),
                }
            } else if self.eat_sym("[") {
                let idx = self.expr()?;
                self.expect_sym("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
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
                (Tok::Ident(n), Tok::Sym(":")) => {
                    self.next();
                    self.next();
                    Some(n)
                }
                _ => None,
            };
            args.push((name, self.expr()?));
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(args)
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
            Tok::Ident(name) => {
                if self.eat_sym("(") {
                    Ok(Expr::Call(name, self.args()?, line))
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
                    items.push(self.expr()?);
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
                self.expect_sym("{")?;
                let mut arms = Vec::new();
                self.skip_seps();
                while !self.is_sym("}") {
                    let p = self.pat()?;
                    self.expect_sym("=>")?;
                    self.skip_newlines();
                    let body = if self.is_sym("{") {
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
                let p = self.pat()?;
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
                '{' if !chars.get(i + 1).map_or(false, |c| c.is_alphabetic() || matches!(c, '_' | '(' | '-' | '!')) => {
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
                    let toks = lex(&inner).map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    let mut sub = Parser { toks, pos: 0 };
                    let e = sub.expr().map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    if !lit.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
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
