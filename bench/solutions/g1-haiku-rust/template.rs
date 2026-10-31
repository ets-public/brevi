use std::io::{self, Read, Write};

// ---------- minimal JSON ----------

#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn lit(&mut self, word: &str) -> Option<()> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Some(())
        } else {
            None
        }
    }

    fn value(&mut self) -> Option<J> {
        self.ws();
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(J::Str),
            b't' => self.lit("true").map(|_| J::Bool(true)),
            b'f' => self.lit("false").map(|_| J::Bool(false)),
            b'n' => self.lit("null").map(|_| J::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn digits(&mut self) -> Option<usize> {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i > start {
            Some(self.i - start)
        } else {
            None
        }
    }

    fn number(&mut self) -> Option<J> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        self.digits()?;
        if self.peek() == Some(b'.') {
            self.i += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            self.digits()?;
        }
        let txt = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        Some(J::Num(txt.parse::<f64>().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    match e {
                        b'"' => buf.push(b'"'),
                        b'\\' => buf.push(b'\\'),
                        b'/' => buf.push(b'/'),
                        b'b' => buf.push(8),
                        b'f' => buf.push(12),
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return None;
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4]).ok()?;
                            let code = u32::from_str_radix(hex, 16).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                            let mut tmp = [0u8; 4];
                            buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).ok()
    }

    fn array(&mut self) -> Option<J> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Some(J::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Some(J::Arr(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self) -> Option<J> {
        self.i += 1;
        let mut fields = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Some(J::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.string()?;
            self.ws();
            if self.peek()? != b':' {
                return None;
            }
            self.i += 1;
            let val = self.value()?;
            fields.push((key, val));
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Some(J::Obj(fields));
                }
                _ => return None,
            }
        }
    }
}

fn parse_json(s: &str) -> Option<J> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i == p.s.len() {
        Some(v)
    } else {
        None
    }
}

fn fmt_num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e18 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn compact(v: &J) -> String {
    match v {
        J::Null => "null".to_string(),
        J::Bool(b) => b.to_string(),
        J::Num(n) => fmt_num(*n),
        J::Str(s) => json_str(s),
        J::Arr(items) => {
            let parts: Vec<String> = items.iter().map(compact).collect();
            format!("[{}]", parts.join(","))
        }
        J::Obj(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}:{}", json_str(k), compact(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

// ---------- template ----------

enum Tok {
    Text(String),
    Tag(String),
}

enum Filter {
    Upper,
    Lower,
    Len,
    Default(String),
}

enum Node {
    Text(String),
    Var(String, Vec<Filter>),
    Each(String, Vec<Node>),
    If(String, Vec<Node>, Vec<Node>),
}

fn tokenize(t: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut rest = t;
    loop {
        match rest.find("{{") {
            None => {
                if !rest.is_empty() {
                    toks.push(Tok::Text(rest.to_string()));
                }
                break;
            }
            Some(start) => {
                let after = &rest[start + 2..];
                match after.find("}}") {
                    None => {
                        toks.push(Tok::Text(rest.to_string()));
                        break;
                    }
                    Some(end) => {
                        if start > 0 {
                            toks.push(Tok::Text(rest[..start].to_string()));
                        }
                        toks.push(Tok::Tag(after[..end].trim().to_string()));
                        rest = &after[end + 2..];
                    }
                }
            }
        }
    }
    toks
}

// Returns the rest of a tag after a keyword such as "#each", if the keyword matches.
fn keyword<'a>(tag: &'a str, kw: &str) -> Option<&'a str> {
    let rest = tag.strip_prefix(kw)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest.trim())
    } else {
        None
    }
}

fn parse_var(tag: &str) -> Node {
    let mut parts = tag.split('|');
    let path = parts.next().unwrap_or("").trim().to_string();
    let mut filters = Vec::new();
    for f in parts {
        let f = f.trim();
        if f == "upper" {
            filters.push(Filter::Upper);
        } else if f == "lower" {
            filters.push(Filter::Lower);
        } else if f == "len" {
            filters.push(Filter::Len);
        } else if let Some(t) = f.strip_prefix("default:") {
            filters.push(Filter::Default(t.to_string()));
        }
    }
    Node::Var(path, filters)
}

fn parse(toks: &[Tok], pos: &mut usize, stops: &[&str]) -> Vec<Node> {
    let mut nodes = Vec::new();
    while *pos < toks.len() {
        match &toks[*pos] {
            Tok::Text(s) => {
                nodes.push(Node::Text(s.clone()));
                *pos += 1;
            }
            Tok::Tag(c) => {
                if stops.contains(&c.as_str()) {
                    break;
                }
                if let Some(name) = keyword(c, "#each") {
                    let name = name.to_string();
                    *pos += 1;
                    let body = parse(toks, pos, &["/each"]);
                    *pos += 1; // closing {{/each}}
                    nodes.push(Node::Each(name, body));
                } else if let Some(name) = keyword(c, "#if") {
                    let name = name.to_string();
                    *pos += 1;
                    let then = parse(toks, pos, &["else", "/if"]);
                    let mut els = Vec::new();
                    if *pos < toks.len() {
                        if let Tok::Tag(t) = &toks[*pos] {
                            if t == "else" {
                                *pos += 1;
                                els = parse(toks, pos, &["/if"]);
                            }
                        }
                    }
                    *pos += 1; // closing {{/if}}
                    nodes.push(Node::If(name, then, els));
                } else if c.starts_with('/') || c == "else" {
                    // stray block marker: ignored
                    *pos += 1;
                } else {
                    nodes.push(parse_var(c));
                    *pos += 1;
                }
            }
        }
    }
    nodes
}

fn path_get<'a>(v: &'a J, path: &str) -> Option<&'a J> {
    let mut cur = v;
    for seg in path.split('.') {
        cur = match cur {
            J::Obj(fields) => fields.iter().find(|(k, _)| k == seg).map(|(_, x)| x)?,
            J::Arr(items) => items.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

struct Ctx<'a> {
    root: &'a J,
    // Loop frames: (element, index). Innermost is last.
    frames: Vec<(J, usize)>,
}

impl<'a> Ctx<'a> {
    fn lookup(&self, name: &str) -> Option<J> {
        if name == "." {
            return Some(match self.frames.last() {
                Some((e, _)) => e.clone(),
                None => self.root.clone(),
            });
        }
        if name == "@index" {
            return self.frames.last().map(|(_, i)| J::Num(*i as f64));
        }
        for (elem, _) in self.frames.iter().rev() {
            if let Some(v) = path_get(elem, name) {
                return Some(v.clone());
            }
        }
        path_get(self.root, name).cloned()
    }

    fn render(&mut self, nodes: &[Node], out: &mut String) {
        for n in nodes {
            match n {
                Node::Text(s) => out.push_str(s),
                Node::Var(path, filters) => {
                    let mut v = self.lookup(path);
                    for f in filters {
                        v = apply(v, f);
                    }
                    if let Some(v) = v {
                        out.push_str(&text_of(&v));
                    }
                }
                Node::Each(name, body) => {
                    if let Some(J::Arr(items)) = self.lookup(name) {
                        for (i, item) in items.into_iter().enumerate() {
                            self.frames.push((item, i));
                            self.render(body, out);
                            self.frames.pop();
                        }
                    }
                }
                Node::If(name, then, els) => {
                    if truthy(self.lookup(name).as_ref()) {
                        self.render(then, out);
                    } else {
                        self.render(els, out);
                    }
                }
            }
        }
    }
}

fn text_of(v: &J) -> String {
    match v {
        J::Str(s) => s.clone(),
        J::Null => String::new(),
        other => compact(other),
    }
}

fn apply(v: Option<J>, f: &Filter) -> Option<J> {
    match f {
        Filter::Upper => match v {
            Some(J::Str(s)) => Some(J::Str(s.to_uppercase())),
            Some(x @ (J::Num(_) | J::Bool(_))) => Some(J::Str(compact(&x).to_uppercase())),
            other => other,
        },
        Filter::Lower => match v {
            Some(J::Str(s)) => Some(J::Str(s.to_lowercase())),
            Some(x @ (J::Num(_) | J::Bool(_))) => Some(J::Str(compact(&x).to_lowercase())),
            other => other,
        },
        Filter::Len => match v {
            Some(J::Str(s)) => Some(J::Num(s.chars().count() as f64)),
            Some(J::Arr(a)) => Some(J::Num(a.len() as f64)),
            Some(J::Obj(o)) => Some(J::Num(o.len() as f64)),
            _ => None,
        },
        Filter::Default(t) => match v {
            None | Some(J::Null) => Some(J::Str(t.clone())),
            Some(J::Str(s)) if s.is_empty() => Some(J::Str(t.clone())),
            other => other,
        },
    }
}

fn truthy(v: Option<&J>) -> bool {
    match v {
        None | Some(J::Null) => false,
        Some(J::Bool(b)) => *b,
        Some(J::Num(n)) => *n != 0.0,
        Some(J::Str(s)) => !s.is_empty(),
        Some(J::Arr(a)) => !a.is_empty(),
        Some(J::Obj(_)) => true,
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let (first, template) = match input.split_once('\n') {
        Some((a, b)) => (a, b),
        None => (input.as_str(), ""),
    };
    let data = parse_json(first.trim()).unwrap_or(J::Obj(Vec::new()));

    let toks = tokenize(template);
    let mut pos = 0;
    let nodes = parse(&toks, &mut pos, &[]);

    let mut ctx = Ctx {
        root: &data,
        frames: Vec::new(),
    };
    let mut out = String::new();
    ctx.render(&nodes, &mut out);

    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    w.write_all(out.as_bytes()).unwrap();
    w.flush().unwrap();
}
