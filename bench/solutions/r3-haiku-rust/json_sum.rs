use std::io::{self, Read};

struct Val {
    sum: i64,
    red: bool, // true if this value is the string "red"
}

struct P {
    b: Vec<char>,
    i: usize,
}

impl P {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_whitespace() {
            self.i += 1;
        }
    }

    fn string(&mut self) -> String {
        // assumes current char is '"'
        self.i += 1;
        let mut s = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            if c == '"' {
                break;
            }
            if c == '\\' && self.i < self.b.len() {
                s.push(self.b[self.i]);
                self.i += 1;
            } else {
                s.push(c);
            }
        }
        s
    }

    fn value(&mut self) -> Val {
        self.ws();
        if self.i >= self.b.len() {
            return Val { sum: 0, red: false };
        }
        match self.b[self.i] {
            '{' => {
                self.i += 1;
                let mut sum: i64 = 0;
                let mut has_red = false;
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    let c = self.b[self.i];
                    if c == '}' {
                        self.i += 1;
                        break;
                    }
                    if c == ',' {
                        self.i += 1;
                        continue;
                    }
                    if c == '"' {
                        let _key = self.string();
                        self.ws();
                        if self.i < self.b.len() && self.b[self.i] == ':' {
                            self.i += 1;
                        }
                        let v = self.value();
                        sum += v.sum;
                        if v.red {
                            has_red = true;
                        }
                    } else {
                        self.i += 1;
                    }
                }
                if has_red {
                    sum = 0;
                }
                Val { sum, red: false }
            }
            '[' => {
                self.i += 1;
                let mut sum: i64 = 0;
                loop {
                    self.ws();
                    if self.i >= self.b.len() {
                        break;
                    }
                    let c = self.b[self.i];
                    if c == ']' {
                        self.i += 1;
                        break;
                    }
                    if c == ',' {
                        self.i += 1;
                        continue;
                    }
                    let v = self.value();
                    sum += v.sum;
                }
                Val { sum, red: false }
            }
            '"' => {
                let s = self.string();
                Val { sum: 0, red: s == "red" }
            }
            c if c == '-' || c.is_ascii_digit() => {
                let mut neg = false;
                if c == '-' {
                    neg = true;
                    self.i += 1;
                }
                let mut n: i64 = 0;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    n = n * 10 + (self.b[self.i] as u8 - b'0') as i64;
                    self.i += 1;
                }
                Val { sum: if neg { -n } else { n }, red: false }
            }
            _ => {
                // true, false, null or anything unexpected: skip a word
                while self.i < self.b.len()
                    && self.b[self.i].is_ascii_alphabetic()
                {
                    self.i += 1;
                }
                if self.i < self.b.len() && self.b[self.i] != ',' && !self.b[self.i].is_whitespace()
                    && self.b[self.i] != '}' && self.b[self.i] != ']'
                {
                    self.i += 1;
                }
                Val { sum: 0, red: false }
            }
        }
    }
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut p = P { b: s.chars().collect(), i: 0 };
    let v = p.value();
    println!("{}", v.sum);
}
