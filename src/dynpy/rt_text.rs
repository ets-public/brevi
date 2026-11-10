// ---------- re: a backtracking engine with Python's syntax and semantics ----------

#[derive(Clone, Debug)]
pub enum RNode {
    Char(char),
    Any,
    Class(Rc<RClass>),
    Bol,
    Eol,
    StrStart,
    StrEnd,
    WordB(bool),
    Group(Rc<Vec<Vec<RNode>>>, Option<usize>),
    Look(Rc<Vec<Vec<RNode>>>, bool, bool), // behind?, negative?
    Backref(usize),
    Repeat(Box<RNode>, usize, usize, bool),
    Atomic(Rc<Vec<Vec<RNode>>>),
    // (?(1)yes|no)
    Cond(usize, Rc<Vec<RNode>>, Rc<Vec<RNode>>),
}
#[derive(Clone, Debug)]
pub struct RClass {
    items: Vec<CItem>,
    neg: bool,
}
#[derive(Clone, Debug)]
enum CItem {
    Ch(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}
pub struct Prog {
    alts: Vec<Vec<RNode>>,
    ngroups: usize,
    names: Vec<(String, usize)>,
    flags: i64,
    src: String,
}
const F_I: i64 = 2;
const F_M: i64 = 8;
const F_S: i64 = 16;
const F_X: i64 = 64;
const F_A: i64 = 256;

fn re_err<T>(msg: String) -> Result<T, E> {
    Err(exc("error", msg))
}

struct RP<'a> {
    c: &'a [char],
    i: usize,
    flags: i64,
    ngroups: usize,
    names: Vec<(String, usize)>,
}
impl<'a> RP<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn skip_x(&mut self) {
        if self.flags & F_X == 0 {
            return;
        }
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => self.i += 1,
                Some('#') => {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }
    fn alts(&mut self) -> Result<Vec<Vec<RNode>>, E> {
        let mut alts = vec![self.seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.seq()?);
        }
        Ok(alts)
    }
    fn seq(&mut self) -> Result<Vec<RNode>, E> {
        let mut v = vec![];
        loop {
            self.skip_x();
            match self.peek() {
                None | Some('|') | Some(')') => return Ok(v),
                _ => {}
            }
            let start = self.i;
            let atom = self.atom()?;
            let Some(atom) = atom else { continue };
            self.skip_x();
            let q = self.quant()?;
            match q {
                Some((lo, hi, greedy, poss)) => {
                    if matches!(atom, RNode::Bol | RNode::Eol | RNode::StrStart | RNode::StrEnd | RNode::WordB(_) | RNode::Look(..)) && false {
                        let _ = start;
                    }
                    if let RNode::Repeat(..) = atom {
                        return re_err(format!("multiple repeat at position {}", self.i - 1));
                    }
                    let r = RNode::Repeat(Box::new(atom), lo, hi, greedy);
                    v.push(if poss { RNode::Atomic(Rc::new(vec![vec![r]])) } else { r });
                }
                None => v.push(atom),
            }
        }
    }
    fn num(&mut self) -> Option<usize> {
        let s = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if s == self.i {
            None
        } else {
            self.c[s..self.i].iter().collect::<String>().parse().ok()
        }
    }
    fn quant(&mut self) -> Result<Option<(usize, usize, bool, bool)>, E> {
        let (lo, hi) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, usize::MAX)
            }
            Some('+') => {
                self.i += 1;
                (1, usize::MAX)
            }
            Some('?') => {
                self.i += 1;
                (0, 1)
            }
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let lo = self.num();
                let r = if self.peek() == Some(',') {
                    self.i += 1;
                    let hi = self.num();
                    (lo.unwrap_or(0), hi.unwrap_or(usize::MAX))
                } else {
                    match lo {
                        Some(n) => (n, n),
                        None => {
                            self.i = save;
                            return Ok(None);
                        }
                    }
                };
                if self.peek() != Some('}') {
                    self.i = save;
                    return Ok(None);
                }
                self.i += 1;
                if r.1 < r.0 {
                    return re_err(format!("min repeat greater than max repeat at position {}", save + 1));
                }
                r
            }
            _ => return Ok(None),
        };
        let mut greedy = true;
        let mut poss = false;
        if self.peek() == Some('?') {
            self.i += 1;
            greedy = false;
        } else if self.peek() == Some('+') {
            self.i += 1;
            poss = true;
        }
        Ok(Some((lo, hi, greedy, poss)))
    }
    fn escape_char(&mut self, c: char) -> Result<Option<char>, E> {
        Ok(Some(match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'f' => '\x0c',
            'v' => '\x0b',
            'a' => '\x07',
            '0' => {
                let mut v = 0u32;
                let mut k = 0;
                while k < 2 && self.peek().is_some_and(|c| ('0'..='7').contains(&c)) {
                    v = v * 8 + self.peek().unwrap().to_digit(8).unwrap();
                    self.i += 1;
                    k += 1;
                }
                char::from_u32(v).unwrap_or('\0')
            }
            'x' | 'u' | 'U' => {
                let n = match c {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let h: String = self.c.get(self.i..self.i + n).map(|x| x.iter().collect()).unwrap_or_default();
                let v = u32::from_str_radix(&h, 16).ok().filter(|_| h.len() == n);
                match v.and_then(char::from_u32) {
                    Some(ch) => {
                        self.i += n;
                        ch
                    }
                    None => return re_err(format!("incomplete escape \\{c}{h}")),
                }
            }
            c if c.is_ascii_alphanumeric() => return Ok(None),
            c => c,
        }))
    }
    fn atom(&mut self) -> Result<Option<RNode>, E> {
        let c = self.peek().unwrap();
        self.i += 1;
        let ic = self.flags & F_I != 0;
        let ch = |c: char| if ic { RNode::Class(Rc::new(RClass { items: fold_items(c), neg: false })) } else { RNode::Char(c) };
        Ok(Some(match c {
            '.' => RNode::Any,
            '^' => RNode::Bol,
            '$' => RNode::Eol,
            '*' | '+' | '?' => return re_err(format!("nothing to repeat at position {}", self.i - 1)),
            '{' => {
                // a literal { unless it is a valid quantifier (which has nothing to repeat)
                ch('{')
            }
            '[' => self.class()?,
            '(' => return self.group(),
            '\\' => {
                let Some(e) = self.peek() else { return re_err("bad escape (end of pattern)".into()) };
                self.i += 1;
                match e {
                    'd' | 'D' => RNode::Class(Rc::new(RClass { items: vec![CItem::Digit(e == 'D')], neg: false })),
                    'w' | 'W' => RNode::Class(Rc::new(RClass { items: vec![CItem::Word(e == 'W')], neg: false })),
                    's' | 'S' => RNode::Class(Rc::new(RClass { items: vec![CItem::Space(e == 'S')], neg: false })),
                    'b' => RNode::WordB(true),
                    'B' => RNode::WordB(false),
                    'A' => RNode::StrStart,
                    'Z' | 'z' => RNode::StrEnd,
                    '1'..='9' => {
                        self.i -= 1;
                        let n = self.num().unwrap();
                        if n > self.ngroups {
                            return re_err(format!("invalid group reference {n} at position {}", self.i));
                        }
                        RNode::Backref(n)
                    }
                    _ => match self.escape_char(e)? {
                        Some(c) => ch(c),
                        None => return re_err(format!("bad escape \\{e} at position {}", self.i - 2)),
                    },
                }
            }
            c => ch(c),
        }))
    }
    fn group(&mut self) -> Result<Option<RNode>, E> {
        if self.peek() == Some('?') {
            self.i += 1;
            let k = self.peek();
            match k {
                Some(':') => {
                    self.i += 1;
                    let a = self.alts()?;
                    self.close()?;
                    return Ok(Some(RNode::Group(Rc::new(a), None)));
                }
                Some('>') => {
                    self.i += 1;
                    let a = self.alts()?;
                    self.close()?;
                    return Ok(Some(RNode::Atomic(Rc::new(a))));
                }
                Some('=') | Some('!') => {
                    self.i += 1;
                    let a = self.alts()?;
                    self.close()?;
                    return Ok(Some(RNode::Look(Rc::new(a), false, k == Some('!'))));
                }
                Some('<') if matches!(self.c.get(self.i + 1), Some('=') | Some('!')) => {
                    let neg = self.c[self.i + 1] == '!';
                    self.i += 2;
                    let a = self.alts()?;
                    self.close()?;
                    return Ok(Some(RNode::Look(Rc::new(a), true, neg)));
                }
                Some('P') | Some('<') => {
                    if k == Some('P') {
                        self.i += 1;
                    }
                    match self.peek() {
                        Some('<') => {
                            self.i += 1;
                            let s = self.i;
                            while self.peek().is_some_and(|c| c != '>') {
                                self.i += 1;
                            }
                            let name: String = self.c[s..self.i].iter().collect();
                            self.i += 1;
                            self.ngroups += 1;
                            let g = self.ngroups;
                            self.names.push((name, g));
                            let a = self.alts()?;
                            self.close()?;
                            return Ok(Some(RNode::Group(Rc::new(a), Some(g))));
                        }
                        Some('=') => {
                            self.i += 1;
                            let s = self.i;
                            while self.peek().is_some_and(|c| c != ')') {
                                self.i += 1;
                            }
                            let name: String = self.c[s..self.i].iter().collect();
                            self.i += 1;
                            return match self.names.iter().find(|x| x.0 == name) {
                                Some(x) => Ok(Some(RNode::Backref(x.1))),
                                None => re_err(format!("unknown group name '{name}'")),
                            };
                        }
                        _ => return re_err("unknown extension ?P".into()),
                    }
                }
                Some('#') => {
                    while self.peek().is_some_and(|c| c != ')') {
                        self.i += 1;
                    }
                    self.i += 1;
                    return Ok(None);
                }
                Some('(') => {
                    self.i += 1;
                    let n = match self.num() {
                        Some(n) => n,
                        None => {
                            let s = self.i;
                            while self.peek().is_some_and(|c| c != ')') {
                                self.i += 1;
                            }
                            let name: String = self.c[s..self.i].iter().collect();
                            self.names.iter().find(|x| x.0 == name).map(|x| x.1).ok_or_else(|| exc("error", format!("unknown group name '{name}'")))?
                        }
                    };
                    self.close()?;
                    let yes = self.seq()?;
                    let no = if self.peek() == Some('|') {
                        self.i += 1;
                        self.seq()?
                    } else {
                        vec![]
                    };
                    self.close()?;
                    return Ok(Some(RNode::Cond(n, Rc::new(yes), Rc::new(no))));
                }
                _ => {
                    // inline flags: (?aiLmsux) or (?flags:...)
                    let mut on = 0;
                    let mut off = 0;
                    let mut neg = false;
                    loop {
                        match self.peek() {
                            Some('i') => *(if neg { &mut off } else { &mut on }) |= F_I,
                            Some('m') => *(if neg { &mut off } else { &mut on }) |= F_M,
                            Some('s') => *(if neg { &mut off } else { &mut on }) |= F_S,
                            Some('x') => *(if neg { &mut off } else { &mut on }) |= F_X,
                            Some('a') => on |= F_A,
                            Some('u') | Some('L') => {}
                            Some('-') => neg = true,
                            Some(')') => {
                                self.i += 1;
                                self.flags |= on;
                                return Ok(None);
                            }
                            Some(':') => {
                                self.i += 1;
                                let saved = self.flags;
                                self.flags = (self.flags | on) & !off;
                                let a = self.alts()?;
                                self.flags = saved;
                                self.close()?;
                                return Ok(Some(RNode::Group(Rc::new(a), None)));
                            }
                            _ => return re_err(format!("unknown extension ?{} at position {}", self.peek().unwrap_or(' '), self.i - 1)),
                        }
                        self.i += 1;
                    }
                }
            }
        }
        self.ngroups += 1;
        let g = self.ngroups;
        let a = self.alts()?;
        self.close()?;
        Ok(Some(RNode::Group(Rc::new(a), Some(g))))
    }
    fn close(&mut self) -> Result<(), E> {
        if self.peek() == Some(')') {
            self.i += 1;
            Ok(())
        } else {
            re_err(format!("missing ), unterminated subpattern at position {}", self.i))
        }
    }
    fn class(&mut self) -> Result<RNode, E> {
        let mut items = vec![];
        let mut neg = false;
        if self.peek() == Some('^') {
            neg = true;
            self.i += 1;
        }
        let mut first = true;
        loop {
            let Some(c) = self.peek() else { return re_err("unterminated character set".into()) };
            self.i += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = if c == '\\' {
                let e = self.peek().ok_or_else(|| exc("error", "bad escape (end of pattern)".into()))?;
                self.i += 1;
                match e {
                    'd' | 'D' => {
                        items.push(CItem::Digit(e == 'D'));
                        continue;
                    }
                    'w' | 'W' => {
                        items.push(CItem::Word(e == 'W'));
                        continue;
                    }
                    's' | 'S' => {
                        items.push(CItem::Space(e == 'S'));
                        continue;
                    }
                    'b' => '\x08',
                    _ => match self.escape_char(e)? {
                        Some(c) => c,
                        None => return re_err(format!("bad escape \\{e}")),
                    },
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.c.get(self.i + 1).is_some_and(|&x| x != ']') {
                self.i += 1;
                let mut hi = self.peek().unwrap();
                self.i += 1;
                if hi == '\\' {
                    let e = self.peek().unwrap_or('\\');
                    self.i += 1;
                    hi = self.escape_char(e)?.unwrap_or(e);
                }
                if hi < lo {
                    return re_err(format!("bad character range {lo}-{hi}"));
                }
                items.push(CItem::Range(lo, hi));
            } else {
                items.push(CItem::Ch(lo));
            }
        }
        if self.flags & F_I != 0 {
            let mut more = vec![];
            for it in &items {
                match it {
                    CItem::Ch(c) => more.extend(fold_items(*c)),
                    CItem::Range(a, b) => {
                        if (*b as u32) - (*a as u32) < 512 {
                            for x in *a..=*b {
                                more.extend(fold_items(x));
                            }
                        }
                    }
                    _ => {}
                }
            }
            items.extend(more);
        }
        Ok(RNode::Class(Rc::new(RClass { items, neg })))
    }
}
fn fold_items(c: char) -> Vec<CItem> {
    let mut v = vec![CItem::Ch(c)];
    for l in c.to_lowercase() {
        v.push(CItem::Ch(l));
    }
    for u in c.to_uppercase() {
        v.push(CItem::Ch(u));
    }
    v
}
fn is_word(c: char, ascii: bool) -> bool {
    if ascii {
        c.is_ascii_alphanumeric() || c == '_'
    } else {
        // Unicode categories L and N, as str.isalnum() and CPython's \w take them (not combining marks)
        c.is_ascii_alphanumeric() || c == '_' || (!c.is_ascii() && tab_has(PY_LN, c))
    }
}
fn tab_has(t: &[(u32, u32)], c: char) -> bool {
    let u = c as u32;
    t.binary_search_by(|&(a, b)| if b < u { std::cmp::Ordering::Less } else if a > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}
fn is_digit(c: char, ascii: bool) -> bool {
    // \d is Unicode category Nd (decimal digits), not every numeric character: Ⅵ and ½ are not digits
    c.is_ascii_digit() || (!ascii && !c.is_ascii() && is_nd(c))
}
fn is_space(c: char, ascii: bool) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') || (!ascii && !c.is_ascii() && c.is_whitespace()) || (!ascii && matches!(c, '\x1c'..='\x1f'))
}
impl RClass {
    fn matches(&self, c: char, ascii: bool) -> bool {
        let mut hit = false;
        for it in &self.items {
            if match it {
                CItem::Ch(x) => *x == c,
                CItem::Range(a, b) => *a <= c && c <= *b,
                CItem::Digit(n) => is_digit(c, ascii) != *n,
                CItem::Word(n) => is_word(c, ascii) != *n,
                CItem::Space(n) => is_space(c, ascii) != *n,
            } {
                hit = true;
                break;
            }
        }
        hit != self.neg
    }
}

pub fn re_compile(src: &str, flags: i64) -> Result<Rc<Prog>, E> {
    thread_local! {
        static CACHE: RefCell<HashMap<(String, i64), Rc<Prog>>> = RefCell::new(HashMap::new());
    }
    if let Some(p) = CACHE.with(|c| c.borrow().get(&(src.to_string(), flags)).cloned()) {
        return Ok(p);
    }
    let chars: Vec<char> = src.chars().collect();
    // global inline flags anywhere at the top level are applied from the start, as Python does at the start
    let mut fl = flags;
    {
        let mut p = RP { c: &chars, i: 0, flags: fl, ngroups: 0, names: vec![] };
        if chars.starts_with(&['(', '?']) {
            p.i = 2;
            let mut f = 0;
            while let Some(c) = p.peek() {
                match c {
                    'i' => f |= F_I,
                    'm' => f |= F_M,
                    's' => f |= F_S,
                    'x' => f |= F_X,
                    'a' => f |= F_A,
                    'u' | 'L' => {}
                    ')' => {
                        fl |= f;
                        break;
                    }
                    _ => break,
                }
                p.i += 1;
            }
        }
    }
    let mut p = RP { c: &chars, i: 0, flags: fl, ngroups: 0, names: vec![] };
    let alts = p.alts()?;
    if p.i < chars.len() {
        return re_err(format!("unbalanced parenthesis at position {}", p.i));
    }
    let prog = Rc::new(Prog { alts, ngroups: p.ngroups, names: p.names, flags: p.flags | fl, src: src.to_string() });
    CACHE.with(|c| c.borrow_mut().insert((src.to_string(), flags), prog.clone()));
    Ok(prog)
}

struct Ctx<'a> {
    s: &'a [char],
    caps: Vec<usize>,
    flags: i64,
    steps: u64,
}
const UNSET: usize = usize::MAX;
type Kont<'k> = &'k mut dyn FnMut(&mut Ctx, usize) -> bool;

impl<'a> Ctx<'a> {
    fn single(&self, n: &RNode, pos: usize) -> Option<bool> {
        let ascii = self.flags & F_A != 0;
        Some(match n {
            RNode::Char(c) => self.s.get(pos) == Some(c),
            RNode::Any => match self.s.get(pos) {
                Some('\n') => self.flags & F_S != 0,
                Some(_) => true,
                None => false,
            },
            RNode::Class(k) => match self.s.get(pos) {
                Some(&c) => k.matches(c, ascii),
                None => false,
            },
            _ => return None,
        })
    }
    fn alts(&mut self, alts: &[Vec<RNode>], pos: usize, k: Kont) -> bool {
        for a in alts {
            if self.seq(a, pos, k) {
                return true;
            }
        }
        false
    }
    fn seq(&mut self, seq: &[RNode], pos: usize, k: Kont) -> bool {
        self.steps += 1;
        let Some(n) = seq.first() else { return k(self, pos) };
        let rest = &seq[1..];
        let ascii = self.flags & F_A != 0;
        match n {
            RNode::Char(_) | RNode::Any | RNode::Class(_) => {
                if self.single(n, pos).unwrap() {
                    self.seq(rest, pos + 1, k)
                } else {
                    false
                }
            }
            RNode::Bol => {
                if pos == 0 || (self.flags & F_M != 0 && self.s[pos - 1] == '\n') {
                    self.seq(rest, pos, k)
                } else {
                    false
                }
            }
            RNode::Eol => {
                let n = self.s.len();
                if pos == n || (pos == n - 1 && self.s[pos] == '\n') || (self.flags & F_M != 0 && self.s[pos] == '\n') {
                    self.seq(rest, pos, k)
                } else {
                    false
                }
            }
            RNode::StrStart => pos == 0 && self.seq(rest, pos, k),
            RNode::StrEnd => pos == self.s.len() && self.seq(rest, pos, k),
            RNode::WordB(want) => {
                let a = pos > 0 && is_word(self.s[pos - 1], ascii);
                let b = pos < self.s.len() && is_word(self.s[pos], ascii);
                if (a != b) == *want {
                    self.seq(rest, pos, k)
                } else {
                    false
                }
            }
            RNode::Group(alts, g) => match g {
                None => self.alts(alts, pos, &mut |c: &mut Ctx, p| c.seq(rest, p, k)),
                Some(g) => {
                    let g = *g;
                    self.alts(alts, pos, &mut |c: &mut Ctx, p| {
                        let (os, oe) = (c.caps[2 * g], c.caps[2 * g + 1]);
                        c.caps[2 * g] = pos;
                        c.caps[2 * g + 1] = p;
                        if c.seq(rest, p, k) {
                            return true;
                        }
                        c.caps[2 * g] = os;
                        c.caps[2 * g + 1] = oe;
                        false
                    })
                }
            },
            RNode::Atomic(alts) => {
                let mut end = None;
                let saved = self.caps.clone();
                if self.alts(alts, pos, &mut |_c: &mut Ctx, p| {
                    end = Some(p);
                    true
                }) {
                    if self.seq(rest, end.unwrap(), k) {
                        return true;
                    }
                }
                self.caps = saved;
                false
            }
            RNode::Look(alts, behind, neg) => {
                let saved = self.caps.clone();
                let ok = if *behind {
                    let mut ok = false;
                    for st in (0..=pos).rev() {
                        if self.alts(alts, st, &mut |_c: &mut Ctx, p| p == pos) {
                            ok = true;
                            break;
                        }
                    }
                    ok
                } else {
                    self.alts(alts, pos, &mut |_c: &mut Ctx, _p| true)
                };
                if ok == *neg {
                    self.caps = saved;
                    return false;
                }
                if *neg {
                    self.caps = saved.clone();
                }
                if self.seq(rest, pos, k) {
                    return true;
                }
                self.caps = saved;
                false
            }
            RNode::Backref(g) => {
                let (a, b) = (self.caps[2 * g], self.caps[2 * g + 1]);
                if a == UNSET {
                    return false;
                }
                let len = b - a;
                if pos + len > self.s.len() {
                    return false;
                }
                let ic = self.flags & F_I != 0;
                for i in 0..len {
                    let (x, y) = (self.s[a + i], self.s[pos + i]);
                    if x != y && !(ic && x.to_lowercase().eq(y.to_lowercase())) {
                        return false;
                    }
                }
                self.seq(rest, pos + len, k)
            }
            RNode::Cond(g, yes, no) => {
                if self.caps[2 * g] != UNSET {
                    self.seq(yes, pos, &mut |c: &mut Ctx, p| c.seq(rest, p, k))
                } else {
                    self.seq(no, pos, &mut |c: &mut Ctx, p| c.seq(rest, p, k))
                }
            }
            RNode::Repeat(node, lo, hi, greedy) => {
                if self.single(node, pos).is_some() {
                    // single-character repeats: count, then backtrack over the count
                    let mut n = 0;
                    let max_n = (*hi).min(self.s.len() - pos);
                    if *greedy {
                        while n < max_n && self.single(node, pos + n).unwrap() {
                            n += 1;
                        }
                        if n < *lo {
                            return false;
                        }
                        loop {
                            if self.seq(rest, pos + n, k) {
                                return true;
                            }
                            if n == *lo {
                                return false;
                            }
                            n -= 1;
                        }
                    } else {
                        while n < *lo {
                            if n >= max_n || !self.single(node, pos + n).unwrap() {
                                return false;
                            }
                            n += 1;
                        }
                        loop {
                            if self.seq(rest, pos + n, k) {
                                return true;
                            }
                            if n >= max_n || !self.single(node, pos + n).unwrap() {
                                return false;
                            }
                            n += 1;
                        }
                    }
                }
                self.rep(node, *lo, *hi, *greedy, 0, pos, usize::MAX, rest, k)
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn rep(&mut self, node: &RNode, lo: usize, hi: usize, greedy: bool, cnt: usize, pos: usize, last: usize, rest: &[RNode], k: Kont) -> bool {
        let one = std::slice::from_ref(node);
        if cnt < lo {
            return self.seq(one, pos, &mut |c: &mut Ctx, p| c.rep(node, lo, hi, greedy, cnt + 1, p, pos, rest, k));
        }
        // an iteration that matched empty ends the loop (as sre does)
        if last == pos && cnt > lo {
            return self.seq(rest, pos, k);
        }
        if greedy {
            // sre keeps one iteration that matched empty (its groups stay set), then ends the loop
            if cnt < hi && self.seq(one, pos, &mut |c: &mut Ctx, p| c.rep(node, lo, hi, greedy, cnt + 1, p, pos, rest, k)) {
                return true;
            }
            self.seq(rest, pos, k)
        } else {
            if self.seq(rest, pos, k) {
                return true;
            }
            cnt < hi && self.seq(one, pos, &mut |c: &mut Ctx, p| p != pos && c.rep(node, lo, hi, greedy, cnt + 1, p, pos, rest, k))
        }
    }
}

// A position single() can look at to tell whether a node is one character test (it answers None otherwise).
fn st_probe(pos: usize, len: usize) -> usize {
    pos.min(len)
}
// Tries to match at each start in [pos, endpos]. anchored: only at pos. full: must end at endpos.
// no_empty_at: an empty match at that position is not accepted.
fn re_exec(p: &Prog, s: &[char], pos: usize, endpos: usize, anchored: bool, full: bool, no_empty_at: Option<usize>) -> Option<Vec<usize>> {
    let sub = &s[..endpos];
    let mut c = Ctx { s: sub, caps: vec![UNSET; 2 * (p.ngroups + 1)], flags: p.flags, steps: 0 };
    let last = if anchored { pos } else { endpos };
    // The characters a match can start with, when the pattern says (a literal, or a class of literals such as
    // re.I gives, after any \b, ^ or \A): the search skips to the next of them.
    let firsts: Option<Vec<char>> = match p.alts.as_slice() {
        [a] => match a.iter().find(|n| !matches!(n, RNode::WordB(_) | RNode::Bol | RNode::StrStart)) {
            Some(RNode::Char(ch)) => Some(vec![*ch]),
            Some(RNode::Class(k)) if !k.neg && k.items.iter().all(|i| matches!(i, CItem::Ch(_))) => Some(k.items.iter().map(|i| if let CItem::Ch(c) = i { *c } else { unreachable!() }).collect()),
            _ => None,
        },
        _ => None,
    };
    // A pattern that is one character test (`[^a-z ]`, `\d`, `x`): the match is the next character passing it.
    if let [a] = p.alts.as_slice() {
        if let [n] = a.as_slice() {
            if !full && no_empty_at.is_none() && c.single(n, st_probe(pos, sub.len())).is_some() {
                let mut st = pos;
                while st <= last && st < sub.len() {
                    if c.single(n, st).unwrap() {
                        c.caps[0] = st;
                        c.caps[1] = st + 1;
                        return Some(c.caps);
                    }
                    st += 1;
                }
                return None;
            }
        }
    }
    let mut st = pos;
    while st <= last {
        if let Some(f) = &firsts {
            let found = if let [one] = f.as_slice() { sub[st..].iter().position(|x| x == one) } else { sub[st..].iter().position(|x| f.contains(x)) };
            match found {
                Some(o) => st += o,
                None => return None,
            }
            if st > last {
                return None;
            }
        }
        let mut end = UNSET;
        let start = st;
        let ok = c.alts(&p.alts, st, &mut |_c: &mut Ctx, e| {
            if full && e != endpos {
                return false;
            }
            if no_empty_at == Some(start) && e == start {
                return false;
            }
            end = e;
            true
        });
        if ok {
            c.caps[0] = st;
            c.caps[1] = end;
            return Some(c.caps);
        }
        for x in c.caps.iter_mut() {
            *x = UNSET;
        }
        st += 1;
    }
    None
}

pub struct RPat {
    prog: Rc<Prog>,
}
pub struct RMatch {
    prog: Rc<Prog>,
    s: Rc<Vec<char>>,
    sv: V,
    caps: Vec<usize>,
    pos: usize,
    endpos: usize,
}
// The integer a re group argument means: ints, bools and (transitively) int subclasses. Anything else has no index.
fn group_int(v: &V) -> Option<i64> {
    match v {
        V::Int(i) => Some(*i),
        V::Bool(b) => Some(*b as i64),
        V::Big(b) => b.to_i64(),
        V::Obj(_) => inner_of(v).and_then(|i| group_int(&i)),
        _ => None,
    }
}
fn group_is_int(v: &V) -> bool {
    matches!(v, V::Int(_) | V::Bool(_) | V::Big(_)) || matches!(v, V::Obj(_)) && inner_of(v).is_some_and(|i| group_is_int(&i))
}
impl RMatch {
    fn group_idx(&self, g: &V) -> Result<usize, E> {
        let n = match g {
            V::Str(n) => return self.prog.names.iter().find(|x| x.0 == n.s).map(|x| x.1).ok_or_else(|| exc("IndexError", "no such group".into())),
            V::Obj(o) => {
                if let Some(f) = o.cls.lookup("__index__") {
                    let r = call(&f, Args::p(vec![g.clone()]))?;
                    if !group_is_int(&r) {
                        return type_err(format!("__index__ returned non-int (type {})", tn(&r)));
                    }
                    group_int(&r)
                } else {
                    group_int(g)
                }
            }
            _ => group_int(g),
        };
        match n {
            Some(i) if i >= 0 && (i as usize) <= self.prog.ngroups => Ok(i as usize),
            _ => Err(exc("IndexError", "no such group".into())),
        }
    }
    fn text(&self, g: usize) -> Option<String> {
        let (a, b) = (self.caps[2 * g], self.caps[2 * g + 1]);
        if a == UNSET {
            None
        } else {
            Some(self.s[a..b].iter().collect())
        }
    }
    fn gv(&self, g: usize, dflt: &V) -> V {
        match self.text(g) {
            Some(t) => st(t),
            None => dflt.clone(),
        }
    }
}
impl Ext for RMatch {
    fn tname(&self) -> &'static str {
        "re.Match"
    }
    fn repr(&self) -> String {
        format!("<re.Match object; span=({}, {}), match={}>", self.caps[0], self.caps[1], str_repr(&self.text(0).unwrap_or_default()))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "string" => self.sv.clone(),
            "pos" => V::Int(self.pos as i64),
            "endpos" => V::Int(self.endpos as i64),
            "re" => V::Ext(ExtRc::new(RPat { prog: self.prog.clone() })),
            "regs" => {
                let f = |v: usize| if v == UNSET { V::Int(-1) } else { V::Int(v as i64) };
                tup((0..=self.prog.ngroups).map(|g| tup(vec![f(self.caps[2 * g]), f(self.caps[2 * g + 1])])).collect())
            }
            "lastindex" => {
                let mut best: Option<(usize, usize)> = None;
                for g in 1..=self.prog.ngroups {
                    if self.caps[2 * g] != UNSET {
                        let e = self.caps[2 * g + 1];
                        if best.is_none_or(|b| e > b.1 || (e == b.1 && false)) {
                            best = Some((g, e));
                        }
                    }
                }
                match best {
                    Some((g, _)) => V::Int(g as i64),
                    None => V::None,
                }
            }
            "lastgroup" => {
                let mut best: Option<(usize, usize)> = None;
                for g in 1..=self.prog.ngroups {
                    if self.caps[2 * g] != UNSET {
                        let e = self.caps[2 * g + 1];
                        if best.is_none_or(|b| e > b.1) {
                            best = Some((g, e));
                        }
                    }
                }
                match best.and_then(|(g, _)| self.prog.names.iter().find(|x| x.1 == g)) {
                    Some((n, _)) => s(n),
                    None => V::None,
                }
            }
            _ => return None,
        }))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        // No item assignment or deletion: leave it to the caller's TypeError, like len() and iteration.
        if matches!(n, "__setitem__" | "__delitem__") {
            return None;
        }
        let r = (|| -> R {
            match n {
                "group" | "__getitem__" => {
                    if a.p.is_empty() {
                        return Ok(self.gv(0, &V::None));
                    }
                    if a.p.len() == 1 {
                        let g = self.group_idx(&a.p[0])?;
                        return Ok(self.gv(g, &V::None));
                    }
                    let mut v = vec![];
                    for x in &a.p {
                        let g = self.group_idx(x)?;
                        v.push(self.gv(g, &V::None));
                    }
                    Ok(tup(v))
                }
                "groups" => {
                    let d = a.kw("default").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
                    Ok(tup((1..=self.prog.ngroups).map(|g| self.gv(g, &d)).collect()))
                }
                "groupdict" => {
                    let d = a.kw("default").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
                    let mut out = Dict::new();
                    for (nm, g) in &self.prog.names {
                        out.set(s(nm), self.gv(*g, &d))?;
                    }
                    Ok(V::Dict(Rc::new(RefCell::new(out))))
                }
                "start" | "end" | "span" => {
                    let g = match a.p.first() {
                        Some(x) => self.group_idx(x)?,
                        None => 0,
                    };
                    let (x, y) = (self.caps[2 * g], self.caps[2 * g + 1]);
                    let f = |v: usize| if v == UNSET { V::Int(-1) } else { V::Int(v as i64) };
                    Ok(match n {
                        "start" => f(x),
                        "end" => f(y),
                        _ => tup(vec![f(x), f(y)]),
                    })
                }
                "expand" => {
                    let t = to_str(&a.p[0])?;
                    Ok(st(expand_template(&t, self)?))
                }
                "__bool__" => Ok(V::Bool(true)),
                _ => Err(exc("AttributeError", format!("'re.Match' object has no attribute '{n}'"))),
            }
        })();
        let _ = me;
        Some(r)
    }
}
impl Ext for RPat {
    fn tname(&self) -> &'static str {
        "re.Pattern"
    }
    fn repr(&self) -> String {
        // CPython's: the pattern's repr cut to 200 characters, then the flags other than the implied UNICODE
        let r: String = str_repr(&self.prog.src).chars().take(200).collect();
        match self.prog.flags & !32 {
            0 => format!("re.compile({r})"),
            f => format!("re.compile({r}, {})", re_flag_names(f)),
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(match o {
            V::Ext(x) => x.any().downcast_ref::<RPat>().is_some_and(|p| p.prog.src == self.prog.src && p.prog.flags == self.prog.flags),
            _ => false,
        })
    }
    fn hash(&self) -> Option<i64> {
        Some(self.prog.src.len() as i64)
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "pattern" => s(&self.prog.src),
            "flags" => V::Int(if self.prog.flags & F_A != 0 { self.prog.flags } else { self.prog.flags | 32 }),
            "groups" => V::Int(self.prog.ngroups as i64),
            "groupindex" => {
                let mut d = Dict::new();
                for (nm, g) in &self.prog.names {
                    if let Err(e) = d.set(s(nm), V::Int(*g as i64)) {
                        return Some(Err(e));
                    }
                }
                // a read-only view, as CPython's mappingproxy (a plain empty dict when there are no names)
                if self.prog.names.is_empty() {
                    V::Dict(Rc::new(RefCell::new(d)))
                } else {
                    V::View(Rc::new((V::Dict(Rc::new(RefCell::new(d))), 3)))
                }
            }
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let mut args = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
        Some(pat_call(&self.prog, n, &mut args))
    }
}

fn re_str(v: &V) -> Result<(Rc<Vec<char>>, V), E> {
    match v {
        // The last string searched keeps its characters: a loop that tries many patterns on one text (#195)
        // converts it once. Holding the string keeps its address from being reused.
        V::Str(x) => {
            thread_local! { static LAST: RefCell<Option<(Rc<S>, Rc<Vec<char>>)>> = RefCell::new(None); }
            if let Some(c) = LAST.with(|l| l.borrow().as_ref().filter(|(k, _)| Rc::ptr_eq(k, x)).map(|(_, c)| c.clone())) {
                return Ok((c, v.clone()));
            }
            let c: Rc<Vec<char>> = Rc::new(x.s.chars().collect());
            LAST.with(|l| *l.borrow_mut() = Some((x.clone(), c.clone())));
            Ok((c, v.clone()))
        }
        V::Ext(e) if matches!(e.native(), Some(Ok(V::Str(_)))) => re_str(&e.native().unwrap()?),
        _ => type_err(format!("expected string or bytes-like object, got '{}'", tn(v))),
    }
}
fn opt_pos(a: &mut Args, i: usize, name: &str, n: usize) -> Result<Option<usize>, E> {
    let v = a.kw(name).or_else(|| a.p.get(i).cloned());
    match v {
        None | Some(V::None) => Ok(None),
        Some(V::Int(x)) => Ok(Some(if x < 0 { 0 } else { (x as usize).min(n) })),
        Some(x) => type_err(format!("'{}' object cannot be interpreted as an integer", tn(&x))),
    }
}
fn mk_match(prog: &Rc<Prog>, s: &Rc<Vec<char>>, sv: &V, caps: Vec<usize>, pos: usize, endpos: usize) -> V {
    V::Ext(ExtRc::new(RMatch { prog: prog.clone(), s: s.clone(), sv: sv.clone(), caps, pos, endpos }))
}
// all non-overlapping matches, as Python's scanner finds them
fn all_matches(prog: &Prog, s: &[char], pos: usize, endpos: usize, limit: usize) -> Vec<Vec<usize>> {
    let mut out = vec![];
    let mut at = pos;
    let mut no_empty = None;
    while at <= endpos && out.len() < limit {
        match re_exec(prog, s, at, endpos, false, false, no_empty) {
            Some(c) => {
                let (a, b) = (c[0], c[1]);
                no_empty = if a == b { Some(b) } else { None };
                at = b;
                out.push(c);
                if a == b && b >= endpos {
                    break;
                }
            }
            None => break,
        }
    }
    out
}
fn expand_template(t: &str, m: &RMatch) -> Result<String, E> {
    let c: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] != '\\' || i + 1 >= c.len() {
            if c[i] == '\\' {
                return re_err("bad escape (end of pattern)".into());
            }
            out.push(c[i]);
            i += 1;
            continue;
        }
        let e = c[i + 1];
        i += 2;
        match e {
            'g' => {
                if c.get(i) != Some(&'<') {
                    return re_err("missing <".into());
                }
                let st = i + 1;
                let Some(end) = c[st..].iter().position(|x| *x == '>') else { return re_err("missing >, unterminated name".into()) };
                let name: String = c[st..st + end].iter().collect();
                i = st + end + 1;
                let g = match name.parse::<usize>() {
                    Ok(n) if n <= m.prog.ngroups => n,
                    Ok(n) => return re_err(format!("invalid group reference {n}")),
                    Err(_) => m.prog.names.iter().find(|x| x.0 == name).map(|x| x.1).ok_or_else(|| exc("IndexError", format!("unknown group name '{name}'")))?,
                };
                out.push_str(&m.text(g).unwrap_or_default());
            }
            '0'..='9' => {
                let mut n = e.to_digit(10).unwrap() as usize;
                if c.get(i).is_some_and(|x| x.is_ascii_digit()) && n * 10 + (c[i].to_digit(10).unwrap() as usize) <= m.prog.ngroups {
                    n = n * 10 + c[i].to_digit(10).unwrap() as usize;
                    i += 1;
                }
                if e == '0' {
                    out.push('\0');
                    continue;
                }
                if n > m.prog.ngroups {
                    return re_err(format!("invalid group reference {n} at position {}", i - 1));
                }
                out.push_str(&m.text(n).unwrap_or_default());
            }
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'f' => out.push('\x0c'),
            'v' => out.push('\x0b'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            '\\' => out.push('\\'),
            x if x.is_ascii_alphabetic() => return re_err(format!("bad escape \\{x} at position {}", i - 2)),
            x => {
                out.push('\\');
                out.push(x);
            }
        }
    }
    Ok(out)
}
fn pat_call(prog: &Rc<Prog>, n: &str, a: &mut Args) -> R {
    match n {
        "match" | "fullmatch" | "search" => {
            let sv = a.kw("string").or_else(|| a.p.first().cloned()).ok_or_else(|| exc("TypeError", format!("{n}() missing required argument 'string' (pos 1)")))?;
            let (sc, sv) = re_str(&sv)?;
            let len = sc.len();
            let pos = opt_pos(a, 1, "pos", len)?.unwrap_or(0);
            let endpos = opt_pos(a, 2, "endpos", len)?.unwrap_or(len);
            if pos > endpos {
                return Ok(V::None);
            }
            Ok(match re_exec(prog, &sc, pos, endpos, n != "search", n == "fullmatch", None) {
                Some(c) => mk_match(prog, &sc, &sv, c, pos, endpos),
                None => V::None,
            })
        }
        "findall" | "finditer" => {
            let sv = a.kw("string").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
            let (sc, sv) = re_str(&sv)?;
            let len = sc.len();
            let pos = opt_pos(a, 1, "pos", len)?.unwrap_or(0);
            let endpos = opt_pos(a, 2, "endpos", len)?.unwrap_or(len);
            let ms = all_matches(prog, &sc, pos, endpos, usize::MAX);
            if n == "finditer" {
                return Ok(vec_iter(ms.into_iter().map(|c| mk_match(prog, &sc, &sv, c, pos, endpos)).collect()));
            }
            let txt = |c: &Vec<usize>, g: usize| -> V {
                if c[2 * g] == UNSET {
                    s("")
                } else {
                    st(sc[c[2 * g]..c[2 * g + 1]].iter().collect())
                }
            };
            Ok(list(
                ms.iter()
                    .map(|c| match prog.ngroups {
                        0 => txt(c, 0),
                        1 => txt(c, 1),
                        k => tup((1..=k).map(|g| txt(c, g)).collect()),
                    })
                    .collect(),
            ))
        }
        "sub" | "subn" => {
            let repl = a.kw("repl").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
            let sv = a.kw("string").or_else(|| a.p.get(1).cloned()).unwrap_or(V::None);
            let count = match a.kw("count").or_else(|| a.p.get(2).cloned()) {
                Some(V::Int(c)) if c > 0 => c as usize,
                _ => usize::MAX,
            };
            let (sc, sv) = re_str(&sv)?;
            let ms = all_matches(prog, &sc, 0, sc.len(), count);
            let mut out = String::new();
            let mut last = 0;
            let nsub = ms.len();
            let tmpl = match &repl {
                V::Str(t) => Some(t.s.clone()),
                _ => None,
            };
            for c in ms {
                out.extend(sc[last..c[0]].iter());
                let m = RMatch { prog: prog.clone(), s: sc.clone(), sv: sv.clone(), caps: c.clone(), pos: 0, endpos: sc.len() };
                match &tmpl {
                    Some(t) => {
                        if t.contains('\\') {
                            out.push_str(&expand_template(t, &m)?);
                        } else {
                            out.push_str(t);
                        }
                    }
                    None => {
                        let r = call(&repl, Args::p(vec![V::Ext(ExtRc::new(m))]))?;
                        match &r {
                            V::Str(x) => out.push_str(&x.s),
                            _ => return type_err(format!("expected str instance, {} found", tn(&r))),
                        }
                    }
                }
                last = c[1];
            }
            out.extend(sc[last..].iter());
            Ok(if n == "subn" { tup(vec![st(out), V::Int(nsub as i64)]) } else { st(out) })
        }
        "split" => {
            let sv = a.kw("string").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
            let maxsplit = match a.kw("maxsplit").or_else(|| a.p.get(1).cloned()) {
                Some(V::Int(c)) if c > 0 => c as usize,
                _ => usize::MAX,
            };
            let (sc, _) = re_str(&sv)?;
            let ms = all_matches(prog, &sc, 0, sc.len(), maxsplit);
            let mut out = vec![];
            let mut last = 0;
            for c in ms {
                out.push(st(sc[last..c[0]].iter().collect()));
                for g in 1..=prog.ngroups {
                    out.push(if c[2 * g] == UNSET { V::None } else { st(sc[c[2 * g]..c[2 * g + 1]].iter().collect()) });
                }
                last = c[1];
            }
            out.push(st(sc[last..].iter().collect()));
            Ok(list(out))
        }
        _ => Err(exc("AttributeError", format!("'re.Pattern' object has no attribute '{n}'"))),
    }
}
fn flags_arg(a: &mut Args, i: usize) -> Result<i64, E> {
    let v = a.kw("flags").or_else(|| if a.p.len() > i { Some(a.p.remove(i)) } else { None });
    match v {
        None => Ok(0),
        Some(V::Int(f)) => Ok(f),
        Some(V::Bool(b)) => Ok(b as i64),
        Some(x) => match inner_of(&x) {
            Some(V::Int(f)) => Ok(f),
            _ => type_err(format!("unsupported operand type(s) for &: '{}' and 'int'", tn(&x))),
        },
    }
}
fn pat_of(v: &V, flags: i64) -> Result<Rc<Prog>, E> {
    match v {
        V::Str(x) => re_compile(&x.s, flags),
        V::Ext(x) => match x.any().downcast_ref::<RPat>() {
            Some(p) => {
                if flags != 0 {
                    return Err(exc("ValueError", "cannot process flags argument with a compiled pattern".into()));
                }
                Ok(p.prog.clone())
            }
            None if tn(v) == "bytes" => Err(unsupported("re with a bytes pattern")),
            None => type_err("first argument must be string or compiled pattern".into()),
        },
        _ => type_err("first argument must be string or compiled pattern".into()),
    }
}
pub fn re_fn(n: &str, mut a: Args) -> R {
    a.k.retain(|(k, _)| *k != "pattern" || {
        true
    });
    let pv = match a.kw("pattern") {
        Some(p) => p,
        None => {
            if a.p.is_empty() {
                return type_err(format!("{n}() missing required argument 'pattern' (pos 1)"));
            }
            a.p.remove(0)
        }
    };
    a.k.retain(|(k, _)| *k != "pattern");
    match n {
        "compile" => {
            let f = flags_arg(&mut a, 0)?;
            if let V::Ext(x) = &pv {
                if x.any().downcast_ref::<RPat>().is_some() {
                    if f != 0 {
                        return Err(exc("ValueError", "cannot process flags argument with a compiled pattern".into()));
                    }
                    return Ok(pv);
                }
            }
            Ok(V::Ext(ExtRc::new(RPat { prog: pat_of(&pv, f)? })))
        }
        "escape" => {
            let t = to_str(&pv)?;
            let mut out = String::new();
            for c in t.chars() {
                if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
                    out.push('\\');
                }
                out.push(c);
            }
            Ok(st(out))
        }
        "purge" => Ok(V::None),
        "match" | "fullmatch" | "search" | "findall" | "finditer" | "split" => {
            let fi = if n == "split" { 2 } else { 1 };
            let f = flags_arg(&mut a, fi)?;
            let prog = pat_of(&pv, f)?;
            a.k.retain(|(k, _)| *k != "flags");
            if n == "split" {
                if let Some(m) = a.kw("maxsplit") {
                    a.k.retain(|(k, _)| *k != "maxsplit");
                    a.p.truncate(1);
                    a.p.push(m);
                }
            }
            pat_call(&prog, n, &mut a)
        }
        "sub" | "subn" => {
            let f = flags_arg(&mut a, 3)?;
            let prog = pat_of(&pv, f)?;
            a.k.retain(|(k, _)| *k != "flags");
            pat_call(&prog, n, &mut a)
        }
        _ => Err(unsupported(&format!("re.{n}"))),
    }
}

// ---------- json ----------

struct JP<'a> {
    s: &'a [char],
    i: usize,
    h: Option<&'a JHooks>,
}
// json.JSONDecoder's options, as its __init__ takes them (None: the default)
#[derive(Clone, Default)]
struct JHooks {
    object_hook: Option<V>,
    parse_float: Option<V>,
    parse_int: Option<V>,
    parse_constant: Option<V>,
    pairs_hook: Option<V>,
    lax: bool, // strict=False: control characters allowed in strings
}
fn json_err<T>(msg: &str, s: &[char], pos: usize) -> Result<T, E> {
    let line = s[..pos].iter().filter(|c| **c == '\n').count() + 1;
    let col = match s[..pos].iter().rposition(|c| *c == '\n') {
        Some(p) => pos - p,
        None => pos + 1,
    };
    let full = format!("{msg}: line {line} column {col} (char {pos})");
    let c = exc_class("JSONDecodeError");
    let o = Obj::new(c);
    o.set("args", tup(vec![st(full)]));
    o.set("msg", s_(msg));
    o.set("doc", st(s.iter().collect()));
    o.set("pos", V::Int(pos as i64));
    o.set("lineno", V::Int(line as i64));
    o.set("colno", V::Int(col as i64));
    Err(E(V::Obj(o)))
}
fn s_(x: &str) -> V {
    s(x)
}
impl<'a> JP<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], ' ' | '\t' | '\n' | '\r') {
            self.i += 1;
        }
    }
    fn lit(&self, w: &str) -> bool {
        let w: Vec<char> = w.chars().collect();
        self.s.get(self.i..self.i + w.len()) == Some(&w[..])
    }
    fn value(&mut self) -> R {
        self.ws();
        let Some(&c) = self.s.get(self.i) else { return json_err("Expecting value", self.s, self.i) };
        match c {
            '{' => {
                let start = self.i;
                let _ = start;
                self.i += 1;
                let mut d = Dict::new();
                let pairs_hook = self.h.and_then(|h| h.pairs_hook.clone());
                let mut pairs = vec![];
                self.ws();
                if self.s.get(self.i) == Some(&'}') {
                    self.i += 1;
                    return self.object(d, pairs);
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&'"') {
                        return json_err("Expecting property name enclosed in double quotes", self.s, self.i);
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&':') {
                        return json_err("Expecting ':' delimiter", self.s, self.i);
                    }
                    self.i += 1;
                    let v = self.value()?;
                    if pairs_hook.is_some() {
                        pairs.push(tup(vec![k, v]));
                    } else {
                        d.set(k, v)?;
                    }
                    self.ws();
                    match self.s.get(self.i) {
                        Some(',') => {
                            let comma = self.i;
                            self.i += 1;
                            self.ws();
                            if self.s.get(self.i) == Some(&'}') {
                                return json_err("Illegal trailing comma before end of object", self.s, comma);
                            }
                            if self.s.get(self.i) != Some(&'"') {
                                return json_err("Expecting property name enclosed in double quotes", self.s, self.i);
                            }
                        }
                        Some('}') => {
                            self.i += 1;
                            return self.object(d, pairs);
                        }
                        _ => return json_err("Expecting ',' delimiter", self.s, self.i),
                    }
                }
            }
            '[' => {
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.s.get(self.i) == Some(&']') {
                    self.i += 1;
                    return Ok(list(v));
                }
                loop {
                    v.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(',') => {
                            self.i += 1;
                            self.ws();
                            if self.s.get(self.i) == Some(&']') {
                                return json_err("Illegal trailing comma before end of array", self.s, self.i - 1);
                            }
                        }
                        Some(']') => {
                            self.i += 1;
                            return Ok(list(v));
                        }
                        _ => return json_err("Expecting ',' delimiter", self.s, self.i),
                    }
                }
            }
            '"' => self.string(),
            'n' if self.lit("null") => {
                self.i += 4;
                Ok(V::None)
            }
            't' if self.lit("true") => {
                self.i += 4;
                Ok(V::Bool(true))
            }
            'f' if self.lit("false") => {
                self.i += 5;
                Ok(V::Bool(false))
            }
            'N' if self.lit("NaN") => {
                self.i += 3;
                self.constant("NaN", f64::NAN)
            }
            'I' if self.lit("Infinity") => {
                self.i += 8;
                self.constant("Infinity", f64::INFINITY)
            }
            '-' if self.s.get(self.i + 1..self.i + 9).is_some_and(|w| w.iter().collect::<String>() == "Infinity") => {
                self.i += 9;
                self.constant("-Infinity", f64::NEG_INFINITY)
            }
            '-' | '0'..='9' => {
                let st0 = self.i;
                if c == '-' {
                    self.i += 1;
                }
                let ds = self.i;
                if self.s.get(self.i) == Some(&'0') {
                    self.i += 1;
                } else {
                    while self.s.get(self.i).is_some_and(|c| c.is_ascii_digit()) {
                        self.i += 1;
                    }
                }
                if self.i == ds {
                    return json_err("Expecting value", self.s, st0);
                }
                let mut is_f = false;
                if self.s.get(self.i) == Some(&'.') && self.s.get(self.i + 1).is_some_and(|c| c.is_ascii_digit()) {
                    is_f = true;
                    self.i += 1;
                    while self.s.get(self.i).is_some_and(|c| c.is_ascii_digit()) {
                        self.i += 1;
                    }
                }
                if matches!(self.s.get(self.i), Some('e') | Some('E')) {
                    let save = self.i;
                    self.i += 1;
                    if matches!(self.s.get(self.i), Some('+') | Some('-')) {
                        self.i += 1;
                    }
                    if self.s.get(self.i).is_some_and(|c| c.is_ascii_digit()) {
                        is_f = true;
                        while self.s.get(self.i).is_some_and(|c| c.is_ascii_digit()) {
                            self.i += 1;
                        }
                    } else {
                        self.i = save;
                    }
                }
                let t: String = self.s[st0..self.i].iter().collect();
                let hook = self.h.and_then(|h| if is_f { h.parse_float.clone() } else { h.parse_int.clone() });
                if let Some(f) = hook {
                    return call(&f, Args::p(vec![st(t)]));
                }
                if is_f {
                    Ok(V::Float(t.parse().unwrap_or(0.0)))
                } else {
                    parse_int(&t, 10).ok_or_else(|| exc("ValueError", "bad number".into()))
                }
            }
            _ => json_err("Expecting value", self.s, self.i),
        }
    }
    fn object(&self, d: Dict, pairs: Vec<V>) -> R {
        if let Some(f) = self.h.and_then(|h| h.pairs_hook.clone()) {
            return call(&f, Args::p(vec![list(pairs)]));
        }
        let d = V::Dict(Rc::new(RefCell::new(d)));
        match self.h.and_then(|h| h.object_hook.clone()) {
            Some(f) => call(&f, Args::p(vec![d])),
            None => Ok(d),
        }
    }
    fn constant(&self, name: &str, f: f64) -> R {
        match self.h.and_then(|h| h.parse_constant.clone()) {
            Some(c) => call(&c, Args::p(vec![s(name)])),
            None => Ok(V::Float(f)),
        }
    }
    fn string(&mut self) -> R {
        let start = self.i;
        self.i += 1;
        let mut out = String::new();
        loop {
            let Some(&c) = self.s.get(self.i) else { return json_err("Unterminated string starting at", self.s, start) };
            self.i += 1;
            match c {
                '"' => return Ok(st(out)),
                '\\' => {
                    let Some(&e) = self.s.get(self.i) else { return json_err("Unterminated string starting at", self.s, start) };
                    self.i += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\x08'),
                        'f' => out.push('\x0c'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex = |p: &Self, at: usize| -> Option<u32> {
                                let h: String = p.s.get(at..at + 4)?.iter().collect();
                                if h.len() == 4 && h.chars().all(|c| c.is_ascii_hexdigit()) {
                                    u32::from_str_radix(&h, 16).ok()
                                } else {
                                    None
                                }
                            };
                            let Some(u) = hex(self, self.i) else { return json_err("Invalid \\uXXXX escape", self.s, self.i - 1) };
                            self.i += 4;
                            let mut cp = u;
                            if (0xd800..0xdc00).contains(&u) && self.s.get(self.i) == Some(&'\\') && self.s.get(self.i + 1) == Some(&'u') {
                                if let Some(l) = hex(self, self.i + 2) {
                                    if (0xdc00..0xe000).contains(&l) {
                                        cp = 0x10000 + ((u - 0xd800) << 10) + (l - 0xdc00);
                                        self.i += 6;
                                    }
                                }
                            }
                            // a lone surrogate: CPython keeps it, a Rust string cannot hold it
                            out.push(char::from_u32(cp).ok_or_else(|| unsupported("json: a lone surrogate \\u escape"))?);
                        }
                        _ => return json_err("Invalid \\escape", self.s, self.i - 2),
                    }
                }
                c if (c as u32) < 0x20 && !self.h.is_some_and(|h| h.lax) => return json_err("Invalid control character at", self.s, self.i - 1),
                c => out.push(c),
            }
        }
    }
}
pub fn json_loads(t: &str) -> R {
    json_loads_h(t, None)
}
fn json_loads_h(t: &str, h: Option<&JHooks>) -> R {
    let c: Vec<char> = t.chars().collect();
    let mut p = JP { s: &c, i: 0, h };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != c.len() {
        return json_err("Extra data", &c, p.i);
    }
    Ok(v)
}
struct JOpts {
    indent: Option<String>,
    item_sep: String,
    key_sep: String,
    sort_keys: bool,
    ascii: bool,
    default: Option<V>,
    skipkeys: bool,
    allow_nan: bool,
    // check_circular: the ids of the containers (and objects handed to default) being encoded
    markers: Option<RefCell<Vec<usize>>>,
}
impl Default for JOpts {
    fn default() -> Self {
        JOpts { indent: None, item_sep: ", ".into(), key_sep: ": ".into(), sort_keys: false, ascii: true, default: None, skipkeys: false, allow_nan: true, markers: Some(RefCell::new(vec![])) }
    }
}
fn json_mark(o: &JOpts, v: &V) -> Result<Option<usize>, E> {
    let Some(m) = &o.markers else { return Ok(None) };
    let id = match v {
        V::List(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Dict(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Tuple(x) => Tup::as_ptr(x) as *const u8 as usize,
        V::Obj(x) => Rc::as_ptr(x) as *const u8 as usize,
        _ => return Ok(None),
    };
    if m.borrow().contains(&id) {
        return val_err("Circular reference detected".into());
    }
    m.borrow_mut().push(id);
    Ok(Some(id))
}
fn json_unmark(o: &JOpts, id: Option<usize>) {
    if let (Some(m), Some(_)) = (&o.markers, id) {
        m.borrow_mut().pop();
    }
}
fn json_str(t: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in t.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if ascii && (c as u32) > 0x7e => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    let _ = write!(out, "\\u{:04x}", u);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
fn json_float(f: f64, o: &JOpts) -> Result<String, E> {
    if !o.allow_nan && !f.is_finite() {
        return val_err(format!("Out of range float values are not JSON compliant: {}", float_repr(f)));
    }
    Ok(if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else {
        float_repr(f)
    })
}
fn json_dump(v: &V, o: &JOpts, depth: usize, out: &mut String) -> Result<(), E> {
    enter()?;
    let r = json_dump_inner(v, o, depth, out);
    leave();
    r
}
fn json_dump_inner(v: &V, o: &JOpts, depth: usize, out: &mut String) -> Result<(), E> {
    let nl = |out: &mut String, d: usize| {
        if let Some(ind) = &o.indent {
            out.push('\n');
            for _ in 0..d {
                out.push_str(ind);
            }
        }
    };
    match v {
        V::None => out.push_str("null"),
        V::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        V::Int(_) | V::Big(_) => out.push_str(&repr(v)?),
        V::Float(f) => out.push_str(&json_float(*f, o)?),
        V::Str(x) => json_str(&x.s, o.ascii, out),
        V::List(_) | V::Tuple(_) => {
            let items = to_vec(v)?;
            if items.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            let id = json_mark(o, v)?;
            out.push('[');
            for (k, x) in items.iter().enumerate() {
                if k > 0 {
                    out.push_str(&o.item_sep);
                }
                nl(out, depth + 1);
                json_dump(x, o, depth + 1, out)?;
            }
            nl(out, depth);
            out.push(']');
            json_unmark(o, id);
        }
        V::Dict(d) => {
            let mut items: Vec<(V, V)> = d.borrow().ents.iter().flatten().cloned().collect();
            if items.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            let id = json_mark(o, v)?;
            if o.sort_keys {
                let mut err = None;
                // sorted(dct.items()): keys first, then (equal keys) values
                let cmp = |a: &V, b: &V, err: &mut Option<E>| match lt(a, b) {
                    Ok(true) => Some(std::cmp::Ordering::Less),
                    Ok(false) => match lt(b, a) {
                        Ok(true) => Some(std::cmp::Ordering::Greater),
                        Ok(false) => None,
                        Err(e) => {
                            err.get_or_insert(e);
                            Some(std::cmp::Ordering::Equal)
                        }
                    },
                    Err(e) => {
                        err.get_or_insert(e);
                        Some(std::cmp::Ordering::Equal)
                    }
                };
                items.sort_by(|a, b| cmp(&a.0, &b.0, &mut err).unwrap_or_else(|| if eq(&a.0, &b.0).unwrap_or(false) { cmp(&a.1, &b.1, &mut err).unwrap_or(std::cmp::Ordering::Equal) } else { std::cmp::Ordering::Equal }));
                if let Some(e) = err {
                    return Err(e);
                }
            }
            let mut keyed = vec![];
            for (k, x) in items.drain(..) {
                let ks = match &k {
                    V::Str(x) => x.s.clone(),
                    V::Int(_) | V::Big(_) => repr(&k)?,
                    V::Float(f) => json_float(*f, o)?,
                    V::Bool(b) => (if *b { "true" } else { "false" }).into(),
                    V::None => "null".into(),
                    V::Ext(e) if matches!(e.native(), Some(Ok(V::Str(_)))) => match e.native() {
                        Some(Ok(V::Str(x))) => x.s.clone(),
                        _ => unreachable!(),
                    },
                    // a str or int subclass (StrEnum, IntEnum): its str or int
                    V::Obj(_) if matches!(inner_of(&k), Some(V::Str(_) | V::Int(_) | V::Big(_) | V::Float(_))) => match inner_of(&k).unwrap() {
                        V::Str(x) => x.s.clone(),
                        V::Float(f) => json_float(f, o)?,
                        i => repr(&i)?,
                    },
                    _ => {
                        if o.skipkeys {
                            continue;
                        }
                        return type_err(format!("keys must be str, int, float, bool or None, not {}", tn(&k)));
                    }
                };
                keyed.push((ks, k, x));
            }
            out.push('{');
            for (k, (ks, _, x)) in keyed.iter().enumerate() {
                if k > 0 {
                    out.push_str(&o.item_sep);
                }
                nl(out, depth + 1);
                json_str(ks, o.ascii, out);
                out.push_str(&o.key_sep);
                json_dump(x, o, depth + 1, out)?;
            }
            nl(out, depth);
            out.push('}');
            json_unmark(o, id);
        }
        V::Obj(ob) if inner_of(v).is_some() => {
            let _ = ob;
            let inn = inner_of(v).unwrap();
            if matches!(inn, V::Float(_)) && false {
                return Ok(());
            }
            json_dump(&inn, o, depth, out)?;
        }
        V::Ext(x) if x.native().is_some() => {
            let n = x.native().unwrap()?;
            if matches!(n, V::Ext(_)) {
                return type_err(format!("Object of type {} is not JSON serializable", tn(v)));
            }
            json_dump(&n, o, depth, out)?;
        }
        _ => match &o.default {
            Some(f) => {
                let id = json_mark(o, v)?;
                let r = call(f, Args::p(vec![v.clone()]))?;
                json_dump(&r, o, depth, out)?;
                json_unmark(o, id);
            }
            None => return type_err(format!("Object of type {} is not JSON serializable", tn(v))),
        },
    }
    Ok(())
}
// JSONEncoder.__init__'s options (keyword-only, as in CPython); unknown keywords are its TypeError.
fn json_opts(a: &mut Args) -> Result<JOpts, E> {
    if let Some(x) = a.p.get(1) {
        let _ = x;
        return type_err(format!("JSONEncoder.__init__() takes 1 positional argument but {} were given", a.p.len()));
    }
    let indent = match a.kw("indent") {
        None | Some(V::None) => None,
        Some(V::Str(x)) => Some(x.s.clone()),
        Some(V::Int(n)) => Some(" ".repeat(n.max(0) as usize)),
        Some(V::Bool(b)) => Some(" ".repeat(b as usize)),
        Some(x) => return type_err(format!("can't multiply sequence by non-int of type '{}'", tn(&x))),
    };
    let (mut item_sep, mut key_sep) = (if indent.is_some() { "," } else { ", " }.to_string(), ": ".to_string());
    if let Some(sep) = a.kw("separators") {
        if !matches!(sep, V::None) {
            let v = to_vec(&sep)?;
            if v.len() != 2 {
                return val_err(format!("{} values to unpack (expected 2)", if v.len() > 2 { "too many".to_string() } else { format!("not enough values to unpack (expected 2, got {})", v.len()) }));
            }
            item_sep = to_str(&v[0])?;
            key_sep = to_str(&v[1])?;
        }
    }
    let default = a.kw("default").filter(|d| !matches!(d, V::None));
    let mut flag = |n: &str, d: bool| -> Result<bool, E> { match a.kw(n) { Some(v) => truth(&v), None => Ok(d) } };
    let o = JOpts {
        indent,
        item_sep,
        key_sep,
        sort_keys: flag("sort_keys", false)?,
        ascii: flag("ensure_ascii", true)?,
        default,
        skipkeys: flag("skipkeys", false)?,
        allow_nan: flag("allow_nan", true)?,
        markers: if flag("check_circular", true)? { Some(RefCell::new(vec![])) } else { None },
    };
    a.no_kw("JSONEncoder.__init__")?;
    Ok(o)
}
// An encoder object's options, read when it encodes (as CPython's iterencode reads them).
fn json_enc_opts(enc: &V) -> Result<JOpts, E> {
    let g = |n: &str| getattr(enc, n);
    let indent = match g("indent")? {
        V::None => None,
        V::Str(x) => Some(x.s.clone()),
        V::Int(n) => Some(" ".repeat(n.max(0) as usize)),
        V::Bool(b) => Some(" ".repeat(b as usize)),
        x => return type_err(format!("can't multiply sequence by non-int of type '{}'", tn(&x))),
    };
    Ok(JOpts {
        indent,
        item_sep: to_str(&g("item_separator")?)?,
        key_sep: to_str(&g("key_separator")?)?,
        sort_keys: truth(&g("sort_keys")?)?,
        ascii: truth(&g("ensure_ascii")?)?,
        default: Some(g("default")?),
        skipkeys: truth(&g("skipkeys")?)?,
        allow_nan: truth(&g("allow_nan")?)?,
        markers: if truth(&g("check_circular")?)? { Some(RefCell::new(vec![])) } else { None },
    })
}
fn json_text(v: &V, o: &JOpts) -> Result<String, E> {
    let mut out = String::new();
    json_dump(v, o, 0, &mut out)?;
    Ok(out)
}
// dumps/dump's keywords for the encoder (cls), CPython's order
const JENC_KW: [&str; 8] = ["skipkeys", "ensure_ascii", "check_circular", "allow_nan", "indent", "separators", "default", "sort_keys"];
const JDEC_KW: [&str; 5] = ["object_hook", "parse_float", "parse_int", "parse_constant", "object_pairs_hook"];
// The text dumps(v, **a) makes: with cls, cls(**options).encode(v) (dump: the encoder's own options).
fn json_encode(v: &V, a: &mut Args, dump: bool) -> Result<String, E> {
    match a.kw("cls").filter(|c| !matches!(c, V::None)) {
        None => {
            let o = json_opts(a)?;
            json_text(v, &o)
        }
        Some(cls) => {
            let enc = call(&cls, Args { p: vec![], k: std::mem::take(&mut a.k) })?;
            if dump {
                json_text(v, &json_enc_opts(&enc)?)
            } else {
                to_str(&call_method(&enc, "encode", Args::p(vec![v.clone()]))?)
            }
        }
    }
}
// The text loads takes: str, or bytes in the encoding json.detect_encoding names.
fn json_source(v: &V) -> Result<String, E> {
    if let V::Str(x) = v {
        if x.s.starts_with('\u{feff}') {
            let c: Vec<char> = x.s.chars().collect();
            return json_err("Unexpected UTF-8 BOM (decode using utf-8-sig)", &c, 0);
        }
        return Ok(x.s.clone());
    }
    let Some(b) = bytes_of(v) else {
        return type_err(format!("the JSON object must be str, bytes or bytearray, not {}", tn(v)));
    };
    let utf = |b: &[u8], w: usize, be: bool| -> Result<String, E> {
        let units: Vec<u32> = b.chunks(w).map(|c| {
            let mut x = 0u32;
            for k in 0..c.len() {
                let byte = if be { c[k] } else { c[c.len() - 1 - k] } as u32;
                x = (x << 8) | byte;
            }
            x
        }).collect();
        if b.len() % w != 0 {
            return Err(unsupported("json: truncated utf-16/utf-32 bytes"));
        }
        let r: Option<String> = if w == 2 {
            char::decode_utf16(units.iter().map(|&u| u as u16)).collect::<Result<String, _>>().ok()
        } else {
            units.iter().map(|&u| char::from_u32(u)).collect()
        };
        r.ok_or_else(|| unsupported("json: invalid utf-16/utf-32 bytes"))
    };
    let st = |b: &[u8]| -> Result<String, E> { to_str(&bytes_decode(b, None, None)?) };
    let n = b.len();
    if b.starts_with(&[0, 0, 0xfe, 0xff]) {
        return utf(&b[4..], 4, true);
    }
    if b.starts_with(&[0xff, 0xfe, 0, 0]) {
        return utf(&b[4..], 4, false);
    }
    if b.starts_with(&[0xfe, 0xff]) {
        return utf(&b[2..], 2, true);
    }
    if b.starts_with(&[0xff, 0xfe]) {
        return utf(&b[2..], 2, false);
    }
    if b.starts_with(&[0xef, 0xbb, 0xbf]) {
        return st(&b[3..]);
    }
    if n >= 4 {
        if b[0] == 0 {
            return if b[1] != 0 { utf(&b, 2, true) } else { utf(&b, 4, true) };
        }
        if b[1] == 0 {
            return if b[2] != 0 || b[3] != 0 { utf(&b, 2, false) } else { utf(&b, 4, false) };
        }
    } else if n == 2 {
        if b[0] == 0 {
            return utf(&b, 2, true);
        }
        if b[1] == 0 {
            return utf(&b, 2, false);
        }
    }
    st(&b)
}
// loads(s, **kw): no options, the default decoder; otherwise (cls or JSONDecoder)(**kw).decode(s)
fn json_decode(t: &V, a: &mut Args) -> R {
    let cls = a.kw("cls").filter(|c| !matches!(c, V::None));
    let text = json_source(t)?;
    a.k.retain(|(n, v)| !(JDEC_KW.contains(n) && matches!(v, V::None)));
    if cls.is_none() && a.k.is_empty() {
        return json_loads(&text);
    }
    let dec = call(&cls.unwrap_or_else(|| V::Class(json_decoder_class())), Args { p: vec![], k: std::mem::take(&mut a.k) })?;
    call_method(&dec, "decode", Args::p(vec![st(text)]))
}
pub fn json_fn(n: &str, mut a: Args) -> R {
    match n {
        "loads" => {
            let kv = a.kw("s");
            let Some(t) = a.p.first().cloned().or(kv) else {
                return type_err("loads() missing 1 required positional argument: 's'".into());
            };
            if a.p.len() > 1 {
                return type_err(format!("loads() takes 1 positional argument but {} were given", a.p.len()));
            }
            json_decode(&t, &mut a)
        }
        "load" => {
            let f = a.p.first().cloned().or_else(|| a.kw("fp")).ok_or_else(|| exc("TypeError", "load() missing 1 required positional argument: 'fp'".into()))?;
            let t = match &f {
                V::Module(&ModName("sys.stdin")) => st(read_all()),
                _ => call_method(&f, "read", Args::p(vec![]))?,
            };
            json_decode(&t, &mut a)
        }
        "dumps" => {
            let v = a.p.first().cloned().or_else(|| a.kw("obj")).ok_or_else(|| exc("TypeError", "dumps() missing 1 required positional argument: 'obj'".into()))?;
            if a.p.len() > 1 {
                return type_err(format!("dumps() takes 1 positional argument but {} were given", a.p.len()));
            }
            a.p.clear();
            Ok(st(json_encode(&v, &mut a, false)?))
        }
        "dump" => {
            let v = a.p.first().cloned().or_else(|| a.kw("obj")).unwrap_or(V::None);
            let f = a.p.get(1).cloned().or_else(|| a.kw("fp")).unwrap_or(V::None);
            if a.p.len() > 2 {
                return type_err(format!("dump() takes 2 positional arguments but {} were given", a.p.len()));
            }
            a.p.clear();
            let out = json_encode(&v, &mut a, true)?;
            match &f {
                V::Module(&ModName("sys.stdout")) => out_write(&out),
                V::Module(&ModName("sys.stderr")) => {
                    out_flush();
                    err_write(&out)
                }
                _ => {
                    call_method(&f, "write", Args::p(vec![st(out)]))?;
                }
            }
            Ok(V::None)
        }
        _ => Err(unsupported(&format!("json.{n}"))),
    }
}

// json.decoder.scanstring(s, end): the string whose opening quote is at end - 1, and the index after its
// closing quote. json.JSONDecoder().raw_decode(s, idx): the value starting exactly at idx (no whitespace
// skipped, as in CPython) and the index after it; .decode(s) is loads.
fn json_index(a: &Args, k: usize, n: usize) -> Result<usize, E> {
    match a.p.get(k) {
        None => Ok(0),
        Some(V::Int(i)) if *i >= 0 && (*i as usize) <= n => Ok(*i as usize),
        Some(V::Int(_)) => Ok(usize::MAX),
        Some(x) => type_err(format!("an integer is required, not {}", tn(x))),
    }
}
pub fn json_scanstring(a: Args) -> R {
    let t = match a.p.first() {
        Some(V::Str(x)) => x.s.clone(),
        _ => return type_err("first argument must be a string".into()),
    };
    let c: Vec<char> = t.chars().collect();
    let end = json_index(&a, 1, c.len())?;
    if end == 0 || end == usize::MAX {
        return val_err("end is out of bounds".into());
    }
    let mut p = JP { s: &c, i: end - 1, h: None };
    let v = p.string()?;
    Ok(tup(vec![v, V::Int(p.i as i64)]))
}
thread_local! {
    static JDEC: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static JENC: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
// The hooks a decoder's __init__ took (CPython's scanner copies them then, so later attribute changes do not count).
fn json_hooks(dec: &V) -> Result<JHooks, E> {
    let V::Obj(o) = dec else { return type_err("JSONDecoder method on a non-decoder".into()) };
    let Some(V::Tuple(t)) = o.get("__bv_scan__") else {
        return Err(exc("AttributeError", format!("'{}' object has no attribute 'scan_once'", tn(dec))));
    };
    let opt = |v: &V| if matches!(v, V::None) { None } else { Some(v.clone()) };
    Ok(JHooks { object_hook: opt(&t[0]), parse_float: opt(&t[1]), parse_int: opt(&t[2]), parse_constant: opt(&t[3]), pairs_hook: opt(&t[5]), lax: !truth(&t[4])? })
}
pub fn json_decoder_class() -> Rc<Class> {
    JDEC.with(|j| {
        let mut j = j.borrow_mut();
        if j.is_none() {
            let c = Class::new("JSONDecoder", "json.decoder", vec![bclass("object")], BT::Other);
            c.set("__init__", builtin("json.__init__", |mut a: Args| {
                if a.p.len() > 1 {
                    return type_err(format!("JSONDecoder.__init__() takes 1 positional argument but {} were given", a.p.len()));
                }
                let me = a.p[0].clone();
                let mut vals = vec![];
                for n in ["object_hook", "parse_float", "parse_int", "parse_constant", "strict", "object_pairs_hook"] {
                    let v = a.kw(n).unwrap_or(if n == "strict" { V::Bool(true) } else { V::None });
                    vals.push(v);
                }
                a.no_kw("JSONDecoder.__init__")?;
                if let V::Obj(o) = &me {
                    for (n, v) in ["object_hook", "parse_float", "parse_int", "parse_constant", "strict", "object_pairs_hook"].iter().zip(&vals) {
                        let v = match (*n, v) {
                            ("parse_float", V::None) => V::Class(bclass("float")),
                            ("parse_int", V::None) => V::Class(bclass("int")),
                            _ => v.clone(),
                        };
                        o.set(n, v);
                    }
                    o.set("__bv_scan__", tup(vals));
                }
                Ok(V::None)
            }));
            c.set("raw_decode", builtin("raw_decode", |mut a: Args| {
                let idx_kw = a.kw("idx");
                let t = match a.p.get(1) {
                    Some(V::Str(x)) => x.s.clone(),
                    Some(x) => return type_err(format!("first argument must be a string, not {}", tn(x))),
                    None => return type_err("JSONDecoder.raw_decode() missing 1 required positional argument: 's'".into()),
                };
                let h = json_hooks(&a.p[0])?;
                let c: Vec<char> = t.chars().collect();
                let mut ia = Args::p(a.p[1..].to_vec());
                if let Some(i) = idx_kw {
                    ia.p.truncate(1);
                    ia.p.push(i);
                }
                let idx = json_index(&ia, 1, c.len())?;
                if idx == usize::MAX || idx >= c.len() || matches!(c[idx], ' ' | '\t' | '\n' | '\r') {
                    return json_err("Expecting value", &c, idx.min(c.len()));
                }
                let mut p = JP { s: &c, i: idx, h: Some(&h) };
                let v = p.value()?;
                Ok(tup(vec![v, V::Int(p.i as i64)]))
            }));
            // decode(s): raw_decode after leading whitespace, then only whitespace (CPython's decode)
            c.set("decode", builtin("decode", |a: Args| {
                let me = a.p[0].clone();
                let t = match a.p.get(1) {
                    Some(V::Str(x)) => x.s.clone(),
                    Some(x) => return type_err(format!("expected string or bytes-like object, got '{}'", tn(x))),
                    None => return type_err("JSONDecoder.decode() missing 1 required positional argument: 's'".into()),
                };
                let c: Vec<char> = t.chars().collect();
                let ws = |mut i: usize| {
                    while i < c.len() && matches!(c[i], ' ' | '\t' | '\n' | '\r') {
                        i += 1;
                    }
                    i
                };
                let start = ws(0);
                let mut ra = Args::p(vec![st(t.clone())]);
                ra.k.push(("idx", V::Int(start as i64)));
                let r = call_method(&me, "raw_decode", ra)?;
                let r = to_vec(&r)?;
                let end = match r.get(1) {
                    Some(V::Int(e)) => *e as usize,
                    _ => return type_err("raw_decode() must return (obj, end)".into()),
                };
                let end = ws(end.min(c.len()));
                if end != c.len() {
                    return json_err("Extra data", &c, end);
                }
                Ok(r[0].clone())
            }));
            *j = Some(c);
        }
        j.clone().unwrap()
    })
}
// json.JSONEncoder: __init__ keeps the options as attributes; encode(o) reads them (and self.default).
pub fn json_encoder_class() -> Rc<Class> {
    JENC.with(|j| {
        let mut j = j.borrow_mut();
        if j.is_none() {
            let c = Class::new("JSONEncoder", "json.encoder", vec![bclass("object")], BT::Other);
            c.set("item_separator", s(", "));
            c.set("key_separator", s(": "));
            c.set("__init__", builtin("json.__init__", |mut a: Args| {
                let me = a.p[0].clone();
                let raw_indent = a.k.iter().find(|(n, _)| *n == "indent").map(|x| x.1.clone()).unwrap_or(V::None);
                let has_default = a.k.iter().any(|(n, v)| *n == "default" && !matches!(v, V::None));
                let o = json_opts(&mut a)?;
                if let V::Obj(ob) = &me {
                    ob.set("skipkeys", V::Bool(o.skipkeys));
                    ob.set("ensure_ascii", V::Bool(o.ascii));
                    ob.set("check_circular", V::Bool(o.markers.is_some()));
                    ob.set("allow_nan", V::Bool(o.allow_nan));
                    ob.set("sort_keys", V::Bool(o.sort_keys));
                    ob.set("indent", raw_indent.clone());
                    if o.indent.is_some() || o.item_sep != ", " || o.key_sep != ": " {
                        ob.set("item_separator", st(o.item_sep.clone()));
                        ob.set("key_separator", st(o.key_sep.clone()));
                    }
                    if has_default {
                        ob.set("default", o.default.clone().unwrap());
                    }
                }
                Ok(V::None)
            }));
            c.set("default", builtin("default", |a: Args| {
                let o = a.p.get(1).cloned().unwrap_or(V::None);
                type_err(format!("Object of type {} is not JSON serializable", tn(&o)))
            }));
            c.set("encode", builtin("encode", |a: Args| {
                let me = a.p[0].clone();
                let v = a.p.get(1).cloned().ok_or_else(|| exc("TypeError", "JSONEncoder.encode() missing 1 required positional argument: 'o'".into()))?;
                let o = json_enc_opts(&me)?;
                Ok(st(json_text(&v, &o)?))
            }));
            *j = Some(c);
        }
        j.clone().unwrap()
    })
}

// Unicode category Nd: the decimal digits of every script
pub fn is_nd(c: char) -> bool {
    tab_has(PY_ND, c)
}

// Unicode categories L and N (Python \w) (non-ASCII, from CPython 3.13.13's unicodedata 15.1.0)
const PY_LN: &[(u32, u32)] = &[(0xaa,0xaa),(0xb2,0xb3),(0xb5,0xb5),(0xb9,0xba),(0xbc,0xbe),(0xc0,0xd6),(0xd8,0xf6),(0xf8,0x2c1),(0x2c6,0x2d1),(0x2e0,0x2e4),(0x2ec,0x2ec),(0x2ee,0x2ee),(0x370,0x374),(0x376,0x377),(0x37a,0x37d),(0x37f,0x37f),(0x386,0x386),(0x388,0x38a),(0x38c,0x38c),(0x38e,0x3a1),(0x3a3,0x3f5),(0x3f7,0x481),(0x48a,0x52f),(0x531,0x556),(0x559,0x559),(0x560,0x588),(0x5d0,0x5ea),(0x5ef,0x5f2),(0x620,0x64a),(0x660,0x669),(0x66e,0x66f),(0x671,0x6d3),(0x6d5,0x6d5),(0x6e5,0x6e6),(0x6ee,0x6fc),(0x6ff,0x6ff),(0x710,0x710),(0x712,0x72f),(0x74d,0x7a5),(0x7b1,0x7b1),(0x7c0,0x7ea),(0x7f4,0x7f5),(0x7fa,0x7fa),(0x800,0x815),(0x81a,0x81a),(0x824,0x824),(0x828,0x828),(0x840,0x858),(0x860,0x86a),(0x870,0x887),(0x889,0x88e),(0x8a0,0x8c9),(0x904,0x939),(0x93d,0x93d),(0x950,0x950),(0x958,0x961),(0x966,0x96f),(0x971,0x980),(0x985,0x98c),(0x98f,0x990),(0x993,0x9a8),(0x9aa,0x9b0),(0x9b2,0x9b2),(0x9b6,0x9b9),(0x9bd,0x9bd),(0x9ce,0x9ce),(0x9dc,0x9dd),(0x9df,0x9e1),(0x9e6,0x9f1),(0x9f4,0x9f9),(0x9fc,0x9fc),(0xa05,0xa0a),(0xa0f,0xa10),(0xa13,0xa28),(0xa2a,0xa30),(0xa32,0xa33),(0xa35,0xa36),(0xa38,0xa39),(0xa59,0xa5c),(0xa5e,0xa5e),(0xa66,0xa6f),(0xa72,0xa74),(0xa85,0xa8d),(0xa8f,0xa91),(0xa93,0xaa8),(0xaaa,0xab0),(0xab2,0xab3),(0xab5,0xab9),(0xabd,0xabd),(0xad0,0xad0),(0xae0,0xae1),(0xae6,0xaef),(0xaf9,0xaf9),(0xb05,0xb0c),(0xb0f,0xb10),(0xb13,0xb28),(0xb2a,0xb30),(0xb32,0xb33),(0xb35,0xb39),(0xb3d,0xb3d),(0xb5c,0xb5d),(0xb5f,0xb61),(0xb66,0xb6f),(0xb71,0xb77),(0xb83,0xb83),(0xb85,0xb8a),(0xb8e,0xb90),(0xb92,0xb95),(0xb99,0xb9a),(0xb9c,0xb9c),(0xb9e,0xb9f),(0xba3,0xba4),(0xba8,0xbaa),(0xbae,0xbb9),(0xbd0,0xbd0),(0xbe6,0xbf2),(0xc05,0xc0c),(0xc0e,0xc10),(0xc12,0xc28),(0xc2a,0xc39),(0xc3d,0xc3d),(0xc58,0xc5a),(0xc5d,0xc5d),(0xc60,0xc61),(0xc66,0xc6f),(0xc78,0xc7e),(0xc80,0xc80),(0xc85,0xc8c),(0xc8e,0xc90),(0xc92,0xca8),(0xcaa,0xcb3),(0xcb5,0xcb9),(0xcbd,0xcbd),(0xcdd,0xcde),(0xce0,0xce1),(0xce6,0xcef),(0xcf1,0xcf2),(0xd04,0xd0c),(0xd0e,0xd10),(0xd12,0xd3a),(0xd3d,0xd3d),(0xd4e,0xd4e),(0xd54,0xd56),(0xd58,0xd61),(0xd66,0xd78),(0xd7a,0xd7f),(0xd85,0xd96),(0xd9a,0xdb1),(0xdb3,0xdbb),(0xdbd,0xdbd),(0xdc0,0xdc6),(0xde6,0xdef),(0xe01,0xe30),(0xe32,0xe33),(0xe40,0xe46),(0xe50,0xe59),(0xe81,0xe82),(0xe84,0xe84),(0xe86,0xe8a),(0xe8c,0xea3),(0xea5,0xea5),(0xea7,0xeb0),(0xeb2,0xeb3),(0xebd,0xebd),(0xec0,0xec4),(0xec6,0xec6),(0xed0,0xed9),(0xedc,0xedf),(0xf00,0xf00),(0xf20,0xf33),(0xf40,0xf47),(0xf49,0xf6c),(0xf88,0xf8c),(0x1000,0x102a),(0x103f,0x1049),(0x1050,0x1055),(0x105a,0x105d),(0x1061,0x1061),(0x1065,0x1066),(0x106e,0x1070),(0x1075,0x1081),(0x108e,0x108e),(0x1090,0x1099),(0x10a0,0x10c5),(0x10c7,0x10c7),(0x10cd,0x10cd),(0x10d0,0x10fa),(0x10fc,0x1248),(0x124a,0x124d),(0x1250,0x1256),(0x1258,0x1258),(0x125a,0x125d),(0x1260,0x1288),(0x128a,0x128d),(0x1290,0x12b0),(0x12b2,0x12b5),(0x12b8,0x12be),(0x12c0,0x12c0),(0x12c2,0x12c5),(0x12c8,0x12d6),(0x12d8,0x1310),(0x1312,0x1315),(0x1318,0x135a),(0x1369,0x137c),(0x1380,0x138f),(0x13a0,0x13f5),(0x13f8,0x13fd),(0x1401,0x166c),(0x166f,0x167f),(0x1681,0x169a),(0x16a0,0x16ea),(0x16ee,0x16f8),(0x1700,0x1711),(0x171f,0x1731),(0x1740,0x1751),(0x1760,0x176c),(0x176e,0x1770),(0x1780,0x17b3),(0x17d7,0x17d7),(0x17dc,0x17dc),(0x17e0,0x17e9),(0x17f0,0x17f9),(0x1810,0x1819),(0x1820,0x1878),(0x1880,0x1884),(0x1887,0x18a8),(0x18aa,0x18aa),(0x18b0,0x18f5),(0x1900,0x191e),(0x1946,0x196d),(0x1970,0x1974),(0x1980,0x19ab),(0x19b0,0x19c9),(0x19d0,0x19da),(0x1a00,0x1a16),(0x1a20,0x1a54),(0x1a80,0x1a89),(0x1a90,0x1a99),(0x1aa7,0x1aa7),(0x1b05,0x1b33),(0x1b45,0x1b4c),(0x1b50,0x1b59),(0x1b83,0x1ba0),(0x1bae,0x1be5),(0x1c00,0x1c23),(0x1c40,0x1c49),(0x1c4d,0x1c7d),(0x1c80,0x1c88),(0x1c90,0x1cba),(0x1cbd,0x1cbf),(0x1ce9,0x1cec),(0x1cee,0x1cf3),(0x1cf5,0x1cf6),(0x1cfa,0x1cfa),(0x1d00,0x1dbf),(0x1e00,0x1f15),(0x1f18,0x1f1d),(0x1f20,0x1f45),(0x1f48,0x1f4d),(0x1f50,0x1f57),(0x1f59,0x1f59),(0x1f5b,0x1f5b),(0x1f5d,0x1f5d),(0x1f5f,0x1f7d),(0x1f80,0x1fb4),(0x1fb6,0x1fbc),(0x1fbe,0x1fbe),(0x1fc2,0x1fc4),(0x1fc6,0x1fcc),(0x1fd0,0x1fd3),(0x1fd6,0x1fdb),(0x1fe0,0x1fec),(0x1ff2,0x1ff4),(0x1ff6,0x1ffc),(0x2070,0x2071),(0x2074,0x2079),(0x207f,0x2089),(0x2090,0x209c),(0x2102,0x2102),(0x2107,0x2107),(0x210a,0x2113),(0x2115,0x2115),(0x2119,0x211d),(0x2124,0x2124),(0x2126,0x2126),(0x2128,0x2128),(0x212a,0x212d),(0x212f,0x2139),(0x213c,0x213f),(0x2145,0x2149),(0x214e,0x214e),(0x2150,0x2189),(0x2460,0x249b),(0x24ea,0x24ff),(0x2776,0x2793),(0x2c00,0x2ce4),(0x2ceb,0x2cee),(0x2cf2,0x2cf3),(0x2cfd,0x2cfd),(0x2d00,0x2d25),(0x2d27,0x2d27),(0x2d2d,0x2d2d),(0x2d30,0x2d67),(0x2d6f,0x2d6f),(0x2d80,0x2d96),(0x2da0,0x2da6),(0x2da8,0x2dae),(0x2db0,0x2db6),(0x2db8,0x2dbe),(0x2dc0,0x2dc6),(0x2dc8,0x2dce),(0x2dd0,0x2dd6),(0x2dd8,0x2dde),(0x2e2f,0x2e2f),(0x3005,0x3007),(0x3021,0x3029),(0x3031,0x3035),(0x3038,0x303c),(0x3041,0x3096),(0x309d,0x309f),(0x30a1,0x30fa),(0x30fc,0x30ff),(0x3105,0x312f),(0x3131,0x318e),(0x3192,0x3195),(0x31a0,0x31bf),(0x31f0,0x31ff),(0x3220,0x3229),(0x3248,0x324f),(0x3251,0x325f),(0x3280,0x3289),(0x32b1,0x32bf),(0x3400,0x4dbf),(0x4e00,0xa48c),(0xa4d0,0xa4fd),(0xa500,0xa60c),(0xa610,0xa62b),(0xa640,0xa66e),(0xa67f,0xa69d),(0xa6a0,0xa6ef),(0xa717,0xa71f),(0xa722,0xa788),(0xa78b,0xa7ca),(0xa7d0,0xa7d1),(0xa7d3,0xa7d3),(0xa7d5,0xa7d9),(0xa7f2,0xa801),(0xa803,0xa805),(0xa807,0xa80a),(0xa80c,0xa822),(0xa830,0xa835),(0xa840,0xa873),(0xa882,0xa8b3),(0xa8d0,0xa8d9),(0xa8f2,0xa8f7),(0xa8fb,0xa8fb),(0xa8fd,0xa8fe),(0xa900,0xa925),(0xa930,0xa946),(0xa960,0xa97c),(0xa984,0xa9b2),(0xa9cf,0xa9d9),(0xa9e0,0xa9e4),(0xa9e6,0xa9fe),(0xaa00,0xaa28),(0xaa40,0xaa42),(0xaa44,0xaa4b),(0xaa50,0xaa59),(0xaa60,0xaa76),(0xaa7a,0xaa7a),(0xaa7e,0xaaaf),(0xaab1,0xaab1),(0xaab5,0xaab6),(0xaab9,0xaabd),(0xaac0,0xaac0),(0xaac2,0xaac2),(0xaadb,0xaadd),(0xaae0,0xaaea),(0xaaf2,0xaaf4),(0xab01,0xab06),(0xab09,0xab0e),(0xab11,0xab16),(0xab20,0xab26),(0xab28,0xab2e),(0xab30,0xab5a),(0xab5c,0xab69),(0xab70,0xabe2),(0xabf0,0xabf9),(0xac00,0xd7a3),(0xd7b0,0xd7c6),(0xd7cb,0xd7fb),(0xf900,0xfa6d),(0xfa70,0xfad9),(0xfb00,0xfb06),(0xfb13,0xfb17),(0xfb1d,0xfb1d),(0xfb1f,0xfb28),(0xfb2a,0xfb36),(0xfb38,0xfb3c),(0xfb3e,0xfb3e),(0xfb40,0xfb41),(0xfb43,0xfb44),(0xfb46,0xfbb1),(0xfbd3,0xfd3d),(0xfd50,0xfd8f),(0xfd92,0xfdc7),(0xfdf0,0xfdfb),(0xfe70,0xfe74),(0xfe76,0xfefc),(0xff10,0xff19),(0xff21,0xff3a),(0xff41,0xff5a),(0xff66,0xffbe),(0xffc2,0xffc7),(0xffca,0xffcf),(0xffd2,0xffd7),(0xffda,0xffdc),(0x10000,0x1000b),(0x1000d,0x10026),(0x10028,0x1003a),(0x1003c,0x1003d),(0x1003f,0x1004d),(0x10050,0x1005d),(0x10080,0x100fa),(0x10107,0x10133),(0x10140,0x10178),(0x1018a,0x1018b),(0x10280,0x1029c),(0x102a0,0x102d0),(0x102e1,0x102fb),(0x10300,0x10323),(0x1032d,0x1034a),(0x10350,0x10375),(0x10380,0x1039d),(0x103a0,0x103c3),(0x103c8,0x103cf),(0x103d1,0x103d5),(0x10400,0x1049d),(0x104a0,0x104a9),(0x104b0,0x104d3),(0x104d8,0x104fb),(0x10500,0x10527),(0x10530,0x10563),(0x10570,0x1057a),(0x1057c,0x1058a),(0x1058c,0x10592),(0x10594,0x10595),(0x10597,0x105a1),(0x105a3,0x105b1),(0x105b3,0x105b9),(0x105bb,0x105bc),(0x10600,0x10736),(0x10740,0x10755),(0x10760,0x10767),(0x10780,0x10785),(0x10787,0x107b0),(0x107b2,0x107ba),(0x10800,0x10805),(0x10808,0x10808),(0x1080a,0x10835),(0x10837,0x10838),(0x1083c,0x1083c),(0x1083f,0x10855),(0x10858,0x10876),(0x10879,0x1089e),(0x108a7,0x108af),(0x108e0,0x108f2),(0x108f4,0x108f5),(0x108fb,0x1091b),(0x10920,0x10939),(0x10980,0x109b7),(0x109bc,0x109cf),(0x109d2,0x10a00),(0x10a10,0x10a13),(0x10a15,0x10a17),(0x10a19,0x10a35),(0x10a40,0x10a48),(0x10a60,0x10a7e),(0x10a80,0x10a9f),(0x10ac0,0x10ac7),(0x10ac9,0x10ae4),(0x10aeb,0x10aef),(0x10b00,0x10b35),(0x10b40,0x10b55),(0x10b58,0x10b72),(0x10b78,0x10b91),(0x10ba9,0x10baf),(0x10c00,0x10c48),(0x10c80,0x10cb2),(0x10cc0,0x10cf2),(0x10cfa,0x10d23),(0x10d30,0x10d39),(0x10e60,0x10e7e),(0x10e80,0x10ea9),(0x10eb0,0x10eb1),(0x10f00,0x10f27),(0x10f30,0x10f45),(0x10f51,0x10f54),(0x10f70,0x10f81),(0x10fb0,0x10fcb),(0x10fe0,0x10ff6),(0x11003,0x11037),(0x11052,0x1106f),(0x11071,0x11072),(0x11075,0x11075),(0x11083,0x110af),(0x110d0,0x110e8),(0x110f0,0x110f9),(0x11103,0x11126),(0x11136,0x1113f),(0x11144,0x11144),(0x11147,0x11147),(0x11150,0x11172),(0x11176,0x11176),(0x11183,0x111b2),(0x111c1,0x111c4),(0x111d0,0x111da),(0x111dc,0x111dc),(0x111e1,0x111f4),(0x11200,0x11211),(0x11213,0x1122b),(0x1123f,0x11240),(0x11280,0x11286),(0x11288,0x11288),(0x1128a,0x1128d),(0x1128f,0x1129d),(0x1129f,0x112a8),(0x112b0,0x112de),(0x112f0,0x112f9),(0x11305,0x1130c),(0x1130f,0x11310),(0x11313,0x11328),(0x1132a,0x11330),(0x11332,0x11333),(0x11335,0x11339),(0x1133d,0x1133d),(0x11350,0x11350),(0x1135d,0x11361),(0x11400,0x11434),(0x11447,0x1144a),(0x11450,0x11459),(0x1145f,0x11461),(0x11480,0x114af),(0x114c4,0x114c5),(0x114c7,0x114c7),(0x114d0,0x114d9),(0x11580,0x115ae),(0x115d8,0x115db),(0x11600,0x1162f),(0x11644,0x11644),(0x11650,0x11659),(0x11680,0x116aa),(0x116b8,0x116b8),(0x116c0,0x116c9),(0x11700,0x1171a),(0x11730,0x1173b),(0x11740,0x11746),(0x11800,0x1182b),(0x118a0,0x118f2),(0x118ff,0x11906),(0x11909,0x11909),(0x1190c,0x11913),(0x11915,0x11916),(0x11918,0x1192f),(0x1193f,0x1193f),(0x11941,0x11941),(0x11950,0x11959),(0x119a0,0x119a7),(0x119aa,0x119d0),(0x119e1,0x119e1),(0x119e3,0x119e3),(0x11a00,0x11a00),(0x11a0b,0x11a32),(0x11a3a,0x11a3a),(0x11a50,0x11a50),(0x11a5c,0x11a89),(0x11a9d,0x11a9d),(0x11ab0,0x11af8),(0x11c00,0x11c08),(0x11c0a,0x11c2e),(0x11c40,0x11c40),(0x11c50,0x11c6c),(0x11c72,0x11c8f),(0x11d00,0x11d06),(0x11d08,0x11d09),(0x11d0b,0x11d30),(0x11d46,0x11d46),(0x11d50,0x11d59),(0x11d60,0x11d65),(0x11d67,0x11d68),(0x11d6a,0x11d89),(0x11d98,0x11d98),(0x11da0,0x11da9),(0x11ee0,0x11ef2),(0x11f02,0x11f02),(0x11f04,0x11f10),(0x11f12,0x11f33),(0x11f50,0x11f59),(0x11fb0,0x11fb0),(0x11fc0,0x11fd4),(0x12000,0x12399),(0x12400,0x1246e),(0x12480,0x12543),(0x12f90,0x12ff0),(0x13000,0x1342f),(0x13441,0x13446),(0x14400,0x14646),(0x16800,0x16a38),(0x16a40,0x16a5e),(0x16a60,0x16a69),(0x16a70,0x16abe),(0x16ac0,0x16ac9),(0x16ad0,0x16aed),(0x16b00,0x16b2f),(0x16b40,0x16b43),(0x16b50,0x16b59),(0x16b5b,0x16b61),(0x16b63,0x16b77),(0x16b7d,0x16b8f),(0x16e40,0x16e96),(0x16f00,0x16f4a),(0x16f50,0x16f50),(0x16f93,0x16f9f),(0x16fe0,0x16fe1),(0x16fe3,0x16fe3),(0x17000,0x187f7),(0x18800,0x18cd5),(0x18d00,0x18d08),(0x1aff0,0x1aff3),(0x1aff5,0x1affb),(0x1affd,0x1affe),(0x1b000,0x1b122),(0x1b132,0x1b132),(0x1b150,0x1b152),(0x1b155,0x1b155),(0x1b164,0x1b167),(0x1b170,0x1b2fb),(0x1bc00,0x1bc6a),(0x1bc70,0x1bc7c),(0x1bc80,0x1bc88),(0x1bc90,0x1bc99),(0x1d2c0,0x1d2d3),(0x1d2e0,0x1d2f3),(0x1d360,0x1d378),(0x1d400,0x1d454),(0x1d456,0x1d49c),(0x1d49e,0x1d49f),(0x1d4a2,0x1d4a2),(0x1d4a5,0x1d4a6),(0x1d4a9,0x1d4ac),(0x1d4ae,0x1d4b9),(0x1d4bb,0x1d4bb),(0x1d4bd,0x1d4c3),(0x1d4c5,0x1d505),(0x1d507,0x1d50a),(0x1d50d,0x1d514),(0x1d516,0x1d51c),(0x1d51e,0x1d539),(0x1d53b,0x1d53e),(0x1d540,0x1d544),(0x1d546,0x1d546),(0x1d54a,0x1d550),(0x1d552,0x1d6a5),(0x1d6a8,0x1d6c0),(0x1d6c2,0x1d6da),(0x1d6dc,0x1d6fa),(0x1d6fc,0x1d714),(0x1d716,0x1d734),(0x1d736,0x1d74e),(0x1d750,0x1d76e),(0x1d770,0x1d788),(0x1d78a,0x1d7a8),(0x1d7aa,0x1d7c2),(0x1d7c4,0x1d7cb),(0x1d7ce,0x1d7ff),(0x1df00,0x1df1e),(0x1df25,0x1df2a),(0x1e030,0x1e06d),(0x1e100,0x1e12c),(0x1e137,0x1e13d),(0x1e140,0x1e149),(0x1e14e,0x1e14e),(0x1e290,0x1e2ad),(0x1e2c0,0x1e2eb),(0x1e2f0,0x1e2f9),(0x1e4d0,0x1e4eb),(0x1e4f0,0x1e4f9),(0x1e7e0,0x1e7e6),(0x1e7e8,0x1e7eb),(0x1e7ed,0x1e7ee),(0x1e7f0,0x1e7fe),(0x1e800,0x1e8c4),(0x1e8c7,0x1e8cf),(0x1e900,0x1e943),(0x1e94b,0x1e94b),(0x1e950,0x1e959),(0x1ec71,0x1ecab),(0x1ecad,0x1ecaf),(0x1ecb1,0x1ecb4),(0x1ed01,0x1ed2d),(0x1ed2f,0x1ed3d),(0x1ee00,0x1ee03),(0x1ee05,0x1ee1f),(0x1ee21,0x1ee22),(0x1ee24,0x1ee24),(0x1ee27,0x1ee27),(0x1ee29,0x1ee32),(0x1ee34,0x1ee37),(0x1ee39,0x1ee39),(0x1ee3b,0x1ee3b),(0x1ee42,0x1ee42),(0x1ee47,0x1ee47),(0x1ee49,0x1ee49),(0x1ee4b,0x1ee4b),(0x1ee4d,0x1ee4f),(0x1ee51,0x1ee52),(0x1ee54,0x1ee54),(0x1ee57,0x1ee57),(0x1ee59,0x1ee59),(0x1ee5b,0x1ee5b),(0x1ee5d,0x1ee5d),(0x1ee5f,0x1ee5f),(0x1ee61,0x1ee62),(0x1ee64,0x1ee64),(0x1ee67,0x1ee6a),(0x1ee6c,0x1ee72),(0x1ee74,0x1ee77),(0x1ee79,0x1ee7c),(0x1ee7e,0x1ee7e),(0x1ee80,0x1ee89),(0x1ee8b,0x1ee9b),(0x1eea1,0x1eea3),(0x1eea5,0x1eea9),(0x1eeab,0x1eebb),(0x1f100,0x1f10c),(0x1fbf0,0x1fbf9),(0x20000,0x2a6df),(0x2a700,0x2b739),(0x2b740,0x2b81d),(0x2b820,0x2cea1),(0x2ceb0,0x2ebe0),(0x2ebf0,0x2ee5d),(0x2f800,0x2fa1d),(0x30000,0x3134a),(0x31350,0x323af)];

// Unicode category Nd (Python \d) (non-ASCII, from CPython 3.13.13's unicodedata 15.1.0)
const PY_ND: &[(u32, u32)] = &[(0x660,0x669),(0x6f0,0x6f9),(0x7c0,0x7c9),(0x966,0x96f),(0x9e6,0x9ef),(0xa66,0xa6f),(0xae6,0xaef),(0xb66,0xb6f),(0xbe6,0xbef),(0xc66,0xc6f),(0xce6,0xcef),(0xd66,0xd6f),(0xde6,0xdef),(0xe50,0xe59),(0xed0,0xed9),(0xf20,0xf29),(0x1040,0x1049),(0x1090,0x1099),(0x17e0,0x17e9),(0x1810,0x1819),(0x1946,0x194f),(0x19d0,0x19d9),(0x1a80,0x1a89),(0x1a90,0x1a99),(0x1b50,0x1b59),(0x1bb0,0x1bb9),(0x1c40,0x1c49),(0x1c50,0x1c59),(0xa620,0xa629),(0xa8d0,0xa8d9),(0xa900,0xa909),(0xa9d0,0xa9d9),(0xa9f0,0xa9f9),(0xaa50,0xaa59),(0xabf0,0xabf9),(0xff10,0xff19),(0x104a0,0x104a9),(0x10d30,0x10d39),(0x11066,0x1106f),(0x110f0,0x110f9),(0x11136,0x1113f),(0x111d0,0x111d9),(0x112f0,0x112f9),(0x11450,0x11459),(0x114d0,0x114d9),(0x11650,0x11659),(0x116c0,0x116c9),(0x11730,0x11739),(0x118e0,0x118e9),(0x11950,0x11959),(0x11c50,0x11c59),(0x11d50,0x11d59),(0x11da0,0x11da9),(0x11f50,0x11f59),(0x16a60,0x16a69),(0x16ac0,0x16ac9),(0x16b50,0x16b59),(0x1d7ce,0x1d7ff),(0x1e140,0x1e149),(0x1e2f0,0x1e2f9),(0x1e4f0,0x1e4f9),(0x1e950,0x1e959),(0x1fbf0,0x1fbf9)];
