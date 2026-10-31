use std::io::{stdin, Read};

#[derive(Clone, Debug)]
enum J { Null, Bool(bool), Num(String), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }

struct P { b: Vec<char>, i: usize }
impl P {
    fn ws(&mut self) { while self.i < self.b.len() && self.b[self.i].is_whitespace() { self.i += 1; } }
    fn val(&mut self) -> J {
        self.ws();
        let c = self.b[self.i];
        match c {
            '{' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b[self.i] == '}' { self.i += 1; return J::Obj(v); }
                loop {
                    self.ws();
                    let k = match self.val() { J::Str(s) => s, _ => String::new() };
                    self.ws(); self.i += 1;
                    let x = self.val();
                    v.push((k, x));
                    self.ws();
                    let d = self.b[self.i]; self.i += 1;
                    if d == '}' { return J::Obj(v); }
                }
            }
            '[' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b[self.i] == ']' { self.i += 1; return J::Arr(v); }
                loop {
                    v.push(self.val());
                    self.ws();
                    let d = self.b[self.i]; self.i += 1;
                    if d == ']' { return J::Arr(v); }
                }
            }
            '"' => {
                self.i += 1; let mut s = String::new();
                loop {
                    let c = self.b[self.i]; self.i += 1;
                    match c {
                        '"' => break,
                        '\\' => {
                            let e = self.b[self.i]; self.i += 1;
                            match e {
                                'n' => s.push('\n'), 't' => s.push('\t'), 'r' => s.push('\r'),
                                'b' => s.push('\u{8}'), 'f' => s.push('\u{c}'),
                                'u' => {
                                    let h: String = self.b[self.i..self.i + 4].iter().collect();
                                    self.i += 4;
                                    let n = u32::from_str_radix(&h, 16).unwrap_or(63);
                                    if (0xD800..0xDC00).contains(&n) && self.b.get(self.i) == Some(&'\\') && self.b.get(self.i + 1) == Some(&'u') {
                                        let h2: String = self.b[self.i + 2..self.i + 6].iter().collect();
                                        let m = u32::from_str_radix(&h2, 16).unwrap_or(0);
                                        self.i += 6;
                                        s.push(char::from_u32(0x10000 + ((n - 0xD800) << 10) + (m.wrapping_sub(0xDC00) & 0x3ff)).unwrap_or('?'));
                                    } else { s.push(char::from_u32(n).unwrap_or('?')); }
                                }
                                o => s.push(o),
                            }
                        }
                        c => s.push(c),
                    }
                }
                J::Str(s)
            }
            't' => { self.i += 4; J::Bool(true) }
            'f' => { self.i += 5; J::Bool(false) }
            'n' => { self.i += 4; J::Null }
            _ => {
                let st = self.i;
                while self.i < self.b.len() && "+-.eE0123456789".contains(self.b[self.i]) { self.i += 1; }
                J::Num(self.b[st..self.i].iter().collect())
            }
        }
    }
}

fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"", '\\' => o += "\\\\", '\n' => o += "\\n", '\r' => o += "\\r", '\t' => o += "\\t",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o.push('"'); o
}
fn compact(j: &J) -> String {
    match j {
        J::Null => "null".into(), J::Bool(b) => b.to_string(), J::Num(n) => n.clone(), J::Str(s) => jstr(s),
        J::Arr(a) => format!("[{}]", a.iter().map(compact).collect::<Vec<_>>().join(",")),
        J::Obj(o) => format!("{{{}}}", o.iter().map(|(k, v)| format!("{}:{}", jstr(k), compact(v))).collect::<Vec<_>>().join(",")),
    }
}
fn show(j: &Option<J>) -> String {
    match j {
        None | Some(J::Null) => String::new(),
        Some(J::Str(s)) => s.clone(),
        Some(J::Num(n)) => n.clone(),
        Some(J::Bool(b)) => b.to_string(),
        Some(o) => compact(o),
    }
}
fn truthy(j: &Option<J>) -> bool {
    match j {
        None | Some(J::Null) => false,
        Some(J::Bool(b)) => *b,
        Some(J::Num(n)) => n.parse::<f64>().map_or(true, |x| x != 0.0),
        Some(J::Str(s)) => !s.is_empty(),
        Some(J::Arr(a)) => !a.is_empty(),
        Some(J::Obj(_)) => true,
    }
}

enum N { Text(String), Var(String), Each(String, Vec<N>), If(String, Vec<N>, Vec<N>) }

fn tokenize(t: &str) -> Vec<Result<String, String>> {
    // Ok(text), Err(tag content)
    let mut v = vec![];
    let mut rest = t;
    loop {
        match rest.find("{{") {
            None => { if !rest.is_empty() { v.push(Ok(rest.to_string())); } break; }
            Some(a) => match rest[a + 2..].find("}}") {
                None => { v.push(Ok(rest.to_string())); break; }
                Some(b) => {
                    if a > 0 { v.push(Ok(rest[..a].to_string())); }
                    v.push(Err(rest[a + 2..a + 2 + b].trim().to_string()));
                    rest = &rest[a + 2 + b + 2..];
                }
            },
        }
    }
    v
}

// returns nodes and terminator ("" at eof, "else", "/each", "/if")
fn parse(toks: &[Result<String, String>], pos: &mut usize) -> (Vec<N>, String) {
    let mut out = vec![];
    while *pos < toks.len() {
        let t = &toks[*pos];
        *pos += 1;
        match t {
            Ok(s) => out.push(N::Text(s.clone())),
            Err(tag) => {
                if tag == "else" || tag == "/each" || tag == "/if" { return (out, tag.clone()); }
                if let Some(r) = tag.strip_prefix("#each") {
                    let (body, _) = parse(toks, pos);
                    out.push(N::Each(r.trim().to_string(), body));
                } else if let Some(r) = tag.strip_prefix("#if") {
                    let (a, term) = parse(toks, pos);
                    let b = if term == "else" { parse(toks, pos).0 } else { vec![] };
                    out.push(N::If(r.trim().to_string(), a, b));
                } else {
                    out.push(N::Var(tag.clone()));
                }
            }
        }
    }
    (out, String::new())
}

struct Sc { val: J, idx: Option<usize> }

fn lookup(path: &str, sc: &[Sc]) -> Option<J> {
    if path == "." { return sc.last().map(|s| s.val.clone()); }
    if path == "@index" {
        return sc.iter().rev().find_map(|s| s.idx).map(|i| J::Num(i.to_string()));
    }
    let segs: Vec<&str> = path.split('.').collect();
    for s in sc.iter().rev() {
        if let J::Obj(o) = &s.val {
            if let Some((_, v)) = o.iter().rev().find(|(k, _)| k == segs[0]) {
                let mut cur = v.clone();
                for seg in &segs[1..] {
                    cur = match &cur {
                        J::Obj(o) => match o.iter().rev().find(|(k, _)| k == seg) { Some((_, v)) => v.clone(), None => return None },
                        J::Arr(a) => match seg.parse::<usize>().ok().and_then(|i| a.get(i)) { Some(v) => v.clone(), None => return None },
                        _ => return None,
                    };
                }
                return Some(cur);
            }
        }
    }
    None
}

fn eval(expr: &str, sc: &[Sc]) -> Option<J> {
    let mut parts = expr.split('|');
    let mut v = lookup(parts.next().unwrap().trim(), sc);
    for f in parts {
        let f = f.trim();
        v = match f {
            "upper" => Some(J::Str(show(&v).to_uppercase())),
            "lower" => Some(J::Str(show(&v).to_lowercase())),
            "len" => Some(J::Num(match &v {
                Some(J::Str(s)) => s.chars().count(),
                Some(J::Arr(a)) => a.len(),
                Some(J::Obj(o)) => o.len(),
                _ => 0,
            }.to_string())),
            _ => {
                if let Some(t) = f.strip_prefix("default:") {
                    match &v { None | Some(J::Null) => Some(J::Str(t.to_string())), Some(J::Str(s)) if s.is_empty() => Some(J::Str(t.to_string())), _ => v }
                } else { v }
            }
        };
    }
    v
}

fn render(ns: &[N], sc: &mut Vec<Sc>, out: &mut String) {
    for n in ns {
        match n {
            N::Text(t) => out.push_str(t),
            N::Var(e) => out.push_str(&show(&eval(e, sc))),
            N::Each(e, body) => {
                if let Some(J::Arr(a)) = eval(e, sc) {
                    for (i, x) in a.into_iter().enumerate() {
                        sc.push(Sc { val: x, idx: Some(i) });
                        render(body, sc, out);
                        sc.pop();
                    }
                }
            }
            N::If(e, a, b) => {
                if truthy(&eval(e, sc)) { render(a, sc, out); } else { render(b, sc, out); }
            }
        }
    }
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let (first, tpl) = match s.find('\n') { Some(i) => (&s[..i], &s[i + 1..]), None => (&s[..], "") };
    let mut p = P { b: first.chars().collect(), i: 0 };
    let data = p.val();
    let toks = tokenize(tpl);
    let mut pos = 0;
    let (tree, _) = parse(&toks, &mut pos);
    let mut sc = vec![Sc { val: data, idx: None }];
    let mut out = String::new();
    render(&tree, &mut sc, &mut out);
    print!("{}", out);
}
