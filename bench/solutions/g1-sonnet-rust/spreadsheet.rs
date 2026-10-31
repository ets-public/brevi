use std::collections::HashMap;
use std::io::{stdin, Read};

#[derive(Clone, Copy, PartialEq, Debug)]
enum E { Cycle, Div0 }
type V = Result<i64, E>;

enum X {
    Num(i64),
    Ref(usize, usize), // col,row
    Neg(Box<X>),
    Bin(char, Box<X>, Box<X>),
    Func(String, (usize, usize), (usize, usize)),
}

struct Ps { t: Vec<char>, i: usize, maxc: usize, maxr: usize }
impl Ps {
    fn peek(&self) -> Option<char> { self.t.get(self.i).copied() }
    fn cell(&mut self) -> Option<(usize, usize)> {
        let c = self.peek()?;
        if !c.is_ascii_alphabetic() { return None; }
        let col = (c.to_ascii_uppercase() as u8 - b'A') as usize;
        self.i += 1;
        let st = self.i;
        while self.peek().map_or(false, |c| c.is_ascii_digit()) { self.i += 1; }
        if st == self.i { return None; }
        let row: usize = self.t[st..self.i].iter().collect::<String>().parse().ok()?;
        self.maxc = self.maxc.max(col);
        self.maxr = self.maxr.max(row);
        Some((col, row))
    }
    fn expr(&mut self) -> Option<X> {
        let mut l = self.term()?;
        while let Some(c) = self.peek() {
            if c == '+' || c == '-' { self.i += 1; let r = self.term()?; l = X::Bin(c, Box::new(l), Box::new(r)); } else { break; }
        }
        Some(l)
    }
    fn term(&mut self) -> Option<X> {
        let mut l = self.unary()?;
        while let Some(c) = self.peek() {
            if c == '*' || c == '/' { self.i += 1; let r = self.unary()?; l = X::Bin(c, Box::new(l), Box::new(r)); } else { break; }
        }
        Some(l)
    }
    fn unary(&mut self) -> Option<X> {
        match self.peek()? {
            '-' => { self.i += 1; Some(X::Neg(Box::new(self.unary()?))) }
            '+' => { self.i += 1; self.unary() }
            '(' => { self.i += 1; let e = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(e) }
            c if c.is_ascii_digit() => {
                let st = self.i;
                while self.peek().map_or(false, |c| c.is_ascii_digit()) { self.i += 1; }
                Some(X::Num(self.t[st..self.i].iter().collect::<String>().parse().ok()?))
            }
            c if c.is_ascii_alphabetic() => {
                // function or cell
                let st = self.i;
                let mut j = st;
                while j < self.t.len() && self.t[j].is_ascii_alphabetic() { j += 1; }
                if j - st > 1 && self.t.get(j) == Some(&'(') {
                    let name: String = self.t[st..j].iter().collect::<String>().to_uppercase();
                    self.i = j + 1;
                    let a = self.cell()?;
                    if self.peek()? != ':' { return None; }
                    self.i += 1;
                    let b = self.cell()?;
                    if self.peek()? != ')' { return None; }
                    self.i += 1;
                    Some(X::Func(name, a, b))
                } else {
                    let (c, r) = self.cell()?;
                    Some(X::Ref(c, r))
                }
            }
            _ => None,
        }
    }
}

enum Cell { Num(i64), F(X) }

struct S { cells: HashMap<(usize, usize), Cell>, memo: HashMap<(usize, usize), V>, vis: HashMap<(usize, usize), bool> }

fn comb(a: V, b: V) -> Result<(i64, i64), E> {
    match (a, b) {
        (Ok(x), Ok(y)) => Ok((x, y)),
        (Err(E::Cycle), _) | (_, Err(E::Cycle)) => Err(E::Cycle),
        (Err(e), _) => Err(e),
        (_, Err(e)) => Err(e),
    }
}

impl S {
    fn cell(&mut self, k: (usize, usize)) -> V {
        if let Some(v) = self.memo.get(&k) { return *v; }
        if self.vis.get(&k).copied().unwrap_or(false) { return Err(E::Cycle); }
        let v = match self.cells.get(&k) {
            None => Ok(0),
            Some(Cell::Num(n)) => Ok(*n),
            Some(Cell::F(_)) => {
                self.vis.insert(k, true);
                let x = match self.cells.remove(&k) { Some(Cell::F(x)) => x, _ => unreachable!() };
                let r = self.ev(&x);
                self.cells.insert(k, Cell::F(x));
                self.vis.insert(k, false);
                r
            }
        };
        self.memo.insert(k, v);
        v
    }
    fn ev(&mut self, x: &X) -> V {
        match x {
            X::Num(n) => Ok(*n),
            X::Ref(c, r) => self.cell((*c, *r)),
            X::Neg(a) => self.ev(a).map(|v| v.wrapping_neg()),
            X::Bin(op, a, b) => {
                let l = self.ev(a);
                let r = self.ev(b);
                let (l, r) = comb(l, r)?;
                match op {
                    '+' => Ok(l.wrapping_add(r)),
                    '-' => Ok(l.wrapping_sub(r)),
                    '*' => Ok(l.wrapping_mul(r)),
                    _ => if r == 0 { Err(E::Div0) } else { Ok(l.wrapping_div(r)) },
                }
            }
            X::Func(name, a, b) => {
                let (c0, c1) = (a.0.min(b.0), a.0.max(b.0));
                let (r0, r1) = (a.1.min(b.1), a.1.max(b.1));
                let mut err: Option<E> = None;
                let mut vals = vec![];
                for c in c0..=c1 { for r in r0..=r1 {
                    match self.cell((c, r)) {
                        Ok(v) => vals.push(v),
                        Err(E::Cycle) => err = Some(E::Cycle),
                        Err(e) => if err != Some(E::Cycle) { err = Some(e) },
                    }
                } }
                if let Some(e) = err { return Err(e); }
                Ok(match name.as_str() {
                    "SUM" => vals.iter().fold(0i64, |a, b| a.wrapping_add(*b)),
                    "MIN" => *vals.iter().min().unwrap(),
                    _ => *vals.iter().max().unwrap(),
                })
            }
        }
    }
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let mut st = S { cells: HashMap::new(), memo: HashMap::new(), vis: HashMap::new() };
    let (mut mc, mut mr) = (0usize, 0usize);
    let mut any = false;
    for line in s.lines() {
        if line.trim().is_empty() { continue; }
        let (lhs, rhs) = match line.split_once('=') { Some(p) => p, None => continue };
        let mut p = Ps { t: lhs.trim().chars().collect(), i: 0, maxc: 0, maxr: 0 };
        let k = match p.cell() { Some(k) => k, None => continue };
        any = true;
        mc = mc.max(p.maxc); mr = mr.max(p.maxr);
        let rhs = rhs.trim_start_matches(|c: char| c == ' ');
        let rhs = rhs.trim();
        if let Some(f) = rhs.strip_prefix('=') {
            let mut q = Ps { t: f.chars().filter(|c| !c.is_whitespace()).collect(), i: 0, maxc: 0, maxr: 0 };
            match q.expr() {
                Some(x) if q.i == q.t.len() => {
                    mc = mc.max(q.maxc); mr = mr.max(q.maxr);
                    st.cells.insert(k, Cell::F(x));
                }
                _ => { st.cells.insert(k, Cell::Num(0)); }
            }
        } else {
            let n: i64 = rhs.replace(' ', "").parse().unwrap_or(0);
            st.cells.insert(k, Cell::Num(n));
        }
    }
    if !any { return; }
    let mut out = String::new();
    for c in 0..=mc { out.push('\t'); out.push((b'A' + c as u8) as char); }
    out.push('\n');
    for r in 1..=mr {
        out += &r.to_string();
        for c in 0..=mc {
            out.push('\t');
            if st.cells.contains_key(&(c, r)) {
                match st.cell((c, r)) {
                    Ok(v) => out += &v.to_string(),
                    Err(E::Cycle) => out += "#CYCLE",
                    Err(E::Div0) => out += "#DIV0",
                }
            }
        }
        out.push('\n');
    }
    print!("{}", out);
}
