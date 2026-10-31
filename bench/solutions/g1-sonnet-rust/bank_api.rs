use std::io::{stdin, Read};

#[derive(Clone, Debug)]
enum J { Null, Bool(bool), Int(i64), Float, Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }

struct P<'a> { b: &'a [u8], i: usize }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() { self.i += 1; } }
    fn val(&mut self) -> Option<J> {
        self.ws();
        let c = *self.b.get(self.i)?;
        match c {
            b'{' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b.get(self.i) == Some(&b'}') { self.i += 1; return Some(J::Obj(v)); }
                loop {
                    self.ws();
                    let k = match self.val()? { J::Str(s) => s, _ => return None };
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') { return None; }
                    self.i += 1;
                    let x = self.val()?;
                    v.push((k, x));
                    self.ws();
                    match self.b.get(self.i)? { b',' => self.i += 1, b'}' => { self.i += 1; return Some(J::Obj(v)); } _ => return None }
                }
            }
            b'[' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b.get(self.i) == Some(&b']') { self.i += 1; return Some(J::Arr(v)); }
                loop {
                    v.push(self.val()?);
                    self.ws();
                    match self.b.get(self.i)? { b',' => self.i += 1, b']' => { self.i += 1; return Some(J::Arr(v)); } _ => return None }
                }
            }
            b'"' => {
                self.i += 1; let mut s = Vec::new();
                loop {
                    let c = *self.b.get(self.i)?; self.i += 1;
                    match c {
                        b'"' => break,
                        b'\\' => {
                            let e = *self.b.get(self.i)?; self.i += 1;
                            match e {
                                b'n' => s.push(b'\n'), b't' => s.push(b'\t'), b'r' => s.push(b'\r'),
                                b'b' => s.push(8), b'f' => s.push(12),
                                b'"' | b'\\' | b'/' => s.push(e),
                                b'u' => {
                                    let h = std::str::from_utf8(self.b.get(self.i..self.i + 4)?).ok()?;
                                    let n = u32::from_str_radix(h, 16).ok()?; self.i += 4;
                                    let ch = char::from_u32(n).unwrap_or('?');
                                    let mut buf = [0u8; 4];
                                    s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                }
                                _ => return None,
                            }
                        }
                        c if c < 0x20 => return None,
                        c => s.push(c),
                    }
                }
                Some(J::Str(String::from_utf8_lossy(&s).to_string()))
            }
            b't' => self.lit("true", J::Bool(true)),
            b'f' => self.lit("false", J::Bool(false)),
            b'n' => self.lit("null", J::Null),
            b'-' | b'0'..=b'9' => {
                let st = self.i;
                if self.b[self.i] == b'-' { self.i += 1; }
                let ds = self.i;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() { self.i += 1; }
                if self.i == ds { return None; }
                if self.b[ds] == b'0' && self.i - ds > 1 { return None; }
                let mut fl = false;
                if self.b.get(self.i) == Some(&b'.') {
                    fl = true; self.i += 1; let d = self.i;
                    while self.i < self.b.len() && self.b[self.i].is_ascii_digit() { self.i += 1; }
                    if self.i == d { return None; }
                }
                if matches!(self.b.get(self.i), Some(b'e') | Some(b'E')) {
                    fl = true; self.i += 1;
                    if matches!(self.b.get(self.i), Some(b'+') | Some(b'-')) { self.i += 1; }
                    let d = self.i;
                    while self.i < self.b.len() && self.b[self.i].is_ascii_digit() { self.i += 1; }
                    if self.i == d { return None; }
                }
                if fl { return Some(J::Float); }
                let t = std::str::from_utf8(&self.b[st..self.i]).ok()?;
                Some(t.parse::<i64>().map(J::Int).unwrap_or(J::Float))
            }
            _ => None,
        }
    }
    fn lit(&mut self, w: &str, v: J) -> Option<J> {
        if self.b[self.i..].starts_with(w.as_bytes()) { self.i += w.len(); Some(v) } else { None }
    }
}

fn parse(s: &str) -> Option<J> {
    let mut p = P { b: s.as_bytes(), i: 0 };
    let v = p.val()?;
    p.ws();
    if p.i != p.b.len() { return None; }
    Some(v)
}

fn get<'a>(j: &'a J, k: &str) -> Option<&'a J> {
    if let J::Obj(v) = j { v.iter().rev().find(|(a, _)| a == k).map(|(_, x)| x) } else { None }
}

struct Acc { owner: String, bal: i64 }

fn show(id: usize, a: &Acc) -> String {
    format!("{{\"id\":{},\"owner\":\"{}\",\"balance\":{}}}", id, a.owner, a.bal)
}

fn amount(j: &J) -> Option<i64> {
    match get(j, "amount") { Some(J::Int(n)) if *n > 0 => Some(*n), _ => None }
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let mut accs: Vec<Acc> = vec![];
    let nf = "404 {\"error\":\"not found\"}".to_string();
    let noacc = "404 {\"error\":\"no such account\"}".to_string();
    let badj = "400 {\"error\":\"bad json\"}".to_string();
    let bada = "400 {\"error\":\"bad amount\"}".to_string();
    let insuf = "409 {\"error\":\"insufficient funds\"}".to_string();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut it = line.splitn(3, ' ');
        let method = it.next().unwrap_or("");
        let path = it.next().unwrap_or("");
        let body = it.next().unwrap_or("").trim();
        let segs: Vec<&str> = path.split('/').collect();
        // segs[0] is "" for valid paths
        let valid = segs.len() >= 2 && segs[0].is_empty();
        let parts: Vec<&str> = if valid { segs[1..].to_vec() } else { vec![] };
        let idof = |t: &str| -> Option<usize> {
            if !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit()) { t.parse::<usize>().ok() } else { None }
        };
        // route classification
        #[derive(PartialEq)]
        enum R { Create, GetOne(usize), Dep(usize), Wd(usize), Tr, List, None }
        let r = match (method, parts.as_slice()) {
            ("POST", ["accounts"]) => R::Create,
            ("GET", ["accounts"]) => R::List,
            ("GET", ["accounts", i]) => idof(i).map(R::GetOne).unwrap_or(R::None),
            ("POST", ["accounts", i, "deposit"]) => idof(i).map(R::Dep).unwrap_or(R::None),
            ("POST", ["accounts", i, "withdraw"]) => idof(i).map(R::Wd).unwrap_or(R::None),
            ("POST", ["transfer"]) => R::Tr,
            _ => R::None,
        };
        let res: String = (|| -> String {
            if r == R::None { return nf.clone(); }
            let mut j = J::Null;
            if matches!(r, R::Create | R::Dep(_) | R::Wd(_) | R::Tr) {
                match parse(body) { Some(x) => j = x, None => return badj.clone() }
            }
            match r {
                R::Create => match get(&j, "owner") {
                    Some(J::Str(o)) if !o.is_empty() => {
                        accs.push(Acc { owner: o.clone(), bal: 0 });
                        format!("201 {}", show(accs.len(), accs.last().unwrap()))
                    }
                    _ => "400 {\"error\":\"owner required\"}".to_string(),
                },
                R::GetOne(i) => if i >= 1 && i <= accs.len() { format!("200 {}", show(i, &accs[i - 1])) } else { noacc.clone() },
                R::List => {
                    let mut v: Vec<usize> = (1..=accs.len()).collect();
                    v.sort_by(|a, b| accs[b - 1].bal.cmp(&accs[a - 1].bal).then(a.cmp(b)));
                    format!("200 [{}]", v.iter().map(|&i| show(i, &accs[i - 1])).collect::<Vec<_>>().join(","))
                }
                R::Dep(i) | R::Wd(i) => {
                    if i < 1 || i > accs.len() { return noacc.clone(); }
                    let n = match amount(&j) { Some(n) => n, None => return bada.clone() };
                    if let R::Wd(_) = r {
                        if n > accs[i - 1].bal { return insuf.clone(); }
                        accs[i - 1].bal -= n;
                    } else { accs[i - 1].bal += n; }
                    format!("200 {}", show(i, &accs[i - 1]))
                }
                R::Tr => {
                    let f = match get(&j, "from") { Some(J::Int(n)) if *n >= 1 && (*n as usize) <= accs.len() => Some(*n as usize), _ => None };
                    let t = match get(&j, "to") { Some(J::Int(n)) if *n >= 1 && (*n as usize) <= accs.len() => Some(*n as usize), _ => None };
                    let (f, t) = match (f, t) { (Some(f), Some(t)) => (f, t), _ => return noacc.clone() };
                    if f == t { return "400 {\"error\":\"same account\"}".to_string(); }
                    let n = match amount(&j) { Some(n) => n, None => return bada.clone() };
                    if n > accs[f - 1].bal { return insuf.clone(); }
                    accs[f - 1].bal -= n; accs[t - 1].bal += n;
                    format!("200 {{\"from\":{},\"to\":{}}}", accs[f - 1].bal, accs[t - 1].bal)
                }
                R::None => nf.clone(),
            }
        })();
        println!("{}", res);
    }
}
