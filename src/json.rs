// Small strict JSON reader: rustc's --error-format=json output and the MCP server's JSON-RPC messages.
// Strict on purpose: the MCP server reads whatever a client sends, so bad input is refused (None) instead of
// being half-read, \u escapes with surrogate pairs decode to the real character, and nesting is limited so
// deep input cannot overflow the stack.
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

const MAX_DEPTH: usize = 200;

pub fn parse(s: &str) -> Option<J> {
    let b = s.as_bytes();
    let mut i = 0;
    let v = value(b, &mut i, 0)?;
    ws(b, &mut i);
    (i == b.len()).then_some(v)
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn lit(b: &[u8], i: &mut usize, word: &str, v: J) -> Option<J> {
    if b[*i..].starts_with(word.as_bytes()) {
        *i += word.len();
        Some(v)
    } else {
        None
    }
}

fn value(b: &[u8], i: &mut usize, depth: usize) -> Option<J> {
    if depth > MAX_DEPTH {
        return None;
    }
    ws(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut kv = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(J::Obj(kv));
            }
            loop {
                ws(b, i);
                if b.get(*i) != Some(&b'"') {
                    return None;
                }
                let k = string(b, i)?;
                ws(b, i);
                if b.get(*i) != Some(&b':') {
                    return None;
                }
                *i += 1;
                let v = value(b, i, depth + 1)?;
                kv.push((k, v));
                ws(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(J::Obj(kv));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut a = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(J::Arr(a));
            }
            loop {
                a.push(value(b, i, depth + 1)?);
                ws(b, i);
                match b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(J::Arr(a));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => string(b, i).map(J::Str),
        b't' => lit(b, i, "true", J::Bool(true)),
        b'f' => lit(b, i, "false", J::Bool(false)),
        b'n' => lit(b, i, "null", J::Null),
        b'-' | b'0'..=b'9' => {
            let start = *i;
            if b[*i] == b'-' {
                *i += 1;
            }
            let digits = |i: &mut usize| {
                let s = *i;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    *i += 1;
                }
                *i > s
            };
            if !digits(i) {
                return None;
            }
            if b.get(*i) == Some(&b'.') {
                *i += 1;
                if !digits(i) {
                    return None;
                }
            }
            if matches!(b.get(*i), Some(b'e' | b'E')) {
                *i += 1;
                if matches!(b.get(*i), Some(b'+' | b'-')) {
                    *i += 1;
                }
                if !digits(i) {
                    return None;
                }
            }
            std::str::from_utf8(&b[start..*i])
                .ok()?
                .parse()
                .ok()
                .map(J::Num)
        }
        _ => None,
    }
}

fn hex4(b: &[u8], i: usize) -> Option<u32> {
    let h = std::str::from_utf8(b.get(i..i + 4)?).ok()?;
    if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(h, 16).ok()
}

// At the opening quote; leaves i after the closing quote.
fn string(b: &[u8], i: &mut usize) -> Option<String> {
    *i += 1;
    let mut s: Vec<u8> = Vec::new();
    loop {
        let c = *b.get(*i)?;
        match c {
            b'"' => {
                *i += 1;
                return String::from_utf8(s).ok();
            }
            b'\\' => {
                *i += 1;
                let e = *b.get(*i)?;
                *i += 1;
                let ch = match e {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'u' => {
                        let hi = hex4(b, *i)?;
                        *i += 4;
                        if (0xD800..0xDC00).contains(&hi) {
                            // A surrogate pair: the low half must follow. A lone half becomes U+FFFD.
                            match (b.get(*i..*i + 2), hex4(b, *i + 2)) {
                                (Some(br"\u"), Some(lo)) if (0xDC00..0xE000).contains(&lo) => {
                                    *i += 6;
                                    char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                                        .unwrap_or('\u{FFFD}')
                                }
                                _ => '\u{FFFD}',
                            }
                        } else {
                            char::from_u32(hi).unwrap_or('\u{FFFD}')
                        }
                    }
                    _ => return None,
                };
                let mut buf = [0u8; 4];
                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            c if c < 0x20 => return None,
            c => {
                s.push(c);
                *i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_and_unicode() {
        assert_eq!(
            parse(r#""hé 😀 \/ \b""#).unwrap().str(),
            Some("hé 😀 / \u{8}")
        );
        assert_eq!(parse(r#""\ud83d x""#).unwrap().str(), Some("\u{FFFD} x"));
        assert_eq!(
            parse(r#"{"a":[1,-2.5e3,true,false,null],"b":{}}"#)
                .unwrap()
                .get("a")
                .unwrap()
                .arr()
                .len(),
            5
        );
        for bad in [
            "",
            "{",
            "[1,]x",
            "{\"a\" 1}",
            "tru",
            "{\"a\":1} x",
            "01x",
            "\"a",
            "[1 2]",
            "{\"a\":1,}",
            "-",
            "1.",
            "\"\u{1}\"",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
        assert!(parse(&"[".repeat(100_000)).is_none());
        assert!(parse(&format!("{}{}", "[".repeat(150), "]".repeat(150))).is_some());
    }
}
