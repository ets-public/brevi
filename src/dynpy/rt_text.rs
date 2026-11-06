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
        c.is_alphanumeric() || c == '_'
    }
}
fn is_digit(c: char, ascii: bool) -> bool {
    c.is_ascii_digit() || (!ascii && !c.is_ascii() && c.is_numeric())
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
            if cnt < hi && self.seq(one, pos, &mut |c: &mut Ctx, p| p != pos && c.rep(node, lo, hi, greedy, cnt + 1, p, pos, rest, k)) {
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
impl RMatch {
    fn group_idx(&self, g: &V) -> Result<usize, E> {
        match g {
            V::Int(i) if *i >= 0 && (*i as usize) <= self.prog.ngroups => Ok(*i as usize),
            V::Bool(b) => Ok(*b as usize),
            V::Str(n) => self.prog.names.iter().find(|x| x.0 == n.s).map(|x| x.1).ok_or_else(|| exc("IndexError", "no such group".into())),
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
            "re" => V::Ext(Rc::new(RPat { prog: self.prog.clone() })),
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
        format!("re.compile({})", str_repr(&self.prog.src))
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
            "flags" => V::Int(self.prog.flags | 32),
            "groups" => V::Int(self.prog.ngroups as i64),
            "groupindex" => {
                let mut d = Dict::new();
                for (nm, g) in &self.prog.names {
                    if let Err(e) = d.set(s(nm), V::Int(*g as i64)) {
                        return Some(Err(e));
                    }
                }
                V::Dict(Rc::new(RefCell::new(d)))
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
    V::Ext(Rc::new(RMatch { prog: prog.clone(), s: s.clone(), sv: sv.clone(), caps, pos, endpos }))
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
                        let r = call(&repl, Args::p(vec![V::Ext(Rc::new(m))]))?;
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
        Some(x) => type_err(format!("unsupported operand type(s) for &: '{}' and 'int'", tn(&x))),
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
            if let V::Ext(_) = &pv {
                return Ok(pv);
            }
            Ok(V::Ext(Rc::new(RPat { prog: pat_of(&pv, f)? })))
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
                self.ws();
                if self.s.get(self.i) == Some(&'}') {
                    self.i += 1;
                    return Ok(V::Dict(Rc::new(RefCell::new(d))));
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
                    d.set(k, v)?;
                    self.ws();
                    match self.s.get(self.i) {
                        Some(',') => {
                            self.i += 1;
                            self.ws();
                            if self.s.get(self.i) != Some(&'"') {
                                return json_err("Expecting property name enclosed in double quotes", self.s, self.i);
                            }
                        }
                        Some('}') => {
                            self.i += 1;
                            return Ok(V::Dict(Rc::new(RefCell::new(d))));
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
                Ok(V::Float(f64::NAN))
            }
            'I' if self.lit("Infinity") => {
                self.i += 8;
                Ok(V::Float(f64::INFINITY))
            }
            '-' if self.s.get(self.i + 1..self.i + 9).is_some_and(|w| w.iter().collect::<String>() == "Infinity") => {
                self.i += 9;
                Ok(V::Float(f64::NEG_INFINITY))
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
                if is_f {
                    Ok(V::Float(t.parse().unwrap_or(0.0)))
                } else {
                    parse_int(&t, 10).ok_or_else(|| exc("ValueError", "bad number".into()))
                }
            }
            _ => json_err("Expecting value", self.s, self.i),
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
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        _ => return json_err("Invalid \\escape", self.s, self.i - 2),
                    }
                }
                c if (c as u32) < 0x20 => return json_err("Invalid control character at", self.s, self.i - 1),
                c => out.push(c),
            }
        }
    }
}
pub fn json_loads(t: &str) -> R {
    let c: Vec<char> = t.chars().collect();
    let mut p = JP { s: &c, i: 0 };
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
fn json_float(f: f64) -> Result<String, E> {
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
        V::Float(f) => out.push_str(&json_float(*f)?),
        V::Str(x) => json_str(&x.s, o.ascii, out),
        V::List(_) | V::Tuple(_) => {
            let items = to_vec(v)?;
            if items.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
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
        }
        V::Dict(d) => {
            let mut items: Vec<(V, V)> = d.borrow().ents.iter().flatten().cloned().collect();
            if items.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            let mut keyed = vec![];
            for (k, x) in items.drain(..) {
                let ks = match &k {
                    V::Str(x) => x.s.clone(),
                    V::Int(_) | V::Big(_) => repr(&k)?,
                    V::Float(f) => json_float(*f)?,
                    V::Bool(b) => (if *b { "true" } else { "false" }).into(),
                    V::None => "null".into(),
                    _ => {
                        if o.skipkeys {
                            continue;
                        }
                        return type_err(format!("keys must be str, int, float, bool or None, not {}", tn(&k)));
                    }
                };
                keyed.push((ks, k, x));
            }
            if o.sort_keys {
                let mut err = None;
                let mut idx: Vec<usize> = (0..keyed.len()).collect();
                idx.sort_by(|&a, &b| match lt(&keyed[a].1, &keyed[b].1) {
                    Ok(true) => std::cmp::Ordering::Less,
                    Ok(false) => match lt(&keyed[b].1, &keyed[a].1) {
                        Ok(true) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    },
                    Err(e) => {
                        err.get_or_insert(e);
                        std::cmp::Ordering::Equal
                    }
                });
                if let Some(e) = err {
                    return Err(e);
                }
                let mut sorted = vec![];
                let mut tmp: Vec<Option<(String, V, V)>> = keyed.into_iter().map(Some).collect();
                for i in idx {
                    sorted.push(tmp[i].take().unwrap());
                }
                keyed = sorted;
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
                let r = call(f, Args::p(vec![v.clone()]))?;
                json_dump(&r, o, depth, out)?;
            }
            None => return type_err(format!("Object of type {} is not JSON serializable", tn(v))),
        },
    }
    Ok(())
}
fn json_opts(a: &mut Args) -> Result<JOpts, E> {
    let indent = match a.kw("indent") {
        None | Some(V::None) => None,
        Some(V::Int(n)) => Some(" ".repeat(n.max(0) as usize)),
        Some(V::Str(x)) => Some(x.s.clone()),
        Some(x) => return type_err(format!("indent must be int or str, not {}", tn(&x))),
    };
    let (mut item_sep, mut key_sep) = (if indent.is_some() { "," } else { ", " }.to_string(), ": ".to_string());
    if let Some(sep) = a.kw("separators") {
        if !matches!(sep, V::None) {
            let v = to_vec(&sep)?;
            if v.len() != 2 {
                return val_err("separators must be a pair".into());
            }
            item_sep = to_str(&v[0])?;
            key_sep = to_str(&v[1])?;
        }
    }
    let default = a.kw("default").filter(|d| !matches!(d, V::None));
    let mut flag = |n: &str, d: bool| -> Result<bool, E> { match a.kw(n) { Some(v) => truth(&v), None => Ok(d) } };
    Ok(JOpts {
        indent,
        item_sep,
        key_sep,
        sort_keys: flag("sort_keys", false)?,
        ascii: flag("ensure_ascii", true)?,
        default,
        skipkeys: flag("skipkeys", false)?,
    })
}
pub fn json_fn(n: &str, mut a: Args) -> R {
    match n {
        "loads" => {
            let kv = a.kw("s");
            let t = match a.p.first().or(kv.as_ref()) {
                Some(V::Str(x)) => x.s.clone(),
                Some(x) => return type_err(format!("the JSON object must be str, bytes or bytearray, not {}", tn(x))),
                None => return type_err("loads() missing 1 required positional argument: 's'".into()),
            };
            json_loads(&t)
        }
        "load" => {
            let f = a.p.first().cloned().unwrap_or(V::None);
            let t = match &f {
                V::Module("sys.stdin") => read_all(),
                _ => to_str(&call_method(&f, "read", Args::p(vec![]))?)?,
            };
            json_loads(&t)
        }
        "dumps" => {
            let v = a.p.first().cloned().ok_or_else(|| exc("TypeError", "dumps() missing 1 required positional argument: 'obj'".into()))?;
            let o = json_opts(&mut a)?;
            let mut out = String::new();
            json_dump(&v, &o, 0, &mut out)?;
            Ok(st(out))
        }
        "dump" => {
            let v = a.p.first().cloned().unwrap_or(V::None);
            let f = a.p.get(1).cloned().or_else(|| a.kw("fp")).unwrap_or(V::None);
            let o = json_opts(&mut a)?;
            let mut out = String::new();
            json_dump(&v, &o, 0, &mut out)?;
            match &f {
                V::Module("sys.stdout") => out_write(&out),
                V::Module("sys.stderr") => {
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
    let mut p = JP { s: &c, i: end - 1 };
    let v = p.string()?;
    Ok(tup(vec![v, V::Int(p.i as i64)]))
}
thread_local! {
    static JDEC: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
pub fn json_decoder_class() -> Rc<Class> {
    JDEC.with(|j| {
        let mut j = j.borrow_mut();
        if j.is_none() {
            let c = Class::new("JSONDecoder", "json.decoder", vec![bclass("object")], BT::Other);
            c.set("raw_decode", builtin("raw_decode", |a: Args| {
                let t = match a.p.get(1) {
                    Some(V::Str(x)) => x.s.clone(),
                    _ => return type_err("raw_decode() argument must be a string".into()),
                };
                let c: Vec<char> = t.chars().collect();
                let idx = json_index(&Args::p(a.p[1..].to_vec()), 1, c.len())?;
                if idx == usize::MAX || idx >= c.len() || matches!(c[idx], ' ' | '\t' | '\n' | '\r') {
                    return json_err("Expecting value", &c, idx.min(c.len()));
                }
                let mut p = JP { s: &c, i: idx };
                let v = p.value()?;
                Ok(tup(vec![v, V::Int(p.i as i64)]))
            }));
            c.set("decode", builtin("decode", |a: Args| match a.p.get(1) {
                Some(V::Str(x)) => json_loads(&x.s.clone()),
                _ => type_err("decode() argument must be a string".into()),
            }));
            *j = Some(c);
        }
        j.clone().unwrap()
    })
}
