use std::io::{self, Read};

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(i128),
    Op(char),
    LP,
    RP,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let cs: Vec<char> = s.chars().collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_ascii_digit() {
            let mut n: i128 = 0;
            while i < cs.len() && cs[i].is_ascii_digit() {
                n = n * 10 + (cs[i] as u8 - b'0') as i128;
                i += 1;
            }
            v.push(Tok::Num(n));
            continue;
        }
        match c {
            '+' | '-' | '*' | '/' => v.push(Tok::Op(c)),
            '(' => v.push(Tok::LP),
            ')' => v.push(Tok::RP),
            _ => {}
        }
        i += 1;
    }
    v
}

#[derive(Debug)]
struct DivZero;

struct Parser {
    t: Vec<Tok>,
    p: usize,
}

impl Parser {
    fn peek(&self) -> Option<Tok> {
        self.t.get(self.p).cloned()
    }

    fn expr(&mut self) -> Result<i128, DivZero> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some(Tok::Op('+')) => {
                    self.p += 1;
                    let r = self.term()?;
                    v += r;
                }
                Some(Tok::Op('-')) => {
                    self.p += 1;
                    let r = self.term()?;
                    v -= r;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<i128, DivZero> {
        let mut v = self.factor()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.p += 1;
                    let r = self.factor()?;
                    v *= r;
                }
                Some(Tok::Op('/')) => {
                    self.p += 1;
                    let r = self.factor()?;
                    if r == 0 {
                        return Err(DivZero);
                    }
                    v /= r;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    fn factor(&mut self) -> Result<i128, DivZero> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.p += 1;
                Ok(-self.factor()?)
            }
            Some(Tok::Op('+')) => {
                self.p += 1;
                self.factor()
            }
            Some(Tok::Num(n)) => {
                self.p += 1;
                Ok(n)
            }
            Some(Tok::LP) => {
                self.p += 1;
                let v = self.expr()?;
                if let Some(Tok::RP) = self.peek() {
                    self.p += 1;
                }
                Ok(v)
            }
            _ => {
                self.p += 1;
                Ok(0)
            }
        }
    }
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut out = String::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut p = Parser { t: tokenize(line), p: 0 };
        match p.expr() {
            Ok(v) => out.push_str(&format!("{}\n", v)),
            Err(_) => out.push_str("error\n"),
        }
    }
    print!("{}", out);
}
