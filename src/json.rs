// Minimal JSON reader, only for rustc's --error-format=json output.
#[derive(Debug, Clone)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(kv) => kv.iter().find(|(key, _)| key == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            J::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
    pub fn truthy(&self) -> bool {
        matches!(self, J::Bool(true))
    }
}

pub fn parse(s: &str) -> Option<J> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let v = value(&c, &mut i)?;
    Some(v)
}

fn ws(c: &[char], i: &mut usize) {
    while *i < c.len() && c[*i].is_whitespace() {
        *i += 1;
    }
}

fn value(c: &[char], i: &mut usize) -> Option<J> {
    ws(c, i);
    match c.get(*i)? {
        '{' => {
            *i += 1;
            let mut kv = Vec::new();
            loop {
                ws(c, i);
                if c.get(*i) == Some(&'}') {
                    *i += 1;
                    return Some(J::Obj(kv));
                }
                let k = match value(c, i)? {
                    J::Str(s) => s,
                    _ => return None,
                };
                ws(c, i);
                *i += 1; // ':'
                let v = value(c, i)?;
                kv.push((k, v));
                ws(c, i);
                if c.get(*i) == Some(&',') {
                    *i += 1;
                }
            }
        }
        '[' => {
            *i += 1;
            let mut a = Vec::new();
            loop {
                ws(c, i);
                if c.get(*i) == Some(&']') {
                    *i += 1;
                    return Some(J::Arr(a));
                }
                a.push(value(c, i)?);
                ws(c, i);
                if c.get(*i) == Some(&',') {
                    *i += 1;
                }
            }
        }
        '"' => {
            *i += 1;
            let mut s = String::new();
            while *i < c.len() && c[*i] != '"' {
                if c[*i] == '\\' {
                    *i += 1;
                    match c.get(*i)? {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        'u' => {
                            let hex: String = c.get(*i + 1..*i + 5)?.iter().collect();
                            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                s.push(ch);
                            }
                            *i += 4;
                        }
                        other => s.push(*other),
                    }
                } else {
                    s.push(c[*i]);
                }
                *i += 1;
            }
            *i += 1;
            Some(J::Str(s))
        }
        't' => {
            *i += 4;
            Some(J::Bool(true))
        }
        'f' => {
            *i += 5;
            Some(J::Bool(false))
        }
        'n' => {
            *i += 4;
            Some(J::Null)
        }
        _ => {
            let start = *i;
            while *i < c.len() && (c[*i].is_ascii_digit() || "-+.eE".contains(c[*i])) {
                *i += 1;
            }
            let t: String = c[start..*i].iter().collect();
            t.parse().ok().map(J::Num)
        }
    }
}
