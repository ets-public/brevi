// A Python parser for dynamic builds: source text to a small syntax tree.
// Anything outside the supported subset is an error with a line number, so the build refuses
// the program loudly instead of guessing.

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Name(String),
    Int(String),
    Float(f64),
    Imag,
    Str(String, bool),
    FStr(String, bool),
    Bytes(String),
    Op(&'static str),
    Newline,
    Indent,
    Dedent,
    Eof,
}

pub struct Lexed {
    pub toks: Vec<(Tok, usize)>,
}

const OPS: [&str; 49] = [
    "**=", "//=", ">>=", "<<=", "...", "->", ":=", "**", "//", ">>", "<<", "<=", ">=", "==", "!=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", "+", "-", "*", "/", "%", "&", "|", "^", "~", "<", ">", "(", ")", "[", "]", "{", "}", ",", ":", ".", ";", "@", "=", "!", "\\",
];

pub fn err(line: usize, msg: &str) -> (usize, String) {
    (line, msg.to_string())
}

// A bytes literal's escapes: a str's, without \u, \U and \N (kept as written); every char is below 256.
fn bytes_unescape(body: &str, line: usize) -> Result<String, (usize, String)> {
    let cs: Vec<char> = body.chars().collect();
    let mut pre = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '\\' && i + 1 < cs.len() {
            if matches!(cs[i + 1], 'u' | 'U' | 'N') {
                pre.push_str("\\\\");
            } else {
                pre.push('\\');
            }
            pre.push(cs[i + 1]);
            i += 2;
            continue;
        }
        pre.push(cs[i]);
        i += 1;
    }
    let v = unescape(&pre, line)?;
    Ok(v.chars().map(|c| char::from_u32(c as u32 & 0xff).unwrap()).collect())
}

fn unescape(body: &str, line: usize) -> Result<String, (usize, String)> {
    let mut out = String::new();
    let cs: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c != '\\' || i + 1 >= cs.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let n = cs[i + 1];
        i += 2;
        match n {
            '\n' => {}
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'f' => out.push('\x0c'),
            'v' => out.push('\x0b'),
            'x' | 'u' | 'U' => {
                let k = match n {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let h: String = cs[i..(i + k).min(cs.len())].iter().collect();
                let v = u32::from_str_radix(&h, 16).map_err(|_| err(line, "bad escape in string"))?;
                out.push(char::from_u32(v).ok_or_else(|| err(line, "bad escape in string"))?);
                i += k;
            }
            '0'..='7' => {
                let mut v = n.to_digit(8).unwrap();
                let mut k = 0;
                while k < 2 && i < cs.len() && cs[i].is_digit(8) {
                    v = v * 8 + cs[i].to_digit(8).unwrap();
                    i += 1;
                    k += 1;
                }
                out.push(char::from_u32(v).unwrap_or('?'));
            }
            'N' => return Err(err(line, "\\N{...} escapes are not supported")),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    Ok(out)
}

pub fn lex(src: &str) -> Result<Lexed, (usize, String)> {
    let cs: Vec<char> = src.chars().collect();
    let mut toks: Vec<(Tok, usize)> = vec![];
    let mut i = 0;
    let mut line = 1;
    let mut indents = vec![0usize];
    let mut depth = 0i32;
    let mut at_line_start = true;
    while i < cs.len() {
        if at_line_start && depth == 0 {
            // measure indentation
            let mut col = 0;
            let mut j = i;
            while j < cs.len() && (cs[j] == ' ' || cs[j] == '\t' || cs[j] == '\x0c') {
                col += if cs[j] == '\t' { 8 - col % 8 } else { 1 };
                j += 1;
            }
            if j >= cs.len() {
                break;
            }
            if cs[j] == '\n' || cs[j] == '#' || (cs[j] == '\r') {
                // blank or comment line
                while j < cs.len() && cs[j] != '\n' {
                    j += 1;
                }
                i = j + 1;
                line += 1;
                continue;
            }
            if cs[j] == '\\' && j + 1 < cs.len() && cs[j + 1] == '\n' {
                i = j + 2;
                line += 1;
                continue;
            }
            let cur = *indents.last().unwrap();
            if col > cur {
                indents.push(col);
                toks.push((Tok::Indent, line));
            } else {
                while col < *indents.last().unwrap() {
                    indents.pop();
                    toks.push((Tok::Dedent, line));
                }
                if col != *indents.last().unwrap() {
                    return Err(err(line, "unindent does not match any outer indentation level"));
                }
            }
            i = j;
            at_line_start = false;
        }
        let c = cs[i];
        if c == '\n' {
            if depth == 0 {
                if !matches!(toks.last(), Some((Tok::Newline, _)) | None) {
                    toks.push((Tok::Newline, line));
                }
                at_line_start = true;
            }
            line += 1;
            i += 1;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' || c == '\x0c' {
            i += 1;
            continue;
        }
        if c == '#' {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\\' && i + 1 < cs.len() && (cs[i + 1] == '\n' || (cs[i + 1] == '\r' && i + 2 < cs.len() && cs[i + 2] == '\n')) {
            i += if cs[i + 1] == '\r' { 3 } else { 2 };
            line += 1;
            continue;
        }
        // strings (with prefixes)
        let mut j = i;
        while j < cs.len() && j - i < 2 && cs[j].is_ascii_alphabetic() && "rRbBfFuU".contains(cs[j]) {
            j += 1;
        }
        if j < cs.len() && (cs[j] == '\'' || cs[j] == '"') && (j == i || cs[i..j].iter().all(|c| "rRbBfFuU".contains(*c))) {
            let prefix: String = cs[i..j].iter().collect::<String>().to_lowercase();
            let q = cs[j];
            let triple = j + 2 < cs.len() && cs[j + 1] == q && cs[j + 2] == q;
            let start_line = line;
            let mut k = if triple { j + 3 } else { j + 1 };
            let body_start = k;
            let raw = prefix.contains('r');
            loop {
                if k >= cs.len() {
                    return Err(err(start_line, "unterminated string literal"));
                }
                if cs[k] == '\\' {
                    if k + 1 < cs.len() && cs[k + 1] == '\n' {
                        line += 1;
                    }
                    k += 2;
                    continue;
                }
                if cs[k] == '\n' {
                    if !triple {
                        return Err(err(start_line, "unterminated string literal"));
                    }
                    line += 1;
                }
                if cs[k] == q {
                    if !triple {
                        break;
                    }
                    if k + 2 < cs.len() && cs[k + 1] == q && cs[k + 2] == q {
                        break;
                    }
                }
                k += 1;
            }
            let body: String = cs[body_start..k].iter().collect();
            i = if triple { k + 3 } else { k + 1 };
            if prefix.contains('b') {
                if let Some(c) = body.chars().find(|c| !c.is_ascii()) {
                    let _ = c;
                    return Err(err(start_line, "bytes can only contain ASCII literal characters"));
                }
                let v = if raw { body } else { bytes_unescape(&body, start_line)? };
                toks.push((Tok::Bytes(v), start_line));
                continue;
            }
            if prefix.contains('f') {
                toks.push((Tok::FStr(body, raw), start_line));
            } else {
                let v = if raw { body } else { unescape(&body, start_line)? };
                toks.push((Tok::Str(v, raw), start_line));
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            toks.push((Tok::Name(cs[i..j].iter().collect()), line));
            i = j;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit()) {
            let mut j = i;
            let lower: String;
            if c == '0' && j + 1 < cs.len() && "xXoObB".contains(cs[j + 1]) {
                j += 2;
                while j < cs.len() && (cs[j].is_ascii_hexdigit() || cs[j] == '_') {
                    j += 1;
                }
                lower = cs[i..j].iter().collect::<String>().to_lowercase().replace('_', "");
                toks.push((Tok::Int(lower), line));
                i = j;
                continue;
            }
            let mut is_float = false;
            while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '_') {
                j += 1;
            }
            if j < cs.len() && cs[j] == '.' && !(j + 1 < cs.len() && cs[j + 1] == '.') {
                is_float = true;
                j += 1;
                while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '_') {
                    j += 1;
                }
            }
            if j < cs.len() && (cs[j] == 'e' || cs[j] == 'E') {
                let mut k = j + 1;
                if k < cs.len() && (cs[k] == '+' || cs[k] == '-') {
                    k += 1;
                }
                if k < cs.len() && cs[k].is_ascii_digit() {
                    is_float = true;
                    j = k;
                    while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '_') {
                        j += 1;
                    }
                }
            }
            if j < cs.len() && (cs[j] == 'j' || cs[j] == 'J') {
                toks.push((Tok::Imag, line));
                i = j + 1;
                continue;
            }
            let t: String = cs[i..j].iter().filter(|c| **c != '_').collect();
            if is_float {
                toks.push((Tok::Float(t.parse().map_err(|_| err(line, "bad number"))?), line));
            } else {
                toks.push((Tok::Int(t), line));
            }
            i = j;
            continue;
        }
        let mut matched = false;
        for op in OPS.iter() {
            let n = op.len();
            if i + n <= cs.len() && cs[i..i + n].iter().collect::<String>() == *op {
                match *op {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    _ => {}
                }
                toks.push((Tok::Op(op), line));
                i += n;
                matched = true;
                break;
            }
        }
        if !matched {
            return Err(err(line, &format!("unexpected character {c:?}")));
        }
    }
    if !matches!(toks.last(), Some((Tok::Newline, _)) | None) {
        toks.push((Tok::Newline, line));
    }
    while indents.len() > 1 {
        indents.pop();
        toks.push((Tok::Dedent, line));
    }
    toks.push((Tok::Eof, line));
    Ok(Lexed { toks })
}

// ---------- syntax tree ----------

#[derive(Clone, Debug)]
pub enum FPart {
    Lit(String),
    Expr(Box<Ex>, Option<char>, Vec<FPart>, String),
}

#[derive(Clone, Debug)]
pub enum Arg {
    Pos(Ex),
    Star(Ex),
    Kw(String, Ex),
    KwStar(Ex),
}

#[derive(Clone, Debug)]
pub struct CompFor {
    pub target: Ex,
    pub iter: Ex,
    pub ifs: Vec<Ex>,
}

#[derive(Clone, Debug)]
pub enum Ex {
    Name(String),
    Int(String),
    Float(f64),
    Str(String),
    FStr(Vec<FPart>),
    NoneV,
    True,
    False,
    Ellipsis,
    List(Vec<Ex>),
    Tuple(Vec<Ex>),
    Set(Vec<Ex>),
    Dict(Vec<(Option<Ex>, Ex)>),
    Star(Box<Ex>),
    Attr(Box<Ex>, String),
    Index(Box<Ex>, Box<Ex>),
    Slice(Option<Box<Ex>>, Option<Box<Ex>>, Option<Box<Ex>>),
    Call(Box<Ex>, Vec<Arg>),
    Bin(&'static str, Box<Ex>, Box<Ex>),
    Unary(&'static str, Box<Ex>),
    And(Box<Ex>, Box<Ex>),
    Or(Box<Ex>, Box<Ex>),
    Cmp(Box<Ex>, Vec<(&'static str, Ex)>),
    IfExp(Box<Ex>, Box<Ex>, Box<Ex>),
    Lambda(Box<Params>, Box<Ex>),
    // kind: 'l' list, 's' set, 'g' generator, 'd' dict (val is Some)
    Comp(char, Box<Ex>, Option<Box<Ex>>, Vec<CompFor>),
    Walrus(String, Box<Ex>),
    Yield(Option<Box<Ex>>),
    YieldFrom(Box<Ex>),
}

#[derive(Clone, Debug, Default)]
pub struct Params {
    pub pos: Vec<(String, Option<Ex>)>,
    pub star: Option<String>,
    pub kwonly: Vec<(String, Option<Ex>)>,
    pub kwstar: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Handler {
    pub types: Option<Ex>,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct FuncDef {
    pub name: String,
    pub params: Params,
    pub body: Vec<Stmt>,
    pub decorators: Vec<Ex>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ClassDef {
    pub name: String,
    pub bases: Vec<Ex>,
    pub body: Vec<Stmt>,
    pub decorators: Vec<Ex>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum St {
    Expr(Ex),
    Assign(Vec<Ex>, Ex),
    AugAssign(Ex, &'static str, Ex),
    AnnAssign(Ex, Option<Ex>),
    Pass,
    Break,
    Continue,
    Return(Option<Ex>),
    Raise(Option<Ex>, Option<Ex>),
    Global(Vec<String>),
    Nonlocal(Vec<String>),
    Del(Vec<Ex>),
    Assert(Ex, Option<Ex>),
    Import(Vec<(String, Option<String>)>),
    ImportFrom(String, Vec<(String, Option<String>)>),
    If(Vec<(Ex, Vec<Stmt>)>, Option<Vec<Stmt>>),
    While(Ex, Vec<Stmt>, Option<Vec<Stmt>>),
    For(Ex, Ex, Vec<Stmt>, Option<Vec<Stmt>>),
    Try(Vec<Stmt>, Vec<Handler>, Option<Vec<Stmt>>, Option<Vec<Stmt>>),
    With(Vec<(Ex, Option<Ex>)>, Vec<Stmt>),
    Def(Box<FuncDef>),
    Class(Box<ClassDef>),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub line: usize,
    pub st: St,
}

// ---------- parser ----------

pub struct P {
    toks: Vec<(Tok, usize)>,
    i: usize,
}
type PR<T> = Result<T, (usize, String)>;

impl P {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }
    fn line(&self) -> usize {
        self.toks[self.i].1
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.i].0.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == op)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Name(n) if n == k)
    }
    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.next();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect_op(&mut self, op: &str) -> PR<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(err(self.line(), &format!("expected '{op}', found {:?}", self.peek())))
        }
    }
    fn expect_kw(&mut self, k: &str) -> PR<()> {
        if self.eat_kw(k) {
            Ok(())
        } else {
            Err(err(self.line(), &format!("expected '{k}'")))
        }
    }
    fn name(&mut self) -> PR<String> {
        match self.next() {
            Tok::Name(n) if !is_keyword(&n) => Ok(n),
            t => Err(err(self.line(), &format!("expected a name, found {t:?}"))),
        }
    }

    pub fn file(&mut self) -> PR<Vec<Stmt>> {
        let mut out = vec![];
        while !matches!(self.peek(), Tok::Eof) {
            if matches!(self.peek(), Tok::Newline) {
                self.next();
                continue;
            }
            out.extend(self.stmt()?);
        }
        Ok(out)
    }
    fn block(&mut self) -> PR<Vec<Stmt>> {
        self.expect_op(":")?;
        if !matches!(self.peek(), Tok::Newline) {
            // one-line suite
            return self.simple_line();
        }
        self.next();
        if !matches!(self.peek(), Tok::Indent) {
            return Err(err(self.line(), "expected an indented block"));
        }
        self.next();
        let mut out = vec![];
        while !matches!(self.peek(), Tok::Dedent | Tok::Eof) {
            if matches!(self.peek(), Tok::Newline) {
                self.next();
                continue;
            }
            out.extend(self.stmt()?);
        }
        self.next();
        Ok(out)
    }
    fn stmt(&mut self) -> PR<Vec<Stmt>> {
        let line = self.line();
        let kw = match self.peek() {
            Tok::Name(n) => n.clone(),
            Tok::Op("@") => "@".into(),
            _ => String::new(),
        };
        let st = match kw.as_str() {
            "if" => {
                self.next();
                let mut arms = vec![];
                let c = self.expr()?;
                let b = self.block()?;
                arms.push((c, b));
                let mut els = None;
                loop {
                    if self.eat_kw("elif") {
                        let c = self.expr()?;
                        let b = self.block()?;
                        arms.push((c, b));
                    } else if self.eat_kw("else") {
                        els = Some(self.block()?);
                        break;
                    } else {
                        break;
                    }
                }
                St::If(arms, els)
            }
            "while" => {
                self.next();
                let c = self.expr()?;
                let b = self.block()?;
                let els = if self.eat_kw("else") { Some(self.block()?) } else { None };
                St::While(c, b, els)
            }
            "for" => {
                self.next();
                let t = self.target_list()?;
                self.expect_kw("in")?;
                let it = self.exprlist()?;
                let b = self.block()?;
                let els = if self.eat_kw("else") { Some(self.block()?) } else { None };
                St::For(t, it, b, els)
            }
            "try" => {
                self.next();
                let body = self.block()?;
                let mut hs = vec![];
                while self.is_kw("except") {
                    let hl = self.line();
                    self.next();
                    if self.eat_op("*") {
                        return Err(err(hl, "except* is not supported"));
                    }
                    let (types, name) = if self.is_op(":") {
                        (None, None)
                    } else {
                        let t = self.expr()?;
                        let n = if self.eat_kw("as") { Some(self.name()?) } else { None };
                        (Some(t), n)
                    };
                    let b = self.block()?;
                    hs.push(Handler { types, name, body: b, line: hl });
                }
                let els = if self.eat_kw("else") { Some(self.block()?) } else { None };
                let fin = if self.eat_kw("finally") { Some(self.block()?) } else { None };
                if hs.is_empty() && fin.is_none() {
                    return Err(err(line, "try needs except or finally"));
                }
                St::Try(body, hs, els, fin)
            }
            "with" => {
                self.next();
                let mut items = vec![];
                let paren = self.is_op("(") && self.with_paren();
                if paren {
                    self.next();
                }
                loop {
                    let e = self.expr()?;
                    let t = if self.eat_kw("as") { Some(self.target()?) } else { None };
                    items.push((e, t));
                    if !self.eat_op(",") {
                        break;
                    }
                    if paren && self.is_op(")") {
                        break;
                    }
                }
                if paren {
                    self.expect_op(")")?;
                }
                let b = self.block()?;
                St::With(items, b)
            }
            "def" => St::Def(Box::new(self.funcdef(vec![])?)),
            "class" => St::Class(Box::new(self.classdef(vec![])?)),
            "@" => {
                let mut decs = vec![];
                while self.eat_op("@") {
                    decs.push(self.expr()?);
                    if !matches!(self.next(), Tok::Newline) {
                        return Err(err(self.line(), "expected a newline after a decorator"));
                    }
                }
                if self.is_kw("def") {
                    St::Def(Box::new(self.funcdef(decs)?))
                } else if self.is_kw("class") {
                    St::Class(Box::new(self.classdef(decs)?))
                } else if self.is_kw("async") {
                    return Err(err(self.line(), "async functions are not supported"));
                } else {
                    return Err(err(self.line(), "expected def or class after a decorator"));
                }
            }
            "async" => return Err(err(line, "async is not supported")),
            "match" if self.is_match_stmt() => return Err(err(line, "match statements are not supported")),
            _ => return self.simple_line(),
        };
        Ok(vec![Stmt { line, st }])
    }
    fn with_paren(&self) -> bool {
        // `with (a as b, c as d):` versus `with (expr).x as y:` : look for 'as' at depth 1
        let mut d = 0;
        let mut j = self.i;
        while j < self.toks.len() {
            match &self.toks[j].0 {
                Tok::Op("(") | Tok::Op("[") | Tok::Op("{") => d += 1,
                Tok::Op(")") | Tok::Op("]") | Tok::Op("}") => {
                    d -= 1;
                    if d == 0 {
                        return matches!(self.toks.get(j + 1).map(|t| &t.0), Some(Tok::Op(":")));
                    }
                }
                Tok::Name(n) if n == "as" && d == 1 => return true,
                Tok::Newline => return false,
                _ => {}
            }
            j += 1;
        }
        false
    }
    fn is_match_stmt(&self) -> bool {
        // `match x:` at statement start (soft keyword)
        let mut j = self.i + 1;
        let mut d = 0;
        while j < self.toks.len() {
            match &self.toks[j].0 {
                Tok::Op("(") | Tok::Op("[") | Tok::Op("{") => d += 1,
                Tok::Op(")") | Tok::Op("]") | Tok::Op("}") => d -= 1,
                Tok::Op(":") if d == 0 => return matches!(self.toks.get(j + 1).map(|t| &t.0), Some(Tok::Newline)) && j > self.i + 1,
                Tok::Op("=") if d == 0 => return false,
                Tok::Newline => return false,
                _ => {}
            }
            j += 1;
        }
        false
    }
    fn simple_line(&mut self) -> PR<Vec<Stmt>> {
        let mut out = vec![];
        loop {
            let line = self.line();
            let st = self.small()?;
            out.push(Stmt { line, st });
            if self.eat_op(";") {
                if matches!(self.peek(), Tok::Newline) {
                    break;
                }
                continue;
            }
            break;
        }
        match self.next() {
            Tok::Newline | Tok::Eof => Ok(out),
            t => Err(err(self.line(), &format!("unexpected {t:?} at the end of a statement"))),
        }
    }
    fn small(&mut self) -> PR<St> {
        let line = self.line();
        if let Tok::Name(n) = self.peek().clone() {
            match n.as_str() {
                "pass" => {
                    self.next();
                    return Ok(St::Pass);
                }
                "break" => {
                    self.next();
                    return Ok(St::Break);
                }
                "continue" => {
                    self.next();
                    return Ok(St::Continue);
                }
                "return" => {
                    self.next();
                    if matches!(self.peek(), Tok::Newline) || self.is_op(";") {
                        return Ok(St::Return(None));
                    }
                    return Ok(St::Return(Some(self.exprlist_star()?)));
                }
                "raise" => {
                    self.next();
                    if matches!(self.peek(), Tok::Newline) || self.is_op(";") {
                        return Ok(St::Raise(None, None));
                    }
                    let e = self.expr()?;
                    let c = if self.eat_kw("from") { Some(self.expr()?) } else { None };
                    return Ok(St::Raise(Some(e), c));
                }
                "global" | "nonlocal" => {
                    self.next();
                    let mut ns = vec![self.name()?];
                    while self.eat_op(",") {
                        ns.push(self.name()?);
                    }
                    return Ok(if n == "global" { St::Global(ns) } else { St::Nonlocal(ns) });
                }
                "del" => {
                    self.next();
                    let mut ts = vec![self.or_expr_level()?];
                    while self.eat_op(",") {
                        if matches!(self.peek(), Tok::Newline) {
                            break;
                        }
                        ts.push(self.or_expr_level()?);
                    }
                    return Ok(St::Del(ts));
                }
                "assert" => {
                    self.next();
                    let c = self.expr()?;
                    let m = if self.eat_op(",") { Some(self.expr()?) } else { None };
                    return Ok(St::Assert(c, m));
                }
                "import" => {
                    self.next();
                    let mut v = vec![];
                    loop {
                        let mut m = self.name()?;
                        while self.eat_op(".") {
                            m.push('.');
                            m.push_str(&self.name()?);
                        }
                        let a = if self.eat_kw("as") { Some(self.name()?) } else { None };
                        v.push((m, a));
                        if !self.eat_op(",") {
                            break;
                        }
                    }
                    return Ok(St::Import(v));
                }
                "from" => {
                    self.next();
                    let mut m = String::new();
                    while self.is_op(".") || self.is_op("...") {
                        m.push_str(if self.eat_op(".") { "." } else {
                            self.next();
                            "..."
                        });
                    }
                    if !self.is_kw("import") {
                        m.push_str(&self.name()?);
                        while self.eat_op(".") {
                            m.push('.');
                            m.push_str(&self.name()?);
                        }
                    }
                    self.expect_kw("import")?;
                    let paren = self.eat_op("(");
                    let mut v = vec![];
                    if self.eat_op("*") {
                        v.push(("*".to_string(), None));
                    } else {
                        loop {
                            if paren && self.is_op(")") {
                                break;
                            }
                            let n = self.name()?;
                            let a = if self.eat_kw("as") { Some(self.name()?) } else { None };
                            v.push((n, a));
                            if !self.eat_op(",") {
                                break;
                            }
                        }
                    }
                    if paren {
                        self.expect_op(")")?;
                    }
                    return Ok(St::ImportFrom(m, v));
                }
                _ => {}
            }
        }
        // expression statement or assignment
        let first = self.exprlist_star()?;
        if self.is_op(":") {
            // annotated assignment
            self.next();
            let _ann = self.expr()?;
            let v = if self.eat_op("=") { Some(self.exprlist_star_or_yield()?) } else { None };
            check_target(&first, line)?;
            return Ok(St::AnnAssign(first, v));
        }
        for op in ["+=", "-=", "*=", "/=", "//=", "%=", "**=", "&=", "|=", "^=", ">>=", "<<=", "@="] {
            if self.is_op(op) {
                self.next();
                let v = self.exprlist_star_or_yield()?;
                check_target(&first, line)?;
                let bop: &'static str = match op {
                    "+=" => "+",
                    "-=" => "-",
                    "*=" => "*",
                    "/=" => "/",
                    "//=" => "//",
                    "%=" => "%",
                    "**=" => "**",
                    "&=" => "&",
                    "|=" => "|",
                    "^=" => "^",
                    ">>=" => ">>",
                    "<<=" => "<<",
                    _ => "@",
                };
                return Ok(St::AugAssign(first, bop, v));
            }
        }
        if self.is_op("=") {
            let mut targets = vec![first];
            let mut value;
            loop {
                self.next();
                value = self.exprlist_star_or_yield()?;
                if self.is_op("=") {
                    targets.push(value);
                    continue;
                }
                break;
            }
            for t in &targets {
                check_target(t, line)?;
            }
            return Ok(St::Assign(targets, value));
        }
        Ok(St::Expr(first))
    }
    fn funcdef(&mut self, decorators: Vec<Ex>) -> PR<FuncDef> {
        let line = self.line();
        self.expect_kw("def")?;
        let name = self.name()?;
        self.expect_op("(")?;
        let params = self.params(")")?;
        self.expect_op(")")?;
        if self.eat_op("->") {
            self.expr()?;
        }
        let body = self.block()?;
        Ok(FuncDef { name, params, body, decorators, line })
    }
    fn params(&mut self, end: &str) -> PR<Params> {
        let mut p = Params::default();
        let mut kwonly = false;
        loop {
            if self.is_op(end) {
                break;
            }
            if self.eat_op("/") {
                // positional-only marker: treat as ordinary
            } else if self.eat_op("**") {
                p.kwstar = Some(self.name()?);
                if end == ")" && self.eat_op(":") {
                    self.expr()?;
                }
            } else if self.eat_op("*") {
                kwonly = true;
                if !self.is_op(",") && !self.is_op(end) {
                    p.star = Some(self.name()?);
                    if end == ")" && self.eat_op(":") {
                        self.expr()?;
                    }
                }
            } else {
                let n = self.name()?;
                if end == ")" && self.eat_op(":") {
                    self.expr()?;
                }
                let d = if self.eat_op("=") { Some(self.expr()?) } else { None };
                if kwonly {
                    p.kwonly.push((n, d));
                } else {
                    p.pos.push((n, d));
                }
            }
            if !self.eat_op(",") {
                break;
            }
        }
        Ok(p)
    }
    fn classdef(&mut self, decorators: Vec<Ex>) -> PR<ClassDef> {
        let line = self.line();
        self.expect_kw("class")?;
        let name = self.name()?;
        let mut bases = vec![];
        if self.eat_op("(") {
            while !self.is_op(")") {
                if let Tok::Name(n) = self.peek().clone() {
                    if matches!(self.toks.get(self.i + 1).map(|t| &t.0), Some(Tok::Op("="))) {
                        if n == "metaclass" {
                            // ABCMeta is harmless; others are refused
                            self.next();
                            self.next();
                            let m = self.expr()?;
                            if !matches!(&m, Ex::Name(x) if x == "ABCMeta") && !matches!(&m, Ex::Attr(_, x) if x == "ABCMeta") {
                                return Err(err(line, "metaclasses are not supported"));
                            }
                            if !self.eat_op(",") {
                                break;
                            }
                            continue;
                        }
                        return Err(err(line, "class keyword arguments are not supported"));
                    }
                }
                bases.push(self.expr()?);
                if !self.eat_op(",") {
                    break;
                }
            }
            self.expect_op(")")?;
        }
        let body = self.block()?;
        Ok(ClassDef { name, bases, body, decorators, line })
    }

    // ---- expressions ----
    fn exprlist_star_or_yield(&mut self) -> PR<Ex> {
        if self.is_kw("yield") {
            return self.yield_expr();
        }
        self.exprlist_star()
    }
    fn yield_expr(&mut self) -> PR<Ex> {
        self.expect_kw("yield")?;
        if self.eat_kw("from") {
            return Ok(Ex::YieldFrom(Box::new(self.expr()?)));
        }
        if matches!(self.peek(), Tok::Newline) || self.is_op(")") || self.is_op(";") {
            return Ok(Ex::Yield(None));
        }
        Ok(Ex::Yield(Some(Box::new(self.exprlist_star()?))))
    }
    // a, b = ...  /  return a, *b
    fn exprlist_star(&mut self) -> PR<Ex> {
        if self.is_kw("yield") {
            return self.yield_expr();
        }
        let first = self.star_or_expr()?;
        if !self.is_op(",") {
            return Ok(first);
        }
        let mut v = vec![first];
        while self.eat_op(",") {
            if self.ends_list() {
                break;
            }
            v.push(self.star_or_expr()?);
        }
        Ok(Ex::Tuple(v))
    }
    fn ends_list(&self) -> bool {
        matches!(self.peek(), Tok::Newline | Tok::Eof) || self.is_op("=") || self.is_op(")") || self.is_op("]") || self.is_op("}") || self.is_op(":") || self.is_op(";") || self.is_kw("in") || ["+=", "-=", "*=", "/=", "//=", "%=", "|=", "&=", "^=", "**=", ">>=", "<<="].iter().any(|o| self.is_op(o))
    }
    fn exprlist(&mut self) -> PR<Ex> {
        self.exprlist_star()
    }
    fn star_or_expr(&mut self) -> PR<Ex> {
        if self.eat_op("*") {
            return Ok(Ex::Star(Box::new(self.or_expr_level()?)));
        }
        self.expr()
    }
    fn target(&mut self) -> PR<Ex> {
        let line = self.line();
        let t = self.or_expr_level()?;
        check_target(&t, line)?;
        Ok(t)
    }
    fn target_list(&mut self) -> PR<Ex> {
        let line = self.line();
        let first = if self.eat_op("*") { Ex::Star(Box::new(self.or_expr_level()?)) } else { self.or_expr_level()? };
        let t = if self.is_op(",") {
            let mut v = vec![first];
            while self.eat_op(",") {
                if self.is_kw("in") || self.is_op("=") {
                    break;
                }
                v.push(if self.eat_op("*") { Ex::Star(Box::new(self.or_expr_level()?)) } else { self.or_expr_level()? });
            }
            Ex::Tuple(v)
        } else {
            first
        };
        check_target(&t, line)?;
        Ok(t)
    }
    pub fn expr(&mut self) -> PR<Ex> {
        if self.is_kw("lambda") {
            self.next();
            let p = self.params(":")?;
            self.expect_op(":")?;
            let b = self.expr()?;
            return Ok(Ex::Lambda(Box::new(p), Box::new(b)));
        }
        if let (Tok::Name(n), Some(Tok::Op(":="))) = (self.peek().clone(), self.toks.get(self.i + 1).map(|t| t.0.clone())) {
            self.next();
            self.next();
            let v = self.expr()?;
            return Ok(Ex::Walrus(n, Box::new(v)));
        }
        let e = self.or_test()?;
        if self.is_kw("if") {
            // conditional expression (but not a comprehension's if: handled by the caller stopping)
            let save = self.i;
            self.next();
            let c = self.or_test()?;
            if self.eat_kw("else") {
                let other = self.expr()?;
                return Ok(Ex::IfExp(Box::new(c), Box::new(e), Box::new(other)));
            }
            self.i = save;
        }
        Ok(e)
    }
    fn expr_nocond(&mut self) -> PR<Ex> {
        if self.is_kw("lambda") {
            return self.expr();
        }
        self.or_test()
    }
    fn or_test(&mut self) -> PR<Ex> {
        let mut e = self.and_test()?;
        while self.eat_kw("or") {
            let r = self.and_test()?;
            e = Ex::Or(Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn and_test(&mut self) -> PR<Ex> {
        let mut e = self.not_test()?;
        while self.eat_kw("and") {
            let r = self.not_test()?;
            e = Ex::And(Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn not_test(&mut self) -> PR<Ex> {
        if self.eat_kw("not") {
            return Ok(Ex::Unary("not", Box::new(self.not_test()?)));
        }
        self.comparison()
    }
    fn comparison(&mut self) -> PR<Ex> {
        let first = self.or_expr_level()?;
        let mut rest = vec![];
        loop {
            let op: &'static str = if self.eat_op("<") {
                "<"
            } else if self.eat_op(">") {
                ">"
            } else if self.eat_op("==") {
                "=="
            } else if self.eat_op("!=") {
                "!="
            } else if self.eat_op("<=") {
                "<="
            } else if self.eat_op(">=") {
                ">="
            } else if self.is_kw("in") {
                self.next();
                "in"
            } else if self.is_kw("not") && matches!(self.toks.get(self.i + 1).map(|t| &t.0), Some(Tok::Name(n)) if n == "in") {
                self.next();
                self.next();
                "not in"
            } else if self.eat_kw("is") {
                if self.eat_kw("not") {
                    "is not"
                } else {
                    "is"
                }
            } else {
                break;
            };
            rest.push((op, self.or_expr_level()?));
        }
        if rest.is_empty() {
            Ok(first)
        } else {
            Ok(Ex::Cmp(Box::new(first), rest))
        }
    }
    pub fn or_expr_level(&mut self) -> PR<Ex> {
        self.bin_level(0)
    }
    fn bin_level(&mut self, lvl: usize) -> PR<Ex> {
        const LEVELS: [&[&str]; 6] = [&["|"], &["^"], &["&"], &["<<", ">>"], &["+", "-"], &["*", "/", "//", "%", "@"]];
        if lvl == LEVELS.len() {
            return self.unary();
        }
        let mut e = self.bin_level(lvl + 1)?;
        loop {
            let mut found = None;
            for op in LEVELS[lvl] {
                if self.is_op(op) {
                    found = Some(*op);
                    break;
                }
            }
            match found {
                Some(op) => {
                    self.next();
                    let r = self.bin_level(lvl + 1)?;
                    e = Ex::Bin(op, Box::new(e), Box::new(r));
                }
                None => break,
            }
        }
        Ok(e)
    }
    fn unary(&mut self) -> PR<Ex> {
        if self.eat_op("-") {
            return Ok(Ex::Unary("-", Box::new(self.unary()?)));
        }
        if self.eat_op("+") {
            return Ok(Ex::Unary("+", Box::new(self.unary()?)));
        }
        if self.eat_op("~") {
            return Ok(Ex::Unary("~", Box::new(self.unary()?)));
        }
        self.power()
    }
    fn power(&mut self) -> PR<Ex> {
        if self.is_kw("await") {
            return Err(err(self.line(), "await is not supported"));
        }
        let base = self.primary()?;
        if self.eat_op("**") {
            let e = self.unary()?;
            return Ok(Ex::Bin("**", Box::new(base), Box::new(e)));
        }
        Ok(base)
    }
    fn primary(&mut self) -> PR<Ex> {
        let mut e = self.atom()?;
        loop {
            if self.eat_op(".") {
                let n = match self.next() {
                    Tok::Name(n) => n,
                    t => return Err(err(self.line(), &format!("expected an attribute name, found {t:?}"))),
                };
                e = Ex::Attr(Box::new(e), n);
            } else if self.eat_op("(") {
                let args = self.call_args()?;
                self.expect_op(")")?;
                e = Ex::Call(Box::new(e), args);
            } else if self.eat_op("[") {
                let idx = self.subscript()?;
                self.expect_op("]")?;
                e = Ex::Index(Box::new(e), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(e)
    }
    fn subscript(&mut self) -> PR<Ex> {
        let first = self.slice_item()?;
        if self.is_op(",") {
            let mut v = vec![first];
            while self.eat_op(",") {
                if self.is_op("]") {
                    break;
                }
                v.push(self.slice_item()?);
            }
            return Ok(Ex::Tuple(v));
        }
        Ok(first)
    }
    fn slice_item(&mut self) -> PR<Ex> {
        let a = if self.is_op(":") { None } else { Some(Box::new(self.star_or_expr()?)) };
        if !self.eat_op(":") {
            return Ok(*a.unwrap());
        }
        let b = if self.is_op(":") || self.is_op("]") || self.is_op(",") { None } else { Some(Box::new(self.expr()?)) };
        let c = if self.eat_op(":") {
            if self.is_op("]") || self.is_op(",") {
                None
            } else {
                Some(Box::new(self.expr()?))
            }
        } else {
            None
        };
        Ok(Ex::Slice(a, b, c))
    }
    fn call_args(&mut self) -> PR<Vec<Arg>> {
        let mut args = vec![];
        while !self.is_op(")") {
            if self.eat_op("**") {
                args.push(Arg::KwStar(self.expr()?));
            } else if self.eat_op("*") {
                args.push(Arg::Star(self.expr()?));
            } else if let (Tok::Name(n), Some(Tok::Op("="))) = (self.peek().clone(), self.toks.get(self.i + 1).map(|t| t.0.clone())) {
                self.next();
                self.next();
                args.push(Arg::Kw(n, self.expr()?));
            } else {
                let e = self.expr()?;
                if self.is_kw("for") {
                    let gens = self.comp_fors()?;
                    args.push(Arg::Pos(Ex::Comp('g', Box::new(e), None, gens)));
                } else {
                    args.push(Arg::Pos(e));
                }
            }
            if !self.eat_op(",") {
                break;
            }
        }
        Ok(args)
    }
    fn comp_fors(&mut self) -> PR<Vec<CompFor>> {
        let mut gens = vec![];
        while self.is_kw("for") || (self.is_kw("async")) {
            if self.is_kw("async") {
                return Err(err(self.line(), "async comprehensions are not supported"));
            }
            self.next();
            let t = self.target_list()?;
            self.expect_kw("in")?;
            let it = self.or_test()?;
            let mut ifs = vec![];
            while self.is_kw("if") {
                self.next();
                ifs.push(self.expr_nocond()?);
            }
            gens.push(CompFor { target: t, iter: it, ifs });
        }
        Ok(gens)
    }
    fn atom(&mut self) -> PR<Ex> {
        let line = self.line();
        match self.next() {
            Tok::Name(n) => match n.as_str() {
                "None" => Ok(Ex::NoneV),
                "True" => Ok(Ex::True),
                "False" => Ok(Ex::False),
                "yield" => Err(err(line, "yield here is not supported")),
                k if is_keyword(k) => Err(err(line, &format!("unexpected '{k}'"))),
                _ => Ok(Ex::Name(n)),
            },
            Tok::Int(t) => Ok(Ex::Int(t)),
            Tok::Float(f) => Ok(Ex::Float(f)),
            Tok::Imag => Err(err(line, "complex numbers are not supported")),
            Tok::Bytes(b0) => {
                // adjacent literals concatenate; b'...' 'x' mixes kinds, a SyntaxError in Python
                let mut b = b0;
                loop {
                    match self.peek().clone() {
                        Tok::Bytes(x) => {
                            b.push_str(&x);
                            self.next();
                        }
                        Tok::Str(..) | Tok::FStr(..) => return Err(err(line, "cannot mix bytes and nonbytes literals")),
                        _ => break,
                    }
                }
                Ok(Ex::Call(Box::new(Ex::Name("__bytes_lit".into())), vec![Arg::Pos(Ex::Str(b))]))
            }
            Tok::Op("...") => Ok(Ex::Ellipsis),
            Tok::Str(s0, _) | Tok::FStr(s0, _) => {
                self.i -= 1;
                // adjacent literals concatenate
                let mut parts: Vec<FPart> = vec![];
                let mut any_f = false;
                loop {
                    match self.peek().clone() {
                        Tok::Str(s, _) => {
                            parts.push(FPart::Lit(s));
                            self.next();
                        }
                        Tok::FStr(body, raw) => {
                            any_f = true;
                            let l = self.line();
                            parts.extend(parse_fstring(&body, raw, l)?);
                            self.next();
                        }
                        Tok::Bytes(_) => return Err(err(line, "cannot mix bytes and nonbytes literals")),
                        _ => break,
                    }
                }
                let _ = s0;
                if !any_f {
                    let mut s = String::new();
                    for p in parts {
                        if let FPart::Lit(x) = p {
                            s.push_str(&x);
                        }
                    }
                    return Ok(Ex::Str(s));
                }
                Ok(Ex::FStr(parts))
            }
            Tok::Op("(") => {
                if self.eat_op(")") {
                    return Ok(Ex::Tuple(vec![]));
                }
                if self.is_kw("yield") {
                    let y = self.yield_expr()?;
                    self.expect_op(")")?;
                    return Ok(y);
                }
                let first = self.star_or_expr()?;
                if self.is_kw("for") {
                    let gens = self.comp_fors()?;
                    self.expect_op(")")?;
                    return Ok(Ex::Comp('g', Box::new(first), None, gens));
                }
                if self.eat_op(")") {
                    return Ok(first);
                }
                let mut v = vec![first];
                while self.eat_op(",") {
                    if self.is_op(")") {
                        break;
                    }
                    v.push(self.star_or_expr()?);
                }
                self.expect_op(")")?;
                Ok(Ex::Tuple(v))
            }
            Tok::Op("[") => {
                if self.eat_op("]") {
                    return Ok(Ex::List(vec![]));
                }
                let first = self.star_or_expr()?;
                if self.is_kw("for") {
                    let gens = self.comp_fors()?;
                    self.expect_op("]")?;
                    return Ok(Ex::Comp('l', Box::new(first), None, gens));
                }
                let mut v = vec![first];
                while self.eat_op(",") {
                    if self.is_op("]") {
                        break;
                    }
                    v.push(self.star_or_expr()?);
                }
                self.expect_op("]")?;
                Ok(Ex::List(v))
            }
            Tok::Op("{") => {
                if self.eat_op("}") {
                    return Ok(Ex::Dict(vec![]));
                }
                if self.eat_op("**") {
                    let v = self.or_expr_level()?;
                    let mut items = vec![(None, v)];
                    self.dict_rest(&mut items)?;
                    return Ok(Ex::Dict(items));
                }
                let first = self.star_or_expr()?;
                if self.eat_op(":") {
                    let v = self.expr()?;
                    if self.is_kw("for") {
                        let gens = self.comp_fors()?;
                        self.expect_op("}")?;
                        return Ok(Ex::Comp('d', Box::new(first), Some(Box::new(v)), gens));
                    }
                    let mut items = vec![(Some(first), v)];
                    self.dict_rest(&mut items)?;
                    return Ok(Ex::Dict(items));
                }
                if self.is_kw("for") {
                    let gens = self.comp_fors()?;
                    self.expect_op("}")?;
                    return Ok(Ex::Comp('s', Box::new(first), None, gens));
                }
                let mut v = vec![first];
                while self.eat_op(",") {
                    if self.is_op("}") {
                        break;
                    }
                    v.push(self.star_or_expr()?);
                }
                self.expect_op("}")?;
                Ok(Ex::Set(v))
            }
            t => Err(err(line, &format!("unexpected {t:?}"))),
        }
    }
    fn dict_rest(&mut self, items: &mut Vec<(Option<Ex>, Ex)>) -> PR<()> {
        while self.eat_op(",") {
            if self.is_op("}") {
                break;
            }
            if self.eat_op("**") {
                items.push((None, self.or_expr_level()?));
                continue;
            }
            let k = self.expr()?;
            self.expect_op(":")?;
            let v = self.expr()?;
            items.push((Some(k), v));
        }
        self.expect_op("}")
    }
}

fn is_keyword(n: &str) -> bool {
    matches!(
        n,
        "False" | "None" | "True" | "and" | "as" | "assert" | "async" | "await" | "break" | "class" | "continue" | "def" | "del" | "elif" | "else" | "except" | "finally" | "for" | "from" | "global" | "if" | "import" | "in" | "is" | "lambda" | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" | "try" | "while" | "with" | "yield"
    )
}

fn check_target(t: &Ex, line: usize) -> PR<()> {
    match t {
        Ex::Name(_) | Ex::Attr(..) | Ex::Index(..) => Ok(()),
        Ex::Tuple(v) | Ex::List(v) => {
            let stars = v.iter().filter(|x| matches!(x, Ex::Star(_))).count();
            if stars > 1 {
                return Err(err(line, "multiple starred expressions in assignment"));
            }
            for x in v {
                match x {
                    Ex::Star(inner) => check_target(inner, line)?,
                    _ => check_target(x, line)?,
                }
            }
            Ok(())
        }
        _ => Err(err(line, "cannot assign to this expression")),
    }
}

// f-string body to parts
pub fn parse_fstring(body: &str, raw: bool, line: usize) -> PR<Vec<FPart>> {
    let cs: Vec<char> = body.chars().collect();
    let mut parts = vec![];
    let mut lit = String::new();
    let mut i = 0;
    let flush = |lit: &mut String, parts: &mut Vec<FPart>| -> PR<()> {
        if !lit.is_empty() {
            let v = if raw { lit.clone() } else { unescape(lit, line)? };
            parts.push(FPart::Lit(v));
            lit.clear();
        }
        Ok(())
    };
    while i < cs.len() {
        let c = cs[i];
        if c == '{' {
            if i + 1 < cs.len() && cs[i + 1] == '{' {
                lit.push('{');
                i += 2;
                continue;
            }
            flush(&mut lit, &mut parts)?;
            // find the expression end: a top-level '!', ':' or '}'
            let mut j = i + 1;
            let mut depth = 0;
            let mut quote: Option<char> = None;
            let mut expr_end = None;
            let mut conv = None;
            let mut spec_start = None;
            while j < cs.len() {
                let d = cs[j];
                if let Some(q) = quote {
                    if d == q {
                        quote = None;
                    }
                    j += 1;
                    continue;
                }
                match d {
                    '\'' | '"' => quote = Some(d),
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' => depth -= 1,
                    '}' if depth > 0 => depth -= 1,
                    '}' => {
                        if expr_end.is_none() {
                            expr_end = Some(j);
                        }
                        break;
                    }
                    '!' if depth == 0 && j + 1 < cs.len() && cs[j + 1] != '=' && expr_end.is_none() => {
                        expr_end = Some(j);
                        conv = Some(cs[j + 1]);
                        j += 2;
                        if j < cs.len() && cs[j] == ':' {
                            spec_start = Some(j + 1);
                            // skip to matching close brace
                            let mut d2 = 0;
                            j += 1;
                            while j < cs.len() {
                                if cs[j] == '{' {
                                    d2 += 1;
                                } else if cs[j] == '}' {
                                    if d2 == 0 {
                                        break;
                                    }
                                    d2 -= 1;
                                }
                                j += 1;
                            }
                        }
                        break;
                    }
                    ':' if depth == 0 && expr_end.is_none() => {
                        expr_end = Some(j);
                        spec_start = Some(j + 1);
                        let mut d2 = 0;
                        j += 1;
                        while j < cs.len() {
                            if cs[j] == '{' {
                                d2 += 1;
                            } else if cs[j] == '}' {
                                if d2 == 0 {
                                    break;
                                }
                                d2 -= 1;
                            }
                            j += 1;
                        }
                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            if j >= cs.len() {
                return Err(err(line, "f-string: expecting '}'"));
            }
            let end = expr_end.unwrap_or(j);
            let mut text: String = cs[i + 1..end].iter().collect();
            // `{x=}` debugging form
            let mut self_doc = None;
            let trimmed = text.trim_end();
            if trimmed.ends_with('=') && !trimmed.ends_with("==") && !trimmed.ends_with("!=") && !trimmed.ends_with("<=") && !trimmed.ends_with(">=") {
                self_doc = Some(text.clone());
                text = trimmed[..trimmed.len() - 1].to_string();
            }
            let spec_parts = match spec_start {
                Some(s0) => {
                    let spec_text: String = cs[s0..j].iter().collect();
                    parse_fstring(&spec_text, true, line)?
                }
                None => vec![],
            };
            let lx = lex(&format!("({})", text.replace('\n', " "))).map_err(|(_, m)| err(line, &format!("f-string: {m}")))?;
            let mut p = P { toks: lx.toks, i: 0 };
            let e = p.expr().map_err(|(_, m)| err(line, &format!("f-string: {m}")))?;
            if let Some(d) = self_doc {
                parts.push(FPart::Lit(d));
                let conv = conv.or(if spec_parts.is_empty() { Some('r') } else { None });
                parts.push(FPart::Expr(Box::new(e), conv, spec_parts, String::new()));
            } else {
                parts.push(FPart::Expr(Box::new(e), conv, spec_parts, String::new()));
            }
            i = j + 1;
        } else if c == '}' {
            if i + 1 < cs.len() && cs[i + 1] == '}' {
                lit.push('}');
                i += 2;
                continue;
            }
            return Err(err(line, "f-string: single '}' is not allowed"));
        } else {
            lit.push(c);
            i += 1;
        }
    }
    flush(&mut lit, &mut parts)?;
    Ok(parts)
}

pub fn parse(src: &str) -> PR<Vec<Stmt>> {
    let lx = lex(src)?;
    let mut p = P { toks: lx.toks, i: 0 };
    p.file()
}
