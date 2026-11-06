// ---------- tiktoken's split patterns (rt_tokre.rs) ----------
// A backtracking regex engine for the patterns tiktoken splits text with (fancy-regex over the Rust regex crate:
// leftmost-first alternation, greedy / lazy / possessive repeats, atomic groups, lookahead, `$` at the end of
// the text, \p{..} classes as tiktoken's regex sees them, see rt_tokuni.rs). Anything else in a pattern is an
// error from tk_compile, which the compiler also runs on a program's literal patterns (src/tokre.rs), so a
// pattern this engine lacks sends the program to CPython at build time.

pub enum TkN {
    // one character of a class
    C(TkCls),
    // alternation (any group: captures are not used)
    G(Vec<Vec<TkN>>),
    // repeat: min, max (u32::MAX: no bound), 0 greedy, 1 lazy, 2 possessive
    R(Box<TkN>, u32, u32, u8),
    // atomic group
    A(Vec<Vec<TkN>>),
    // lookahead: negative?
    L(bool, Vec<Vec<TkN>>),
    End,
    Start,
}

pub struct TkCls {
    // the class's answer for ASCII, after negation
    ascii: [u64; 2],
    neg: bool,
    items: Vec<TkItem>,
}

enum TkItem {
    R(u32, u32),
    // a Unicode class, negated?
    P(&'static [(u32, u32)], bool),
}

fn tk_in(t: &[(u32, u32)], u: u32) -> bool {
    let (mut lo, mut hi) = (0usize, t.len());
    while lo < hi {
        let m = (lo + hi) / 2;
        if t[m].1 < u {
            lo = m + 1;
        } else if t[m].0 > u {
            hi = m;
        } else {
            return true;
        }
    }
    false
}

impl TkCls {
    fn new(neg: bool, items: Vec<TkItem>) -> TkCls {
        let mut c = TkCls { ascii: [0, 0], neg, items };
        for u in 0..128u32 {
            if c.slow(u) {
                c.ascii[(u >> 6) as usize] |= 1 << (u & 63);
            }
        }
        c
    }
    fn slow(&self, u: u32) -> bool {
        let m = self.items.iter().any(|it| match it {
            TkItem::R(a, b) => *a <= u && u <= *b,
            TkItem::P(t, n) => tk_in(t, u) != *n,
        });
        m != self.neg
    }
    #[inline]
    pub fn has(&self, c: char) -> bool {
        let u = c as u32;
        if u < 128 {
            return (self.ascii[(u >> 6) as usize] >> (u & 63)) & 1 == 1;
        }
        self.slow(u)
    }
}

pub struct TkRe {
    alts: Vec<Vec<TkN>>,
}

struct TkP<'a> {
    c: Vec<char>,
    i: usize,
    ci: bool,
    _s: &'a str,
}

fn tk_prop(name: &str) -> Option<&'static [(u32, u32)]> {
    Some(match name {
        "L" | "Letter" => TU_L,
        "Lu" | "Uppercase_Letter" => TU_LU,
        "Ll" | "Lowercase_Letter" => TU_LL,
        "Lt" | "Titlecase_Letter" => TU_LT,
        "Lm" | "Modifier_Letter" => TU_LM,
        "Lo" | "Other_Letter" => TU_LO,
        "M" | "Mark" => TU_M,
        "Mn" | "Nonspacing_Mark" => TU_MN,
        "Mc" | "Spacing_Mark" => TU_MC,
        "Me" | "Enclosing_Mark" => TU_ME,
        "N" | "Number" => TU_N,
        "Nd" | "Decimal_Number" => TU_ND,
        "Nl" | "Letter_Number" => TU_NL,
        "No" | "Other_Number" => TU_NO,
        "P" | "Punctuation" => TU_P,
        "S" | "Symbol" => TU_S,
        "Z" | "Separator" => TU_Z,
        _ => return None,
    })
}

// the characters c stands for, case-insensitively: ASCII letters and the few others tiktoken's regex folds them with
fn tk_fold(c: char, out: &mut Vec<TkItem>) -> Result<(), String> {
    out.push(TkItem::R(c as u32, c as u32));
    if c.is_ascii_alphabetic() {
        let o = if c.is_ascii_lowercase() { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() };
        out.push(TkItem::R(o as u32, o as u32));
        let l = c.to_ascii_lowercase();
        for (a, b) in TU_FOLD {
            if *a == l {
                out.push(TkItem::R(*b as u32, *b as u32));
            }
        }
    } else if !c.is_ascii() && (c.to_lowercase().ne(std::iter::once(c)) || c.to_uppercase().ne(std::iter::once(c)) || TU_FOLD.iter().any(|(_, b)| *b == c)) {
        return Err(format!("case-insensitive {c:?}"));
    }
    Ok(())
}

impl<'a> TkP<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn alts(&mut self) -> Result<Vec<Vec<TkN>>, String> {
        let mut out = vec![self.seq()?];
        while self.eat('|') {
            out.push(self.seq()?);
        }
        Ok(out)
    }
    fn seq(&mut self) -> Result<Vec<TkN>, String> {
        let mut out = vec![];
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let a = self.atom()?;
            let a = self.quant(a)?;
            out.push(a);
        }
        Ok(out)
    }
    fn num(&mut self) -> Option<u32> {
        let s = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.i == s {
            return None;
        }
        self.c[s..self.i].iter().collect::<String>().parse().ok()
    }
    fn quant(&mut self, a: TkN) -> Result<TkN, String> {
        let (min, max) = match self.peek() {
            Some('*') => (0, u32::MAX),
            Some('+') => (1, u32::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let Some(lo) = self.num() else {
                    return Err("'{' without a count".into());
                };
                let hi = if self.eat(',') { self.num().unwrap_or(u32::MAX) } else { lo };
                if !self.eat('}') || hi < lo {
                    self.i = save;
                    return Err("bad {m,n}".into());
                }
                self.i -= 1;
                (lo, hi)
            }
            _ => return Ok(a),
        };
        self.i += 1;
        if matches!(a, TkN::End | TkN::Start | TkN::L(..)) {
            return Err("repeat of an assertion".into());
        }
        let kind = if self.eat('?') {
            1
        } else if self.eat('+') {
            2
        } else {
            0
        };
        if matches!(self.peek(), Some('*' | '+' | '?' | '{')) {
            return Err("nested repeat".into());
        }
        Ok(TkN::R(Box::new(a), min, max, kind))
    }
    fn group(&mut self) -> Result<TkN, String> {
        // after '('
        let ci0 = self.ci;
        let n = if self.eat('?') {
            match self.peek() {
                Some(':') => {
                    self.i += 1;
                    TkN::G(self.alts()?)
                }
                Some('=') | Some('!') => {
                    let neg = self.peek() == Some('!');
                    self.i += 1;
                    TkN::L(neg, self.alts()?)
                }
                Some('>') => {
                    self.i += 1;
                    TkN::A(self.alts()?)
                }
                Some('i') => {
                    self.i += 1;
                    if !self.eat(':') {
                        return Err("flags other than i".into());
                    }
                    self.ci = true;
                    TkN::G(self.alts()?)
                }
                _ => return Err("group kind".into()),
            }
        } else {
            TkN::G(self.alts()?)
        };
        self.ci = ci0;
        if !self.eat(')') {
            return Err("unclosed group".into());
        }
        Ok(n)
    }
    fn hex(&mut self) -> Result<char, String> {
        let s;
        let e;
        if self.eat('{') {
            s = self.i;
            while self.peek().is_some_and(|c| c != '}') {
                self.i += 1;
            }
            e = self.i;
            if !self.eat('}') {
                return Err("bad \\x{..}".into());
            }
        } else {
            s = self.i;
            self.i = (self.i + 2).min(self.c.len());
            e = self.i;
        }
        let h: String = self.c[s..e].iter().collect();
        u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).ok_or_else(|| "bad hex escape".to_string())
    }
    // an escape after '\': a class's items (negated?) or one character
    fn escape(&mut self) -> Result<Result<(Vec<TkItem>, bool), char>, String> {
        let Some(c) = self.peek() else {
            return Err("trailing backslash".into());
        };
        self.i += 1;
        Ok(match c {
            's' | 'S' => Ok((vec![TkItem::P(TU_SPACE, false)], c == 'S')),
            'd' | 'D' => Ok((vec![TkItem::P(TU_ND, false)], c == 'D')),
            'w' | 'W' => Ok((vec![TkItem::P(TU_WORD, false)], c == 'W')),
            'p' | 'P' => {
                let name: String = if self.eat('{') {
                    let s = self.i;
                    while self.peek().is_some_and(|c| c != '}') {
                        self.i += 1;
                    }
                    let n = self.c[s..self.i].iter().collect();
                    if !self.eat('}') {
                        return Err("bad \\p{..}".into());
                    }
                    n
                } else {
                    let Some(n) = self.peek() else {
                        return Err("bad \\p".into());
                    };
                    self.i += 1;
                    n.to_string()
                };
                let (neg0, name) = match name.strip_prefix('^') {
                    Some(n) => (true, n.to_string()),
                    None => (false, name),
                };
                let Some(t) = tk_prop(&name) else {
                    return Err(format!("\\p{{{name}}}"));
                };
                if self.ci && !matches!(name.as_str(), "L" | "Letter" | "M" | "Mark" | "Mn" | "Mc" | "Me" | "N" | "Number" | "Nd" | "Nl" | "No" | "P" | "S" | "Z") {
                    return Err(format!("case-insensitive \\p{{{name}}}"));
                }
                Ok((vec![TkItem::P(t, false)], (c == 'P') != neg0))
            }
            'n' => Err('\n'),
            'r' => Err('\r'),
            't' => Err('\t'),
            'f' => Err('\x0c'),
            'v' => Err('\x0b'),
            'a' => Err('\x07'),
            'x' => Err(self.hex()?),
            c if c.is_ascii_punctuation() || c == ' ' => Err(c),
            c => return Err(format!("escape \\{c}")),
        })
    }
    fn lit(&self, c: char) -> Result<TkN, String> {
        let mut items = vec![];
        if self.ci {
            tk_fold(c, &mut items)?;
        } else {
            items.push(TkItem::R(c as u32, c as u32));
        }
        Ok(TkN::C(TkCls::new(false, items)))
    }
    fn class(&mut self) -> Result<TkN, String> {
        // after '['
        let neg = self.eat('^');
        let mut items: Vec<TkItem> = vec![];
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return Err("unclosed class".into());
            };
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            if c == '[' {
                return Err("nested class".into());
            }
            if (c == '&' || c == '-' || c == '~') && self.c.get(self.i + 1) == Some(&c) {
                return Err("class set operations".into());
            }
            self.i += 1;
            let lo = if c == '\\' {
                match self.escape()? {
                    Ok((its, n)) => {
                        if n {
                            // a negated class inside a class: everything but it
                            let t = match &its[0] {
                                TkItem::P(t, _) => *t,
                                _ => unreachable!(),
                            };
                            items.push(TkItem::P(t, true));
                        } else {
                            items.extend(its);
                        }
                        continue;
                    }
                    Err(ch) => ch,
                }
            } else {
                c
            };
            // a range lo-hi
            if self.peek() == Some('-') && self.c.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let h = self.c[self.i];
                self.i += 1;
                let hi = if h == '\\' {
                    match self.escape()? {
                        Err(ch) => ch,
                        Ok(_) => return Err("class in a range".into()),
                    }
                } else {
                    h
                };
                if hi < lo {
                    return Err("bad range".into());
                }
                if self.ci {
                    let (a, b) = (lo as u32, hi as u32);
                    if b >= 0x80 && (a..=b).any(|u| char::from_u32(u).is_some_and(|ch| tk_fold(ch, &mut vec![]).is_err())) {
                        return Err("case-insensitive range".into());
                    }
                    for u in a..=b.min(0x7f) {
                        tk_fold(char::from_u32(u).unwrap(), &mut items)?;
                    }
                    if b >= 0x80 {
                        items.push(TkItem::R(a.max(0x80), b));
                    }
                } else {
                    items.push(TkItem::R(lo as u32, hi as u32));
                }
                continue;
            }
            if self.ci {
                tk_fold(lo, &mut items)?;
            } else {
                items.push(TkItem::R(lo as u32, lo as u32));
            }
        }
        Ok(TkN::C(TkCls::new(neg, items)))
    }
    fn atom(&mut self) -> Result<TkN, String> {
        let c = self.peek().unwrap();
        self.i += 1;
        match c {
            '(' => self.group(),
            '[' => self.class(),
            '.' => Ok(TkN::C(TkCls::new(true, vec![TkItem::R(10, 10)]))),
            '$' => Ok(TkN::End),
            '^' => Ok(TkN::Start),
            '*' | '+' | '?' | '{' => Err("repeat of nothing".into()),
            '\\' if self.eat('z') => Ok(TkN::End),
            '\\' if self.eat('A') => Ok(TkN::Start),
            '\\' => match self.escape()? {
                Ok((its, n)) => Ok(TkN::C(TkCls::new(n, its))),
                Err(ch) => self.lit(ch),
            },
            c => self.lit(c),
        }
    }
}

// the fewest characters a match of the nodes takes
fn tk_min(s: &[TkN]) -> u64 {
    s.iter()
        .map(|n| match n {
            TkN::C(_) => 1,
            TkN::G(a) | TkN::A(a) => a.iter().map(|x| tk_min(x)).min().unwrap_or(0),
            TkN::R(n, min, _, _) => tk_min(std::slice::from_ref(&**n)) * *min as u64,
            _ => 0,
        })
        .sum()
}

pub fn tk_compile(p: &str) -> Result<TkRe, String> {
    // (?i) only at the start: the whole pattern
    let (ci, p) = match p.strip_prefix("(?i)") {
        Some(r) => (true, r),
        None => (false, p),
    };
    let mut ps = TkP { c: p.chars().collect(), i: 0, ci, _s: p };
    let alts = ps.alts()?;
    if ps.i != ps.c.len() {
        return Err("unbalanced ')'".into());
    }
    // tiktoken cannot encode an empty piece (it panics)
    if alts.iter().any(|a| tk_min(a) == 0) {
        return Err("a pattern that can match nothing".into());
    }
    Ok(TkRe { alts })
}

struct TkM<'a> {
    t: &'a [char],
}

impl<'a> TkM<'a> {
    fn alts(&self, alts: &[Vec<TkN>], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        for a in alts {
            if self.seq(a, i, k) {
                return true;
            }
        }
        false
    }
    // the end of the first match of the nodes at i (as an atomic group)
    fn first(&self, alts: &[Vec<TkN>], i: usize) -> Option<usize> {
        let mut e = None;
        self.alts(alts, i, &mut |j| {
            e = Some(j);
            true
        });
        e
    }
    fn seq(&self, s: &[TkN], i: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let Some((n, rest)) = s.split_first() else {
            return k(i);
        };
        let t = self.t;
        match n {
            TkN::C(c) => i < t.len() && c.has(t[i]) && self.seq(rest, i + 1, k),
            TkN::G(a) => self.alts(a, i, &mut |j| self.seq(rest, j, k)),
            TkN::A(a) => match self.first(a, i) {
                Some(j) => self.seq(rest, j, k),
                None => false,
            },
            TkN::L(neg, a) => {
                let f = self.alts(a, i, &mut |_| true);
                f != *neg && self.seq(rest, i, k)
            }
            TkN::End => i == t.len() && self.seq(rest, i, k),
            TkN::Start => i == 0 && self.seq(rest, i, k),
            TkN::R(x, min, max, kind) => {
                let (min, max) = (*min as usize, *max as usize);
                if let TkN::C(c) = &**x {
                    let mut n = 0;
                    while n < max && i + n < t.len() && c.has(t[i + n]) {
                        n += 1;
                    }
                    if n < min {
                        return false;
                    }
                    match kind {
                        2 => self.seq(rest, i + n, k),
                        1 => (min..=n).any(|m| self.seq(rest, i + m, k)),
                        _ => (min..=n).rev().any(|m| self.seq(rest, i + m, k)),
                    }
                } else if *kind == 2 {
                    let mut e = None;
                    self.rep(x, min, max, 0, 0, i, &[], &mut |j| {
                        e = Some(j);
                        true
                    });
                    match e {
                        Some(j) => self.seq(rest, j, k),
                        None => false,
                    }
                } else {
                    self.rep(x, min, max, *kind, 0, i, rest, k)
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn rep(&self, x: &TkN, min: usize, max: usize, kind: u8, cnt: usize, i: usize, rest: &[TkN], k: &mut dyn FnMut(usize) -> bool) -> bool {
        let one = std::slice::from_ref(x);
        let more = |k: &mut dyn FnMut(usize) -> bool| cnt < max && self.seq(one, i, &mut |j| (j != i || cnt < min) && self.rep(x, min, max, kind, cnt + 1, j, rest, k));
        if kind == 1 {
            (cnt >= min && self.seq(rest, i, k)) || more(k)
        } else {
            more(k) || (cnt >= min && self.seq(rest, i, k))
        }
    }
}

impl TkRe {
    // the end of the match at s, if one starts there
    pub fn match_at(&self, t: &[char], s: usize) -> Option<usize> {
        TkM { t }.first(&self.alts, s)
    }
    // regex find_iter over the text: each match's (start, end), in characters
    pub fn find_all(&self, t: &[char], mut f: impl FnMut(usize, usize)) {
        let mut s = 0;
        while s < t.len() {
            match self.match_at(t, s) {
                Some(e) => {
                    f(s, e);
                    s = e;
                }
                None => s += 1,
            }
        }
    }
}
