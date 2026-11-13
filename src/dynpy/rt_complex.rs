// ---------- complex ----------
//
// Python's complex, as CPython 3.13's Objects/complexobject.c computes it: the same formulas in the same order
// (Smith's division, the integer-power fast path, _Py_c_pow, _Py_c_abs), the same repr, hash and errors. Mixed
// arithmetic converts the int or float operand to complex first, as 3.13 does (3.14 changed that).

pub struct CplxV(pub f64, pub f64);
pub fn cplx(re: f64, im: f64) -> V {
    V::Ext(ExtRc::new(CplxV(re, im)))
}
pub fn cplx_of(v: &V) -> Option<(f64, f64)> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<CplxV>().map(|c| (c.0, c.1)),
        _ => None,
    }
}
// TO_COMPLEX: an int (exactly converted, or OverflowError), a float or a complex; None for anything else
// (the operation is NotImplemented for it).
fn to_cplx(v: &V) -> Option<Result<(f64, f64), E>> {
    Some(match v {
        V::Bool(b) => Ok((*b as i64 as f64, 0.0)),
        V::Int(_) | V::Big(_) => to_float(v).map(|f| (f, 0.0)),
        V::Float(f) => Ok((*f, 0.0)),
        _ => return cplx_of(v).map(Ok),
    })
}
fn c_prod(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
// _Py_c_quot; None where CPython sets errno = EDOM (division by zero)
fn c_quot(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let abs_breal = if b.0 < 0.0 { -b.0 } else { b.0 };
    let abs_bimag = if b.1 < 0.0 { -b.1 } else { b.1 };
    if abs_breal >= abs_bimag {
        if abs_breal == 0.0 {
            return None;
        }
        let ratio = b.1 / b.0;
        let denom = b.0 + b.1 * ratio;
        Some(((a.0 + a.1 * ratio) / denom, (a.1 - a.0 * ratio) / denom))
    } else if abs_bimag >= abs_breal {
        let ratio = b.0 / b.1;
        let denom = b.0 * ratio + b.1;
        Some(((a.0 * ratio + a.1) / denom, (a.1 * ratio - a.0) / denom))
    } else {
        Some((f64::NAN, f64::NAN))
    }
}
fn c_powu(x: (f64, f64), n: i64) -> (f64, f64) {
    let (mut r, mut p) = ((1.0, 0.0), x);
    let mut mask: i64 = 1;
    while mask > 0 && n >= mask {
        if n & mask != 0 {
            r = c_prod(r, p);
        }
        mask <<= 1;
        p = c_prod(p, p);
    }
    r
}
// complex_pow without the modulus: the result, or the error CPython's errno check raises. errno is modelled
// exactly: EDOM from a zero base (or a division by zero in the negative-power path), and from cos/sin of an
// infinite phase; ERANGE survives _Py_ADJUST_ERANGE2 only when a part of the result is infinite.
pub fn c_pow(a: (f64, f64), b: (f64, f64)) -> R {
    let mut edom = false;
    let p = if b.1 == 0.0 && b.0 == b.0.floor() && b.0.abs() <= 100.0 {
        let n = b.0 as i64;
        if n > 0 {
            c_powu(a, n)
        } else {
            match c_quot((1.0, 0.0), c_powu(a, -n)) {
                Some(q) => q,
                None => {
                    edom = true;
                    (0.0, 0.0)
                }
            }
        }
    } else if b.0 == 0.0 && b.1 == 0.0 {
        (1.0, 0.0)
    } else if a.0 == 0.0 && a.1 == 0.0 {
        if b.1 != 0.0 || b.0 < 0.0 {
            edom = true;
        }
        (0.0, 0.0)
    } else {
        let vabs = a.0.hypot(a.1);
        let mut len = vabs.powf(b.0);
        let at = a.1.atan2(a.0);
        let mut phase = at * b.0;
        if b.1 != 0.0 {
            len *= (-at * b.1).exp();
            phase += b.1 * vabs.ln();
        }
        if phase.is_infinite() {
            edom = true;
        }
        (len * phase.cos(), len * phase.sin())
    };
    if edom {
        return Err(exc("ZeroDivisionError", "0.0 to a negative or complex power".into()));
    }
    if p.0.is_infinite() || p.1.is_infinite() {
        return Err(exc("OverflowError", "complex exponentiation".into()));
    }
    Ok(cplx(fresh_float(p.0), fresh_float(p.1)))
}
// _Py_c_abs, with complex_abs's OverflowError
fn c_abs(z: (f64, f64)) -> R {
    if !z.0.is_finite() || !z.1.is_finite() {
        if z.0.is_infinite() {
            return Ok(V::Float(z.0.abs()));
        }
        if z.1.is_infinite() {
            return Ok(V::Float(z.1.abs()));
        }
        return Ok(V::Float(nan_obj()));
    }
    let r = z.0.hypot(z.1);
    if !r.is_finite() {
        return Err(exc("OverflowError", "absolute value too large".into()));
    }
    Ok(V::Float(r))
}
// PyOS_double_to_string(x, 'r', 0, flags): repr's shortest digits, without the ".0" a float's repr adds; the sign
// of a NaN is ignored, and Py_DTSF_SIGN puts a '+' before a non-negative part.
fn c_part(x: f64, sign: bool) -> String {
    let mut t = if x.is_nan() { "nan".to_string() } else { float_repr(x) };
    if let Some(s) = t.strip_suffix(".0") {
        t = s.to_string();
    }
    if sign && !t.starts_with('-') {
        t.insert(0, '+');
    }
    t
}
pub fn complex_repr(re: f64, im: f64) -> String {
    if re == 0.0 && re.is_sign_positive() {
        format!("{}j", c_part(im, false))
    } else {
        format!("({}{}j)", c_part(re, false), c_part(im, true))
    }
}
// The names dir(complex) lists in CPython 3.13. A name outside it is an AttributeError (so hasattr is right);
// one inside it that this file does not implement is refused where it is called.
const COMPLEX_NAMES: &[&str] = &[
    "__abs__", "__add__", "__bool__", "__class__", "__complex__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__", "__getattribute__", "__getnewargs__",
    "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__", "__le__", "__lt__", "__mul__", "__ne__", "__neg__", "__new__", "__pos__", "__pow__", "__radd__",
    "__reduce__", "__reduce_ex__", "__repr__", "__rmul__", "__rpow__", "__rsub__", "__rtruediv__", "__setattr__", "__sizeof__", "__str__", "__sub__", "__subclasshook__",
    "__truediv__", "conjugate", "imag", "real",
];
impl CplxV {
    fn arith(&self, op: &str, o: &V, right: bool) -> Option<R> {
        let me = (self.0, self.1);
        let other = match to_cplx(o)? {
            Ok(c) => c,
            Err(e) => return Some(Err(e)),
        };
        let (a, b) = if right { (other, me) } else { (me, other) };
        Some(match op {
            "+" => Ok(cplx(fresh_float(a.0 + b.0), fresh_float(a.1 + b.1))),
            "-" => Ok(cplx(fresh_float(a.0 - b.0), fresh_float(a.1 - b.1))),
            "*" => {
                let p = c_prod(a, b);
                Ok(cplx(fresh_float(p.0), fresh_float(p.1)))
            }
            "/" => match c_quot(a, b) {
                Some(q) => Ok(cplx(fresh_float(q.0), fresh_float(q.1))),
                None => Err(exc("ZeroDivisionError", "complex division by zero".into())),
            },
            "**" => c_pow(a, b),
            _ => return None,
        })
    }
}
impl Ext for CplxV {
    fn tname(&self) -> &'static str {
        "complex"
    }
    fn repr(&self) -> String {
        complex_repr(self.0, self.1)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        let hr = hash_float(self.0) as u64;
        let hi = hash_float(self.1) as u64;
        let h = hr.wrapping_add(1000003u64.wrapping_mul(hi)) as i64;
        Some(if h == -1 { -2 } else { h })
    }
    fn eq(&self, o: &V) -> Option<bool> {
        match o {
            // a complex equals an int when its imaginary part is 0 and its real part equals the int exactly
            V::Bool(_) | V::Int(_) | V::Big(_) => Some(self.1 == 0.0 && eq(&V::Float(self.0), o).unwrap_or(false)),
            V::Float(f) => Some(self.0 == *f && self.1 == 0.0),
            _ => match cplx_of(o) {
                Some((r, i)) => Some(self.0 == r && self.1 == i),
                // Fraction and Decimal compare with a complex whose imaginary part is 0 through its real part
                None => match o {
                    V::Ext(x) if self.1 == 0.0 && (x.any().is::<FracV>() || x.any().is::<DecV>()) => x.eq(&V::Float(self.0)),
                    V::Ext(x) if x.any().is::<FracV>() || x.any().is::<DecV>() => Some(false),
                    _ => None,
                },
            },
        }
    }
    fn eq_rich(&self, me: &V, o: &V, _right: bool) -> Option<Result<bool, E>> {
        // complex.__eq__ is NotImplemented for an instance of a class of the program's: that class's own __eq__
        // decides, else identity
        if let V::Obj(ob) = o {
            if let Some(f) = ob.cls.lookup("__eq__") {
                return Some(call(&f, Args::p(vec![o.clone(), me.clone()])).and_then(|r| if matches!(r, V::NotImpl) { Ok(is(me, o)) } else { truth(&r) }));
            }
        }
        None
    }
    fn truth(&self) -> bool {
        self.0 != 0.0 || self.1 != 0.0
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.repr())));
        }
        Some(complex_format(self.0, self.1, spec).map(st))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        match op {
            "neg" => return Some(Ok(cplx(-self.0, -self.1))),
            "pos" => return Some(Ok(cplx(self.0, self.1))),
            _ => {}
        }
        self.arith(op, o, right)
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        match n {
            "real" => Some(Ok(V::Float(self.0))),
            "imag" => Some(Ok(V::Float(self.1))),
            _ if COMPLEX_NAMES.contains(&n) => None,
            _ => Some(Err(exc("AttributeError", format!("'complex' object has no attribute '{n}'")))),
        }
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(exc(
            "AttributeError",
            if matches!(n, "real" | "imag") {
                format!("readonly attribute")
            } else if COMPLEX_NAMES.contains(&n) {
                format!("'complex' object attribute '{n}' is read-only")
            } else {
                format!("'complex' object has no attribute '{n}' and no __dict__ for setting new attributes")
            },
        )))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let one = |a: &Args| -> Result<V, E> {
            a.no_kw(n)?;
            if a.p.len() != 1 {
                return type_err(format!("expected 1 argument, got {}", a.p.len()));
            }
            Ok(a.p[0].clone())
        };
        let none = |a: &Args| -> Result<(), E> {
            a.no_kw(n)?;
            if !a.p.is_empty() {
                return type_err(format!("complex.{n}() takes no arguments ({} given)", a.p.len()));
            }
            Ok(())
        };
        if !COMPLEX_NAMES.contains(&n) {
            return None;
        }
        Some((|| -> R {
            Ok(match n {
                "conjugate" => {
                    none(a)?;
                    cplx(self.0, -self.1)
                }
                "__complex__" => {
                    none(a)?;
                    me.clone()
                }
                "__getnewargs__" => {
                    none(a)?;
                    tup(vec![V::Float(self.0), V::Float(self.1)])
                }
                "__abs__" => c_abs((self.0, self.1))?,
                "__bool__" => V::Bool(self.truth()),
                "__hash__" => V::Int(self.hash().unwrap()),
                "__repr__" | "__str__" => st(self.repr()),
                "__neg__" => cplx(-self.0, -self.1),
                "__pos__" => cplx(self.0, self.1),
                "__format__" => {
                    let s = one(a)?;
                    let V::Str(s) = s else { return type_err(format!("complex.__format__() argument must be str, not {}", tn(&s))) };
                    self.fmt(&s.text()).unwrap()?
                }
                "__eq__" | "__ne__" => {
                    let o = one(a)?;
                    match self.eq(&o) {
                        Some(b) => V::Bool(b == (n == "__eq__")),
                        None => V::NotImpl,
                    }
                }
                "__lt__" | "__le__" | "__gt__" | "__ge__" => {
                    one(a)?;
                    V::NotImpl
                }
                "__add__" | "__sub__" | "__mul__" | "__truediv__" | "__pow__" | "__radd__" | "__rsub__" | "__rmul__" | "__rtruediv__" | "__rpow__" => {
                    let o = if n.ends_with("pow__") {
                        a.no_kw(n)?;
                        if a.p.is_empty() || a.p.len() > 2 {
                            return type_err(format!("expected 1 or 2 arguments, got {}", a.p.len()));
                        }
                        if a.p.len() == 2 && !matches!(a.p[1], V::None) {
                            return val_err("complex modulo".into());
                        }
                        a.p[0].clone()
                    } else {
                        one(a)?
                    };
                    let right = n.starts_with("__r");
                    let op = match n.trim_start_matches("__r").trim_start_matches("__").trim_end_matches("__") {
                        "add" => "+",
                        "sub" => "-",
                        "mul" => "*",
                        "truediv" => "/",
                        _ => "**",
                    };
                    match self.arith(op, &o, right) {
                        Some(r) => r?,
                        None => V::NotImpl,
                    }
                }
                _ if COMPLEX_NAMES.contains(&n) => return Err(unsupported(&format!("complex.{n}"))),
                _ => unreachable!(),
            })
        })())
    }
}

// float() of a value for complex(): PyNumber_Float, whose errors complex() reports in its own words when the value
// is not a number at all.
fn complex_part(v: &V, first: bool) -> Result<(f64, f64, bool), E> {
    if let Some((r, i)) = cplx_of(v) {
        return Ok((r, i, true));
    }
    let number = match v {
        V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_) => true,
        V::Obj(o) => o.cls.lookup("__float__").is_some() || o.cls.lookup("__index__").is_some() || inner_of(v).is_some_and(|i| matches!(i, V::Int(_) | V::Big(_) | V::Float(_) | V::Bool(_))),
        V::Ext(x) => x.to_f64().is_some(),
        _ => false,
    };
    if !number {
        return type_err(if first {
            format!("complex() first argument must be a string or a number, not '{}'", tn(v))
        } else {
            format!("complex() second argument must be a number, not '{}'", tn(v))
        });
    }
    let f = match v {
        V::Obj(o) if o.cls.lookup("__float__").is_none() && o.cls.lookup("__index__").is_some() => to_float(&to_int(v)?)?,
        _ => to_float(v)?,
    };
    Ok((f, 0.0, false))
}
// try_complex_special_method
fn complex_special(v: &V) -> Result<Option<V>, E> {
    if let V::Obj(o) = v {
        if let Some(f) = o.cls.lookup("__complex__") {
            let r = call(&f, Args::p(vec![v.clone()]))?;
            if cplx_of(&r).is_none() {
                return type_err(format!("__complex__ returned non-complex (type {})", tn(&r)));
            }
            return Ok(Some(r));
        }
    }
    Ok(None)
}
pub fn bi_complex(mut a: Args) -> R {
    let mut real = a.kw("real");
    let mut imag = a.kw("imag");
    if let Some((k, _)) = a.k.first() {
        return type_err(format!("complex() got an unexpected keyword argument '{k}'"));
    }
    if a.p.len() > 2 {
        return type_err(format!("complex() takes at most 2 arguments ({} given)", a.p.len()));
    }
    let mut it = std::mem::take(&mut a.p).into_iter();
    if let Some(x) = it.next() {
        if real.is_some() {
            return type_err("argument for complex() given by name ('real') and position (1)".into());
        }
        real = Some(x);
    }
    if let Some(x) = it.next() {
        if imag.is_some() {
            return type_err("argument for complex() given by name ('imag') and position (2)".into());
        }
        imag = Some(x);
    }
    let r = real.unwrap_or(V::Int(0));
    if cplx_of(&r).is_some() && imag.is_none() {
        return Ok(r);
    }
    if let V::Str(s) = &r {
        if imag.is_some() {
            return type_err("complex() can't take second arg if first is a string".into());
        }
        return complex_from_str(&s.text());
    }
    if let Some(V::Str(_)) = &imag {
        return type_err("complex() second arg can't be a string".into());
    }
    let r = complex_special(&r)?.unwrap_or(r);
    let (mut cr_re, cr_im, cr_is_complex) = complex_part(&r, true)?;
    let (mut ci_re, ci_im, ci_is_complex) = match &imag {
        None => (cr_im, 0.0, false),
        Some(i) => complex_part(i, false)?,
    };
    if ci_is_complex {
        cr_re -= ci_im;
    }
    if cr_is_complex && imag.is_some() {
        ci_re += cr_im;
    }
    Ok(cplx(cr_re, ci_re))
}
// a complex literal's imaginary part (`2.5j`)
pub fn bi___complex_lit(a: Args) -> R {
    match a.p.first() {
        Some(V::Float(f)) => Ok(cplx(0.0, *f)),
        _ => Ok(cplx(0.0, 0.0)),
    }
}
// The longest float at the start of s, as PyOS_string_to_double reads it (no leading space, an optional sign,
// digits with an optional fraction and exponent, or inf / infinity / nan): the value and the bytes it took.
fn float_prefix(s: &[u8]) -> Option<(f64, usize)> {
    let mut i = 0;
    let neg = match s.first() {
        Some(b'+') => {
            i = 1;
            false
        }
        Some(b'-') => {
            i = 1;
            true
        }
        _ => false,
    };
    let rest = &s[i..];
    let lower: Vec<u8> = rest.iter().take(8).map(|c| c.to_ascii_lowercase()).collect();
    let special = if lower.starts_with(b"infinity") {
        Some((f64::INFINITY, 8))
    } else if lower.starts_with(b"inf") {
        Some((f64::INFINITY, 3))
    } else if lower.starts_with(b"nan") {
        Some((nan_obj(), 3))
    } else {
        None
    };
    if let Some((v, n)) = special {
        return Some((if neg { -v } else { v }, i + n));
    }
    let start = i;
    let mut digits = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < s.len() && s[i] == b'.' {
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let mut k = i + 1;
        if k < s.len() && (s[k] == b'+' || s[k] == b'-') {
            k += 1;
        }
        if k < s.len() && s[k].is_ascii_digit() {
            while k < s.len() && s[k].is_ascii_digit() {
                k += 1;
            }
            i = k;
        }
    }
    let t = std::str::from_utf8(&s[start..i]).ok()?;
    let v: f64 = t.parse().ok()?;
    Some((if neg { -v } else { v }, i))
}
fn py_isspace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'\x0b' | b'\x0c')
}
// complex(str): complex_subtype_from_string, then complex_from_string_inner
fn complex_from_str(orig: &str) -> R {
    // _PyUnicode_TransformDecimalAndSpaceToASCII: Unicode spaces become ' ', decimal digits their ASCII digit,
    // any other non-ASCII character '?'
    let mut t = String::with_capacity(orig.len());
    for c in orig.chars() {
        if c.is_ascii() {
            t.push(c);
        } else if c.is_whitespace() {
            t.push(' ');
        } else if let Some(d) = uni_decimal(c) {
            t.push((b'0' + d as u8) as char);
        } else {
            t.push('?');
        }
    }
    // _Py_string_to_number_with_underscores: an underscore must sit between two digits; they are then dropped
    let b = t.as_bytes();
    let s: Vec<u8> = if b.contains(&b'_') {
        let mut out = Vec::with_capacity(b.len());
        let mut prev = 0u8;
        for (k, &c) in b.iter().enumerate() {
            if c == b'_' {
                let next = b.get(k + 1).copied().unwrap_or(0);
                if !prev.is_ascii_digit() || !next.is_ascii_digit() {
                    return val_err(format!("could not convert string to complex: {}", str_repr(orig)));
                }
            } else {
                out.push(c);
            }
            prev = c;
        }
        out
    } else {
        b.to_vec()
    };
    let bad = || val_err("complex() arg is a malformed string".into());
    let mut i = 0;
    while i < s.len() && py_isspace(s[i]) {
        i += 1;
    }
    let mut bracket = false;
    if i < s.len() && s[i] == b'(' {
        bracket = true;
        i += 1;
        while i < s.len() && py_isspace(s[i]) {
            i += 1;
        }
    }
    let at = |k: usize| s.get(k).copied().unwrap_or(0);
    let (mut x, mut y) = (0.0, 0.0);
    match float_prefix(&s[i..]) {
        Some((z, n)) => {
            i += n;
            if at(i) == b'+' || at(i) == b'-' {
                x = z;
                match float_prefix(&s[i..]) {
                    Some((w, m)) => {
                        y = w;
                        i += m;
                    }
                    None => {
                        y = if at(i) == b'+' { 1.0 } else { -1.0 };
                        i += 1;
                    }
                }
                if !(at(i) == b'j' || at(i) == b'J') {
                    return bad();
                }
                i += 1;
            } else if at(i) == b'j' || at(i) == b'J' {
                i += 1;
                y = z;
            } else {
                x = z;
            }
        }
        None => {
            if at(i) == b'+' || at(i) == b'-' {
                y = if at(i) == b'+' { 1.0 } else { -1.0 };
                i += 1;
            } else {
                y = 1.0;
            }
            if !(at(i) == b'j' || at(i) == b'J') {
                return bad();
            }
            i += 1;
        }
    }
    while i < s.len() && py_isspace(s[i]) {
        i += 1;
    }
    if bracket {
        if at(i) != b')' {
            return bad();
        }
        i += 1;
        while i < s.len() && py_isspace(s[i]) {
            i += 1;
        }
    }
    if i != s.len() {
        return bad();
    }
    Ok(cplx(x, y))
}
// pow(a, b, m) when one of them is a complex: CPython's ternary_op asks each distinct operand type's nb_power in
// turn (a's, b's, then m's). int's is NotImplemented here; float's raises when a and b are real numbers; complex's
// raises "complex modulo" when a and b are numbers. None: no complex is involved.
pub fn complex_pow3(a: &V, b: &V, m: &V) -> Option<R> {
    if cplx_of(a).is_none() && cplx_of(b).is_none() && cplx_of(m).is_none() {
        return None;
    }
    let real = |v: &V| matches!(v, V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_));
    let num = |v: &V| real(v) || cplx_of(v).is_some();
    for v in [a, b, m] {
        if matches!(v, V::Float(_)) && real(a) && real(b) {
            return Some(type_err("pow() 3rd argument not allowed unless all arguments are integers".into()));
        }
        if cplx_of(v).is_some() && num(a) && num(b) {
            return Some(val_err("complex modulo".into()));
        }
    }
    Some(type_err(format!("unsupported operand type(s) for ** or pow(): '{}', '{}', '{}'", tn(a), tn(b), tn(m))))
}

// format(z, spec): format_complex_internal (Python/formatter_unicode.c). Each part is formatted as a float with
// the spec's sign, z, # and grouping options and no width; the imaginary part always carries a sign unless the
// real part is left out; then the whole is padded.
fn complex_format(re: f64, im: f64, spec: &str) -> Result<String, E> {
    let c: Vec<char> = spec.chars().collect();
    let invalid = || val_err(format!("Invalid format specifier '{spec}' for object of type 'complex'"));
    let mut i = 0;
    let (mut fill, mut align) = (None, None);
    if c.len() >= 2 && "<>=^".contains(c[1]) {
        fill = Some(c[0]);
        align = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && "<>=^".contains(c[0]) {
        align = Some(c[0]);
        i = 1;
    }
    let mut sign = None;
    if i < c.len() && "+- ".contains(c[i]) {
        sign = Some(c[i]);
        i += 1;
    }
    let mut z = false;
    if i < c.len() && c[i] == 'z' {
        z = true;
        i += 1;
    }
    let mut alt = false;
    if i < c.len() && c[i] == '#' {
        alt = true;
        i += 1;
    }
    if fill.is_none() && i < c.len() && c[i] == '0' {
        fill = Some('0');
        if align.is_none() {
            align = Some('=');
        }
        i += 1;
    }
    let mut width = 0usize;
    while i < c.len() && c[i].is_ascii_digit() {
        width = width.saturating_mul(10).saturating_add(c[i] as usize - '0' as usize);
        i += 1;
    }
    let mut group = None;
    if i < c.len() && (c[i] == ',' || c[i] == '_') {
        group = Some(c[i]);
        i += 1;
    }
    let mut prec = None;
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st0 = i;
        let mut p = 0usize;
        while i < c.len() && c[i].is_ascii_digit() {
            p = p.saturating_mul(10).saturating_add(c[i] as usize - '0' as usize);
            i += 1;
        }
        if i == st0 {
            return val_err("Format specifier missing precision".into());
        }
        prec = Some(p);
    }
    if c.len() - i > 1 {
        return invalid();
    }
    let ty = c.get(i).copied();
    if let (Some(g), Some('n')) = (group, ty) {
        return val_err(format!("Cannot specify '{g}' with 'n'."));
    }
    if !matches!(ty, None | Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | 'n')) {
        return val_err(format!("Unknown format code '{}' for object of type 'complex'", ty.unwrap()));
    }
    if fill == Some('0') {
        return val_err("Zero padding is not allowed in complex format specifier".into());
    }
    if align == Some('=') {
        return val_err("'=' alignment flag is not allowed in complex format specifier".into());
    }
    // the omitted type is str()'s shortest repr digits, without the ".0" a float's adds; '#' would add a point
    let shortest = ty.is_none() && prec.is_none();
    let (skip_re, parens) = if ty.is_none() { (re == 0.0 && re.is_sign_positive(), !(re == 0.0 && re.is_sign_positive())) } else { (false, false) };
    let ft = match ty {
        None if prec.is_some() => "g".to_string(),
        None => String::new(),
        Some(t) => t.to_string(),
    };
    let part = |x: f64, sg: Option<char>| -> Result<String, E> {
        let mut f = String::new();
        if let Some(s) = sg {
            f.push(s);
        }
        if z {
            f.push('z');
        }
        if alt && !shortest {
            f.push('#');
        }
        if let Some(g) = group {
            f.push(g);
        }
        if let Some(p) = prec {
            f.push_str(&format!(".{p}"));
        }
        f.push_str(&ft);
        let mut t = format_spec(&V::Float(x), &f)?;
        if shortest && x.is_finite() {
            if let Some(s) = t.strip_suffix(".0") {
                t = s.to_string();
            }
            // '#' with repr's digits (Py_DTSF_ALT): a decimal point always, before any exponent
            if alt && !t.contains('.') {
                match t.find(['e', 'E']) {
                    Some(k) => t.insert(k, '.'),
                    None => t.push('.'),
                }
            }
        }
        Ok(t)
    };
    let mut body = String::new();
    if parens {
        body.push('(');
    }
    if !skip_re {
        body.push_str(&part(re, sign)?);
    }
    body.push_str(&part(im, if skip_re { sign } else { Some('+') })?);
    body.push('j');
    if parens {
        body.push(')');
    }
    let n = body.chars().count();
    if width <= n {
        return Ok(body);
    }
    let padn = width - n;
    let fc = fill.unwrap_or(' ');
    let lpad = match align.unwrap_or('>') {
        '>' => padn,
        '^' => padn / 2,
        _ => 0,
    };
    let mut out: String = std::iter::repeat(fc).take(lpad).collect();
    out.push_str(&body);
    out.extend(std::iter::repeat(fc).take(padn - lpad));
    Ok(out)
}
