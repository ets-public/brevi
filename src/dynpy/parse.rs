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
            'N' => {
                let (ch, k) = crate::unicode_names::escape(&cs[i..]).ok_or_else(|| err(line, "cannot read this \\N{...} escape"))?;
                out.push(ch);
                i += k;
            }
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
            // An f-string's replacement fields may hold strings in the same quotes and, in a single-quoted f-string,
            // newlines (PEP 701, Python 3.12): skip over them as nested literals. Each open field is (bracket depth,
            // in its format spec): a `#` is a comment only in a field's expression, never in a spec (`{255:#x}`),
            // and a `{` in a spec opens a nested field (`{x:>{w}}`).
            let mut fields: Vec<(usize, bool)> = vec![];
            loop {
                if k >= cs.len() {
                    return Err(err(start_line, "unterminated string literal"));
                }
                if prefix.contains('f') {
                    match fields.last_mut() {
                        None => {
                            if cs[k] == '{' {
                                if k + 1 < cs.len() && cs[k + 1] == '{' {
                                    k += 2;
                                    continue;
                                }
                                fields.push((0, false));
                                k += 1;
                                continue;
                            }
                        }
                        Some((_, true)) => {
                            if cs[k] == '{' {
                                fields.push((0, false));
                                k += 1;
                                continue;
                            }
                            if cs[k] == '}' {
                                fields.pop();
                                k += 1;
                                continue;
                            }
                        }
                        Some((br, spec)) => {
                            match cs[k] {
                                '\n' => line += 1,
                                '\'' | '"' => {
                                    k = skip_nested_str(&cs, k, &mut line).ok_or_else(|| err(start_line, "unterminated string literal"))?;
                                    continue;
                                }
                                '#' => {
                                    while k < cs.len() && cs[k] != '\n' {
                                        k += 1;
                                    }
                                    continue;
                                }
                                '(' | '[' | '{' => *br += 1,
                                ')' | ']' => *br = br.saturating_sub(1),
                                '}' if *br > 0 => *br -= 1,
                                '}' => {
                                    fields.pop();
                                }
                                ':' if *br == 0 => *spec = true,
                                _ => {}
                            }
                            k += 1;
                            continue;
                        }
                    }
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
    pub is_async: bool,
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
    Await(Box<Ex>),
}

#[derive(Clone, Debug, Default)]
pub struct Params {
    pub pos: Vec<(String, Option<Ex>)>,
    pub star: Option<String>,
    pub kwonly: Vec<(String, Option<Ex>)>,
    pub kwstar: Option<String>,
    // the number of `pos` parameters before `/` (positional-only)
    pub posonly: usize,
    // each annotated parameter's annotation (read by the MCP signature pass, mcpsig.rs; nothing else uses them)
    pub ann: Vec<(String, Ex)>,
    // a lambda's body line, when the whole body is on that one line (0 otherwise, and for a def)
    pub line: usize,
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
    pub is_async: bool,
    // the return annotation
    pub ret: Option<Ex>,
    // PEP 695 type parameters (`def f[T, U]`): plain names only (a bound, constraints, a default, `*Ts` and `**P`
    // are refused when parsed)
    pub tparams: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ClassDef {
    pub name: String,
    pub bases: Vec<Ex>,
    pub body: Vec<Stmt>,
    pub decorators: Vec<Ex>,
    pub line: usize,
    // `total=False`, TypedDict's keyword (dynpy/typing.rs refuses it on any other class), and `metaclass=M`
    pub kwargs: Vec<(String, Ex)>,
    // the statement's first line: its first decorator's, else the class keyword's (CPython's __firstlineno__)
    pub first_line: usize,
}

#[derive(Clone, Debug)]
pub enum St {
    Expr(Ex),
    Assign(Vec<Ex>, Ex),
    AugAssign(Ex, &'static str, Ex),
    // target, value, annotation (never evaluated; dataclasses read ClassVar, InitVar and KW_ONLY off it)
    AnnAssign(Ex, Option<Ex>, Box<Ex>),
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
    // numbers the hidden names `async for` and `async with` are rewritten with
    anon: usize,
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
    // `for` or `async for`: a comprehension's first clause
    fn comp_starts(&self) -> bool {
        self.is_kw("for") || (self.is_kw("async") && matches!(self.toks.get(self.i + 1).map(|t| &t.0), Some(Tok::Name(n)) if n == "for"))
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
                let mut star: Option<bool> = None;
                while self.is_kw("except") {
                    let hl = self.line();
                    self.next();
                    let is_star = self.eat_op("*");
                    if star.is_some_and(|s| s != is_star) {
                        return Err(err(hl, "cannot have both 'except' and 'except*' on the same 'try'"));
                    }
                    star = Some(is_star);
                    if is_star && self.is_op(":") {
                        return Err(err(hl, "expected one or more exception types"));
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
                if star == Some(true) {
                    let h = self.except_star(line, hs)?;
                    St::Try(body, vec![h], els, fin)
                } else {
                    St::Try(body, hs, els, fin)
                }
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
                    let mut c = self.classdef(decs)?;
                    c.first_line = line;
                    St::Class(Box::new(c))
                } else if self.is_kw("async") {
                    self.next();
                    if !self.is_kw("def") {
                        return Err(err(self.line(), "expected def after async"));
                    }
                    let mut d = self.funcdef(decs)?;
                    d.is_async = true;
                    St::Def(Box::new(d))
                } else {
                    return Err(err(self.line(), "expected def or class after a decorator"));
                }
            }
            "async" => {
                self.next();
                if self.is_kw("def") {
                    let mut d = self.funcdef(vec![])?;
                    d.is_async = true;
                    St::Def(Box::new(d))
                } else if self.eat_kw("for") {
                    let t = self.target_list()?;
                    self.expect_kw("in")?;
                    let it = self.exprlist()?;
                    let b = self.block()?;
                    let els = if self.eat_kw("else") { Some(self.block()?) } else { None };
                    return Ok(self.async_for(line, t, it, b, els));
                } else if self.is_kw("with") {
                    let w = self.stmt()?;
                    let Some(Stmt { st: St::With(items, b), .. }) = w.into_iter().next() else {
                        return Err(err(line, "expected a with statement"));
                    };
                    return Ok(self.async_with(line, &items, b));
                } else {
                    return Err(err(line, "expected def, for or with after async"));
                }
            }
            // `type X = ...` (PEP 695): `type` is a soft keyword there
            "type" if matches!(self.toks.get(self.i + 1).map(|t| &t.0), Some(Tok::Name(_))) && matches!(self.toks.get(self.i + 2).map(|t| &t.0), Some(Tok::Op("=") | Tok::Op("["))) => {
                return Err(err(line, "the type statement (PEP 695 type aliases) is not supported in dynamic builds"))
            }
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
            let ann = self.expr()?;
            let v = if self.eat_op("=") { Some(self.exprlist_star_or_yield()?) } else { None };
            check_target(&first, line)?;
            return Ok(St::AnnAssign(first, v, Box::new(ann)));
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
    // `try: ... except* T as n: h ...` as CPython runs it: one handler for any exception, which splits it by each
    // T in turn (a plain exception counts as a group of one), runs that handler with the matching part as the
    // exception being handled, and at the end raises what is left and what the handlers raised:
    //     except BaseException as __x:
    //         __r = __x; __l = []; __g = []
    //         __m, __r = __brevi_eg_split(__r, T)
    //         if __m is not None:
    //             __g.append(__m)
    //             try:
    //                 try: raise __m
    //                 except BaseException:
    //                     n = __m
    //                     h
    //                     [del n]
    //             except BaseException as __h: __l.append(__h)
    //         ...
    //         __brevi_eg_reraise(__x, __r, __l, __g)
    fn except_star(&mut self, line: usize, hs: Vec<Handler>) -> PR<Handler> {
        self.anon += 1;
        let k = self.anon;
        let (x, r, l, g, m, h) = (format!("__brevi_sx{k}"), format!("__brevi_sr{k}"), format!("__brevi_sl{k}"), format!("__brevi_sg{k}"), format!("__brevi_sm{k}"), format!("__brevi_sh{k}"));
        let st = |st: St| Stmt { line, st };
        let name = |n: &str| Ex::Name(n.to_string());
        let call = |f: &str, a: Vec<Ex>| Ex::Call(Box::new(Ex::Name(f.to_string())), a.into_iter().map(Arg::Pos).collect());
        let append = |lst: &str, v: &str| St::Expr(Ex::Call(Box::new(Ex::Attr(Box::new(Ex::Name(lst.to_string())), "append".into())), vec![Arg::Pos(Ex::Name(v.to_string()))]));
        let mut out = vec![
            st(St::Assign(vec![name(&r)], name(&x))),
            st(St::Assign(vec![name(&l)], Ex::List(vec![]))),
            st(St::Assign(vec![name(&g)], Ex::List(vec![]))),
        ];
        for hd in hs {
            if jumps_out(&hd.body) {
                return Err(err(hd.line, "'break', 'continue' and 'return' cannot appear in an except* block"));
            }
            let t = hd.types.clone().unwrap();
            out.push(Stmt { line: hd.line, st: St::Assign(vec![Ex::Tuple(vec![name(&m), name(&r)])], call("__brevi_eg_split", vec![name(&r), t])) });
            let mut body = vec![];
            if let Some(n) = &hd.name {
                body.push(st(St::Assign(vec![name(n)], name(&m))));
            }
            body.extend(hd.body.clone());
            if let Some(n) = &hd.name {
                body.push(st(St::Del(vec![name(n)])));
            }
            let inner = st(St::Try(
                vec![st(St::Raise(Some(name(&m)), None))],
                vec![Handler { types: Some(name("BaseException")), name: None, body, line: hd.line }],
                None,
                None,
            ));
            let outer = st(St::Try(vec![inner], vec![Handler { types: Some(name("BaseException")), name: Some(h.clone()), body: vec![st(append(&l, &h))], line: hd.line }], None, None));
            out.push(Stmt {
                line: hd.line,
                st: St::If(vec![(Ex::Cmp(Box::new(name(&m)), vec![("is not", Ex::NoneV)]), vec![st(append(&g, &m)), outer])], None),
            });
        }
        out.push(st(St::Expr(call("__brevi_eg_reraise", vec![name(&x), name(&r), name(&l), name(&g)]))));
        Ok(Handler { types: Some(name("BaseException")), name: Some(x), body: out, line })
    }
    // `async for t in it: body else: els` as CPython runs it:
    //     __ai = aiter(it)
    //     while True:
    //         try: __ax = await anext(__ai)
    //         except StopAsyncIteration: [__ae = True;] break
    //         t = __ax
    //         body
    //     [if __ae: els]
    fn async_for(&mut self, line: usize, t: Ex, it: Ex, body: Vec<Stmt>, els: Option<Vec<Stmt>>) -> Vec<Stmt> {
        self.anon += 1;
        let k = self.anon;
        let (ai, ax, ae) = (format!("__brevi_ai{k}"), format!("__brevi_ax{k}"), format!("__brevi_ae{k}"));
        let st = |st: St| Stmt { line, st };
        let name = |n: &str| Ex::Name(n.to_string());
        let call = |f: &str, a: Ex| Ex::Call(Box::new(Ex::Name(f.to_string())), vec![Arg::Pos(a)]);
        let mut out = vec![st(St::Assign(vec![name(&ai)], call("aiter", it)))];
        let mut stop = vec![];
        if els.is_some() {
            out.push(st(St::Assign(vec![name(&ae)], Ex::False)));
            stop.push(st(St::Assign(vec![name(&ae)], Ex::True)));
        }
        stop.push(st(St::Break));
        let get = st(St::Try(
            vec![st(St::Assign(vec![name(&ax)], Ex::Await(Box::new(call("anext", name(&ai))))))],
            vec![Handler { types: Some(name("StopAsyncIteration")), name: None, body: stop, line }],
            None,
            None,
        ));
        let mut wb = vec![get, st(St::Assign(vec![t], name(&ax)))];
        wb.extend(body);
        // The iterator is let go when the loop ends, however it ends, as CPython's stack lets it go: an async
        // generator left part way is then finalized (closed by the loop) at the same point.
        out.push(st(St::Try(vec![st(St::While(Ex::True, wb, None))], vec![], None, Some(vec![st(St::Assign(vec![name(&ai)], Ex::NoneV))]))));
        if let Some(e) = els {
            out.push(st(St::If(vec![(name(&ae), e)], None)));
        }
        out
    }
    // `async with m as t: body` as CPython runs it (one item at a time):
    //     __am = m; __ax = type(__am).__aexit__
    //     t = await type(__am).__aenter__(__am)
    //     __ak = True
    //     try: body
    //     except BaseException as __ae:
    //         __ak = False
    //         if not await __ax(__am, type(__ae), __ae, None): raise
    //     finally:
    //         if __ak: await __ax(__am, None, None, None)
    fn async_with(&mut self, line: usize, items: &[(Ex, Option<Ex>)], body: Vec<Stmt>) -> Vec<Stmt> {
        let body = if items.len() > 1 { self.async_with(line, &items[1..], body) } else { body };
        let (m, t) = items[0].clone();
        self.anon += 1;
        let k = self.anon;
        let (am, ax, ak, ae) = (format!("__brevi_am{k}"), format!("__brevi_ax{k}"), format!("__brevi_ak{k}"), format!("__brevi_ae{k}"));
        let st = |st: St| Stmt { line, st };
        let name = |n: &str| Ex::Name(n.to_string());
        let call = |f: Ex, a: Vec<Ex>| Ex::Call(Box::new(f), a.into_iter().map(Arg::Pos).collect());
        let mut out = vec![
            st(St::Assign(vec![name(&am)], m)),
            st(St::Assign(vec![name(&ax)], call(name("__brevi_aexit"), vec![name(&am)]))),
        ];
        let enter = Ex::Await(Box::new(call(call(name("__brevi_aenter"), vec![name(&am)]), vec![name(&am)])));
        out.push(st(match t {
            Some(t) => St::Assign(vec![t], enter),
            None => St::Expr(enter),
        }));
        out.push(st(St::Assign(vec![name(&ak)], Ex::True)));
        let exit = |args: Vec<Ex>| Ex::Await(Box::new(call(name(&ax), args)));
        let h = vec![
            st(St::Assign(vec![name(&ak)], Ex::False)),
            st(St::If(
                vec![(Ex::Unary("not", Box::new(exit(vec![name(&am), call(name("type"), vec![name(&ae)]), name(&ae), Ex::NoneV]))), vec![st(St::Raise(None, None))])],
                None,
            )),
        ];
        let fin = vec![st(St::If(vec![(name(&ak), vec![st(St::Expr(exit(vec![name(&am), Ex::NoneV, Ex::NoneV, Ex::NoneV])))])], None))];
        out.push(st(St::Try(body, vec![Handler { types: Some(name("BaseException")), name: Some(ae), body: h, line }], None, Some(fin))));
        out
    }
    fn funcdef(&mut self, decorators: Vec<Ex>) -> PR<FuncDef> {
        let line = self.line();
        self.expect_kw("def")?;
        let name = self.name()?;
        let tparams = if self.is_op("[") { self.type_params()? } else { vec![] };
        self.expect_op("(")?;
        let params = self.params(")")?;
        self.expect_op(")")?;
        let ret = if self.eat_op("->") { Some(self.expr()?) } else { None };
        let body = self.block()?;
        Ok(FuncDef { name, params, body, decorators, line, is_async: false, ret, tparams })
    }
    // `[T, U]` after a def's name (PEP 695). Only plain type variables are taken: the others need CPython's lazily
    // evaluated bounds and defaults, or TypeVarTuple and ParamSpec objects, which this build does not have.
    fn type_params(&mut self) -> PR<Vec<String>> {
        let line = self.line();
        self.expect_op("[")?;
        let mut out: Vec<String> = vec![];
        while !self.is_op("]") {
            if self.is_op("*") || self.is_op("**") {
                return Err(err(line, "a `*Ts` or `**P` type parameter (PEP 695) is not supported in dynamic builds"));
            }
            let n = self.name()?;
            if self.is_op(":") || self.is_op("=") {
                return Err(err(line, "a type parameter with a bound, constraints or a default (PEP 695) is not supported in dynamic builds"));
            }
            if out.contains(&n) {
                return Err(err(line, &format!("duplicate type parameter '{n}'")));
            }
            out.push(n);
            if !self.eat_op(",") {
                break;
            }
        }
        self.expect_op("]")?;
        if out.is_empty() {
            return Err(err(line, "Type parameter list cannot be empty"));
        }
        Ok(out)
    }
    fn params(&mut self, end: &str) -> PR<Params> {
        let mut p = Params::default();
        let mut kwonly = false;
        loop {
            if self.is_op(end) {
                break;
            }
            if self.eat_op("/") {
                // positional-only marker: parameters before it cannot be passed by keyword
                p.posonly = p.pos.len();
            } else if self.eat_op("**") {
                let n = self.name()?;
                if end == ")" && self.eat_op(":") {
                    let a = self.expr()?;
                    p.ann.push((n.clone(), a));
                }
                p.kwstar = Some(n);
            } else if self.eat_op("*") {
                kwonly = true;
                if !self.is_op(",") && !self.is_op(end) {
                    let n = self.name()?;
                    if end == ")" && self.eat_op(":") {
                        let a = self.expr()?;
                        p.ann.push((n.clone(), a));
                    }
                    p.star = Some(n);
                }
            } else {
                let n = self.name()?;
                if end == ")" && self.eat_op(":") {
                    let a = self.expr()?;
                    p.ann.push((n.clone(), a));
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
        if self.is_op("[") {
            return Err(err(line, "a generic class (`class C[T]`, PEP 695) is not supported in dynamic builds"));
        }
        let mut bases = vec![];
        let mut kwargs = vec![];
        if self.eat_op("(") {
            while !self.is_op(")") {
                if let Tok::Name(n) = self.peek().clone() {
                    if matches!(self.toks.get(self.i + 1).map(|t| &t.0), Some(Tok::Op("="))) {
                        if n == "metaclass" {
                            // ABCMeta is the abc module's own (collections.abc's rules); any other is called
                            // as CPython's __build_class__ calls it (dynpy/emit.rs, meta_build)
                            self.next();
                            self.next();
                            let m = self.expr()?;
                            if !matches!(&m, Ex::Name(x) if x == "ABCMeta") && !matches!(&m, Ex::Attr(_, x) if x == "ABCMeta") {
                                kwargs.push(("metaclass".to_string(), m));
                            }
                            if !self.eat_op(",") {
                                break;
                            }
                            continue;
                        }
                        if n == "total" {
                            self.next();
                            self.next();
                            let v = self.expr()?;
                            kwargs.push((n, v));
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
        Ok(ClassDef { name, bases, body, decorators, line, kwargs, first_line: line })
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
            let mut p = self.params(":")?;
            self.expect_op(":")?;
            let l0 = self.toks.get(self.i).map_or(0, |t| t.1);
            let b = self.expr()?;
            let l1 = self.i.checked_sub(1).and_then(|k| self.toks.get(k)).map_or(0, |t| t.1);
            p.line = if l0 == l1 { l0 } else { 0 };
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
        let base = if self.eat_kw("await") { Ex::Await(Box::new(self.primary()?)) } else { self.primary()? };
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
                if self.comp_starts() {
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
        while self.comp_starts() {
            let is_async = self.eat_kw("async");
            if is_async && !self.is_kw("for") {
                return Err(err(self.line(), "expected for after async"));
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
            gens.push(CompFor { target: t, iter: it, ifs, is_async });
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
                if self.comp_starts() {
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
                if self.comp_starts() {
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
                    if self.comp_starts() {
                        let gens = self.comp_fors()?;
                        self.expect_op("}")?;
                        return Ok(Ex::Comp('d', Box::new(first), Some(Box::new(v)), gens));
                    }
                    let mut items = vec![(Some(first), v)];
                    self.dict_rest(&mut items)?;
                    return Ok(Ex::Dict(items));
                }
                if self.comp_starts() {
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
        // `\\` before a brace is one backslash, and the braces of `\N{BULLET}` are no value: unescape reads them.
        if c == '\\' && !raw && matches!(cs.get(i + 1), Some('\\' | 'N')) {
            let n = if cs[i + 1] == 'N' && cs.get(i + 2) == Some(&'{') { cs[i..].iter().position(|c| *c == '}').map_or(cs.len() - i, |e| e + 1) } else { 2 };
            lit.extend(&cs[i..i + n]);
            i += n;
            continue;
        }
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
                    parse_fstring(&spec_text, raw, line)?
                }
                None => vec![],
            };
            let lx = lex(&format!("({})", text.replace('\n', " "))).map_err(|(_, m)| err(line, &format!("f-string: {m}")))?;
            let mut p = P { toks: lx.toks, i: 0, anon: 0 };
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
    let mut p = P { toks: lx.toks, i: 0, anon: 0 };
    p.file()
}

// A break, continue or return in this block that leaves it (not inside a nested loop or function).
fn jumps_out(b: &[Stmt]) -> bool {
    b.iter().any(|s| match &s.st {
        St::Break | St::Continue | St::Return(_) => true,
        St::If(arms, els) => arms.iter().any(|(_, x)| jumps_out(x)) || els.as_deref().is_some_and(jumps_out),
        St::While(_, x, e) | St::For(_, _, x, e) => x.iter().any(|s| matches!(s.st, St::Return(_))) || e.as_deref().is_some_and(jumps_out),
        St::Try(x, hs, e, f) => jumps_out(x) || hs.iter().any(|h| jumps_out(&h.body)) || e.as_deref().is_some_and(jumps_out) || f.as_deref().is_some_and(jumps_out),
        St::With(_, x) => jumps_out(x),
        _ => false,
    })
}

// The index just past a string literal starting at cs[k] (a quote) inside an f-string field, any prefix already
// passed; nested f-strings are skipped the same way. None when it does not end.
fn skip_nested_str(cs: &[char], k: usize, line: &mut usize) -> Option<usize> {
    let mut p = k;
    // a prefix right before the quote (f, rb, ...)
    let mut b = k;
    while b > 0 && b + 2 >= k && "rRbBfFuU".contains(cs[b - 1]) {
        b -= 1;
    }
    let is_f = cs[b..k].iter().any(|c| *c == 'f' || *c == 'F');
    let q = cs[p];
    let triple = p + 2 < cs.len() && cs[p + 1] == q && cs[p + 2] == q;
    p += if triple { 3 } else { 1 };
    let mut depth = 0usize;
    while p < cs.len() {
        let c = cs[p];
        if c == '\\' {
            p += 2;
            continue;
        }
        if c == '\n' {
            *line += 1;
        }
        if is_f && c == '{' {
            if depth == 0 && p + 1 < cs.len() && cs[p + 1] == '{' {
                p += 2;
                continue;
            }
            depth += 1;
        } else if is_f && c == '}' && depth > 0 {
            depth -= 1;
        } else if is_f && depth > 0 && (c == '\'' || c == '"') {
            p = skip_nested_str(cs, p, line)?;
            continue;
        } else if c == q && depth == 0 {
            if !triple {
                return Some(p + 1);
            }
            if p + 2 < cs.len() && cs[p + 1] == q && cs[p + 2] == q {
                return Some(p + 3);
            }
        }
        p += 1;
    }
    None
}
