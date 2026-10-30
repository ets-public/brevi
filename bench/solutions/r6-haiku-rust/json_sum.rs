use std::io::{self, Read, Write};

struct P {
    s: Vec<u8>,
    i: usize,
}

impl P {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }
    // returns decoded string content
    fn string(&mut self) -> String {
        let mut out: Vec<u8> = Vec::new();
        // assumes current char is opening quote
        self.i += 1;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'"' {
                self.i += 1;
                break;
            }
            if c == b'\\' && self.i + 1 < self.s.len() {
                let n = self.s[self.i + 1];
                self.i += 2;
                match n {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'u' => {
                        // skip 4 hex digits; keep a placeholder
                        self.i += 4;
                        out.push(b'?');
                    }
                    other => out.push(other),
                }
                continue;
            }
            out.push(c);
            self.i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    // returns (sum of numbers counted, is_red_string)
    fn value(&mut self) -> (i64, bool) {
        self.ws();
        if self.i >= self.s.len() {
            return (0, false);
        }
        match self.s[self.i] {
            b'{' => {
                self.i += 1;
                let mut sum: i64 = 0;
                let mut red = false;
                loop {
                    self.ws();
                    if self.i >= self.s.len() {
                        break;
                    }
                    match self.s[self.i] {
                        b'}' => {
                            self.i += 1;
                            break;
                        }
                        b',' => {
                            self.i += 1;
                        }
                        b'"' => {
                            let _key = self.string();
                            self.ws();
                            if self.i < self.s.len() && self.s[self.i] == b':' {
                                self.i += 1;
                            }
                            let (v, is_red) = self.value();
                            sum = sum.wrapping_add(v);
                            if is_red {
                                red = true;
                            }
                        }
                        _ => {
                            self.i += 1;
                        }
                    }
                }
                if red {
                    (0, false)
                } else {
                    (sum, false)
                }
            }
            b'[' => {
                self.i += 1;
                let mut sum: i64 = 0;
                loop {
                    self.ws();
                    if self.i >= self.s.len() {
                        break;
                    }
                    match self.s[self.i] {
                        b']' => {
                            self.i += 1;
                            break;
                        }
                        b',' => {
                            self.i += 1;
                        }
                        _ => {
                            let (v, _) = self.value();
                            sum = sum.wrapping_add(v);
                        }
                    }
                }
                (sum, false)
            }
            b'"' => {
                let s = self.string();
                (0, s == "red")
            }
            b'-' | b'0'..=b'9' => {
                let mut neg = false;
                if self.s[self.i] == b'-' {
                    neg = true;
                    self.i += 1;
                }
                let mut v: i64 = 0;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    v = v.wrapping_mul(10).wrapping_add((self.s[self.i] - b'0') as i64);
                    self.i += 1;
                }
                // skip fraction/exponent if present (not expected)
                while self.i < self.s.len()
                    && matches!(self.s[self.i], b'.' | b'e' | b'E' | b'+' | b'-')
                {
                    self.i += 1;
                    while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                        self.i += 1;
                    }
                }
                (if neg { v.wrapping_neg() } else { v }, false)
            }
            _ => {
                // true, false, null
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
                    self.i += 1;
                }
                (0, false)
            }
        }
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut p = P { s: input.into_bytes(), i: 0 };
    let (sum, _) = p.value();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", sum).unwrap();
}
