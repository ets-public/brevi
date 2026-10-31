use std::collections::HashMap;
use std::io::{self, Read, Write};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Fail {
    Cycle,
    Div0,
}

type Res = Result<i64, Fail>;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Func {
    Sum,
    Min,
    Max,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tok {
    Num(i64),
    Ref(usize, usize),
    Range(usize, usize, usize, usize),
    Func(Func),
    LP,
    RP,
    Plus,
    Minus,
    Star,
    Slash,
    Bad,
}

enum Content {
    Int(i64),
    Formula(String),
    Text(String),
}

#[derive(Clone, Copy)]
enum St {
    Busy,
    Done(Res),
}

// Reads a cell reference (column letter + row digits) starting at i.
// Returns the zero-based column, row, and the index after the reference.
fn read_ref(b: &[u8], i: usize) -> Option<((usize, usize), usize)> {
    if i >= b.len() || !b[i].is_ascii_uppercase() {
        return None;
    }
    let mut j = i + 1;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    if j == i + 1 {
        return None;
    }
    let row: usize = std::str::from_utf8(&b[i + 1..j]).ok()?.parse().ok()?;
    if (1..=99).contains(&row) {
        Some((((b[i] - b'A') as usize, row), j))
    } else {
        None
    }
}

fn lex(s: &str) -> Vec<Tok> {
    let b: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let st = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = std::str::from_utf8(&b[st..i])
                .ok()
                .and_then(|t| t.parse::<i64>().ok())
                .unwrap_or(0);
            out.push(Tok::Num(n));
        } else if c.is_ascii_uppercase() {
            if let Some(((col, row), j)) = read_ref(&b, i) {
                i = j;
                // Range A1:B3
                if i + 1 < b.len() && b[i] == b':' {
                    if let Some(((c2, r2), j2)) = read_ref(&b, i + 1) {
                        out.push(Tok::Range(col, row, c2, r2));
                        i = j2;
                        continue;
                    }
                }
                out.push(Tok::Ref(col, row));
            } else {
                // Function name letters
                let st = i;
                while i < b.len() && b[i].is_ascii_uppercase() {
                    i += 1;
                }
                let name = &b[st..i];
                let f = match name {
                    b"SUM" => Some(Func::Sum),
                    b"MIN" => Some(Func::Min),
                    b"MAX" => Some(Func::Max),
                    _ => None,
                };
                match f {
                    Some(f) if i < b.len() && b[i] == b'(' => out.push(Tok::Func(f)),
                    _ => out.push(Tok::Bad),
                }
            }
        } else {
            let t = match c {
                b'(' => Tok::LP,
                b')' => Tok::RP,
                b'+' => Tok::Plus,
                b'-' => Tok::Minus,
                b'*' => Tok::Star,
                b'/' => Tok::Slash,
                _ => Tok::Bad,
            };
            out.push(t);
            i += 1;
        }
    }
    out
}

fn bin(a: Res, b: Res, f: impl Fn(i64, i64) -> Res) -> Res {
    match (a, b) {
        (Ok(x), Ok(y)) => f(x, y),
        (Err(Fail::Cycle), _) | (_, Err(Fail::Cycle)) => Err(Fail::Cycle),
        _ => Err(Fail::Div0),
    }
}

struct Sheet {
    cells: HashMap<(usize, usize), Content>,
    states: HashMap<(usize, usize), St>,
}

impl Sheet {
    fn eval(&mut self, c: usize, r: usize) -> Res {
        let key = (c, r);
        if let Some(st) = self.states.get(&key) {
            return match *st {
                St::Busy => Err(Fail::Cycle),
                St::Done(res) => res,
            };
        }
        let formula = match self.cells.get(&key) {
            None => return Ok(0),
            Some(Content::Int(n)) => return Ok(*n),
            Some(Content::Text(_)) => return Ok(0),
            Some(Content::Formula(f)) => f.clone(),
        };
        self.states.insert(key, St::Busy);
        let toks = lex(&formula);
        let mut pos = 0;
        let res = self.expr(&toks, &mut pos);
        self.states.insert(key, St::Done(res));
        res
    }

    fn expr(&mut self, t: &[Tok], p: &mut usize) -> Res {
        let mut acc = self.term(t, p);
        while *p < t.len() {
            match t[*p] {
                Tok::Plus => {
                    *p += 1;
                    let rhs = self.term(t, p);
                    acc = bin(acc, rhs, |x, y| Ok(x.wrapping_add(y)));
                }
                Tok::Minus => {
                    *p += 1;
                    let rhs = self.term(t, p);
                    acc = bin(acc, rhs, |x, y| Ok(x.wrapping_sub(y)));
                }
                _ => break,
            }
        }
        acc
    }

    fn term(&mut self, t: &[Tok], p: &mut usize) -> Res {
        let mut acc = self.unary(t, p);
        while *p < t.len() {
            match t[*p] {
                Tok::Star => {
                    *p += 1;
                    let rhs = self.unary(t, p);
                    acc = bin(acc, rhs, |x, y| Ok(x.wrapping_mul(y)));
                }
                Tok::Slash => {
                    *p += 1;
                    let rhs = self.unary(t, p);
                    acc = bin(acc, rhs, |x, y| {
                        if y == 0 {
                            Err(Fail::Div0)
                        } else {
                            Ok(x.wrapping_div(y))
                        }
                    });
                }
                _ => break,
            }
        }
        acc
    }

    fn unary(&mut self, t: &[Tok], p: &mut usize) -> Res {
        if *p < t.len() && t[*p] == Tok::Minus {
            *p += 1;
            return match self.unary(t, p) {
                Ok(x) => Ok(x.wrapping_neg()),
                e => e,
            };
        }
        self.primary(t, p)
    }

    fn primary(&mut self, t: &[Tok], p: &mut usize) -> Res {
        if *p >= t.len() {
            return Ok(0);
        }
        match t[*p] {
            Tok::Num(n) => {
                *p += 1;
                Ok(n)
            }
            Tok::Ref(c, r) => {
                *p += 1;
                self.eval(c, r)
            }
            Tok::LP => {
                *p += 1;
                let v = self.expr(t, p);
                if *p < t.len() && t[*p] == Tok::RP {
                    *p += 1;
                }
                v
            }
            Tok::Func(f) => {
                *p += 1;
                if *p < t.len() && t[*p] == Tok::LP {
                    *p += 1;
                }
                let mut range = None;
                match t.get(*p) {
                    Some(Tok::Range(a, b, c, d)) => {
                        range = Some((*a, *b, *c, *d));
                        *p += 1;
                    }
                    Some(Tok::Ref(a, b)) => {
                        range = Some((*a, *b, *a, *b));
                        *p += 1;
                    }
                    _ => {}
                }
                while *p < t.len() && t[*p] != Tok::RP {
                    *p += 1;
                }
                if *p < t.len() {
                    *p += 1;
                }
                match range {
                    None => Ok(0),
                    Some((c1, r1, c2, r2)) => self.func(f, c1, r1, c2, r2),
                }
            }
            _ => Ok(0),
        }
    }

    fn func(&mut self, f: Func, c1: usize, r1: usize, c2: usize, r2: usize) -> Res {
        let (ca, cb) = (c1.min(c2), c1.max(c2));
        let (ra, rb) = (r1.min(r2), r1.max(r2));
        let mut acc: Res = match f {
            Func::Sum => Ok(0),
            Func::Min => Ok(i64::MAX),
            Func::Max => Ok(i64::MIN),
        };
        for c in ca..=cb {
            for r in ra..=rb {
                let v = self.eval(c, r);
                acc = match f {
                    Func::Sum => bin(acc, v, |x, y| Ok(x.wrapping_add(y))),
                    Func::Min => bin(acc, v, |x, y| Ok(x.min(y))),
                    Func::Max => bin(acc, v, |x, y| Ok(x.max(y))),
                };
            }
        }
        acc
    }

    fn show(&mut self, c: usize, r: usize) -> String {
        match self.cells.get(&(c, r)) {
            None => String::new(),
            Some(Content::Text(t)) => t.clone(),
            Some(Content::Int(n)) => n.to_string(),
            Some(Content::Formula(_)) => match self.eval(c, r) {
                Ok(v) => v.to_string(),
                Err(Fail::Cycle) => "#CYCLE".to_string(),
                Err(Fail::Div0) => "#DIV0".to_string(),
            },
        }
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut cells: HashMap<(usize, usize), Content> = HashMap::new();
    let mut max_col = 0usize;
    let mut max_row = 1usize;
    let mut mentions: Vec<(usize, usize)> = Vec::new();

    for line in input.lines() {
        let (left, right) = match line.split_once('=') {
            Some(p) => p,
            None => continue,
        };
        let left = left.trim();
        let lb = left.as_bytes();
        let key = match read_ref(lb, 0) {
            Some((k, end)) if end == lb.len() => k,
            _ => continue,
        };
        max_col = max_col.max(key.0);
        max_row = max_row.max(key.1);
        let right = right.trim();
        let content = if let Some(f) = line.split_once('=').map(|(_, r)| r.trim()) {
            let _ = f;
            if let Some(formula) = right.strip_prefix('=') {
                for tok in lex(formula) {
                    match tok {
                        Tok::Ref(c, r) => mentions.push((c, r)),
                        Tok::Range(a, b, c, d) => {
                            mentions.push((a, b));
                            mentions.push((c, d));
                        }
                        _ => {}
                    }
                }
                Content::Formula(formula.to_string())
            } else if let Ok(n) = right.parse::<i64>() {
                Content::Int(n)
            } else {
                Content::Text(right.to_string())
            }
        } else {
            Content::Text(String::new())
        };
        cells.insert(key, content);
    }

    for (c, r) in mentions {
        max_col = max_col.max(c);
        max_row = max_row.max(r);
    }

    let mut sheet = Sheet {
        cells,
        states: HashMap::new(),
    };

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let mut header = String::from("\t");
    let cols: Vec<String> = (0..=max_col)
        .map(|c| ((b'A' + c as u8) as char).to_string())
        .collect();
    header.push_str(&cols.join("\t"));
    writeln!(out, "{}", header).unwrap();

    for r in 1..=max_row {
        let mut line = r.to_string();
        for c in 0..=max_col {
            line.push('\t');
            line.push_str(&sheet.show(c, r));
        }
        writeln!(out, "{}", line).unwrap();
    }
    out.flush().unwrap();
}
