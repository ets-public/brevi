// ---------- Hugging Face tokenizers: the pipeline (rt_hfcore.rs) ----------
// A port of tokenizers 0.23.3 (Apache-2.0): a tokenizer.json read into the components it names (normalizers,
// pre-tokenizers, the BPE / WordPiece / WordLevel models, post-processors, decoders, added tokens, truncation
// and padding) and Tokenizer::encode_char_offsets / decode over them, step for step. Nothing here touches V, so
// the compiler includes this file too (src/dynpy/hf.rs) and refuses a tokenizer.json it cannot run, at build
// time. A component tokenizers has and this lacks (Unigram, Precompiled, UnicodeScripts, BPE dropout, ...) is
// an error naming it: HfErr::Lacks.

pub enum HfErr {
    // tokenizers raises Exception(message)
    Py(String),
    // a part of tokenizers Brevi lacks
    Lacks(String),
}
type HR<T> = Result<T, HfErr>;
fn hpy<T>(m: impl Into<String>) -> HR<T> {
    Err(HfErr::Py(m.into()))
}
fn hlacks<T>(m: impl Into<String>) -> HR<T> {
    Err(HfErr::Lacks(m.into()))
}

// ---------- JSON (serde_json's reading of a tokenizer.json) ----------

#[derive(Clone, Debug)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(kv) => kv.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v),
            _ => None,
        }
    }
    fn opt(&self, k: &str) -> Option<&J> {
        self.get(k).filter(|v| !matches!(v, J::Null))
    }
    fn str_(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    fn tname(&self) -> &str {
        self.get("type").and_then(|t| t.str_()).unwrap_or("")
    }
}

pub fn hj_parse(t: &str) -> HR<J> {
    let b = t.as_bytes();
    let mut p = 0;
    let v = hj_val(t, b, &mut p, 0)?;
    hj_ws(b, &mut p);
    if p != b.len() {
        return hj_err("trailing characters", b, p);
    }
    Ok(v)
}

fn hj_err<T>(what: &str, b: &[u8], p: usize) -> HR<T> {
    let pre = &b[..p.min(b.len())];
    let line = pre.iter().filter(|&&c| c == b'\n').count() + 1;
    let col = pre.len() - pre.iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
    hpy(format!("{what} at line {line} column {col}"))
}

fn hj_ws(b: &[u8], p: &mut usize) {
    while *p < b.len() && matches!(b[*p], b' ' | b'\t' | b'\n' | b'\r') {
        *p += 1;
    }
}

fn hj_val(t: &str, b: &[u8], p: &mut usize, depth: usize) -> HR<J> {
    hj_ws(b, p);
    if depth > 128 {
        return hj_err("recursion limit exceeded", b, *p);
    }
    let Some(&c) = b.get(*p) else {
        return hj_err("EOF while parsing a value", b, *p);
    };
    match c {
        b'{' => {
            *p += 1;
            let mut kv = vec![];
            hj_ws(b, p);
            if b.get(*p) == Some(&b'}') {
                *p += 1;
                return Ok(J::Obj(kv));
            }
            loop {
                hj_ws(b, p);
                if b.get(*p) != Some(&b'"') {
                    return hj_err("key must be a string", b, *p);
                }
                let k = hj_str(t, b, p)?;
                hj_ws(b, p);
                if b.get(*p) != Some(&b':') {
                    return hj_err("expected `:`", b, *p);
                }
                *p += 1;
                let v = hj_val(t, b, p, depth + 1)?;
                kv.push((k, v));
                hj_ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b'}') => {
                        *p += 1;
                        return Ok(J::Obj(kv));
                    }
                    _ => return hj_err("expected `,` or `}`", b, *p),
                }
            }
        }
        b'[' => {
            *p += 1;
            let mut xs = vec![];
            hj_ws(b, p);
            if b.get(*p) == Some(&b']') {
                *p += 1;
                return Ok(J::Arr(xs));
            }
            loop {
                xs.push(hj_val(t, b, p, depth + 1)?);
                hj_ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b']') => {
                        *p += 1;
                        return Ok(J::Arr(xs));
                    }
                    _ => return hj_err("expected `,` or `]`", b, *p),
                }
            }
        }
        b'"' => Ok(J::Str(hj_str(t, b, p)?)),
        b't' if b[*p..].starts_with(b"true") => {
            *p += 4;
            Ok(J::Bool(true))
        }
        b'f' if b[*p..].starts_with(b"false") => {
            *p += 5;
            Ok(J::Bool(false))
        }
        b'n' if b[*p..].starts_with(b"null") => {
            *p += 4;
            Ok(J::Null)
        }
        b'-' | b'0'..=b'9' => {
            let st = *p;
            if b[*p] == b'-' {
                *p += 1;
            }
            let mut float = false;
            while *p < b.len() && (b[*p].is_ascii_digit() || matches!(b[*p], b'.' | b'e' | b'E' | b'+' | b'-')) {
                float |= matches!(b[*p], b'.' | b'e' | b'E');
                *p += 1;
            }
            let txt = &t[st..*p];
            if !float {
                if let Ok(i) = txt.parse::<i64>() {
                    return Ok(J::Int(i));
                }
            }
            match txt.parse::<f64>() {
                Ok(f) => Ok(J::Num(f)),
                Err(_) => hj_err("invalid number", b, st),
            }
        }
        _ => hj_err("expected value", b, *p),
    }
}

fn hj_str(t: &str, b: &[u8], p: &mut usize) -> HR<String> {
    *p += 1;
    let mut out = String::new();
    let mut run = *p;
    loop {
        let Some(&c) = b.get(*p) else {
            return hj_err("EOF while parsing a string", b, *p);
        };
        match c {
            b'"' => {
                out.push_str(&t[run..*p]);
                *p += 1;
                return Ok(out);
            }
            b'\\' => {
                out.push_str(&t[run..*p]);
                *p += 1;
                let Some(&e) = b.get(*p) else {
                    return hj_err("EOF while parsing a string", b, *p);
                };
                *p += 1;
                match e {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = |p: &mut usize| -> HR<u32> {
                            let h = t.get(*p..*p + 4).and_then(|h| u32::from_str_radix(h, 16).ok());
                            match h {
                                Some(v) => {
                                    *p += 4;
                                    Ok(v)
                                }
                                None => hj_err("invalid escape", b, *p),
                            }
                        };
                        let u = hex(p)?;
                        let ch = if (0xD800..0xDC00).contains(&u) {
                            if b.get(*p) == Some(&b'\\') && b.get(*p + 1) == Some(&b'u') {
                                *p += 2;
                                let lo = hex(p)?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return hj_err("lone leading surrogate in hex escape", b, *p);
                                }
                                char::from_u32(0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00))
                            } else {
                                return hj_err("lone leading surrogate in hex escape", b, *p);
                            }
                        } else {
                            char::from_u32(u)
                        };
                        match ch {
                            Some(ch) => out.push(ch),
                            None => return hj_err("invalid unicode code point", b, *p),
                        }
                    }
                    _ => return hj_err("invalid escape", b, *p),
                }
                run = *p;
            }
            0..=0x1f => return hj_err("control character (\\u0000-\\u001F) found while parsing a string", b, *p),
            _ => *p += 1,
        }
    }
}

fn hj_bool(v: &J, k: &str, d: bool) -> HR<bool> {
    match v.get(k) {
        None | Some(J::Null) => Ok(d),
        Some(J::Bool(b)) => Ok(*b),
        Some(_) => hlacks(format!("tokenizer.json field {k} that is not true or false")),
    }
}
fn hj_u32(v: &J) -> HR<u32> {
    match v {
        J::Int(i) if (0..=u32::MAX as i64).contains(i) => Ok(*i as u32),
        _ => hlacks("tokenizer.json id that is not a 32-bit unsigned int"),
    }
}
fn hj_usize(v: &J, k: &str, d: usize) -> HR<usize> {
    match v.get(k) {
        None | Some(J::Null) => Ok(d),
        Some(J::Int(i)) if *i >= 0 => Ok(*i as usize),
        Some(_) => hlacks(format!("tokenizer.json field {k} that is not an unsigned int")),
    }
}
fn hj_string(v: &J, k: &str) -> HR<Option<String>> {
    match v.get(k) {
        None | Some(J::Null) => Ok(None),
        Some(J::Str(s)) => Ok(Some(s.clone())),
        Some(_) => hlacks(format!("tokenizer.json field {k} that is not a string")),
    }
}
fn hj_char(v: &J, k: &str, d: char) -> HR<char> {
    match hj_string(v, k)? {
        None => Ok(d),
        Some(s) => {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(c),
                _ => hpy(format!("invalid value: string {s:?}, expected a character")),
            }
        }
    }
}

// ---------- components ----------

// serde's required fields: a tokenizer.json without one does not load in tokenizers (an error whose text names
// tokenizers' wrapper enums and a JSON position), so Brevi refuses it: CPython's tokenizers reports it
fn need(v: &J, kind: &str) -> HR<()> {
    let what = v.tname();
    let fields: &[&str] = match (kind, what) {
        ("n", "BertNormalizer") => &["clean_text", "handle_chinese_chars", "lowercase"],
        ("n", "Strip") => &["strip_left", "strip_right"],
        ("n" | "d", "Replace") => &["pattern", "content"],
        ("n", "Prepend") => &["prepend"],
        ("p" | "pp" | "d", "ByteLevel") => &["add_prefix_space", "trim_offsets"],
        ("p", "Split") => &["pattern", "behavior", "invert"],
        ("p", "Digits") => &["individual_digits"],
        ("p" | "d", "Metaspace") => &["replacement"],
        ("p", "CharDelimiterSplit") => &["delimiter"],
        ("pp", "BertProcessing") => &["sep", "cls"],
        ("pp", "RobertaProcessing") => &["sep", "cls", "trim_offsets", "add_prefix_space"],
        ("pp", "TemplateProcessing") => &["single", "pair", "special_tokens"],
        ("d", "WordPiece") => &["prefix", "cleanup"],
        ("d", "Strip") => &["content", "start", "stop"],
        ("d", "BPEDecoder") => &["suffix"],
        ("d", "CTC") => &["pad_token", "word_delimiter_token", "cleanup"],
        _ => &[],
    };
    for f in fields {
        if v.get(f).is_none() {
            return hlacks(format!("a tokenizer.json that tokenizers does not load ({what} without {f})"));
        }
    }
    Ok(())
}

// Whether a pattern uses what Oniguruma reads unlike the regex crate's tables: \w \W \b \B, and ^ $ outside a
// class (Oniguruma's match at line breaks)
fn onig_differs(r: &str) -> bool {
    let cs: Vec<char> = r.chars().collect();
    let mut i = 0;
    let mut class = false;
    let mut class_start = 0;
    while i < cs.len() {
        match cs[i] {
            '\\' => {
                if matches!(cs.get(i + 1), Some('w' | 'W' | 'b' | 'B')) {
                    return true;
                }
                i += 2;
                continue;
            }
            '[' if !class => {
                class = true;
                class_start = i + 1;
                if cs.get(i + 1) == Some(&'^') {
                    class_start += 1;
                    i += 1;
                }
            }
            ']' if class && i != class_start => class = false,
            '^' | '$' if !class => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

// a Split / Replace pattern: {"String": s} or {"Regex": r}, the regex run by Oniguruma in tokenizers
fn hj_pattern(v: Option<&J>) -> HR<HPat> {
    let Some(v) = v else {
        return hlacks("tokenizer.json pattern missing");
    };
    if let Some(s) = v.get("String").and_then(|s| s.str_()) {
        return Ok(HPat::Str(s.to_string()));
    }
    if let Some(r) = v.get("Regex").and_then(|s| s.str_()) {
        // Oniguruma's \w (and its ^ $, which match at line breaks) differ from the regex crate's
        if onig_differs(r) {
            return hlacks(format!("tokenizers Regex {r:?} (\\w, \\b, ^ and $ in Oniguruma)"));
        }
        return match tk_compile(r) {
            Ok(re) => Ok(HPat::Re(Rc::new(re))),
            Err(m) => hlacks(format!("tokenizers Regex {r:?} ({m})")),
        };
    }
    hlacks("tokenizer.json pattern that is neither String nor Regex")
}

fn hj_behavior(v: &J, k: &str, d: Behavior) -> HR<Behavior> {
    match v.get(k).and_then(|x| x.str_()) {
        None => Ok(d),
        Some("Removed") => Ok(Behavior::Removed),
        Some("Isolated") => Ok(Behavior::Isolated),
        Some("MergedWithPrevious") => Ok(Behavior::MergedWithPrevious),
        Some("MergedWithNext") => Ok(Behavior::MergedWithNext),
        Some("Contiguous") => Ok(Behavior::Contiguous),
        Some(b) => hpy(format!("unknown variant `{b}`, expected one of `Removed`, `Isolated`, `MergedWithPrevious`, `MergedWithNext`, `Contiguous`")),
    }
}

pub enum HNorm {
    Seq(Vec<HNorm>),
    Bert { clean: bool, chinese: bool, strip: Option<bool>, lower: bool },
    Strip(bool, bool),
    StripAccents,
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
    Lower,
    Replace(HPat, String),
    Prepend(String),
    ByteLevel,
}

fn hn_load(v: &J) -> HR<HNorm> {
    need(v, "n")?;
    Ok(match v.tname() {
        "Sequence" => {
            let Some(J::Arr(xs)) = v.get("normalizers") else {
                return hlacks("normalizers.Sequence without normalizers");
            };
            HNorm::Seq(xs.iter().map(hn_load).collect::<HR<_>>()?)
        }
        "BertNormalizer" => HNorm::Bert {
            clean: hj_bool(v, "clean_text", true)?,
            chinese: hj_bool(v, "handle_chinese_chars", true)?,
            strip: match v.get("strip_accents") {
                None | Some(J::Null) => None,
                Some(J::Bool(b)) => Some(*b),
                _ => return hlacks("BertNormalizer strip_accents"),
            },
            lower: hj_bool(v, "lowercase", true)?,
        },
        "Strip" => HNorm::Strip(hj_bool(v, "strip_left", true)?, hj_bool(v, "strip_right", true)?),
        "StripAccents" => HNorm::StripAccents,
        "NFC" => HNorm::Nfc,
        "NFD" => HNorm::Nfd,
        "NFKC" => HNorm::Nfkc,
        "NFKD" => HNorm::Nfkd,
        "Lowercase" => HNorm::Lower,
        "Replace" => HNorm::Replace(hj_pattern(v.get("pattern"))?, hj_string(v, "content")?.unwrap_or_default()),
        "Prepend" => HNorm::Prepend(hj_string(v, "prepend")?.unwrap_or_default()),
        "ByteLevel" => HNorm::ByteLevel,
        t => return hlacks(format!("tokenizers normalizer {}", if t.is_empty() { "without a type" } else { t })),
    })
}

fn is_chinese(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x2A700..=0x2B73F | 0x2B740..=0x2B81F | 0x2B920..=0x2CEAF | 0xF900..=0xFAFF | 0x2F800..=0x2FA1F)
}

fn hbl_chars() -> &'static [char; 256] {
    static T: std::sync::OnceLock<[char; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut bs: Vec<u32> = (b'!' as u32..=b'~' as u32).chain(0xA1..=0xAC).chain(0xAE..=0xFF).collect();
        let mut cs = bs.clone();
        let mut n = 0;
        for b in 0..256u32 {
            if !bs.contains(&b) {
                bs.push(b);
                cs.push(256 + n);
                n += 1;
            }
        }
        let mut t = ['\0'; 256];
        for (b, c) in bs.into_iter().zip(cs) {
            t[b as usize] = char::from_u32(c).unwrap();
        }
        t
    })
}
fn hbl_byte(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x21..=0x7E | 0xA1..=0xAC | 0xAE..=0xFF => Some(u as u8),
        0x100..=0x143 => hbl_chars().iter().position(|&x| x == c).map(|b| b as u8),
        _ => None,
    }
}

fn hbl_transform(n: &mut NStr) {
    let t = hbl_chars();
    let s = n.get();
    let mut tr: Vec<(char, isize)> = Vec::with_capacity(s.len());
    for (_, ch) in s.char_indices() {
        let mut buf = [0u8; 4];
        for (i, b) in ch.encode_utf8(&mut buf).bytes().enumerate() {
            tr.push((t[b as usize], (i > 0) as isize));
        }
    }
    n.transform(&tr, 0);
}

impl HNorm {
    pub fn normalize(&self, n: &mut NStr) {
        match self {
            HNorm::Seq(xs) => xs.iter().for_each(|x| x.normalize(n)),
            HNorm::Bert { clean, chinese, strip, lower } => {
                if *clean {
                    n.filter(|c| !(c == '\0' || c == '\u{fffd}' || (!matches!(c, '\t' | '\n' | '\r') && hc_control(c))));
                    n.map(|c| if matches!(c, '\t' | '\n' | '\r') || hc_ws(c) { ' ' } else { c });
                }
                if *chinese {
                    let mut tr = vec![];
                    for c in n.get().chars() {
                        if is_chinese(c) {
                            tr.extend([(' ', 0), (c, 1), (' ', 1)]);
                        } else {
                            tr.push((c, 0));
                        }
                    }
                    n.transform(&tr, 0);
                }
                if strip.unwrap_or(*lower) {
                    n.nfd();
                    n.filter(|c| !hc_mn(c));
                }
                if *lower {
                    n.lowercase();
                }
            }
            HNorm::Strip(l, r) => n.lrstrip(*l, *r),
            HNorm::StripAccents => n.filter(|c| !hc_comb(c)),
            HNorm::Nfc => n.nfc(),
            HNorm::Nfd => n.nfd(),
            HNorm::Nfkc => n.nfkc(),
            HNorm::Nfkd => n.nfkd(),
            HNorm::Lower => n.lowercase(),
            HNorm::Replace(p, c) => n.replace(p, c),
            HNorm::Prepend(p) => {
                if !n.is_empty() {
                    n.prepend(p)
                }
            }
            HNorm::ByteLevel => {
                if !n.is_empty() {
                    hbl_transform(n)
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Prepend {
    First,
    Never,
    Always,
}

pub enum HPre {
    Seq(Vec<HPre>),
    ByteLevel { prefix: bool, regex: bool },
    Split { pat: HPat, beh: Behavior },
    Digits(bool),
    Metaspace { rep: char, scheme: Prepend, split: bool },
    Whitespace,
    WhitespaceSplit,
    Bert,
    Punct(Behavior),
    Delim(char),
}

fn hscheme(v: &J) -> HR<Prepend> {
    let scheme = match v.get("prepend_scheme").and_then(|x| x.str_()) {
        None => Prepend::Always,
        Some("first") => Prepend::First,
        Some("never") => Prepend::Never,
        Some("always") => Prepend::Always,
        Some(s) => return hpy(format!("unknown variant `{s}`, expected one of `first`, `never`, `always`")),
    };
    if let Some(J::Bool(false)) = v.get("add_prefix_space") {
        if scheme != Prepend::Never {
            return hpy("add_prefix_space does not match declared prepend_scheme");
        }
    }
    Ok(scheme)
}

fn hp_load(v: &J) -> HR<HPre> {
    need(v, "p")?;
    Ok(match v.tname() {
        "Sequence" => {
            let Some(J::Arr(xs)) = v.get("pretokenizers") else {
                return hlacks("pre_tokenizers.Sequence without pretokenizers");
            };
            HPre::Seq(xs.iter().map(hp_load).collect::<HR<_>>()?)
        }
        "ByteLevel" => HPre::ByteLevel { prefix: hj_bool(v, "add_prefix_space", true)?, regex: hj_bool(v, "use_regex", true)? },
        "Split" => {
            let pat = hj_pattern(v.get("pattern"))?;
            let pat = if hj_bool(v, "invert", false)? { HPat::Invert(Box::new(pat)) } else { pat };
            HPre::Split { pat, beh: hj_behavior(v, "behavior", Behavior::Isolated)? }
        }
        "Digits" => HPre::Digits(hj_bool(v, "individual_digits", false)?),
        "Metaspace" => HPre::Metaspace { rep: hj_char(v, "replacement", '▁')?, scheme: hscheme(v)?, split: hj_bool(v, "split", true)? },
        "Whitespace" => HPre::Whitespace,
        "WhitespaceSplit" => HPre::WhitespaceSplit,
        "BertPreTokenizer" => HPre::Bert,
        "Punctuation" => HPre::Punct(hj_behavior(v, "behavior", Behavior::Isolated)?),
        "CharDelimiterSplit" => HPre::Delim(hj_char(v, "delimiter", ' ')?),
        t => return hlacks(format!("tokenizers pre-tokenizer {}", if t.is_empty() { "without a type" } else { t })),
    })
}

fn hbl_regex() -> Rc<TkRe> {
    thread_local! {
        static RE: Rc<TkRe> = Rc::new(tk_compile(r"'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+").unwrap());
    }
    RE.with(|r| r.clone())
}

// the regex crate's \w+|[^\w\s]+ (the Whitespace pre-tokenizer), as the matches of a char test
fn hws_matches(inside: &str) -> Vec<(Offs, bool)> {
    if inside.is_empty() {
        return vec![((0, 0), false)];
    }
    let mut out = vec![];
    let mut prev = 0;
    let cs: Vec<(usize, char)> = inside.char_indices().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i].1;
        let kind = if hc_rew(c) { 1 } else if hc_res(c) { 0 } else { 2 };
        if kind == 0 {
            i += 1;
            continue;
        }
        let st = cs[i].0;
        let mut j = i + 1;
        while j < cs.len() {
            let d = cs[j].1;
            let k = if hc_rew(d) { 1 } else if hc_res(d) { 0 } else { 2 };
            if k != kind {
                break;
            }
            j += 1;
        }
        let en = if j < cs.len() { cs[j].0 } else { inside.len() };
        if prev != st {
            out.push(((prev, st), false));
        }
        out.push(((st, en), true));
        prev = en;
        i = j;
    }
    if prev != inside.len() {
        out.push(((prev, inside.len()), false));
    }
    out
}

pub struct PSplit {
    pub n: NStr,
    pub toks: Option<Vec<HToken>>,
}

#[derive(Clone)]
pub struct HToken {
    pub id: u32,
    pub value: String,
    pub offs: Offs,
}

pub struct PreStr {
    pub original: String,
    pub splits: Vec<PSplit>,
}

impl PreStr {
    fn new(s: &str) -> PreStr {
        PreStr { original: s.to_string(), splits: vec![PSplit { n: NStr::from_str(s), toks: None }] }
    }
    fn split(&mut self, mut f: impl FnMut(NStr) -> Vec<PSplit>) {
        let mut out = Vec::with_capacity(self.splits.len());
        for sp in self.splits.drain(..) {
            if sp.toks.is_some() {
                out.push(sp);
                continue;
            }
            out.extend(f(sp.n).into_iter().filter(|s| !s.n.is_empty()));
        }
        self.splits = out;
    }
    fn split_pat(&mut self, pat: &HPat, beh: Behavior) {
        self.split(|n| n.split(pat, beh).into_iter().map(|n| PSplit { n, toks: None }).collect())
    }
    fn normalize(&mut self, f: impl Fn(&mut NStr)) {
        for sp in self.splits.iter_mut().filter(|s| s.toks.is_none()) {
            f(&mut sp.n);
        }
    }
}

impl HPre {
    fn pre_tokenize(&self, p: &mut PreStr) {
        match self {
            HPre::Seq(xs) => xs.iter().for_each(|x| x.pre_tokenize(p)),
            HPre::ByteLevel { prefix, regex } => {
                let re = HPat::Re(hbl_regex());
                p.split(|mut n| {
                    if *prefix && !n.get().starts_with(' ') {
                        n.prepend(" ");
                    }
                    let parts = if *regex { n.split(&re, Behavior::Isolated) } else { vec![n] };
                    parts.into_iter().map(|n| PSplit { n, toks: None }).collect()
                });
                p.normalize(hbl_transform);
            }
            HPre::Split { pat, beh } => p.split_pat(pat, *beh),
            HPre::Digits(ind) => p.split_pat(&HPat::Fn(hc_num), if *ind { Behavior::Isolated } else { Behavior::Contiguous }),
            HPre::Metaspace { rep, scheme, split } => {
                let rs = rep.to_string();
                p.split(|mut n| {
                    n.replace(&HPat::Char(' '), &rs);
                    match scheme {
                        Prepend::Always => {
                            if !n.get().starts_with(*rep) {
                                n.prepend(&rs);
                            }
                        }
                        Prepend::First => {
                            if !n.get().starts_with(*rep) && n.offsets_original().0 == 0 {
                                n.prepend(&rs);
                            }
                        }
                        Prepend::Never => {}
                    }
                    let parts = if *split { n.split(&HPat::Char(*rep), Behavior::MergedWithNext) } else { vec![n] };
                    parts.into_iter().map(|n| PSplit { n, toks: None }).collect()
                });
            }
            HPre::Whitespace => p.split(|n| {
                let ms: Vec<(Offs, bool)> = hws_matches(n.get()).into_iter().map(|(o, m)| (o, !m)).collect();
                hsplit_by(&n, ms, Behavior::Removed)
            }),
            HPre::WhitespaceSplit => p.split_pat(&HPat::Fn(hc_ws), Behavior::Removed),
            HPre::Bert => {
                p.split_pat(&HPat::Fn(hc_ws), Behavior::Removed);
                p.split_pat(&HPat::Fn(hc_punct), Behavior::Isolated);
            }
            HPre::Punct(b) => p.split_pat(&HPat::Fn(hc_punct), *b),
            HPre::Delim(c) => p.split_pat(&HPat::Char(*c), Behavior::Removed),
        }
    }
}

// NormalizedString::split over matches already found
fn hsplit_by(n: &NStr, ms: Vec<(Offs, bool)>, beh: Behavior) -> Vec<PSplit> {
    n.split_found(ms, beh).into_iter().map(|n| PSplit { n, toks: None }).collect()
}

// ---------- models ----------

pub struct Bpe {
    vocab: HashMap<String, u32>,
    vocab_r: HashMap<u32, String>,
    merges: HashMap<(u32, u32), (u32, u32)>,
    unk: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    fuse_unk: bool,
    byte_fallback: bool,
    ignore_merges: bool,
    cache: std::cell::RefCell<HashMap<String, Vec<(u32, usize)>>>,
}

pub struct WordPiece {
    vocab: HashMap<String, u32>,
    vocab_r: HashMap<u32, String>,
    unk: String,
    prefix: String,
    max_chars: usize,
}

pub struct WordLevel {
    vocab: HashMap<String, u32>,
    vocab_r: HashMap<u32, String>,
    unk: String,
}

pub enum HModel {
    Bpe(Bpe),
    WordPiece(WordPiece),
    WordLevel(WordLevel),
}

fn hvocab(v: &J) -> HR<(HashMap<String, u32>, HashMap<u32, String>)> {
    let Some(J::Obj(kv)) = v.get("vocab") else {
        return hlacks("a tokenizers model without a vocab dict");
    };
    let mut vocab = HashMap::with_capacity(kv.len());
    for (k, i) in kv {
        vocab.insert(k.clone(), hj_u32(i)?);
    }
    let mut vocab_r = HashMap::with_capacity(vocab.len());
    for (k, i) in &vocab {
        if vocab_r.insert(*i, k.clone()).is_some() {
            // tokenizers keeps one of the two, by hash order
            return hlacks("a tokenizers vocab with two tokens on one id");
        }
    }
    Ok((vocab, vocab_r))
}

fn hm_load(v: &J) -> HR<HModel> {
    let t = match v.tname() {
        "" if v.get("merges").is_some() => "BPE",
        "" if v.get("max_input_chars_per_word").is_some() => "WordPiece",
        t => t,
    };
    Ok(match t {
        "BPE" => {
            if let Some(d) = v.opt("dropout") {
                if !matches!(d, J::Int(0)) && !matches!(d, J::Num(f) if *f == 0.0) {
                    return hlacks("BPE dropout (random merges)");
                }
            }
            let (vocab, vocab_r) = hvocab(v)?;
            let prefix = hj_string(v, "continuing_subword_prefix")?;
            let mut pairs: Vec<(String, String)> = vec![];
            match v.get("merges") {
                Some(J::Arr(ms)) => {
                    let mut rank = 0;
                    for m in ms {
                        match m {
                            J::Str(l) => {
                                if l.starts_with("#version") {
                                    continue;
                                }
                                rank += 1;
                                let parts: Vec<&str> = l.split(' ').collect();
                                if parts.len() != 2 {
                                    return hpy(format!("Merges text file invalid at line {rank}"));
                                }
                                pairs.push((parts[0].to_string(), parts[1].to_string()));
                            }
                            J::Arr(ab) if ab.len() == 2 => match (&ab[0], &ab[1]) {
                                (J::Str(a), J::Str(b)) => pairs.push((a.clone(), b.clone())),
                                _ => return hlacks("BPE merges"),
                            },
                            _ => return hlacks("BPE merges"),
                        }
                    }
                }
                _ => return hpy("Missing vocab/merges"),
            }
            let plen = prefix.as_ref().map_or(0, |p| p.len());
            let mut merges = HashMap::with_capacity(pairs.len());
            for (i, (a, b)) in pairs.iter().enumerate() {
                let oov = |t: &str| format!("Token `{t}` out of vocabulary");
                let a_id = *vocab.get(a).ok_or_else(|| HfErr::Py(oov(a)))?;
                let b_id = *vocab.get(b).ok_or_else(|| HfErr::Py(oov(b)))?;
                let Some(rest) = b.get(plen..) else {
                    return hlacks("BPE merges shorter than continuing_subword_prefix (tokenizers panics)");
                };
                let new = format!("{a}{rest}");
                let n_id = *vocab.get(&new).ok_or_else(|| HfErr::Py(oov(&new)))?;
                merges.insert((a_id, b_id), (i as u32, n_id));
            }
            HModel::Bpe(Bpe {
                vocab,
                vocab_r,
                merges,
                unk: hj_string(v, "unk_token")?,
                prefix,
                suffix: hj_string(v, "end_of_word_suffix")?,
                fuse_unk: hj_bool(v, "fuse_unk", false)?,
                byte_fallback: hj_bool(v, "byte_fallback", false)?,
                ignore_merges: hj_bool(v, "ignore_merges", false)?,
                cache: Default::default(),
            })
        }
        "WordPiece" => {
            let (vocab, vocab_r) = hvocab(v)?;
            HModel::WordPiece(WordPiece {
                vocab,
                vocab_r,
                unk: hj_string(v, "unk_token")?.unwrap_or_else(|| "[UNK]".into()),
                prefix: hj_string(v, "continuing_subword_prefix")?.unwrap_or_else(|| "##".into()),
                max_chars: hj_usize(v, "max_input_chars_per_word", 100)?,
            })
        }
        "WordLevel" => {
            let (vocab, vocab_r) = hvocab(v)?;
            HModel::WordLevel(WordLevel { vocab, vocab_r, unk: hj_string(v, "unk_token")?.unwrap_or_else(|| "<unk>".into()) })
        }
        t => return hlacks(format!("tokenizers model {}", if t.is_empty() { "without a type" } else { t })),
    })
}

#[derive(Clone, Copy)]
struct HSym {
    c: u32,
    prev: isize,
    next: isize,
    len: usize,
}

#[derive(PartialEq, Eq)]
struct HMerge {
    pos: usize,
    rank: u32,
    new_id: u32,
}
impl Ord for HMerge {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        if self.rank != o.rank {
            o.rank.cmp(&self.rank)
        } else {
            o.pos.cmp(&self.pos)
        }
    }
}
impl PartialOrd for HMerge {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Bpe {
    fn merge_word(&self, w: &str) -> HR<Vec<(u32, usize)>> {
        let mut syms: Vec<HSym> = Vec::with_capacity(w.len());
        let add = |syms: &mut Vec<HSym>, c: u32, len: usize| {
            let n = syms.len() as isize;
            let prev = if let Some(l) = syms.last_mut() {
                l.next = n;
                n - 1
            } else {
                -1
            };
            syms.push(HSym { c, prev, next: -1, len });
        };
        let mut unk: Option<(u32, usize)> = None;
        let idx: Vec<usize> = w.char_indices().map(|(i, _)| i).collect();
        for (k, &i) in idx.iter().enumerate() {
            let end = idx.get(k + 1).copied();
            let mut s = w[i..end.unwrap_or(w.len())].to_string();
            let byte_len = s.len();
            if i != 0 {
                if let Some(p) = &self.prefix {
                    s = format!("{p}{s}");
                }
            }
            if end.is_none() {
                if let Some(x) = &self.suffix {
                    s.push_str(x);
                }
            }
            if let Some(id) = self.vocab.get(&s) {
                if let Some((u, l)) = unk.take() {
                    add(&mut syms, u, l);
                }
                add(&mut syms, *id, byte_len);
            } else {
                if self.byte_fallback {
                    let ids: Option<Vec<u32>> = s.bytes().map(|b| self.vocab.get(&format!("<0x{b:02X}>")).copied()).collect();
                    if let Some(ids) = ids {
                        for t in ids {
                            add(&mut syms, t, 1);
                        }
                        continue;
                    }
                }
                if let Some(u) = &self.unk {
                    let uid = || self.vocab.get(u).copied().ok_or_else(|| HfErr::Py(format!("Unk token `{u}` not found in the vocabulary")));
                    unk = match (unk, self.fuse_unk) {
                        (Some((id, l)), true) => Some((id, l + byte_len)),
                        (Some((id, l)), false) => {
                            add(&mut syms, id, l);
                            Some((uid()?, byte_len))
                        }
                        _ => Some((uid()?, byte_len)),
                    };
                }
            }
        }
        if let Some((u, l)) = unk {
            add(&mut syms, u, l);
        }
        // Word::merge_all
        let mut q = std::collections::BinaryHeap::with_capacity(syms.len());
        for i in 0..syms.len().saturating_sub(1) {
            if let Some(m) = self.merges.get(&(syms[i].c, syms[i + 1].c)) {
                q.push(HMerge { pos: i, rank: m.0, new_id: m.1 });
            }
        }
        while let Some(top) = q.pop() {
            if syms[top.pos].len == 0 || syms[top.pos].next == -1 {
                continue;
            }
            let next_pos = syms[top.pos].next as usize;
            let right = syms[next_pos];
            if self.merges.get(&(syms[top.pos].c, right.c)).is_none_or(|m| m.1 != top.new_id) {
                continue;
            }
            syms[top.pos].c = top.new_id;
            syms[top.pos].len += right.len;
            syms[top.pos].next = right.next;
            syms[next_pos].len = 0;
            if right.next > -1 && (right.next as usize) < syms.len() {
                syms[right.next as usize].prev = top.pos as isize;
            }
            let cur = syms[top.pos];
            if cur.prev >= 0 {
                let p = cur.prev as usize;
                if let Some(m) = self.merges.get(&(syms[p].c, cur.c)) {
                    q.push(HMerge { pos: p, rank: m.0, new_id: m.1 });
                }
            }
            let nx = cur.next as usize;
            if cur.next >= 0 && nx < syms.len() {
                if let Some(m) = self.merges.get(&(cur.c, syms[nx].c)) {
                    q.push(HMerge { pos: top.pos, rank: m.0, new_id: m.1 });
                }
            }
        }
        Ok(syms.into_iter().filter(|s| s.len != 0).map(|s| (s.c, s.len)).collect())
    }
    fn tokenize(&self, seq: &str) -> HR<Vec<HToken>> {
        if seq.is_empty() {
            return Ok(vec![]);
        }
        if self.ignore_merges {
            if let Some(id) = self.vocab.get(seq) {
                return Ok(vec![HToken { id: *id, value: seq.to_string(), offs: (0, seq.len()) }]);
            }
        }
        let cached = self.cache.borrow().get(seq).cloned();
        let word = match cached {
            Some(w) => w,
            None => {
                let w = self.merge_word(seq)?;
                let mut c = self.cache.borrow_mut();
                if seq.len() < 256 && c.len() < 10_000 {
                    c.insert(seq.to_string(), w.clone());
                }
                w
            }
        };
        let mut pos = 0;
        Ok(word
            .into_iter()
            .map(|(id, len)| {
                let t = HToken { id, value: self.vocab_r[&id].clone(), offs: (pos, pos + len) };
                pos += len;
                t
            })
            .collect())
    }
}

impl HModel {
    fn tokenize(&self, seq: &str) -> HR<Vec<HToken>> {
        match self {
            HModel::Bpe(b) => b.tokenize(seq),
            HModel::WordPiece(w) => {
                let unk = || -> HR<Vec<HToken>> {
                    let id = w.vocab.get(&w.unk).copied().ok_or_else(|| HfErr::Py("WordPiece error: Missing [UNK] token from the vocabulary".into()))?;
                    Ok(vec![HToken { id, value: w.unk.clone(), offs: (0, seq.len()) }])
                };
                if seq.chars().count() > w.max_chars {
                    return unk();
                }
                let mut start = 0;
                let mut subs = vec![];
                while start < seq.len() {
                    let mut end = seq.len();
                    let mut cur = None;
                    while start < end {
                        let mut sub = seq[start..end].to_string();
                        if start > 0 {
                            sub = format!("{}{sub}", w.prefix);
                        }
                        if let Some(id) = w.vocab.get(&sub) {
                            cur = Some(HToken { id: *id, value: sub, offs: (start, end) });
                            break;
                        }
                        end -= sub.chars().last().map_or(1, |c| c.len_utf8());
                    }
                    match cur {
                        None => return unk(),
                        Some(t) => subs.push(t),
                    }
                    start = end;
                }
                Ok(subs)
            }
            HModel::WordLevel(w) => {
                if let Some(id) = w.vocab.get(seq) {
                    Ok(vec![HToken { id: *id, value: seq.to_string(), offs: (0, seq.len()) }])
                } else if let Some(id) = w.vocab.get(&w.unk) {
                    Ok(vec![HToken { id: *id, value: w.unk.clone(), offs: (0, seq.len()) }])
                } else {
                    hpy("WordLevel error: Missing [UNK] token from the vocabulary")
                }
            }
        }
    }
    fn vocab(&self) -> &HashMap<String, u32> {
        match self {
            HModel::Bpe(b) => &b.vocab,
            HModel::WordPiece(w) => &w.vocab,
            HModel::WordLevel(w) => &w.vocab,
        }
    }
    fn vocab_r(&self) -> &HashMap<u32, String> {
        match self {
            HModel::Bpe(b) => &b.vocab_r,
            HModel::WordPiece(w) => &w.vocab_r,
            HModel::WordLevel(w) => &w.vocab_r,
        }
    }
}

// ---------- Encoding ----------

#[derive(Clone, Default)]
pub struct HEnc {
    pub ids: Vec<u32>,
    pub type_ids: Vec<u32>,
    pub tokens: Vec<String>,
    pub words: Vec<Option<u32>>,
    pub offsets: Vec<Offs>,
    pub special: Vec<u32>,
    pub attention: Vec<u32>,
    pub overflowing: Vec<HEnc>,
    // sequence id -> range; tokenizers keeps these in a hash map
    pub ranges: Vec<(usize, (usize, usize))>,
}

impl HEnc {
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    fn set_seq(&mut self, id: usize) {
        let n = self.len();
        match self.ranges.iter_mut().find(|r| r.0 == id) {
            Some(r) => r.1 = (0, n),
            None => self.ranges.push((id, (0, n))),
        }
    }
    pub fn n_sequences(&self) -> usize {
        if self.ranges.is_empty() {
            1
        } else {
            self.ranges.len()
        }
    }
    pub fn seq_range(&self, id: usize) -> (usize, usize) {
        self.ranges.iter().find(|r| r.0 == id).map_or((0, self.len()), |r| r.1)
    }
    pub fn sequence_ids(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.len()];
        for id in 0..self.n_sequences() {
            let (a, b) = self.seq_range(id);
            for x in out.iter_mut().take(b.min(self.len())).skip(a) {
                *x = Some(id);
            }
        }
        out
    }
    pub fn token_to_sequence(&self, t: usize) -> Option<usize> {
        if t > self.len() {
            None
        } else if self.ranges.is_empty() {
            Some(0)
        } else {
            self.ranges.iter().find(|(_, (a, b))| t >= *a && t < *b).map(|r| r.0)
        }
    }
    pub fn word_to_tokens(&self, word: u32, seq: usize) -> Option<(usize, usize)> {
        let (a, b) = self.seq_range(seq);
        let ws = self.words.get(a..b)?;
        let (mut st, mut en) = (None, None);
        for (i, w) in ws.iter().enumerate() {
            if !(*w <= Some(word)) {
                break;
            }
            if *w == Some(word) {
                if st.is_none() || Some(i) < st {
                    st = Some(i);
                }
                if en.is_none() || Some(i) >= en {
                    en = Some(i + 1);
                }
            }
        }
        match (st, en) {
            (Some(s), Some(e)) => Some((a + s, a + e)),
            _ => None,
        }
    }
    pub fn word_to_chars(&self, word: u32, seq: usize) -> Option<Offs> {
        self.word_to_tokens(word, seq).and_then(|(s, e)| if e == 0 { None } else { Some((self.offsets[s].0, self.offsets[e - 1].1)) })
    }
    pub fn token_to_chars(&self, t: usize) -> Option<Offs> {
        self.token_to_sequence(t)?;
        self.offsets.get(t).copied()
    }
    pub fn token_to_word(&self, t: usize) -> Option<u32> {
        self.token_to_sequence(t)?;
        self.words.get(t).copied().flatten()
    }
    pub fn char_to_token(&self, pos: usize, seq: usize) -> Option<usize> {
        let (a, b) = self.seq_range(seq);
        self.offsets.get(a..b)?.iter().position(|(s, e)| pos >= *s && pos < *e).map(|p| a + p)
    }
    pub fn char_to_word(&self, pos: usize, seq: usize) -> Option<u32> {
        self.char_to_token(pos, seq).and_then(|t| self.token_to_word(t))
    }
    fn slice(&self, s: usize, e: usize) -> HEnc {
        HEnc {
            ids: self.ids[s..e].to_vec(),
            type_ids: self.type_ids[s..e].to_vec(),
            tokens: self.tokens[s..e].to_vec(),
            words: self.words[s..e].to_vec(),
            offsets: self.offsets[s..e].to_vec(),
            special: self.special[s..e].to_vec(),
            attention: self.attention[s..e].to_vec(),
            overflowing: vec![],
            ranges: vec![],
        }
    }
    pub fn truncate(&mut self, max_len: usize, stride: usize, left: bool) -> HR<()> {
        let n = self.len();
        if max_len >= n {
            return Ok(());
        }
        if max_len == 0 {
            let o = std::mem::take(self);
            self.overflowing.push(o);
            return Ok(());
        }
        if stride >= max_len {
            return hlacks("truncation with stride >= max_length (tokenizers panics)");
        }
        self.ranges.clear();
        let offset = max_len - stride;
        let mut end = false;
        let mut parts = vec![];
        if !left {
            let mut start = 0;
            while start < n {
                if !end {
                    let stop = (start + max_len).min(n);
                    end = stop == n;
                    parts.push((start, stop));
                }
                start += offset;
            }
        } else {
            let mut k = n as isize - 1;
            while k >= 0 {
                let stop = k as usize + 1;
                let start = stop.saturating_sub(max_len);
                if start < stop && !end {
                    end = start == 0;
                    parts.push((start, stop));
                }
                k -= offset as isize;
            }
        }
        let mut ne = self.slice(parts[0].0, parts[0].1);
        for &(s, e) in &parts[1..] {
            ne.overflowing.push(self.slice(s, e));
        }
        *self = ne;
        Ok(())
    }
    pub fn merge_with(&mut self, pair: HEnc, growing: bool) {
        let mut ov = vec![];
        for so in &self.overflowing {
            let mut x = so.clone();
            x.merge_with(pair.clone(), growing);
            ov.push(x);
            for po in &pair.overflowing {
                let mut x = so.clone();
                x.merge_with(po.clone(), growing);
                ov.push(x);
            }
        }
        for po in &pair.overflowing {
            let mut x = self.clone();
            x.merge_with(po.clone(), growing);
            ov.push(x);
        }
        let n = self.len();
        for (id, (a, b)) in pair.ranges {
            let r = (n + a, n + b);
            match self.ranges.iter_mut().find(|x| x.0 == id) {
                Some(x) => x.1 = r,
                None => self.ranges.push((id, r)),
            }
        }
        self.ids.extend(pair.ids);
        self.type_ids.extend(pair.type_ids);
        self.tokens.extend(pair.tokens);
        self.words.extend(pair.words);
        let st = if growing { self.offsets.last().map_or(0, |o| o.1) } else { 0 };
        self.offsets.extend(pair.offsets.into_iter().map(|(a, b)| (a + st, b + st)));
        self.special.extend(pair.special);
        self.attention.extend(pair.attention);
        self.overflowing = ov;
    }
    pub fn merge(encs: Vec<HEnc>, growing: bool) -> HEnc {
        let mut e = HEnc::default();
        for x in encs {
            e.merge_with(x, growing);
        }
        e
    }
    pub fn pad(&mut self, target: usize, pad_id: u32, pad_type: u32, pad_tok: &str, left: bool) {
        for o in self.overflowing.iter_mut() {
            o.pad(target, pad_id, pad_type, pad_tok, left);
        }
        if self.len() >= target {
            return;
        }
        let k = target - self.len();
        fn put<T: Clone>(v: &mut Vec<T>, x: T, k: usize, left: bool) {
            if left {
                v.splice(0..0, std::iter::repeat_n(x, k));
            } else {
                v.extend(std::iter::repeat_n(x, k));
            }
        }
        put(&mut self.ids, pad_id, k, left);
        put(&mut self.type_ids, pad_type, k, left);
        put(&mut self.tokens, pad_tok.to_string(), k, left);
        put(&mut self.words, None, k, left);
        put(&mut self.attention, 0, k, left);
        put(&mut self.special, 1, k, left);
        put(&mut self.offsets, (0, 0), k, left);
        if left {
            for r in self.ranges.iter_mut() {
                r.1 = (r.1 .0 + k, r.1 .1 + k);
            }
        }
    }
    // ByteLevel's process_offsets
    fn trim_offsets(&mut self, prefix: bool) {
        let sp = hbl_chars()[b' ' as usize];
        for (i, (tok, o)) in self.tokens.iter().zip(self.offsets.iter_mut()).enumerate() {
            let isw = |c: &char| *c == sp || hc_ws(*c);
            let mut lead = tok.chars().take_while(isw).count();
            let trail = tok.chars().rev().take_while(isw).count();
            if lead > 0 || trail > 0 {
                if lead > 0 {
                    let first = i == 0 || o.0 == 0;
                    if first && prefix && lead == 1 {
                        lead = 0;
                    }
                    o.0 = (o.0 + lead).min(o.1);
                }
                if trail > 0 && o.1 >= trail {
                    o.1 = (o.1 - trail).max(o.0);
                }
            }
        }
    }
}

// ---------- post-processors ----------

pub struct HSpecial {
    ids: Vec<u32>,
    tokens: Vec<String>,
}

#[derive(Clone)]
pub enum Piece {
    Seq(bool, u32),
    Special(String, u32),
}

pub enum HPost {
    Bert { sep: (String, u32), cls: (String, u32) },
    Roberta { sep: (String, u32), cls: (String, u32), trim: bool, prefix: bool },
    ByteLevel { trim: bool, prefix: bool },
    Template { single: Vec<Piece>, pair: Vec<Piece>, specials: HashMap<String, HSpecial>, n_single: usize, n_pair: usize },
    Seq(Vec<HPost>),
}

fn hj_tokid(v: Option<&J>, d: (&str, u32)) -> HR<(String, u32)> {
    match v {
        None => Ok((d.0.to_string(), d.1)),
        Some(J::Arr(x)) if x.len() == 2 => match (&x[0], &x[1]) {
            (J::Str(s), i) => Ok((s.clone(), hj_u32(i)?)),
            _ => hlacks("a post-processor token"),
        },
        _ => hlacks("a post-processor token"),
    }
}

fn hpieces(v: Option<&J>) -> HR<Vec<Piece>> {
    let Some(J::Arr(xs)) = v else {
        return hlacks("a TemplateProcessing template");
    };
    xs.iter()
        .map(|p| {
            if let Some(s) = p.get("Sequence") {
                let id = s.get("id").and_then(|x| x.str_()).unwrap_or("A");
                Ok(Piece::Seq(id == "B", s.get("type_id").map_or(Ok(0), hj_u32)?))
            } else if let Some(s) = p.get("SpecialToken") {
                Ok(Piece::Special(s.get("id").and_then(|x| x.str_()).unwrap_or("").to_string(), s.get("type_id").map_or(Ok(0), hj_u32)?))
            } else {
                hlacks("a TemplateProcessing piece")
            }
        })
        .collect()
}

fn hpp_load(v: &J) -> HR<HPost> {
    need(v, "pp")?;
    Ok(match v.tname() {
        "BertProcessing" => HPost::Bert { sep: hj_tokid(v.get("sep"), ("[SEP]", 102))?, cls: hj_tokid(v.get("cls"), ("[CLS]", 101))? },
        "RobertaProcessing" => HPost::Roberta {
            sep: hj_tokid(v.get("sep"), ("</s>", 2))?,
            cls: hj_tokid(v.get("cls"), ("<s>", 0))?,
            trim: hj_bool(v, "trim_offsets", true)?,
            prefix: hj_bool(v, "add_prefix_space", true)?,
        },
        "ByteLevel" => HPost::ByteLevel { trim: hj_bool(v, "trim_offsets", true)?, prefix: hj_bool(v, "add_prefix_space", true)? },
        "TemplateProcessing" => {
            let single = hpieces(v.get("single"))?;
            let pair = hpieces(v.get("pair"))?;
            let mut specials = HashMap::new();
            if let Some(J::Obj(kv)) = v.get("special_tokens") {
                for (k, s) in kv {
                    let ids = match s.get("ids") {
                        Some(J::Arr(xs)) => xs.iter().map(hj_u32).collect::<HR<Vec<_>>>()?,
                        _ => return hlacks("TemplateProcessing special_tokens"),
                    };
                    let tokens = match s.get("tokens") {
                        Some(J::Arr(xs)) => xs.iter().map(|x| x.str_().map(|s| s.to_string())).collect::<Option<Vec<_>>>(),
                        _ => None,
                    };
                    let Some(tokens) = tokens else {
                        return hlacks("TemplateProcessing special_tokens");
                    };
                    if ids.len() != tokens.len() {
                        return hpy("ids and tokens must be of the same length");
                    }
                    specials.insert(k.clone(), HSpecial { ids, tokens });
                }
            }
            for p in single.iter().chain(&pair) {
                if let Piece::Special(id, _) = p {
                    if !specials.contains_key(id) {
                        return hpy(format!("Missing SpecialToken(s) with id(s) `{id}`"));
                    }
                }
            }
            let count = |t: &[Piece]| t.iter().map(|p| if let Piece::Special(id, _) = p { specials.get(id).map_or(0, |s| s.ids.len()) } else { 0 }).sum();
            let (n_single, n_pair) = (count(&single), count(&pair));
            HPost::Template { single, pair, specials, n_single, n_pair }
        }
        "Sequence" => {
            let Some(J::Arr(xs)) = v.get("processors") else {
                return hlacks("processors.Sequence without processors");
            };
            HPost::Seq(xs.iter().map(hpp_load).collect::<HR<_>>()?)
        }
        t => return hlacks(format!("tokenizers post-processor {}", if t.is_empty() { "without a type" } else { t })),
    })
}

fn hwrap(e: &HEnc, pre: Option<&(String, u32)>, post: &(String, u32), types: Vec<u32>, seq: usize, inner_start: usize) -> HEnc {
    let n = e.len();
    let mut ids = vec![];
    let mut tokens = vec![];
    if let Some(p) = pre {
        ids.push(p.1);
        tokens.push(p.0.clone());
    }
    ids.extend(&e.ids);
    tokens.extend(e.tokens.iter().cloned());
    ids.push(post.1);
    tokens.push(post.0.clone());
    let np = pre.is_some() as usize;
    let mut words = vec![None; np];
    words.extend(&e.words);
    words.push(None);
    let mut offsets = vec![(0, 0); np];
    offsets.extend(&e.offsets);
    offsets.push((0, 0));
    let mut special = vec![1; np];
    special.extend(std::iter::repeat_n(0, n));
    special.push(1);
    let len = ids.len();
    HEnc { ids, type_ids: types, tokens, words, offsets, special, attention: vec![1; len], overflowing: vec![], ranges: vec![(seq, (inner_start, len - 1))] }
}

impl HPost {
    fn added(&self, pair: bool) -> usize {
        match self {
            HPost::Bert { .. } => 2 + pair as usize,
            HPost::Roberta { .. } => 2 + 2 * pair as usize,
            HPost::ByteLevel { .. } => 0,
            HPost::Template { n_single, n_pair, .. } => {
                if pair {
                    *n_pair
                } else {
                    *n_single
                }
            }
            HPost::Seq(xs) => xs.iter().map(|x| x.added(pair)).sum(),
        }
    }
    fn process_encodings(&self, mut encs: Vec<HEnc>, add: bool) -> Vec<HEnc> {
        match self {
            HPost::Bert { sep, cls } => {
                if !add {
                    return encs;
                }
                encs.iter_mut()
                    .enumerate()
                    .map(|(i, e)| {
                        let ov = std::mem::take(&mut e.overflowing);
                        if i == 0 {
                            let t = |e: &HEnc| {
                                let mut t = vec![0];
                                t.extend(&e.type_ids);
                                t.push(0);
                                t
                            };
                            let mut out = hwrap(e, Some(cls), sep, t(e), 0, 1);
                            out.overflowing = ov.iter().map(|o| hwrap(o, Some(cls), sep, t(o), 0, 1)).collect();
                            out
                        } else {
                            let t = |e: &HEnc| {
                                let mut t = e.type_ids.clone();
                                t.push(1);
                                t
                            };
                            let mut out = hwrap(e, None, sep, t(e), 1, 0);
                            out.overflowing = ov.iter().map(|o| hwrap(o, None, sep, t(o), 1, 0)).collect();
                            out
                        }
                    })
                    .collect()
            }
            HPost::Roberta { sep, cls, trim, prefix } => {
                if *trim {
                    for e in encs.iter_mut() {
                        e.trim_offsets(*prefix);
                        e.overflowing.iter_mut().for_each(|o| o.trim_offsets(*prefix));
                    }
                }
                for e in encs.iter_mut() {
                    e.type_ids = vec![0; e.len()];
                }
                if !add {
                    return encs;
                }
                encs.iter_mut()
                    .enumerate()
                    .map(|(i, e)| {
                        let ov = std::mem::take(&mut e.overflowing);
                        let first = if i == 0 { cls } else { sep };
                        let mut out = hwrap(e, Some(first), sep, vec![0; e.len() + 2], i.min(1), 1);
                        out.overflowing = ov.iter().map(|o| hwrap(o, Some(first), sep, vec![0; o.len() + 2], i.min(1), 1)).collect();
                        out
                    })
                    .collect()
            }
            HPost::ByteLevel { trim, prefix } => {
                if *trim {
                    for e in encs.iter_mut() {
                        e.trim_offsets(*prefix);
                        e.overflowing.iter_mut().for_each(|o| o.trim_offsets(*prefix));
                    }
                }
                for (i, e) in encs.iter_mut().enumerate() {
                    e.set_seq(i);
                }
                encs
            }
            HPost::Template { single, pair, specials, .. } => {
                let t = if encs.len() == 2 { pair } else { single };
                let mut out = vec![];
                for p in t {
                    match p {
                        Piece::Seq(b, type_id) => {
                            let i = *b as usize;
                            let e = &mut encs[i];
                            e.type_ids = vec![*type_id; e.len()];
                            e.set_seq(i);
                            out.push(e.clone());
                        }
                        Piece::Special(id, type_id) => {
                            if add {
                                let s = &specials[id];
                                let n = s.ids.len();
                                out.push(HEnc {
                                    ids: s.ids.clone(),
                                    type_ids: vec![*type_id; n],
                                    tokens: s.tokens.clone(),
                                    words: vec![None; n],
                                    offsets: vec![(0, 0); n],
                                    special: vec![1; n],
                                    attention: vec![1; n],
                                    overflowing: vec![],
                                    ranges: vec![],
                                });
                            }
                        }
                    }
                }
                out
            }
            HPost::Seq(xs) => {
                for x in xs {
                    encs = x.process_encodings(encs, add);
                }
                encs
            }
        }
    }
    fn process(&self, e: HEnc, pair: Option<HEnc>, add: bool) -> HEnc {
        let mut encs = vec![e];
        encs.extend(pair);
        for (i, e) in encs.iter_mut().enumerate() {
            e.set_seq(i);
            e.overflowing.iter_mut().for_each(|o| o.set_seq(i));
            e.type_ids = vec![i as u32; e.len()];
        }
        HEnc::merge(self.process_encodings(encs, add), false)
    }
}

// ---------- decoders ----------

pub enum HDec {
    ByteLevel,
    WordPiece { prefix: String, cleanup: bool },
    Metaspace { rep: char, scheme: Prepend },
    Replace(HPat, String),
    ByteFallback,
    Fuse,
    Strip { c: char, start: usize, stop: usize },
    Bpe(String),
    Ctc { pad: String, delim: String, cleanup: bool },
    Seq(Vec<HDec>),
}

fn hd_load(v: &J) -> HR<HDec> {
    need(v, "d")?;
    Ok(match v.tname() {
        "ByteLevel" => HDec::ByteLevel,
        "WordPiece" => HDec::WordPiece { prefix: hj_string(v, "prefix")?.unwrap_or_else(|| "##".into()), cleanup: hj_bool(v, "cleanup", true)? },
        "Metaspace" => HDec::Metaspace { rep: hj_char(v, "replacement", '▁')?, scheme: hscheme(v)? },
        "Replace" => HDec::Replace(hj_pattern(v.get("pattern"))?, hj_string(v, "content")?.unwrap_or_default()),
        "ByteFallback" => HDec::ByteFallback,
        "Fuse" => HDec::Fuse,
        "Strip" => HDec::Strip { c: hj_char(v, "content", ' ')?, start: hj_usize(v, "start", 0)?, stop: hj_usize(v, "stop", 0)? },
        "BPEDecoder" => HDec::Bpe(hj_string(v, "suffix")?.unwrap_or_else(|| "</w>".into())),
        "CTC" => HDec::Ctc {
            pad: hj_string(v, "pad_token")?.unwrap_or_else(|| "<pad>".into()),
            delim: hj_string(v, "word_delimiter_token")?.unwrap_or_else(|| "|".into()),
            cleanup: hj_bool(v, "cleanup", true)?,
        },
        "Sequence" => {
            let Some(J::Arr(xs)) = v.get("decoders") else {
                return hlacks("decoders.Sequence without decoders");
            };
            HDec::Seq(xs.iter().map(hd_load).collect::<HR<_>>()?)
        }
        t => return hlacks(format!("tokenizers decoder {}", if t.is_empty() { "without a type" } else { t })),
    })
}

fn wp_cleanup(s: &str) -> String {
    s.replace(" .", ".")
        .replace(" ?", "?")
        .replace(" !", "!")
        .replace(" ,", ",")
        .replace(" ' ", "'")
        .replace(" n't", "n't")
        .replace(" 'm", "'m")
        .replace(" do not", " don't")
        .replace(" 's", "'s")
        .replace(" 've", "'ve")
        .replace(" 're", "'re")
}

impl HDec {
    fn chain(&self, toks: Vec<String>) -> HR<Vec<String>> {
        Ok(match self {
            HDec::ByteLevel => {
                let mut bytes = vec![];
                for t in &toks {
                    let bs: Option<Vec<u8>> = t.chars().map(hbl_byte).collect();
                    match bs {
                        Some(b) => bytes.extend(b),
                        None => bytes.extend(t.as_bytes()),
                    }
                }
                vec![String::from_utf8_lossy(&bytes).into_owned()]
            }
            HDec::WordPiece { prefix, cleanup } => toks
                .into_iter()
                .enumerate()
                .map(|(i, mut t)| {
                    if i != 0 {
                        t = match t.strip_prefix(prefix.as_str()) {
                            Some(x) => x.to_string(),
                            None => format!(" {t}"),
                        };
                    }
                    if *cleanup {
                        t = wp_cleanup(&t);
                    }
                    t
                })
                .collect(),
            HDec::Metaspace { rep, scheme } => toks
                .iter()
                .enumerate()
                .map(|(i, t)| t.chars().filter_map(|c| if c == *rep { if i == 0 && *scheme != Prepend::Never { None } else { Some(' ') } } else { Some(c) }).collect())
                .collect(),
            HDec::Replace(p, content) => toks
                .iter()
                .map(|t| {
                    let mut out = String::new();
                    for ((s, e), m) in p.find_matches(t) {
                        if m {
                            out.push_str(content);
                        } else {
                            out.push_str(&t[s..e]);
                        }
                    }
                    out
                })
                .collect(),
            HDec::ByteFallback => {
                let mut out = vec![];
                let mut prev: Vec<u8> = vec![];
                let flush = |prev: &mut Vec<u8>, out: &mut Vec<String>| {
                    if !prev.is_empty() {
                        match String::from_utf8(prev.clone()) {
                            Ok(s) => out.push(s),
                            Err(_) => out.extend(std::iter::repeat_n("�".to_string(), prev.len())),
                        }
                        prev.clear();
                    }
                };
                for t in toks {
                    let b = if t.len() == 6 && t.starts_with("<0x") && t.ends_with('>') { u8::from_str_radix(&t[3..5], 16).ok() } else { None };
                    match b {
                        Some(b) => prev.push(b),
                        None => {
                            flush(&mut prev, &mut out);
                            out.push(t);
                        }
                    }
                }
                flush(&mut prev, &mut out);
                out
            }
            HDec::Fuse => vec![toks.concat()],
            HDec::Strip { c, start, stop } => {
                let mut out = vec![];
                for t in toks {
                    let cs: Vec<char> = t.chars().collect();
                    let mut sc = 0;
                    for (i, &x) in cs.iter().enumerate().take(*start) {
                        if x == *c {
                            sc = i + 1;
                        } else {
                            break;
                        }
                    }
                    let mut ec = cs.len();
                    for i in 0..*stop {
                        if i >= cs.len() {
                            return hlacks("decoders.Strip on a token shorter than stop (tokenizers panics)");
                        }
                        let idx = cs.len() - i - 1;
                        if cs[idx] == *c {
                            ec = idx;
                        } else {
                            break;
                        }
                    }
                    if sc > ec {
                        return hlacks("decoders.Strip that cuts past the token (tokenizers panics)");
                    }
                    out.push(cs[sc..ec].iter().collect());
                }
                out
            }
            HDec::Bpe(suffix) => {
                    // tokens.len() - 1 wraps in tokenizers' release build: nothing is the last token
                let n = toks.len().wrapping_sub(1);
                toks.into_iter().enumerate().map(|(i, t)| t.replace(suffix.as_str(), if i == n { "" } else { " " })).collect()
            }
            HDec::Ctc { pad, delim, cleanup } => {
                let mut out: Vec<String> = vec![];
                let mut last: Option<String> = None;
                for t in toks {
                    if last.as_ref() == Some(&t) {
                        continue;
                    }
                    last = Some(t.clone());
                    let mut r = t.replace(pad.as_str(), "");
                    if *cleanup {
                        r = wp_cleanup(&r).replace(delim.as_str(), " ");
                    }
                    if !r.is_empty() {
                        out.push(r);
                    }
                }
                out
            }
            HDec::Seq(xs) => {
                let mut t = toks;
                for x in xs {
                    t = x.chain(t)?;
                }
                t
            }
        })
    }
}

// ---------- added tokens ----------

#[derive(Clone, PartialEq)]
pub struct HAddedTok {
    pub content: String,
    pub single_word: bool,
    pub lstrip: bool,
    pub rstrip: bool,
    pub normalized: bool,
    pub special: bool,
}

#[derive(Default)]
pub struct HAdded {
    map: HashMap<String, u32>,
    pub map_r: std::collections::BTreeMap<u32, HAddedTok>,
    specials: std::collections::HashSet<String>,
    norm_cache: HashMap<u32, String>,
    // the two tries: (pattern, id) by first byte
    raw: Vec<(String, u32)>,
    normd: Vec<(String, u32)>,
    pub encode_special: bool,
}

fn hfind_longest(pats: &[(String, u32)], s: &str) -> Vec<(usize, usize, u32)> {
    // Aho-Corasick, LeftmostLongest: at the leftmost position where a pattern starts, the longest one
    let b = s.as_bytes();
    let mut first: HashMap<u8, Vec<usize>> = HashMap::new();
    for (i, (p, _)) in pats.iter().enumerate() {
        first.entry(p.as_bytes()[0]).or_default().push(i);
    }
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let mut best: Option<(usize, u32)> = None;
        if let Some(cands) = first.get(&b[i]) {
            for &k in cands {
                let (p, id) = &pats[k];
                if b[i..].starts_with(p.as_bytes()) && best.is_none_or(|x| p.len() > x.0) {
                    best = Some((p.len(), *id));
                }
            }
        }
        match best {
            Some((l, id)) => {
                out.push((i, i + l, id));
                i += l;
            }
            None => i += 1,
        }
    }
    out
}

impl HAdded {
    fn refresh(&mut self) {
        self.raw.clear();
        self.normd.clear();
        for (id, t) in &self.map_r {
            if t.normalized {
                self.normd.push((self.norm_cache.get(id).cloned().unwrap_or_else(|| t.content.clone()), *id));
            } else {
                self.raw.push((t.content.clone(), *id));
            }
        }
        self.raw.retain(|p| !p.0.is_empty());
        self.normd.retain(|p| !p.0.is_empty());
    }
    pub fn add(&mut self, toks: Vec<HAddedTok>, model: &HModel, norm: Option<&HNorm>) -> usize {
        let vs = model.vocab().len() as u32;
        let mut next = self.map_r.keys().max().map_or(vs, |m| if *m >= vs || vs == 0 { m + 1 } else { vs });
        let (mut total, mut ignored) = (0, 0);
        for t in toks {
            total += 1;
            if t.content.is_empty() {
                ignored += 1;
                continue;
            }
            if let Some(id) = self.map.get(&t.content) {
                if self.map_r.get(id) == Some(&t) {
                    ignored += 1;
                    continue;
                }
            }
            let id = match self.map.get(&t.content).copied().or_else(|| model.vocab().get(&t.content).copied()) {
                Some(i) => i,
                None => {
                    next += 1;
                    next - 1
                }
            };
            if t.normalized {
                if let Some(n) = norm {
                    let mut s = NStr::from_str(&t.content);
                    n.normalize(&mut s);
                    if s.get() != t.content {
                        self.norm_cache.insert(id, s.get().to_string());
                    }
                }
            }
            self.map.insert(t.content.clone(), id);
            if t.special {
                self.specials.insert(t.content.clone());
            }
            self.map_r.insert(id, t);
        }
        self.refresh();
        total - ignored
    }
    fn find_matches(&self, s: &str, pats: &[(String, u32)]) -> Vec<(Option<u32>, Offs)> {
        if s.is_empty() {
            return vec![(None, (0, 0))];
        }
        if pats.is_empty() {
            return vec![(None, (0, s.len()))];
        }
        let mut start_off = 0;
        let mut out = vec![];
        for (ms, me, id) in hfind_longest(pats, s) {
            let (mut st, mut en) = (ms, me);
            let t = &self.map_r[&id];
            if self.encode_special && self.specials.contains(&t.content) {
                continue;
            }
            if t.single_word {
                let start_space = st == 0 || !s[..st].chars().next_back().is_some_and(hc_rew);
                let stop_space = en == s.len() || !s[en..].chars().next().is_some_and(hc_rew);
                if !stop_space || !start_space {
                    continue;
                }
            }
            if t.lstrip {
                // \s*$
                let mut k = st;
                for (i, c) in s[..st].char_indices().rev() {
                    if hc_res(c) {
                        k = i;
                    } else {
                        break;
                    }
                }
                st = k.max(start_off);
            }
            if t.rstrip {
                en += s[en..].chars().take_while(|c| hc_res(*c)).map(|c| c.len_utf8()).sum::<usize>();
            }
            if start_off < st {
                out.push((None, (start_off, st)));
            }
            out.push((Some(id), (st, en)));
            start_off = en;
        }
        if start_off != s.len() {
            out.push((None, (start_off, s.len())));
        }
        out
    }
    fn split_with(&self, n: NStr, pats: &[(String, u32)]) -> Vec<PSplit> {
        self.find_matches(n.get(), pats)
            .into_iter()
            .map(|(id, (a, b))| {
                let sl = n.slice(HRange::Norm(a, b)).expect("AddedVocabulary bad split");
                match id {
                    Some(id) => {
                        let v = sl.get().to_string();
                        let l = v.len();
                        PSplit { n: sl, toks: Some(vec![HToken { id, value: v, offs: (0, l) }]) }
                    }
                    None => PSplit { n: sl, toks: None },
                }
            })
            .collect()
    }
    fn extract_and_normalize(&self, norm: Option<&HNorm>, s: &str) -> PreStr {
        let mut p = PreStr::new(s);
        p.split(|n| self.split_with(n, &self.raw));
        p.split(|mut n| {
            if let Some(x) = norm {
                x.normalize(&mut n);
            }
            self.split_with(n, &self.normd)
        });
        p
    }
    pub fn id_to_token(&self, id: u32) -> Option<String> {
        self.map_r.get(&id).map(|t| self.norm_cache.get(&id).cloned().unwrap_or_else(|| t.content.clone()))
    }
}

// ---------- the tokenizer ----------

#[derive(Clone)]
pub struct HTrunc {
    pub max_length: usize,
    pub stride: usize,
    // "longest_first", "only_first", "only_second"
    pub strategy: &'static str,
    pub left: bool,
}

#[derive(Clone)]
pub struct HPad {
    pub fixed: Option<usize>,
    pub left: bool,
    pub multiple: Option<usize>,
    pub pad_id: u32,
    pub pad_type: u32,
    pub pad_token: String,
}

pub struct HTokenizer {
    pub model: HModel,
    pub norm: Option<HNorm>,
    pre: Option<HPre>,
    post: Option<HPost>,
    dec: Option<HDec>,
    pub added: HAdded,
    pub trunc: Option<HTrunc>,
    pub pad: Option<HPad>,
}

// one input: a str, or a list of words (is_pretokenized)
pub enum HInput {
    Raw(String),
    Words(Vec<String>),
}

fn hj_strategy(s: &str) -> HR<&'static str> {
    Ok(match s {
        "LongestFirst" => "longest_first",
        "OnlyFirst" => "only_first",
        "OnlySecond" => "only_second",
        _ => return hpy(format!("unknown variant `{s}`, expected one of `LongestFirst`, `OnlyFirst`, `OnlySecond`")),
    })
}

pub fn hf_load(text: &str) -> HR<HTokenizer> {
    let j = hj_parse(text)?;
    if !matches!(j, J::Obj(_)) {
        return hpy("invalid type: expected struct Tokenizer");
    }
    if let Some(v) = j.get("version").and_then(|v| v.str_()) {
        if v != "1.0" {
            return hpy(format!("Unknown tokenizer version '{v}'"));
        }
    }
    let Some(m) = j.get("model") else {
        return hpy("missing field `model`");
    };
    let model = hm_load(m)?;
    let norm = j.opt("normalizer").map(hn_load).transpose()?;
    let pre = j.opt("pre_tokenizer").map(hp_load).transpose()?;
    let post = j.opt("post_processor").map(hpp_load).transpose()?;
    let dec = j.opt("decoder").map(hd_load).transpose()?;
    let trunc = match j.opt("truncation") {
        None => None,
        Some(t) => Some(HTrunc {
            max_length: hj_usize(t, "max_length", 512)?,
            stride: hj_usize(t, "stride", 0)?,
            strategy: hj_strategy(t.get("strategy").and_then(|s| s.str_()).unwrap_or("LongestFirst"))?,
            left: t.get("direction").and_then(|s| s.str_()) == Some("Left"),
        }),
    };
    let pad = match j.opt("padding") {
        None => None,
        Some(p) => Some(HPad {
            fixed: match p.get("strategy") {
                Some(J::Str(s)) if s == "BatchLongest" => None,
                Some(s) if s.get("Fixed").is_some() => Some(hj_usize(s, "Fixed", 0)?),
                _ => return hlacks("tokenizer.json padding strategy"),
            },
            left: p.get("direction").and_then(|s| s.str_()) == Some("Left"),
            multiple: match p.opt("pad_to_multiple_of") {
                None => None,
                Some(_) => Some(hj_usize(p, "pad_to_multiple_of", 0)?),
            },
            pad_id: p.get("pad_id").map_or(Ok(0), hj_u32)?,
            pad_type: p.get("pad_type_id").map_or(Ok(0), hj_u32)?,
            pad_token: hj_string(p, "pad_token")?.unwrap_or_else(|| "[PAD]".into()),
        }),
    };
    let mut toks = vec![];
    if let Some(J::Arr(xs)) = j.get("added_tokens") {
        for x in xs {
            let content = hj_string(x, "content")?.unwrap_or_default();
            let special = hj_bool(x, "special", false)?;
            toks.push(HAddedTok {
                content,
                single_word: hj_bool(x, "single_word", false)?,
                lstrip: hj_bool(x, "lstrip", false)?,
                rstrip: hj_bool(x, "rstrip", false)?,
                normalized: hj_bool(x, "normalized", !special)?,
                special,
            });
        }
    }
    let mut t = HTokenizer { model, norm, pre, post, dec, added: HAdded::default(), trunc, pad };
    let mut added = std::mem::take(&mut t.added);
    added.add(toks, &t.model, t.norm.as_ref());
    t.added = added;
    Ok(t)
}

struct ByteToChar(Vec<usize>);
impl ByteToChar {
    fn new(s: &str) -> ByteToChar {
        let mut m = Vec::with_capacity(s.len());
        for (i, c) in s.chars().enumerate() {
            for _ in 0..c.len_utf8() {
                m.push(i);
            }
        }
        ByteToChar(m)
    }
    fn convert(&self, o: Offs) -> Option<Offs> {
        match (self.0.get(o.0), self.0.get(o.1)) {
            (Some(s), Some(e)) => Some((*s, *e)),
            (Some(s), None) => {
                let last = o.1.checked_sub(1).and_then(|k| self.0.get(k).copied()).unwrap_or(s + 1);
                Some((*s, last + 1))
            }
            _ => None,
        }
    }
}

impl HTokenizer {
    pub fn added_tokens(&self, pair: bool) -> usize {
        self.post.as_ref().map_or(0, |p| p.added(pair))
    }
    fn encode_one(&self, s: &str, type_id: u32, word: Option<u32>, fast: bool) -> HR<HEnc> {
        let mut p = self.added.extract_and_normalize(self.norm.as_ref(), s);
        if let Some(pre) = &self.pre {
            pre.pre_tokenize(&mut p);
        }
        for sp in p.splits.iter_mut().filter(|s| s.toks.is_none()) {
            sp.toks = Some(self.model.tokenize(sp.n.get())?);
        }
        let mut e = HEnc::default();
        if p.splits.is_empty() {
            return Ok(e);
        }
        if fast {
            // OffsetType::None
            for sp in p.splits {
                for t in sp.toks.unwrap() {
                    e.ids.push(t.id);
                    e.tokens.push(String::new());
                    e.offsets.push((0, 0));
                    e.type_ids.push(0);
                    e.words.push(None);
                    e.special.push(0);
                    e.attention.push(1);
                }
            }
            return Ok(e);
        }
        let conv = ByteToChar::new(&p.original);
        for (idx, sp) in p.splits.into_iter().enumerate() {
            let base = sp.n.offsets_original();
            for t in sp.toks.unwrap() {
                let mut o = sp.n.convert_offsets(HRange::Norm(t.offs.0, t.offs.1)).map_or(t.offs, |r| (base.0 + r.0, base.0 + r.1));
                o = conv.convert(o).unwrap_or(o);
                e.ids.push(t.id);
                e.tokens.push(t.value);
                e.offsets.push(o);
                e.type_ids.push(type_id);
                e.words.push(if word.is_some() { word } else { Some(idx as u32) });
                e.special.push(0);
                e.attention.push(1);
            }
        }
        Ok(e)
    }
    fn encode_seq(&self, x: &HInput, type_id: u32, fast: bool) -> HR<HEnc> {
        match x {
            HInput::Raw(s) => self.encode_one(s, type_id, None, fast),
            HInput::Words(ws) => {
                let mut encs = vec![];
                for (i, w) in ws.iter().enumerate() {
                    encs.push(self.encode_one(w, type_id, Some(i as u32), fast)?);
                }
                Ok(HEnc::merge(encs, false))
            }
        }
    }
    fn truncate(&self, mut e: HEnc, mut pair: Option<HEnc>, t: &HTrunc) -> HR<(HEnc, Option<HEnc>)> {
        if t.max_length == 0 {
            e.truncate(0, t.stride, t.left)?;
            if let Some(p) = pair.as_mut() {
                p.truncate(0, t.stride, t.left)?;
            }
            return Ok((e, pair));
        }
        let total = e.len() + pair.as_ref().map_or(0, |p| p.len());
        if total <= t.max_length {
            return Ok((e, pair));
        }
        let remove = total - t.max_length;
        match t.strategy {
            "longest_first" => {
                if let Some(p) = pair.as_mut() {
                    let (mut n1, mut n2) = (e.len(), p.len());
                    let swap = n1 > n2;
                    if swap {
                        std::mem::swap(&mut n1, &mut n2);
                    }
                    if n1 > t.max_length {
                        n2 = n1;
                    } else {
                        n2 = n1.max(t.max_length - n1);
                    }
                    if n1 + n2 > t.max_length {
                        n1 = t.max_length / 2;
                        n2 = n1 + t.max_length % 2;
                    }
                    if swap {
                        std::mem::swap(&mut n1, &mut n2);
                    }
                    e.truncate(n1, t.stride, t.left)?;
                    p.truncate(n2, t.stride, t.left)?;
                } else {
                    e.truncate(total - remove, t.stride, t.left)?;
                }
            }
            s => {
                let target = if s == "only_first" {
                    &mut e
                } else {
                    match pair.as_mut() {
                        Some(p) => p,
                        None => return hpy("Truncation error: Second sequence not provided"),
                    }
                };
                let n = target.len();
                if n > remove {
                    target.truncate(n - remove, t.stride, t.left)?;
                } else {
                    return hpy("Truncation error: Sequence to truncate too short to respect the provided max_length");
                }
            }
        }
        Ok((e, pair))
    }
    pub fn encode(&self, a: &HInput, b: Option<&HInput>, add: bool) -> HR<HEnc> {
        self.encode_full(a, b, add, false)
    }
    pub fn encode_full(&self, a: &HInput, b: Option<&HInput>, add: bool, fast: bool) -> HR<HEnc> {
        let e = self.encode_seq(a, 0, fast)?;
        let pair = b.map(|b| self.encode_seq(b, 1, fast)).transpose()?;
        let (e, pair) = match &self.trunc {
            Some(t) => {
                let n = self.added_tokens(pair.is_some());
                if add && n > 0 {
                    let Some(m) = t.max_length.checked_sub(n) else {
                        return hlacks("truncation max_length below the special tokens' count (tokenizers panics)");
                    };
                    let t2 = HTrunc { max_length: m, ..t.clone() };
                    self.truncate(e, pair, &t2)?
                } else {
                    self.truncate(e, pair, t)?
                }
            }
            None => (e, pair),
        };
        let mut fin = match &self.post {
            Some(p) => p.process(e, pair, add),
            None => match pair {
                None => e,
                Some(p) => {
                    let mut f = HEnc::default();
                    for (i, mut x) in [e, p].into_iter().enumerate() {
                        x.set_seq(i);
                        f.merge_with(x, false);
                    }
                    f
                }
            },
        };
        if let Some(p) = &self.pad {
            hpad(std::slice::from_mut(&mut fin), p);
        }
        Ok(fin)
    }
    pub fn encode_batch_full(&self, xs: &[(HInput, Option<HInput>)], add: bool, fast: bool) -> HR<Vec<HEnc>> {
        let mut out = vec![];
        for (a, b) in xs {
            out.push(self.encode_full(a, b.as_ref(), add, fast)?);
        }
        if let Some(p) = &self.pad {
            hpad(&mut out, p);
        }
        Ok(out)
    }
    pub fn decode(&self, ids: &[u32], skip: bool) -> HR<String> {
        let mut toks = vec![];
        for id in ids {
            if let Some(t) = self.added.id_to_token(*id).or_else(|| self.model.vocab_r().get(id).cloned()) {
                if !skip || !self.added.specials.contains(&t) {
                    toks.push(t);
                }
            }
        }
        match &self.dec {
            Some(d) => Ok(d.chain(toks)?.concat()),
            None => Ok(toks.join(" ")),
        }
    }
    pub fn token_to_id(&self, t: &str) -> Option<u32> {
        self.added.map.get(t).copied().or_else(|| self.model.vocab().get(t).copied())
    }
    pub fn id_to_token(&self, id: u32) -> Option<String> {
        self.added.id_to_token(id).or_else(|| self.model.vocab_r().get(&id).cloned())
    }
    pub fn vocab(&self, with_added: bool) -> Vec<(String, u32)> {
        let mut v: HashMap<String, u32> = self.model.vocab().clone();
        if with_added {
            for (k, i) in &self.added.map {
                v.insert(k.clone(), *i);
            }
        }
        v.into_iter().collect()
    }
    pub fn vocab_size(&self, with_added: bool) -> usize {
        let base = self.model.vocab().len();
        if with_added {
            let over = self.added.map.keys().filter(|t| self.model.vocab().contains_key(*t)).count();
            base + self.added.map.len() - over
        } else {
            base
        }
    }
    pub fn is_special(&self, t: &str) -> bool {
        self.added.specials.contains(t)
    }
}

pub fn hpad(encs: &mut [HEnc], p: &HPad) {
    if encs.is_empty() {
        return;
    }
    let mut n = match p.fixed {
        Some(n) => n,
        None => encs.iter().map(|e| e.len()).max().unwrap(),
    };
    if let Some(m) = p.multiple {
        if m > 0 && n % m > 0 {
            n += m - n % m;
        }
    }
    for e in encs.iter_mut() {
        e.pad(n, p.pad_id, p.pad_type, &p.pad_token, p.left);
    }
}
