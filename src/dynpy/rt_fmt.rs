
// ---------- str() and repr() ----------

// The shortest round-trip digits as `{:e}` gives them, but a tie between two shortest candidates goes to the
// exact value rounded half-to-even, as in Python's repr (Rust rounds such ties up: -254.62014770507812).
pub fn short_e(x: f64) -> String {
    let s = format!("{:e}", x);
    let n = s.split_once('e').unwrap().0.bytes().filter(|c| c.is_ascii_digit()).count();
    // (two shortest candidates fit only when 15+ digits are needed)
    if n >= 15 {
        let t = format!("{:.*e}", n - 1, x);
        if t != s && t.parse::<f64>() == Ok(x) {
            return t;
        }
    }
    s
}
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let e = short_e(x);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if (-4..16).contains(&exp) {
        if exp < 0 {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        } else {
            let ip = exp as usize + 1;
            if digits.len() <= ip {
                out.push_str(&digits);
                for _ in 0..(ip - digits.len()) {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                out.push_str(&digits[..ip]);
                out.push('.');
                out.push_str(&digits[ip..]);
            }
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exp.abs()));
    }
    out
}
pub fn str_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if (0x80..0xa0).contains(&(c as u32)) => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if c as u32 == 0xad || c as u32 == 0xa0 => out.push_str(&format!("\\x{:02x}", c as u32)),
            // the common separators, format and private-use characters Python's repr escapes
            c if matches!(c as u32, 0x1680 | 0x2000..=0x200f | 0x2028..=0x202f | 0x205f..=0x2064 | 0x2066..=0x206f | 0x3000 | 0xe000..=0xf8ff | 0xfeff | 0xfff9..=0xfffb | 0xfffe | 0xffff) => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) >= 0xf0000 => out.push_str(&format!("\\U{:08x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}
thread_local! {
    static REPR_DEPTH: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}
fn guard(p: usize) -> bool {
    REPR_DEPTH.with(|d| {
        let mut d = d.borrow_mut();
        if d.contains(&p) {
            return false;
        }
        d.push(p);
        true
    })
}
fn unguard() {
    REPR_DEPTH.with(|d| {
        d.borrow_mut().pop();
    });
}
pub fn repr(v: &V) -> Result<String, E> {
    Ok(match v {
        V::None => "None".into(),
        V::Bool(b) => if *b { "True" } else { "False" }.into(),
        V::Int(i) => i.to_string(),
        V::Big(b) => {
            let s = b.to_string();
            if s.len() > 4300 + b.neg as usize {
                return val_err("Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit".into());
            }
            s
        }
        V::Float(f) => float_repr(*f),
        V::Str(s) => str_repr(&s.s),
        V::List(l) => {
            let p = Rc::as_ptr(l) as usize;
            if !guard(p) {
                return Ok("[...]".into());
            }
            let items = l.borrow().clone();
            let r = join_repr(&items);
            unguard();
            format!("[{}]", r?)
        }
        V::Tuple(t) => {
            if t.len() == 1 {
                format!("({},)", repr(&t[0])?)
            } else {
                format!("({})", join_repr(t)?)
            }
        }
        V::Dict(d) => {
            let p = Rc::as_ptr(d) as usize;
            if !guard(p) {
                return Ok("{...}".into());
            }
            let (items, kind, dflt) = {
                let d = d.borrow();
                (d.items(), d.kind, d.dflt.clone())
            };
            let mut parts = vec![];
            for (k, x) in &items {
                match (|| -> Result<String, E> { Ok(format!("{}: {}", repr(k)?, repr(x)?)) })() {
                    Ok(s) => parts.push(s),
                    Err(e) => {
                        unguard();
                        return Err(e);
                    }
                }
            }
            unguard();
            let body = format!("{{{}}}", parts.join(", "));
            match kind {
                1 => format!("defaultdict({}, {})", match &dflt {
                    Some(f) => repr(f)?,
                    None => "None".into(),
                }, body),
                2 => {
                    // Counter repr: most common first
                    let mut items = items.clone();
                    sort_by_key_desc_count(&mut items)?;
                    if items.is_empty() {
                        "Counter()".into()
                    } else {
                        let mut ps = vec![];
                        for (k, x) in &items {
                            ps.push(format!("{}: {}", repr(k)?, repr(x)?));
                        }
                        format!("Counter({{{}}})", ps.join(", "))
                    }
                }
                3 => {
                    if items.is_empty() {
                        "OrderedDict()".into()
                    } else {
                        format!("OrderedDict({body})")
                    }
                }
                _ => body,
            }
        }
        V::Set(s) => {
            let ks = s.borrow().keys();
            if ks.is_empty() {
                "set()".into()
            } else {
                format!("{{{}}}", join_repr(&ks)?)
            }
        }
        V::FSet(s) => {
            let ks = s.keys();
            if ks.is_empty() {
                "frozenset()".into()
            } else {
                format!("frozenset({{{}}})", join_repr(&ks)?)
            }
        }
        V::Func(f) => {
            if f.kind.get() == 3 {
                format!("<built-in function {}>", f.name)
            } else {
                format!("<function {} at 0x{:x}>", f.name, Rc::as_ptr(f) as *const u8 as usize)
            }
        }
        V::Class(c) => {
            if is_enum(c) {
                format!("<{} '{}'>", if c.lookup("_flag_mask_").is_some() { "flag" } else { "enum" }, c.name)
            } else if c.module == "builtins" {
                format!("<class '{}'>", c.name)
            } else {
                format!("<class '{}.{}'>", c.module, c.name)
            }
        }
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__repr__") {
                return match call(&f, Args::p(vec![v.clone()]))? {
                    V::Str(s) => Ok(s.s.clone()),
                    x => type_err(format!("__repr__ returned non-string (type {})", tn(&x))),
                };
            }
            if o.cls.base_bt() == BT::Exc {
                let args = o.get("args").unwrap_or(tup(vec![]));
                let a = match &args {
                    V::Tuple(t) if t.len() == 1 => format!("({})", repr(&t[0])?),
                    _ => repr(&args)?,
                };
                return Ok(format!("{}{}", o.cls.name, a));
            }
            if let Some(i) = inner_of(v) {
                return repr(&i);
            }
            format!("<{}.{} object at 0x{:x}>", o.cls.module, o.cls.name, Rc::as_ptr(o) as *const u8 as usize)
        }
        V::Bound(b) => {
            let fname = match &b.1 {
                V::Func(f) => f.name,
                _ => "?",
            };
            format!("<bound method {} of {}>", fname, repr(&b.0)?)
        }
        V::BMeth(b) => format!("<built-in method {} of {} object>", b.1, tn(&b.0)),
        V::Iter(_) => "<iterator object>".into(),
        V::Range(r) => {
            if r.2 == 1 {
                format!("range({}, {})", r.0, r.1)
            } else {
                format!("range({}, {}, {})", r.0, r.1, r.2)
            }
        }
        V::Slice(s) => format!("slice({}, {}, {})", repr(&s.0)?, repr(&s.1)?, repr(&s.2)?),
        V::Deque(d) => {
            let v: Vec<V> = d.borrow().v.iter().cloned().collect();
            match d.borrow().max {
                Some(m) => format!("deque([{}], maxlen={})", join_repr(&v)?, m),
                None => format!("deque([{}])", join_repr(&v)?),
            }
        }
        V::Module(m) => format!("<module '{}'>", m),
        V::View(x) => {
            let name = ["dict_keys", "dict_values", "dict_items"][x.1 as usize];
            format!("{}([{}])", name, join_repr(&to_vec(v)?)?)
        }
        V::Prop(_) => "<property object>".into(),
        V::Ext(x) => x.repr(),
        V::NotImpl => "NotImplemented".into(),
        V::Ellipsis => "Ellipsis".into(),
        V::Unbound => "<unbound>".into(),
    })
}
fn sort_by_key_desc_count(items: &mut Vec<(V, V)>) -> Result<(), E> {
    // stable sort by count, highest first (Counter.most_common order)
    let mut keyed: Vec<(V, (V, V))> = items.drain(..).map(|p| (p.1.clone(), p)).collect();
    let mut err = None;
    merge_sort(&mut keyed, &mut |a: &(V, (V, V)), b: &(V, (V, V))| match lt(&b.0, &a.0) {
        Ok(x) => x,
        Err(e) => {
            err.get_or_insert(e);
            false
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    items.extend(keyed.into_iter().map(|x| x.1));
    Ok(())
}
fn join_repr(xs: &[V]) -> Result<String, E> {
    let mut s = String::new();
    for (i, x) in xs.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&repr(x)?);
    }
    Ok(s)
}
pub fn to_str(v: &V) -> Result<String, E> {
    Ok(match v {
        V::Str(s) => s.s.clone(),
        V::Float(f) => float_repr(*f),
        V::Int(i) => i.to_string(),
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__str__") {
                return match call(&f, Args::p(vec![v.clone()]))? {
                    V::Str(s) => Ok(s.s.clone()),
                    x => type_err(format!("__str__ returned non-string (type {})", tn(&x))),
                };
            }
            if o.cls.base_bt() == BT::Exc {
                let args = o.get("args").unwrap_or(tup(vec![]));
                // UnicodeDecodeError(encoding, object, start, end, reason)
                if let V::Tuple(t) = &args {
                    if t.len() == 5 && o.cls.is_sub(&exc_class("UnicodeDecodeError")) {
                        if let (V::Str(e), Some(b), V::Int(a), V::Int(z), V::Str(r)) = (&t[0], bytes_of(&t[1]), &t[2], &t[3], &t[4]) {
                            return Ok(if *z == *a + 1 && (*a as usize) < b.len() {
                                format!("'{}' codec can't decode byte 0x{:02x} in position {a}: {}", e.s, b[*a as usize], r.s)
                            } else {
                                format!("'{}' codec can't decode bytes in position {a}-{}: {}", e.s, z - 1, r.s)
                            });
                        }
                    }
                }
                return Ok(match &args {
                    V::Tuple(t) if t.is_empty() => String::new(),
                    V::Tuple(t) if t.len() == 1 => {
                        if o.cls.is_sub(&exc_class("KeyError")) {
                            repr(&t[0])?
                        } else {
                            to_str(&t[0])?
                        }
                    }
                    _ => repr(&args)?,
                });
            }
            if o.cls.lookup("__repr__").is_some() {
                return repr(v);
            }
            if let Some(i) = inner_of(v) {
                return to_str(&i);
            }
            repr(v)?
        }
        V::Ext(x) => x.str_(),
        _ => repr(v)?,
    })
}
pub fn str_v(v: &V) -> R {
    if let V::Str(_) = v {
        return Ok(v.clone());
    }
    Ok(st(to_str(v)?))
}

// ---------- format specs ----------

struct Spec {
    fill: char,
    align: Option<char>,
    sign: char,
    alt: bool,
    zero: bool,
    width: usize,
    group: Option<char>,
    prec: Option<usize>,
    ty: Option<char>,
}
fn parse_spec(spec: &str) -> Result<Spec, E> {
    let c: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut sp = Spec { fill: ' ', align: None, sign: '-', alt: false, zero: false, width: 0, group: None, prec: None, ty: None };
    if c.len() >= 2 && matches!(c[1], '<' | '>' | '^' | '=') {
        sp.fill = c[0];
        sp.align = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && matches!(c[0], '<' | '>' | '^' | '=') {
        sp.align = Some(c[0]);
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        sp.sign = c[i];
        i += 1;
    }
    if i < c.len() && c[i] == 'z' {
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        sp.alt = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        sp.zero = true;
        i += 1;
    }
    let st = i;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i > st {
        sp.width = c[st..i].iter().collect::<String>().parse().unwrap_or(0);
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        sp.group = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == st {
            return val_err("Format specifier missing precision".into());
        }
        sp.prec = Some(c[st..i].iter().collect::<String>().parse().unwrap_or(0));
    }
    if i < c.len() {
        sp.ty = Some(c[i]);
        i += 1;
    }
    if i < c.len() {
        return val_err(format!("Invalid format specifier '{spec}'"));
    }
    Ok(sp)
}
fn group_digits(int_part: &str, sep: char, every: usize) -> String {
    let n = int_part.len();
    let mut out = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (n - i) % every == 0 {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}
fn pad(body: String, sign: &str, sp: &Spec, default_right: bool) -> String {
    let len = body.chars().count() + sign.chars().count();
    if len >= sp.width {
        return format!("{sign}{body}");
    }
    let n = sp.width - len;
    let (fill, align) = if sp.zero && sp.align.is_none() { ('0', '=') } else { (sp.fill, sp.align.unwrap_or(if default_right { '>' } else { '<' })) };
    let f = |k: usize| std::iter::repeat(fill).take(k).collect::<String>();
    match align {
        '<' => format!("{sign}{body}{}", f(n)),
        '>' => format!("{}{sign}{body}", f(n)),
        '^' => format!("{}{sign}{body}{}", f(n / 2), f(n - n / 2)),
        _ => format!("{sign}{}{body}", f(n)),
    }
}
// Fixed/exponent digits for a float with Python's rules.
fn fmt_float(x: f64, ty: char, prec: Option<usize>, alt: bool) -> String {
    if x.is_nan() {
        return if ty.is_ascii_uppercase() { "NAN".into() } else { "nan".into() };
    }
    if x.is_infinite() {
        return if ty.is_ascii_uppercase() { "INF".into() } else { "inf".into() };
    }
    match ty {
        'f' | 'F' => {
            let p = prec.unwrap_or(6);
            let mut s = format!("{:.*}", p, x.abs());
            if alt && p == 0 {
                s.push('.');
            }
            s
        }
        'e' | 'E' => {
            let p = prec.unwrap_or(6);
            let s = format!("{:.*e}", p, x.abs());
            let (m, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let mut m = m.to_string();
            if alt && p == 0 {
                m.push('.');
            }
            let r = format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs());
            if ty == 'E' {
                r.to_uppercase()
            } else {
                r
            }
        }
        '%' => {
            let p = prec.unwrap_or(6);
            format!("{:.*}%", p, x.abs() * 100.0)
        }
        'g' | 'G' | 'n' => {
            let p = prec.unwrap_or(6).max(1);
            if x == 0.0 {
                return if alt { format!("{:.*}", p - 1, 0.0) } else { "0".into() };
            }
            let s = format!("{:.*e}", p - 1, x.abs());
            let (_, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let r = if e >= -4 && e < p as i32 {
                let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                if !alt && s.contains('.') {
                    s = s.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                s
            } else {
                let (m, _) = s.split_once('e').unwrap();
                let mut m = m.to_string();
                if !alt && m.contains('.') {
                    m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
            };
            if ty == 'G' {
                r.to_uppercase()
            } else {
                r
            }
        }
        _ => {
            // no type: repr-like, or 'g'-like with at least one decimal when a precision is given
            match prec {
                None => float_repr(x.abs()),
                Some(p) => {
                    let p = p.max(1);
                    let s = format!("{:.*e}", p - 1, x.abs());
                    let (_, e) = s.split_once('e').unwrap();
                    let e: i32 = e.parse().unwrap();
                    if e >= -4 && e < p as i32 {
                        let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                        if s.contains('.') {
                            s = s.trim_end_matches('0').to_string();
                            if s.ends_with('.') {
                                s.push('0');
                            }
                        } else {
                            s.push_str(".0");
                        }
                        s
                    } else {
                        let (m, _) = s.split_once('e').unwrap();
                        let mut m = m.to_string();
                        if m.contains('.') {
                            m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                        }
                        format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
                    }
                }
            }
        }
    }
}
fn sign_str(neg: bool, sp: &Spec) -> &'static str {
    if neg {
        "-"
    } else {
        match sp.sign {
            '+' => "+",
            ' ' => " ",
            _ => "",
        }
    }
}
fn group_num(body: &str, sp: &Spec) -> String {
    match sp.group {
        None => body.to_string(),
        Some(g) => {
            let (ip, rest) = match body.find(|c: char| !c.is_ascii_digit()) {
                Some(i) => (&body[..i], &body[i..]),
                None => (body, ""),
            };
            format!("{}{}", group_digits(ip, g, 3), rest)
        }
    }
}
pub fn format_spec(v: &V, spec: &str) -> Result<String, E> {
    match v {
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__format__") {
                return to_str(&call(&f, Args::p(vec![v.clone(), s(spec)]))?);
            }
            // Enum.__format__ formats str(member); IntEnum, StrEnum and IntFlag format the value
            if is_enum(&o.cls) {
                return match inner_of(v) {
                    Some(i) if is_repr_enum(&o.cls) => format_spec(&i, spec),
                    _ => format_spec(&st(to_str(v)?), spec),
                };
            }
            if spec.is_empty() {
                return to_str(v);
            }
            if let Some(i) = inner_of(v) {
                return format_spec(&i, spec);
            }
            return type_err(format!("unsupported format string passed to {}.__format__", o.cls.name));
        }
        V::Ext(x) => {
            if let Some(r) = x.fmt(spec) {
                return to_str(&r?);
            }
            if spec.is_empty() {
                return to_str(v);
            }
        }
        _ => {}
    }
    if spec.is_empty() {
        return to_str(v);
    }
    let sp = parse_spec(spec)?;
    match v {
        V::Str(x) => {
            if let Some(t) = sp.ty {
                if t != 's' {
                    return val_err(format!("Unknown format code '{t}' for object of type 'str'"));
                }
            }
            if sp.sign != '-' {
                return val_err("Sign not allowed in string format specifier".into());
            }
            if sp.align == Some('=') {
                return val_err("'=' alignment not allowed in string format specifier".into());
            }
            let body: String = match sp.prec {
                Some(p) => x.s.chars().take(p).collect(),
                None => x.s.clone(),
            };
            Ok(pad(body, "", &sp, false))
        }
        V::Int(_) | V::Big(_) | V::Bool(_) if !matches!(sp.ty, Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%')) => {
            if let V::Bool(b) = v {
                if sp.ty.is_none() {
                    return Ok(pad(if *b { "True" } else { "False" }.into(), "", &sp, false));
                }
            }
            let b = big_of(v).unwrap();
            let neg = b.neg;
            let mag = b.neg().clone();
            let a = if neg { mag } else { b.clone() };
            if sp.prec.is_some() {
                return val_err("Precision not allowed in integer format specifier".into());
            }
            let (body, prefix) = match sp.ty {
                None | Some('d') | Some('n') => (a.to_string(), ""),
                Some('x') => (a.to_radix(16), "0x"),
                Some('X') => (a.to_radix(16).to_uppercase(), "0X"),
                Some('o') => (a.to_radix(8), "0o"),
                Some('b') => (a.to_radix(2), "0b"),
                Some('c') => (char::from_u32(a.to_i64().unwrap_or(0) as u32).unwrap_or('?').to_string(), ""),
                Some('s') => return val_err("Unknown format code 's' for object of type 'int'".into()),
                Some(t) => return val_err(format!("Unknown format code '{t}' for object of type 'int'")),
            };
            let body = match sp.group {
                Some(g) => group_digits(&body, g, if matches!(sp.ty, Some('x' | 'X' | 'o' | 'b')) { 4 } else { 3 }),
                None => body,
            };
            let sign = format!("{}{}", sign_str(neg, &sp), if sp.alt { prefix } else { "" });
            if sp.zero && sp.align.is_none() && sp.group.is_some() && body.len() + sign.len() < sp.width {
                // zero padding goes through the grouping
                let digits: String = body.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                let g = sp.group.unwrap();
                let mut d = digits.clone();
                loop {
                    let t = group_digits(&d, g, 3);
                    if t.len() + sign.len() >= sp.width {
                        return Ok(format!("{sign}{t}"));
                    }
                    d.insert(0, '0');
                }
            }
            Ok(pad(body, &sign, &sp, true))
        }
        V::Float(_) | V::Int(_) | V::Big(_) | V::Bool(_) => {
            let x = num_f(v).unwrap();
            if let Some(t) = sp.ty {
                if !matches!(t, 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | 'n' | '%') {
                    return val_err(format!("Unknown format code '{t}' for object of type 'float'"));
                }
            }
            let neg = x.is_sign_negative() && !(x == 0.0 && false) && !x.is_nan();
            let neg = neg && !(x == 0.0 && matches!(v, V::Int(_)));
            let mut body = fmt_float(x, sp.ty.unwrap_or('\0'), sp.prec, sp.alt);
            body = group_num(&body, &sp);
            let sign = sign_str(neg, &sp);
            if sp.zero && sp.align.is_none() && sp.group.is_some() {
                let mut b = body.clone();
                while b.len() + sign.len() < sp.width {
                    let ip: String = b.split(|c: char| c == '.' || c == 'e' || c == '%').next().unwrap().chars().filter(|c| c.is_ascii_digit()).collect();
                    let rest = &b[b.find(|c: char| c == '.' || c == 'e' || c == '%').unwrap_or(b.len())..];
                    let t = group_digits(&format!("0{ip}"), sp.group.unwrap(), 3);
                    b = format!("{t}{rest}");
                }
                return Ok(format!("{sign}{b}"));
            }
            Ok(pad(body, sign, &sp, true))
        }
        V::None | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) => type_err(format!("unsupported format string passed to {}.__format__", tn(v))),
        _ => {
            let s = to_str(v)?;
            Ok(pad(s, "", &sp, false))
        }
    }
}
pub fn fmt_v(v: &V, spec: &str) -> R {
    Ok(st(format_spec(v, spec)?))
}

// '%5.2f' % x
pub fn percent_format(f: &str, args: &V) -> Result<String, E> {
    let (items, dict): (Vec<V>, Option<V>) = match args {
        V::Tuple(t) => ((**t).clone(), None),
        V::Dict(_) => (vec![args.clone()], Some(args.clone())),
        x => (vec![x.clone()], None),
    };
    let c: Vec<char> = f.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut k = 0;
    let next = |k: &mut usize| -> Result<V, E> {
        let v = items.get(*k).cloned().ok_or_else(|| exc("TypeError", "not enough arguments for format string".into()))?;
        *k += 1;
        Ok(v)
    };
    while i < c.len() {
        if c[i] != '%' {
            out.push(c[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= c.len() {
            return val_err("incomplete format".into());
        }
        if c[i] == '%' {
            out.push('%');
            i += 1;
            continue;
        }
        let mut key = None;
        if c[i] == '(' {
            let st = i + 1;
            while i < c.len() && c[i] != ')' {
                i += 1;
            }
            key = Some(c[st..i].iter().collect::<String>());
            i += 1;
        }
        let mut flags = String::new();
        while i < c.len() && matches!(c[i], '-' | '+' | ' ' | '#' | '0') {
            flags.push(c[i]);
            i += 1;
        }
        let mut width = String::new();
        if i < c.len() && c[i] == '*' {
            width = to_str(&next(&mut k)?)?;
            i += 1;
        }
        while i < c.len() && c[i].is_ascii_digit() {
            width.push(c[i]);
            i += 1;
        }
        let mut prec = None;
        if i < c.len() && c[i] == '.' {
            i += 1;
            let mut p = String::new();
            if i < c.len() && c[i] == '*' {
                p = to_str(&next(&mut k)?)?;
                i += 1;
            }
            while i < c.len() && c[i].is_ascii_digit() {
                p.push(c[i]);
                i += 1;
            }
            prec = Some(p.parse::<usize>().unwrap_or(0));
        }
        while i < c.len() && matches!(c[i], 'l' | 'h' | 'L') {
            i += 1;
        }
        if i >= c.len() {
            return val_err("incomplete format".into());
        }
        let ty = c[i];
        i += 1;
        let v = match &key {
            Some(kk) => getitem(dict.as_ref().ok_or_else(|| exc("TypeError", "format requires a mapping".into()))?, &s(kk))?,
            None => next(&mut k)?,
        };
        let left = flags.contains('-');
        let mut spec = String::new();
        if left {
            spec.push('<');
        } else if !matches!(ty, 's' | 'r' | 'a' | 'c') {
            // numbers right aligned by default
        } else {
            spec.push('>');
        }
        if flags.contains('+') {
            spec.push('+');
        } else if flags.contains(' ') {
            spec.push(' ');
        }
        if flags.contains('#') {
            spec.push('#');
        }
        if flags.contains('0') && !left && !matches!(ty, 's' | 'r' | 'a' | 'c') {
            spec.push('0');
        }
        spec.push_str(&width);
        let piece = match ty {
            's' | 'r' | 'a' => {
                let mut t = if ty == 's' { to_str(&v)? } else { repr(&v)? };
                if let Some(p) = prec {
                    t = t.chars().take(p).collect();
                }
                format_spec(&st(t), &spec)?
            }
            'd' | 'i' | 'u' => {
                let n = match &v {
                    V::Float(x) => {
                        if !x.is_finite() {
                            return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
                        }
                        Big::from_f64(*x).norm()
                    }
                    V::Int(_) | V::Big(_) | V::Bool(_) => v.clone(),
                    V::Ext(_) => to_int(&v)?,
                    // an int subclass (IntEnum): its int
                    V::Obj(_) if matches!(inner_of(&v), Some(V::Int(_) | V::Big(_) | V::Bool(_))) => inner_of(&v).unwrap(),
                    _ => return type_err(format!("%{ty} format: a real number is required, not {}", tn(&v))),
                };
                let n = if let V::Bool(b) = n { V::Int(b as i64) } else { n };
                let mut t = format_spec(&n, &format!("{spec}d"))?;
                if let Some(p) = prec {
                    // precision = minimum digits
                    let neg = t.trim_start().starts_with('-');
                    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
                    if digits.len() < p {
                        let d = format!("{}{}", "0".repeat(p - digits.len()), digits);
                        t = format_spec(&st(format!("{}{}", if neg { "-" } else { "" }, d)), &format!("{}{}", if left { "<" } else { ">" }, width))?;
                    }
                }
                t
            }
            'x' | 'X' | 'o' => {
                let n = match &v {
                    V::Int(_) | V::Big(_) | V::Bool(_) => v.clone(),
                    _ => return type_err(format!("%{ty} format: an integer is required, not {}", tn(&v))),
                };
                let mut sp2 = spec.clone();
                let alt = sp2.contains('#');
                sp2 = sp2.replace('#', "");
                let t = format_spec(&n, &format!("{}{}{ty}", if alt { "#" } else { "" }, sp2))?;
                t
            }
            'e' | 'E' | 'f' | 'F' | 'g' | 'G' => {
                let x = match &v {
                    V::Str(_) | V::None | V::List(_) | V::Tuple(_) | V::Dict(_) => return type_err(format!("must be real number, not {}", tn(&v))),
                    _ => V::Float(to_float(&v)?),
                };
                format_spec(&x, &format!("{spec}.{}{ty}", prec.unwrap_or(6)))?
            }
            'c' => {
                let t = match &v {
                    V::Int(i) => char::from_u32(*i as u32).unwrap_or('?').to_string(),
                    V::Str(x) => x.s.clone(),
                    _ => return type_err("%c requires int or char".into()),
                };
                format_spec(&st(t), &spec)?
            }
            _ => return val_err(format!("unsupported format character '{ty}' (0x{:x}) at index {}", ty as u32, i - 1)),
        };
        out.push_str(&piece);
    }
    if k < items.len() && dict.is_none() {
        return type_err("not all arguments converted during string formatting".into());
    }
    Ok(out)
}

// ---------- numbers from values ----------

pub fn to_float(v: &V) -> Result<f64, E> {
    match v {
        V::Float(f) => Ok(*f),
        V::Int(i) => Ok(*i as f64),
        V::Bool(b) => Ok(*b as i64 as f64),
        V::Big(b) => {
            let f = b.to_f64();
            if f.is_infinite() {
                return Err(exc("OverflowError", "int too large to convert to float".into()));
            }
            Ok(f)
        }
        V::Str(s) => parse_float(&s.s).ok_or_else(|| exc("ValueError", format!("could not convert string to float: {}", str_repr(&s.s)))),
        V::Obj(o) => match o.cls.lookup("__float__") {
            Some(f) => to_float(&call(&f, Args::p(vec![v.clone()]))?),
            None => type_err(format!("float() argument must be a string or a real number, not '{}'", tn(v))),
        },
        V::Ext(x) => x.to_f64().ok_or_else(|| exc("TypeError", format!("float() argument must be a string or a real number, not '{}'", tn(v)))),
        _ => type_err(format!("float() argument must be a string or a real number, not '{}'", tn(v))),
    }
}
pub fn parse_float(t: &str) -> Option<f64> {
    let t = t.trim();
    let l = t.to_ascii_lowercase();
    let body = l.trim_start_matches(['+', '-']);
    let neg = l.starts_with('-');
    let r = match body {
        "inf" | "infinity" => Some(f64::INFINITY),
        "nan" => Some(f64::NAN),
        _ => {
            if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | '+' | '-' | '_')) || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
                return None;
            }
            if body.starts_with('e') || body.starts_with('.') && body.len() == 1 {
                return None;
            }
            let b = body.replace('_', "");
            b.parse::<f64>().ok()
        }
    }?;
    Some(if neg { -r } else { r })
}
pub fn parse_int(t: &str, base: u32) -> Option<V> {
    let t = t.trim();
    let (neg, mut body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let mut base = base;
    let lower = body.to_ascii_lowercase();
    if base == 16 && lower.starts_with("0x") || base == 8 && lower.starts_with("0o") || base == 2 && lower.starts_with("0b") {
        body = &body[2..];
    } else if base == 0 {
        if lower.starts_with("0x") {
            base = 16;
            body = &body[2..];
        } else if lower.starts_with("0o") {
            base = 8;
            body = &body[2..];
        } else if lower.starts_with("0b") {
            base = 2;
            body = &body[2..];
        } else {
            base = 10;
            if body.len() > 1 && body.starts_with('0') && body.chars().any(|c| c != '0' && c != '_') {
                return None;
            }
        }
    }
    if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
        return None;
    }
    let digits: String = body.chars().filter(|&c| c != '_').collect();
    if !digits.chars().all(|c| c.is_digit(base)) {
        // allow non-ascii digits for base 10
        if base == 10 && digits.chars().all(|c| c.is_numeric()) {
            let d: String = digits.chars().map(|c| c.to_digit(10).map(|x| std::char::from_digit(x, 10).unwrap()).unwrap_or(c)).collect();
            if d.chars().all(|c| c.is_ascii_digit()) {
                return parse_int(&format!("{}{}", if neg { "-" } else { "" }, d), 10);
            }
        }
        return None;
    }
    if digits.len() < 18 {
        if let Ok(n) = i64::from_str_radix(&digits, base) {
            return Some(V::Int(if neg { -n } else { n }));
        }
    }
    let b = Big::parse(&digits, base)?;
    Some(if neg { b.neg() } else { b }.norm())
}
pub fn to_int(v: &V) -> R {
    match v {
        V::Int(_) | V::Big(_) => Ok(v.clone()),
        V::Bool(b) => Ok(V::Int(*b as i64)),
        V::Float(f) => {
            if f.is_nan() {
                return val_err("cannot convert float NaN to integer".into());
            }
            if f.is_infinite() {
                return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
            }
            let t = f.trunc();
            if t.abs() < 9.2e18 {
                Ok(V::Int(t as i64))
            } else {
                Ok(Big::from_f64(t).norm())
            }
        }
        V::Str(s) => parse_int(&s.s, 10).ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base 10: {}", str_repr(&s.s)))),
        V::Obj(o) => {
            for n in ["__int__", "__index__", "__trunc__"] {
                if let Some(f) = o.cls.lookup(n) {
                    return call(&f, Args::p(vec![v.clone()]));
                }
            }
            // a subclass of int, float or str (IntEnum, StrEnum)
            if let Some(i) = inner_of(v).filter(|i| matches!(i, V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_) | V::Str(_))) {
                return to_int(&i);
            }
            type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v)))
        }
        V::Ext(x) => match x.method(v, "__int__", &mut Args::p(vec![])) {
            Some(r) => r,
            None => type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v))),
        },
        _ => type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v))),
    }
}

// Stable merge sort using only a "less than" test (Python's sort only calls <).
pub fn merge_sort<T: Clone>(v: &mut Vec<T>, lt: &mut dyn FnMut(&T, &T) -> bool) {
    let n = v.len();
    if n < 2 {
        return;
    }
    // insertion sort for small runs
    if n <= 16 {
        for i in 1..n {
            let mut j = i;
            while j > 0 && lt(&v[j], &v[j - 1]) {
                v.swap(j, j - 1);
                j -= 1;
            }
        }
        return;
    }
    let mut right = v.split_off(n / 2);
    merge_sort(v, lt);
    merge_sort(&mut right, lt);
    let left = std::mem::take(v);
    let mut out = Vec::with_capacity(n);
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if lt(&right[j], &left[i]) {
            out.push(right[j].clone());
            j += 1;
        } else {
            out.push(left[i].clone());
            i += 1;
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    *v = out;
}
// list.sort(key=..., reverse=...)
pub fn sort_values(v: &mut Vec<V>, key: Option<&V>, reverse: bool) -> Result<(), E> {
    let mut keyed: Vec<(V, V)> = match key {
        Some(k) if !matches!(k, V::None) => {
            let mut out = Vec::with_capacity(v.len());
            for x in v.iter() {
                out.push((call(k, Args::p(vec![x.clone()]))?, x.clone()));
            }
            out
        }
        _ => v.iter().map(|x| (x.clone(), x.clone())).collect(),
    };
    if reverse {
        keyed.reverse();
    }
    let mut err: Option<E> = None;
    let all_int = keyed.iter().all(|(k, _)| matches!(k, V::Int(_)));
    let all_str = keyed.iter().all(|(k, _)| matches!(k, V::Str(_)));
    if all_int {
        keyed.sort_by(|a, b| if let (V::Int(x), V::Int(y)) = (&a.0, &b.0) { x.cmp(y) } else { std::cmp::Ordering::Equal });
    } else if all_str {
        keyed.sort_by(|a, b| if let (V::Str(x), V::Str(y)) = (&a.0, &b.0) { x.s.cmp(&y.s) } else { std::cmp::Ordering::Equal });
    } else {
        merge_sort(&mut keyed, &mut |a: &(V, V), b: &(V, V)| {
            if err.is_some() {
                return false;
            }
            match lt(&a.0, &b.0) {
                Ok(r) => r,
                Err(e) => {
                    err = Some(e);
                    false
                }
            }
        });
    }
    if let Some(e) = err {
        return Err(e);
    }
    if reverse {
        keyed.reverse();
    }
    *v = keyed.into_iter().map(|x| x.1).collect();
    Ok(())
}
