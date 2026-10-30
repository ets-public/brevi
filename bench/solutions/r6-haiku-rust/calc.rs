use std::io::{self, BufRead, Write};

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\r' {
                self.i += 1;
            } else {
                break;
            }
        }
    }
    fn expr(&mut self) -> Result<i128, ()> {
        let mut v = self.term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    let r = self.term()?;
                    v = v.wrapping_add(r);
                }
                Some(b'-') => {
                    self.i += 1;
                    let r = self.term()?;
                    v = v.wrapping_sub(r);
                }
                _ => break,
            }
        }
        Ok(v)
    }
    fn term(&mut self) -> Result<i128, ()> {
        let mut v = self.unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    let r = self.unary()?;
                    v = v.wrapping_mul(r);
                }
                Some(b'/') => {
                    self.i += 1;
                    let r = self.unary()?;
                    if r == 0 {
                        return Err(());
                    }
                    v = v.wrapping_div(r);
                }
                _ => break,
            }
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<i128, ()> {
        self.skip_ws();
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(self.unary()?.wrapping_neg())
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.primary(),
        }
    }
    fn primary(&mut self) -> Result<i128, ()> {
        self.skip_ws();
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let v = self.expr()?;
                self.skip_ws();
                if self.peek() == Some(b')') {
                    self.i += 1;
                }
                Ok(v)
            }
            Some(c) if c.is_ascii_digit() => {
                let mut v: i128 = 0;
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        v = v.wrapping_mul(10).wrapping_add((d - b'0') as i128);
                        self.i += 1;
                    } else {
                        break;
                    }
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
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let mut p = P { s: line.as_bytes(), i: 0 };
        match p.expr() {
            Ok(v) => writeln!(out, "{}", v).unwrap(),
            Err(_) => writeln!(out, "error").unwrap(),
        }
    }
}
