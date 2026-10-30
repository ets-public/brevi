use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i128),
    Op(char),
    LP,
    RP,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let b: Vec<char> = s.chars().collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let mut n: i128 = 0;
            while i < b.len() && b[i].is_ascii_digit() {
                n = n * 10 + (b[i] as i128 - '0' as i128);
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

struct Parser {
    t: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<Tok> {
        self.t.get(self.pos).cloned()
    }

    fn expr(&mut self) -> Result<i128, ()> {
        let mut acc = self.term()?;
        loop {
            match self.peek() {
                Some(Tok::Op('+')) => {
                    self.pos += 1;
                    acc += self.term()?;
                }
                Some(Tok::Op('-')) => {
                    self.pos += 1;
                    acc -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(acc)
    }

    fn term(&mut self) -> Result<i128, ()> {
        let mut acc = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.pos += 1;
                    acc *= self.unary()?;
                }
                Some(Tok::Op('/')) => {
                    self.pos += 1;
                    let r = self.unary()?;
                    if r == 0 {
                        return Err(());
                    }
                    acc /= r;
                }
                _ => break,
            }
        }
        Ok(acc)
    }

    fn unary(&mut self) -> Result<i128, ()> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(-self.unary()?)
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<i128, ()> {
        match self.peek() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Tok::LP) => {
                self.pos += 1;
                let v = self.expr()?;
                if let Some(Tok::RP) = self.peek() {
                    self.pos += 1;
                }
                Ok(v)
            }
            _ => Err(()),
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let mut p = Parser { t: tokenize(&line), pos: 0 };
        match p.expr() {
            Ok(v) => writeln!(out, "{}", v).unwrap(),
            Err(_) => writeln!(out, "error").unwrap(),
        }
    }
}
