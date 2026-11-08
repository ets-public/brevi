// ---------- _brevi_st: the safetensors format, for Brevi's safetensors (src/dynpy/py/safetensors.py) ----------
// A port of the safetensors 0.8 crate: the header's JSON read as serde_json reads it (its error messages and
// positions, `__metadata__` checked where it stands and the tensor entries once the whole header is read, as
// serde's flatten does), the checks of Metadata::validate, slicing (slice_byte_ranges) and serialization (tensors
// sorted by dtype then name, the header padded with spaces to 8 bytes). Tensors become arrays of Brevi's numpy:
// F64, F32, I64, I32 and BOOL; any other dtype raises NotImplementedError when it is turned into an array. This
// module reads and writes no file: the copy hands it bytes from _brevi_fs.

const ST_DTYPES: [&str; 22] = [
    "BOOL", "F4", "F6_E2M3", "F6_E3M2", "U8", "I8", "F8_E5M2", "F8_E4M3", "F8_E8M0", "F8_E4M3FNUZ", "F8_E5M2FNUZ", "I16", "U16", "F16", "BF16", "I32",
    "U32", "F32", "C64", "F64", "I64", "U64",
];
const ST_MAX_HEADER: usize = 100_000_000;

fn st_bits(d: usize) -> usize {
    match ST_DTYPES[d] {
        "F4" => 4,
        "F6_E2M3" | "F6_E3M2" => 6,
        "BOOL" | "U8" | "I8" | "F8_E5M2" | "F8_E4M3" | "F8_E8M0" | "F8_E4M3FNUZ" | "F8_E5M2FNUZ" => 8,
        "I16" | "U16" | "F16" | "BF16" => 16,
        "I32" | "U32" | "F32" => 32,
        _ => 64,
    }
}

// serde's Content: a JSON value held until the whole header is read
enum StJ {
    Null,
    Bool(bool),
    U(u64),
    I(i64),
    F(f64),
    Str(String),
    Seq(Vec<StJ>),
    Map(Vec<(String, StJ)>),
}

// serde's Unexpected, as its Display words it
fn st_unexp(v: &StJ) -> String {
    match v {
        StJ::Null => "null".into(),
        StJ::Bool(b) => format!("boolean `{b}`"),
        StJ::U(n) => format!("integer `{n}`"),
        StJ::I(n) => format!("integer `{n}`"),
        StJ::F(f) => format!("floating point `{}`", st_fdisp(*f)),
        StJ::Str(s) => format!("string {s:?}"),
        StJ::Seq(_) => "sequence".into(),
        StJ::Map(_) => "map".into(),
    }
}
// serde's WithDecimalPoint
fn st_fdisp(f: f64) -> String {
    let s = format!("{f}");
    if f.is_finite() && !s.contains('.') && !s.contains('e') {
        format!("{s}.0")
    } else {
        s
    }
}

struct StP<'a> {
    b: &'a [u8],
    i: usize,
    depth: i32,
}

impl<'a> StP<'a> {
    // serde_json's position of an index: the line, and the bytes read on it
    fn at(&self, i: usize) -> String {
        let i = i.min(self.b.len());
        let mut line = 1;
        let mut col = 0;
        for &c in &self.b[..i] {
            if c == b'\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        format!("at line {line} column {col}")
    }
    fn err(&self, m: &str) -> String {
        format!("{m} {}", self.at(self.i))
    }
    fn peek_err(&self, m: &str) -> String {
        format!("{m} {}", self.at((self.i + 1).min(self.b.len())))
    }
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\t' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn ident(&mut self, rest: &[u8]) -> Result<(), String> {
        for &c in rest {
            match self.peek() {
                None => return Err(self.err("EOF while parsing a value")),
                Some(x) => {
                    self.i += 1;
                    if x != c {
                        return Err(self.err("expected ident"));
                    }
                }
            }
        }
        Ok(())
    }
    fn hex4(&mut self) -> Result<u16, String> {
        let mut n: u16 = 0;
        for _ in 0..4 {
            let Some(c) = self.peek() else { return Err(self.err("EOF while parsing a string")) };
            self.i += 1;
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return Err(self.err("invalid escape")),
            };
            n = n * 16 + d as u16;
        }
        Ok(n)
    }
    // a string, its opening quote read
    fn string(&mut self) -> Result<String, String> {
        let mut out: Vec<u8> = vec![];
        loop {
            let Some(c) = self.peek() else { return Err(self.err("EOF while parsing a string")) };
            self.i += 1;
            match c {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let Some(e) = self.peek() else { return Err(self.err("EOF while parsing a string")) };
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let n = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&n) {
                                if self.b.get(self.i) == Some(&b'\\') && self.b.get(self.i + 1) == Some(&b'u') {
                                    self.i += 2;
                                    let m = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&m) {
                                        return Err(self.err("lone leading surrogate in hex escape"));
                                    }
                                    char::from_u32(0x10000 + (((n as u32) - 0xD800) << 10) + ((m as u32) - 0xDC00)).unwrap_or('\u{FFFD}')
                                } else {
                                    return Err(self.err("lone leading surrogate in hex escape"));
                                }
                            } else if (0xDC00..0xE000).contains(&n) {
                                return Err(self.err("lone leading surrogate in hex escape"));
                            } else {
                                char::from_u32(n as u32).unwrap_or('\u{FFFD}')
                            };
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                c if c < 0x20 => return Err(self.err("control character (\\u0000-\\u001F) found while parsing a string")),
                c => out.push(c),
            }
        }
    }
    fn digits(&mut self) -> usize {
        let s = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        self.i - s
    }
    // a number, its first character not yet read
    fn number(&mut self) -> Result<StJ, String> {
        let start = self.i;
        let neg = self.peek() == Some(b'-');
        if neg {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.i += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.peek_err("invalid number"));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            None => return Err(self.err("EOF while parsing a value")),
            _ => {
                self.i += 1;
                return Err(self.err("invalid number"));
            }
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            self.i += 1;
            float = true;
            if self.digits() == 0 {
                return Err(if self.peek().is_none() { self.err("EOF while parsing a value") } else { self.peek_err("invalid number") });
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            float = true;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.digits() == 0 {
                return Err(if self.peek().is_none() { self.err("EOF while parsing a value") } else { self.peek_err("invalid number") });
            }
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).unwrap_or("0");
        if !float {
            if neg {
                if let Ok(n) = text.parse::<i64>() {
                    return Ok(if n == 0 { StJ::F(-0.0) } else { StJ::I(n) });
                }
            } else if let Ok(n) = text.parse::<u64>() {
                return Ok(StJ::U(n));
            }
        }
        let f: f64 = text.parse().unwrap_or(0.0);
        if f.is_infinite() {
            return Err(self.err("number out of range"));
        }
        Ok(StJ::F(f))
    }
    fn value(&mut self) -> Result<StJ, String> {
        self.ws();
        let Some(c) = self.peek() else { return Err(self.err("EOF while parsing a value")) };
        match c {
            b'n' => {
                self.i += 1;
                self.ident(b"ull")?;
                Ok(StJ::Null)
            }
            b't' => {
                self.i += 1;
                self.ident(b"rue")?;
                Ok(StJ::Bool(true))
            }
            b'f' => {
                self.i += 1;
                self.ident(b"alse")?;
                Ok(StJ::Bool(false))
            }
            b'"' => {
                self.i += 1;
                Ok(StJ::Str(self.string()?))
            }
            b'-' | b'0'..=b'9' => self.number(),
            b'[' => {
                self.depth -= 1;
                if self.depth == 0 {
                    return Err(self.err("recursion limit exceeded"));
                }
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    self.depth += 1;
                    return Ok(StJ::Seq(v));
                }
                loop {
                    v.push(self.value()?);
                    self.ws();
                    match self.peek() {
                        Some(b',') => {
                            self.i += 1;
                            self.ws();
                            if self.peek() == Some(b']') {
                                return Err(self.peek_err("trailing comma"));
                            }
                        }
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        None => return Err(self.err("EOF while parsing a list")),
                        _ => return Err(self.peek_err("expected `,` or `]`")),
                    }
                }
                self.depth += 1;
                Ok(StJ::Seq(v))
            }
            b'{' => {
                self.depth -= 1;
                if self.depth == 0 {
                    return Err(self.err("recursion limit exceeded"));
                }
                self.i += 1;
                let mut v = vec![];
                let mut first = true;
                loop {
                    self.ws();
                    match self.peek() {
                        Some(b'}') if first => {
                            self.i += 1;
                            break;
                        }
                        Some(b'}') => return Err(self.peek_err("trailing comma")),
                        Some(b'"') => {}
                        None => return Err(self.err("EOF while parsing an object")),
                        _ => return Err(self.peek_err("key must be a string")),
                    }
                    first = false;
                    self.i += 1;
                    let k = self.string()?;
                    self.colon()?;
                    let x = self.value()?;
                    v.push((k, x));
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            break;
                        }
                        None => return Err(self.err("EOF while parsing an object")),
                        _ => return Err(self.peek_err("expected `,` or `}`")),
                    }
                }
                self.depth += 1;
                Ok(StJ::Map(v))
            }
            _ => Err(self.peek_err("expected value")),
        }
    }
    fn colon(&mut self) -> Result<(), String> {
        self.ws();
        match self.peek() {
            Some(b':') => {
                self.i += 1;
                Ok(())
            }
            None => Err(self.err("EOF while parsing an object")),
            _ => Err(self.peek_err("expected `:`")),
        }
    }
    // A value of a type it should not be: serde_json reads a scalar to name it, but not a sequence or map.
    fn wrong(&mut self, expected: &str) -> Result<String, String> {
        self.ws();
        let at = self.i;
        let unexp = match self.peek() {
            Some(b'[') => "sequence".to_string(),
            Some(b'{') => "map".to_string(),
            _ => {
                let v = self.value()?;
                st_unexp(&v)
            }
        };
        if matches!(self.b.get(at), Some(b'[' | b'{')) {
            self.i = at;
        }
        Ok(self.err(&format!("invalid type: {unexp}, expected {expected}")))
    }
    // __metadata__: Option<HashMap<String, String>>
    fn metadata(&mut self) -> Result<Option<Vec<(String, String)>>, String> {
        self.ws();
        match self.peek() {
            Some(b'n') => {
                self.i += 1;
                self.ident(b"ull")?;
                return Ok(None);
            }
            Some(b'{') => {}
            None => return Err(self.err("EOF while parsing a value")),
            _ => return Err(self.wrong("a map")?),
        }
        self.depth -= 1;
        self.i += 1;
        let mut out: Vec<(String, String)> = vec![];
        let mut first = true;
        loop {
            self.ws();
            match self.peek() {
                Some(b'}') if first => {
                    self.i += 1;
                    break;
                }
                Some(b'}') => return Err(self.peek_err("trailing comma")),
                Some(b'"') => {}
                None => return Err(self.err("EOF while parsing an object")),
                _ => return Err(self.peek_err("key must be a string")),
            }
            first = false;
            self.i += 1;
            let k = self.string()?;
            self.colon()?;
            self.ws();
            let val = match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    self.string()?
                }
                None => return Err(self.err("EOF while parsing a value")),
                _ => return Err(self.wrong("a string")?),
            };
            if let Some(p) = out.iter_mut().find(|(x, _)| *x == k) {
                p.1 = val;
            } else {
                out.push((k, val));
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                None => return Err(self.err("EOF while parsing an object")),
                _ => return Err(self.peek_err("expected `,` or `}`")),
            }
        }
        self.depth += 1;
        Ok(Some(out))
    }
}

// usize from Content
fn st_usize(v: &StJ) -> Result<usize, String> {
    match v {
        StJ::U(n) => Ok(*n as usize),
        StJ::I(n) if *n < 0 => Err(format!("invalid value: integer `{n}`, expected usize")),
        StJ::I(n) => Ok(*n as usize),
        x => Err(format!("invalid type: {}, expected usize", st_unexp(x))),
    }
}

struct StInfo {
    dtype: usize,
    shape: Vec<usize>,
    offs: (usize, usize),
}

// TensorInfo from Content, as serde's derive reads it (a map, or a sequence of its three fields)
fn st_info(v: &StJ) -> Result<StInfo, String> {
    let dtype = |x: &StJ| -> Result<usize, String> {
        match x {
            StJ::Str(s) => ST_DTYPES.iter().position(|d| d == s).ok_or_else(|| {
                format!("unknown variant `{s}`, expected one of {}", ST_DTYPES.iter().map(|d| format!("`{d}`")).collect::<Vec<_>>().join(", "))
            }),
            x => Err(format!("invalid type: {}, expected string or map", st_unexp(x))),
        }
    };
    let shape = |x: &StJ| -> Result<Vec<usize>, String> {
        match x {
            StJ::Seq(v) => v.iter().map(st_usize).collect(),
            x => Err(format!("invalid type: {}, expected a sequence", st_unexp(x))),
        }
    };
    let offs = |x: &StJ| -> Result<(usize, usize), String> {
        match x {
            StJ::Seq(v) => {
                let a = match v.first() {
                    Some(a) => st_usize(a)?,
                    None => return Err(format!("invalid length {}, expected a tuple of size 2", v.len())),
                };
                let b = match v.get(1) {
                    Some(b) => st_usize(b)?,
                    None => return Err(format!("invalid length {}, expected a tuple of size 2", v.len())),
                };
                if v.len() > 2 {
                    return Err(format!("invalid length {}, expected 2 elements in sequence", v.len()));
                }
                Ok((a, b))
            }
            x => Err(format!("invalid type: {}, expected a tuple of size 2", st_unexp(x))),
        }
    };
    match v {
        StJ::Map(m) => {
            let (mut d, mut s, mut o) = (None, None, None);
            for (k, x) in m {
                match k.as_str() {
                    "dtype" => {
                        if d.is_some() {
                            return Err("duplicate field `dtype`".into());
                        }
                        d = Some(dtype(x)?);
                    }
                    "shape" => {
                        if s.is_some() {
                            return Err("duplicate field `shape`".into());
                        }
                        s = Some(shape(x)?);
                    }
                    "data_offsets" => {
                        if o.is_some() {
                            return Err("duplicate field `data_offsets`".into());
                        }
                        o = Some(offs(x)?);
                    }
                    _ => {}
                }
            }
            let d = d.ok_or("missing field `dtype`")?;
            let s = s.ok_or("missing field `shape`")?;
            let o = o.ok_or("missing field `data_offsets`")?;
            Ok(StInfo { dtype: d, shape: s, offs: o })
        }
        StJ::Seq(v) => {
            let n = v.len();
            let short = || format!("invalid length {n}, expected struct TensorInfo with 3 elements");
            let d = dtype(v.first().ok_or_else(short)?)?;
            let s = shape(v.get(1).ok_or_else(short)?)?;
            let o = offs(v.get(2).ok_or_else(short)?)?;
            if n > 3 {
                return Err(format!("invalid length {n}, expected 3 elements in sequence"));
            }
            Ok(StInfo { dtype: d, shape: s, offs: o })
        }
        x => Err(format!("invalid type: {}, expected struct TensorInfo", st_unexp(x))),
    }
}

struct StHeader {
    n: usize,
    meta: Option<Vec<(String, String)>>,
    // by offset
    tensors: Vec<(String, StInfo)>,
}

// SafeTensors::read_metadata over the first bytes of a buffer of `total` bytes (the error is the crate's Display)
fn st_read_header(b: &[u8], total: usize) -> Result<StHeader, String> {
    if b.len() < 8 {
        return Err("header too small".into());
    }
    let n = u64::from_le_bytes(b[..8].try_into().unwrap());
    if n > ST_MAX_HEADER as u64 {
        return Err("header too large".into());
    }
    let n = n as usize;
    let Some(hb) = b.get(8..8 + n) else { return Err("invalid header length".into()) };
    let text = std::str::from_utf8(hb).map_err(|e| format!("invalid UTF-8 in header: {e}"))?;
    let mut p = StP { b: text.as_bytes(), i: 0, depth: 128 };
    let json = |e: String| format!("invalid JSON in header: {e}");
    p.ws();
    match p.peek() {
        Some(b'{') => {}
        None => return Err(json(p.err("EOF while parsing a value"))),
        _ => return Err(json(p.wrong("struct HashMetadata")?)),
    }
    p.i += 1;
    p.depth -= 1;
    let mut meta: Option<Option<Vec<(String, String)>>> = None;
    let mut raw: Vec<(String, StJ)> = vec![];
    let mut first = true;
    loop {
        p.ws();
        match p.peek() {
            Some(b'}') if first => {
                p.i += 1;
                break;
            }
            Some(b'}') => return Err(json(p.peek_err("trailing comma"))),
            Some(b'"') => {}
            None => return Err(json(p.err("EOF while parsing an object"))),
            _ => return Err(json(p.peek_err("key must be a string"))),
        }
        first = false;
        p.i += 1;
        let k = p.string().map_err(json)?;
        if k == "__metadata__" {
            if meta.is_some() {
                return Err(json(p.err("duplicate field `__metadata__`")));
            }
            p.colon().map_err(json)?;
            meta = Some(p.metadata().map_err(json)?);
        } else {
            p.colon().map_err(json)?;
            let v = p.value().map_err(json)?;
            raw.push((k, v));
        }
        p.ws();
        match p.peek() {
            Some(b',') => p.i += 1,
            Some(b'}') => {
                p.i += 1;
                break;
            }
            None => return Err(json(p.err("EOF while parsing an object"))),
            _ => return Err(json(p.peek_err("expected `,` or `}`"))),
        }
    }
    // the flattened tensor entries, read once the map is: their errors are at its end
    let mut tensors: Vec<(String, StInfo)> = vec![];
    for (k, v) in &raw {
        let info = st_info(v).map_err(|e| json(p.err(&e)))?;
        if let Some(t) = tensors.iter_mut().find(|(x, _)| x == k) {
            t.1 = info;
        } else {
            tensors.push((k.clone(), info));
        }
    }
    p.ws();
    if p.peek().is_some() {
        return Err(json(p.peek_err("trailing characters")));
    }
    tensors.sort_by(|a, b| a.1.offs.cmp(&b.1.offs).then(a.0.cmp(&b.0)));
    let end = st_validate(&tensors)?;
    if end + 8 + n != total {
        return Err("incomplete metadata, file not fully covered".into());
    }
    Ok(StHeader { n, meta: meta.flatten(), tensors })
}

// Metadata::validate: the end of the data
fn st_validate(t: &[(String, StInfo)]) -> Result<usize, String> {
    let mut start = 0usize;
    for (name, info) in t {
        let (s, e) = info.offs;
        if s != start || e < s {
            return Err(format!("invalid offset for tensor `{name}`"));
        }
        start = e;
        let n = info.shape.iter().try_fold(1usize, |a, &b| a.checked_mul(b)).ok_or("overflow computing buffer size from shape and/or element type")?;
        let bits = n.checked_mul(st_bits(info.dtype)).ok_or("overflow computing buffer size from shape and/or element type")?;
        if bits % 8 != 0 {
            return Err("The slice is slicing for subbytes dtypes, and the slice does not end up at a byte boundary, this is invalid.".into());
        }
        if e - s != bits / 8 {
            return Err("invalid shape, data type, or offset for tensor".into());
        }
    }
    Ok(start)
}

// An array of Brevi's numpy from a tensor's little-endian bytes. float16, the small and unsigned integers and
// complex64 are storage dtypes (rt_np.rs SD). Types numpy lacks fail as the package fails on them: load() looks
// the dtype up in its table (KeyError), safe_open asks numpy for the type (opened).
fn st_array(data: &[u8], d: usize, shape: Vec<usize>, opened: bool) -> R {
    let b = match ST_DTYPES[d] {
        "F64" => Buf::F(data.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()),
        "F32" => Buf::F32(data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()),
        "I64" => Buf::I(data.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect()),
        "I32" => Buf::I32(data.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()),
        "BOOL" => Buf::B(data.iter().map(|&c| c != 0).collect()),
        other => match st_sd(other) {
            Some(sd) => Buf::S(data.to_vec(), sd),
            None if !opened => return Err(exc_v("KeyError", s(other))),
            None => {
                return Err(match other {
                    "BF16" => exc("TypeError", "data type 'bfloat16' not understood".into()),
                    "F6_E2M3" | "F6_E3M2" => exc("ValueError", format!("Dtype not understood: {other}")),
                    f => exc("AttributeError", format!("module 'numpy' has no attribute '{}'", st_npname(f))),
                })
            }
        },
    };
    Ok(np_new(b, shape))
}

fn st_sd(d: &str) -> Option<SD> {
    Some(match d {
        "F16" => SD::F16,
        "U8" => SD::U8,
        "I8" => SD::I8,
        "I16" => SD::I16,
        "U16" => SD::U16,
        "U32" => SD::U32,
        "U64" => SD::U64,
        "C64" => SD::C64,
        _ => return None,
    })
}

// the type the package asks numpy for
fn st_npname(d: &str) -> &'static str {
    match d {
        "F8_E5M2" => "float8_e5m2",
        "F8_E4M3" => "float8_e4m3fn",
        "F8_E8M0" => "float8_e8m0fnu",
        "F8_E4M3FNUZ" => "float8_e4m3fnuz",
        "F8_E5M2FNUZ" => "float8_e5m2fnuz",
        "F4" => "float4_e2m1fn_x2",
        _ => "a type numpy lacks",
    }
}

// The bytes numpy's ctypes.data .. + nbytes hold for an array: its values when C-contiguous, else the
// buffer from its first element on (what safetensors' numpy save reads), when that is inside the buffer.
fn st_raw(a: &NdArr) -> Result<(usize, Vec<u8>), E> {
    let n = a.size();
    let vals = if a.contig() {
        a.vals()
    } else if a.st.iter().all(|&s| s >= 0) && a.off + n <= a.buf.borrow().len() {
        a.buf.borrow().slice(a.off, n)
    } else {
        return Err(np_unsup("saving an array whose memory starts past its first element (negative strides)"));
    };
    let (d, bytes) = match vals {
        Buf::F(v) => (19, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Buf::F32(v) => (17, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Buf::I(v) => (20, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Buf::I32(v) => (15, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        Buf::B(v) => (0, v.iter().map(|&x| x as u8).collect()),
        Buf::S(v, sd) => (ST_DTYPES.iter().position(|n| st_sd(n) == Some(sd)).unwrap(), v),
        Buf::U(..) => return Err(exc("ValueError", "Unknown dtype str32. Supported dtypes: bool, int8, uint8, int16, uint16, int32, uint32, int64, uint64, float16, float32, float64, bfloat16, float8_e4m3fn, float8_e4m3fnuz, float8_e5m2, float8_e5m2fnuz, float8_e8m0fnu, float4_e2m1fn_x2, complex64".into())),
    };
    Ok((d, bytes))
}

// serde_json's string
fn st_jstr(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

// serialize: the file's bytes for (name, array) pairs and the metadata
fn st_serialize(items: Vec<(String, V)>, meta: Option<Vec<(String, String)>>) -> Result<Vec<u8>, E> {
    let mut data: Vec<(String, usize, Vec<usize>, Vec<u8>)> = vec![];
    for (k, v) in items {
        let Some(a) = as_np(&v) else { return Err(exc("AttributeError", format!("'{}' object has no attribute 'ctypes'", tn(&v)))) };
        let (d, bytes) = st_raw(a)?;
        data.push((k, d, a.shape.clone(), bytes));
    }
    data.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut h = String::from("{");
    let mut first = true;
    if let Some(mut m) = meta {
        m.sort();
        h.push_str("\"__metadata__\":{");
        for (i, (k, v)) in m.iter().enumerate() {
            if i > 0 {
                h.push(',');
            }
            st_jstr(k, &mut h);
            h.push(':');
            st_jstr(v, &mut h);
        }
        h.push('}');
        first = false;
    }
    let mut off = 0usize;
    for (k, d, shape, bytes) in &data {
        if !first {
            h.push(',');
        }
        first = false;
        st_jstr(k, &mut h);
        let dims: Vec<String> = shape.iter().map(|x| x.to_string()).collect();
        h.push_str(&format!(":{{\"dtype\":\"{}\",\"shape\":[{}],\"data_offsets\":[{},{}]}}", ST_DTYPES[*d], dims.join(","), off, off + bytes.len()));
        off += bytes.len();
    }
    h.push('}');
    while h.len() % 8 != 0 {
        h.push(' ');
    }
    let mut out = Vec::with_capacity(8 + h.len() + off);
    out.extend_from_slice(&(h.len() as u64).to_le_bytes());
    out.extend_from_slice(h.as_bytes());
    for (_, _, _, b) in data {
        out.extend_from_slice(&b);
    }
    Ok(out)
}

// slice_byte_ranges: the byte ranges (in order) and the new shape. Each indexer is (0, index) for a selection
// or (1, start, stop, step) with -1 for an open end.
fn st_slice_ranges(d: usize, shape: &[usize], ix: &[(bool, i64, i64, usize)]) -> Result<(Vec<(usize, usize)>, Vec<usize>), String> {
    if ix.len() > shape.len() {
        return Err("more slicing indexes than dimensions in tensor".into());
    }
    let mis = "The slice is slicing for subbytes dtypes, and the slice does not end up at a byte boundary, this is invalid.".to_string();
    let mut newshape = vec![];
    let mut span = st_bits(d);
    let mut idx: Vec<(usize, usize)> = vec![];
    for (i, &dim) in shape.iter().enumerate().rev() {
        if i >= ix.len() {
            newshape.push(dim);
        } else {
            let (sel, a, b, step) = ix[i];
            let (start, stop, step) = if sel {
                (a as usize, a as usize + 1, 1)
            } else {
                (if a < 0 { 0 } else { a as usize }, if b < 0 { dim } else { b as usize }, step)
            };
            if start >= dim || stop > dim {
                let asked = if start >= dim { start } else { stop.saturating_sub(1) };
                return Err(format!("index {asked} out of bounds for tensor dimension #{i} of size {dim}"));
            }
            if !sel {
                newshape.push(stop.saturating_sub(start).div_ceil(step));
            }
            if idx.is_empty() {
                if step == 1 && start == 0 && stop == dim {
                } else if step == 1 {
                    if start * span % 8 != 0 || stop * span % 8 != 0 {
                        return Err(mis);
                    }
                    let off = start * span / 8;
                    idx.push((off, stop * span / 8));
                } else {
                    for n in (start..stop).step_by(step) {
                        if n * span % 8 != 0 || (n + 1) * span % 8 != 0 {
                            return Err(mis);
                        }
                        idx.push((n * span / 8, (n + 1) * span / 8));
                    }
                }
            } else {
                let mut ni = vec![];
                for n in (start..stop).step_by(step) {
                    if n * span % 8 != 0 {
                        return Err(mis);
                    }
                    let off = n * span / 8;
                    for (s, e) in &idx {
                        ni.push((s + off, e + off));
                    }
                }
                idx = ni;
            }
        }
        span *= dim;
    }
    if idx.is_empty() {
        if span % 8 != 0 {
            return Err(mis);
        }
        idx.push((0, span / 8));
    }
    newshape.reverse();
    Ok((idx, newshape))
}

fn st_int(v: &V) -> i64 {
    match v {
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        _ => 0,
    }
}

fn st_str(v: &V) -> String {
    match v {
        V::Str(x) => x.s.clone(),
        _ => String::new(),
    }
}

fn st_err(m: String) -> E {
    // the copy raises SafetensorError with it
    exc("ValueError", m)
}

fn st_header_v(h: StHeader) -> V {
    let meta = match h.meta {
        None => V::None,
        Some(m) => {
            let mut d = Dict::new();
            for (k, v) in m {
                let _ = d.set(st(k), st(v));
            }
            V::Dict(Rc::new(RefCell::new(d)))
        }
    };
    let ts = h
        .tensors
        .into_iter()
        .map(|(k, i)| tup(vec![st(k), V::Int(i.dtype as i64), list(i.shape.into_iter().map(|x| V::Int(x as i64)).collect()), V::Int(i.offs.0 as i64), V::Int(i.offs.1 as i64)]))
        .collect();
    tup(vec![V::Int(h.n as i64), meta, list(ts)])
}

pub fn brevi_st_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    let bytes = |i: usize| bytes_of(&arg(i)).unwrap_or_default();
    match n {
        // header(first bytes, total length) -> (n, metadata, [(name, dtype, shape, start, end)])
        "header" => {
            let b = bytes(0);
            let total = st_int(&arg(1));
            let total = if total < 0 { b.len() } else { total as usize };
            Ok(st_header_v(st_read_header(&b, total).map_err(st_err)?))
        }
        // the header's length as its first 8 bytes say (the header-too-large check done), for reading it from a file
        "header_len" => {
            let b = bytes(0);
            if b.len() < 8 {
                return Err(st_err("header too small".into()));
            }
            let n = u64::from_le_bytes(b[..8].try_into().unwrap());
            if n > ST_MAX_HEADER as u64 {
                return Err(st_err("header too large".into()));
            }
            Ok(V::Int(n as i64))
        }
        // tensor(bytes, dtype, shape, opened) -> array; opened: read through safe_open (not load())
        "tensor" => {
            let shape = to_vec(&arg(2))?.iter().map(|x| st_int(x) as usize).collect();
            st_array(&bytes(0), st_int(&arg(1)) as usize, shape, truth(&arg(3))?)
        }
        "dtype_name" => Ok(s(ST_DTYPES[st_int(&arg(0)) as usize])),
        // slice(bytes, dtype, shape, indexers, shown) -> array; shown is how the indexers print in errors
        "slice" => {
            let d = st_int(&arg(1)) as usize;
            let shape: Vec<usize> = to_vec(&arg(2))?.iter().map(|x| st_int(x) as usize).collect();
            let mut ix = vec![];
            for t in to_vec(&arg(3))? {
                let t = to_vec(&t)?;
                ix.push((truth(&t[0])?, st_int(&t[1]), st_int(&t[2]), st_int(&t[3]).max(1) as usize));
            }
            let (ranges, newshape) = st_slice_ranges(d, &shape, &ix).map_err(|e| {
                let dims: Vec<String> = shape.iter().map(|x| x.to_string()).collect();
                st_err(format!("Error during slicing {} with shape [{}]: {e}", st_str(&arg(4)), dims.join(", ")))
            })?;
            let data = bytes(0);
            let mut out = Vec::with_capacity(ranges.iter().map(|(s, e)| e - s).sum());
            for (s, e) in ranges {
                out.extend_from_slice(&data[s..e]);
            }
            st_array(&out, d, newshape, true)
        }
        // serialize([(name, array)], metadata pairs or None) -> bytes
        "serialize" => {
            let mut items = vec![];
            for p in to_vec(&arg(0))? {
                let p = to_vec(&p)?;
                items.push((st_str(&p[0]), p[1].clone()));
            }
            let meta = match arg(1) {
                V::None => None,
                m => Some(to_vec(&m)?.iter().map(|p| to_vec(p).map(|kv| (st_str(&kv[0]), st_str(&kv[1])))).collect::<Result<Vec<_>, E>>()?),
            };
            Ok(bytesv(st_serialize(items, meta)?))
        }
        _ => Err(unsupported(&format!("_brevi_st.{n}"))),
    }
}
