// ---------- decimal, fractions, datetime, csv, html, statistics, threading ----------

fn ten_pow(n: u64) -> Big {
    Big::from_i64(10).pow(n)
}
fn ndig(c: &Big) -> usize {
    if c.is_zero() {
        1
    } else {
        c.to_string().len()
    }
}
fn big_is_odd(c: &Big) -> bool {
    c.mag.first().is_some_and(|w| w & 1 == 1)
}
fn bigi(i: i64) -> Big {
    Big::from_i64(i)
}

thread_local! {
    static DPREC: Cell<usize> = Cell::new(28);
    static DROUND: Cell<u8> = Cell::new(0);
}
const ROUNDINGS: [&str; 8] = ["ROUND_HALF_EVEN", "ROUND_HALF_UP", "ROUND_HALF_DOWN", "ROUND_UP", "ROUND_DOWN", "ROUND_CEILING", "ROUND_FLOOR", "ROUND_05UP"];
fn round_code(v: &V) -> Result<u8, E> {
    let t = to_str(v)?;
    ROUNDINGS.iter().position(|r| *r == t).map(|i| i as u8).ok_or_else(|| exc("TypeError", "valid values for rounding are: [ROUND_CEILING, ROUND_FLOOR, ROUND_UP, ROUND_DOWN, ROUND_HALF_UP, ROUND_HALF_DOWN, ROUND_HALF_EVEN, ROUND_05UP]".into()))
}

#[derive(Clone, Debug)]
pub struct Dec {
    neg: bool,
    c: Big,
    e: i64,
    sp: u8, // 0 finite, 1 infinity, 2 nan
}
fn dec_err(kind: &str) -> E {
    let cls = match kind {
        "DivisionByZero" => "DivisionByZero",
        _ => "InvalidOperation",
    };
    exc(cls, format!("[<class 'decimal.{kind}'>]"))
}
impl Dec {
    fn zero() -> Dec {
        Dec { neg: false, c: bigi(0), e: 0, sp: 0 }
    }
    fn from_big(b: &Big) -> Dec {
        Dec { neg: b.neg, c: if b.neg { b.neg() } else { b.clone() }, e: 0, sp: 0 }
    }
    fn from_f64(f: f64) -> Dec {
        if f.is_nan() {
            return Dec { neg: false, c: bigi(0), e: 0, sp: 2 };
        }
        if f.is_infinite() {
            return Dec { neg: f < 0.0, c: bigi(0), e: 0, sp: 1 };
        }
        let neg = f.is_sign_negative();
        let bits = f.abs().to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i64;
        let mut m = bits & ((1u64 << 52) - 1);
        let mut e2 = if exp == 0 { -1074 } else { exp - 1075 };
        if exp != 0 {
            m |= 1u64 << 52;
        }
        if m == 0 {
            return Dec { neg, c: bigi(0), e: 0, sp: 0 };
        }
        while m & 1 == 0 {
            m >>= 1;
            e2 += 1;
        }
        let mb = Big::from_i128(m as i128);
        if e2 >= 0 {
            Dec { neg, c: mb.shl(e2 as usize), e: 0, sp: 0 }
        } else {
            Dec { neg, c: mb.mul(&Big::from_i64(5).pow((-e2) as u64)), e: e2, sp: 0 }
        }
    }
    fn parse(t: &str) -> Option<Dec> {
        let t = t.trim().replace('_', "");
        let (neg, body) = match t.strip_prefix('-') {
            Some(r) => (true, r.to_string()),
            None => (false, t.strip_prefix('+').unwrap_or(&t).to_string()),
        };
        let low = body.to_ascii_lowercase();
        if low == "inf" || low == "infinity" {
            return Some(Dec { neg, c: bigi(0), e: 0, sp: 1 });
        }
        if low == "nan" {
            return Some(Dec { neg, c: bigi(0), e: 0, sp: 2 });
        }
        let (mant, ex) = match low.find('e') {
            Some(i) => (&body[..i], body[i + 1..].parse::<i64>().ok()?),
            None => (&body[..], 0),
        };
        let (ip, fp) = match mant.find('.') {
            Some(i) => (&mant[..i], &mant[i + 1..]),
            None => (mant, ""),
        };
        if ip.is_empty() && fp.is_empty() {
            return None;
        }
        if !ip.chars().chain(fp.chars()).all(|c| c.is_ascii_digit()) {
            return None;
        }
        let digits = format!("{ip}{fp}");
        let c = Big::parse(if digits.is_empty() { "0" } else { &digits }, 10)?;
        Some(Dec { neg, c, e: ex - fp.len() as i64, sp: 0 })
    }
    fn of(v: &V) -> Option<Dec> {
        match v {
            V::Int(_) | V::Bool(_) | V::Big(_) => Some(Dec::from_big(&big_of(v)?)),
            V::Ext(x) => x.any().downcast_ref::<DecV>().map(|d| d.0.clone()),
            _ => None,
        }
    }
    fn signed(&self) -> Big {
        if self.neg {
            self.c.neg()
        } else {
            self.c.clone()
        }
    }
    fn is_zero(&self) -> bool {
        self.sp == 0 && self.c.is_zero()
    }
    // the coefficient divided by 10^drop, rounded
    fn round_off(neg: bool, c: &Big, drop: u64, mode: u8) -> Big {
        if drop == 0 {
            return c.clone();
        }
        let p = ten_pow(drop);
        let (q, r) = c.divmod(&p);
        if r.is_zero() {
            return q;
        }
        let half = r.add(&r).cmp(&p);
        use std::cmp::Ordering::*;
        let inc = match mode {
            0 => half == Greater || (half == Equal && big_is_odd(&q)),
            1 => half != Less,
            2 => half == Greater,
            3 => true,
            4 => false,
            5 => !neg,
            6 => neg,
            _ => {
                let last = q.divmod(&bigi(10)).1.to_i64().unwrap_or(0);
                last == 0 || last == 5
            }
        };
        if inc {
            q.add(&bigi(1))
        } else {
            q
        }
    }
    fn fix(mut self) -> Dec {
        if self.sp != 0 {
            return self;
        }
        let prec = DPREC.with(|p| p.get());
        let nd = ndig(&self.c);
        if nd > prec {
            let drop = (nd - prec) as u64;
            self.c = Dec::round_off(self.neg, &self.c, drop, DROUND.with(|r| r.get()));
            self.e += drop as i64;
            if ndig(&self.c) > prec {
                self.c = self.c.divmod(&bigi(10)).0;
                self.e += 1;
            }
        }
        self
    }
    fn to_s(&self) -> String {
        let sign = if self.neg { "-" } else { "" };
        match self.sp {
            1 => return format!("{sign}Infinity"),
            2 => return "NaN".into(),
            _ => {}
        }
        let digits = self.c.to_string();
        let n = digits.len() as i64;
        let left = self.e + n;
        let dot = if self.e <= 0 && left > -6 { left } else { 1 };
        let (ip, fp) = if dot <= 0 {
            ("0".to_string(), format!(".{}{}", "0".repeat((-dot) as usize), digits))
        } else if dot >= n {
            (format!("{}{}", digits, "0".repeat((dot - n) as usize)), String::new())
        } else {
            (digits[..dot as usize].to_string(), format!(".{}", &digits[dot as usize..]))
        };
        let ex = if left == dot { String::new() } else { format!("E{:+}", left - dot) };
        format!("{sign}{ip}{fp}{ex}")
    }
    fn align(a: &Dec, b: &Dec) -> (Big, Big, i64) {
        let e = a.e.min(b.e);
        let x = a.signed().mul(&ten_pow((a.e - e) as u64));
        let y = b.signed().mul(&ten_pow((b.e - e) as u64));
        (x, y, e)
    }
    fn from_signed(s: Big, e: i64) -> Dec {
        Dec { neg: s.neg, c: if s.neg { s.neg() } else { s }, e, sp: 0 }
    }
    fn cmp(&self, o: &Dec) -> Option<std::cmp::Ordering> {
        if self.sp == 2 || o.sp == 2 {
            return None;
        }
        if self.sp == 1 || o.sp == 1 {
            let k = |d: &Dec| if d.sp == 1 { if d.neg { -1 } else { 1 } } else { 0 };
            let (a, b) = (k(self), k(o));
            if a != b {
                return Some(a.cmp(&b));
            }
            if a != 0 {
                return Some(std::cmp::Ordering::Equal);
            }
        }
        let (x, y, _) = Dec::align(self, o);
        Some(x.cmp(&y))
    }
    fn add(&self, o: &Dec) -> Result<Dec, E> {
        if self.sp != 0 || o.sp != 0 {
            if self.sp == 2 || o.sp == 2 {
                return Ok(Dec { neg: false, c: bigi(0), e: 0, sp: 2 });
            }
            if self.sp == 1 && o.sp == 1 && self.neg != o.neg {
                return Err(dec_err("InvalidOperation"));
            }
            return Ok(if self.sp == 1 { self.clone() } else { o.clone() });
        }
        let (x, y, e) = Dec::align(self, o);
        let s = x.add(&y);
        let mut d = Dec::from_signed(s, e);
        if d.c.is_zero() {
            d.neg = self.neg && o.neg;
        }
        Ok(d.fix())
    }
    fn neg_(&self) -> Dec {
        let mut d = self.clone();
        if !(d.sp == 0 && d.c.is_zero()) || d.neg {
            d.neg = !d.neg;
        }
        if d.sp == 0 && d.c.is_zero() {
            d.neg = false;
        }
        d.fix()
    }
    fn mul(&self, o: &Dec) -> Result<Dec, E> {
        let neg = self.neg != o.neg;
        if self.sp != 0 || o.sp != 0 {
            if self.sp == 2 || o.sp == 2 {
                return Ok(Dec { neg: false, c: bigi(0), e: 0, sp: 2 });
            }
            if self.is_zero() || o.is_zero() {
                return Err(dec_err("InvalidOperation"));
            }
            return Ok(Dec { neg, c: bigi(0), e: 0, sp: 1 });
        }
        Ok(Dec { neg, c: self.c.mul(&o.c), e: self.e + o.e, sp: 0 }.fix())
    }
    fn div(&self, o: &Dec) -> Result<Dec, E> {
        let neg = self.neg != o.neg;
        if self.sp != 0 || o.sp != 0 {
            if self.sp == 2 || o.sp == 2 || (self.sp == 1 && o.sp == 1) {
                return if self.sp == 1 && o.sp == 1 { Err(dec_err("InvalidOperation")) } else { Ok(Dec { neg: false, c: bigi(0), e: 0, sp: 2 }) };
            }
            if self.sp == 1 {
                return Ok(Dec { neg, c: bigi(0), e: 0, sp: 1 });
            }
            return Ok(Dec { neg, c: bigi(0), e: 0, sp: 0 });
        }
        if o.c.is_zero() {
            return Err(dec_err(if self.c.is_zero() { "DivisionUndefined" } else { "DivisionByZero" }));
        }
        if self.c.is_zero() {
            return Ok(Dec { neg, c: bigi(0), e: self.e - o.e, sp: 0 });
        }
        let prec = DPREC.with(|p| p.get()) as i64;
        let shift = ndig(&o.c) as i64 - ndig(&self.c) as i64 + prec + 1;
        let mut exp = self.e - o.e - shift;
        let (mut q, r) = if shift >= 0 { self.c.mul(&ten_pow(shift as u64)).divmod(&o.c) } else { self.c.divmod(&o.c.mul(&ten_pow((-shift) as u64))) };
        if !r.is_zero() {
            if q.divmod(&bigi(5)).1.is_zero() {
                q = q.add(&bigi(1));
            }
        } else {
            let ideal = self.e - o.e;
            while exp < ideal {
                let (qq, rr) = q.divmod(&bigi(10));
                if !rr.is_zero() {
                    break;
                }
                q = qq;
                exp += 1;
            }
        }
        Ok(Dec { neg, c: q, e: exp, sp: 0 }.fix())
    }
    // truncating integer division and remainder (Decimal semantics)
    fn divmod_(&self, o: &Dec) -> Result<(Dec, Dec), E> {
        if self.sp != 0 || o.sp != 0 {
            return Err(dec_err("InvalidOperation"));
        }
        if o.c.is_zero() {
            return Err(if self.c.is_zero() { dec_err("DivisionUndefined") } else { dec_err("DivisionByZero") });
        }
        let e = self.e.min(o.e);
        let x = self.c.mul(&ten_pow((self.e - e) as u64));
        let y = o.c.mul(&ten_pow((o.e - e) as u64));
        let (q, r) = x.divmod(&y);
        let qd = Dec { neg: (self.neg != o.neg) && !q.is_zero(), c: q, e: 0, sp: 0 };
        let rd = Dec { neg: self.neg && !r.is_zero(), c: r, e, sp: 0 };
        Ok((qd.fix(), rd.fix()))
    }
    fn quantize(&self, exp: i64, mode: u8) -> Result<Dec, E> {
        if self.sp != 0 {
            return Err(dec_err("InvalidOperation"));
        }
        let mut d = self.clone();
        if exp > d.e {
            d.c = Dec::round_off(d.neg, &d.c, (exp - d.e) as u64, mode);
        } else {
            d.c = d.c.mul(&ten_pow((d.e - exp) as u64));
        }
        d.e = exp;
        if ndig(&d.c) > DPREC.with(|p| p.get()) {
            return Err(dec_err("InvalidOperation"));
        }
        Ok(d)
    }
    fn to_f64(&self) -> f64 {
        match self.sp {
            1 => {
                if self.neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }
            }
            2 => f64::NAN,
            _ => {
                let t = format!("{}{}e{}", if self.neg { "-" } else { "" }, self.c.to_string(), self.e);
                t.parse().unwrap_or(0.0)
            }
        }
    }
    fn trunc(&self) -> Result<Big, E> {
        if self.sp != 0 {
            return Err(exc(if self.sp == 1 { "OverflowError" } else { "ValueError" }, if self.sp == 1 { "cannot convert Infinity to integer".into() } else { "cannot convert NaN to integer".into() }));
        }
        let m = if self.e >= 0 { self.c.mul(&ten_pow(self.e as u64)) } else { self.c.divmod(&ten_pow((-self.e) as u64)).0 };
        Ok(if self.neg { m.neg() } else { m })
    }
    fn hash(&self) -> i64 {
        if self.sp != 0 {
            return 0;
        }
        const P: i128 = (1i128 << 61) - 1;
        if self.e >= 0 {
            let b = self.signed().mul(&ten_pow(self.e as u64));
            return hash_big(&b);
        }
        // exactly integral?
        let p10 = ten_pow((-self.e) as u64);
        let (q, r) = self.c.divmod(&p10);
        if r.is_zero() {
            return hash_big(&if self.neg { q.neg() } else { q });
        }
        let modp = |b: &Big| -> i128 { b.divmod(&Big::from_i128(P)).1.to_i128().unwrap_or(0) };
        let powm = |mut b: i128, mut e: u64| -> i128 {
            let mut r = 1i128;
            b %= P;
            while e > 0 {
                if e & 1 == 1 {
                    r = r * b % P;
                }
                b = b * b % P;
                e >>= 1;
            }
            r
        };
        let inv10 = powm(10, (P - 2) as u64);
        let h = modp(&self.c) * powm(inv10, (-self.e) as u64) % P;
        let ans = if self.neg { -h } else { h } as i64;
        if ans == -1 {
            -2
        } else {
            ans
        }
    }
    fn fmt(&self, spec: &str) -> Result<String, E> {
        let sp = parse_spec(spec, None)?;
        let mut d = self.clone();
        let mode = DROUND.with(|r| r.get());
        let body = match sp.ty {
            Some('f') | Some('F') | Some('%') => {
                if sp.ty == Some('%') {
                    d = Dec { neg: d.neg, c: d.c.clone(), e: d.e + 2, sp: d.sp };
                }
                if d.sp != 0 {
                    d.to_s().trim_start_matches('-').to_string()
                } else {
                    if let Some(p) = sp.prec {
                        let mut q = d.clone();
                        q.c = if -(p as i64) > q.e { Dec::round_off(q.neg, &q.c, (-(p as i64) - q.e) as u64, mode) } else { q.c.mul(&ten_pow((q.e + p as i64) as u64)) };
                        q.e = -(p as i64);
                        d = q;
                    }
                    let digits = d.c.to_string();
                    let mut t = if d.e >= 0 {
                        format!("{}{}", digits, "0".repeat(d.e as usize))
                    } else {
                        let k = (-d.e) as usize;
                        let padded = if digits.len() <= k { format!("{}{}", "0".repeat(k + 1 - digits.len()), digits) } else { digits };
                        format!("{}.{}", &padded[..padded.len() - k], &padded[padded.len() - k..])
                    };
                    if sp.alt && !t.contains('.') {
                        t.push('.');
                    }
                    if sp.ty == Some('%') {
                        t.push('%');
                    }
                    t
                }
            }
            None => {
                if let Some(p) = sp.prec {
                    // significant digits, like 'g'
                    let nd = ndig(&d.c);
                    if nd > p {
                        let drop = (nd - p) as u64;
                        d.c = Dec::round_off(d.neg, &d.c, drop, mode);
                        d.e += drop as i64;
                    }
                }
                d.to_s().trim_start_matches('-').to_string()
            }
            Some('e') | Some('E') | Some('g') | Some('G') => {
                let f = d.to_f64().abs();
                fmt_float(f, sp.ty.unwrap(), sp.prec, sp.alt)
            }
            Some(t) => return val_err(format!("Unknown format code '{t}' for object of type 'Decimal'")),
        };
        let body = group_num(&body, &sp);
        let neg = d.neg && !(d.sp == 0 && d.c.is_zero() && false);
        Ok(pad(body, sign_str(neg, &sp), &sp, true))
    }
}

pub struct DecV(pub Dec);
fn decv(d: Dec) -> V {
    V::Ext(ExtRc::new(DecV(d)))
}
impl Ext for DecV {
    fn tname(&self) -> &'static str {
        "decimal.Decimal"
    }
    fn repr(&self) -> String {
        format!("Decimal('{}')", self.0.to_s())
    }
    fn str_(&self) -> String {
        self.0.to_s()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(self.0.hash())
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if let V::Float(f) = o {
            return Some(self.0.cmp(&Dec::from_f64(*f)) == Some(std::cmp::Ordering::Equal));
        }
        if let Some(fr) = Frac::of(o) {
            return Some(Frac::of_dec(&self.0).is_some_and(|x| x.cmp(&fr) == std::cmp::Ordering::Equal));
        }
        Dec::of(o).map(|d| self.0.cmp(&d) == Some(std::cmp::Ordering::Equal))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        if let V::Float(f) = o {
            return self.0.cmp(&Dec::from_f64(*f));
        }
        Dec::of(o).and_then(|d| self.0.cmp(&d))
    }
    fn truth(&self) -> bool {
        !self.0.is_zero()
    }
    fn to_f64(&self) -> Option<f64> {
        Some(self.0.to_f64())
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        Some(self.0.fmt(spec).map(st))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        if op == "neg" {
            return Some(Ok(decv(self.0.neg_())));
        }
        if op == "pos" {
            return Some(Ok(decv(self.0.clone().fix())));
        }
        let other = Dec::of(o)?;
        let (a, b) = if right { (other, self.0.clone()) } else { (self.0.clone(), other) };
        Some((|| -> R {
            Ok(match op {
                "+" => decv(a.add(&b)?),
                "-" => decv(a.add(&b.neg_())?),
                "*" => decv(a.mul(&b)?),
                "/" => decv(a.div(&b)?),
                "//" => decv(a.divmod_(&b)?.0),
                "%" => decv(a.divmod_(&b)?.1),
                "**" => {
                    let n = b.trunc()?;
                    if b.cmp(&Dec::from_big(&n)) != Some(std::cmp::Ordering::Equal) {
                        return Err(unsupported("Decimal ** non-integer"));
                    }
                    let n = n.to_i64().ok_or_else(|| dec_err("Overflow"))?;
                    let mut r = Dec { neg: false, c: bigi(1), e: 0, sp: 0 };
                    for _ in 0..n.unsigned_abs() {
                        r = r.mul(&a)?;
                    }
                    if n < 0 {
                        r = Dec { neg: false, c: bigi(1), e: 0, sp: 0 }.div(&r)?;
                    }
                    decv(r)
                }
                _ => return Err(exc("TypeError", format!("unsupported operand type(s) for {op}: 'decimal.Decimal' and '{}'", tn(o)))),
            })
        })())
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let d = &self.0;
        Some((|| -> R {
            Ok(match n {
                "quantize" => {
                    let q = Dec::of(&a.p.first().cloned().unwrap_or(V::None)).ok_or_else(|| exc("TypeError", "conversion from object to Decimal is not supported".into()))?;
                    let mode = match a.kw("rounding").or_else(|| a.p.get(1).cloned()) {
                        Some(V::None) | None => DROUND.with(|r| r.get()),
                        Some(r) => round_code(&r)?,
                    };
                    decv(d.quantize(q.e, mode)?)
                }
                "__round__" => match a.p.first() {
                    None | Some(V::None) => {
                        let q = d.quantize(0, 0)?;
                        q.trunc()?.norm()
                    }
                    Some(nd) => {
                        let k = match nd {
                            V::Int(k) => *k,
                            _ => return type_err("integer argument expected".into()),
                        };
                        decv(d.quantize(-k, DROUND.with(|r| r.get()))?)
                    }
                },
                "__int__" | "__trunc__" | "to_integral_int" => d.trunc()?.norm(),
                "__float__" => V::Float(d.to_f64()),
                "__abs__" | "copy_abs" => {
                    let mut x = d.clone();
                    x.neg = false;
                    decv(if n == "__abs__" { x.fix() } else { x })
                }
                "__floor__" | "__ceil__" => {
                    let q = d.quantize(0, if n == "__floor__" { 6 } else { 5 })?;
                    q.trunc()?.norm()
                }
                "to_integral_value" | "to_integral" | "to_integral_exact" => {
                    let mode = match a.kw("rounding").or_else(|| a.p.first().cloned()) {
                        Some(V::None) | None => DROUND.with(|r| r.get()),
                        Some(r) => round_code(&r)?,
                    };
                    if d.e >= 0 {
                        decv(d.clone())
                    } else {
                        decv(d.quantize(0, mode)?)
                    }
                }
                "normalize" => {
                    let mut x = d.clone().fix();
                    if x.sp == 0 {
                        if x.c.is_zero() {
                            x.e = 0;
                        } else {
                            loop {
                                let (q, r) = x.c.divmod(&bigi(10));
                                if !r.is_zero() {
                                    break;
                                }
                                x.c = q;
                                x.e += 1;
                            }
                        }
                    }
                    decv(x)
                }
                "is_zero" => V::Bool(d.is_zero()),
                "is_signed" => V::Bool(d.neg),
                "is_nan" => V::Bool(d.sp == 2),
                "is_infinite" => V::Bool(d.sp == 1),
                "is_finite" => V::Bool(d.sp == 0),
                "adjusted" => V::Int(d.e + ndig(&d.c) as i64 - 1),
                "scaleb" => {
                    let k = match a.p.first() {
                        Some(V::Int(k)) => *k,
                        Some(o) => Dec::of(o).and_then(|x| x.trunc().ok()).and_then(|x| x.to_i64()).ok_or_else(|| exc("TypeError", "conversion from object to Decimal is not supported".into()))?,
                        None => return type_err("scaleb() missing required argument 'other' (pos 1)".into()),
                    };
                    let mut x = d.clone();
                    if x.sp == 0 {
                        x.e += k;
                    }
                    decv(x.fix())
                }
                "sqrt" => {
                    if d.neg && !d.is_zero() {
                        return Err(dec_err("InvalidOperation"));
                    }
                    // Newton's method on a scaled integer, then rounded to the context precision
                    let prec = DPREC.with(|p| p.get()) as i64;
                    let mut e = d.e;
                    let mut c = d.c.clone();
                    if e % 2 != 0 {
                        c = c.mul(&bigi(10));
                        e -= 1;
                    }
                    let extra = (2 * (prec + 2) - ndig(&c) as i64).max(0);
                    let extra = extra + extra % 2;
                    c = c.mul(&ten_pow(extra as u64));
                    e -= extra;
                    let mut x = ten_pow(((ndig(&c) + 1) / 2) as u64);
                    loop {
                        let y = x.add(&c.divmod(&x).0).divmod(&bigi(2)).0;
                        if y.cmp(&x) != std::cmp::Ordering::Less {
                            break;
                        }
                        x = y;
                    }
                    let exact = x.mul(&x).cmp(&c) == std::cmp::Ordering::Equal;
                    if !exact {
                        x = x.mul(&bigi(10)).add(&bigi(1));
                        e -= 2;
                    }
                    let mut r = Dec { neg: false, c: x, e: e / 2, sp: 0 }.fix();
                    if exact {
                        let ideal = d.e.div_euclid(2);
                        while r.e < ideal {
                            let (q, rr) = r.c.divmod(&bigi(10));
                            if !rr.is_zero() {
                                break;
                            }
                            r.c = q;
                            r.e += 1;
                        }
                    }
                    decv(r)
                }
                "as_tuple" => tup(vec![V::Int(d.neg as i64), tup(d.c.to_string().chars().map(|c| V::Int(c.to_digit(10).unwrap() as i64)).collect()), V::Int(d.e)]),
                "compare" => {
                    let o = Dec::of(&a.p[0]).unwrap_or(Dec::zero());
                    decv(Dec::from_big(&bigi(match d.cmp(&o) {
                        Some(std::cmp::Ordering::Less) => -1,
                        Some(std::cmp::Ordering::Greater) => 1,
                        _ => 0,
                    })))
                }
                "max" | "min" => {
                    let o = Dec::of(&a.p[0]).unwrap_or(Dec::zero());
                    let less = d.cmp(&o) == Some(std::cmp::Ordering::Less);
                    decv(if less == (n == "max") { o } else { d.clone() })
                }
                "__format__" => st(d.fmt(&to_str(&a.p[0])?)?),
                "__str__" => st(d.to_s()),
                "__repr__" => st(format!("Decimal('{}')", d.to_s())),
                _ => return Err(exc("AttributeError", format!("'decimal.Decimal' object has no attribute '{n}'"))),
            })
        })())
    }
}
fn decimal_new(mut a: Args) -> R {
    let v = a.kw("value").or_else(|| a.p.first().cloned()).unwrap_or(V::Int(0));
    let d = match &v {
        V::Str(x) => Dec::parse(&x.s).ok_or_else(|| dec_err("ConversionSyntax"))?,
        V::Float(f) => Dec::from_f64(*f),
        V::Tuple(t) if t.len() == 3 => {
            let neg = truth(&t[0])?;
            let digits: String = to_vec(&t[1])?.iter().map(|x| to_str(x).unwrap_or_default()).collect();
            Dec { neg, c: Big::parse(if digits.is_empty() { "0" } else { &digits }, 10).unwrap_or(bigi(0)), e: match &t[2] {
                V::Int(e) => *e,
                _ => 0,
            }, sp: 0 }
        }
        _ => Dec::of(&v).ok_or_else(|| exc("TypeError", format!("conversion from {} to Decimal is not supported", tn(&v))))?,
    };
    Ok(decv(d))
}

// ---------- fractions ----------

#[derive(Clone, Debug)]
pub struct Frac {
    n: Big,
    d: Big,
}
impl Frac {
    fn new(n: Big, d: Big) -> Result<Frac, E> {
        if d.is_zero() {
            return Err(exc("ZeroDivisionError", format!("Fraction({}, 0)", n.to_string())));
        }
        let g = big_gcd(&n, &d);
        let (mut n, mut d) = if g.is_zero() || g.cmp(&bigi(1)) == std::cmp::Ordering::Equal { (n, d) } else { (n.divmod(&g).0, d.divmod(&g).0) };
        if d.neg {
            n = n.neg();
            d = d.neg();
        }
        Ok(Frac { n, d })
    }
    fn of(v: &V) -> Option<Frac> {
        match v {
            V::Int(_) | V::Bool(_) | V::Big(_) => Some(Frac { n: big_of(v)?, d: bigi(1) }),
            V::Ext(x) => x.any().downcast_ref::<FracV>().map(|f| f.0.clone()),
            _ => None,
        }
    }
    fn of_f64(f: f64) -> Option<Frac> {
        if !f.is_finite() {
            return None;
        }
        let d = Dec::from_f64(f);
        Frac::of_dec(&d)
    }
    fn of_dec(d: &Dec) -> Option<Frac> {
        if d.sp != 0 {
            return None;
        }
        let s = d.signed();
        if d.e >= 0 {
            Frac::new(s.mul(&ten_pow(d.e as u64)), bigi(1)).ok()
        } else {
            Frac::new(s, ten_pow((-d.e) as u64)).ok()
        }
    }
    fn cmp(&self, o: &Frac) -> std::cmp::Ordering {
        self.n.mul(&o.d).cmp(&o.n.mul(&self.d))
    }
    fn to_f64(&self) -> f64 {
        match binop(&self.n.clone().norm(), &self.d.clone().norm(), "/") {
            Ok(V::Float(f)) => f,
            _ => self.n.to_f64() / self.d.to_f64(),
        }
    }
    fn floor(&self) -> Big {
        self.n.divmod(&self.d).0
    }
    fn to_s(&self) -> String {
        if self.d.cmp(&bigi(1)) == std::cmp::Ordering::Equal {
            self.n.to_string()
        } else {
            format!("{}/{}", self.n.to_string(), self.d.to_string())
        }
    }
}
pub struct FracV(pub Frac);
fn fracv(f: Frac) -> V {
    V::Ext(ExtRc::new(FracV(f)))
}
impl Ext for FracV {
    fn tname(&self) -> &'static str {
        "fractions.Fraction"
    }
    fn repr(&self) -> String {
        format!("Fraction({}, {})", self.0.n.to_string(), self.0.d.to_string())
    }
    fn str_(&self) -> String {
        self.0.to_s()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        const P: i128 = (1i128 << 61) - 1;
        if self.0.d.cmp(&bigi(1)) == std::cmp::Ordering::Equal {
            return Some(hash_big(&self.0.n));
        }
        let modp = |b: &Big| -> i128 { b.divmod(&Big::from_i128(P)).1.to_i128().unwrap_or(0) };
        let powm = |mut b: i128, mut e: i128| -> i128 {
            let mut r = 1i128;
            b %= P;
            while e > 0 {
                if e & 1 == 1 {
                    r = r * b % P;
                }
                b = b * b % P;
                e >>= 1;
            }
            r
        };
        let dinv = powm(modp(&self.0.d), P - 2);
        let an = if self.0.n.neg { self.0.n.neg() } else { self.0.n.clone() };
        let h = modp(&an) * dinv % P;
        let h = if self.0.n.neg { -h } else { h } as i64;
        Some(if h == -1 { -2 } else { h })
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if let V::Float(f) = o {
            return Some(Frac::of_f64(*f).is_some_and(|x| x.cmp(&self.0) == std::cmp::Ordering::Equal));
        }
        Frac::of(o).map(|x| x.cmp(&self.0) == std::cmp::Ordering::Equal)
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        if let V::Float(f) = o {
            return Frac::of_f64(*f).map(|x| self.0.cmp(&x));
        }
        Frac::of(o).map(|x| self.0.cmp(&x))
    }
    fn truth(&self) -> bool {
        !self.0.n.is_zero()
    }
    fn to_f64(&self) -> Option<f64> {
        Some(self.0.to_f64())
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.0.to_s())));
        }
        Some(format_spec(&V::Float(self.0.to_f64()), spec).map(st))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        let a0 = &self.0;
        if op == "neg" {
            return Some(Ok(fracv(Frac { n: a0.n.neg(), d: a0.d.clone() })));
        }
        if op == "pos" {
            return Some(Ok(fracv(a0.clone())));
        }
        if let V::Float(f) = o {
            let x = a0.to_f64();
            return Some(if right { binop(&V::Float(*f), &V::Float(x), op) } else { binop(&V::Float(x), &V::Float(*f), op) });
        }
        let other = Frac::of(o)?;
        let (a, b) = if right { (other, a0.clone()) } else { (a0.clone(), other) };
        Some((|| -> R {
            Ok(match op {
                "+" => fracv(Frac::new(a.n.mul(&b.d).add(&b.n.mul(&a.d)), a.d.mul(&b.d))?),
                "-" => fracv(Frac::new(a.n.mul(&b.d).sub(&b.n.mul(&a.d)), a.d.mul(&b.d))?),
                "*" => fracv(Frac::new(a.n.mul(&b.n), a.d.mul(&b.d))?),
                "/" => {
                    if b.n.is_zero() {
                        return Err(exc("ZeroDivisionError", format!("Fraction({}, 0)", a.n.mul(&b.d).to_string())));
                    }
                    fracv(Frac::new(a.n.mul(&b.d), a.d.mul(&b.n))?)
                }
                "//" => {
                    if b.n.is_zero() {
                        return Err(exc("ZeroDivisionError", "division by zero".into()));
                    }
                    a.n.mul(&b.d).divmod(&a.d.mul(&b.n)).0.norm()
                }
                "%" => {
                    if b.n.is_zero() {
                        return Err(exc("ZeroDivisionError", "division by zero".into()));
                    }
                    let (x, y) = (a.n.mul(&b.d), b.n.mul(&a.d));
                    fracv(Frac::new(x.divmod(&y).1, a.d.mul(&b.d))?)
                }
                "**" => {
                    if b.d.cmp(&bigi(1)) != std::cmp::Ordering::Equal {
                        return Ok(V::Float(a.to_f64().powf(b.to_f64())));
                    }
                    let k = b.n.to_i64().unwrap_or(0);
                    let (n, d) = (a.n.pow(k.unsigned_abs()), a.d.pow(k.unsigned_abs()));
                    fracv(if k >= 0 { Frac::new(n, d)? } else { Frac::new(d, n)? })
                }
                _ => return Err(exc("TypeError", format!("unsupported operand type(s) for {op}: 'Fraction' and '{}'", tn(o)))),
            })
        })())
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "numerator" => self.0.n.clone().norm(),
            "denominator" => self.0.d.clone().norm(),
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let f = &self.0;
        Some((|| -> R {
            Ok(match n {
                "__int__" | "__trunc__" => {
                    let q = f.n.divmod(&f.d);
                    let t = if f.n.neg && !q.1.is_zero() { q.0.add(&bigi(1)) } else { q.0 };
                    t.norm()
                }
                "__floor__" => f.floor().norm(),
                "__ceil__" => f.n.neg().divmod(&f.d).0.neg().norm(),
                "__float__" => V::Float(f.to_f64()),
                "__abs__" => fracv(Frac { n: if f.n.neg { f.n.neg() } else { f.n.clone() }, d: f.d.clone() }),
                "__round__" => {
                    let nd = a.p.first().cloned().unwrap_or(V::None);
                    let k = match nd {
                        V::None => None,
                        V::Int(k) => Some(k),
                        _ => return type_err("integer argument expected".into()),
                    };
                    let scale = ten_pow(k.unwrap_or(0).unsigned_abs());
                    let (num, den) = if k.unwrap_or(0) >= 0 { (f.n.mul(&scale), f.d.clone()) } else { (f.n.clone(), f.d.mul(&scale)) };
                    let (q, r) = num.divmod(&den);
                    let twice = r.add(&r).cmp(&den);
                    let q = if twice == std::cmp::Ordering::Greater || (twice == std::cmp::Ordering::Equal && big_is_odd(&q)) { q.add(&bigi(1)) } else { q };
                    match k {
                        None => q.norm(),
                        Some(k) if k >= 0 => fracv(Frac::new(q, scale)?),
                        Some(_) => fracv(Frac::new(q.mul(&scale), bigi(1))?),
                    }
                }
                "limit_denominator" => {
                    let maxd = big_of(&a.p.first().cloned().unwrap_or(V::Int(1000000))).unwrap_or(bigi(1000000));
                    if f.d.cmp(&maxd) != std::cmp::Ordering::Greater {
                        return Ok(fracv(f.clone()));
                    }
                    let (mut p0, mut q0, mut p1, mut q1) = (bigi(0), bigi(1), bigi(1), bigi(0));
                    let (mut n, mut d) = (f.n.clone(), f.d.clone());
                    loop {
                        let a = n.divmod(&d).0;
                        let q2 = q0.add(&a.mul(&q1));
                        if q2.cmp(&maxd) == std::cmp::Ordering::Greater {
                            break;
                        }
                        let np = p0.add(&a.mul(&p1));
                        p0 = p1;
                        q0 = q1;
                        p1 = np;
                        q1 = q2;
                        let nd = n.sub(&a.mul(&d));
                        n = d;
                        d = nd;
                        if d.is_zero() {
                            break;
                        }
                    }
                    let k = maxd.sub(&q0).divmod(&q1).0;
                    let b1 = Frac::new(p0.add(&k.mul(&p1)), q0.add(&k.mul(&q1)))?;
                    let b2 = Frac::new(p1.clone(), q1.clone())?;
                    let dist = |x: &Frac| {
                        let df = Frac::new(x.n.mul(&f.d).sub(&f.n.mul(&x.d)), x.d.mul(&f.d)).ok().unwrap();
                        Frac { n: if df.n.neg { df.n.neg() } else { df.n }, d: df.d }
                    };
                    fracv(if dist(&b2).cmp(&dist(&b1)) != std::cmp::Ordering::Greater { b2 } else { b1 })
                }
                "as_integer_ratio" => tup(vec![f.n.clone().norm(), f.d.clone().norm()]),
                "is_integer" => V::Bool(f.d.cmp(&bigi(1)) == std::cmp::Ordering::Equal),
                _ => return Err(exc("AttributeError", format!("'Fraction' object has no attribute '{n}'"))),
            })
        })())
    }
}
fn fraction_new(a: Args) -> R {
    let x = a.p.first().cloned().unwrap_or(V::Int(0));
    if let Some(y) = a.p.get(1) {
        let (Some(p), Some(q)) = (Frac::of(&x), Frac::of(y)) else { return type_err("both arguments should be Rational instances".into()) };
        if q.n.is_zero() {
            return Err(exc("ZeroDivisionError", format!("Fraction({}, 0)", p.n.to_string())));
        }
        return Ok(fracv(Frac::new(p.n.mul(&q.d), p.d.mul(&q.n))?));
    }
    let f = match &x {
        V::Str(t) => {
            let t = t.s.trim();
            let bad = || exc("ValueError", format!("Invalid literal for Fraction: {}", str_repr(t)));
            if let Some((p, q)) = t.split_once('/') {
                let p = Big::parse(p.trim(), 10).ok_or_else(bad)?;
                let q = Big::parse(q.trim(), 10).ok_or_else(bad)?;
                Frac::new(p, q)?
            } else {
                Frac::of_dec(&Dec::parse(t).filter(|d| d.sp == 0).ok_or_else(bad)?).ok_or_else(bad)?
            }
        }
        V::Float(f) => Frac::of_f64(*f).ok_or_else(|| exc("ValueError", format!("cannot convert {} to integer ratio", float_repr(*f))))?,
        V::Ext(e) if e.any().downcast_ref::<DecV>().is_some() => Frac::of_dec(&e.any().downcast_ref::<DecV>().unwrap().0).ok_or_else(|| exc("ValueError", "cannot convert to Fraction".into()))?,
        _ => Frac::of(&x).ok_or_else(|| exc("TypeError", "argument should be a string or a Rational instance".into()))?,
    };
    Ok(fracv(f))
}

// ---------- datetime ----------

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
fn dim(y: i64, m: i64) -> i64 {
    [31, if is_leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize]
}
fn days_before_year(y: i64) -> i64 {
    let y = y - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}
fn days_before_month(y: i64, m: i64) -> i64 {
    (1..m).map(|k| dim(y, k)).sum()
}
fn ymd_to_ord(y: i64, m: i64, d: i64) -> i64 {
    days_before_year(y) + days_before_month(y, m) + d
}
fn ord_to_ymd(n: i64) -> (i64, i64, i64) {
    let mut y = (n as f64 / 365.2425) as i64 + 1;
    while days_before_year(y) >= n {
        y -= 1;
    }
    while days_before_year(y + 1) < n {
        y += 1;
    }
    let mut rem = n - days_before_year(y);
    let mut m = 1;
    while rem > dim(y, m) {
        rem -= dim(y, m);
        m += 1;
    }
    (y, m, rem)
}
fn check_date(y: i64, m: i64, d: i64) -> Result<(), E> {
    if !(1..=9999).contains(&y) {
        return val_err(format!("year {y} is out of range"));
    }
    if !(1..=12).contains(&m) {
        return val_err("month must be in 1..12".into());
    }
    if d < 1 || d > dim(y, m) {
        return val_err(format!("day {d} must be in range 1..{} for month {m} in year {y}", dim(y, m)));
    }
    Ok(())
}
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

// A date or datetime: days since 0001-01-01 (ordinal) plus microseconds into the day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DT {
    ord: i64,
    us: i64,
    has_time: bool,
    // An aware datetime's tzinfo: a fixed UTC offset in seconds (datetime.timezone) or a zoneinfo.ZoneInfo;
    // None for a naive one or a date.
    tz: Option<Tz>,
    // PEP 495's fold: the second of two wall times a clock change repeats
    fold: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tz {
    F(i32),
    Z(u32),
}
impl DT {
    fn ymd(&self) -> (i64, i64, i64) {
        ord_to_ymd(self.ord)
    }
    fn hms(&self) -> (i64, i64, i64, i64) {
        let s = self.us / 1_000_000;
        (s / 3600, s / 60 % 60, s % 60, self.us % 1_000_000)
    }
    fn weekday(&self) -> i64 {
        (self.ord + 6) % 7
    }
    fn iso(&self, sep: char) -> String {
        self.iso_spec(sep, "auto").unwrap_or_default()
    }
    // isoformat(sep, timespec): the time part cut to hours, minutes, seconds, milliseconds or microseconds.
    fn iso_spec(&self, sep: char, spec: &str) -> Result<String, E> {
        let (y, m, d) = self.ymd();
        let mut t = format!("{y:04}-{m:02}-{d:02}");
        if self.has_time {
            let (h, mi, s, us) = self.hms();
            t.push(sep);
            t.push_str(&time_iso(h, mi, s, us, spec)?);
            if let Some(o) = self.off() {
                t.push_str(&off_str(o, true));
            }
        }
        Ok(t)
    }
    fn isocal(&self) -> (i64, i64, i64) {
        let (y, _, _) = self.ymd();
        let week1 = |y: i64| {
            let first = ymd_to_ord(y, 1, 1);
            let wd = (first + 6) % 7;
            let mut monday = first - wd;
            if wd > 3 {
                monday += 7;
            }
            monday
        };
        let mut yy = y;
        let mut start = week1(yy);
        if self.ord < start {
            yy -= 1;
            start = week1(yy);
        } else if self.ord >= week1(yy + 1) {
            yy += 1;
            start = week1(yy);
        }
        (yy, (self.ord - start) / 7 + 1, self.weekday() + 1)
    }
    fn strftime(&self, f: &str) -> String {
        let (y, m, d) = self.ymd();
        let (h, mi, s, us) = self.hms();
        let mut out = String::new();
        let mut it = f.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let Some(k) = it.next() else {
                out.push('%');
                break;
            };
            match k {
                'Y' => {
                    let _ = write!(out, "{y:04}");
                }
                'y' => {
                    let _ = write!(out, "{:02}", y % 100);
                }
                'm' => {
                    let _ = write!(out, "{m:02}");
                }
                'd' => {
                    let _ = write!(out, "{d:02}");
                }
                'e' => {
                    let _ = write!(out, "{d:>2}");
                }
                'H' => {
                    let _ = write!(out, "{h:02}");
                }
                'I' => {
                    let _ = write!(out, "{:02}", if h % 12 == 0 { 12 } else { h % 12 });
                }
                'p' => out.push_str(if h < 12 { "AM" } else { "PM" }),
                'M' => {
                    let _ = write!(out, "{mi:02}");
                }
                'S' => {
                    let _ = write!(out, "{s:02}");
                }
                'f' => {
                    let _ = write!(out, "{us:06}");
                }
                'A' => out.push_str(DAYS[self.weekday() as usize]),
                'a' => out.push_str(&DAYS[self.weekday() as usize][..3]),
                'B' => out.push_str(MONTHS[(m - 1) as usize]),
                'b' | 'h' => out.push_str(&MONTHS[(m - 1) as usize][..3]),
                'j' => {
                    let _ = write!(out, "{:03}", days_before_month(y, m) + d);
                }
                'w' => {
                    let _ = write!(out, "{}", (self.weekday() + 1) % 7);
                }
                'u' => {
                    let _ = write!(out, "{}", self.weekday() + 1);
                }
                'U' | 'W' => {
                    let yday = days_before_month(y, m) + d - 1;
                    let wd = if k == 'U' { (self.weekday() + 1) % 7 } else { self.weekday() };
                    let _ = write!(out, "{:02}", (yday + 7 - wd) / 7);
                }
                'V' => {
                    let _ = write!(out, "{:02}", self.isocal().1);
                }
                'G' => {
                    let _ = write!(out, "{:04}", self.isocal().0);
                }
                'D' => {
                    let _ = write!(out, "{m:02}/{d:02}/{:02}", y % 100);
                }
                'F' => {
                    let _ = write!(out, "{y:04}-{m:02}-{d:02}");
                }
                'T' => {
                    let _ = write!(out, "{h:02}:{mi:02}:{s:02}");
                }
                'R' => {
                    let _ = write!(out, "{h:02}:{mi:02}");
                }
                'c' => {
                    let _ = write!(out, "{} {} {:>2} {h:02}:{mi:02}:{s:02} {y}", &DAYS[self.weekday() as usize][..3], &MONTHS[(m - 1) as usize][..3], d);
                }
                'x' => {
                    let _ = write!(out, "{m:02}/{d:02}/{:02}", y % 100);
                }
                'X' => {
                    let _ = write!(out, "{h:02}:{mi:02}:{s:02}");
                }
                'z' | 'Z' => out.push_str(&self.tz_fmt(k)),
                '%' => out.push('%'),
                k => {
                    out.push('%');
                    out.push(k);
                }
            }
        }
        out
    }
}
// A time of day as isoformat writes it for a timespec.
fn time_iso(h: i64, mi: i64, s: i64, us: i64, spec: &str) -> Result<String, E> {
    Ok(match spec {
        "auto" if us != 0 => format!("{h:02}:{mi:02}:{s:02}.{us:06}"),
        "auto" | "seconds" => format!("{h:02}:{mi:02}:{s:02}"),
        "hours" => format!("{h:02}"),
        "minutes" => format!("{h:02}:{mi:02}"),
        "milliseconds" => format!("{h:02}:{mi:02}:{s:02}.{:03}", us / 1000),
        "microseconds" => format!("{h:02}:{mi:02}:{s:02}.{us:06}"),
        _ => return val_err("Unknown timespec value".into()),
    })
}
fn timespec_arg(v: Option<V>) -> Result<String, E> {
    match v {
        None => Ok("auto".into()),
        Some(V::Str(x)) => Ok(x.s.to_string()),
        Some(x) => type_err(format!("isoformat() argument 'timespec' must be str, not {}", tn(&x))),
    }
}
// Python's datetime run by CPython (pyglue.rs sets it): for a tzinfo Brevi lacks, such as zoneinfo.ZoneInfo.
thread_local! {
    pub static PY_DT: Cell<Option<fn(&str, &str, Args) -> R>> = Cell::new(None);
}
fn has_py(a: &Args) -> bool {
    a.p.iter().chain(a.k.iter().map(|(_, v)| v)).any(|v| matches!(v, V::Ext(x) if x.is_py()))
}
// datetime.<cls>.<meth>(*a) by CPython (meth "" calls the class).
fn py_dt(cls: &str, meth: &str, a: Args) -> R {
    match PY_DT.with(|h| h.get()) {
        Some(f) => f(cls, meth, a),
        None => type_err("tzinfo argument must be None or of a tzinfo subclass".into()),
    }
}
fn py_dt_self(me: &V, cls: &str, meth: &str, a: &mut Args) -> R {
    let mut p = vec![me.clone()];
    p.append(&mut a.p);
    py_dt(cls, meth, Args { p, k: std::mem::take(&mut a.k) })
}
const DAY_US: i128 = 86_400_000_000;
// A UTC offset as datetime prints it: +HH:MM (isoformat, str, tzname) or +HHMM (%z), with seconds when it has them.
fn off_str(off: i32, colon: bool) -> String {
    let sign = if off < 0 { '-' } else { '+' };
    let a = off.unsigned_abs();
    let (h, m, sec) = (a / 3600, a / 60 % 60, a % 60);
    let c = if colon { ":" } else { "" };
    let mut t = format!("{sign}{h:02}{c}{m:02}");
    if sec != 0 {
        let _ = write!(t, "{c}{sec:02}");
    }
    t
}
fn tz_name(off: i32) -> String {
    if off == 0 {
        "UTC".into()
    } else {
        format!("UTC{}", off_str(off, true))
    }
}
impl DT {
    // Microseconds since 0001-01-01 00:00 UTC for an aware datetime; the wall clock's for a naive one.
    fn utc_us(&self) -> i128 {
        self.ord as i128 * DAY_US + self.us as i128 - self.off().unwrap_or(0) as i128 * 1_000_000
    }
    // The tzinfo's local time type at this wall clock: utcoffset, dst and tzname (Python's for a zone).
    fn tti(&self) -> Option<Tti> {
        match self.tz? {
            Tz::F(o) => Some(Tti { off: o, dst: 0, name: Rc::from(tz_name(o).as_str()) }),
            Tz::Z(z) => Some(zone(z).find(self.local_secs(), self.ymd().0, self.fold)),
        }
    }
    // utcoffset() in seconds
    fn off(&self) -> Option<i32> {
        match self.tz? {
            Tz::F(o) => Some(o),
            Tz::Z(_) => self.tti().map(|t| t.off),
        }
    }
    // Seconds since 1970 on the wall clock, as zoneinfo counts them (microseconds dropped).
    fn local_secs(&self) -> i64 {
        (self.ord - 719163) * 86400 + self.us.div_euclid(1_000_000)
    }
    // A wall time whose offset depends on its fold (in a gap or a repeat): PEP 495's exception to ==.
    fn fold_sensitive(&self) -> bool {
        matches!(self.tz, Some(Tz::Z(_))) && {
            let mut o = *self;
            o.fold = !o.fold;
            o.off() != self.off()
        }
    }
    fn from_utc_us(t: i128, tz: Option<Tz>) -> Result<DT, E> {
        let (off, fold) = match tz {
            Some(Tz::Z(z)) => {
                let utc_ord = t.div_euclid(DAY_US) as i64;
                let ts = t.div_euclid(1_000_000) as i64 - 719163 * 86400;
                let (tti, fold) = zone(z).fromutc(ts, ord_to_ymd(utc_ord.clamp(1, 3652059)).0);
                (tti.off, fold)
            }
            Some(Tz::F(o)) => (o, false),
            None => (0, false),
        };
        let w = t + off as i128 * 1_000_000;
        let ord = w.div_euclid(DAY_US) as i64;
        if !(1..=3652059).contains(&ord) {
            return Err(exc("OverflowError", "date value out of range".into()));
        }
        Ok(DT { ord, us: w.rem_euclid(DAY_US) as i64, has_time: true, tz, fold })
    }
    // %z and %Z (empty for a naive datetime, as in Python)
    fn tz_fmt(&self, k: char) -> String {
        match (self.tti(), k) {
            (Some(t), 'z') => off_str(t.off, false),
            (Some(t), _) => t.name.to_string(),
            (None, _) => String::new(),
        }
    }
}
const EPOCH_US: i128 = 719163 * DAY_US;
fn now_us() -> i128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_micros() as i128).unwrap_or(0)
}
// A timestamp as Python rounds it to microseconds (half to even).
fn ts_us(v: &V) -> Result<i128, E> {
    match v {
        V::Int(i) => Ok(*i as i128 * 1_000_000),
        V::Bool(b) => Ok(*b as i128 * 1_000_000),
        V::Float(f) if f.is_finite() => Ok((f * 1e6).round_ties_even() as i128),
        V::Float(_) => val_err("cannot convert float NaN or infinity to integer".into()),
        x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
    }
}

// datetime.timezone: a fixed UTC offset in seconds.
pub struct TZV(pub i32);
fn tzv(off: i32) -> V {
    V::Ext(ExtRc::new(TZV(off)))
}
fn tz_of(v: &V) -> Option<i32> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<TZV>().map(|t| t.0),
        _ => None,
    }
}
fn tzinfo_v(t: Tz) -> V {
    match t {
        Tz::F(o) => tzv(o),
        Tz::Z(z) => ZONES.with(|r| r.borrow().vals[z as usize].clone()),
    }
}
fn td_secs(s: i64) -> V {
    tdv(TD { us: s as i128 * 1_000_000 })
}
// ZoneInfo.utcoffset(dt) and friends: the zone's local time type at dt's wall clock and fold
fn dt_zone_tti(v: &V, z: u32) -> Option<Tti> {
    let d = dt_of(v).filter(|d| d.has_time)?;
    Some(zone(z).find(d.local_secs(), d.ymd().0, d.fold))
}
// ZoneInfo.fromutc(dt): dt (a UTC wall time carrying this zone) as the zone's wall time
fn dt_fromutc(me: &V, v: &V) -> R {
    let Some(d) = dt_of(v).filter(|d| d.has_time) else { return type_err("fromutc() requires a datetime argument".into()) };
    if d.tz != zi_of(me).map(Tz::Z) {
        return val_err("dt.tzinfo is not self".into());
    }
    let mut n = d;
    n.tz = None;
    Ok(dtv(DT::from_utc_us(n.utc_us(), d.tz)?))
}
// A tzinfo argument: None (naive) or a datetime.timezone.
fn tz_arg(v: Option<V>) -> Result<Option<Tz>, E> {
    match v {
        None | Some(V::None) => Ok(None),
        Some(v) => match tz_of(&v).map(Tz::F).or_else(|| zi_of(&v).map(Tz::Z)) {
            Some(o) => Ok(Some(o)),
            None => type_err(format!("tzinfo argument must be None or of a tzinfo subclass, not type '{}'", tn(&v))),
        },
    }
}
fn tz_new(mut a: Args) -> R {
    let off = a.kw("offset").or_else(|| a.p.first().cloned());
    if a.kw("name").is_some() || a.p.len() > 1 {
        return Err(unsupported("datetime.timezone with a name"));
    }
    let Some(t) = off.as_ref().and_then(td_of) else {
        return type_err("timezone() argument 1 must be datetime.timedelta".into());
    };
    if t.us <= -DAY_US || t.us >= DAY_US {
        return val_err("offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24).".into());
    }
    if t.us % 1_000_000 != 0 {
        return Err(unsupported("datetime.timezone with a fraction of a second"));
    }
    Ok(tzv((t.us / 1_000_000) as i32))
}
impl Ext for TZV {
    fn tname(&self) -> &'static str {
        "datetime.timezone"
    }
    fn repr(&self) -> String {
        if self.0 == 0 {
            "datetime.timezone.utc".into()
        } else {
            format!("datetime.timezone({})", TDV(TD { us: self.0 as i128 * 1_000_000 }).repr())
        }
    }
    fn str_(&self) -> String {
        tz_name(self.0)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(hash_int(self.0 as i64))
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(tz_of(o) == Some(self.0))
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(Ok(match n {
            "utcoffset" => tdv(TD { us: self.0 as i128 * 1_000_000 }),
            "tzname" => st(tz_name(self.0)),
            "dst" => V::None,
            _ => return Some(Err(exc("AttributeError", format!("'datetime.timezone' object has no attribute '{n}'")))),
        }))
    }
}
fn strptime(t: &str, f: &str) -> Result<DT, E> {
    let fail = || exc("ValueError", format!("time data {} does not match format {}", str_repr(t), str_repr(f)));
    let tc: Vec<char> = t.chars().collect();
    let mut i = 0;
    let (mut y, mut mo, mut d, mut h, mut mi, mut s, mut us) = (1900, 1, 1, 0, 0, 0, 0);
    let mut pm: Option<bool> = None;
    let mut it = f.chars().peekable();
    let num = |i: &mut usize, max: usize| -> Option<i64> {
        let st = *i;
        while *i < tc.len() && *i - st < max && tc[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == st {
            None
        } else {
            tc[st..*i].iter().collect::<String>().parse().ok()
        }
    };
    let word = |i: &mut usize, names: &[&str], short: bool| -> Option<i64> {
        for (k, n) in names.iter().enumerate() {
            let w: &str = if short { &n[..3] } else { n };
            let wl = w.chars().count();
            if *i + wl <= tc.len() && tc[*i..*i + wl].iter().collect::<String>().to_lowercase() == w.to_lowercase() {
                *i += wl;
                return Some(k as i64);
            }
        }
        None
    };
    while let Some(c) = it.next() {
        if c == '%' {
            let k = it.next().ok_or_else(fail)?;
            match k {
                'Y' => y = num(&mut i, 4).ok_or_else(fail)?,
                'y' => {
                    let v = num(&mut i, 2).ok_or_else(fail)?;
                    y = if v < 69 { 2000 + v } else { 1900 + v };
                }
                'm' => mo = num(&mut i, 2).ok_or_else(fail)?,
                'd' => d = num(&mut i, 2).ok_or_else(fail)?,
                'H' => h = num(&mut i, 2).ok_or_else(fail)?,
                'I' => h = num(&mut i, 2).ok_or_else(fail)?,
                'M' => mi = num(&mut i, 2).ok_or_else(fail)?,
                'S' => s = num(&mut i, 2).ok_or_else(fail)?,
                'f' => {
                    let st = i;
                    let v = num(&mut i, 6).ok_or_else(fail)?;
                    us = v * 10i64.pow((6 - (i - st)) as u32);
                }
                'j' => {
                    let v = num(&mut i, 3).ok_or_else(fail)?;
                    let (yy, mm, dd) = ord_to_ymd(ymd_to_ord(y, 1, 1) + v - 1);
                    y = yy;
                    mo = mm;
                    d = dd;
                }
                'p' => {
                    let w: String = tc.get(i..i + 2).map(|x| x.iter().collect()).unwrap_or_default();
                    match w.to_uppercase().as_str() {
                        "AM" => pm = Some(false),
                        "PM" => pm = Some(true),
                        _ => return Err(fail()),
                    }
                    i += 2;
                }
                'B' => mo = word(&mut i, &MONTHS, false).ok_or_else(fail)? + 1,
                'b' => mo = word(&mut i, &MONTHS, true).ok_or_else(fail)? + 1,
                'A' => {
                    word(&mut i, &DAYS, false).ok_or_else(fail)?;
                }
                'a' => {
                    word(&mut i, &DAYS, true).ok_or_else(fail)?;
                }
                '%' => {
                    if tc.get(i) != Some(&'%') {
                        return Err(fail());
                    }
                    i += 1;
                }
                _ => return Err(unsupported(&format!("strptime %{k}"))),
            }
        } else if c.is_whitespace() {
            while i < tc.len() && tc[i].is_whitespace() {
                i += 1;
            }
        } else {
            if tc.get(i) != Some(&c) {
                return Err(fail());
            }
            i += 1;
        }
    }
    if i != tc.len() {
        return val_err(format!("unconverted data remains: {}", tc[i..].iter().collect::<String>()));
    }
    if let Some(p) = pm {
        if p && h < 12 {
            h += 12;
        }
        if !p && h == 12 {
            h = 0;
        }
    }
    check_date(y, mo, d)?;
    if h > 23 || mi > 59 || s > 61 {
        return Err(fail());
    }
    Ok(DT { ord: ymd_to_ord(y, mo, d), us: ((h * 60 + mi) * 60 + s) * 1_000_000 + us, has_time: true, tz: None, fold: false })
}
fn fromiso(t: &str, with_time: bool) -> Result<DT, E> {
    let bad = || exc("ValueError", format!("Invalid isoformat string: {}", str_repr(t)));
    let b = t.as_bytes();
    let n2 = |s: &[u8]| -> Option<i64> {
        if s.iter().all(|c| c.is_ascii_digit()) && !s.is_empty() {
            std::str::from_utf8(s).ok()?.parse().ok()
        } else {
            None
        }
    };
    let (date_part, rest) = if b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        (Some((n2(&b[0..4]), n2(&b[5..7]), n2(&b[8..10]))), &t[10..])
    } else if b.len() >= 8 && b[..8].iter().all(|c| c.is_ascii_digit()) {
        (Some((n2(&b[0..4]), n2(&b[4..6]), n2(&b[6..8]))), &t[8..])
    } else {
        (None, t)
    };
    let Some((Some(y), Some(m), Some(d))) = date_part else { return Err(bad()) };
    check_date(y, m, d).map_err(|_| bad())?;
    let mut us = 0;
    let mut tz = None;
    if !rest.is_empty() {
        if !with_time {
            return Err(bad());
        }
        let r = rest.as_bytes();
        if r[0] != b'T' && r[0] != b' ' {
            return Err(bad());
        }
        let mut tp = &rest[1..];
        // A UTC offset at the end (Z, +HH, +HH:MM, +HHMM, +HH:MM:SS) makes the datetime aware.
        if let Some(x) = tp.strip_suffix('Z') {
            tp = x;
            tz = Some(Tz::F(0));
        } else if let Some(k) = tp.find(['+', '-']) {
            let (body, o) = tp.split_at(k);
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let o = &o[1..];
            let ps: Vec<&str> = if o.contains(':') { o.split(':').collect() } else { o.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).unwrap_or("")).collect() };
            if ps.is_empty() || ps.len() > 3 || ps.iter().any(|p| p.len() != 2) {
                return Err(bad());
            }
            let v: Vec<i64> = ps.iter().map(|p| n2(p.as_bytes()).ok_or_else(bad)).collect::<Result<_, _>>()?;
            let secs = v[0] * 3600 + v.get(1).copied().unwrap_or(0) * 60 + v.get(2).copied().unwrap_or(0);
            if v[0] > 23 || v.get(1).copied().unwrap_or(0) > 59 || v.get(2).copied().unwrap_or(0) > 59 {
                return Err(bad());
            }
            tp = body;
            tz = Some(Tz::F((sign * secs) as i32));
        }
        // The basic format: HHMM or HHMMSS[.ffffff].
        let (head, tail) = tp.split_once(['.', ',']).map_or((tp, ""), |(h, _)| (h, &tp[h.len()..]));
        let tp = if !head.contains(':') && (head.len() == 4 || head.len() == 6) && head.bytes().all(|c| c.is_ascii_digit()) {
            head.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join(":") + tail
        } else {
            tp.to_string()
        }
        .replace(',', ".");
        let parts: Vec<&str> = tp.split(':').collect();
        if parts.len() > 3 || parts.iter().any(|x| x.split('.').next().map_or(true, |w| w.len() != 2)) {
            return Err(bad());
        }
        let h = parts.first().and_then(|x| n2(x.as_bytes())).ok_or_else(bad)?;
        let mi = parts.get(1).map(|x| n2(x.as_bytes()).ok_or_else(bad)).transpose()?.unwrap_or(0);
        let (s, frac) = match parts.get(2) {
            Some(x) => match x.split_once('.') {
                Some((a, f)) => (n2(a.as_bytes()).ok_or_else(bad)?, {
                    let f6 = format!("{:0<6}", &f[..f.len().min(6)]);
                    f6.parse::<i64>().map_err(|_| bad())?
                }),
                None => (n2(x.as_bytes()).ok_or_else(bad)?, 0),
            },
            None => (0, 0),
        };
        if h > 23 || mi > 59 || s > 59 {
            return Err(bad());
        }
        us = ((h * 60 + mi) * 60 + s) * 1_000_000 + frac;
    }
    Ok(DT { ord: ymd_to_ord(y, m, d), us, has_time: with_time, tz, fold: false })
}

pub struct DTV(pub DT);
fn dtv(d: DT) -> V {
    V::Ext(ExtRc::new(DTV(d)))
}
#[derive(Clone, Copy, Debug)]
pub struct TD {
    us: i128,
}
impl TD {
    fn parts(&self) -> (i64, i64, i64) {
        let day = 86_400_000_000i128;
        let days = self.us.div_euclid(day);
        let rem = self.us.rem_euclid(day);
        (days as i64, (rem / 1_000_000) as i64, (rem % 1_000_000) as i64)
    }
    fn to_s(&self) -> String {
        let (d, s, us) = self.parts();
        let mut t = String::new();
        if d != 0 {
            let _ = write!(t, "{d} day{}, ", if d.abs() != 1 { "s" } else { "" });
        }
        let _ = write!(t, "{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60);
        if us != 0 {
            let _ = write!(t, ".{us:06}");
        }
        t
    }
}
pub struct TDV(pub TD);
fn tdv(t: TD) -> V {
    V::Ext(ExtRc::new(TDV(t)))
}
fn td_of(v: &V) -> Option<TD> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<TDV>().map(|t| t.0),
        _ => None,
    }
}
fn dt_of(v: &V) -> Option<DT> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<DTV>().map(|t| t.0),
        _ => None,
    }
}

// datetime.time: a time of day, kept as microseconds since midnight.
pub struct TMV(pub i64);
fn tmv(us: i64) -> V {
    V::Ext(ExtRc::new(TMV(us)))
}
fn tm_of(v: &V) -> Option<i64> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<TMV>().map(|t| t.0),
        _ => None,
    }
}
fn tm_dt(us: i64) -> DT {
    DT { ord: ymd_to_ord(1900, 1, 1), us, has_time: true, tz: None, fold: false }
}
fn tm_new(mut a: Args) -> R {
    if has_py(&a) {
        return py_dt("time", "", a);
    }
    // A time carrying a UTC offset is not native: refuse rather than drop the offset.
    let _ = a.kw("fold");
    match a.kw("tzinfo").or_else(|| a.p.get(4).cloned()) {
        None | Some(V::None) => {}
        Some(_) => return Err(unsupported("datetime.time with a tzinfo")),
    }
    let v = int_args(&mut a, &["hour", "minute", "second", "microsecond"], 0, "time")?;
    let get = |k: usize| v.get(k).copied().unwrap_or(0);
    let (h, m, s, us) = (get(0), get(1), get(2), get(3));
    if !(0..24).contains(&h) {
        return val_err("hour must be in 0..23".into());
    }
    if !(0..60).contains(&m) {
        return val_err("minute must be in 0..59".into());
    }
    if !(0..60).contains(&s) {
        return val_err("second must be in 0..59".into());
    }
    if !(0..1_000_000).contains(&us) {
        return val_err("microsecond must be in 0..999999".into());
    }
    Ok(tmv(((h * 60 + m) * 60 + s) * 1_000_000 + us))
}
impl Ext for TMV {
    fn tname(&self) -> &'static str {
        "datetime.time"
    }
    fn repr(&self) -> String {
        let (h, mi, s, us) = tm_dt(self.0).hms();
        let mut t = format!("datetime.time({h}, {mi}");
        if s != 0 || us != 0 {
            let _ = write!(t, ", {s}");
        }
        if us != 0 {
            let _ = write!(t, ", {us}");
        }
        t.push(')');
        t
    }
    fn str_(&self) -> String {
        let (h, mi, s, us) = tm_dt(self.0).hms();
        if us != 0 {
            format!("{h:02}:{mi:02}:{s:02}.{us:06}")
        } else {
            format!("{h:02}:{mi:02}:{s:02}")
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(hash_int(self.0))
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(tm_of(o) == Some(self.0))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        tm_of(o).map(|t| self.0.cmp(&t))
    }
    fn truth(&self) -> bool {
        true
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.str_())));
        }
        Some(Ok(st(tm_dt(self.0).strftime(spec))))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let (h, mi, s, us) = tm_dt(self.0).hms();
        Some(Ok(match n {
            "hour" => V::Int(h),
            "minute" => V::Int(mi),
            "second" => V::Int(s),
            "microsecond" => V::Int(us),
            "tzinfo" => V::None,
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let us = self.0;
        Some((|| -> R {
            Ok(match n {
                "isoformat" => {
                    let (h, mi, s, u) = tm_dt(us).hms();
                    st(time_iso(h, mi, s, u, &timespec_arg(a.kw("timespec").or_else(|| a.p.first().cloned()))?)?)
                }
                "strftime" | "__format__" => st(tm_dt(us).strftime(&to_str(&a.p[0])?)),
                "replace" => {
                    let (h, mi, s, u) = tm_dt(us).hms();
                    let mut v = [h, mi, s, u];
                    for (k, nm) in ["hour", "minute", "second", "microsecond"].iter().enumerate() {
                        match a.kw(nm).or_else(|| a.p.get(k).cloned()) {
                            Some(V::Int(i)) => v[k] = i,
                            Some(_) => return type_err("an integer is required".into()),
                            None => {}
                        }
                    }
                    tm_new(Args::p(v.iter().map(|x| V::Int(*x)).collect()))?
                }
                _ => return Err(exc("AttributeError", format!("'datetime.time' object has no attribute '{n}'"))),
            })
        })())
    }
}
impl Ext for TDV {
    fn tname(&self) -> &'static str {
        "datetime.timedelta"
    }
    fn repr(&self) -> String {
        let (d, s, us) = self.0.parts();
        let mut parts = vec![];
        if d != 0 {
            parts.push(format!("days={d}"));
        }
        if s != 0 {
            parts.push(format!("seconds={s}"));
        }
        if us != 0 {
            parts.push(format!("microseconds={us}"));
        }
        if parts.is_empty() {
            "datetime.timedelta(0)".into()
        } else {
            format!("datetime.timedelta({})", parts.join(", "))
        }
    }
    fn str_(&self) -> String {
        self.0.to_s()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(hash_int((self.0.us % (1i128 << 62)) as i64))
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(td_of(o).is_some_and(|t| t.us == self.0.us))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        td_of(o).map(|t| self.0.us.cmp(&t.us))
    }
    fn truth(&self) -> bool {
        self.0.us != 0
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let (d, s, us) = self.0.parts();
        Some(Ok(match n {
            "days" => V::Int(d),
            "seconds" => V::Int(s),
            "microseconds" => V::Int(us),
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(Ok(match n {
            "total_seconds" => V::Float(self.0.us as f64 / 1e6),
            "__abs__" => tdv(TD { us: self.0.us.abs() }),
            _ => return Some(Err(exc("AttributeError", format!("'datetime.timedelta' object has no attribute '{n}'")))),
        }))
    }
    fn binop(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        let a = self.0;
        match op {
            "neg" => return Some(Ok(tdv(TD { us: -a.us }))),
            "pos" => return Some(Ok(me.clone())),
            _ => {}
        }
        if let Some(b) = td_of(o) {
            let (x, y) = if right { (b, a) } else { (a, b) };
            return Some(Ok(match op {
                "+" => tdv(TD { us: x.us + y.us }),
                "-" => tdv(TD { us: x.us - y.us }),
                "/" => {
                    if y.us == 0 {
                        return Some(Err(exc("ZeroDivisionError", "division by zero".into())));
                    }
                    V::Float(x.us as f64 / y.us as f64)
                }
                "//" => {
                    if y.us == 0 {
                        return Some(Err(exc("ZeroDivisionError", "integer division or modulo by zero".into())));
                    }
                    V::Int(x.us.div_euclid(y.us) as i64)
                }
                "%" => {
                    if y.us == 0 {
                        return Some(Err(exc("ZeroDivisionError", "integer division or modulo by zero".into())));
                    }
                    tdv(TD { us: x.us.rem_euclid(y.us) })
                }
                _ => return None,
            }));
        }
        match (op, o) {
            ("*", V::Int(k)) => Some(Ok(tdv(TD { us: a.us * *k as i128 }))),
            ("*", V::Float(f)) => Some(Ok(tdv(TD { us: (a.us as f64 * f).round_ties_even() as i128 }))),
            ("//", V::Int(k)) if !right => {
                if *k == 0 {
                    return Some(Err(exc("ZeroDivisionError", "integer division or modulo by zero".into())));
                }
                Some(Ok(tdv(TD { us: a.us.div_euclid(*k as i128) })))
            }
            ("/", V::Int(_) | V::Float(_)) if !right => {
                let k = num_f(o).unwrap_or(1.0);
                if k == 0.0 {
                    return Some(Err(exc("ZeroDivisionError", "division by zero".into())));
                }
                Some(Ok(tdv(TD { us: (a.us as f64 / k).round_ties_even() as i128 })))
            }
            _ => None,
        }
    }
}
fn td_new(mut a: Args) -> R {
    let names = ["days", "seconds", "microseconds", "milliseconds", "minutes", "hours", "weeks"];
    let mult: [f64; 7] = [86400e6, 1e6, 1.0, 1e3, 60e6, 3600e6, 604800e6];
    let multi: [i128; 7] = [86_400_000_000, 1_000_000, 1, 1000, 60_000_000, 3_600_000_000, 604_800_000_000];
    let mut us: i128 = 0;
    let mut fus: f64 = 0.0;
    for (k, n) in names.iter().enumerate() {
        let v = a.kw(n).or_else(|| a.p.get(k).cloned());
        match v {
            None => {}
            Some(V::Int(i)) => us += i as i128 * multi[k],
            Some(V::Bool(b)) => us += b as i128 * multi[k],
            Some(V::Float(f)) => fus += f * mult[k],
            Some(x) => return type_err(format!("unsupported type for timedelta {n} component: {}", tn(&x))),
        }
    }
    if let Some((k, _)) = a.k.first() {
        return type_err(format!("'{k}' is an invalid keyword argument for __new__()"));
    }
    us += fus.round_ties_even() as i128;
    Ok(tdv(TD { us }))
}
impl Ext for DTV {
    fn tname(&self) -> &'static str {
        if self.0.has_time {
            "datetime.datetime"
        } else {
            "datetime.date"
        }
    }
    fn repr(&self) -> String {
        let (y, m, d) = self.0.ymd();
        if !self.0.has_time {
            return format!("datetime.date({y}, {m}, {d})");
        }
        let (h, mi, s, us) = self.0.hms();
        let mut t = format!("datetime.datetime({y}, {m}, {d}, {h}, {mi}");
        if s != 0 || us != 0 {
            let _ = write!(t, ", {s}");
        }
        if us != 0 {
            let _ = write!(t, ", {us}");
        }
        if self.0.fold {
            t.push_str(", fold=1");
        }
        if let Some(o) = self.0.tz {
            let _ = write!(t, ", tzinfo={}", repr(&tzinfo_v(o)).unwrap_or_default());
        }
        t.push(')');
        t
    }
    fn str_(&self) -> String {
        self.0.iso(' ')
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        let mut d = self.0;
        d.fold = false;
        Some(hash_int(d.utc_us() as i64))
    }
    // Aware datetimes compare as instants, or by wall clock when they share a tzinfo; an aware one never
    // equals a naive one, nor (PEP 495) one whose offset depends on its fold.
    fn eq(&self, o: &V) -> Option<bool> {
        let a = self.0;
        Some(dt_of(o).is_some_and(|t| {
            t.has_time == a.has_time
                && t.tz.is_some() == a.tz.is_some()
                && if t.tz == a.tz { (t.ord, t.us) == (a.ord, a.us) } else { !a.fold_sensitive() && !t.fold_sensitive() && t.utc_us() == a.utc_us() }
        }))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        let t = dt_of(o)?;
        if t.has_time != self.0.has_time || t.tz.is_some() != self.0.tz.is_some() {
            return None;
        }
        if t.tz == self.0.tz {
            return Some((self.0.ord, self.0.us).cmp(&(t.ord, t.us)));
        }
        Some(self.0.utc_us().cmp(&t.utc_us()))
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.str_())));
        }
        Some(Ok(st(self.0.strftime(spec))))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let (y, m, d) = self.0.ymd();
        let (h, mi, s, us) = self.0.hms();
        Some(Ok(match n {
            "year" => V::Int(y),
            "month" => V::Int(m),
            "day" => V::Int(d),
            "hour" if self.0.has_time => V::Int(h),
            "minute" if self.0.has_time => V::Int(mi),
            "second" if self.0.has_time => V::Int(s),
            "microsecond" if self.0.has_time => V::Int(us),
            "tzinfo" if self.0.has_time => self.0.tz.map_or(V::None, tzinfo_v),
            "fold" if self.0.has_time => V::Int(self.0.fold as i64),
            _ => return None,
        }))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        let a = self.0;
        if let Some(t) = td_of(o) {
            let shift = |sign: i128| -> R {
                let total = a.ord as i128 * 86_400_000_000 + a.us as i128 + if a.has_time { sign * t.us } else { sign * (t.us.div_euclid(86_400_000_000) * 86_400_000_000) };
                let ord = total.div_euclid(86_400_000_000) as i64;
                if !(1..=3652059).contains(&ord) {
                    return Err(exc("OverflowError", "date value out of range".into()));
                }
                Ok(dtv(DT { ord, us: total.rem_euclid(86_400_000_000) as i64, has_time: a.has_time, tz: a.tz, fold: false }))
            };
            return match (op, right) {
                ("+", _) => Some(shift(1)),
                ("-", false) => Some(shift(-1)),
                _ => None,
            };
        }
        if let Some(b) = dt_of(o) {
            if op == "-" && b.has_time == a.has_time {
                let (x, y) = if right { (b, a) } else { (a, b) };
                if x.tz.is_some() != y.tz.is_some() {
                    return Some(type_err("can't subtract offset-naive and offset-aware datetimes".into()));
                }
                if x.tz == y.tz {
                    return Some(Ok(tdv(TD { us: (x.ord as i128 * DAY_US + x.us as i128) - (y.ord as i128 * DAY_US + y.us as i128) })));
                }
                return Some(Ok(tdv(TD { us: x.utc_us() - y.utc_us() })));
            }
        }
        None
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let t = self.0;
        // A tzinfo Brevi lacks (zoneinfo.ZoneInfo): CPython's datetime does the call, and the result is its datetime.
        if t.has_time && matches!(n, "astimezone" | "replace") && has_py(a) {
            return Some(py_dt_self(_me, "datetime", n, a));
        }
        Some((|| -> R {
            Ok(match n {
                "isoformat" => {
                    let sep = match a.kw("sep").or_else(|| a.p.first().cloned()) {
                        Some(V::Str(x)) => x.s.chars().next().unwrap_or('T'),
                        _ => 'T',
                    };
                    let spec = timespec_arg(a.kw("timespec").or_else(|| a.p.get(1).cloned()))?;
                    st(t.iso_spec(sep, &spec)?)
                }
                "strftime" | "__format__" => st(t.strftime(&to_str(&a.p[0])?)),
                "weekday" => V::Int(t.weekday()),
                "isoweekday" => V::Int(t.weekday() + 1),
                "toordinal" => V::Int(t.ord),
                "isocalendar" => {
                    let (y, w, d) = t.isocal();
                    tup(vec![V::Int(y), V::Int(w), V::Int(d)])
                }
                "date" => dtv(DT { ord: t.ord, us: 0, has_time: false, tz: None, fold: false }),
                "time" if t.has_time => tmv(t.us),
                "timetz" if t.has_time && t.tz.is_none() => tmv(t.us),
                "timetz" if t.has_time => return Err(unsupported("datetime.timetz of an aware datetime")),
                "utcoffset" if t.has_time => t.off().map_or(V::None, |o| td_secs(o as i64)),
                "tzname" if t.has_time => t.tti().map_or(V::None, |x| st(x.name.to_string())),
                "dst" if t.has_time => match t.tz {
                    Some(Tz::Z(_)) => t.tti().map_or(V::None, |x| td_secs(x.dst as i64)),
                    _ => V::None,
                },
                "timetuple" => return Err(unsupported("timetuple")),
                "ctime" => st(t.strftime("%c")),
                // A naive datetime's timestamp reads it as local time, which dynamic builds do not know.
                "timestamp" if t.tz.is_some() => V::Float((t.utc_us() - EPOCH_US) as f64 / 1e6),
                "timestamp" => return Err(unsupported("datetime.timestamp of a naive datetime (local time)")),
                "astimezone" if t.has_time => match (t.tz, tz_arg(a.kw("tz").or_else(|| a.p.first().cloned()))?) {
                    (Some(x), Some(o)) if x == o => _me.clone(),
                    (Some(_), Some(o)) => dtv(DT::from_utc_us(t.utc_us(), Some(o))?),
                    _ => return Err(unsupported("datetime.astimezone to or from local time")),
                },
                "replace" => {
                    let (mut y, mut m, mut d) = t.ymd();
                    let (mut h, mut mi, mut s, mut us) = t.hms();
                    let names = ["year", "month", "day", "hour", "minute", "second", "microsecond"];
                    for (k, nm) in names.iter().enumerate() {
                        let v = a.kw(nm).or_else(|| a.p.get(k).cloned());
                        if let Some(v) = v {
                            let x = match v {
                                V::Int(i) => i,
                                _ => return type_err("an integer is required".into()),
                            };
                            match k {
                                0 => y = x,
                                1 => m = x,
                                2 => d = x,
                                3 => h = x,
                                4 => mi = x,
                                5 => s = x,
                                _ => us = x,
                            }
                        }
                    }
                    check_date(y, m, d)?;
                    let tz = match a.kw("tzinfo").or_else(|| a.p.get(7).cloned()) {
                        Some(v) if t.has_time => tz_arg(Some(v))?,
                        _ => t.tz,
                    };
                    let fold = match a.kw("fold") {
                        Some(v) if t.has_time => fold_arg(v)?,
                        _ => t.fold,
                    };
                    check_time(h, mi, s, us)?;
                    dtv(DT { ord: ymd_to_ord(y, m, d), us: ((h * 60 + mi) * 60 + s) * 1_000_000 + us, has_time: t.has_time, tz, fold })
                }
                _ => return Err(exc("AttributeError", format!("'{}' object has no attribute '{n}'", self.tname()))),
            })
        })())
    }
}
fn int_args(a: &mut Args, names: &[&str], req: usize, what: &str) -> Result<Vec<i64>, E> {
    let mut out = vec![];
    for (k, n) in names.iter().enumerate() {
        let v = a.kw(n).or_else(|| a.p.get(k).cloned());
        match v {
            Some(V::Int(i)) => out.push(i),
            Some(V::Bool(b)) => out.push(b as i64),
            Some(x) => return type_err(format!("'{}' object cannot be interpreted as an integer", tn(&x))),
            None if k < req => return type_err(format!("{what}() missing required argument '{n}' (pos {})", k + 1)),
            None => out.push(0),
        }
    }
    Ok(out)
}
fn date_new(mut a: Args) -> R {
    let v = int_args(&mut a, &["year", "month", "day"], 3, "date")?;
    check_date(v[0], v[1], v[2])?;
    Ok(dtv(DT { ord: ymd_to_ord(v[0], v[1], v[2]), us: 0, has_time: false, tz: None, fold: false }))
}
fn datetime_new(mut a: Args) -> R {
    if has_py(&a) {
        return py_dt("datetime", "", a);
    }
    let tz = tz_arg(a.kw("tzinfo").or_else(|| a.p.get(7).cloned()))?;
    let fold = match a.kw("fold") {
        Some(v) => fold_arg(v)?,
        None => false,
    };
    let v = int_args(&mut a, &["year", "month", "day", "hour", "minute", "second", "microsecond"], 3, "datetime")?;
    check_date(v[0], v[1], v[2])?;
    check_time(v[3], v[4], v[5], v[6])?;
    Ok(dtv(DT { ord: ymd_to_ord(v[0], v[1], v[2]), us: ((v[3] * 60 + v[4]) * 60 + v[5]) * 1_000_000 + v[6], has_time: true, tz, fold }))
}
fn check_time(h: i64, mi: i64, s: i64, us: i64) -> Result<(), E> {
    if !(0..24).contains(&h) {
        return val_err("hour must be in 0..23".into());
    }
    if !(0..60).contains(&mi) {
        return val_err("minute must be in 0..59".into());
    }
    if !(0..60).contains(&s) {
        return val_err("second must be in 0..59".into());
    }
    if !(0..1_000_000).contains(&us) {
        return val_err("microsecond must be in 0..999999".into());
    }
    Ok(())
}
fn fold_arg(v: V) -> Result<bool, E> {
    match v {
        V::Int(0) | V::Bool(false) => Ok(false),
        V::Int(1) | V::Bool(true) => Ok(true),
        V::Int(_) => val_err("fold must be either 0 or 1".into()),
        x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(&x))),
    }
}
fn date_class_attr(cls: &'static str, n: &str) -> Option<V> {
    let has_time = cls == "datetime.datetime";
    let f: Sym = sym(n);
    Some(match n {
        "fromisoformat" => builtin(f, move |a: Args| Ok(dtv(fromiso(&to_str(&a1(&a, "fromisoformat")?)?, has_time)?))),
        "fromordinal" => builtin(f, move |a: Args| match a1(&a, "fromordinal")? {
            V::Int(o) if o >= 1 => Ok(dtv(DT { ord: o, us: 0, has_time, tz: None, fold: false })),
            _ => val_err("ordinal must be >= 1".into()),
        }),
        "strptime" if has_time => builtin(f, |a: Args| {
            let t = to_str(a.p.first().unwrap_or(&V::None))?;
            let fm = to_str(a.p.get(1).unwrap_or(&V::None))?;
            Ok(dtv(strptime(&t, &fm)?))
        }),
        "combine" if has_time => builtin(f, |mut a: Args| {
            if has_py(&a) {
                return py_dt("datetime", "combine", a);
            }
            let d = dt_of(a.p.first().unwrap_or(&V::None)).ok_or_else(|| exc("TypeError", "combine() argument 1 must be datetime.date".into()))?;
            let t = a.p.get(1).and_then(|v| tm_of(v).or_else(|| dt_of(v).map(|t| t.us))).unwrap_or(0);
            let tz = tz_arg(a.kw("tzinfo").or_else(|| a.p.get(2).cloned()))?;
            Ok(dtv(DT { ord: d.ord, us: t, has_time: true, tz, fold: false }))
        }),
        "now" if has_time => builtin(f, |mut a: Args| match if has_py(&a) { return py_dt("datetime", "now", a) } else { tz_arg(a.kw("tz").or_else(|| a.p.first().cloned()))? } {
            Some(o) => Ok(dtv(DT::from_utc_us(EPOCH_US + now_us(), Some(o))?)),
            None => Ok(dtv(DT { tz: None, ..DT::from_utc_us(EPOCH_US + now_us(), None)? })),
        }),
        "utcfromtimestamp" if has_time => builtin(f, |a: Args| Ok(dtv(DT::from_utc_us(EPOCH_US + ts_us(&a1(&a, "utcfromtimestamp")?)?, None)?))),
        // Without tz, fromtimestamp gives local time, which dynamic builds do not know.
        "fromtimestamp" if has_time => builtin(f, |mut a: Args| {
            if has_py(&a) {
                return py_dt("datetime", "fromtimestamp", a);
            }
            let t = ts_us(a.p.first().unwrap_or(&V::None))?;
            match tz_arg(a.kw("tz").or_else(|| a.p.get(1).cloned()))? {
                Some(o) => Ok(dtv(DT::from_utc_us(EPOCH_US + t, Some(o))?)),
                None => Err(unsupported("datetime.fromtimestamp without tz (local time)")),
            }
        }),
        "today" | "now" | "utcnow" => builtin(f, move |_a: Args| {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_micros() as i64).unwrap_or(0);
            let ord = ymd_to_ord(1970, 1, 1) + secs.div_euclid(86_400_000_000);
            Ok(dtv(DT { ord, us: if has_time { secs.rem_euclid(86_400_000_000) } else { 0 }, has_time, tz: None, fold: false }))
        }),
        "min" => dtv(DT { ord: 1, us: 0, has_time, tz: None, fold: false }),
        "max" => dtv(DT { ord: ymd_to_ord(9999, 12, 31), us: if has_time { 86_399_999_999 } else { 0 }, has_time, tz: None, fold: false }),
        _ => return None,
    })
}

// ---------- csv ----------

struct CsvDialect {
    delim: char,
    quote: char,
    skipinit: bool,
    lineterm: String,
    quoting: i64,
}
fn csv_dialect(a: &mut Args) -> Result<CsvDialect, E> {
    let ch = |v: Option<V>, d: char| -> Result<char, E> {
        match v {
            None => Ok(d),
            Some(V::Str(x)) if x.s.chars().count() == 1 => Ok(x.s.chars().next().unwrap()),
            Some(_) => type_err("\"delimiter\" must be a 1-character string".into()),
        }
    };
    let delim = ch(a.kw("delimiter"), ',')?;
    let quote = ch(a.kw("quotechar"), '"')?;
    let skipinit = match a.kw("skipinitialspace") {
        Some(v) => truth(&v)?,
        None => false,
    };
    let lineterm = match a.kw("lineterminator") {
        Some(v) => to_str(&v)?,
        None => "\r\n".into(),
    };
    let quoting = match a.kw("quoting") {
        Some(V::Int(q)) => q,
        _ => 0,
    };
    a.kw("dialect");
    a.kw("strict");
    a.kw("doublequote");
    a.kw("escapechar");
    Ok(CsvDialect { delim, quote, skipinit, lineterm, quoting })
}
// One record from the line iterator, or None at the end.
fn csv_record(it: &mut It, d: &CsvDialect) -> Result<Option<Vec<String>>, E> {
    let Some(first) = it.next()? else { return Ok(None) };
    let mut line = to_str(&first)?;
    let mut fields = vec![];
    let mut cur = String::new();
    let mut in_q = false;
    let mut was_q = false;
    let mut at_start = true;
    loop {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if in_q {
                if c == d.quote {
                    if chars.get(i + 1) == Some(&d.quote) {
                        cur.push(c);
                        i += 2;
                        continue;
                    }
                    in_q = false;
                } else {
                    cur.push(c);
                }
            } else if c == d.delim {
                fields.push(std::mem::take(&mut cur));
                at_start = true;
                was_q = false;
                i += 1;
                continue;
            } else if c == d.quote && at_start && d.quoting != 3 {
                in_q = true;
                was_q = true;
            } else if c == '\n' || c == '\r' {
                // end of record (a trailing newline)
            } else if at_start && d.skipinit && c == ' ' {
                i += 1;
                continue;
            } else {
                cur.push(c);
            }
            at_start = false;
            i += 1;
        }
        if in_q {
            match it.next()? {
                Some(next) => {
                    line = to_str(&next)?;
                    continue;
                }
                None => return Err(exc("Error", "unexpected end of data".into())),
            }
        }
        break;
    }
    let _ = was_q;
    if fields.is_empty() && cur.is_empty() && !line.trim_end_matches(['\r', '\n']).contains(d.quote) && line.trim_end_matches(['\r', '\n']).is_empty() {
        return Ok(Some(vec![]));
    }
    fields.push(cur);
    Ok(Some(fields))
}
fn csv_field(v: &V, d: &CsvDialect) -> Result<String, E> {
    let t = match v {
        V::None => String::new(),
        V::Str(x) => x.s.clone(),
        V::Float(f) => float_repr(*f),
        _ => to_str(v)?,
    };
    let numeric = matches!(v, V::Int(_) | V::Float(_) | V::Big(_) | V::Bool(_));
    let need = match d.quoting {
        1 => true,
        2 => !numeric,
        3 => false,
        _ => t.contains(d.delim) || t.contains(d.quote) || t.contains('\n') || t.contains('\r') || (t.is_empty() && false),
    };
    if need {
        let q = d.quote.to_string();
        Ok(format!("{q}{}{q}", t.replace(&q, &format!("{q}{q}"))))
    } else {
        Ok(t)
    }
}
pub struct CsvWriter {
    f: V,
    d: CsvDialect,
    fields: Option<Vec<V>>,
}
impl Ext for CsvWriter {
    fn tname(&self) -> &'static str {
        "_csv.writer"
    }
    fn repr(&self) -> String {
        "<_csv.writer object>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let write_row = |row: Vec<V>| -> R {
            let mut parts = vec![];
            for x in &row {
                parts.push(csv_field(x, &self.d)?);
            }
            if parts.len() == 1 && parts[0].is_empty() && self.d.quoting != 3 {
                parts[0] = format!("{0}{0}", self.d.quote);
            }
            let line = format!("{}{}", parts.join(&self.d.delim.to_string()), self.d.lineterm);
            match &self.f {
                V::Module(&ModName("sys.stdout")) => out_write(&line),
                V::Module(&ModName("sys.stderr")) => {
                    out_flush();
                    err_write(&line)
                }
                f => {
                    call_method(f, "write", Args::p(vec![st(line)]))?;
                }
            }
            Ok(V::None)
        };
        let dict_row = |r: &V| -> Result<Vec<V>, E> {
            match &self.fields {
                None => to_vec(r),
                Some(fs) => {
                    let mut out = vec![];
                    for f in fs {
                        out.push(match r {
                            V::Dict(d) => d.borrow().get(f)?.unwrap_or(s("")),
                            _ => getitem(r, f)?,
                        });
                    }
                    Ok(out)
                }
            }
        };
        Some(match n {
            "writerow" => dict_row(&a.p[0]).and_then(write_row),
            "writerows" => (|| {
                for r in to_vec(&a.p[0])? {
                    write_row(dict_row(&r)?)?;
                }
                Ok(V::None)
            })(),
            "writeheader" => match &self.fields {
                Some(fs) => write_row(fs.clone()),
                None => Ok(V::None),
            },
            _ => Err(exc("AttributeError", format!("'_csv.writer' object has no attribute '{n}'"))),
        })
    }
}
fn csv_fn(n: &str, mut a: Args) -> R {
    match n {
        "reader" => {
            let src = a.p.first().cloned().unwrap_or(V::None);
            let d = csv_dialect(&mut a)?;
            let mut it = iter(&src)?;
            Ok(gen_iter(move || Ok(csv_record(&mut it, &d)?.map(|f| list(f.into_iter().map(st).collect())))))
        }
        "DictReader" => {
            let src = a.p.first().cloned().unwrap_or(V::None);
            let fieldnames = a.kw("fieldnames").or_else(|| a.p.get(1).cloned());
            let restkey = a.kw("restkey").unwrap_or(V::None);
            let restval = a.kw("restval").unwrap_or(V::None);
            let d = csv_dialect(&mut a)?;
            let mut it = iter(&src)?;
            let mut names: Option<Vec<V>> = match fieldnames {
                Some(f) if !matches!(f, V::None) => Some(to_vec(&f)?),
                _ => None,
            };
            Ok(gen_iter(move || {
                if names.is_none() {
                    loop {
                        match csv_record(&mut it, &d)? {
                            None => return Ok(None),
                            Some(r) if r.is_empty() => continue,
                            Some(r) => {
                                names = Some(r.into_iter().map(st).collect());
                                break;
                            }
                        }
                    }
                }
                let ns = names.as_ref().unwrap();
                loop {
                    let Some(row) = csv_record(&mut it, &d)? else { return Ok(None) };
                    if row.is_empty() {
                        continue;
                    }
                    let mut out = Dict::new();
                    for (k, nm) in ns.iter().enumerate() {
                        out.set(nm.clone(), row.get(k).map(|x| s(x)).unwrap_or(restval.clone()))?;
                    }
                    if row.len() > ns.len() {
                        out.set(restkey.clone(), list(row[ns.len()..].iter().map(|x| s(x)).collect()))?;
                    }
                    return Ok(Some(V::Dict(Rc::new(RefCell::new(out)))));
                }
            }))
        }
        "writer" | "DictWriter" => {
            let f = a.p.first().cloned().unwrap_or(V::None);
            let fields = if n == "DictWriter" { Some(to_vec(&a.kw("fieldnames").or_else(|| a.p.get(1).cloned()).unwrap_or(V::None))?) } else { None };
            a.kw("extrasaction");
            a.kw("restval");
            let d = csv_dialect(&mut a)?;
            Ok(V::Ext(ExtRc::new(CsvWriter { f, d, fields })))
        }
        _ => Err(unsupported(&format!("csv.{n}"))),
    }
}

// ---------- html, statistics, threading ----------

fn html_fn(n: &str, a: Args) -> R {
    let t = to_str(a.p.first().unwrap_or(&V::None))?;
    match n {
        "escape" => {
            let quote = match a.p.get(1).cloned().or_else(|| a.k.iter().find(|(k, _)| *k == "quote").map(|x| x.1.clone())) {
                Some(v) => truth(&v)?,
                None => true,
            };
            let mut out = String::new();
            for c in t.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' if quote => out.push_str("&quot;"),
                    '\'' if quote => out.push_str("&#x27;"),
                    c => out.push(c),
                }
            }
            Ok(st(out))
        }
        "unescape" => {
            let mut out = t.clone();
            for (e, r) in [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#x27;", "'"), ("&#39;", "'"), ("&nbsp;", "\u{a0}"), ("&amp;", "&")] {
                out = out.replace(e, r);
            }
            Ok(st(out))
        }
        _ => Err(unsupported(&format!("html.{n}"))),
    }
}
fn frac_of_num(v: &V) -> Result<Frac, E> {
    match v {
        V::Float(f) => Frac::of_f64(*f).ok_or_else(|| exc("ValueError", "nan or inf in data".into())),
        V::Ext(x) if x.any().downcast_ref::<DecV>().is_some() => Frac::of_dec(&x.any().downcast_ref::<DecV>().unwrap().0).ok_or_else(|| exc("ValueError", "bad Decimal".into())),
        _ => Frac::of(v).ok_or_else(|| exc("TypeError", format!("can't convert type '{}' to numerator/denominator", tn(v)))),
    }
}
fn stat_kind(xs: &[V]) -> u8 {
    // 0 int, 1 float, 2 Fraction, 3 Decimal
    let mut k = 0;
    for x in xs {
        let t = match x {
            V::Float(_) => 1,
            V::Ext(e) if e.any().downcast_ref::<FracV>().is_some() => 2,
            V::Ext(e) if e.any().downcast_ref::<DecV>().is_some() => 3,
            _ => 0,
        };
        k = k.max(t);
    }
    k
}
fn stat_out(f: Frac, kind: u8) -> V {
    match kind {
        0 if f.d.cmp(&bigi(1)) == std::cmp::Ordering::Equal => f.n.norm(),
        0 | 1 => V::Float(f.to_f64()),
        2 => fracv(f),
        _ => decv(Dec { neg: false, c: bigi(0), e: 0, sp: 0 }.add(&Dec::from_big(&f.n)).and_then(|n| n.div(&Dec::from_big(&f.d))).unwrap_or(Dec::zero())),
    }
}
fn statistics_fn(n: &str, a: Args) -> R {
    let xs = to_vec(a.p.first().unwrap_or(&V::None))?;
    let kind = stat_kind(&xs);
    let err = |what: &str| exc("StatisticsError", what.into());
    let sum_frac = |xs: &[V]| -> Result<Frac, E> {
        let mut t = Frac { n: bigi(0), d: bigi(1) };
        for x in xs {
            let f = frac_of_num(x)?;
            t = Frac::new(t.n.mul(&f.d).add(&f.n.mul(&t.d)), t.d.mul(&f.d))?;
        }
        Ok(t)
    };
    let mean_frac = |xs: &[V]| -> Result<Frac, E> {
        let t = sum_frac(xs)?;
        Frac::new(t.n, t.d.mul(&Big::from_i64(xs.len() as i64)))
    };
    let var_frac = |xs: &[V], sample: bool| -> Result<Frac, E> {
        let m = mean_frac(xs)?;
        let mut ss = Frac { n: bigi(0), d: bigi(1) };
        for x in xs {
            let f = frac_of_num(x)?;
            let dlt = Frac::new(f.n.mul(&m.d).sub(&m.n.mul(&f.d)), f.d.mul(&m.d))?;
            let sq = Frac { n: dlt.n.mul(&dlt.n), d: dlt.d.mul(&dlt.d) };
            ss = Frac::new(ss.n.mul(&sq.d).add(&sq.n.mul(&ss.d)), ss.d.mul(&sq.d))?;
        }
        let k = xs.len() as i64 - sample as i64;
        Frac::new(ss.n, ss.d.mul(&Big::from_i64(k)))
    };
    match n {
        "mean" | "fmean" => {
            if xs.is_empty() {
                return Err(err(if n == "mean" { "mean requires at least one data point" } else { "fmean requires at least one data point" }));
            }
            let m = mean_frac(&xs)?;
            Ok(if n == "fmean" { V::Float(m.to_f64()) } else { stat_out(m, kind) })
        }
        "median" | "median_low" | "median_high" => {
            if xs.is_empty() {
                return Err(err("no median for empty data"));
            }
            let mut v = xs;
            sort_values(&mut v, None, false)?;
            let k = v.len();
            if k % 2 == 1 {
                return Ok(v[k / 2].clone());
            }
            match n {
                "median_low" => Ok(v[k / 2 - 1].clone()),
                "median_high" => Ok(v[k / 2].clone()),
                _ => {
                    let s = add(&v[k / 2 - 1], &v[k / 2])?;
                    binop(&s, &V::Int(2), "/")
                }
            }
        }
        "mode" => {
            if xs.is_empty() {
                return Err(err("no mode for empty data"));
            }
            let mut d = Dict::new();
            for x in &xs {
                let c = d.get(x)?.unwrap_or(V::Int(0));
                d.set(x.clone(), add(&c, &V::Int(1))?)?;
            }
            let mut best: Option<(V, i64)> = None;
            for (k, c) in d.ents.iter().flatten() {
                let c = if let V::Int(c) = c { *c } else { 0 };
                if best.as_ref().is_none_or(|b| c > b.1) {
                    best = Some((k.clone(), c));
                }
            }
            Ok(best.unwrap().0)
        }
        "multimode" => {
            let mut d = Dict::new();
            for x in &xs {
                let c = d.get(x)?.unwrap_or(V::Int(0));
                d.set(x.clone(), add(&c, &V::Int(1))?)?;
            }
            let mx = d.ents.iter().flatten().map(|(_, c)| if let V::Int(c) = c { *c } else { 0 }).max().unwrap_or(0);
            Ok(list(d.ents.iter().flatten().filter(|(_, c)| matches!(c, V::Int(k) if *k == mx)).map(|(k, _)| k.clone()).collect()))
        }
        "variance" | "pvariance" | "stdev" | "pstdev" => {
            let sample = n == "variance" || n == "stdev";
            if xs.len() < 1 + sample as usize {
                return Err(err(if sample { &"variance requires at least two data points"[..] } else { "pvariance requires at least one data point" }));
            }
            let v = var_frac(&xs, sample)?;
            if n.ends_with("stdev") {
                return Ok(match kind {
                    3 => {
                        let dv = Dec::from_big(&v.n).div(&Dec::from_big(&v.d))?;
                        let r = DecV(dv).method(&V::None, "sqrt", &mut Args::p(vec![])).unwrap()?;
                        r
                    }
                    _ => V::Float(v.to_f64().sqrt()),
                });
            }
            Ok(stat_out(v, kind))
        }
        _ => Err(unsupported(&format!("statistics.{n}"))),
    }
}
pub struct Thread {
    target: V,
    args: V,
    kwargs: V,
}
impl Ext for Thread {
    fn tname(&self) -> &'static str {
        "threading.Thread"
    }
    fn repr(&self) -> String {
        "<Thread>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(match n {
            "start" | "run" => (|| {
                if matches!(self.target, V::None) {
                    return Ok(V::None);
                }
                let mut a = Args::p(to_vec(&self.args)?);
                if !matches!(self.kwargs, V::None) {
                    star_kwargs(&mut a, &self.kwargs)?;
                }
                call(&self.target, a)?;
                Ok(V::None)
            })(),
            "join" | "setDaemon" => Ok(V::None),
            "is_alive" => Ok(V::Bool(false)),
            _ => Err(exc("AttributeError", format!("'Thread' object has no attribute '{n}'"))),
        })
    }
}
pub struct Lock;
impl Ext for Lock {
    fn tname(&self) -> &'static str {
        "_thread.lock"
    }
    fn repr(&self) -> String {
        "<unlocked _thread.lock object>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(Ok(match n {
            "acquire" | "__enter__" => V::Bool(true),
            "release" | "__exit__" => V::None,
            "locked" => V::Bool(false),
            _ => return Some(Err(exc("AttributeError", format!("'lock' object has no attribute '{n}'")))),
        }))
    }
}
fn threading_fn(n: &str, mut a: Args) -> R {
    match n {
        "stack_size" => Ok(V::Int(0)),
        "Thread" => {
            let target = a.kw("target").unwrap_or(V::None);
            let args = a.kw("args").unwrap_or(tup(vec![]));
            let kwargs = a.kw("kwargs").unwrap_or(V::None);
            Ok(V::Ext(ExtRc::new(Thread { target, args, kwargs })))
        }
        "Lock" | "RLock" => Ok(V::Ext(ExtRc::new(Lock))),
        "current_thread" | "main_thread" => Ok(V::None),
        "active_count" => Ok(V::Int(1)),
        _ => Err(unsupported(&format!("threading.{n}"))),
    }
}

// ---------- dispatch ----------

pub fn ext_construct(cls: &str, a: Args) -> Option<R> {
    Some(match cls {
        "decimal.Decimal" => decimal_new(a),
        "fractions.Fraction" => fraction_new(a),
        "datetime.date" => date_new(a),
        "datetime.datetime" => datetime_new(a),
        "datetime.timedelta" => td_new(a),
        "datetime.time" => tm_new(a),
        "datetime.timezone" => tz_new(a),
        "zoneinfo.ZoneInfo" => zoneinfo_new(a),
        _ => return None,
    })
}
pub fn ext_class_attr(cls: &'static str, n: &str) -> Option<V> {
    match cls {
        "datetime.date" | "datetime.datetime" => date_class_attr(cls, n),
        "datetime.time" => match n {
            "fromisoformat" => Some(builtin(sym(n), |a: Args| {
                let t = to_str(&a1(&a, "fromisoformat")?)?;
                let body = t.strip_prefix('T').unwrap_or(&t);
                match fromiso(&format!("0001-01-01T{body}"), true) {
                    Ok(d) if d.tz.is_none() => Ok(tmv(d.us)),
                    Ok(_) => Err(unsupported("datetime.time with a UTC offset")),
                    Err(_) => val_err(format!("Invalid isoformat string: {}", str_repr(&t))),
                }
            })),
            "min" => Some(tmv(0)),
            "max" => Some(tmv(DAY_US as i64 - 1)),
            "resolution" => Some(tdv(TD { us: 1 })),
            _ => None,
        },
        "datetime.timezone" => match n {
            "utc" => Some(tzv(0)),
            "min" => Some(tzv(-86_340)),
            "max" => Some(tzv(86_340)),
            _ => None,
        },
        "datetime.timedelta" => match n {
            "min" => Some(tdv(TD { us: -999999999i128 * 86_400_000_000 })),
            "max" => Some(tdv(TD { us: 1_000_000_000i128 * 86_400_000_000 - 1 })),
            "resolution" => Some(tdv(TD { us: 1 })),
            _ => None,
        },
        "fractions.Fraction" => match n {
            "from_float" => Some(builtin("from_float", |a: Args| fraction_new(a))),
            "from_decimal" => Some(builtin("from_decimal", |a: Args| fraction_new(a))),
            _ => None,
        },
        "decimal.Decimal" => match n {
            "from_float" => Some(builtin("from_float", |a: Args| decimal_new(a))),
            _ => None,
        },
        _ => None,
    }
}
pub fn ext_module_call2(m: &str, n: &str, a: Args) -> Option<R> {
    if let Some(V::Class(c)) = ext_module_attr2(m, n) {
        return Some(instantiate(&c, a));
    }
    Some(match m {
        "decimal" => match n {
            "getcontext" => Ok(V::Module(&ModName("decimal.context"))),
            "localcontext" => Err(unsupported("decimal.localcontext")),
            _ => Err(unsupported(&format!("decimal.{n}"))),
        },
        "fractions" => Err(unsupported(&format!("fractions.{n}"))),
        "datetime" => Err(unsupported(&format!("datetime.{n}"))),
        "csv" => csv_fn(n, a),
        "html" => html_fn(n, a),
        "statistics" => statistics_fn(n, a),
        "zoneinfo" => zoneinfo_fn(n, a),
        "tzlocal" => tzlocal_fn(n, a),
        "importlib.metadata" => metadata_fn(n, a),
        "threading" => threading_fn(n, a),
        _ => return None,
    })
}
pub fn ext_module_attr2(m: &str, n: &str) -> Option<V> {
    Some(match (m, n) {
        ("decimal", "Decimal") => V::Class(ext_class("decimal.Decimal")),
        ("decimal", r) if r.starts_with("ROUND_") => s(r),
        ("decimal", "InvalidOperation" | "DivisionByZero") => V::Class(exc_class(n)),
        ("decimal", "DecimalException") => V::Class(exc_class("ArithmeticError")),
        ("decimal.context", "prec") => V::Int(DPREC.with(|p| p.get()) as i64),
        ("decimal.context", "rounding") => s(ROUNDINGS[DROUND.with(|r| r.get()) as usize]),
        ("fractions", "Fraction") => V::Class(ext_class("fractions.Fraction")),
        ("datetime", "date") => V::Class(ext_class("datetime.date")),
        ("datetime", "datetime") => V::Class(ext_class("datetime.datetime")),
        ("datetime", "timedelta") => V::Class(ext_class("datetime.timedelta")),
        ("datetime", "time") => V::Class(ext_class("datetime.time")),
        ("datetime", "timezone") => V::Class(ext_class("datetime.timezone")),
        ("datetime", "UTC") => tzv(0),
        ("datetime", "MINYEAR") => V::Int(1),
        ("datetime", "MAXYEAR") => V::Int(9999),
        ("csv", "QUOTE_MINIMAL") => V::Int(0),
        ("csv", "QUOTE_ALL") => V::Int(1),
        ("csv", "QUOTE_NONNUMERIC") => V::Int(2),
        ("csv", "QUOTE_NONE") => V::Int(3),
        ("csv", "Error") => V::Class(exc_class("Error")),
        ("statistics", "StatisticsError") => V::Class(exc_class("StatisticsError")),
        ("zoneinfo", n) => return zoneinfo_attr(n),
        ("importlib.metadata", "PackageNotFoundError") => V::Class(exc_class("PackageNotFoundError")),
        _ => return None,
    })
}
pub fn decimal_context_set(n: &str, v: &V) -> Result<(), E> {
    match n {
        "prec" => match v {
            V::Int(p) if *p >= 1 => {
                DPREC.with(|x| x.set(*p as usize));
                Ok(())
            }
            _ => val_err("valid range for prec is [1, MAX_PREC]".into()),
        },
        "rounding" => {
            let c = round_code(v)?;
            DROUND.with(|x| x.set(c));
            Ok(())
        }
        _ => Err(unsupported(&format!("decimal context attribute {n}"))),
    }
}

// ---------- time.gmtime / time.strftime ----------

thread_local! { static STRUCT_TIME: RefCell<Option<V>> = RefCell::new(None); }
fn struct_time_class() -> R {
    if let Some(c) = STRUCT_TIME.with(|c| c.borrow().clone()) {
        return Ok(c);
    }
    let c = namedtuple_runtime_type(Args::p(vec![s("time.struct_time"), s("tm_year tm_mon tm_mday tm_hour tm_min tm_sec tm_wday tm_yday tm_isdst")]))?;
    STRUCT_TIME.with(|x| *x.borrow_mut() = Some(c.clone()));
    Ok(c)
}
fn time_gmtime(a: Args) -> R {
    let t = match a.p.first() {
        None | Some(V::None) => now_us().div_euclid(1_000_000) * 1_000_000,
        // gmtime drops the fraction of a second, rounding down.
        Some(V::Float(f)) if f.is_finite() => f.floor() as i128 * 1_000_000,
        Some(v) => ts_us(v)?,
    };
    let d = DT::from_utc_us(EPOCH_US + t, None).map_err(|_| exc("OverflowError", "timestamp out of range for platform time_t".into()))?;
    let (y, m, dd) = d.ymd();
    let (h, mi, sec, _) = d.hms();
    let yday = days_before_month(y, m) + dd;
    let vals = [y, m, dd, h, mi, sec, d.weekday(), yday, 0].iter().map(|x| V::Int(*x)).collect();
    let o = call(&struct_time_class()?, Args::p(vals))?;
    if let V::Obj(x) = &o {
        x.set("tm_zone", s("GMT"));
        x.set("tm_gmtoff", V::Int(0));
    }
    Ok(o)
}
fn time_strftime(a: Args) -> R {
    let f = to_str(a.p.first().unwrap_or(&V::None))?;
    // Without a time tuple strftime uses local time, which dynamic builds do not know.
    let Some(t) = a.p.get(1) else { return Err(unsupported("time.strftime without a time tuple (local time)")) };
    let v = to_vec(t)?;
    if v.len() != 9 {
        return type_err("time.struct_time() takes a 9-sequence".into());
    }
    let mut n = [0i64; 9];
    for (k, x) in v.iter().enumerate() {
        n[k] = match x {
            V::Int(i) => *i,
            V::Bool(b) => *b as i64,
            x => return type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
        };
    }
    let (y, m, d, h, mi, sec) = (n[0], n[1], n[2], n[3], n[4], n[5]);
    if !(1..=12).contains(&m) {
        return val_err("month out of range".into());
    }
    if d < 1 || d > 31 || check_date(y, m, d).is_err() {
        return val_err("day of month out of range".into());
    }
    if !(0..=23).contains(&h) || !(0..=59).contains(&mi) || !(0..=61).contains(&sec) {
        return val_err("time out of range".into());
    }
    // %z and %Z come from the tuple's own zone: gmtime's is UTC. A plain tuple's would be local time.
    let tz = match t {
        V::Obj(o) => match o.get("tm_gmtoff") {
            Some(V::Int(g)) => Some(g as i32),
            _ => None,
        },
        _ => None,
    };
    if tz.is_none() && (f.contains("%z") || f.contains("%Z")) {
        return Err(unsupported("time.strftime %z or %Z of a local time"));
    }
    // %Z is the tuple's tm_zone (GMT for gmtime), not datetime's UTC name.
    let zone = match t {
        V::Obj(o) => o.get("tm_zone").map(|z| to_str(&z)).transpose()?.unwrap_or_default(),
        _ => String::new(),
    };
    let mut f2 = String::new();
    let mut it = f.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            f2.push(c);
            continue;
        }
        match it.next() {
            Some('Z') => f2.push_str(&zone.replace('%', "%%")),
            Some(k) => {
                f2.push('%');
                f2.push(k);
            }
            None => f2.push('%'),
        }
    }
    let dt = DT { ord: ymd_to_ord(y, m, d), us: ((h * 60 + mi) * 60 + sec.min(59)) * 1_000_000, has_time: true, tz: tz.map(Tz::F), fold: false };
    Ok(st(dt.strftime(&f2)))
}
