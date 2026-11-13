// ---------- tiktoken, natively (rt_tok.rs) ----------
// The core of tiktoken (its Rust CoreBPE) for src/dynpy/py/tiktoken.py, Brevi's copy of tiktoken's Python API:
// the module _brevi_tok. The split patterns run on rt_tokre.rs. The vocabularies of OpenAI's encodings are built
// into the program by the compiler (src/tokdata.rs), which sets TOK_DATA before the program starts; so a program
// that tokenizes needs no network, no file and no CPython at run time. Results and messages are tiktoken 0.14's.

// the vocabularies built into the program: (file name such as "cl100k_base", blob from src/tokdata.rs)
pub static TOK_DATA: std::sync::OnceLock<Vec<(&'static str, &'static [u8])>> = std::sync::OnceLock::new();

// FxHash, as tiktoken's maps use
#[derive(Default, Clone, Copy)]
pub struct TkFx(u64);
impl std::hash::Hasher for TkFx {
    #[inline]
    fn write(&mut self, b: &[u8]) {
        let mut h = self.0;
        let mut c = b.chunks_exact(8);
        for w in &mut c {
            h = (h.rotate_left(5) ^ u64::from_le_bytes(w.try_into().unwrap())).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        for &x in c.remainder() {
            h = (h.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        self.0 = h;
    }
    #[inline]
    fn write_u32(&mut self, x: u32) {
        self.0 = (self.0.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_usize(&mut self, x: usize) {
        self.0 = (self.0.rotate_left(5) ^ x as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}
type TkMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<TkFx>>;

// mergeable ranks: bytes -> rank and back, in rank file order
pub struct TokRanks {
    enc: TkMap<&'static [u8], u32>,
    dec: TkMap<u32, &'static [u8]>,
    order: Vec<(&'static [u8], u32)>,
    max: Option<u32>,
}

impl TokRanks {
    fn new(order: Vec<(&'static [u8], u32)>) -> TokRanks {
        let mut enc = TkMap::default();
        let mut dec = TkMap::default();
        enc.reserve(order.len());
        dec.reserve(order.len());
        for (b, r) in &order {
            enc.insert(*b, *r);
            dec.insert(*r, *b);
        }
        let max = order.iter().map(|x| x.1).max();
        TokRanks { enc, dec, order, max }
    }
}

// a blob of src/tokdata.rs: "BTK1", count, count x (length byte, bytes), gap count, gaps x (index, rank)
fn tok_blob(b: &'static [u8]) -> Vec<(&'static [u8], u32)> {
    let u32at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    assert!(&b[..4] == b"BTK1", "bad tokenizer data");
    let n = u32at(4) as usize;
    let mut out = Vec::with_capacity(n);
    let mut at = 8;
    for _ in 0..n {
        let l = b[at] as usize;
        out.push((&b[at + 1..at + 1 + l], 0));
        at += 1 + l;
    }
    let g = u32at(at) as usize;
    at += 4;
    let gaps: Vec<(usize, u32)> = (0..g).map(|k| (u32at(at + 8 * k) as usize, u32at(at + 8 * k + 4))).collect();
    let mut base = 0u32;
    let mut gi = 0;
    for (i, e) in out.iter_mut().enumerate() {
        if gi < gaps.len() && gaps[gi].0 == i {
            base = gaps[gi].1.wrapping_sub(i as u32);
            gi += 1;
        }
        e.1 = base.wrapping_add(i as u32);
    }
    out
}

thread_local! {
    static TOK_FILES: RefCell<std::collections::HashMap<String, Rc<TokRanks>>> = RefCell::new(Default::default());
}

fn tok_file(name: &str) -> Result<Rc<TokRanks>, E> {
    if let Some(r) = TOK_FILES.with(|f| f.borrow().get(name).cloned()) {
        return Ok(r);
    }
    let Some(b) = TOK_DATA.get().and_then(|d| d.iter().find(|(n, _)| *n == name).map(|x| x.1)) else {
        return Err(unsupported(&format!("tiktoken encoding data {name} (not built into the program)")));
    };
    let r = Rc::new(TokRanks::new(tok_blob(b)));
    TOK_FILES.with(|f| f.borrow_mut().insert(name.to_string(), r.clone()));
    Ok(r)
}

pub struct TokCore {
    r: Rc<TokRanks>,
    // special tokens in their dict's order
    sp: Vec<(String, u32)>,
    sp_enc: TkMap<String, u32>,
    sp_dec: TkMap<u32, Vec<u8>>,
    // the special tokens by first byte (indices into sp)
    sp_first: [Vec<u32>; 256],
    re: TkRe,
}

// tiktoken's byte pair merge: the lowest-ranked adjacent pair merges first (the leftmost of equals)
fn tok_merge(r: &TokRanks, p: &[u8]) -> Vec<(usize, u32)> {
    let rank = |s: &[u8]| r.enc.get(s).copied().unwrap_or(u32::MAX);
    let mut parts: Vec<(usize, u32)> = Vec::with_capacity(p.len() + 1);
    let mut min = (u32::MAX, usize::MAX);
    for i in 0..p.len() - 1 {
        let k = rank(&p[i..i + 2]);
        if k < min.0 {
            min = (k, i);
        }
        parts.push((i, k));
    }
    parts.push((p.len() - 1, u32::MAX));
    parts.push((p.len(), u32::MAX));
    let get = |parts: &Vec<(usize, u32)>, i: usize| if i + 3 < parts.len() { rank(&p[parts[i].0..parts[i + 3].0]) } else { u32::MAX };
    while min.0 != u32::MAX {
        let i = min.1;
        if i > 0 {
            parts[i - 1].1 = get(&parts, i - 1);
        }
        parts[i].1 = get(&parts, i);
        parts.remove(i + 1);
        min = (u32::MAX, usize::MAX);
        for (j, &(_, k)) in parts[..parts.len() - 1].iter().enumerate() {
            if k < min.0 {
                min = (k, j);
            }
        }
    }
    parts
}

fn tok_bpe(r: &TokRanks, p: &[u8], out: &mut Vec<u32>) -> Result<(), E> {
    if p.len() == 1 {
        return match r.enc.get(p) {
            Some(t) => {
                out.push(*t);
                Ok(())
            }
            None => Err(tok_panic()),
        };
    }
    let parts = tok_merge(r, p);
    for w in parts.windows(2) {
        match r.enc.get(&p[w[0].0..w[1].0]) {
            Some(t) => out.push(*t),
            None => return Err(tok_panic()),
        }
    }
    Ok(())
}

// a vocabulary without every byte: tiktoken's Rust panics (pyo3_runtime.PanicException)
fn tok_panic() -> E {
    unsupported("tiktoken on a vocabulary that lacks a byte")
}

impl TokCore {
    fn piece(&self, p: &[u8], out: &mut Vec<u32>) -> Result<(), E> {
        if let Some(t) = self.r.enc.get(p) {
            out.push(*t);
            return Ok(());
        }
        tok_bpe(&self.r, p, out)
    }
    fn ordinary(&self, text: &str, out: &mut Vec<u32>) -> Result<(), E> {
        let cs: Vec<char> = text.chars().collect();
        let b = text.as_bytes();
        let mut res = Ok(());
        if text.is_ascii() {
            self.re.find_all(&cs, |s, e| {
                if res.is_ok() {
                    res = self.piece(&b[s..e], out);
                }
            });
        } else {
            let mut off = Vec::with_capacity(cs.len() + 1);
            let mut o = 0;
            for c in &cs {
                off.push(o);
                o += c.len_utf8();
            }
            off.push(o);
            self.re.find_all(&cs, |s, e| {
                if res.is_ok() {
                    res = self.piece(&b[off[s]..off[e]], out);
                }
            });
        }
        res
    }
    // the first special token at or after byte `from`: (start, index into sp)
    fn next_special(&self, b: &[u8], from: usize) -> Option<(usize, usize)> {
        for i in from..b.len() {
            for &k in &self.sp_first[b[i] as usize] {
                if b[i..].starts_with(self.sp[k as usize].0.as_bytes()) {
                    return Some((i, k as usize));
                }
            }
        }
        None
    }
    fn encode(&self, text: &str, allowed: &std::collections::HashSet<String>) -> Result<Vec<u32>, E> {
        let b = text.as_bytes();
        let mut out = vec![];
        let mut start = 0;
        loop {
            let mut find = start;
            let next = loop {
                match self.next_special(b, find) {
                    Some((s, k)) if !allowed.contains(&self.sp[k].0) => find = s + 1,
                    x => break x,
                }
            };
            let end = next.map_or(b.len(), |x| x.0);
            self.ordinary(&text[start..end], &mut out)?;
            match next {
                Some((s, k)) => {
                    out.push(self.sp[k].1);
                    start = s + self.sp[k].0.len();
                }
                None => break,
            }
        }
        Ok(out)
    }
    fn decode_one(&self, t: u32) -> Option<&[u8]> {
        self.r.dec.get(&t).copied().or_else(|| self.sp_dec.get(&t).map(|v| v.as_slice()))
    }
}

// pyo3's conversions, with its messages
fn tok_str(v: &V) -> Result<&str, E> {
    match v {
        V::Str(x) => Ok(&x.text()),
        V::None => type_err("'None' is not an instance of 'str'".into()),
        _ => type_err(format!("'{}' object is not an instance of 'str'", tn(v))),
    }
}
fn tok_rank(v: &V) -> Result<u32, E> {
    match v {
        V::Bool(b) => Ok(*b as u32),
        V::Int(i) if (0..=u32::MAX as i64).contains(i) => Ok(*i as u32),
        V::Int(_) => Err(exc("OverflowError", "out of range integral type conversion attempted".into())),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C long".into())),
        _ => type_err(format!("'{}' object cannot be interpreted as an integer", tn(v))),
    }
}
fn tok_ranks_arg(v: &V) -> Result<Vec<u32>, E> {
    let xs = match v {
        V::List(_) | V::Tuple(_) | V::Range(_) => to_vec(v)?,
        V::Str(_) => return type_err("Can't extract `str` to `Vec`".into()),
        V::None => return type_err("'None' is not an instance of 'Sequence'".into()),
        _ => match bytes_of(v) {
            Some(b) => b.into_iter().map(|x| V::Int(x as i64)).collect(),
            None => return type_err(format!("'{}' object is not an instance of 'Sequence'", tn(v))),
        },
    };
    xs.iter().map(tok_rank).collect()
}
fn tok_bytes_arg(v: &V) -> Result<Vec<u8>, E> {
    bytes_of(v).ok_or_else(|| exc("TypeError", if matches!(v, V::None) { "'None' is not an instance of 'bytes'".into() } else { format!("'{}' object is not an instance of 'bytes'", tn(v)) }))
}
fn tok_list(t: Vec<u32>) -> V {
    list(t.into_iter().map(|x| V::Int(x as i64)).collect())
}

fn tok_core_of(v: &V) -> Result<ExtRc, E> {
    match v {
        V::Ext(x) if x.any().downcast_ref::<TokCore>().is_some() => Ok(x.clone()),
        _ => type_err("expected a CoreBPE".into()),
    }
}

impl Ext for TokCore {
    fn tname(&self) -> &'static str {
        "tiktoken._tiktoken.CoreBPE"
    }
    fn repr(&self) -> String {
        "<builtins.CoreBPE object>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
        Some((|| -> R {
            match n {
                "encode_ordinary" => {
                    let mut out = vec![];
                    self.ordinary(tok_str(&arg(0))?, &mut out)?;
                    Ok(tok_list(out))
                }
                "encode" => {
                    let t0 = arg(0);
                    let text = tok_str(&t0)?;
                    let mut allowed = std::collections::HashSet::new();
                    let al = arg(1);
                    let items = match &al {
                        V::Set(s) => s.borrow().keys(),
                        V::FSet(s) => s.keys(),
                        _ => to_vec(&al)?,
                    };
                    for x in &items {
                        allowed.insert(tok_str(x)?.to_string());
                    }
                    Ok(tok_list(self.encode(text, &allowed)?))
                }
                "encode_single_piece" => {
                    let p = tok_bytes_arg(&arg(0))?;
                    if p.is_empty() {
                        return Err(tok_panic());
                    }
                    let mut out = vec![];
                    self.piece(&p, &mut out)?;
                    Ok(tok_list(out))
                }
                "encode_single_token" => {
                    let p = tok_bytes_arg(&arg(0))?;
                    if let Some(t) = self.r.enc.get(p.as_slice()) {
                        return Ok(V::Int(*t as i64));
                    }
                    if let Ok(s) = std::str::from_utf8(&p) {
                        if let Some(t) = self.sp_enc.get(s) {
                            return Ok(V::Int(*t as i64));
                        }
                    }
                    Err(exc_v("KeyError", bytesv(p)))
                }
                "decode_bytes" => {
                    let mut out = vec![];
                    for t in tok_ranks_arg(&arg(0))? {
                        match self.decode_one(t) {
                            Some(b) => out.extend_from_slice(b),
                            None => return Err(exc_v("KeyError", st(format!("Invalid token for decoding: {t}")))),
                        }
                    }
                    Ok(bytesv(out))
                }
                "decode_single_token_bytes" => {
                    let t = tok_rank(&arg(0))?;
                    match self.decode_one(t) {
                        Some(b) => Ok(bytesv(b.to_vec())),
                        None => Err(exc_v("KeyError", st(t.to_string()))),
                    }
                }
                "token_byte_values" => {
                    let mut v: Vec<&[u8]> = self.r.order.iter().map(|x| x.0).collect();
                    v.sort();
                    Ok(list(v.into_iter().map(|b| bytesv(b.to_vec())).collect()))
                }
                _ => Err(exc("AttributeError", format!("'builtins.CoreBPE' object has no attribute '{n}'"))),
            }
        })())
    }
}

// _brevi_tok.find_special(text, tokens): tiktoken's check for disallowed special tokens (a regex search for any
// of them, with the regex module's messages): the leftmost one found in the text, else None.
fn tok_find_special(text: &V, toks: &V) -> R {
    let t = match text {
        V::Str(x) => &x.text(),
        _ if bytes_of(text).is_some() => return type_err("cannot use a string pattern on a bytes-like object".into()),
        _ => return type_err(format!("a bytes-like object is required, not '{}'", tn(text))),
    };
    let items = match toks {
        V::Set(s) => s.borrow().keys(),
        V::FSet(s) => s.keys(),
        _ => to_vec(toks)?,
    };
    let toks: Vec<String> = items.iter().map(|x| tok_str(x).map(|s| s.to_string())).collect::<Result<_, _>>()?;
    let b = t.as_bytes();
    for i in 0..=b.len() {
        if !t.is_char_boundary(i) {
            continue;
        }
        for k in &toks {
            if b[i..].starts_with(k.as_bytes()) {
                return Ok(s(k));
            }
        }
    }
    Ok(V::None)
}

pub fn brevi_tok_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    match n {
        // core(file name or dict of bytes -> rank, dict of special tokens, pattern)
        "core" => {
            let src = arg(0);
            let r = match &src {
                V::Str(x) => tok_file(&x.text())?,
                V::Dict(d) => {
                    let mut order = vec![];
                    for (k, v) in d.borrow().items() {
                        let Some(b) = bytes_of(&k) else {
                            return type_err(if matches!(k, V::Str(_)) { "Can't extract `str` to `Vec`".into() } else { format!("'{}' object is not an instance of 'Sequence'", tn(&k)) });
                        };
                        let b: &'static [u8] = Box::leak(b.into_boxed_slice());
                        order.push((b, tok_rank(&v)?));
                    }
                    let r = TokRanks::new(order);
                    // tiktoken's CoreBPE panics on two byte strings with one rank
                    if r.enc.len() != r.dec.len() {
                        return Err(unsupported("tiktoken mergeable_ranks with duplicate token indices"));
                    }
                    Rc::new(r)
                }
                _ => return type_err(format!("'{}' object is not an instance of 'dict'", tn(&src))),
            };
            let mut sp = vec![];
            let spv = arg(1);
            let V::Dict(d) = &spv else {
                return type_err(format!("'{}' object is not an instance of 'dict'", tn(&spv)));
            };
            for (k, v) in d.borrow().items() {
                sp.push((tok_str(&k)?.to_string(), tok_rank(&v)?));
            }
            let pat = arg(2);
            let re = tk_compile(tok_str(&pat)?).map_err(|m| unsupported(&format!("tiktoken pattern ({m})")))?;
            let mut sp_first: [Vec<u32>; 256] = std::array::from_fn(|_| vec![]);
            let mut sp_enc = TkMap::default();
            let mut sp_dec = TkMap::default();
            for (i, (t, r)) in sp.iter().enumerate() {
                if let Some(&f) = t.as_bytes().first() {
                    sp_first[f as usize].push(i as u32);
                }
                sp_enc.insert(t.clone(), *r);
                sp_dec.insert(*r, t.as_bytes().to_vec());
            }
            Ok(V::Ext(ExtRc::new(TokCore { r, sp, sp_enc, sp_dec, sp_first, re })))
        }
        // ranks(core): the mergeable ranks as a dict, in their file's order
        "ranks" => {
            let c = tok_core_of(&arg(0))?;
            let c = c.any().downcast_ref::<TokCore>().unwrap();
            let mut d = Dict::new();
            for (b, r) in &c.r.order {
                d.set(bytesv(b.to_vec()), V::Int(*r as i64))?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        "max_rank" => {
            let c = tok_core_of(&arg(0))?;
            let c = c.any().downcast_ref::<TokCore>().unwrap();
            Ok(c.r.max.map_or(V::None, |m| V::Int(m as i64)))
        }
        "n_ranks" => {
            let c = tok_core_of(&arg(0))?;
            Ok(V::Int(c.any().downcast_ref::<TokCore>().unwrap().r.order.len() as i64))
        }
        "find_special" => tok_find_special(&arg(0), &arg(1)),
        _ => Err(unsupported(&format!("_brevi_tok.{n}"))),
    }
}
