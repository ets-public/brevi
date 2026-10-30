use std::io::{self, Read, Write};

struct P {
    b: Vec<u8>,
    i: usize,
}

impl P {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    // Parses a string starting at the opening quote; returns its unescaped content.
    fn string(&mut self) -> String {
        let mut s: Vec<u8> = Vec::new();
        self.i += 1; // opening quote
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'"' {
                self.i += 1;
                break;
            }
            if c == b'\\' && self.i + 1 < self.b.len() {
                let e = self.b[self.i + 1];
                self.i += 2;
                match e {
                    b'n' => s.push(b'\n'),
                    b't' => s.push(b'\t'),
                    b'r' => s.push(b'\r'),
                    b'b' => s.push(8),
                    b'f' => s.push(12),
                    b'u' => {
                        // keep the 4 hex digits as-is (not needed for comparisons)
                        s.push(b'?');
                        self.i += 4;
                    }
                    other => s.push(other),
                }
                continue;
            }
            s.push(c);
            self.i += 1;
        }
        String::from_utf8_lossy(&s).into_owned()
    }

    // Returns (sum of numbers counted, whether this value is the string "red").
    fn value(&mut self) -> (i128, bool) {
        self.ws();
        if self.i >= self.b.len() {
            return (0, false);
        }
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut sum: i128 = 0;
                let mut has_red = false;
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    match self.b[self.i] {
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
                            if self.i < self.b.len() && self.b[self.i] == b':' {
                                self.i += 1;
                            }
                            let (s, red) = self.value();
                            sum += s;
                            if red {
                                has_red = true;
                            }
                        }
                        _ => {
                            self.i += 1;
                        }
                    }
                }
                if has_red {
                    (0, false)
                } else {
                    (sum, false)
                }
            }
            b'[' => {
                self.i += 1;
                let mut sum: i128 = 0;
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    match self.b[self.i] {
                        b']' => {
                            self.i += 1;
                            break;
                        }
                        b',' => {
                            self.i += 1;
                        }
                        _ => {
                            let (s, _) = self.value();
                            sum += s;
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
                if self.b[self.i] == b'-' {
                    neg = true;
                    self.i += 1;
                }
                let mut n: i128 = 0;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    n = n * 10 + (self.b[self.i] - b'0') as i128;
                    self.i += 1;
                }
                // skip any fractional / exponent part (not expected)
                while self.i < self.b.len()
                    && matches!(self.b[self.i], b'.' | b'e' | b'E' | b'+' | b'-' | b'0'..=b'9')
                {
                    self.i += 1;
                }
                (if neg { -n } else { n }, false)
            }
            _ => {
                // true, false, null
                while self.i < self.b.len() && self.b[self.i].is_ascii_alphabetic() {
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
    let mut p = P {
        b: input.into_bytes(),
        i: 0,
    };
    let (sum, _) = p.value();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    writeln!(out, "{}", sum).unwrap();
}
