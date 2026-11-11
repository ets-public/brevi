// ---------- bytes, hashlib, base64 ----------
// bytes for dynamic builds: an immutable byte string (an Ext value whose type is builtins.bytes), with
// str.encode and bytes.decode, and hashlib and base64 on top. Messages are CPython's.

pub struct BytesV(pub Vec<u8>);

pub fn bytesv(b: Vec<u8>) -> V {
    V::Ext(ExtRc::new(BytesV(b)))
}

pub fn bytes_of(v: &V) -> Option<Vec<u8>> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<BytesV>().map(|b| b.0.clone()),
        _ => None,
    }
}

pub fn is_bytes_v(v: &V) -> bool {
    matches!(v, V::Ext(x) if x.any().downcast_ref::<BytesV>().is_some())
}

fn want_bytes(v: &V, what: &str) -> Result<Vec<u8>, E> {
    bytes_of(v).ok_or_else(|| exc("TypeError", format!("{what}, not '{}'", tn(v))))
}

pub fn bytes_repr(b: &[u8]) -> String {
    let q = if b.contains(&b'\'') && !b.contains(&b'"') { '"' } else { '\'' };
    let mut out = String::from("b");
    out.push(q);
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            _ if c as char == q => {
                out.push('\\');
                out.push(q);
            }
            32..=126 => out.push(c as char),
            _ => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out.push(q);
    out
}

fn is_py_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn find_sub(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if n.is_empty() {
        return (from <= h.len()).then_some(from);
    }
    if n.len() > h.len() {
        return None;
    }
    (from..=h.len() - n.len()).find(|&i| &h[i..i + n.len()] == n)
}

// A bytes argument of a bytes method: bytes, or an int byte where CPython takes one (find, count, in).
fn sub_arg(v: &V, int_ok: bool) -> Result<Vec<u8>, E> {
    if let Some(b) = bytes_of(v) {
        return Ok(b);
    }
    if int_ok {
        if let Some(i) = try_index(v)? {
            let i = match &i {
                V::Int(i) => *i,
                V::Bool(b) => *b as i64,
                V::Big(_) => return val_err("byte must be in range(0, 256)".into()),
                o => return Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(o)))),
            };
            if !(0..256).contains(&i) {
                return val_err("byte must be in range(0, 256)".into());
            }
            return Ok(vec![i as u8]);
        }
    }
    type_err(format!("a bytes-like object is required, not '{}'", tn(v)))
}

// The needle of find/index/rfind/rindex/count: CPython 3.13's own message for a non-byte, non-int.
fn find_needle(v: &V) -> Result<Vec<u8>, E> {
    if let Some(b) = bytes_of(v) {
        return Ok(b);
    }
    if let Some(i) = try_index(v)? {
        let i = match &i {
            V::Int(i) => *i,
            V::Bool(b) => *b as i64,
            V::Big(_) => return val_err("byte must be in range(0, 256)".into()),
            o => return Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(o)))),
        };
        if !(0..256).contains(&i) {
            return val_err("byte must be in range(0, 256)".into());
        }
        return Ok(vec![i as u8]);
    }
    type_err(format!("argument should be integer or bytes-like object, not '{}'", tn(v)))
}

// __index__ of an int-like value, or None when the value has no __index__ (CPython's PyIndex_Check).
fn try_index(v: &V) -> Result<Option<V>, E> {
    match v {
        V::Int(_) | V::Bool(_) | V::Big(_) => Ok(Some(v.clone())),
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__index__") {
                return call(&f, Args::p(vec![v.clone()])).map(Some);
            }
            match inner_of(v) {
                Some(inner) if matches!(inner, V::Int(_) | V::Bool(_) | V::Big(_)) => Ok(Some(inner)),
                _ => Ok(None),
            }
        }
        V::Ext(x) if is_np(v) => match x.method(v, "__index__", &mut Args::p(vec![])) {
            Some(r) => r.map(Some),
            None => Ok(None),
        },
        _ => Ok(None),
    }
}

// The start/end of find/count/startswith/endswith, as CPython's PyNumber_AsSsize_t parses them:
// a huge int clamps to the extremes like a slice index, a non-index raises the slice TypeError.
fn slice_bound(v: &V) -> Result<i64, E> {
    match try_index(v)? {
        Some(V::Int(i)) => Ok(i),
        Some(V::Bool(b)) => Ok(b as i64),
        Some(V::Big(b)) => Ok(if b.neg { -(i64::MAX / 2) } else { i64::MAX / 2 }),
        Some(o) => Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(&o)))),
        None => Err(exc("TypeError", "slice indices must be integers or None or have an __index__ method".into())),
    }
}

// PyLong_AsSsize_t, as center/zfill/split parse their width/maxsplit arguments.
fn ssize_arg(v: &V) -> Result<i64, E> {
    match try_index(v)? {
        Some(V::Int(i)) => Ok(i),
        Some(V::Bool(b)) => Ok(b as i64),
        Some(V::Big(_)) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        Some(o) => Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(&o)))),
        None => Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v)))),
    }
}

// PyLong_AsInt, as expandtabs and hex parse their tab size / bytes_per_sep arguments.
fn c_int_arg(v: &V) -> Result<i64, E> {
    match try_index(v)? {
        Some(V::Int(i)) => {
            if i < i32::MIN as i64 || i > i32::MAX as i64 {
                return Err(exc("OverflowError", "Python int too large to convert to C int".into()));
            }
            Ok(i)
        }
        Some(V::Bool(b)) => Ok(b as i64),
        Some(V::Big(_)) => Err(exc("OverflowError", "Python int too large to convert to C int".into())),
        Some(o) => Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(&o)))),
        None => Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v)))),
    }
}

// METH_NOARGS bytes methods (upper, capitalize, ...): CPython's "takes no arguments" errors.
fn no_bytes_args(n: &str, a: &Args) -> Result<(), E> {
    if !a.k.is_empty() {
        return type_err(format!("bytes.{n}() takes no keyword arguments"));
    }
    if !a.p.is_empty() {
        return type_err(format!("bytes.{n}() takes no arguments ({} given)", a.p.len()));
    }
    Ok(())
}

// The keyword-less bytes methods refuse any keyword outright (CPython 3.13's positional-only methods).
fn no_kws(n: &str, a: &Args) -> Result<(), E> {
    if let Some((k, _)) = a.k.first() {
        return type_err(format!("bytes.{n}() takes no keyword arguments"));
    }
    Ok(())
}

// "find expected at least 1 argument, got 0": PyArg_UnpackTuple-style range checks, except that a
// fixed count says "expected N arguments" (maketrans).
fn unpack_range(f: &str, a: &Args, lo: usize, hi: usize) -> Result<(), E> {
    if lo == hi {
        if a.p.len() != lo {
            return type_err(format!("{f} expected {lo} argument{}, got {}", if lo == 1 { "" } else { "s" }, a.p.len()));
        }
        return Ok(());
    }
    if a.p.len() < lo {
        return type_err(format!("{f} expected at least {lo} argument{}, got {}", if lo == 1 { "" } else { "s" }, a.p.len()));
    }
    if a.p.len() > hi {
        return type_err(format!("{f} expected at most {hi} argument{}, got {}", if hi == 1 { "" } else { "s" }, a.p.len()));
    }
    Ok(())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

// The closest known keyword one edit away, as CPython's "Did you mean" suggestion does.
fn closest_kw<'a>(k: &str, known: &'a [&'a str]) -> Option<&'a str> {
    let k = k.to_lowercase();
    let mut best: Option<(usize, &'a str)> = None;
    for &n in known {
        let d = levenshtein(&k, &n.to_lowercase());
        if d <= 1 && best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, n));
        }
    }
    best.map(|(_, n)| n)
}

// "split() got an unexpected keyword argument 'x'. Did you mean 'sep'?"
fn unexpected_kw(f: &str, a: &Args, known: &[&str]) -> Result<(), E> {
    let Some((k, _)) = a.k.first() else { return Ok(()) };
    let mut msg = format!("{f}() got an unexpected keyword argument '{k}'");
    if let Some(s) = closest_kw(k, known) {
        msg.push_str(&format!(". Did you mean '{s}'?"));
    }
    type_err(msg)
}

// (start, end) of a find/count/startswith range. Negative values add len; the upper end is not
// clamped here, because CPython uses the raw values to decide the empty-needle cases.
fn sub_range(a: &Args, at: usize, n: usize) -> Result<(i64, i64), E> {
    let get = |k: usize, d: i64| -> Result<i64, E> {
        match a.p.get(k) {
            None | Some(V::None) => Ok(d),
            Some(x) => slice_bound(x),
        }
    };
    let n = n as i64;
    let adj = |x: i64| if x < 0 { x.saturating_add(n).max(0) } else { x };
    Ok((adj(get(at, 0)?), adj(get(at + 1, n)?)))
}

fn codec_name(v: Option<&V>) -> Result<&'static str, E> {
    let n = match v {
        None => return Ok("utf-8"),
        Some(V::Str(s)) => s.s.to_lowercase().replace('_', "-"),
        Some(x) => return type_err(format!("decode() argument 'encoding' must be str, not {}", tn(x))),
    };
    Ok(match n.as_str() {
        "utf-8" | "utf8" | "u8" | "utf" | "cp65001" => "utf-8",
        "ascii" | "us-ascii" | "646" => "ascii",
        "latin-1" | "latin1" | "iso-8859-1" | "iso8859-1" | "l1" | "8859" | "cp819" | "latin" => "latin-1",
        // a codec CPython has and Brevi does not: not "unknown"
        m if ["utf-16", "utf16", "utf-32", "utf32", "utf-7", "utf-8-sig", "cp", "iso-8859", "iso8859", "koi8", "mac-", "gb", "big5", "euc", "shift", "sjis", "latin-", "windows-", "idna", "punycode", "unicode-escape", "raw-unicode-escape", "hz", "johab", "ptcp", "tis"].iter().any(|p| m.starts_with(p)) => {
            return Err(unsupported(&format!("the {m} codec")))
        }
        _ => return Err(exc("LookupError", format!("unknown encoding: {}", match v { Some(V::Str(s)) => s.s.clone(), _ => n }))),
    })
}

fn errors_name(v: Option<&V>) -> Result<String, E> {
    match v {
        None => Ok("strict".into()),
        Some(V::Str(s)) => Ok(s.s.clone()),
        Some(x) => type_err(format!("decode() argument 'errors' must be str, not {}", tn(x))),
    }
}

pub fn bytes_decode(b: &[u8], enc: Option<&V>, errors: Option<&V>) -> R {
    let codec = codec_name(enc)?;
    let errors = errors_name(errors)?;
    let errors = errors.to_lowercase();
    if !matches!(errors.as_str(), "strict" | "replace" | "ignore" | "backslashreplace") {
        return Err(unsupported(&format!("decode errors='{errors}'")));
    }
    match codec {
        "latin-1" => Ok(st(b.iter().map(|&c| c as char).collect())),
        "ascii" => {
            let mut out = String::new();
            for (i, &c) in b.iter().enumerate() {
                if c < 128 {
                    out.push(c as char);
                } else if errors == "strict" {
                    return Err(udec_err("ascii", b, i, i + 1, "ordinal not in range(128)"));
                } else if errors == "replace" {
                    out.push('\u{fffd}');
                } else if errors == "backslashreplace" {
                    out.push_str(&format!("\\x{c:02x}"));
                }
            }
            Ok(st(out))
        }
        _ => {
            let mut out = String::new();
            let mut rest = b;
            let mut base = 0usize;
            loop {
                match std::str::from_utf8(rest) {
                    Ok(t) => {
                        out.push_str(t);
                        return Ok(st(out));
                    }
                    Err(e) => {
                        let ok = e.valid_up_to();
                        out.push_str(std::str::from_utf8(&rest[..ok]).unwrap());
                        let pos = base + ok;
                        let bad = e.error_len();
                        if errors == "strict" {
                            let c = rest[ok];
                            let (end, reason) = match bad {
                                None => (base + rest.len(), "unexpected end of data"),
                                Some(_) if (0x80..=0xc1).contains(&c) || c >= 0xf5 => (pos + 1, "invalid start byte"),
                                Some(n) => (pos + n, "invalid continuation byte"),
                            };
                            return Err(udec_err("utf-8", b, pos, end, reason));
                        }
                        let skip = bad.unwrap_or(rest.len() - ok);
                        if errors == "backslashreplace" {
                            // backslashreplace escapes each byte of the bad run with \xhh
                            for &c in &rest[ok..ok + skip] {
                                out.push_str(&format!("\\x{c:02x}"));
                            }
                        } else if errors == "replace" {
                            out.push('\u{fffd}');
                        }
                        rest = &rest[ok + skip..];
                        base += ok + skip;
                    }
                }
            }
        }
    }
}

// UnicodeDecodeError(encoding, object, start, end, reason), with its attributes (str() is in rt_fmt.rs)
pub fn udec_err(enc: &str, obj: &[u8], start: usize, end: usize, reason: &str) -> E {
    let o = Obj::new(exc_class("UnicodeDecodeError"));
    let (e, b, st0, en, r) = (s(enc), bytesv(obj.to_vec()), V::Int(start as i64), V::Int(end as i64), s(reason));
    o.set("args", tup(vec![e.clone(), b.clone(), st0.clone(), en.clone(), r.clone()]));
    o.set("encoding", e);
    o.set("object", b);
    o.set("start", st0);
    o.set("end", en);
    o.set("reason", r);
    E(V::Obj(o))
}

fn char_repr(c: char) -> String {
    let n = c as u32;
    if n < 0x100 {
        format!("'\\x{n:02x}'")
    } else if n < 0x10000 {
        format!("'\\u{n:04x}'")
    } else {
        format!("'\\U{n:08x}'")
    }
}

pub fn str_encode(s: &str, enc: Option<&V>, errors: Option<&V>) -> R {
    let codec = codec_name(enc)?;
    let errors = errors_name(errors)?;
    if !matches!(errors.as_str(), "strict" | "replace" | "ignore") {
        return Err(unsupported(&format!("encode errors='{errors}'")));
    }
    if codec == "utf-8" {
        return Ok(bytesv(s.as_bytes().to_vec()));
    }
    let lim: u32 = if codec == "ascii" { 128 } else { 256 };
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if (c as u32) < lim {
            out.push(c as u32 as u8);
            i += 1;
            continue;
        }
        if errors == "strict" {
            let mut j = i;
            while j < cs.len() && cs[j] as u32 >= lim {
                j += 1;
            }
            let msg = if j - i == 1 {
                format!("'{codec}' codec can't encode character {} in position {i}: ordinal not in range({lim})", char_repr(c))
            } else {
                format!("'{codec}' codec can't encode characters in position {i}-{}: ordinal not in range({lim})", j - 1)
            };
            return Err(exc("UnicodeEncodeError", msg));
        }
        if errors == "replace" {
            out.push(b'?');
        }
        i += 1;
    }
    Ok(bytesv(out))
}

// bytes(...): b'', zeros, ints, an encoded str, or a copy.
pub fn bi_bytes(a: Args) -> R {
    let mut a = a;
    let enc = a.kw("encoding");
    let errs = a.kw("errors");
    a.no_kw("bytes")?;
    if a.p.len() > 3 {
        return type_err(format!("bytes() takes at most 3 arguments ({} given)", a.p.len()));
    }
    let enc = enc.or_else(|| a.p.get(1).cloned());
    let errs = errs.or_else(|| a.p.get(2).cloned());
    let Some(x) = a.p.first() else {
        if enc.is_some() || errs.is_some() {
            return type_err("encoding without a string argument".into());
        }
        return Ok(bytesv(vec![]));
    };
    if let V::Str(s) = x {
        if enc.is_none() {
            return type_err("string argument without an encoding".into());
        }
        return str_encode(&s.s, enc.as_ref(), errs.as_ref());
    }
    if enc.is_some() {
        return type_err("encoding without a string argument".into());
    }
    if errs.is_some() {
        return type_err("errors without a string argument".into());
    }
    if let Some(b) = bytes_of(x) {
        return Ok(bytesv(b));
    }
    if let V::Int(n) = x {
        if *n < 0 {
            return val_err("negative count".into());
        }
        return Ok(bytesv(vec![0; *n as usize]));
    }
    if matches!(x, V::Float(_) | V::None | V::Dict(_)) && !matches!(x, V::Dict(_)) {
        return type_err(format!("cannot convert '{}' object to bytes", tn(x)));
    }
    let mut out = vec![];
    for v in to_vec(x)? {
        let i = match v {
            V::Int(i) => i,
            V::Bool(b) => b as i64,
            V::Big(_) => 1 << 20,
            o => return type_err(format!("'{}' object cannot be interpreted as an integer", tn(&o))),
        };
        if !(0..256).contains(&i) {
            return val_err("bytes must be in range(0, 256)".into());
        }
        out.push(i as u8);
    }
    Ok(bytesv(out))
}

// b"..." in the program: its bytes are the chars (all below 256) of the str the parser made.
pub fn bi___bytes_lit(a: Args) -> R {
    match a.p.first() {
        Some(V::Str(s)) => Ok(bytesv(s.s.chars().map(|c| c as u32 as u8).collect())),
        _ => Ok(bytesv(vec![])),
    }
}

pub fn bytes_fromhex(a: Args) -> R {
    let s = match a.p.first() {
        Some(V::Str(s)) => s.s.clone(),
        Some(x) => return type_err(format!("fromhex() argument must be str, not {}", tn(x))),
        None => return type_err("fromhex() takes exactly one argument (0 given)".into()),
    };
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        if cs[i].is_whitespace() && (cs[i] as u32) < 128 {
            i += 1;
            continue;
        }
        let hi = cs[i].to_digit(16).filter(|_| cs[i].is_ascii());
        let lo = cs.get(i + 1).and_then(|c| c.to_digit(16).filter(|_| c.is_ascii()));
        match (hi, lo) {
            (Some(h), Some(l)) => out.push((h * 16 + l) as u8),
            (None, _) => return val_err(format!("non-hexadecimal number found in fromhex() arg at position {i}")),
            (Some(_), None) => return val_err(format!("non-hexadecimal number found in fromhex() arg at position {}", i + 1)),
        }
        i += 2;
    }
    Ok(bytesv(out))
}

// bytes.maketrans(frm, to): the 256-byte table that maps frm's bytes to to's (CPython's static method).
pub fn bytes_maketrans(a: Args) -> R {
    let a = a;
    no_kws("maketrans", &a)?;
    unpack_range("maketrans", &a, 2, 2)?;
    let frm = sub_arg(&a.p[0], false)?;
    let to = sub_arg(&a.p[1], false)?;
    if frm.len() != to.len() {
        return val_err("maketrans arguments must have same length".into());
    }
    let mut table: Vec<u8> = (0..=255).collect();
    for (f, t) in frm.iter().zip(to.iter()) {
        table[*f as usize] = *t;
    }
    Ok(bytesv(table))
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|c| format!("{c:02x}")).collect()
}

fn bytes_split(b: &[u8], sep: Option<&[u8]>, max: i64) -> Result<Vec<V>, E> {
    let mut out = vec![];
    match sep {
        Some(sep) => {
            if sep.is_empty() {
                return val_err("empty separator".into());
            }
            let mut from = 0;
            let mut n = 0;
            while max < 0 || n < max {
                match find_sub(b, sep, from) {
                    Some(k) => {
                        out.push(bytesv(b[from..k].to_vec()));
                        from = k + sep.len();
                        n += 1;
                    }
                    None => break,
                }
            }
            out.push(bytesv(b[from..].to_vec()));
        }
        None => {
            // 3.13's whitespace split: with maxsplit, the first split consumes the leading
            // whitespace run and the remainder keeps its trailing whitespace; without, all runs split.
            let mut i = 0;
            let mut n = 0;
            while max < 0 || n < max {
                while i < b.len() && is_py_space(b[i]) {
                    i += 1;
                }
                if i >= b.len() {
                    return Ok(out);
                }
                let st = i;
                while i < b.len() && !is_py_space(b[i]) {
                    i += 1;
                }
                out.push(bytesv(b[st..i].to_vec()));
                n += 1;
            }
            if max >= 0 {
                while i < b.len() && is_py_space(b[i]) {
                    i += 1;
                }
                if i < b.len() {
                    out.push(bytesv(b[i..].to_vec()));
                }
            }
        }
    }
    Ok(out)
}

fn bytes_rsplit(b: &[u8], sep: Option<&[u8]>, max: i64) -> Result<Vec<V>, E> {
    match sep {
        Some(sep) => {
            if sep.is_empty() {
                return val_err("empty separator".into());
            }
            let mut pos = vec![];
            let mut from = 0;
            while let Some(k) = find_sub(b, sep, from) {
                pos.push(k);
                from = k + sep.len();
            }
            let n = if max < 0 { pos.len() as i64 } else { max.min(pos.len() as i64) };
            let mut out = vec![];
            let mut cur = 0;
            for &k in &pos[pos.len() - n as usize..] {
                out.push(bytesv(b[cur..k].to_vec()));
                cur = k + sep.len();
            }
            out.push(bytesv(b[cur..].to_vec()));
            Ok(out)
        }
        None => {
            let mut words = vec![];
            let mut end = b.len();
            if max == 0 {
                // no splits: the trailing whitespace run is consumed, the leading one kept
                while end > 0 && is_py_space(b[end - 1]) {
                    end -= 1;
                }
                if end > 0 {
                    words.push(bytesv(b[..end].to_vec()));
                }
                return Ok(words);
            }
            loop {
                let mut e = end;
                while e > 0 && is_py_space(b[e - 1]) {
                    e -= 1;
                }
                if e == 0 {
                    break;
                }
                let mut s = e;
                while s > 0 && !is_py_space(b[s - 1]) {
                    s -= 1;
                }
                words.push(bytesv(b[s..e].to_vec()));
                if max > 0 && words.len() as i64 >= max {
                    // the whitespace run before the last word found is consumed, too
                    let mut ws = s;
                    while ws > 0 && is_py_space(b[ws - 1]) {
                        ws -= 1;
                    }
                    if ws > 0 {
                        words.push(bytesv(b[..ws].to_vec()));
                    }
                    break;
                }
                end = s;
            }
            words.reverse();
            Ok(words)
        }
    }
}

impl BytesV {
    fn call(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let b = &self.0;
        let a = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
        Some((|| -> R {
            let mut a = a;
            Ok(match n {
                "__getitem__" => match a.p.first() {
                    Some(V::Slice(sl)) => {
                        let (st, en, step) = slice_idx(sl, b.len())?;
                        let mut out = vec![];
                        let mut i = st;
                        while (step > 0 && i < en) || (step < 0 && i > en) {
                            out.push(b[i as usize]);
                            i += step;
                        }
                        bytesv(out)
                    }
                    Some(i) => {
                        let k = match i {
                            V::Int(_) | V::Bool(_) | V::Big(_) => idx_of(i)?,
                            o => return type_err(format!("byte indices must be integers or slices, not {}", tn(o))),
                        };
                        let k = if k < 0 { k + b.len() as i64 } else { k };
                        if k < 0 || k >= b.len() as i64 {
                            return Err(exc("IndexError", "index out of range".into()));
                        }
                        V::Int(b[k as usize] as i64)
                    }
                    None => return type_err("expected 1 argument".into()),
                },
                "__contains__" => {
                    let x = a.p.first().cloned().unwrap_or(V::None);
                    let sub = sub_arg(&x, true)?;
                    V::Bool(find_sub(b, &sub, 0).is_some())
                }
                "__len__" => {
                    if !a.p.is_empty() {
                        return type_err(format!("expected 0 arguments, got {}", a.p.len()));
                    }
                    V::Int(b.len() as i64)
                }
                "__bytes__" => {
                    no_bytes_args("__bytes__", &a)?;
                    me.clone()
                }
                "decode" => {
                    if a.p.len() + a.k.len() > 2 {
                        return type_err(format!("decode() takes at most 2 arguments ({} given)", a.p.len() + a.k.len()));
                    }
                    let has_kw = |name: &str| a.k.iter().any(|(k, _)| *k == name);
                    if has_kw("encoding") && !a.p.is_empty() {
                        return type_err("argument for decode() given by name ('encoding') and position (1)".into());
                    }
                    if has_kw("errors") && a.p.len() >= 2 {
                        return type_err("argument for decode() given by name ('errors') and position (2)".into());
                    }
                    let enc = a.kw("encoding").or_else(|| a.p.first().cloned());
                    let errs = a.kw("errors").or_else(|| a.p.get(1).cloned());
                    unexpected_kw("decode", &a, &["encoding", "errors"])?;
                    bytes_decode(b, enc.as_ref(), errs.as_ref())?
                }
                "hex" => {
                    if a.p.len() + a.k.len() > 2 {
                        return type_err(format!("hex() takes at most 2 arguments ({} given)", a.p.len() + a.k.len()));
                    }
                    let has_kw = |name: &str| a.k.iter().any(|(k, _)| *k == name);
                    if has_kw("sep") && !a.p.is_empty() {
                        return type_err("argument for hex() given by name ('sep') and position (1)".into());
                    }
                    if has_kw("bytes_per_sep") && a.p.len() >= 2 {
                        return type_err("argument for hex() given by name ('bytes_per_sep') and position (2)".into());
                    }
                    let sep = a.kw("sep").or_else(|| a.p.first().cloned());
                    let bps = a.kw("bytes_per_sep").or_else(|| a.p.get(1).cloned());
                    unexpected_kw("hex", &a, &["sep", "bytes_per_sep"])?;
                    let bps = match &bps {
                        None => 1i64,
                        Some(v) => c_int_arg(v)?,
                    };
                    let sep_byte = match &sep {
                        // no separator: CPython's default is an empty separator, so the output is plain hex
                        None => return Ok(st(hex_of(b))),
                        Some(V::Str(sd)) => {
                            let mut cs = sd.s.chars();
                            match (cs.next(), cs.next()) {
                                (Some(c), None) if (c as u32) < 128 => c as u32 as u8,
                                (Some(_), None) => return val_err("sep must be ASCII.".into()),
                                _ => return val_err("sep must be length 1.".into()),
                            }
                        }
                        Some(x) => match bytes_of(x) {
                            Some(bs) => match bs.len() {
                                1 if bs[0] < 128 => bs[0],
                                1 => return val_err("sep must be ASCII.".into()),
                                _ => return val_err("sep must be length 1.".into()),
                            },
                            None => {
                                // CPython takes PyObject_Length(sep): without __len__ it is "object of
                                // type 'X' has no len()", a length != 1 is "sep must be length 1.", and
                                // a length of 1 that is not str or bytes is "sep must be str or bytes.".
                                return match len(x) {
                                    Ok(n) => {
                                        if n != 1 {
                                            val_err("sep must be length 1.".into())
                                        } else {
                                            type_err("sep must be str or bytes.".into())
                                        }
                                    }
                                    Err(_) => type_err(format!("object of type '{}' has no len()", tn(x))),
                                };
                            }
                        },
                    };
                    if bps == 0 {
                        return Ok(st(hex_of(b)));
                    }
                    let g = bps.unsigned_abs() as usize;
                    let mut out = String::new();
                    for (k, c) in b.iter().enumerate() {
                        if k > 0 {
                            // positive: groups from the right; negative: from the left
                            let boundary = if bps > 0 { (b.len() - k) % g == 0 } else { k % g == 0 };
                            if boundary {
                                out.push(sep_byte as char);
                            }
                        }
                        out.push_str(&format!("{c:02x}"));
                    }
                    st(out)
                }
                "startswith" | "endswith" => {
                    no_kws(n, &a)?;
                    unpack_range(n, &a, 1, 3)?;
                    let (st, en) = sub_range(&a, 1, b.len())?;
                    let nlen = b.len() as i64;
                    let lo = st.min(nlen);
                    let hi = en.min(nlen);
                    let test = |p: &[u8], w: &[u8]| if n == "startswith" { w.starts_with(p) } else { w.ends_with(p) };
                    match a.p.first() {
                        Some(V::Tuple(t)) => {
                            for x in t.iter() {
                                let p = sub_arg(x, false)?;
                                if p.is_empty() {
                                    // an empty needle matches only when start <= end and start <= len
                                    if st <= en && st <= nlen {
                                        return Ok(V::Bool(true));
                                    }
                                } else {
                                    let w = if hi > lo { &b[lo as usize..hi as usize] } else { &[] };
                                    if test(&p, w) {
                                        return Ok(V::Bool(true));
                                    }
                                }
                            }
                            V::Bool(false)
                        }
                        Some(x) => match bytes_of(x) {
                            Some(p) => {
                                if p.is_empty() {
                                    return Ok(V::Bool(st <= en && st <= nlen));
                                }
                                let w = if hi > lo { &b[lo as usize..hi as usize] } else { &[] };
                                V::Bool(test(&p, w))
                            }
                            None => return type_err(format!("{n} first arg must be bytes or a tuple of bytes, not {}", tn(x))),
                        },
                        None => unreachable!(),
                    }
                }
                "find" | "index" | "rfind" | "rindex" | "count" => {
                    no_kws(n, &a)?;
                    unpack_range(n, &a, 1, 3)?;
                    // CPython converts start/end before the needle, so their TypeError wins
                    let (st, en) = sub_range(&a, 1, b.len())?;
                    let x = a.p.first().cloned().unwrap_or(V::None);
                    let sub = find_needle(&x)?;
                    let nlen = b.len() as i64;
                    if sub.is_empty() {
                        // an empty needle is found once, when start <= end and start <= len: find at
                        // start, rfind at the end of the window
                        if n == "count" {
                            if st > nlen {
                                return Ok(V::Int(0));
                            }
                            let hi = en.min(nlen);
                            return Ok(V::Int((hi - st + 1).max(0)));
                        }
                        let hit = if st > en || st > nlen {
                            None
                        } else if n.starts_with('r') {
                            Some(en.min(nlen))
                        } else {
                            Some(st)
                        };
                        match hit {
                            Some(k) => return Ok(V::Int(k)),
                            None if n.ends_with("index") => return val_err("subsection not found".into()),
                            None => return Ok(V::Int(-1)),
                        }
                    }
                    let lo = st.max(0).min(nlen);
                    let hi = en.min(nlen).max(lo);
                    let w = &b[lo as usize..hi as usize];
                    if n == "count" {
                        let mut c = 0;
                        let mut from = 0;
                        while let Some(k) = find_sub(w, &sub, from) {
                            c += 1;
                            from = k + sub.len();
                        }
                        return Ok(V::Int(c));
                    }
                    let hit = if n.starts_with('r') {
                        (0..=w.len().saturating_sub(sub.len())).rev().find(|&i| w.len() >= sub.len() && &w[i..i + sub.len()] == sub.as_slice())
                    } else {
                        find_sub(w, &sub, 0)
                    };
                    match hit {
                        Some(k) => V::Int(k as i64 + lo),
                        None if n.ends_with("index") => return val_err("subsection not found".into()),
                        None => V::Int(-1),
                    }
                }
                "split" | "rsplit" => {
                    if a.p.len() + a.k.len() > 2 {
                        return type_err(format!("{n}() takes at most 2 arguments ({} given)", a.p.len() + a.k.len()));
                    }
                    let has_kw = |name: &str| a.k.iter().any(|(k, _)| *k == name);
                    if has_kw("sep") && !a.p.is_empty() {
                        return type_err(format!("argument for {n}() given by name ('sep') and position (1)"));
                    }
                    if has_kw("maxsplit") && a.p.len() >= 2 {
                        return type_err(format!("argument for {n}() given by name ('maxsplit') and position (2)"));
                    }
                    let sep = a.kw("sep").or_else(|| a.p.first().cloned());
                    let max = a.kw("maxsplit").or_else(|| a.p.get(1).cloned());
                    unexpected_kw(n, &a, &["sep", "maxsplit"])?;
                    let max = match max {
                        None => -1,
                        Some(m) => ssize_arg(&m)?,
                    };
                    let sep = match sep {
                        None | Some(V::None) => None,
                        Some(x) => Some(sub_arg(&x, false)?),
                    };
                    if n == "split" {
                        list(bytes_split(b, sep.as_deref(), max)?)
                    } else {
                        list(bytes_rsplit(b, sep.as_deref(), max)?)
                    }
                }
                "strip" | "lstrip" | "rstrip" => {
                    no_kws(n, &a)?;
                    if a.p.len() > 1 {
                        return type_err(format!("{n} expected at most 1 argument, got {}", a.p.len()));
                    }
                    let chars = match a.p.first() {
                        None | Some(V::None) => None,
                        Some(x) => Some(sub_arg(x, false)?),
                    };
                    let keep = |c: u8| match &chars {
                        None => !is_py_space(c),
                        Some(cs) => !cs.contains(&c),
                    };
                    let mut lo = 0;
                    let mut hi = b.len();
                    if n != "rstrip" {
                        while lo < hi && !keep(b[lo]) {
                            lo += 1;
                        }
                    }
                    if n != "lstrip" {
                        while hi > lo && !keep(b[hi - 1]) {
                            hi -= 1;
                        }
                    }
                    bytesv(b[lo..hi].to_vec())
                }
                "join" => {
                    no_kws(n, &a)?;
                    if a.p.len() != 1 {
                        return type_err(format!("bytes.join() takes exactly one argument ({} given)", a.p.len()));
                    }
                    let x = &a.p[0];
                    let mut it = match iter(x) {
                        Ok(it) => it,
                        Err(_) => return type_err("can only join an iterable".into()),
                    };
                    let mut out = vec![];
                    let mut i = 0;
                    while let Some(x) = it.next()? {
                        if i > 0 {
                            out.extend_from_slice(b);
                        }
                        match bytes_of(&x) {
                            Some(p) => out.extend(p),
                            None => return type_err(format!("sequence item {i}: expected a bytes-like object, {} found", tn(&x))),
                        }
                        i += 1;
                    }
                    bytesv(out)
                }
                "replace" => {
                    no_kws(n, &a)?;
                    unpack_range(n, &a, 2, 3)?;
                    let old = sub_arg(&a.p[0], false)?;
                    let new = sub_arg(&a.p[1], false)?;
                    let max = match a.p.get(2) {
                        None => -1,
                        Some(m) => ssize_arg(m)?,
                    };
                    let mut out = vec![];
                    let mut from = 0;
                    let mut done = 0;
                    if old.is_empty() {
                        for (i, &c) in b.iter().enumerate() {
                            if max < 0 || (i as i64) < max {
                                out.extend_from_slice(&new);
                            }
                            out.push(c);
                        }
                        if max < 0 || (b.len() as i64) < max {
                            out.extend_from_slice(&new);
                        }
                        return Ok(bytesv(out));
                    }
                    while max < 0 || done < max {
                        match find_sub(b, &old, from) {
                            Some(k) => {
                                out.extend_from_slice(&b[from..k]);
                                out.extend_from_slice(&new);
                                from = k + old.len();
                                done += 1;
                            }
                            None => break,
                        }
                    }
                    out.extend_from_slice(&b[from..]);
                    bytesv(out)
                }
                "removeprefix" | "removesuffix" => {
                    no_kws(n, &a)?;
                    if a.p.len() != 1 {
                        return type_err(format!("bytes.{n}() takes exactly one argument ({} given)", a.p.len()));
                    }
                    let p = sub_arg(&a.p[0], false)?;
                    if n == "removeprefix" && b.starts_with(&p) {
                        bytesv(b[p.len()..].to_vec())
                    } else if n == "removesuffix" && b.ends_with(&p) && !p.is_empty() {
                        bytesv(b[..b.len() - p.len()].to_vec())
                    } else {
                        bytesv(b.clone())
                    }
                }
                "upper" | "lower" | "capitalize" | "title" | "istitle" | "swapcase" | "isdigit" | "isalpha" | "isalnum" | "isspace" | "isupper" | "islower" | "isascii" => {
                    no_bytes_args(n, &a)?;
                    match n {
                        "upper" => bytesv(b.to_ascii_uppercase()),
                        "lower" => bytesv(b.to_ascii_lowercase()),
                        "capitalize" => {
                            let mut out = b.clone();
                            if !out.is_empty() {
                                out[0] = out[0].to_ascii_uppercase();
                                for c in &mut out[1..] {
                                    *c = c.to_ascii_lowercase();
                                }
                            }
                            bytesv(out)
                        }
                        "title" => {
                            // a letter is uppercased after a non-letter, lowercased otherwise
                            let mut out = vec![];
                            let mut prev_cased = false;
                            for &c in b.iter() {
                                let d = if c.is_ascii_alphabetic() {
                                    if prev_cased { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() }
                                } else {
                                    c
                                };
                                out.push(d);
                                prev_cased = c.is_ascii_alphabetic();
                            }
                            bytesv(out)
                        }
                        "istitle" => {
                            let mut cased = false;
                            let mut prev_cased = false;
                            let mut ok = true;
                            for &c in b.iter() {
                                if c.is_ascii_uppercase() {
                                    if prev_cased {
                                        ok = false;
                                        break;
                                    }
                                    prev_cased = true;
                                    cased = true;
                                } else if c.is_ascii_lowercase() {
                                    if !prev_cased {
                                        ok = false;
                                        break;
                                    }
                                    prev_cased = true;
                                    cased = true;
                                } else {
                                    prev_cased = false;
                                }
                            }
                            V::Bool(cased && ok)
                        }
                        "swapcase" => bytesv(b.iter().map(|&c| {
                            if c.is_ascii_uppercase() {
                                c.to_ascii_lowercase()
                            } else if c.is_ascii_lowercase() {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            }
                        }).collect()),
                        "isdigit" => V::Bool(!b.is_empty() && b.iter().all(|c| c.is_ascii_digit())),
                        "isalpha" => V::Bool(!b.is_empty() && b.iter().all(|c| c.is_ascii_alphabetic())),
                        "isalnum" => V::Bool(!b.is_empty() && b.iter().all(|c| c.is_ascii_alphanumeric())),
                        "isspace" => V::Bool(!b.is_empty() && b.iter().all(|&c| is_py_space(c))),
                        "isupper" => V::Bool(b.iter().any(|c| c.is_ascii_uppercase()) && !b.iter().any(|c| c.is_ascii_lowercase())),
                        "islower" => V::Bool(b.iter().any(|c| c.is_ascii_lowercase()) && !b.iter().any(|c| c.is_ascii_uppercase())),
                        _ => V::Bool(b.is_ascii()),
                    }
                }
                "center" | "ljust" | "rjust" => {
                    no_kws(n, &a)?;
                    unpack_range(n, &a, 1, 2)?;
                    let width = ssize_arg(&a.p[0])?;
                    let fill = match a.p.get(1) {
                        None => b' ',
                        Some(v) => match bytes_of(v) {
                            Some(bs) if bs.len() == 1 => bs[0],
                            Some(_) => return type_err(format!("{n}() argument 2 must be a byte string of length 1, not bytes")),
                            // CPython prints "not None" for None, not the NoneType tn() gives
                            None => return type_err(format!("{n}() argument 2 must be a byte string of length 1, not {}", match v { V::None => "None".into(), _ => tn(v) })),
                        },
                    };
                    let w = width.max(0) as usize;
                    if w <= b.len() {
                        return Ok(bytesv(b.clone()));
                    }
                    let pad = w - b.len();
                    // CPython: the extra padding goes on the left when the total is odd
                    let (left, right) = match n {
                        "center" => (pad / 2 + (pad & w & 1), pad - (pad / 2 + (pad & w & 1))),
                        "ljust" => (0, pad),
                        _ => (pad, 0),
                    };
                    let mut out = Vec::new();
                    if out.try_reserve_exact(w).is_err() {
                        return Err(exc0("MemoryError"));
                    }
                    out.extend(std::iter::repeat(fill).take(left));
                    out.extend_from_slice(b);
                    out.extend(std::iter::repeat(fill).take(right));
                    bytesv(out)
                }
                "zfill" => {
                    no_kws(n, &a)?;
                    if a.p.len() != 1 {
                        return type_err(format!("bytes.zfill() takes exactly one argument ({} given)", a.p.len()));
                    }
                    let width = ssize_arg(&a.p[0])?;
                    let w = width.max(0) as usize;
                    if w <= b.len() {
                        return Ok(bytesv(b.clone()));
                    }
                    let sign = b.first().is_some_and(|&c| c == b'+' || c == b'-');
                    let pad = w - b.len();
                    let mut out = Vec::new();
                    if out.try_reserve_exact(w).is_err() {
                        return Err(exc0("MemoryError"));
                    }
                    if sign {
                        out.push(b[0]);
                        out.extend(std::iter::repeat(b'0').take(pad));
                        out.extend_from_slice(&b[1..]);
                    } else {
                        out.extend(std::iter::repeat(b'0').take(pad));
                        out.extend_from_slice(b);
                    }
                    bytesv(out)
                }
                "expandtabs" => {
                    if a.p.len() + a.k.len() > 1 {
                        return type_err(format!("expandtabs() takes at most 1 argument ({} given)", a.p.len() + a.k.len()));
                    }
                    let ts = match a.kw("tabsize").or_else(|| a.p.first().cloned()) {
                        None => 8,
                        Some(v) => c_int_arg(&v)?,
                    };
                    unexpected_kw("expandtabs", &a, &["tabsize"])?;
                    let mut out: Vec<u8> = vec![];
                    let mut col: u64 = 0;
                    for &c in b {
                        if c == b'\t' {
                            if ts > 0 {
                                // a tab advances to the next multiple of tabsize; tabsize <= 0 drops it
                                let incr = ts as u64 - (col % ts as u64);
                                if out.try_reserve(incr as usize).is_err() {
                                    return Err(exc0("MemoryError"));
                                }
                                out.extend(std::iter::repeat(b' ').take(incr as usize));
                                col += incr;
                            }
                        } else {
                            out.push(c);
                            col += 1;
                            if c == b'\n' || c == b'\r' {
                                col = 0;
                            }
                        }
                    }
                    bytesv(out)
                }
                "splitlines" => {
                    if a.p.len() > 1 {
                        return type_err(format!("splitlines() takes at most 1 argument ({} given)", a.p.len()));
                    }
                    let has_kw = |name: &str| a.k.iter().any(|(k, _)| *k == name);
                    if has_kw("keepends") && !a.p.is_empty() {
                        return type_err("argument for splitlines() given by name ('keepends') and position (1)".into());
                    }
                    let keep = a.kw("keepends").or_else(|| a.p.first().cloned());
                    unexpected_kw("splitlines", &a, &["keepends"])?;
                    let keep = match keep {
                        Some(v) => truth(&v)?,
                        None => false,
                    };
                    let mut out = vec![];
                    let mut st = 0;
                    let mut i = 0;
                    while i < b.len() {
                        if b[i] == b'\n' || b[i] == b'\r' {
                            let e = if b[i] == b'\r' && b.get(i + 1) == Some(&b'\n') { i + 2 } else { i + 1 };
                            // keepends keeps the whole terminator (\r\n); otherwise the piece ends before it
                            let end = if keep { e } else { i };
                            out.push(bytesv(b[st..end].to_vec()));
                            st = e;
                            i = e;
                        } else {
                            i += 1;
                        }
                    }
                    if st < b.len() {
                        out.push(bytesv(b[st..].to_vec()));
                    }
                    list(out)
                }
                "partition" | "rpartition" => {
                    no_kws(n, &a)?;
                    if a.p.len() != 1 {
                        return type_err(format!("bytes.{n}() takes exactly one argument ({} given)", a.p.len()));
                    }
                    let sep = sub_arg(&a.p[0], false)?;
                    if sep.is_empty() {
                        return val_err("empty separator".into());
                    }
                    let hit = if n == "partition" {
                        find_sub(b, &sep, 0)
                    } else {
                        (0..=b.len().saturating_sub(sep.len())).rev().find(|&i| &b[i..i + sep.len()] == sep.as_slice())
                    };
                    match hit {
                        Some(k) => tup(vec![bytesv(b[..k].to_vec()), bytesv(sep.clone()), bytesv(b[k + sep.len()..].to_vec())]),
                        None if n == "partition" => tup(vec![bytesv(b.clone()), bytesv(vec![]), bytesv(vec![])]),
                        None => tup(vec![bytesv(vec![]), bytesv(vec![]), bytesv(b.clone())]),
                    }
                }
                "translate" => {
                    if a.p.len() + a.k.len() > 2 {
                        return type_err(format!("translate() takes at most 2 arguments ({} given)", a.p.len() + a.k.len()));
                    }
                    if a.p.is_empty() {
                        return type_err("translate() takes at least 1 positional argument (0 given)".into());
                    }
                    let has_kw = |name: &str| a.k.iter().any(|(k, _)| *k == name);
                    if has_kw("delete") && a.p.len() >= 2 {
                        return type_err("argument for translate() given by name ('delete') and position (2)".into());
                    }
                    let table = a.p.first().cloned();
                    let delete = a.kw("delete").or_else(|| a.p.get(1).cloned());
                    if let Some((k, _)) = a.k.first() {
                        return type_err(format!("translate() got an unexpected keyword argument '{k}'"));
                    }
                    let table = match table {
                        Some(V::None) => None,
                        Some(x) => match bytes_of(&x) {
                            Some(tb) if tb.len() == 256 => Some(tb),
                            Some(_) => return val_err("translation table must be 256 characters long".into()),
                            None => return type_err(format!("a bytes-like object is required, not '{}'", tn(&x))),
                        },
                        None => unreachable!(),
                    };
                    let delete = match delete {
                        None => vec![],
                        Some(x) => match bytes_of(&x) {
                            Some(d) => d,
                            None => return type_err(format!("a bytes-like object is required, not '{}'", tn(&x))),
                        },
                    };
                    let mut out = vec![];
                    for &c in b.iter() {
                        if delete.contains(&c) {
                            continue;
                        }
                        out.push(match &table {
                            Some(tb) => tb[c as usize],
                            None => c,
                        });
                    }
                    bytesv(out)
                }
                "maketrans" => {
                    no_kws("maketrans", &a)?;
                    unpack_range("maketrans", &a, 2, 2)?;
                    let frm = sub_arg(&a.p[0], false)?;
                    let to = sub_arg(&a.p[1], false)?;
                    if frm.len() != to.len() {
                        return val_err("maketrans arguments must have same length".into());
                    }
                    let mut table: Vec<u8> = (0..=255).collect();
                    for (f, t) in frm.iter().zip(to.iter()) {
                        table[*f as usize] = *t;
                    }
                    bytesv(table)
                }
                "fromhex" => bytes_fromhex(Args { p: a.p, k: a.k })?,
                _ => return Err(exc("AttributeError", format!("'bytes' object has no attribute '{n}'"))),
            })
        })())
    }
}

impl Ext for BytesV {
    fn tname(&self) -> &'static str {
        "bytes"
    }
    fn repr(&self) -> String {
        bytes_repr(&self.0)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        // the same as a str of the same ASCII text, as in CPython
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in &self.0 {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let h = (h >> 1) as i64;
        Some(if h == -1 { -2 } else { h })
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(bytes_of(o).is_some_and(|b| b == self.0))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        bytes_of(o).map(|b| self.0.as_slice().cmp(b.as_slice()))
    }
    fn truth(&self) -> bool {
        !self.0.is_empty()
    }
    fn len(&self) -> Option<usize> {
        Some(self.0.len())
    }
    fn iter(&self) -> Option<Vec<V>> {
        Some(self.0.iter().map(|&c| V::Int(c as i64)).collect())
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        self.call(me, n, a)
    }
    fn attr(&self, me: &V, n: &str) -> Option<R> {
        const M: &[&str] = &["decode", "hex", "startswith", "endswith", "find", "index", "rfind", "rindex", "count", "split", "rsplit", "strip", "lstrip", "rstrip", "join", "replace", "removeprefix", "removesuffix", "upper", "lower", "capitalize", "title", "istitle", "swapcase", "center", "ljust", "rjust", "zfill", "expandtabs", "partition", "rpartition", "translate", "isdigit", "isalpha", "isalnum", "isspace", "isupper", "islower", "isascii", "splitlines", "maketrans", "fromhex"];
        M.contains(&n).then(|| Ok(V::BMeth(Rc::new((me.clone(), sym(n))))))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        match op {
            "+" => {
                let Some(other) = bytes_of(o) else {
                    return (!right).then(|| type_err(format!("can't concat {} to bytes", tn(o))));
                };
                let mut out = if right { other.clone() } else { self.0.clone() };
                out.extend(if right { &self.0 } else { &other });
                Some(Ok(bytesv(out)))
            }
            "*" => {
                let n = match o {
                    V::Int(n) => *n,
                    V::Bool(b) => *b as i64,
                    // a huge int cannot fit the Py_ssize_t CPython's repeat takes
                    V::Big(_) => return Some(Err(exc("OverflowError", "cannot fit 'int' into an index-sized integer".into()))),
                    _ => return Some(type_err(format!("can't multiply sequence by non-int of type '{}'", tn(o)))),
                };
                if n < 0 {
                    return Some(Ok(bytesv(vec![])));
                }
                // CPython's bytes_repeat: the total size cannot fit a Py_ssize_t
                if n > 0 && (self.0.len() as i64) > i64::MAX / n {
                    return Some(Err(exc("OverflowError", "repeated bytes are too long".into())));
                }
                let Some(total) = (self.0.len() as u64).checked_mul(n as u64) else {
                    return Some(Err(exc0("MemoryError")));
                };
                let mut out = Vec::new();
                if out.try_reserve_exact(total as usize).is_err() {
                    return Some(Err(exc0("MemoryError")));
                }
                for _ in 0..n {
                    out.extend_from_slice(&self.0);
                }
                Some(Ok(bytesv(out)))
            }
            _ => None,
        }
    }
}

// ---------- hashlib ----------

struct HashV {
    name: &'static str,
    data: RefCell<Vec<u8>>,
}

fn hash_algo(n: &str) -> Option<&'static str> {
    Some(match n.to_lowercase().as_str() {
        "md5" => "md5",
        "sha1" => "sha1",
        "sha224" => "sha224",
        "sha256" => "sha256",
        "sha384" => "sha384",
        "sha512" => "sha512",
        _ => return None,
    })
}

fn md5(msg: &[u8]) -> Vec<u8> {
    const S: [u32; 64] = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let mut h: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    let mut m = msg.to_vec();
    let bits = (msg.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_le_bytes());
    for ch in m.chunks(64) {
        let w: Vec<u32> = ch.chunks(4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(w[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f2.rotate_left(S[i]));
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
    }
    h.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn pad_be(msg: &[u8], block: usize) -> Vec<u8> {
    let mut m = msg.to_vec();
    m.push(0x80);
    let lenb = if block == 64 { 8 } else { 16 };
    while m.len() % block != block - lenb {
        m.push(0);
    }
    let bits = (msg.len() as u128).wrapping_mul(8);
    if block == 64 {
        m.extend_from_slice(&(bits as u64).to_be_bytes());
    } else {
        m.extend_from_slice(&bits.to_be_bytes());
    }
    m
}

fn sha1(msg: &[u8]) -> Vec<u8> {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for ch in pad_be(msg, 64).chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([ch[4 * i], ch[4 * i + 1], ch[4 * i + 2], ch[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), 0x5A827999),
                1 => (b ^ c ^ d, 0x6ED9EBA1),
                2 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn sha256(msg: &[u8], small: bool) -> Vec<u8> {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = if small {
        [0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939, 0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4]
    } else {
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]
    };
    for ch in pad_be(msg, 64).chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([ch[4 * i], ch[4 * i + 1], ch[4 * i + 2], ch[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch_ = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch_).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let out: Vec<u8> = h.iter().flat_map(|x| x.to_be_bytes()).collect();
    if small { out[..28].to_vec() } else { out }
}

fn sha512(msg: &[u8], small: bool) -> Vec<u8> {
    const K: [u64; 80] = [
        0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc, 0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118, 0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2, 0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694, 0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
        0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5, 0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4, 0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70, 0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df, 0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
        0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30, 0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8, 0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8, 0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3, 0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
        0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b, 0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178, 0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b, 0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c, 0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
    ];
    let mut h: [u64; 8] = if small {
        [0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939, 0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4]
    } else {
        [0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1, 0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179]
    };
    for ch in pad_be(msg, 128).chunks(128) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            let mut x = [0u8; 8];
            x.copy_from_slice(&ch[8 * i..8 * i + 8]);
            w[i] = u64::from_be_bytes(x);
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..80 {
            let s1 = v[4].rotate_right(14) ^ v[4].rotate_right(18) ^ v[4].rotate_right(41);
            let ch_ = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch_).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(28) ^ v[0].rotate_right(34) ^ v[0].rotate_right(39);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let out: Vec<u8> = h.iter().flat_map(|x| x.to_be_bytes()).collect();
    if small { out[..48].to_vec() } else { out }
}

fn digest_of(name: &str, d: &[u8]) -> Vec<u8> {
    match name {
        "md5" => md5(d),
        "sha1" => sha1(d),
        "sha224" => sha256(d, true),
        "sha256" => sha256(d, false),
        "sha384" => sha512(d, true),
        _ => sha512(d, false),
    }
}

fn hash_data(v: Option<&V>) -> Result<Vec<u8>, E> {
    match v {
        None => Ok(vec![]),
        Some(V::Str(_)) => type_err("Strings must be encoded before hashing".into()),
        Some(x) => bytes_of(x).ok_or_else(|| exc("TypeError", format!("object supporting the buffer API required"))),
    }
}

impl Ext for HashV {
    fn tname(&self) -> &'static str {
        "_hashlib.HASH"
    }
    fn repr(&self) -> String {
        format!("<{} _hashlib.HASH object @ {:#x}>", self.name, self as *const HashV as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "name" => s(self.name),
            "digest_size" => V::Int(digest_of(self.name, b"").len() as i64),
            "block_size" => V::Int(if matches!(self.name, "sha384" | "sha512") { 128 } else { 64 }),
            "update" | "digest" | "hexdigest" | "copy" => V::BMeth(Rc::new((me.clone(), sym(n)))),
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some((|| -> R {
            Ok(match n {
                "update" => {
                    let d = match a.p.first() {
                        Some(x) => hash_data(Some(x))?,
                        None => return type_err("update() takes exactly one argument (0 given)".into()),
                    };
                    self.data.borrow_mut().extend(d);
                    V::None
                }
                "digest" => bytesv(digest_of(self.name, &self.data.borrow())),
                "hexdigest" => st(hex_of(&digest_of(self.name, &self.data.borrow()))),
                "copy" => V::Ext(ExtRc::new(HashV { name: self.name, data: RefCell::new(self.data.borrow().clone()) })),
                _ => return Err(exc("AttributeError", format!("'_hashlib.HASH' object has no attribute '{n}'"))),
            })
        })())
    }
}

pub fn hashlib_fn(n: &str, a: Args) -> R {
    let mut a = a;
    a.kw("usedforsecurity");
    let data = a.kw("data").or_else(|| a.kw("string"));
    let (name, data) = if n == "new" {
        let nm = match a.p.first() {
            Some(V::Str(s)) => s.s.clone(),
            Some(x) => return type_err(format!("new() argument 'name' must be str, not {}", tn(x))),
            None => return type_err("new() missing required argument 'name' (pos 1)".into()),
        };
        match hash_algo(&nm) {
            Some(h) => (h, data.or_else(|| a.p.get(1).cloned())),
            None => return val_err(format!("unsupported hash type {nm}")),
        }
    } else {
        match hash_algo(n) {
            Some(h) if h == n => (h, data.or_else(|| a.p.first().cloned())),
            _ => return Err(unsupported(&format!("hashlib.{n}"))),
        }
    };
    a.no_kw(n)?;
    let d = hash_data(data.as_ref())?;
    Ok(V::Ext(ExtRc::new(HashV { name, data: RefCell::new(d) })))
}

// ---------- base64 and binascii ----------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(d: &[u8], alt: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len().div_ceil(3) * 4);
    for ch in d.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= ch.len() {
                out.push(B64[((n >> (18 - 6 * k)) & 63) as usize]);
            } else {
                out.push(b'=');
            }
        }
    }
    if let Some(alt) = alt {
        for c in out.iter_mut() {
            if *c == b'+' {
                *c = alt[0];
            } else if *c == b'/' {
                *c = alt[1];
            }
        }
    }
    out
}

fn b64_err(m: String) -> E {
    E(match exc("ValueError", m) {
        E(V::Obj(o)) => {
            let e = Obj::new(exc_class("binascii.Error"));
            e.set("args", o.get("args").unwrap_or(tup(vec![])));
            V::Obj(e)
        }
        E(v) => v,
    })
}

// binascii.a2b_base64, as CPython 3.13 does it. Not strict: characters outside the alphabet and '=' between
// data are skipped, and the data must end in a full quad or enough '='. Strict (validate=True): only the
// alphabet, and '=' only to pad the last quad.
fn b64_decode(d: &[u8], strict: bool) -> Result<Vec<u8>, E> {
    let val = |c: u8| B64.iter().position(|&x| x == c).map(|p| p as u32);
    let mut out = vec![];
    let mut acc = 0u32;
    let mut q = 0usize;
    let mut chars = 0usize;
    let mut pads = 0usize;
    let mut done = false;
    for &c in d {
        if c == b'=' {
            if strict {
                if done || q == 0 {
                    return Err(b64_err(if chars == 0 { "Leading padding not allowed".into() } else { "Excess padding not allowed".into() }));
                }
                if q == 1 {
                    return Err(b64_err(format!("Invalid base64-encoded string: number of data characters ({chars}) cannot be 1 more than a multiple of 4")));
                }
                if q >= 2 {
                    pads += 1;
                    if q + pads >= 4 {
                        done = true;
                    }
                }
            } else {
                pads += 1;
            }
            continue;
        }
        let Some(v) = val(c) else {
            if strict {
                return Err(b64_err("Only base64 data is allowed".into()));
            }
            continue;
        };
        if strict {
            if done {
                return Err(b64_err("Excess data after padding".into()));
            }
            if pads > 0 {
                return Err(b64_err("Discontinuous padding not allowed".into()));
            }
        }
        pads = 0;
        chars += 1;
        acc = acc << 6 | v;
        q += 1;
        if q == 4 {
            out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
            acc = 0;
            q = 0;
        }
    }
    match q {
        0 => Ok(out),
        1 => Err(b64_err(format!("Invalid base64-encoded string: number of data characters ({chars}) cannot be 1 more than a multiple of 4"))),
        _ if pads + q < 4 => Err(b64_err("Incorrect padding".into())),
        2 => {
            out.push((acc >> 4) as u8);
            Ok(out)
        }
        _ => {
            out.extend_from_slice(&[(acc >> 10) as u8, (acc >> 2) as u8]);
            Ok(out)
        }
    }
}

// A str or bytes argument of a base64 decoder.
fn b64_arg(v: Option<&V>) -> Result<Vec<u8>, E> {
    match v {
        Some(V::Str(s)) => {
            if !s.s.is_ascii() {
                return val_err("string argument should contain only ASCII characters".into());
            }
            Ok(s.s.as_bytes().to_vec())
        }
        Some(x) => bytes_of(x).ok_or_else(|| exc("TypeError", format!("argument should be a bytes-like object or ASCII string, not '{}'", tn(x)))),
        None => type_err("missing required argument 's' (pos 1)".into()),
    }
}

fn alt_arg(v: Option<V>) -> Result<Option<Vec<u8>>, E> {
    match v {
        None | Some(V::None) => Ok(None),
        Some(x) => {
            let b = b64_arg(Some(&x))?;
            if b.len() != 2 {
                return Err(exc("AssertionError", format!("{}", bytes_repr(&b))));
            }
            Ok(Some(b))
        }
    }
}

fn swap_alt(d: &[u8], alt: &[u8]) -> Vec<u8> {
    d.iter().map(|&c| if c == alt[0] { b'+' } else if c == alt[1] { b'/' } else { c }).collect()
}

pub fn base64_fn(n: &str, a: Args) -> R {
    let mut a = a;
    let first = a.p.first().cloned();
    Ok(match n {
        "b64encode" | "standard_b64encode" | "urlsafe_b64encode" | "encodebytes" => {
            let alt = if n == "b64encode" { alt_arg(a.kw("altchars").or_else(|| a.p.get(1).cloned()))? } else { None };
            let d = want_bytes(first.as_ref().unwrap_or(&V::None), "a bytes-like object is required")?;
            match n {
                "urlsafe_b64encode" => bytesv(b64_encode(&d, Some(b"-_"))),
                "encodebytes" => {
                    let enc = b64_encode(&d, None);
                    let mut out = vec![];
                    for line in enc.chunks(76) {
                        out.extend_from_slice(line);
                        out.push(b'\n');
                    }
                    bytesv(out)
                }
                _ => bytesv(b64_encode(&d, alt.as_deref())),
            }
        }
        "b64decode" | "standard_b64decode" | "urlsafe_b64decode" | "decodebytes" => {
            let alt = if n == "b64decode" { alt_arg(a.kw("altchars").or_else(|| a.p.get(1).cloned()))? } else { None };
            let validate = if n == "b64decode" { a.kw("validate").or_else(|| a.p.get(2).cloned()).map(|v| truth(&v)).transpose()?.unwrap_or(false) } else { false };
            let d = if n == "decodebytes" { want_bytes(first.as_ref().unwrap_or(&V::None), "expected bytes-like object")? } else { b64_arg(first.as_ref())? };
            let d = match (n, &alt) {
                ("urlsafe_b64decode", _) => swap_alt(&d, b"-_"),
                (_, Some(alt)) => swap_alt(&d, alt),
                _ => d,
            };
            bytesv(b64_decode(&d, validate)?)
        }
        "b16encode" => bytesv(hex_of(&want_bytes(first.as_ref().unwrap_or(&V::None), "a bytes-like object is required")?).to_uppercase().into_bytes()),
        "b16decode" => {
            let casefold = a.kw("casefold").or_else(|| a.p.get(1).cloned()).map(|v| truth(&v)).transpose()?.unwrap_or(false);
            let mut d = b64_arg(first.as_ref())?;
            if casefold {
                d = d.to_ascii_uppercase();
            }
            if d.len() % 2 == 1 {
                return Err(b64_err("Odd-length string".into()));
            }
            if d.iter().any(|c| !matches!(c, b'0'..=b'9' | b'A'..=b'F')) {
                return Err(b64_err("Non-base16 digit found".into()));
            }
            bytesv(d.chunks(2).map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap()).collect())
        }
        _ => return Err(unsupported(&format!("base64.{n}"))),
    })
}

pub fn binascii_fn(n: &str, a: Args) -> R {
    let first = a.p.first().cloned();
    Ok(match n {
        "hexlify" | "b2a_hex" => bytesv(hex_of(&want_bytes(first.as_ref().unwrap_or(&V::None), "a bytes-like object is required")?).into_bytes()),
        "unhexlify" | "a2b_hex" => {
            let d = b64_arg(first.as_ref())?;
            if d.len() % 2 == 1 {
                return Err(b64_err("Odd-length string".into()));
            }
            let mut out = vec![];
            for p in d.chunks(2) {
                match std::str::from_utf8(p).ok().and_then(|t| u8::from_str_radix(t, 16).ok()) {
                    Some(v) => out.push(v),
                    None => return Err(b64_err("Non-hexadecimal digit found".into())),
                }
            }
            bytesv(out)
        }
        _ => return Err(unsupported(&format!("binascii.{n}"))),
    })
}
