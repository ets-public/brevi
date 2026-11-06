// ---------- Hugging Face tokenizers: strings with alignments (rt_hfnorm.rs) ----------
// A port of tokenizers 0.23's NormalizedString (tokenizer/normalizer.rs, Apache-2.0): the text a normalizer
// changes, with each byte's span in the original, so offsets point at the input. Unicode normalization is
// unicode-normalization-alignments 0.1.12's (MIT/Apache-2.0) with its Unicode 9.0 tables (rt_hfuni.rs).

pub type Offs = (usize, usize);

#[derive(Clone, Debug, Default)]
pub struct NStr {
    pub original: String,
    pub normalized: String,
    pub align: Vec<Offs>,
    pub shift: usize,
}

// a Range of normalizer.rs: in the original or the normalized string, as a full start..end
#[derive(Clone, Copy)]
pub enum HRange {
    Orig(usize, usize),
    Norm(usize, usize),
}

fn expand_align(a: &[Offs]) -> Option<(usize, usize)> {
    if a.is_empty() {
        None
    } else {
        Some((a[0].0, a[a.len() - 1].1))
    }
}

impl NStr {
    pub fn from_str(s: &str) -> NStr {
        let mut align = Vec::with_capacity(s.len());
        for (b, c) in s.char_indices() {
            let l = c.len_utf8();
            for _ in 0..l {
                align.push((b, b + l));
            }
        }
        NStr { original: s.to_string(), normalized: s.to_string(), align, shift: 0 }
    }
    pub fn get(&self) -> &str {
        &self.normalized
    }
    pub fn len(&self) -> usize {
        self.normalized.len()
    }
    pub fn is_empty(&self) -> bool {
        self.normalized.is_empty()
    }
    pub fn offsets_original(&self) -> Offs {
        (self.shift, self.shift + self.original.len())
    }
    pub fn convert_offsets(&self, r: HRange) -> Option<(usize, usize)> {
        let (target, original) = match r {
            HRange::Orig(a, b) => ((a, b), true),
            HRange::Norm(a, b) => ((a, b), false),
        };
        if target.0 == target.1 {
            return Some(target);
        }
        if target.0 > target.1 {
            return None;
        }
        if original && self.original.is_empty() && target == (0, 0) {
            return Some((0, self.len()));
        }
        if !original && self.normalized.is_empty() && target == (0, 0) {
            return Some((0, self.original.len()));
        }
        if original {
            let (mut start, mut end) = (None, None);
            for (i, a) in self.align.iter().enumerate() {
                if target.1 < a.1 {
                    break;
                }
                if start.is_none() && target.0 <= a.0 && a.0 != a.1 {
                    start = Some(i);
                }
                if target.1 >= a.1 {
                    end = Some(i + 1);
                }
            }
            match (start, end) {
                (Some(s), None) => Some((s, s)),
                (None, Some(e)) => Some((e, e)),
                (Some(s), Some(e)) => Some((s, e)),
                _ => None,
            }
        } else {
            self.align.get(target.0..target.1).and_then(expand_align)
        }
    }
    fn get_range(&self, r: HRange) -> Option<&str> {
        match r {
            HRange::Orig(..) => {
                let (a, b) = self.convert_offsets(r)?;
                self.normalized.get(a..b)
            }
            HRange::Norm(a, b) => self.normalized.get(a..b),
        }
    }
    fn get_range_original(&self, r: HRange) -> Option<&str> {
        match r {
            HRange::Orig(a, b) => self.original.get(a..b),
            HRange::Norm(..) => {
                let (a, b) = self.convert_offsets(r)?;
                self.original.get(a..b)
            }
        }
    }
    pub fn slice(&self, r: HRange) -> Option<NStr> {
        // validate_range
        let ok = match r {
            HRange::Orig(a, b) => b <= self.original.len() && self.original.is_char_boundary(a) && self.original.is_char_boundary(b),
            HRange::Norm(a, b) => b <= self.normalized.len() && self.normalized.is_char_boundary(a) && self.normalized.is_char_boundary(b),
        };
        if !ok {
            return None;
        }
        let (nr, or) = match r {
            HRange::Orig(a, b) => (self.convert_offsets(r)?, (a, b)),
            HRange::Norm(a, b) => ((a, b), self.convert_offsets(r)?),
        };
        let n_shift = or.0;
        Some(NStr {
            original: self.get_range_original(r).unwrap_or_default().to_string(),
            normalized: self.get_range(r).unwrap_or_default().to_string(),
            align: self.align.get(nr.0..nr.1)?.iter().map(|(s, e)| (s - n_shift, e - n_shift)).collect(),
            shift: self.shift + or.0,
        })
    }
    pub fn transform_range(&mut self, r: HRange, dest: &[(char, isize)], initial_offset: usize) {
        let (ns, ne) = match r {
            HRange::Norm(a, b) => (a, b),
            HRange::Orig(..) => match self.convert_offsets(r) {
                Some(x) => x,
                None => return,
            },
        };
        let rep: Vec<char> = self.normalized[ns..ne].chars().collect();
        let mut ri = 0;
        let initial_removed: usize = rep.iter().take(initial_offset).map(|c| c.len_utf8()).sum();
        ri += initial_offset.min(rep.len());
        let mut offset = (initial_removed + ns) as isize;
        let mut aligns = Vec::with_capacity(ne - ns);
        let mut out = String::new();
        for &(c, changes) in dest {
            let idx = offset as usize;
            let a = if changes > 0 {
                if idx < 1 {
                    (0, 0)
                } else {
                    self.align[idx - 1]
                }
            } else {
                self.align[idx]
            };
            let replaced = if changes <= 0 && ri < rep.len() {
                ri += 1;
                Some(rep[ri - 1])
            } else {
                None
            };
            let rsize = replaced.map_or(0, |c| c.len_utf8());
            let mut remove = 0;
            if changes < 0 {
                for _ in 0..(-changes) as usize {
                    if ri < rep.len() {
                        remove += rep[ri].len_utf8();
                        ri += 1;
                    }
                }
            }
            offset += rsize as isize + remove as isize;
            for _ in 0..c.len_utf8() {
                aligns.push(a);
            }
            out.push(c);
        }
        self.align.splice(ns..ne, aligns);
        self.normalized.replace_range(ns..ne, &out);
    }
    pub fn transform(&mut self, dest: &[(char, isize)], initial_offset: usize) {
        let n = self.original.len();
        self.transform_range(HRange::Orig(0, n), dest, initial_offset)
    }
    pub fn nfd(&mut self) {
        let d = hu_decompose(self.get(), false);
        self.transform(&d, 0)
    }
    pub fn nfkd(&mut self) {
        let d = hu_decompose(self.get(), true);
        self.transform(&d, 0)
    }
    pub fn nfc(&mut self) {
        let d = hu_recompose(hu_decompose(self.get(), false));
        self.transform(&d, 0)
    }
    pub fn nfkc(&mut self) {
        let d = hu_recompose(hu_decompose(self.get(), true));
        self.transform(&d, 0)
    }
    pub fn filter(&mut self, keep: impl Fn(char) -> bool) {
        let mut removed: isize = 0;
        let mut removed_start = 0usize;
        let mut tr = Vec::with_capacity(self.normalized.len());
        let mut last: Option<char> = None;
        for c in self.normalized.chars() {
            if keep(c) {
                match last {
                    Some(lc) => tr.push((lc, -removed)),
                    None => removed_start = removed as usize,
                }
                last = Some(c);
                removed = 0;
            } else {
                removed += 1;
            }
        }
        if let Some(lc) = last {
            tr.push((lc, -removed));
        }
        self.transform(&tr, removed_start)
    }
    pub fn prepend(&mut self, s: &str) {
        if let Some(next) = self.normalized.chars().next() {
            let mut tr: Vec<(char, isize)> = s.chars().enumerate().map(|(i, c)| (c, (i != 0) as isize)).collect();
            tr.push((next, 1));
            self.transform_range(HRange::Norm(0, next.len_utf8()), &tr, 0);
        }
    }
    pub fn append(&mut self, s: &str) {
        if let Some((b, prev)) = self.normalized.char_indices().last() {
            let mut tr = vec![(prev, 0)];
            tr.extend(s.chars().map(|c| (c, 1)));
            let n = self.len();
            self.transform_range(HRange::Norm(b, n), &tr, 0);
        } else {
            let tr: Vec<(char, isize)> = s.chars().map(|c| (c, 1)).collect();
            let n = self.len();
            self.transform_range(HRange::Norm(0, n), &tr, 0);
        }
    }
    pub fn map(&mut self, f: impl Fn(char) -> char) {
        let tr: Vec<(char, isize)> = self.normalized.chars().map(|c| (f(c), 0)).collect();
        self.transform(&tr, 0)
    }
    pub fn lowercase(&mut self) {
        let mut tr = vec![];
        for c in self.normalized.chars() {
            for (i, l) in hc_lower(c).enumerate() {
                tr.push((l, (i > 0) as isize));
            }
        }
        self.transform(&tr, 0)
    }
    pub fn replace(&mut self, pat: &HPat, content: &str) {
        let mut nn = String::with_capacity(self.normalized.len());
        let mut na: Vec<Offs> = Vec::with_capacity(self.align.len());
        let mut last_end = 0;
        for ((start, end), is_match) in pat.find_matches(&self.normalized) {
            if !is_match {
                continue;
            }
            nn.push_str(&self.normalized[last_end..start]);
            na.extend_from_slice(&self.align[last_end..start]);
            let rep: Vec<char> = self.normalized[start..end].chars().collect();
            let initial_removed: usize = rep.iter().map(|c| c.len_utf8()).sum();
            let offset = (initial_removed + start) as isize;
            for c in content.chars() {
                let idx = offset as usize;
                let a = if idx < 1 { (0, 0) } else { self.align[idx - 1] };
                for _ in 0..c.len_utf8() {
                    na.push(a);
                }
                nn.push(c);
            }
            last_end = end;
        }
        nn.push_str(&self.normalized[last_end..]);
        na.extend_from_slice(&self.align[last_end..]);
        self.normalized = nn;
        self.align = na;
    }
    pub fn split(&self, pat: &HPat, behavior: Behavior) -> Vec<NStr> {
        self.split_found(pat.find_matches(&self.normalized), behavior)
    }
    pub fn split_found(&self, matches: Vec<(Offs, bool)>, behavior: Behavior) -> Vec<NStr> {
        let splits: Vec<(Offs, bool)> = match behavior {
            Behavior::Isolated => matches.into_iter().map(|(o, _)| (o, false)).collect(),
            Behavior::Removed => matches,
            Behavior::Contiguous => {
                let mut prev = false;
                let mut acc: Vec<(Offs, bool)> = vec![];
                for (o, m) in matches {
                    if m == prev {
                        if let Some(((_, e), _)) = acc.last_mut() {
                            *e = o.1;
                        } else {
                            acc.push((o, false));
                        }
                    } else {
                        acc.push((o, false));
                    }
                    prev = m;
                }
                acc
            }
            Behavior::MergedWithPrevious => {
                let mut prev = false;
                let mut acc: Vec<(Offs, bool)> = vec![];
                for (o, m) in matches {
                    if m && !prev {
                        if let Some(((_, e), _)) = acc.last_mut() {
                            *e = o.1;
                        } else {
                            acc.push((o, false));
                        }
                    } else {
                        acc.push((o, false));
                    }
                    prev = m;
                }
                acc
            }
            Behavior::MergedWithNext => {
                let mut prev = false;
                let mut acc: Vec<(Offs, bool)> = vec![];
                for (o, m) in matches.into_iter().rev() {
                    if m && !prev {
                        if let Some(((s, _), _)) = acc.last_mut() {
                            *s = o.0;
                        } else {
                            acc.push((o, false));
                        }
                    } else {
                        acc.push((o, false));
                    }
                    prev = m;
                }
                acc.reverse();
                acc
            }
        };
        splits.into_iter().filter(|(_, rm)| !rm).map(|(o, _)| self.slice(HRange::Norm(o.0, o.1)).expect("NormalizedString bad split")).collect()
    }
    pub fn lrstrip(&mut self, left: bool, right: bool) {
        let lead = if left { self.normalized.chars().take_while(|c| hc_ws(*c)).count() } else { 0 };
        let trail = if right { self.normalized.chars().rev().take_while(|c| hc_ws(*c)).count() } else { 0 };
        if lead > 0 || trail > 0 {
            let count = self.normalized.chars().count();
            let blen = self.len();
            let tr: Vec<(char, isize)> = self
                .normalized
                .chars()
                .enumerate()
                .filter_map(|(i, c)| {
                    if i < lead || i >= count - trail {
                        None
                    } else if i == blen.wrapping_sub(trail).wrapping_sub(1) {
                        Some((c, -(trail as isize)))
                    } else {
                        Some((c, 0))
                    }
                })
                .collect();
            self.transform(&tr, lead)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Behavior {
    Removed,
    Isolated,
    MergedWithPrevious,
    MergedWithNext,
    Contiguous,
}

// tokenizers' Pattern: a string, a regex, or a char test; inverted or not
pub enum HPat {
    Str(String),
    Re(Rc<TkRe>),
    Fn(fn(char) -> bool),
    Char(char),
    Invert(Box<HPat>),
}

impl HPat {
    pub fn find_matches(&self, inside: &str) -> Vec<(Offs, bool)> {
        match self {
            HPat::Invert(p) => p.find_matches(inside).into_iter().map(|(o, f)| (o, !f)).collect(),
            HPat::Str(s) if s.is_empty() => vec![((0, inside.chars().count()), false)],
            HPat::Str(s) => {
                // a regex of the escaped string: leftmost, non-overlapping
                if inside.is_empty() {
                    return vec![((0, 0), false)];
                }
                let mut out = vec![];
                let mut prev = 0;
                let mut at = 0;
                while let Some(k) = inside[at..].find(s.as_str()) {
                    let st = at + k;
                    if prev != st {
                        out.push(((prev, st), false));
                    }
                    out.push(((st, st + s.len()), true));
                    prev = st + s.len();
                    at = prev;
                }
                if prev != inside.len() {
                    out.push(((prev, inside.len()), false));
                }
                out
            }
            HPat::Re(re) => {
                if inside.is_empty() {
                    return vec![((0, 0), false)];
                }
                let cs: Vec<char> = inside.chars().collect();
                let mut off = Vec::with_capacity(cs.len() + 1);
                let mut o = 0;
                for c in &cs {
                    off.push(o);
                    o += c.len_utf8();
                }
                off.push(o);
                let mut out = vec![];
                let mut prev = 0;
                re.find_all(&cs, |s, e| {
                    let (s, e) = (off[s], off[e]);
                    if prev != s {
                        out.push(((prev, s), false));
                    }
                    out.push(((s, e), true));
                    prev = e;
                });
                if prev != inside.len() {
                    out.push(((prev, inside.len()), false));
                }
                out
            }
            HPat::Char(ch) => {
                let ch = *ch;
                hpat_fn(inside, &|c| c == ch)
            }
            HPat::Fn(f) => hpat_fn(inside, f),
        }
    }
}

fn hpat_fn(inside: &str, f: &dyn Fn(char) -> bool) -> Vec<(Offs, bool)> {
    if inside.is_empty() {
        return vec![((0, 0), false)];
    }
    let mut last_offset = 0;
    let mut last_seen = 0;
    let mut out = vec![];
    for (b, c) in inside.char_indices() {
        last_seen = b + c.len_utf8();
        if f(c) {
            if last_offset < b {
                out.push(((last_offset, b), false));
            }
            out.push(((b, b + c.len_utf8()), true));
            last_offset = b + c.len_utf8();
        }
    }
    if last_seen > last_offset {
        out.push(((last_offset, last_seen), false));
    }
    out
}

// ---------- unicode-normalization-alignments ----------

fn hu_ccc(c: char) -> u8 {
    let u = c as u32;
    HU_CCC.binary_search_by_key(&u, |x| x.0).map_or(0, |i| HU_CCC[i].1)
}

fn hu_decomp_of(c: char, compat: bool) -> Option<&'static [char]> {
    let u = c as u32;
    let find = |t: &'static [(u32, &'static [char])]| t.binary_search_by_key(&u, |x| x.0).ok().map(|i| t[i].1);
    if compat {
        find(HU_COMPAT).or_else(|| find(HU_CANON))
    } else {
        find(HU_CANON)
    }
}

const HS_BASE: u32 = 0xAC00;
const HL_BASE: u32 = 0x1100;
const HV_BASE: u32 = 0x1161;
const HT_BASE: u32 = 0x11A7;
const HL_COUNT: u32 = 19;
const HV_COUNT: u32 = 21;
const HT_COUNT: u32 = 28;
const HN_COUNT: u32 = HV_COUNT * HT_COUNT;
const HS_COUNT: u32 = HL_COUNT * HN_COUNT;

fn hu_each_decomp(c: char, compat: bool, emit: &mut dyn FnMut(char)) {
    if c <= '\x7f' {
        return emit(c);
    }
    let u = c as u32;
    if (HS_BASE..HS_BASE + HS_COUNT).contains(&u) {
        let si = u - HS_BASE;
        emit(char::from_u32(HL_BASE + si / HN_COUNT).unwrap());
        emit(char::from_u32(HV_BASE + (si % HN_COUNT) / HT_COUNT).unwrap());
        let ti = si % HT_COUNT;
        if ti > 0 {
            emit(char::from_u32(HT_BASE + ti).unwrap());
        }
        return;
    }
    match hu_decomp_of(c, compat) {
        Some(d) => d.iter().for_each(|&x| emit(x)),
        None => emit(c),
    }
}

// Decompositions: (char, change), marks reordered by combining class (a stable sort of each run)
fn hu_decompose(s: &str, compat: bool) -> Vec<(char, isize)> {
    let mut out: Vec<(char, isize)> = Vec::with_capacity(s.len());
    let mut pending: Vec<(u8, char, isize)> = vec![];
    let flush = |pending: &mut Vec<(u8, char, isize)>, out: &mut Vec<(char, isize)>| {
        pending.sort_by_key(|k| k.0);
        out.extend(pending.drain(..).map(|(_, c, n)| (c, n)));
    };
    for ch in s.chars() {
        let mut first = true;
        hu_each_decomp(ch, compat, &mut |d| {
            let class = hu_ccc(d);
            if class == 0 {
                flush(&mut pending, &mut out);
            }
            pending.push((class, d, if first { 0 } else { 1 }));
            first = false;
        });
    }
    flush(&mut pending, &mut out);
    out
}

fn hu_compose(a: char, b: char) -> Option<char> {
    let (x, y) = (a as u32, b as u32);
    if (HL_BASE..HL_BASE + HL_COUNT).contains(&x) && (HV_BASE..HV_BASE + HV_COUNT).contains(&y) {
        return char::from_u32(HS_BASE + (x - HL_BASE) * HN_COUNT + (y - HV_BASE) * HT_COUNT);
    }
    if (HS_BASE..HS_BASE + HS_COUNT).contains(&x) && (HT_BASE + 1..HT_BASE + HT_COUNT).contains(&y) && (x - HS_BASE) % HT_COUNT == 0 {
        return char::from_u32(x + (y - HT_BASE));
    }
    HU_COMP.binary_search_by(|e| (e.0, e.1).cmp(&(x, y))).ok().and_then(|i| char::from_u32(HU_COMP[i].2))
}

// Recompositions over the decomposed stream (recompose.rs's state machine)
fn hu_recompose(input: Vec<(char, isize)>) -> Vec<(char, isize)> {
    let mut out = Vec::with_capacity(input.len());
    let mut buffer: Vec<(char, isize)> = vec![];
    let mut composee: Option<(char, isize)> = None;
    let mut last_ccc: Option<u8> = None;
    for (ch, change) in input {
        let ch_class = hu_ccc(ch);
        let k = match composee {
            None => {
                if ch_class != 0 {
                    out.push((ch, change));
                    continue;
                }
                composee = Some((ch, change));
                continue;
            }
            Some(k) => k,
        };
        match last_ccc {
            None => match hu_compose(k.0, ch) {
                Some(r) => {
                    composee = Some((r, k.1 + change - 1));
                }
                None => {
                    if ch_class == 0 {
                        composee = Some((ch, change));
                        out.push(k);
                        continue;
                    }
                    buffer.push((ch, change));
                    last_ccc = Some(ch_class);
                }
            },
            Some(l_class) => {
                if l_class >= ch_class {
                    if ch_class == 0 {
                        composee = Some((ch, change));
                        last_ccc = None;
                        out.push(k);
                        out.append(&mut buffer);
                        continue;
                    }
                    buffer.push((ch, change));
                    last_ccc = Some(ch_class);
                    continue;
                }
                match hu_compose(k.0, ch) {
                    Some(r) => composee = Some((r, k.1 + change - 1)),
                    None => {
                        buffer.push((ch, change));
                        last_ccc = Some(ch_class);
                    }
                }
            }
        }
    }
    // Finished: the composee, then the buffer
    if let Some(k) = composee {
        out.push(k);
    }
    out.append(&mut buffer);
    out
}

// ---------- the character tests of tokenizers (tables in rt_hfcls.rs, read from tokenizers itself) ----------

fn hc_in(t: &[(u32, u32)], c: char) -> bool {
    let u = c as u32;
    t.binary_search_by(|r| if r.1 < u { std::cmp::Ordering::Less } else if r.0 > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}
// char::is_whitespace
pub fn hc_ws(c: char) -> bool {
    hc_in(&HC_WS, c)
}
// char::is_numeric
pub fn hc_num(c: char) -> bool {
    hc_in(&HC_NUM, c)
}
// is_ascii_punctuation or unicode_categories' is_punctuation
pub fn hc_punct(c: char) -> bool {
    hc_in(&HC_PUNCT, c)
}
// unicode_categories' is_other, without \t \n \r (BertNormalizer's is_control)
pub fn hc_control(c: char) -> bool {
    hc_in(&HC_OTHER, c)
}
// unicode_categories' is_mark_nonspacing
pub fn hc_mn(c: char) -> bool {
    hc_in(&HC_MN, c)
}
// unicode-normalization-alignments' is_combining_mark
pub fn hc_comb(c: char) -> bool {
    hc_in(&HC_COMB, c)
}
// the regex crate's \w and \s
pub fn hc_rew(c: char) -> bool {
    hc_in(&HC_REW, c)
}
pub fn hc_res(c: char) -> bool {
    hc_in(&HC_RES, c)
}
// char::to_lowercase
pub fn hc_lower(c: char) -> impl Iterator<Item = char> {
    let u = c as u32;
    let mapped = HC_LOWER.binary_search_by_key(&u, |x| x.0).ok().map(|i| HC_LOWER[i].1);
    let one = if mapped.is_none() { Some(c) } else { None };
    mapped.into_iter().flat_map(|s| s.chars()).chain(one)
}
