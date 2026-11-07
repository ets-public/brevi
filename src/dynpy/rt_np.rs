// ---------- numpy, natively ----------
// numpy arrays and scalars for dynamic builds: float64, float32, int64, int32 and bool arrays of any shape over a
// shared buffer (slices are views, as in numpy), numpy 2's type promotion (NEP 50: Python numbers are "weak"),
// broadcasting, ufuncs, reductions in numpy's summation order (pairwise along the contiguous axis), indexing (basic,
// boolean and integer arrays) and numpy's printing (rt_npfmt.rs). What it does not do is refused when the program
// is built (emit.rs np_* tables), so the program goes to CPython's numpy instead; a value it does not know at run
// time (a string array) raises NotImplementedError naming it. float32 and int32 values reach only what
// np_narrow_ok lists (checked where calls come in); the rest refuses them.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NT {
    B,
    I,
    F,
    I32,
    F32,
    // numpy's fixed-width unicode strings (<U n): n characters
    U(u32),
}
impl NT {
    pub fn name(self) -> &'static str {
        match self {
            NT::B => "bool",
            NT::I => "int64",
            NT::F => "float64",
            NT::I32 => "int32",
            NT::F32 => "float32",
            NT::U(_) => "str",
        }
    }
    // the name numpy's repr gives: '<U9' for strings
    pub fn full_name(self) -> String {
        match self {
            NT::U(w) => format!("<U{w}"),
            d => d.name().to_string(),
        }
    }
    pub fn is_u(self) -> bool {
        matches!(self, NT::U(_))
    }
    fn size(self) -> i64 {
        match self {
            NT::B => 1,
            NT::I32 | NT::F32 => 4,
            NT::U(w) => 4 * w as i64,
            _ => 8,
        }
    }
    pub fn is_f(self) -> bool {
        matches!(self, NT::F | NT::F32)
    }
    pub fn is_i(self) -> bool {
        matches!(self, NT::I | NT::I32)
    }
    pub fn narrow(self) -> bool {
        matches!(self, NT::I32 | NT::F32)
    }
    // the kind: bool < integer < float (what same_kind casting compares)
    pub fn kind(self) -> u8 {
        match self {
            NT::B => 0,
            NT::I | NT::I32 => 1,
            NT::U(_) => 3,
            _ => 2,
        }
    }
    // the characters numpy's str() of a value of this type can take (astype(str), np.array([1, "a"]))
    pub fn str_width(self) -> u32 {
        match self {
            NT::B => 5,
            NT::I => 21,
            NT::I32 => 11,
            NT::F | NT::F32 => 32,
            NT::U(w) => w,
        }
    }
    // the default type of numpy (float64, int64, bool): repr leaves its dtype out
    pub fn default_dt(self) -> bool {
        matches!(self, NT::B | NT::I | NT::F)
    }
}
// numpy's promotion of two types (np.result_type).
pub fn np_promote(a: NT, b: NT) -> NT {
    use NT::*;
    match (a, b) {
        (x, y) if x == y => x,
        // strings take the wider width; a number among strings becomes its str() (np.array([1, "a"]))
        // (written without an or-pattern: rustc 1.99's code for `(U(x), y) | (y, U(x))` jumped out of
        // its jump table here)
        (U(x), _) => U(x.max(b.str_width())),
        (_, U(y)) => U(y.max(a.str_width())),
        (B, y) | (y, B) => y,
        (I32, I) | (I, I32) => I,
        (F32, F) | (F, F32) => F,
        _ => F,
    }
}
pub fn np_promote_all(it: impl Iterator<Item = NT>) -> Option<NT> {
    it.reduce(np_promote)
}

#[derive(Clone, Copy, Debug)]
pub enum Sc {
    B(bool),
    I(i64),
    F(f64),
    I32(i32),
    F32(f32),
    // a string, by its number in np_strs (so Sc stays Copy)
    U(u32),
}
thread_local! {
    static NP_STRS: RefCell<(Vec<Rc<str>>, HashMap<Rc<str>, u32>)> = RefCell::new((vec![Rc::from("")], HashMap::from([(Rc::from(""), 0u32)])));
}
// The number of a string in the table of numpy string values (each distinct string is kept once).
pub fn np_sid(x: &str) -> u32 {
    NP_STRS.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(i) = t.1.get(x) {
            return *i;
        }
        let r: Rc<str> = Rc::from(x);
        let i = t.0.len() as u32;
        t.0.push(r.clone());
        t.1.insert(r, i);
        i
    })
}
pub fn np_sget(i: u32) -> Rc<str> {
    NP_STRS.with(|t| t.borrow().0[i as usize].clone())
}
// A string cut to w characters, as numpy stores it in a <U w array.
pub fn np_strunc(i: u32, w: u32) -> u32 {
    let s = np_sget(i);
    if s.len() <= w as usize || s.chars().count() <= w as usize {
        return i;
    }
    np_sid(&s.chars().take(w as usize).collect::<String>())
}
pub fn np_nchars(i: u32) -> u32 {
    np_sget(i).chars().count() as u32
}
// numpy's str() of a number stored in a string array (astype(str), a[i] = 5 in a string array)
pub fn np_num_str(x: Sc) -> String {
    match x {
        Sc::U(i) => np_sget(i).to_string(),
        Sc::B(b) => (if b { "True" } else { "False" }).into(),
        Sc::I(i) => i.to_string(),
        Sc::I32(i) => i.to_string(),
        Sc::F(f) => float_repr(f),
        Sc::F32(f) => np_f32_str(f),
    }
}
impl Sc {
    pub fn dt(self) -> NT {
        match self {
            Sc::B(_) => NT::B,
            Sc::I(_) => NT::I,
            Sc::F(_) => NT::F,
            Sc::I32(_) => NT::I32,
            Sc::F32(_) => NT::F32,
            Sc::U(i) => NT::U(np_nchars(i)),
        }
    }
    // (a string's number: its float() when it has one; casts from strings are checked first, np_str_cast_check)
    pub fn f(self) -> f64 {
        match self {
            Sc::B(b) => b as i64 as f64,
            Sc::I(i) => i as f64,
            Sc::F(x) => x,
            Sc::I32(i) => i as f64,
            Sc::F32(x) => x as f64,
            Sc::U(i) => np_parse_f(&np_sget(i)).unwrap_or(f64::NAN),
        }
    }
    pub fn i(self) -> i64 {
        match self {
            Sc::B(b) => b as i64,
            Sc::I(i) => i,
            Sc::F(x) => f_to_i(x),
            Sc::I32(i) => i as i64,
            Sc::F32(x) => f_to_i(x as f64),
            Sc::U(i) => np_parse_i(&np_sget(i)).unwrap_or(0),
        }
    }
    pub fn b(self) -> bool {
        match self {
            Sc::B(b) => b,
            Sc::I(i) => i != 0,
            Sc::F(x) => x != 0.0,
            Sc::I32(i) => i != 0,
            Sc::F32(x) => x != 0.0,
            Sc::U(i) => i != 0,
        }
    }
    // the C casts numpy's astype makes (an integer becomes float32 in one rounding)
    pub fn f32v(self) -> f32 {
        match self {
            Sc::B(b) => b as i32 as f32,
            Sc::I(i) => i as f32,
            Sc::I32(i) => i as f32,
            Sc::F32(x) => x,
            Sc::F(x) => f_to_f32(x),
            Sc::U(_) => f_to_f32(self.f()),
        }
    }
    pub fn i32v(self) -> i32 {
        match self {
            Sc::B(b) => b as i32,
            Sc::I(i) => i as i32,
            Sc::I32(i) => i,
            Sc::F(x) => f_to_i32(x),
            Sc::F32(x) => f_to_i32(x as f64),
            Sc::U(_) => self.i() as i32,
        }
    }
    // the string number of this value in a <U w array
    pub fn uv(self, w: u32) -> u32 {
        match self {
            Sc::U(i) => np_strunc(i, w),
            x => np_strunc(np_sid(&np_num_str(x)), w),
        }
    }
    pub fn cast(self, d: NT) -> Sc {
        match d {
            NT::B => Sc::B(self.b()),
            NT::I => Sc::I(self.i()),
            NT::F => Sc::F(self.f()),
            NT::I32 => Sc::I32(self.i32v()),
            NT::F32 => Sc::F32(self.f32v()),
            NT::U(w) => Sc::U(self.uv(w)),
        }
    }
}
// a string array's item cast to a number, with Python's errors (numpy's casts call int() and float())
pub fn np_str_int(i: u32) -> Result<i64, E> {
    let s = np_sget(i);
    np_parse_i(&s).ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base 10: np.str_({})", str_repr(&s))))
}
pub fn np_str_float(i: u32) -> Result<f64, E> {
    let s = np_sget(i);
    np_parse_f(&s).ok_or_else(|| exc("ValueError", format!("could not convert string to float: np.str_({})", str_repr(&s))))
}
// Before casting string values to a number type: each must parse (numpy raises otherwise); to bool is refused.
pub fn np_str_cast_check(b: &Buf, d: NT) -> Result<(), E> {
    if let Buf::U(v, _) = b {
        match d {
            NT::U(_) => {}
            NT::B => return Err(np_unsup("casting strings to bool")),
            NT::I | NT::I32 => {
                for i in v {
                    let x = np_str_int(*i)?;
                    if d == NT::I32 {
                        py_i32(x)?;
                    }
                }
            }
            NT::F | NT::F32 => {
                for i in v {
                    np_str_float(*i)?;
                }
            }
        }
    }
    Ok(())
}
// float() and int() of a string as numpy's casts read it (Python's float()/int() rules); None when it is not one
pub fn np_parse_f(s: &str) -> Option<f64> {
    parse_float(s)
}
pub fn np_parse_i(s: &str) -> Option<i64> {
    match to_int(&V::Str(Rc::new(S { s: s.to_string(), ascii: s.is_ascii() }))) {
        Ok(V::Int(i)) => Some(i),
        _ => None,
    }
}
fn f_to_i(x: f64) -> i64 {
    if !(x >= -9.223372036854775808e18 && x < 9.223372036854775808e18) {
        np_warn("invalid value encountered in cast");
        return i64::MIN;
    }
    x as i64
}
fn f_to_i32(x: f64) -> i32 {
    // C's (int)x on x86: out of range (and NaN) is INT_MIN
    if !(x > -2147483649.0 && x < 2147483648.0) {
        np_warn("invalid value encountered in cast");
        return i32::MIN;
    }
    x as i32
}
fn f_to_f32(x: f64) -> f32 {
    let r = x as f32;
    if r.is_infinite() && x.is_finite() {
        np_warn("overflow encountered in cast");
    }
    r
}

#[derive(Clone)]
pub enum Buf {
    B(Vec<bool>),
    I(Vec<i64>),
    F(Vec<f64>),
    I32(Vec<i32>),
    F32(Vec<f32>),
    // string numbers (np_sget) and the width in characters
    U(Vec<u32>, u32),
}
// the same expression over each kind of buffer (v is the Vec, and W the variant)
macro_rules! buf_map {
    ($b:expr, $v:ident => $e:expr) => {
        match $b {
            Buf::B($v) => Buf::B($e),
            Buf::I($v) => Buf::I($e),
            Buf::F($v) => Buf::F($e),
            Buf::I32($v) => Buf::I32($e),
            Buf::F32($v) => Buf::F32($e),
            Buf::U($v, w) => Buf::U($e, *w),
        }
    };
}
impl Buf {
    pub fn dt(&self) -> NT {
        match self {
            Buf::B(_) => NT::B,
            Buf::I(_) => NT::I,
            Buf::F(_) => NT::F,
            Buf::I32(_) => NT::I32,
            Buf::F32(_) => NT::F32,
            Buf::U(_, w) => NT::U(*w),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Buf::B(v) => v.len(),
            Buf::I(v) => v.len(),
            Buf::F(v) => v.len(),
            Buf::I32(v) => v.len(),
            Buf::F32(v) => v.len(),
            Buf::U(v, _) => v.len(),
        }
    }
    pub fn get(&self, p: usize) -> Sc {
        match self {
            Buf::B(v) => Sc::B(v[p]),
            Buf::I(v) => Sc::I(v[p]),
            Buf::F(v) => Sc::F(v[p]),
            Buf::I32(v) => Sc::I32(v[p]),
            Buf::F32(v) => Sc::F32(v[p]),
            Buf::U(v, _) => Sc::U(v[p]),
        }
    }
    pub fn set(&mut self, p: usize, x: Sc) {
        match self {
            Buf::B(v) => v[p] = x.b(),
            Buf::I(v) => v[p] = x.i(),
            Buf::F(v) => v[p] = x.f(),
            Buf::I32(v) => v[p] = x.i32v(),
            Buf::F32(v) => v[p] = x.f32v(),
            Buf::U(v, w) => v[p] = x.uv(*w),
        }
    }
    pub fn zeros(d: NT, n: usize) -> Buf {
        match d {
            NT::B => Buf::B(vec![false; n]),
            NT::I => Buf::I(vec![0; n]),
            NT::F => Buf::F(vec![0.0; n]),
            NT::I32 => Buf::I32(vec![0; n]),
            NT::F32 => Buf::F32(vec![0.0; n]),
            NT::U(w) => Buf::U(vec![0; n], w.max(1)),
        }
    }
    pub fn from_sc(d: NT, xs: &[Sc]) -> Buf {
        match d {
            NT::B => Buf::B(xs.iter().map(|x| x.b()).collect()),
            NT::I => Buf::I(xs.iter().map(|x| x.i()).collect()),
            NT::F => Buf::F(xs.iter().map(|x| x.f()).collect()),
            NT::I32 => Buf::I32(xs.iter().map(|x| x.i32v()).collect()),
            NT::F32 => Buf::F32(xs.iter().map(|x| x.f32v()).collect()),
            NT::U(w) => Buf::U(xs.iter().map(|x| x.uv(w)).collect(), w),
        }
    }
    // one value n times
    pub fn fill(x: Sc, n: usize) -> Buf {
        match x {
            Sc::B(v) => Buf::B(vec![v; n]),
            Sc::I(v) => Buf::I(vec![v; n]),
            Sc::F(v) => Buf::F(vec![v; n]),
            Sc::I32(v) => Buf::I32(vec![v; n]),
            Sc::F32(v) => Buf::F32(vec![v; n]),
            Sc::U(v) => Buf::U(vec![v; n], np_nchars(v)),
        }
    }
    pub fn pick(&self, ps: &[usize]) -> Buf {
        buf_map!(self, v => ps.iter().map(|p| v[*p]).collect())
    }
    pub fn slice(&self, a: usize, n: usize) -> Buf {
        buf_map!(self, v => v[a..a + n].to_vec())
    }
    pub fn cast(self, d: NT) -> Buf {
        if self.dt() == d {
            return self;
        }
        match (self, d) {
            (Buf::B(v), NT::I) => Buf::I(v.into_iter().map(|b| b as i64).collect()),
            (Buf::B(v), NT::F) => Buf::F(v.into_iter().map(|b| b as i64 as f64).collect()),
            (Buf::I(v), NT::F) => Buf::F(v.into_iter().map(|i| i as f64).collect()),
            (Buf::I(v), NT::B) => Buf::B(v.into_iter().map(|i| i != 0).collect()),
            (Buf::F(v), NT::B) => Buf::B(v.into_iter().map(|x| x != 0.0).collect()),
            (Buf::F(v), NT::I) => Buf::I(v.into_iter().map(f_to_i).collect()),
            (Buf::I32(v), NT::I) => Buf::I(v.into_iter().map(|i| i as i64).collect()),
            (Buf::I32(v), NT::F) => Buf::F(v.into_iter().map(|i| i as f64).collect()),
            (Buf::F32(v), NT::F) => Buf::F(v.into_iter().map(|x| x as f64).collect()),
            (Buf::F(v), NT::F32) => Buf::F32(v.into_iter().map(f_to_f32).collect()),
            (Buf::I(v), NT::I32) => Buf::I32(v.into_iter().map(|i| i as i32).collect()),
            (b, d) => {
                let n = b.len();
                Buf::from_sc(d, &(0..n).map(|i| b.get(i)).collect::<Vec<_>>())
            }
        }
    }
    // the values as a slice of one type, borrowed when already that type
    pub fn fc(&self) -> std::borrow::Cow<'_, [f64]> {
        match self {
            Buf::F(v) => std::borrow::Cow::Borrowed(&v[..]),
            b => std::borrow::Cow::Owned(b.clone().fv()),
        }
    }
    pub fn ic(&self) -> std::borrow::Cow<'_, [i64]> {
        match self {
            Buf::I(v) => std::borrow::Cow::Borrowed(&v[..]),
            b => std::borrow::Cow::Owned(b.clone().iv()),
        }
    }
    pub fn bc(&self) -> std::borrow::Cow<'_, [bool]> {
        match self {
            Buf::B(v) => std::borrow::Cow::Borrowed(&v[..]),
            b => std::borrow::Cow::Owned(b.clone().bv()),
        }
    }
    pub fn fv(self) -> Vec<f64> {
        match self.cast(NT::F) {
            Buf::F(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn iv(self) -> Vec<i64> {
        match self.cast(NT::I) {
            Buf::I(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn bv(self) -> Vec<bool> {
        match self.cast(NT::B) {
            Buf::B(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn f32v(self) -> Vec<f32> {
        match self.cast(NT::F32) {
            Buf::F32(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn i32v(self) -> Vec<i32> {
        match self.cast(NT::I32) {
            Buf::I32(v) => v,
            _ => unreachable!(),
        }
    }
}

pub fn cstrides(shape: &[usize]) -> Vec<isize> {
    let mut st = vec![1isize; shape.len()];
    for k in (0..shape.len().saturating_sub(1)).rev() {
        st[k] = st[k + 1] * shape[k + 1].max(1) as isize;
    }
    st
}

// Positions in the buffer of the elements of (off, shape, strides), in C order.
fn walk_pos(off: usize, shape: &[usize], st: &[isize], out: &mut Vec<usize>) {
    let n: usize = shape.iter().product();
    if n == 0 {
        return;
    }
    out.reserve(n);
    let nd = shape.len();
    if nd == 0 {
        out.push(off);
        return;
    }
    let mut idx = vec![0usize; nd];
    let mut p = off as isize;
    let last = nd - 1;
    let (ln, ls) = (shape[last], st[last]);
    loop {
        let mut q = p;
        for _ in 0..ln {
            out.push(q as usize);
            q += ls;
        }
        let mut d = last;
        loop {
            if d == 0 {
                return;
            }
            d -= 1;
            idx[d] += 1;
            p += st[d];
            if idx[d] < shape[d] {
                break;
            }
            p -= st[d] * shape[d] as isize;
            idx[d] = 0;
        }
    }
}

#[derive(Clone)]
pub struct NdArr {
    pub buf: Rc<RefCell<Buf>>,
    pub off: usize,
    pub shape: Vec<usize>,
    pub st: Vec<isize>,
}
impl NdArr {
    pub fn new(b: Buf, shape: Vec<usize>) -> NdArr {
        let st = cstrides(&shape);
        NdArr { buf: Rc::new(RefCell::new(b)), off: 0, shape, st }
    }
    pub fn size(&self) -> usize {
        self.shape.iter().product()
    }
    pub fn dt(&self) -> NT {
        self.buf.borrow().dt()
    }
    pub fn contig(&self) -> bool {
        let c = cstrides(&self.shape);
        self.shape.iter().zip(self.st.iter().zip(&c)).all(|(n, (s, cs))| *n <= 1 || s == cs)
    }
    pub fn pos(&self) -> Vec<usize> {
        let mut v = vec![];
        walk_pos(self.off, &self.shape, &self.st, &mut v);
        v
    }
    pub fn vals(&self) -> Buf {
        let b = self.buf.borrow();
        if self.contig() {
            return b.slice(self.off, self.size());
        }
        b.pick(&self.pos())
    }
    pub fn fv(&self) -> Vec<f64> {
        self.vals().fv()
    }
    pub fn iv(&self) -> Vec<i64> {
        self.vals().iv()
    }
    pub fn bv(&self) -> Vec<bool> {
        self.vals().bv()
    }
    pub fn first(&self) -> Sc {
        self.buf.borrow().get(self.off)
    }
    pub fn copy(&self) -> NdArr {
        NdArr::new(self.vals(), self.shape.clone())
    }
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
    // A view of the same values with another shape (the values are copied when this array is not contiguous).
    pub fn reshaped(&self, shape: Vec<usize>) -> NdArr {
        if self.contig() {
            return NdArr { buf: self.buf.clone(), off: self.off, st: cstrides(&shape), shape };
        }
        NdArr::new(self.vals(), shape)
    }
}

pub struct NpArr(pub NdArr);
pub struct NpScal(pub Sc);
pub struct NpDtype(pub NT);

pub fn np_arr(a: NdArr) -> V {
    V::Ext(Rc::new(NpArr(a)))
}
pub fn np_new(b: Buf, shape: Vec<usize>) -> V {
    np_arr(NdArr::new(b, shape))
}
pub fn np_sc(x: Sc) -> V {
    V::Ext(Rc::new(NpScal(x)))
}
// A 0-d result is a numpy scalar (as ufuncs and reductions return).
pub fn np_out(b: Buf, shape: Vec<usize>) -> V {
    if shape.is_empty() {
        return np_sc(b.get(0));
    }
    np_new(b, shape)
}
pub fn as_np(v: &V) -> Option<&NdArr> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<NpArr>().map(|a| &a.0),
        _ => None,
    }
}
pub fn as_npsc(v: &V) -> Option<Sc> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<NpScal>().map(|a| a.0),
        _ => None,
    }
}
pub fn as_npdt(v: &V) -> Option<NT> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<NpDtype>().map(|a| a.0),
        _ => None,
    }
}
pub fn is_np(v: &V) -> bool {
    as_np(v).is_some() || as_npsc(v).is_some()
}

pub fn np_unsup(what: &str) -> E {
    exc("NotImplementedError", format!("{what} is not supported by Brevi's native numpy yet"))
}

thread_local! {
    static NP_WARNED: RefCell<Vec<String>> = RefCell::new(vec![]);
}
// numpy's RuntimeWarning (shown once per message).
pub fn np_warn(msg: &str) {
    let first = NP_WARNED.with(|w| {
        let mut w = w.borrow_mut();
        if w.iter().any(|m| m == msg) {
            return false;
        }
        w.push(msg.to_string());
        true
    });
    if first {
        out_flush();
        err_write(&format!("RuntimeWarning: {msg}\n"));
    }
}

// dtype= arguments: np.float64, float, "float64", a dtype, ...
pub fn np_dtype_arg(v: &V) -> Result<Option<NT>, E> {
    if let Some(d) = as_npdt(v) {
        return Ok(Some(d));
    }
    let name = match v {
        V::None => return Ok(None),
        V::Class(c) => c.name.to_string(),
        V::Str(s) => s.s.clone(),
        _ => return type_err(format!("Cannot interpret '{}' as a data type", repr(v)?)),
    };
    Ok(Some(match name.as_str() {
        "float" | "numpy.float64" | "float64" | "f8" | "d" | "double" | "<f8" | "numpy.double" => NT::F,
        "int" | "numpy.int64" | "int64" | "i8" | "<i8" | "numpy.int_" | "int_" | "long" | "numpy.intp" | "intp" => NT::I,
        "bool" | "numpy.bool" | "numpy.bool_" | "bool_" | "?" | "b1" => NT::B,
        "numpy.float32" | "float32" | "f4" | "<f4" | "f" | "single" | "numpy.single" => NT::F32,
        "numpy.int32" | "int32" | "i4" | "<i4" | "i" | "intc" | "numpy.intc" => NT::I32,
        // strings; U(0) is "as wide as needed" (each use sizes it)
        "str" | "numpy.str_" | "str_" | "U" | "<U" | "unicode" => NT::U(0),
        n if (n.starts_with("<U") || n.starts_with('U')) && n.trim_start_matches(['<', 'U']).parse::<u32>().is_ok() => NT::U(n.trim_start_matches(['<', 'U']).parse().unwrap()),
        n => return Err(np_unsup(&format!("dtype {n}"))),
    }))
}

// ---------- from Python values ----------

fn py_sc(v: &V) -> Result<Option<Sc>, E> {
    Ok(Some(match v {
        V::Bool(b) => Sc::B(*b),
        V::Int(i) => Sc::I(*i),
        V::Float(f) => Sc::F(*f),
        V::Big(b) => return Err(exc("OverflowError", format!("Python integer {} out of bounds for int64", b.to_string()))),
        _ => match as_npsc(v) {
            Some(s) => s,
            None => return Ok(None),
        },
    }))
}

// A Python integer stored in an int32: numpy refuses one that does not fit.
pub fn py_i32(v: i64) -> Result<i32, E> {
    if v < i32::MIN as i64 || v > i32::MAX as i64 {
        return Err(exc("OverflowError", format!("Python integer {v} out of bounds for int32")));
    }
    Ok(v as i32)
}

fn seq_items(v: &V) -> Option<Vec<V>> {
    match v {
        V::List(l) => Some(l.borrow().clone()),
        V::Tuple(t) => Some((**t).clone()),
        V::Range(_) => to_vec(v).ok(),
        _ => None,
    }
}

fn np_walk(v: &V, depth: usize, shape: &mut Vec<usize>, out: &mut Vec<Sc>) -> Result<(), E> {
    let ragged = |d: usize, shape: &Vec<usize>| -> E {
        exc(
            "ValueError",
            format!("setting an array element with a sequence. The requested array has an inhomogeneous shape after {d} dimensions. The detected shape was {} + inhomogeneous part.", np_shape_repr(&shape[..d.min(shape.len())])),
        )
    };
    if let Some(a) = as_np(v) {
        let want = &a.shape;
        if depth + want.len() > shape.len() && depth == shape.len() && out.is_empty() {
            shape.extend(want.iter().cloned());
        }
        if shape.len() < depth + want.len() || &shape[depth..depth + want.len()] != &want[..] || shape.len() != depth + want.len() {
            return Err(ragged(depth, shape));
        }
        let b = a.vals();
        for i in 0..b.len() {
            out.push(b.get(i));
        }
        return Ok(());
    }
    if let Some(items) = seq_items(v) {
        if depth == shape.len() {
            if !out.is_empty() {
                return Err(ragged(depth, shape));
            }
            shape.push(items.len());
        } else if shape[depth] != items.len() {
            return Err(ragged(depth, shape));
        }
        for x in &items {
            np_walk(x, depth + 1, shape, out)?;
        }
        return Ok(());
    }
    if depth != shape.len() {
        return Err(ragged(depth, shape));
    }
    match py_sc(v)? {
        Some(s) => out.push(s),
        None if matches!(v, V::Str(_)) => {
            let V::Str(x) = v else { unreachable!() };
            out.push(Sc::U(np_sid(&x.s)));
        }
        None => {
            return Err(match v {
                V::None => np_unsup("an array holding None"),
                _ => np_unsup(&format!("an array of {}", tn(v))),
            })
        }
    }
    Ok(())
}

// np.array(v, dtype): always a new array.
pub fn np_from(v: &V, dt: Option<NT>) -> Result<NdArr, E> {
    if let Some(a) = as_np(v) {
        let b = a.vals();
        return Ok(NdArr::new(match dt {
            Some(d) => {
                np_str_cast_check(&b, d)?;
                b.cast(d)
            }
            None => b,
        }, a.shape.clone()));
    }
    let mut shape = vec![];
    let mut out = vec![];
    np_walk(v, 0, &mut shape, &mut out)?;
    let d = dt.unwrap_or_else(|| np_promote_all(out.iter().map(|x| x.dt())).unwrap_or(NT::F));
    // dtype=str with no width: the widest item's str() (np.array([1, 22], dtype=str) is <U2)
    // (an array of "" is still <U1)
    let d = if d == NT::U(0) && dt.is_none() { NT::U(1) } else { d };
    let d = if d == NT::U(0) && dt.is_some() { NT::U(out.iter().map(|x| np_nchars(x.uv(u32::MAX))).max().unwrap_or(1).max(1)) } else { d };
    if !d.is_u() && out.iter().any(|x| matches!(x, Sc::U(_))) {
        let strs: Vec<u32> = out.iter().filter_map(|x| if let Sc::U(i) = x { Some(*i) } else { None }).collect();
        np_str_cast_check(&Buf::U(strs, 0), d)?;
    }
    if dt == Some(NT::I) || dt == Some(NT::B) {
        if let (Some(NT::I), Some(x)) = (dt, out.iter().find(|x| matches!(x, Sc::F(f) if !f.is_finite()))) {
            let _ = x;
            return Err(exc("ValueError", "cannot convert float NaN to integer".into()));
        }
    }
    // numpy stores each item of a list through Python's int() and float(): an int32 refuses what does not fit,
    // and a float32 rounds each number from its double
    if d == NT::I32 {
        let mut b = Vec::with_capacity(out.len());
        for x in &out {
            b.push(match *x {
                Sc::B(v) => v as i32,
                Sc::I32(v) => v,
                Sc::I(v) => py_i32(v)?,
                Sc::U(i) => py_i32(np_str_int(i)?)?,
                Sc::F(_) | Sc::F32(_) => {
                    let f = x.f();
                    if f.is_nan() {
                        return val_err("cannot convert float NaN to integer".into());
                    }
                    if f.is_infinite() {
                        return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
                    }
                    let t = f.trunc();
                    if !(t >= -2147483648.0 && t <= 2147483647.0) {
                        return Err(exc("OverflowError", format!("Python integer {} out of bounds for int32", repr(&to_int(&V::Float(t))?)?)));
                    }
                    t as i32
                }
            });
        }
        return Ok(NdArr::new(Buf::I32(b), shape));
    }
    if d == NT::F32 {
        return Ok(NdArr::new(Buf::F32(out.iter().map(|x| f_to_f32(x.f())).collect()), shape));
    }
    Ok(NdArr::new(Buf::from_sc(d, &out), shape))
}
// np.asarray: the same array when it already is one.
pub fn np_as(v: &V) -> Result<NdArr, E> {
    match as_np(v) {
        Some(a) => Ok(a.clone()),
        None => np_from(v, None),
    }
}

// ---------- broadcasting ----------

pub fn np_bshape(a: &[usize], b: &[usize]) -> Result<Vec<usize>, E> {
    let n = a.len().max(b.len());
    let mut out = vec![0; n];
    for k in 0..n {
        let x = if k + a.len() >= n { a[k + a.len() - n] } else { 1 };
        let y = if k + b.len() >= n { b[k + b.len() - n] } else { 1 };
        out[k] = if x == y || y == 1 {
            x
        } else if x == 1 {
            y
        } else {
            let f = |s: &[usize]| format!("({})", s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",") + if s.len() == 1 { "," } else { "" });
            return val_err(format!("operands could not be broadcast together with shapes {} {} ", f(a), f(b)));
        };
    }
    Ok(out)
}
// f over a's float values for an array of shape `out`: borrowed when a is a contiguous float
// array of that shape, one value when a has one value, a broadcast copy otherwise.
pub fn np_with_f<T>(a: &NdArr, out: &[usize], f: impl FnOnce(&[f64]) -> T) -> T {
    let n: usize = out.iter().product();
    if a.size() == 1 && n > 1 {
        return f(&[a.first().f()]);
    }
    if a.shape == out && a.contig() {
        let b = a.buf.borrow();
        if let Buf::F(v) = &*b {
            return f(&v[a.off..a.off + n]);
        }
    }
    let v = np_bvals(a, out).fv();
    f(&v)
}
// a's values for each element of an array of shape `out` (a broadcast to it).
pub fn np_bvals(a: &NdArr, out: &[usize]) -> Buf {
    if a.shape == out {
        return a.vals();
    }
    if a.size() == 1 {
        let n: usize = out.iter().product();
        return Buf::fill(a.first(), n);
    }
    let n = out.len();
    let st: Vec<isize> = (0..n)
        .map(|k| {
            if k + a.shape.len() < n {
                return 0;
            }
            let j = k + a.shape.len() - n;
            if a.shape[j] == 1 {
                0
            } else {
                a.st[j]
            }
        })
        .collect();
    let mut ps = vec![];
    walk_pos(a.off, out, &st, &mut ps);
    a.buf.borrow().pick(&ps)
}
pub fn np_broadcast_to(a: &NdArr, out: &[usize]) -> Result<Buf, E> {
    let s = np_bshape(&a.shape, out)?;
    if s != out {
        return val_err(format!("could not broadcast input array from shape {} into shape {}", np_shape_repr(&a.shape), np_shape_repr(out)));
    }
    Ok(np_bvals(a, out))
}

// An operand of a ufunc: the array, and whether it is a Python number (NEP 50: its type gives way).
pub struct Opnd {
    pub a: NdArr,
    pub weak: bool,
    pub scalar: bool,
}
pub fn np_opnd(v: &V) -> Result<Option<Opnd>, E> {
    if let Some(a) = as_np(v) {
        return Ok(Some(Opnd { a: a.clone(), weak: false, scalar: false }));
    }
    if let Some(s) = as_npsc(v) {
        return Ok(Some(Opnd { a: NdArr::new(Buf::from_sc(s.dt(), &[s]), vec![]), weak: false, scalar: true }));
    }
    if let Some(s) = py_sc(v)? {
        return Ok(Some(Opnd { a: NdArr::new(Buf::from_sc(s.dt(), &[s]), vec![]), weak: true, scalar: true }));
    }
    if let V::Str(x) = v {
        let i = np_sid(&x.s);
        return Ok(Some(Opnd { a: NdArr::new(Buf::fill(Sc::U(i), 1), vec![]), weak: true, scalar: true }));
    }
    if seq_items(v).is_some() {
        return Ok(Some(Opnd { a: np_from(v, None)?, weak: false, scalar: false }));
    }
    Ok(None)
}
// The result type of a ufunc over these operands.
pub fn np_rtype(ops: &[&Opnd]) -> NT {
    // strings: the widest (a Python str is as wide as itself)
    if ops.iter().any(|o| o.a.dt().is_u()) {
        return np_promote_all(ops.iter().map(|o| o.a.dt())).unwrap();
    }
    let strong = np_promote_all(ops.iter().filter(|o| !o.weak).map(|o| o.a.dt()));
    let mut d = strong.unwrap_or_else(|| np_promote_all(ops.iter().map(|o| o.a.dt())).unwrap_or(NT::F));
    for o in ops.iter().filter(|o| o.weak) {
        let w = o.a.dt();
        if w == NT::F && !d.is_f() {
            d = NT::F;
        } else if w == NT::I && d == NT::B {
            d = NT::I;
        }
    }
    d
}

fn rint(x: f64) -> f64 {
    let r = x.round();
    if (r - x).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}
fn np_fmod(a: f64, b: f64) -> f64 {
    a % b
}
// npy_divmod: (floor quotient, remainder) for doubles, as numpy computes them.
fn np_divmod(a: f64, b: f64) -> (f64, f64) {
    let mut m = np_fmod(a, b);
    if b == 0.0 {
        return (a / b, m);
    }
    let mut div = (a - m) / b;
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) {
            m += b;
            div -= 1.0;
        }
    } else {
        m = 0.0f64.copysign(b);
    }
    let fd = if div != 0.0 {
        let f = div.floor();
        if div - f > 0.5 {
            f + 1.0
        } else {
            f
        }
    } else {
        0.0f64.copysign(a / b)
    };
    (fd, m)
}

fn ufunc_name(op: &str) -> &'static str {
    match op {
        "+" => "add",
        "-" => "subtract",
        "*" => "multiply",
        "/" => "divide",
        "//" => "floor_divide",
        "%" => "remainder",
        "**" => "power",
        "sq" => "square",
        "recip" => "reciprocal",
        "sqrt" => "sqrt",
        "&" => "bitwise_and",
        "|" => "bitwise_or",
        "^" => "bitwise_xor",
        "<<" => "left_shift",
        ">>" => "right_shift",
        "<" => "less",
        ">" => "greater",
        "<=" => "less_equal",
        ">=" => "greater_equal",
        "==" => "equal",
        "!=" => "not_equal",
        _ => "?",
    }
}
fn no_loop(op: &str) -> E {
    exc("TypeError", format!("ufunc '{}' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''", ufunc_name(op)))
}

// Float results: numpy's floating-point warnings (divide by zero, invalid value, overflow).
fn fwarn(op: &str, scalar: bool, x: &[f64], y: &[f64], r: &[f64]) {
    let (mut dz, mut inv, mut ov) = (false, false, false);
    for i in 0..r.len() {
        let (a, b, c) = (x[i], y[i], r[i]);
        if c.is_nan() && !a.is_nan() && !b.is_nan() {
            inv = true;
        } else if c.is_infinite() && a.is_finite() && b.is_finite() {
            if matches!(op, "/" | "//" | "%" | "recip") || (op == "**" && a == 0.0) {
                dz = true;
            } else {
                ov = true;
            }
        }
    }
    let sc = if scalar { "scalar " } else { "" };
    let name = ufunc_name(op);
    if dz {
        np_warn(&format!("divide by zero encountered in {sc}{name}"));
    }
    if ov {
        np_warn(&format!("overflow encountered in {sc}{name}"));
    }
    if inv {
        np_warn(&format!("invalid value encountered in {sc}{name}"));
    }
}

fn kern_f(op: &str, x: &[f64], y: &[f64]) -> Result<Buf, E> {
    // either side may be one value standing for all (a broadcast scalar)
    let n = x.len().max(y.len());
    let mut r: Vec<f64> = Vec::with_capacity(n);
    macro_rules! go {
        ($e:expr) => {{
            let f = $e;
            if x.len() == n && y.len() == n {
                r.extend(x.iter().zip(y).map(|(a, b)| f(*a, *b)));
            } else if y.len() == 1 && x.len() == n {
                let b = y[0];
                r.extend(x.iter().map(|a| f(*a, b)));
            } else if x.len() == 1 && y.len() == n {
                let a = x[0];
                r.extend(y.iter().map(|b| f(a, *b)));
            } else {
                for i in 0..n {
                    r.push(f(x[i], y[i]));
                }
            }
        }};
    }
    match op {
        "+" => go!(|a: f64, b: f64| a + b),
        "-" => go!(|a: f64, b: f64| a - b),
        "*" => go!(|a: f64, b: f64| a * b),
        "/" => go!(|a: f64, b: f64| a / b),
        "//" => go!(|a: f64, b: f64| if b == 0.0 { a / b } else { np_divmod(a, b).0 }),
        "%" => go!(|a: f64, b: f64| if b == 0.0 { np_fmod(a, b) } else { np_divmod(a, b).1 }),
        "**" => go!(|a: f64, b: f64| if b == 2.0 { a * a } else { a.powf(b) }),
        "<" | ">" | "<=" | ">=" | "==" | "!=" => {
            let f: fn(f64, f64) -> bool = match op {
                "<" => |a, b| a < b,
                ">" => |a, b| a > b,
                "<=" => |a, b| a <= b,
                ">=" => |a, b| a >= b,
                "==" => |a, b| a == b,
                _ => |a, b| a != b,
            };
            let o: Vec<bool> = if x.len() == n && y.len() == n {
                x.iter().zip(y).map(|(a, b)| f(*a, *b)).collect()
            } else if y.len() == 1 {
                let b = y[0];
                x.iter().map(|a| f(*a, b)).collect()
            } else {
                let a = x[0];
                y.iter().map(|b| f(a, *b)).collect()
            };
            return Ok(Buf::B(o));
        }
        _ => return Err(no_loop(op)),
    }
    Ok(Buf::F(r))
}
// int64 (bits 64) or int32 (bits 32: the values are int32s, and the results wrap to 32 bits as numpy's do)
fn kern_i(op: &str, x: &[i64], y: &[i64], scalar: bool, bits: u32) -> Result<Buf, E> {
    let n = x.len();
    let mut r = Vec::with_capacity(n);
    let sc = if scalar { "scalar " } else { "" };
    let mut ov = false;
    let mut dz = false;
    let wraps = bits == 32 && matches!(op, "+" | "-" | "*" | "//");
    for i in 0..n {
        let (a, b) = (x[i], y[i]);
        let v = match op {
            "+" => {
                let (v, o) = a.overflowing_add(b);
                ov |= o;
                v
            }
            "-" => {
                let (v, o) = a.overflowing_sub(b);
                ov |= o;
                v
            }
            "*" => {
                let (v, o) = a.overflowing_mul(b);
                ov |= o;
                v
            }
            "//" => {
                if b == 0 {
                    dz = true;
                    0
                } else if a == i64::MIN && b == -1 {
                    ov = true;
                    i64::MIN
                } else {
                    let q = a / b;
                    if (a % b != 0) && ((a < 0) != (b < 0)) {
                        q - 1
                    } else {
                        q
                    }
                }
            }
            "%" => {
                if b == 0 {
                    dz = true;
                    0
                } else if b == -1 {
                    0
                } else {
                    let m = a % b;
                    if m != 0 && ((m < 0) != (b < 0)) {
                        m + b
                    } else {
                        m
                    }
                }
            }
            "**" => {
                if b < 0 {
                    return val_err("Integers to negative integer powers are not allowed.".into());
                }
                let (mut base, mut e, mut acc) = (a, b as u64, 1i64);
                while e > 0 {
                    if e & 1 == 1 {
                        acc = acc.wrapping_mul(base);
                    }
                    base = base.wrapping_mul(base);
                    e >>= 1;
                }
                acc
            }
            "&" => a & b,
            "|" => a | b,
            "^" => a ^ b,
            "<<" => {
                if b < 0 || b >= 64 {
                    0
                } else {
                    a.wrapping_shl(b as u32)
                }
            }
            ">>" => {
                if b < 0 || b >= 64 {
                    if a < 0 {
                        -1
                    } else {
                        0
                    }
                } else {
                    a >> b
                }
            }
            "<" | ">" | "<=" | ">=" | "==" | "!=" => {
                let mut o = Vec::with_capacity(n);
                for i in 0..n {
                    let (a, b) = (x[i], y[i]);
                    o.push(match op {
                        "<" => a < b,
                        ">" => a > b,
                        "<=" => a <= b,
                        ">=" => a >= b,
                        "==" => a == b,
                        _ => a != b,
                    });
                }
                return Ok(Buf::B(o));
            }
            _ => return Err(no_loop(op)),
        };
        if wraps && v != v as i32 as i64 {
            ov = true;
        }
        r.push(v);
    }
    if dz {
        np_warn(&format!("divide by zero encountered in {sc}{}", ufunc_name(op)));
    }
    // numpy's integer loops report overflow for scalars, and for floor_divide (MIN // -1) on arrays too
    if ov && (scalar || op == "//") {
        np_warn(&format!("overflow encountered in {sc}{}", ufunc_name(op)));
    }
    if bits == 32 {
        return Ok(Buf::I32(r.into_iter().map(|v| v as i32).collect()));
    }
    Ok(Buf::I(r))
}
// float32 floor_divide, remainder and power, in float32 arithmetic (npy_divmodf, powf); +, -, *, / and
// comparisons are computed in float64 and rounded once, which gives float32's own results.
fn np_divmod32(a: f32, b: f32) -> (f32, f32) {
    let mut m = a % b;
    if b == 0.0 {
        return (a / b, m);
    }
    let mut div = (a - m) / b;
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) {
            m += b;
            div -= 1.0;
        }
    } else {
        m = 0.0f32.copysign(b);
    }
    let fd = if div != 0.0 {
        let f = div.floor();
        if div - f > 0.5 {
            f + 1.0
        } else {
            f
        }
    } else {
        0.0f32.copysign(a / b)
    };
    (fd, m)
}
fn kern_f32(op: &str, x: &[f32], y: &[f32]) -> Vec<f32> {
    let n = x.len().max(y.len());
    let at = |v: &[f32], i: usize| if v.len() == 1 { v[0] } else { v[i] };
    (0..n)
        .map(|i| {
            let (a, b) = (at(x, i), at(y, i));
            match op {
                "//" => if b == 0.0 { a / b } else { np_divmod32(a, b).0 },
                "%" => if b == 0.0 { a % b } else { np_divmod32(a, b).1 },
                "sq" => a * a,
                "recip" => 1.0 / a,
                "sqrt" => a.sqrt(),
                _ => a.powf(b),
            }
        })
        .collect()
}
fn kern_b(op: &str, x: &[bool], y: &[bool]) -> Result<Buf, E> {
    let f: fn(bool, bool) -> bool = match op {
        "+" | "|" => |a, b| a | b,
        "*" | "&" => |a, b| a & b,
        "^" | "!=" => |a, b| a ^ b,
        "==" => |a, b| a == b,
        "<" => |a, b| !a & b,
        ">" => |a, b| a & !b,
        "<=" => |a, b| !a | b,
        ">=" => |a, b| a | !b,
        "-" => return type_err("numpy boolean subtract, the `-` operator, is not supported, use the bitwise_xor, the `^` operator, or the logical_xor function instead.".into()),
        "/" => return kern_f(op, &x.iter().map(|b| *b as i64 as f64).collect::<Vec<_>>(), &y.iter().map(|b| *b as i64 as f64).collect::<Vec<_>>()),
        _ => return Err(np_unsup(&format!("'{op}' on two bool arrays (numpy gives int8)"))),
    };
    Ok(Buf::B(x.iter().zip(y).map(|(a, b)| f(*a, *b)).collect()))
}

// a OP b with numpy's rules, when either is a numpy value (None otherwise).
pub fn np_binop(a: &V, b: &V, op: &str) -> Option<R> {
    if !is_np(a) && !is_np(b) {
        return None;
    }
    if op == "@" {
        return Some(np_matmul(a, b));
    }
    Some((|| -> R {
        let (x, y) = match (np_opnd(a)?, np_opnd(b)?) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(exc("TypeError", format!("unsupported operand type(s) for {op}: '{}' and '{}'", tn(a), tn(b)))),
        };
        let scalar = x.scalar && y.scalar;
        if x.a.dt().is_u() || y.a.dt().is_u() {
            return np_str_binop(&x, &y, op);
        }
        let d = np_rtype(&[&x, &y]);
        let bitwise = matches!(op, "&" | "|" | "^" | "<<" | ">>");
        if (d.is_f() || op == "/") && bitwise {
            return Err(no_loop(op));
        }
        let shape = np_bshape(&x.a.shape, &y.a.shape)?;
        let cmp = matches!(op, "<" | ">" | "<=" | ">=" | "==" | "!=");
        // a Python int with an int32 must fit (comparisons compare the exact values instead)
        if d == NT::I32 && !cmp {
            for o in [&x, &y] {
                if o.weak && o.a.dt() == NT::I {
                    py_i32(o.a.first().i())?;
                }
            }
        }
        let r = if d == NT::F32 {
            // the operands as float32 (a Python number rounds to float32 first), one value kept as one
            let n: usize = shape.iter().product();
            let vals32 = |a: &NdArr| -> Vec<f32> { if a.size() == 1 && n != 1 { vec![a.first().f32v()] } else { np_bvals(a, &shape).f32v() } };
            let (xv, yv) = (vals32(&x.a), vals32(&y.a));
            let mut fop = op;
            let r: Buf = match op {
                "+" | "-" | "*" | "/" | "<" | ">" | "<=" | ">=" | "==" | "!=" => {
                    let w = |v: &[f32]| v.iter().map(|f| *f as f64).collect::<Vec<f64>>();
                    match kern_f(op, &w(&xv), &w(&yv))? {
                        Buf::F(v) => Buf::F32(v.into_iter().map(|f| f as f32).collect()),
                        b => b,
                    }
                }
                "//" | "%" => Buf::F32(kern_f32(op, &xv, &yv)),
                "**" => {
                    // an array to a scalar 2, -1 or 0.5 is numpy's square, reciprocal or sqrt (fast_scalar_power);
                    // a float32 scalar uses the C library's powf. Other array powers go through numpy's SIMD powf.
                    let fast = np_fast_pow(&x, &y);
                    match fast {
                        Some(f) => {
                            fop = f;
                            Buf::F32(kern_f32(f, &xv, &yv))
                        }
                        _ if x.scalar && y.scalar => Buf::F32(kern_f32("**", &xv, &yv)),
                        _ => return Err(np_unsup("a float32 array to this power")),
                    }
                }
                _ => return Err(no_loop(op)),
            };
            if let Buf::F32(rv) = &r {
                if rv.iter().any(|v| !v.is_finite()) {
                    let full = |v: &[f32]| -> Vec<f64> { if v.len() == rv.len() { v.iter().map(|f| *f as f64).collect() } else { vec![v[0] as f64; rv.len()] } };
                    fwarn(fop, scalar, &full(&xv), &full(&yv), &rv.iter().map(|f| *f as f64).collect::<Vec<_>>());
                }
            }
            r
        } else if d.is_f() || op == "/" {
            // a one-value side stays one value (the kernel spreads it); a float array of the
            // result's shape is read in place
            // a float array to a scalar 2, -1 or 0.5: numpy's square, reciprocal or sqrt (sqrt(-0.0) is -0.0)
            let fop = if op == "**" && x.a.dt().is_f() { np_fast_pow(&x, &y).unwrap_or(op) } else { op };
            np_with_f(&x.a, &shape, |xv| {
                np_with_f(&y.a, &shape, |yv| {
                    let r = match fop {
                        "sq" | "recip" | "sqrt" => {
                            let f: fn(f64) -> f64 = match fop {
                                "sq" => |a| a * a,
                                "recip" => |a| 1.0 / a,
                                _ => f64::sqrt,
                            };
                            Buf::F(xv.iter().map(|a| f(*a)).collect())
                        }
                        _ => kern_f(op, xv, yv)?,
                    };
                    if let Buf::F(rv) = &r {
                        if rv.iter().any(|v| !v.is_finite()) {
                            let full = |v: &[f64]| if v.len() == rv.len() { v.to_vec() } else { vec![v[0]; rv.len()] };
                            fwarn(fop, scalar, &full(xv), &full(yv), rv);
                        }
                    }
                    Ok(r)
                })
            })?
        } else if d.is_i() || matches!(op, "<<" | ">>") {
            let bits = if d == NT::I32 && !cmp { 32 } else { 64 };
            kern_i(op, &np_bvals(&x.a, &shape).iv(), &np_bvals(&y.a, &shape).iv(), scalar, bits)?
        } else {
            kern_b(op, &np_bvals(&x.a, &shape).bv(), &np_bvals(&y.a, &shape).bv())?
        };
        Ok(np_out(r, shape))
    })())
}

// numpy's fast_scalar_power: a float array (not a scalar) to a scalar exponent (a Python number, numpy scalar
// or 0-d array) of 2, -1 or 0.5 runs square, reciprocal or sqrt instead of power.
fn np_fast_pow(x: &Opnd, y: &Opnd) -> Option<&'static str> {
    // (not when the exponent widens the result: float32 ** np.float64(0.5) is float64 power)
    if x.scalar || y.a.ndim() != 0 || !x.a.dt().is_f() || np_rtype(&[x, y]) != x.a.dt() {
        return None;
    }
    let e = y.a.first();
    if e.dt() == NT::B {
        return None;
    }
    match e.f() {
        2.0 => Some("sq"),
        -1.0 => Some("recip"),
        0.5 => Some("sqrt"),
        _ => None,
    }
}

// numpy's own error for a function or method with no loop for strings (d is the string array's dtype)
pub fn np_str_noloop(n: &str, d: NT) -> Option<E> {
    let f = d.full_name();
    Some(match n {
        "sum" | "mean" | "std" | "var" | "median" | "average" | "nansum" | "nanmean" => exc("TypeError", format!("the resolved dtypes are not compatible with add.reduce. Resolved (dtype('{f}'), dtype('{f}'), dtype('{}'))", NT::U(d.str_width() * 2).full_name())),
        "cumsum" => exc("TypeError", format!("the resolved dtypes are not compatible with add.accumulate. Resolved (dtype('{f}'), dtype('{f}'), dtype('{}'))", NT::U(d.str_width() * 2).full_name())),
        "abs" | "absolute" | "negative" | "positive" => exc("UFuncTypeError", format!("ufunc '{}' did not contain a loop with signature matching types <class 'numpy.dtypes.StrDType'> -> None", if n == "abs" { "absolute" } else { n })),
        "sqrt" | "exp" | "exp2" | "expm1" | "log" | "log10" | "log2" | "log1p" | "sin" | "cos" | "tan" | "arcsin" | "arccos" | "arctan" | "sinh" | "cosh" | "tanh"
        | "arcsinh" | "arccosh" | "arctanh" | "cbrt" | "floor" | "ceil" | "trunc" | "rint" | "round" | "around" | "fix" | "square" | "reciprocal" | "sign" | "degrees" | "radians"
        | "deg2rad" | "rad2deg" | "isnan" | "isinf" | "isfinite" | "logical_not" | "fabs" => {
            let u = match n { "round" | "around" => "rint", "fix" => return None, _ => n };
            exc("TypeError", format!("ufunc '{u}' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''"))
        }
        _ => return None,
    })
}

// An operator with a string array or np.str_ on a side: comparisons compare the strings (with a number only
// == and != work, giving all False / all True), + joins them; numpy has no other loop for strings.
fn np_str_binop(x: &Opnd, y: &Opnd, op: &str) -> R {
    let shape = np_bshape(&x.a.shape, &y.a.shape)?;
    let (dx, dy) = (x.a.dt(), y.a.dt());
    let cmp = matches!(op, "<" | ">" | "<=" | ">=" | "==" | "!=");
    let no_loop = || {
        let name = |d: NT, o: &Opnd| if o.weak { match d { NT::U(_) => format!("dtype('{}')", d.full_name()), NT::F => "<class 'numpy.dtypes._PyFloatDType'>".into(), NT::B => "<class 'numpy.dtypes.BoolDType'>".into(), _ => "<class 'numpy.dtypes._PyLongDType'>".into() } } else { format!("dtype('{}')", d.full_name()) };
        exc("UFuncTypeError", format!("ufunc '{}' did not contain a loop with signature matching types ({}, {}) -> None", ufunc_name(op), name(dx, x), name(dy, y)))
    };
    if !(dx.is_u() && dy.is_u()) {
        return match op {
            "==" | "!=" => Ok(np_out(Buf::B(vec![op == "!="; shape.iter().product()]), shape)),
            "*" => type_err("The 'out' kwarg is necessary when using the string multiply ufunc directly. Use numpy.strings.multiply to multiply strings without specifying 'out'.".into()),
            _ => Err(no_loop()),
        };
    }
    let (xv, yv) = (np_bvals(&x.a, &shape), np_bvals(&y.a, &shape));
    let (Buf::U(xv, _), Buf::U(yv, _)) = (xv, yv) else { unreachable!() };
    if cmp {
        let r: Vec<bool> = xv.iter().zip(&yv).map(|(p, q)| {
            if op == "==" || op == "!=" {
                return (p == q) == (op == "==");
            }
            let (p, q) = (np_sget(*p), np_sget(*q));
            match op {
                "<" => p < q,
                ">" => p > q,
                "<=" => p <= q,
                _ => p >= q,
            }
        }).collect();
        return Ok(np_out(Buf::B(r), shape));
    }
    if op == "+" {
        let (NT::U(wx), NT::U(wy)) = (dx, dy) else { unreachable!() };
        let r: Vec<u32> = xv.iter().zip(&yv).map(|(p, q)| np_sid(&format!("{}{}", np_sget(*p), np_sget(*q)))).collect();
        return Ok(np_out(Buf::U(r, wx + wy), shape));
    }
    if op == "*" {
        return type_err("The 'out' kwarg is necessary when using the string multiply ufunc directly. Use numpy.strings.multiply to multiply strings without specifying 'out'.".into());
    }
    Err(no_loop())
}

// a OP= b on an array: in place (other names of the same buffer see it), in the array's own type.
pub fn np_iop(a: &NdArr, me: &V, b: &V, op: &str) -> R {
    let r = match np_binop(me, b, op).unwrap()? {
        v => v,
    };
    let (rb, rshape) = match (as_np(&r), as_npsc(&r)) {
        (Some(x), _) => (x.vals(), x.shape.clone()),
        (None, Some(s)) => (Buf::from_sc(s.dt(), &[s]), vec![]),
        _ => return Ok(r),
    };
    if rshape != a.shape && np_bshape(&rshape, &a.shape)? != a.shape {
        return val_err(format!("non-broadcastable output operand with shape {} doesn't match the broadcast shape {}", np_shape_repr(&a.shape), np_shape_repr(&rshape)));
    }
    let d = a.dt();
    if rb.dt().kind() > d.kind() {
        return Err(exc(
            "UFuncTypeError",
            format!("Cannot cast ufunc '{}' output from dtype('{}') to dtype('{}') with casting rule 'same_kind'", ufunc_name(op), rb.dt().name(), d.name()),
        ));
    }
    let src = NdArr::new(rb, rshape);
    let vals = np_bvals(&src, &a.shape);
    let ps = a.pos();
    let mut buf = a.buf.borrow_mut();
    for (i, p) in ps.iter().enumerate() {
        buf.set(*p, vals.get(i));
    }
    Ok(me.clone())
}

pub fn np_unary(v: &V, op: &str) -> R {
    let o = np_opnd(v)?.unwrap();
    let vals = o.a.vals();
    // a numpy integer scalar at its minimum: numpy warns that -x and abs(x) overflow
    if o.scalar && matches!(op, "neg" | "abs") && matches!(vals.get(0), Sc::I(i64::MIN) | Sc::I32(i32::MIN)) {
        np_warn(&format!("overflow encountered in scalar {}", if op == "neg" { "negative" } else { "absolute" }));
    }
    let r = match (op, vals) {
        ("neg", Buf::B(_)) => return type_err("The numpy boolean negative, the `-` operator, is not supported, use the `~` operator or the logical_not function instead.".into()),
        ("neg", Buf::I(x)) => Buf::I(x.iter().map(|i| i.wrapping_neg()).collect()),
        ("neg", Buf::I32(x)) => Buf::I32(x.iter().map(|i| i.wrapping_neg()).collect()),
        ("neg", Buf::F(x)) => Buf::F(x.iter().map(|i| -i).collect()),
        ("neg", Buf::F32(x)) => Buf::F32(x.iter().map(|i| -i).collect()),
        ("pos", Buf::B(_)) => return type_err("ufunc 'positive' did not contain a loop with signature matching types <class 'numpy.dtypes.BoolDType'> -> None".into()),
        ("pos", b) => b,
        ("invert", Buf::B(x)) => Buf::B(x.iter().map(|b| !b).collect()),
        ("invert", Buf::I(x)) => Buf::I(x.iter().map(|i| !i).collect()),
        ("invert", Buf::I32(x)) => Buf::I32(x.iter().map(|i| !i).collect()),
        ("invert", Buf::F(_) | Buf::F32(_)) => return Err(exc("TypeError", "ufunc 'invert' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''".into())),
        ("abs", Buf::B(x)) => Buf::B(x),
        ("abs", Buf::I(x)) => Buf::I(x.iter().map(|i| i.wrapping_abs()).collect()),
        ("abs", Buf::I32(x)) => Buf::I32(x.iter().map(|i| i.wrapping_abs()).collect()),
        ("abs", Buf::F(x)) => Buf::F(x.iter().map(|i| i.abs()).collect()),
        ("abs", Buf::F32(x)) => Buf::F32(x.iter().map(|i| i.abs()).collect()),
        _ => return Err(np_unsup(op)),
    };
    Ok(np_out(r, o.a.shape.clone()))
}

// a OP b for comparisons, as a value: arrays compare elementwise.
pub fn cmp_v(a: &V, b: &V, op: &str) -> R {
    if matches!(a, V::Obj(_)) || matches!(b, V::Obj(_)) {
        if let Some(r) = obj_cmp(a, b, op) {
            return r;
        }
    }
    if matches!(op, "<" | ">" | "<=" | ">=" | "==" | "!=") && (is_np(a) || is_np(b)) {
        if let Some(r) = np_binop(a, b, op) {
            match r {
                Err(e) if op == "==" || op == "!=" => {
                    if np_opnd(a).ok().flatten().is_none() || np_opnd(b).ok().flatten().is_none() {
                        return Ok(np_sc(Sc::B(op == "!=")));
                    }
                    return Err(e);
                }
                r => return r,
            }
        }
    }
    // a value from CPython (the dynamic build's bridge): the library's own result (a polars expression, a
    // pandas Series), as CPython gives it
    for (x, y, right) in [(a, b, false), (b, a, true)] {
        if let V::Ext(e) = x {
            match e.cmp_value(x, op, y, right) {
                Some(Ok(V::NotImpl)) | None => {}
                Some(r) => return r,
            }
        }
    }
    Ok(V::Bool(cmp_op(a, b, op)?))
}

// `a OP b` for objects: what the first __eq__ / __ne__ / __lt__ ... that does not answer NotImplemented returns,
// as CPython does (not its truth). None when no dunder answers (cmp_op then gives the default).
fn obj_cmp(a: &V, b: &V, op: &str) -> Option<R> {
    let (l, r) = match op {
        "==" => ("__eq__", "__eq__"),
        "!=" => ("__ne__", "__ne__"),
        "<" => ("__lt__", "__gt__"),
        ">" => ("__gt__", "__lt__"),
        "<=" => ("__le__", "__ge__"),
        _ => ("__ge__", "__le__"),
    };
    for (x, y, d) in [(a, b, l), (b, a, r)] {
        if let V::Obj(o) = x {
            if let Some(f) = o.cls.lookup(d) {
                match call(&f, Args::p(vec![x.clone(), y.clone()])) {
                    Ok(V::NotImpl) => {}
                    res => return Some(res),
                }
            }
        }
    }
    None
}

// The truth of an array (numpy refuses one with more than one element).
pub fn np_truth(a: &NdArr) -> Result<bool, E> {
    match a.size() {
        1 => Ok(a.vals().get(0).b()),
        0 => val_err("The truth value of an empty array is ambiguous. Use `array.size > 0` to check that an array is not empty.".into()),
        _ => val_err("The truth value of an array with more than one element is ambiguous. Use a.any() or a.all()".into()),
    }
}

// ---------- matmul ----------

// numpy hands float dot and matmul to OpenBLAS, whose kernels (picked for the CPU) add in their own
// orders with fused multiply-adds. These follow the SkylakeX (AVX-512) and Haswell/Zen (AVX2) kernels, so
// results match numpy's on the same kind of machine; elsewhere a plain loop.
#[derive(Clone, Copy, PartialEq)]
enum Blas {
    Skx,
    Hsw,
    Plain,
}
fn np_blas() -> Blas {
    static K: std::sync::OnceLock<Blas> = std::sync::OnceLock::new();
    *K.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512dq") && is_x86_feature_detected!("avx512cd") && is_x86_feature_detected!("avx512bw") && is_x86_feature_detected!("avx512vl") {
                return Blas::Skx;
            }
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                return Blas::Hsw;
            }
        }
        Blas::Plain
    })
}
// a * b + c rounded once. The runtime is built for baseline x86-64, where this is a software routine; the
// hot kernels below are also compiled for CPUs with FMA (fma_on) and pick that copy at run time, where it is
// the CPU's own instruction. Either way the result is the same.
#[inline(always)]
fn fma(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}
fn fma_cpu() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        return std::arch::is_x86_feature_detected!("fma");
    }
    #[allow(unreachable_code)]
    false
}
// fma_on!(fname(args: types) -> ret): a copy of the #[inline(always)] function `fname_i` built with FMA,
// chosen when the CPU has it
macro_rules! fma_on {
    ($name:ident, $imp:ident, ($($a:ident: $t:ty),*) -> $r:ty) => {
        fn $name($($a: $t),*) -> $r {
            #[cfg(target_arch = "x86_64")]
            {
                #[target_feature(enable = "fma")]
                unsafe fn hw($($a: $t),*) -> $r {
                    $imp($($a),*)
                }
                if fma_cpu() {
                    return unsafe { hw($($a),*) };
                }
            }
            $imp($($a),*)
        }
    };
}
// OpenBLAS ddot on one thread's share (dot_compute in kernel/x86_64/ddot.c)
fma_on!(ddot1, ddot1_i, (x: &[f64], y: &[f64], unit: bool, k: Blas) -> f64);
#[inline(always)]
fn ddot1_i(x: &[f64], y: &[f64], unit: bool, k: Blas) -> f64 {
    let n = x.len();
    if k == Blas::Plain {
        let mut s = 0.0;
        for i in 0..n {
            s += x[i] * y[i];
        }
        return s;
    }
    if !unit {
        let (mut t1, mut t2) = (0.0, 0.0);
        let n1 = n & !3;
        let mut i = 0;
        while i < n1 {
            let (m1, m2, m3, m4) = (y[i] * x[i], y[i + 1] * x[i + 1], y[i + 2] * x[i + 2], y[i + 3] * x[i + 3]);
            if k == Blas::Skx {
                t1 += fma(y[i], x[i], m3);
                t2 += fma(y[i + 1], x[i + 1], m4);
            } else {
                t1 += m1 + m3;
                t2 += m2 + m4;
            }
            i += 4;
        }
        while i < n {
            t1 = if k == Blas::Skx { fma(y[i], x[i], t1) } else { t1 + y[i] * x[i] };
            i += 1;
        }
        return t1 + t2;
    }
    let n1 = n & !15;
    let mut d = 0.0;
    if n1 > 0 {
        // four 4-lane accumulators (Skx: first 8-lane ones over blocks of 32, folded in)
        let mut acc = [[0.0f64; 4]; 4];
        let mut i = 0;
        if k == Blas::Skx {
            let n32 = n1 & !31;
            let mut z = [[0.0f64; 8]; 4];
            while i < n32 {
                for r in 0..4 {
                    for l in 0..8 {
                        let p = i + 8 * r + l;
                        z[r][l] = fma(x[p], y[p], z[r][l]);
                    }
                }
                i += 32;
            }
            for r in 0..4 {
                for l in 0..4 {
                    acc[r][l] = z[r][l] + z[r][l + 4];
                }
            }
        }
        while i < n1 {
            for r in 0..4 {
                for l in 0..4 {
                    let p = i + 4 * r + l;
                    acc[r][l] = fma(x[p], y[p], acc[r][l]);
                }
            }
            i += 16;
        }
        d = if k == Blas::Skx {
            let a: Vec<f64> = (0..4).map(|l| ((acc[0][l] + acc[1][l]) + acc[2][l]) + acc[3][l]).collect();
            (a[0] + a[2]) + (a[1] + a[3])
        } else {
            let h: Vec<[f64; 2]> = (0..4).map(|r| [acc[r][0] + acc[r][2], acc[r][1] + acc[r][3]]).collect();
            let c = [(h[0][0] + h[1][0]) + (h[2][0] + h[3][0]), (h[0][1] + h[1][1]) + (h[2][1] + h[3][1])];
            c[0] + c[1]
        };
    }
    for i in n1..n {
        d = if k == Blas::Skx { fma(y[i], x[i], d) } else { d + y[i] * x[i] };
    }
    d
}
// cblas_ddot: over 10000 values OpenBLAS splits the work across its threads (one per CPU) and adds the parts
fn np_ddot(x: &[f64], y: &[f64], unit: bool) -> f64 {
    let k = np_blas();
    let n = x.len();
    if n <= 10000 || k == Blas::Plain {
        return ddot1(x, y, unit, k);
    }
    let nt = std::thread::available_parallelism().map_or(1, |v| v.get()).min(64);
    let (mut rest, mut pos, mut d) = (n, 0usize, 0.0);
    for j in 0..nt {
        if rest == 0 {
            break;
        }
        let w = ((rest + nt - j - 1) / (nt - j)).min(rest);
        d += ddot1(&x[pos..pos + w], &y[pos..pos + w], unit, k);
        pos += w;
        rest -= w;
    }
    d
}
// one element of a float matrix product: the k-loop as one fused multiply-add chain (OpenBLAS's gemm
// kernels); for a matrix times a vector, gemv's four lanes
fn dot_f(x: &[f64], y: &[f64]) -> f64 {
    if np_blas() == Blas::Plain {
        let mut s = 0.0;
        for i in 0..x.len() {
            s += x[i] * y[i];
        }
        return s;
    }
    let mut s = 0.0;
    for i in 0..x.len() {
        s = fma(x[i], y[i], s);
    }
    s
}
// numpy's float matrix @ vector is OpenBLAS's dgemv: dgemv_t_4.c for a C-ordered matrix (one dot per row),
// dgemv_n_4.c for a Fortran-ordered one and for vector @ matrix (columns added into the result). The kernels
// and their order are the same on Haswell and SkylakeX for dgemv_t; dgemv_n's differ. From 115200 * 4 values
// OpenBLAS splits the work across its threads (gemv_thread.c), which changes which kernel a row gets, or,
// for few output values, splits the dot and adds the parts.
fn np_gemv_nt(m: usize, k: usize) -> usize {
    if m * k < 460800 {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |v| v.get()).min(64)
    }
}
// gemv_thread.c's split of `n` items over `nt` threads: (start, length)
fn np_gemv_split(n: usize, nt: usize) -> Vec<(usize, usize)> {
    let (mut out, mut i, mut pos) = (vec![], n, 0);
    while i > 0 {
        let left = nt.saturating_sub(out.len()).max(1);
        let w = ((i + left - 1) / left).max(4).min(i);
        out.push((pos, w));
        pos += w;
        i -= w;
    }
    out
}
// one dot of dgemv_t: kind 4 (the AVX2 four-row kernel), 2 or 1 (the SSE two- and one-row kernels)
fma_on!(gemv_t_row, gemv_t_row_i, (a: &[f64], x: &[f64], kind: usize) -> f64);
#[inline(always)]
fn gemv_t_row_i(a: &[f64], x: &[f64], kind: usize) -> f64 {
    let k = a.len();
    let kb = k - k % 4;
    let mut y = 0.0;
    let mut lo = 0;
    while lo < kb {
        let hi = (lo + 2048).min(kb);
        let b = match kind {
            4 => {
                let mut v = [0.0f64; 4];
                for t in lo..hi {
                    v[(t - lo) % 4] = fma(a[t], x[t], v[(t - lo) % 4]);
                }
                (v[0] + v[2]) + (v[1] + v[3])
            }
            2 => {
                let mut v = [0.0f64; 2];
                for t in lo..hi {
                    v[(t - lo) % 2] += a[t] * x[t];
                }
                v[0] + v[1]
            }
            _ => {
                let (mut p, mut q) = ([0.0f64; 2], [0.0f64; 2]);
                for t in lo..hi {
                    let r = (t - lo) % 4;
                    if r < 2 {
                        p[r] += a[t] * x[t];
                    } else {
                        q[r - 2] += a[t] * x[t];
                    }
                }
                (p[0] + q[0]) + (p[1] + q[1])
            }
        };
        y += b;
        lo = hi;
    }
    match k - kb {
        1 => fma(a[kb], x[kb], y),
        2 => y + fma(a[kb], x[kb], a[kb + 1] * x[kb + 1]),
        3 => y + fma(a[kb + 2], x[kb + 2], fma(a[kb], x[kb], a[kb + 1] * x[kb + 1])),
        _ => y,
    }
}
// A x for row-major A (m x k)
fn np_gemv_t(a: &[f64], m: usize, k: usize, x: &[f64]) -> Vec<f64> {
    np_gemv_t_ld(a, 0, k, m, k, x)
}
// the same with row r at a[off + r * ld..][..k]
fn np_gemv_t_ld(a: &[f64], off: usize, ld: usize, m: usize, k: usize, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m];
    let nt = np_gemv_nt(m, k);
    let chunks = if nt > 1 { np_gemv_split(m, nt) } else { vec![(0, m)] };
    for (r0, len) in chunks {
        let full = len / 4 * 4;
        for j in 0..len {
            let kind = if j < full {
                4
            } else if len % 4 >= 2 && j < full + 2 {
                2
            } else {
                1
            };
            let o = off + (r0 + j) * ld;
            y[r0 + j] = gemv_t_row(&a[o..o + k], x, kind);
        }
    }
    y
}
// dgemv_n over rows r0..r0+rn and columns c0..c0+cn of the column-major m x k matrix a (lda m); a fresh y
fma_on!(gemv_n_part, gemv_n_part_i, (a: &[f64], m: usize, r0: usize, rn: usize, c0: usize, cn: usize, x: &[f64], unit: bool, ldm: bool) -> Vec<f64>);
#[inline(always)]
fn gemv_n_part_i(a: &[f64], m: usize, r0: usize, rn: usize, c0: usize, cn: usize, x: &[f64], unit: bool, ldm: bool) -> Vec<f64> {
    let skx = np_blas() == Blas::Skx;
    let at = |r: usize, c: usize| a[r0 + r + (c0 + c) * m];
    let xs = &x[c0..c0 + cn];
    let mb = rn - (rn & 3);
    let mut y = vec![0.0; rn];
    for r in 0..mb {
        let (mut c, mut t) = (0, 0.0);
        while c + 4 <= cn {
            let s = if skx {
                let s = fma(at(r, c), xs[c], at(r, c + 1) * xs[c + 1]);
                let s = fma(at(r, c + 2), xs[c + 2], s);
                fma(at(r, c + 3), xs[c + 3], s)
            } else {
                fma(at(r, c + 2), xs[c + 2], at(r, c) * xs[c]) + fma(at(r, c + 3), xs[c + 3], at(r, c + 1) * xs[c + 1])
            };
            t += s;
            c += 4;
        }
        if cn - c >= 2 && unit {
            t += if skx { fma(at(r, c), xs[c], at(r, c + 1) * xs[c + 1]) } else { at(r, c) * xs[c] + at(r, c + 1) * xs[c + 1] };
            c += 2;
        }
        while c < cn {
            t += at(r, c) * xs[c];
            c += 1;
        }
        y[r] = t;
    }
    // the kernel's lda == 2 / 3 special case (ldm: the matrix's leading dimension is its row count)
    let pair = (m == 2 || m == 3) && unit && ldm;
    for r in mb..rn {
        let (mut c, mut t) = (0, 0.0);
        if pair {
            while c + 4 <= cn {
                for h in [c, c + 2] {
                    t += if skx { fma(at(r, h), xs[h], at(r, h + 1) * xs[h + 1]) } else { at(r, h) * xs[h] + at(r, h + 1) * xs[h + 1] };
                }
                c += 4;
            }
        }
        while c < cn {
            t = if skx { fma(at(r, c), xs[c], t) } else { t + at(r, c) * xs[c] };
            c += 1;
        }
        y[r] = t;
    }
    y
}
// A x for column-major A (m x k): numpy's vector @ matrix, and matrix @ vector for a Fortran-ordered matrix
fn np_gemv_n(a: &[f64], m: usize, k: usize, x: &[f64], unit: bool) -> Vec<f64> {
    np_gemv_n_ld(a, m, k, x, unit, true)
}
fn np_gemv_n_ld(a: &[f64], m: usize, k: usize, x: &[f64], unit: bool, ldm: bool) -> Vec<f64> {
    let nt = np_gemv_nt(m, k);
    if nt == 1 {
        return gemv_n_part(a, m, 0, m, 0, k, x, unit, ldm);
    }
    let rows = np_gemv_split(m, nt);
    if rows.len() < nt && m * k > 9216 && m * nt <= 1024 {
        // split the dot instead, then add each thread's part in order
        let mut y = vec![0.0; m];
        for (c0, cn) in np_gemv_split(k, nt) {
            let p = gemv_n_part(a, m, 0, m, c0, cn, x, unit, ldm);
            for j in 0..m {
                y[j] += p[j];
            }
        }
        return y;
    }
    let mut y = vec![];
    for (r0, rn) in rows {
        y.extend(gemv_n_part(a, m, r0, rn, 0, k, x, unit, ldm));
    }
    y
}
// numpy's float matrix @ matrix is OpenBLAS's dgemm (0.3.3x as numpy 2.x ships it), called row-major, so
// OpenBLAS computes the column-major product C^T = Y^T X^T: its rows are numpy's columns. Each element is a
// fused multiply-add chain over k, which the kernels split and regroup:
// - SkylakeX (AVX-512), up to 100^3 multiplies: the small-matrix kernels (dgemm_small_kernel_{nn,nt,tn,tt}
//   _skylakex.c). NN and TN sum some elements in eight lanes of k and reduce the lanes in one of two orders;
//   NT and TT are plain chains. TN is refused when the result has more than 1200 values or k < 32.
// - otherwise the regular path (below): blocks of k (GEMM_Q values) whose sums are added into C, each block
//   through dgemm_kernel_16x2_skylakex.c or dgemm_kernel_4x8_haswell.S, in the pieces level3.c (one thread)
//   or level3_thread.c (from 2 * 262144 multiplies, on as many threads as the machine has cores) cut.
// xf / yf: OpenBLAS reads that operand transposed.
fn np_gemm_kblocks(k: usize, q: usize, um: usize, threaded: bool) -> Vec<(usize, usize)> {
    let (mut out, mut ls) = (vec![], 0);
    while ls < k {
        let mut ml = k - ls;
        if ml >= 2 * q {
            ml = q;
        } else if ml > q {
            ml = if threaded { ml.div_ceil(2) } else { (ml / 2).div_ceil(um) * um };
        }
        out.push((ls, ml));
        ls += ml;
    }
    out
}
fma_on!(np_gemm, np_gemm_i, (xv: &[f64], yv: &[f64], m: usize, k: usize, n: usize, xf: bool, yf: bool) -> Vec<f64>);
#[inline(always)]
fn np_gemm_i(xv: &[f64], yv: &[f64], m: usize, k: usize, n: usize, xf: bool, yf: bool) -> Vec<f64> {
    let ycol: Vec<Vec<f64>> = (0..n).map(|i| (0..k).map(|t| yv[t * n + i]).collect()).collect();
    let xr = |j: usize| &xv[j * k..(j + 1) * k];
    let chain = |a: &[f64], b: &[f64]| {
        let mut s = 0.0;
        for t in 0..a.len() {
            s = fma(a[t], b[t], s);
        }
        s
    };
    let mnk = m as f64 * n as f64 * k as f64;
    let ncpu = std::thread::available_parallelism().map_or(1, |v| v.get()).min(64);
    let nthreads = if mnk <= 262144.0 {
        1
    } else if mnk / (ncpu as f64) < 262144.0 {
        (mnk / 262144.0) as usize
    } else {
        ncpu
    };
    let mut c = vec![0.0; m * n];
    if np_blas() == Blas::Skx {
        if mnk <= 1e6 && !(yf && !xf && (m * n > 1200 || k < 32)) {
            // f4: REDUCE_4's order; otherwise _mm512_reduce_add_pd's
            let lanes = |a: &[f64], b: &[f64], f4: bool| {
                let mut l = [0.0f64; 8];
                for t in 0..a.len() {
                    l[t % 8] = fma(a[t], b[t], l[t % 8]);
                }
                if f4 {
                    ((l[0] + l[1]) + (l[2] + l[3])) + ((l[4] + l[5]) + (l[6] + l[7]))
                } else {
                    ((l[0] + l[4]) + (l[2] + l[6])) + ((l[1] + l[5]) + (l[3] + l[7]))
                }
            };
            let (m8, m4, n4) = (n & !7, n & !3, m & !3);
            // NN: only the last (n % 8 <= 4) rows of a k >= 16 product go through lanes
            let nn_lanes = n - m8 <= 4 && k >= 16;
            for j in 0..m {
                for i in 0..n {
                    let (a, b) = (&ycol[i][..], xr(j));
                    c[j * n + i] = match (yf, xf) {
                        (false, false) if i >= m8 && nn_lanes => lanes(a, b, i < m4 || j < n4),
                        (true, false) => lanes(a, b, i < m4 || j < n4),
                        _ => chain(a, b),
                    };
                }
            }
            return c;
        }
    }
    // the regular path: each kernel call is a k-block of a block of rows by columns, which the kernel takes in
    // chunks (lk_row_chunks, lk_col_chunks). dgemm_kernel_4x8_haswell.S sums a chunk of four rows by one column
    // in four lanes; dgemm_kernel_16x2_skylakex.c sums 4 x 12 chunks two k at a time and 2 x 12 and 1 x 12
    // ones four at a time, except in the calls that pack their columns of B first (`first`), which sum in
    // plain chains (found by testing against numpy; the order of the other calls is the kernel's own).
    let skx = np_blas() == Blas::Skx;
    let lkh = if skx { Lk { skx: true, um: 16, un: 2, q: 384 } } else { Lk { skx: false, um: 4, un: 8, q: if cfg!(windows) { 128 } else { 256 } } };
    let (calls, thr) = np_gemm_calls(n, m, nthreads, skx);
    for (ls, ml) in np_gemm_kblocks(k, lkh.q, lkh.um, thr) {
        for &(i0, mi, j0, nj, first) in &calls {
            for (cc, cs) in lk_col_chunks(&lkh, nj) {
                for (rr, rs) in lk_row_chunks(&lkh, mi) {
                    for i in i0 + rr..i0 + rr + rs {
                        let a = &ycol[i][ls..ls + ml];
                        for j in j0 + cc..j0 + cc + cs {
                            let b = &xr(j)[ls..ls + ml];
                            let acc = np_kacc(a, b, rs, cs, skx, !first);
                            c[j * n + i] += acc;
                        }
                    }
                }
            }
        }
    }
    c
}
// one kernel chunk's sum over a k-block (see np_gemm's regular path): rs x cs is the chunk, wok whether the
// SkylakeX kernel's two- and four-lane sums apply to this call
#[inline(always)]
fn np_kacc(a: &[f64], b: &[f64], rs: usize, cs: usize, skx: bool, wok: bool) -> f64 {
    let ml = a.len();
    let w = if skx && wok && cs == 12 {
        match rs {
            4 => 2,
            2 | 1 => 4,
            _ => 1,
        }
    } else {
        1
    };
    if w > 1 && ml >= w {
        let mut q = [0.0f64; 4];
        let mut t = 0;
        while ml - t >= w {
            for l in 0..w {
                q[l] = fma(a[t + l], b[t + l], q[l]);
            }
            t += w;
        }
        let mut acc = if w == 2 { q[0] + q[1] } else { (q[0] + q[1]) + (q[2] + q[3]) };
        while t < ml {
            acc = fma(a[t], b[t], acc);
            t += 1;
        }
        acc
    } else if !skx && rs == 4 && cs == 1 {
        let mut q = [0.0f64; 4];
        let k8 = ml / 8 * 8;
        for t in 0..k8 {
            q[t % 4] = fma(a[t], b[t], q[t % 4]);
        }
        for t in k8..ml {
            q[0] = fma(a[t], b[t], q[0]);
        }
        (q[0] + q[1]) + (q[2] + q[3])
    } else {
        let mut s = 0.0;
        for t in 0..ml {
            s = fma(a[t], b[t], s);
        }
        s
    }
}
// A @ A.T and A.T @ A: numpy calls OpenBLAS's dsyrk (upper triangle, row-major, so OpenBLAS computes the
// column-major lower one) and copies it across. v holds the vectors whose dot products make the result (rows
// of A for A @ A.T, columns for A.T @ A). level3_syrk.c (one thread) or level3_syrk_threaded.c (from
// (n + 1) n k > 439776 on more than one core, rows split so each thread gets about the same area of the
// triangle) cut it into calls; syrk_kernel.c takes the part of each call below the diagonal through the
// gemm kernel and 32 x 32 diagonal blocks through it into a scratch block.
fma_on!(np_syrk, np_syrk_i, (v: &[Vec<f64>], k: usize) -> Vec<f64>);
#[inline(always)]
fn np_syrk_i(v: &[Vec<f64>], k: usize) -> Vec<f64> {
    let n = v.len();
    let skx = np_blas() == Blas::Skx;
    let lkh = if skx { Lk { skx: true, um: 16, un: 2, q: 384 } } else { Lk { skx: false, um: 4, un: 8, q: if cfg!(windows) { 128 } else { 256 } } };
    let (p, r, mn, sr) = if skx { (192, 8640, 32, 16) } else { (512, 13824, 32, 8) };
    let mut c = vec![0.0; n * n];
    // one gemm-kernel call on rows r0.., cols c0.. (diag: only on and below the diagonal)
    let kcall = |c: &mut Vec<f64>, r0: usize, m: usize, c0: usize, nn: usize, ls: usize, ml: usize, diag: bool| {
        if m == 0 || nn == 0 {
            return;
        }
        for (cc, cs) in lk_col_chunks(&lkh, nn) {
            for (rr, rs) in lk_row_chunks(&lkh, m) {
                for i in r0 + rr..r0 + rr + rs {
                    for j in c0 + cc..c0 + cc + cs {
                        if diag && i < j {
                            continue;
                        }
                        c[i + j * n] += np_kacc(&v[i][ls..ls + ml], &v[j][ls..ls + ml], rs, cs, skx, true);
                    }
                }
            }
        }
    };
    // syrk_kernel.c (LOWER) on m rows from x, nn columns from y
    let kop = |c: &mut Vec<f64>, m: usize, nn: usize, ml: usize, x: usize, y: usize, ls: usize| {
        let (mut m, mut nn, mut r0, mut c0) = (m as isize, nn as isize, x, y);
        let mut off = x as isize - y as isize;
        if m + off < 0 {
            return;
        }
        if nn < off {
            kcall(c, r0, m as usize, c0, nn as usize, ls, ml, false);
            return;
        }
        if off > 0 {
            kcall(c, r0, m as usize, c0, off as usize, ls, ml, false);
            c0 += off as usize;
            nn -= off;
            off = 0;
            if nn <= 0 {
                return;
            }
        }
        if nn > m + off {
            nn = m + off;
            if nn <= 0 {
                return;
            }
        }
        if off < 0 {
            r0 += (-off) as usize;
            m += off;
            off = 0;
            if m <= 0 {
                return;
            }
        }
        if m > nn - off {
            kcall(c, r0 + (nn - off) as usize, (m - nn + off) as usize, c0, nn as usize, ls, ml, false);
            m = nn + off;
            if m <= 0 {
                return;
            }
        }
        let (m, nn) = (m as usize, nn as usize);
        let mut lp = 0;
        while lp < nn {
            let w = mn.min(nn - lp);
            kcall(c, r0 + lp, w, c0 + lp, w, ls, ml, true);
            kcall(c, r0 + lp + w, m.saturating_sub(lp + w), c0 + lp, w, ls, ml, false);
            lp += mn;
        }
    };
    let isz = |rest: usize| {
        if rest >= 2 * p {
            p
        } else if rest > p {
            (rest / 2).div_ceil(mn) * mn
        } else {
            rest
        }
    };
    let ncpu = std::thread::available_parallelism().map_or(1, |v| v.get()).min(64);
    let nnk = (n as f64 + 1.0) * n as f64 * k as f64;
    let nth = if nnk <= 109944.0 * 4.0 { 1 } else { ncpu };
    let blocks = np_gemm_kblocks(k, lkh.q, 0, true);
    if nth > 1 && n >= nth * sr {
        // level3_syrk_threaded.c (LOWER)
        let mask = mn - 1;
        let mut rg = vec![0usize];
        let (mut i, mut num) = (0usize, 0usize);
        let dnum = (n * n) as f64 / nth as f64;
        while i < n {
            let mut w = n - i;
            if nth - num > 1 {
                let di = i as f64;
                let dinum = di * di + dnum;
                w = if dinum > 0.0 { (((dinum.sqrt() - di) as isize + mask as isize) / (mask as isize + 1) * (mask as isize + 1)).max(0) as usize } else { 0 };
                if w > n - i || w < mask {
                    w = n - i;
                }
            }
            rg.push(rg[num] + w);
            num += 1;
            i += w;
        }
        let dv = |cur: usize| (rg[cur + 1] - rg[cur]).div_ceil(2).div_ceil(mn) * mn;
        for pos in 0..num {
            let (mf, mt) = (rg[pos], rg[pos + 1]);
            for &(ls, ml) in &blocks {
                let mut mi = isz(mt - mf);
                let x = (mt - mf - mi) % p;
                if x > 0 {
                    mi -= p - x;
                }
                let d = dv(pos);
                let mut xx = mf;
                while xx < mt {
                    let end = mt.min(xx + d);
                    let mut jjs = xx;
                    while jjs < end {
                        let mjj = (end - jjs).min(mn);
                        kop(&mut c, mi, mjj, ml, mt - mi, jjs, ls);
                        jjs += mjj;
                    }
                    xx += d;
                }
                for cur in (0..pos).rev() {
                    let d = dv(cur);
                    let mut xx = rg[cur];
                    while xx < rg[cur + 1] {
                        kop(&mut c, mi, (rg[cur + 1] - xx).min(d), ml, mt - mi, xx, ls);
                        xx += d;
                    }
                }
                let st = mi;
                let mut is = mf;
                while is < mt - st {
                    let mut mi = mt - st - is;
                    if mi >= 2 * p {
                        mi = p;
                    } else if mi > p {
                        mi = mi.div_ceil(2).div_ceil(mn) * mn;
                    }
                    for cur in (0..=pos).rev() {
                        let d = dv(cur);
                        let mut xx = rg[cur];
                        while xx < rg[cur + 1] {
                            kop(&mut c, mi, (rg[cur + 1] - xx).min(d), ml, is, xx, ls);
                            xx += d;
                        }
                    }
                    is += mi;
                }
            }
        }
    } else {
        // level3_syrk.c (LOWER, TRANS)
        let mut js = 0;
        while js < n {
            let mj = (n - js).min(r);
            let (ms, me) = (js, n);
            for &(ls, ml) in &blocks {
                let mut mi = isz(me - ms);
                kop(&mut c, mi, mi.min(mj + js - ms), ml, ms, ms, ls);
                let mut jjs = js;
                while jjs < ms {
                    let mjj = (ms - jjs).min(lkh.un);
                    kop(&mut c, mi, mjj, ml, ms, jjs, ls);
                    jjs += mjj;
                }
                let mut is = ms + mi;
                while is < me {
                    mi = isz(me - is);
                    if is < js + mj {
                        kop(&mut c, mi, mi.min(mj + js - is), ml, is, is, ls);
                        kop(&mut c, mi, is - js, ml, is, js, ls);
                    } else {
                        kop(&mut c, mi, mj, ml, is, js, ls);
                    }
                    is += mi;
                }
            }
            js += mj;
        }
    }
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = c[i.max(j) + i.min(j) * n];
        }
    }
    out
}
// The kernel calls (row0, rows, col0, cols) OpenBLAS makes for an mm x nn (column-major) product, and whether
// it ran threaded. One thread: level3.c. More: level3_thread.c splits rows and columns over a grid of threads
// (at least SWITCH_RATIO each, widths rounded to GEMM_PREFERRED_SIZE: 8 on Haswell and 16 on SkylakeX, the
// values OpenBLAS's runtime CPU table holds); each thread handles its own columns in min_jj pieces and the
// other threads' columns in halves. GEMM_R (13824 / 8640 columns) is not reached by sizes this handles alike.
fn np_gemm_calls(mm: usize, nn: usize, nth: usize, skx: bool) -> (Vec<(usize, usize, usize, usize, bool)>, bool) {
    let (p, um, un, sr, ps) = if skx { (192, 16, 2, 16, 16) } else { (512, 4, 8, 8, 8) };
    let isz = |rest: usize, first: bool| {
        if rest >= 2 * p {
            p
        } else if rest > p {
            (if first { rest / 2 } else { rest.div_ceil(2) }).div_ceil(um) * um
        } else {
            rest
        }
    };
    let jjsz = |rest: usize| {
        if skx {
            rest.min(6 * un)
        } else if rest >= 3 * un {
            3 * un
        } else {
            rest.min(un)
        }
    };
    let rup = |rem: usize, w: usize| if ps > rem || w <= ps { w } else { w.div_ceil(ps) * ps };
    let mut cl = vec![];
    let (mut nm, mut nnt) = (1, 1);
    if nth > 1 {
        if mm >= 2 * sr {
            nm = nth;
            while mm < nm * sr {
                nm /= 2;
            }
        }
        if nn >= sr * nm {
            nnt = nn.div_ceil(sr * nm);
            if nm * nnt > nth {
                nnt = nth / nm;
            }
            let (mut cost, mut div) = (0, 0);
            let mut i = 1;
            while i * i <= nm {
                if nm % i == 0 {
                    let j = nm / i;
                    let (ci, cj) = (nn * j + mm * nnt * i, nn * i + mm * nnt * j);
                    if cost == 0 || ci < cost {
                        cost = ci;
                        div = i;
                    }
                    if cj < cost {
                        cost = cj;
                        div = j;
                    }
                }
                i += 1;
            }
            if div > 1 {
                nm /= div;
                nnt *= div;
            }
        }
    }
    if nm * nnt <= 1 {
        let r = if skx { 8640 } else { 13824 };
        let mut js = 0;
        while js < nn {
            let mj = (nn - js).min(r);
            let mi = isz(mm, true);
            let mut jjs = js;
            while jjs < js + mj {
                let mjj = jjsz(js + mj - jjs);
                cl.push((0, mi, jjs, mjj, true));
                jjs += mjj;
            }
            let mut is = mi;
            while is < mm {
                let mi = isz(mm - is, true);
                cl.push((is, mi, js, mj, false));
                is += mi;
            }
            js += mj;
        }
        return (cl, false);
    }
    let nt = nm * nnt;
    let mut rm = vec![0usize];
    let (mut m, mut parts) = (mm as isize, 0);
    while m > 0 {
        let mut w = rup(m as usize, (m as usize + nm - parts - 1) / (nm - parts)) as isize;
        m -= w;
        if m < 0 {
            w += m;
        }
        rm.push(rm[parts] + w as usize);
        parts += 1;
    }
    while rm.len() < nm + 1 {
        rm.push(*rm.last().unwrap());
    }
    let mut rn = vec![0usize];
    let mut n = nn;
    for j in 0..nnt {
        let mut wn = n.div_ceil(nnt - j) as isize;
        n -= wn as usize;
        for i in 0..nm {
            let mut w = ((wn as usize + nm - i - 1) / (nm - i)).max(sr);
            w = rup(wn.max(0) as usize, w);
            wn -= w as isize;
            let mut w = w as isize;
            if wn < 0 {
                w += wn;
                wn = 0;
            }
            rn.push(rn[rn.len() - 1] + w as usize);
        }
    }
    for pos in 0..nt {
        let (pm, pn) = (pos % nm, pos / nm);
        let (mf, mt) = (rm[pm], rm[pm + 1]);
        if mt <= mf {
            continue;
        }
        let (nf, ntt) = (rn[pos], rn[pos + 1]);
        let mi = isz(mt - mf, true);
        let divn = (ntt - nf).div_ceil(2).max(1);
        let mut js = nf;
        while js < ntt {
            let end = ntt.min(js + divn);
            let mut jjs = js;
            while jjs < end {
                let mjj = jjsz(end - jjs);
                cl.push((mf, mi, jjs, mjj, true));
                jjs += mjj;
            }
            js += divn;
        }
        let next = |c: usize| if c + 1 >= (pn + 1) * nm { pn * nm } else { c + 1 };
        let halves = |cl: &mut Vec<(usize, usize, usize, usize, bool)>, cur: usize, is: usize, mi: usize| {
            let d = (rn[cur + 1] - rn[cur]).div_ceil(2).max(1);
            let mut js = rn[cur];
            while js < rn[cur + 1] {
                cl.push((is, mi, js, (rn[cur + 1] - js).min(d), false));
                js += d;
            }
        };
        let mut cur = next(pos);
        while cur != pos {
            halves(&mut cl, cur, mf, mi);
            cur = next(cur);
        }
        let mut is = mf + mi;
        while is < mt {
            let mi = isz(mt - is, false);
            let mut cur = pos;
            loop {
                halves(&mut cl, cur, is, mi);
                cur = next(cur);
                if cur == pos {
                    break;
                }
            }
            is += mi;
        }
    }
    (cl, true)
}
pub fn np_matmul(a: &V, b: &V) -> R {
    let (x, y) = match (np_opnd(a)?, np_opnd(b)?) {
        (Some(x), Some(y)) => (x, y),
        _ => return Err(exc("TypeError", format!("unsupported operand type(s) for @: '{}' and '{}'", tn(a), tn(b)))),
    };
    if x.a.ndim() == 0 || y.a.ndim() == 0 {
        return val_err("matmul: Input operand 0 does not have enough dimensions (has 0, gufunc core with signature (n?,k),(k,m?)->(n?,m?) requires 1)".into());
    }
    if x.a.ndim() > 2 || y.a.ndim() > 2 {
        return np_matmul_nd(&x.a, &y.a);
    }
    np_dot2(&x.a, &y.a, true)
}
// matmul over stacks of matrices: the leading dimensions broadcast.
fn np_matmul_nd(x: &NdArr, y: &NdArr) -> R {
    let x2 = if x.ndim() == 1 { x.reshaped(vec![1, x.shape[0]]) } else { x.clone() };
    let y2 = if y.ndim() == 1 { y.reshaped(vec![y.shape[0], 1]) } else { y.clone() };
    let (xb, yb) = (&x2.shape[..x2.ndim() - 2], &y2.shape[..y2.ndim() - 2]);
    let batch = np_bshape(xb, yb)?;
    let (m, k) = (x2.shape[x2.ndim() - 2], x2.shape[x2.ndim() - 1]);
    let (k2, n) = (y2.shape[y2.ndim() - 2], y2.shape[y2.ndim() - 1]);
    if k != k2 {
        return val_err(format!("matmul: Input operand 1 has a mismatch in its core dimension 0, with gufunc signature (n?,k),(k,m?)->(n?,m?) (size {k2} is different from {k})"));
    }
    let mut xs = batch.clone();
    xs.extend([m, k]);
    let mut ys = batch.clone();
    ys.extend([k, n]);
    let xv = NdArr::new(np_bvals(&x2, &xs), xs.clone());
    let yv = NdArr::new(np_bvals(&y2, &ys), ys.clone());
    let nb: usize = batch.iter().product();
    let mut parts: Vec<Sc> = vec![];
    let mut d = np_promote(x.dt(), y.dt());
    for b in 0..nb {
        let xa = NdArr::new(xv.vals().slice(b * m * k, m * k), vec![m, k]);
        let ya = NdArr::new(yv.vals().slice(b * k * n, k * n), vec![k, n]);
        let r = np_dot2(&xa, &ya, true)?;
        let ra = as_np(&r).unwrap().vals();
        d = ra.dt();
        for i in 0..ra.len() {
            parts.push(ra.get(i));
        }
    }
    let mut shape = batch;
    if x.ndim() > 1 {
        shape.push(m);
    }
    if y.ndim() > 1 {
        shape.push(n);
    }
    Ok(np_out(Buf::from_sc(d, &parts), shape))
}
// dot / matmul for 1-D and 2-D arrays.
pub fn np_dot2(x: &NdArr, y: &NdArr, matmul: bool) -> R {
    if x.ndim() > 2 || y.ndim() > 2 {
        return Err(np_unsup("dot/matmul of arrays with more than 2 dimensions"));
    }
    let d = np_promote(x.dt(), y.dt());
    if d == NT::F32 {
        return Err(np_unsup("float32 dot/matmul (OpenBLAS sgemm)"));
    }
    let (m, k) = if x.ndim() == 1 { (1, x.shape[0]) } else { (x.shape[0], x.shape[1]) };
    let (k2, n) = if y.ndim() == 1 { (y.shape[0], 1) } else { (y.shape[0], y.shape[1]) };
    if k != k2 {
        if matmul {
            return val_err(format!("matmul: Input operand 1 has a mismatch in its core dimension 0, with gufunc signature (n?,k),(k,m?)->(n?,m?) (size {k2} is different from {k})"));
        }
        return val_err(format!("shapes {} and {} not aligned: {} (dim {}) != {} (dim 0)", np_shape_repr(&x.shape), np_shape_repr(&y.shape), k, x.ndim() - 1, k2));
    }
    let mut shape = vec![];
    if x.ndim() == 2 {
        shape.push(m);
    }
    if y.ndim() == 2 {
        shape.push(n);
    }
    let out = match d {
        NT::F => {
            let (xv, yv) = (x.fv(), y.fv());
            if x.ndim() == 1 && y.ndim() == 1 {
                let unit = x.st.first().is_none_or(|s| *s == 1) && y.st.first().is_none_or(|s| *s == 1);
                Buf::F(vec![np_ddot(&xv, &yv, unit)])
            } else {
                // matmul hands BLAS only operands it can describe (numpy's is_blasable2d); np.dot copies the rest
                let bl2 = |a: &NdArr| {
                    let ok = |s0: isize, s1: isize, d1: usize| s1 == 1 && s0 >= d1 as isize;
                    a.ndim() != 2 || ok(a.st[0], a.st[1], a.shape[1]) || ok(a.st[1], a.st[0], a.shape[0]) || a.shape.contains(&1)
                };
                let bl1 = |a: &NdArr| a.ndim() != 1 || a.st[0] >= 1 || a.shape[0] <= 1;
                let blas = np_blas() != Blas::Plain && (!matmul || (bl2(x) && bl2(y) && bl1(x) && bl1(y)));
                // a 2-d operand stored in Fortran order (a transpose) reaches BLAS as the other kind of gemv
                let ford = |a: &NdArr| a.ndim() == 2 && a.shape[0] > 1 && a.shape[1] > 1 && a.st[0] == 1;
                let vunit = |a: &NdArr| a.st.first().is_none_or(|s| *s == 1);
                let contig = |a: &NdArr| (a.st[1] == 1 && a.st[0] == a.shape[1] as isize) || (a.st[0] == 1 && a.st[1] == a.shape[0] as isize);
                // a matrix times its own transpose: numpy calls dsyrk
                let syrk = |a: &NdArr, b: &NdArr| {
                    Rc::ptr_eq(&a.buf, &b.buf) && a.off == b.off && a.shape[0] == b.shape[1] && a.shape[1] == b.shape[0] && a.st[0] == b.st[1] && a.st[1] == b.st[0]
                };
                // numpy (matmul and np.dot alike) treats a one-row or one-column matrix as a vector
                let xvec = x.ndim() == 1 || m == 1;
                let yvec = y.ndim() == 1 || n == 1;
                let xunit = if x.ndim() == 1 { vunit(x) } else { k <= 1 || x.st[1] == 1 };
                let yunit = if y.ndim() == 1 { vunit(y) } else { k <= 1 || y.st[0] == 1 };
                if blas && !xvec && yvec && k > 0 {
                    // matrix @ vector
                    Buf::F(if ford(x) {
                        let mut cm = vec![0.0; m * k];
                        for i in 0..m {
                            for t in 0..k {
                                cm[i + t * m] = xv[i * k + t];
                            }
                        }
                        np_gemv_n(&cm, m, k, &yv, yunit)
                    } else {
                        np_gemv_t(&xv, m, k, &yv)
                    })
                } else if blas && xvec && !yvec && k > 0 {
                    // vector @ matrix: y is k x n, column-major n x k when C-ordered
                    Buf::F(if ford(y) {
                        let mut rm = vec![0.0; n * k];
                        for t in 0..k {
                            for j in 0..n {
                                rm[j * k + t] = yv[t * n + j];
                            }
                        }
                        np_gemv_t(&rm, n, k, &xv)
                    } else {
                        np_gemv_n(&yv, n, k, &xv, xunit)
                    })
                } else if xvec && yvec && blas && k > 0 {
                    // one row or one column: numpy takes ddot
                    let mut o = vec![0.0; m * n];
                    let mut col = vec![0.0; k];
                    for j in 0..n {
                        for t in 0..k {
                            col[t] = yv[t * n + j];
                        }
                        for i in 0..m {
                            o[i * n + j] = np_ddot(&xv[i * k..(i + 1) * k], &col, xunit && yunit);
                        }
                    }
                    Buf::F(o)
                } else if np_blas() != Blas::Plain && x.ndim() == 2 && y.ndim() == 2 && m > 1 && n > 1 && k > 0 && syrk(x, y) && (if matmul { bl2(x) } else { contig(x) }) {
                    let v: Vec<Vec<f64>> = (0..m).map(|i| xv[i * k..(i + 1) * k].to_vec()).collect();
                    Buf::F(np_syrk(&v, k))
                } else if np_blas() != Blas::Plain && x.ndim() == 2 && y.ndim() == 2 && m > 1 && n > 1 && k > 0 {
                    // which operands OpenBLAS reads transposed: matmul passes a strided matrix BLAS can describe
                    // as it is and copies any other (numpy 2.5, keeping it Fortran-like when its rows are closer
                    // together than its columns); np.dot passes only a contiguous one and copies the rest to C order
                    let tr = |a: &NdArr| {
                        if !matmul {
                            a.st[0] == 1 && a.st[1] == a.shape[0] as isize
                        } else if bl2(a) {
                            !(a.st[1] == 1 && a.st[0] >= a.shape[1] as isize)
                        } else {
                            a.st[0].abs() < a.st[1].abs()
                        }
                    };
                    Buf::F(np_gemm(&xv, &yv, m, k, n, tr(x), tr(y)))
                } else {
                    let mut o = vec![0.0; m * n];
                    let mut col = vec![0.0; k];
                    for j in 0..n {
                        for t in 0..k {
                            col[t] = yv[t * n + j];
                        }
                        for i in 0..m {
                            o[i * n + j] = if !blas && matmul {
                                // numpy's own loop (no BLAS): left to right
                                let mut acc = 0.0;
                                for t in 0..k {
                                    acc += xv[i * k + t] * col[t];
                                }
                                acc
                            } else {
                                dot_f(&xv[i * k..(i + 1) * k], &col)
                            };
                        }
                    }
                    Buf::F(o)
                }
            }
        }
        NT::I | NT::I32 => {
            let (xv, yv) = (x.iv(), y.iv());
            let mut o = vec![0i64; m * n];
            for i in 0..m {
                for j in 0..n {
                    let mut s = 0i64;
                    for t in 0..k {
                        s = s.wrapping_add(xv[i * k + t].wrapping_mul(yv[t * n + j]));
                    }
                    o[i * n + j] = s;
                }
            }
            // numpy's int32 loop adds in int32: the same sum, wrapped
            if d == NT::I32 {
                Buf::I32(o.into_iter().map(|v| v as i32).collect())
            } else {
                Buf::I(o)
            }
        }
        _ => {
            let (xv, yv) = (x.bv(), y.bv());
            let mut o = vec![false; m * n];
            for i in 0..m {
                for j in 0..n {
                    o[i * n + j] = (0..k).any(|t| xv[i * k + t] && yv[t * n + j]);
                }
            }
            Buf::B(o)
        }
    };
    Ok(np_out(out, shape))
}

// ---------- indexing ----------

enum Ix {
    Int(i64),
    Sl(V, V, V),
    New,
    Ell,
    Adv(NdArr),
}

fn np_index_items(i: &V) -> Result<Vec<Ix>, E> {
    let items: Vec<V> = match i {
        V::Tuple(t) => (**t).clone(),
        _ => vec![i.clone()],
    };
    let mut out = vec![];
    for x in items {
        out.push(match &x {
            V::Int(k) => Ix::Int(*k),
            V::Bool(_) => return Err(np_unsup("indexing with True/False")),
            V::None => Ix::New,
            V::Ellipsis => Ix::Ell,
            V::Slice(s) => Ix::Sl(s.0.clone(), s.1.clone(), s.2.clone()),
            V::List(_) | V::Tuple(_) => {
                let a = np_from(&x, None)?;
                if a.size() == 0 && a.dt().is_f() {
                    Ix::Adv(NdArr::new(Buf::I(vec![]), a.shape.clone()))
                } else {
                    Ix::Adv(a)
                }
            }
            _ => {
                if let Some(a) = as_np(&x) {
                    Ix::Adv(a.clone())
                } else if let Some(s) = as_npsc(&x) {
                    match s {
                        Sc::I(_) | Sc::I32(_) => Ix::Int(s.i()),
                        _ => return Err(np_unsup("indexing with a numpy bool or float")),
                    }
                } else {
                    return Err(exc("IndexError", "only integers, slices (`:`), ellipsis (`...`), numpy.newaxis (`None`) and integer or boolean arrays are valid indices".into()));
                }
            }
        });
    }
    for x in &out {
        if let Ix::Adv(a) = x {
            if a.dt().is_f() {
                return Err(exc("IndexError", "arrays used as indices must be of integer (or boolean) type".into()));
            }
        }
    }
    Ok(out)
}

// What an index selects: a view (basic indexing) and, for integer/boolean arrays, the positions they pick.
struct Sel {
    view: NdArr,
    // advanced: (buffer positions in C order of the result, result shape)
    adv: Option<(Vec<usize>, Vec<usize>)>,
}

fn np_select(a: &NdArr, i: &V) -> Result<Sel, E> {
    let items = np_index_items(i)?;
    // boolean arrays become their nonzero() integer arrays, one per dimension they cover
    let mut its: Vec<Ix> = vec![];
    let mut dim = 0usize;
    let consumes = |x: &Ix| match x {
        Ix::Int(_) | Ix::Sl(..) => 1,
        Ix::Adv(m) if m.dt() == NT::B => m.ndim(),
        Ix::Adv(_) => 1,
        _ => 0,
    };
    let used: usize = items.iter().map(consumes).sum();
    if used > a.ndim() {
        return Err(exc("IndexError", format!("too many indices for array: array is {}-dimensional, but {} were indexed", a.ndim(), used)));
    }
    if items.iter().filter(|x| matches!(x, Ix::Ell)).count() > 1 {
        return Err(exc("IndexError", "an index can only have a single ellipsis ('...')".into()));
    }
    for x in items {
        match x {
            Ix::Adv(m) if m.dt() == NT::B => {
                if m.ndim() == 0 {
                    return Err(np_unsup("indexing with a 0-d boolean"));
                }
                for k in 0..m.ndim() {
                    if m.shape[k] != a.shape[dim + k] {
                        return Err(exc("IndexError", format!("boolean index did not match indexed array along axis {}; size of axis is {} but size of corresponding boolean axis is {}", dim + k, a.shape[dim + k], m.shape[k])));
                    }
                }
                let bv = m.bv();
                let cs = cstrides(&m.shape);
                let mut cols: Vec<Vec<i64>> = vec![vec![]; m.ndim()];
                for (p, b) in bv.iter().enumerate() {
                    if *b {
                        let mut r = p;
                        for k in 0..m.ndim() {
                            let s = cs[k] as usize;
                            cols[k].push((r / s) as i64);
                            r %= s;
                        }
                    }
                }
                dim += m.ndim();
                for c in cols {
                    let n = c.len();
                    its.push(Ix::Adv(NdArr::new(Buf::I(c), vec![n])));
                }
            }
            x => {
                dim += consumes(&x);
                its.push(x);
            }
        }
    }
    // basic indexing, keeping the dimensions integer arrays index
    let mut shape = vec![];
    let mut st = vec![];
    let mut off = a.off as isize;
    let mut d = 0usize;
    let mut adv: Vec<(usize, NdArr)> = vec![]; // (dimension of the view, index array)
    let n_used: usize = its.iter().map(|x| matches!(x, Ix::Int(_) | Ix::Sl(..) | Ix::Adv(_)) as usize).sum();
    for x in its {
        match x {
            Ix::Int(k) => {
                let n = a.shape[d] as i64;
                let j = if k < 0 { k + n } else { k };
                if j < 0 || j >= n {
                    return Err(exc("IndexError", format!("index {k} is out of bounds for axis {d} with size {n}")));
                }
                off += j as isize * a.st[d];
                d += 1;
            }
            Ix::Sl(x, y, z) => {
                let n = a.shape[d];
                let (s0, _s1, step) = slice_idx(&(x.clone(), y.clone(), z.clone()), n)?;
                let len = slice_len(&(x, y, z), n)?;
                off += if len > 0 { s0 as isize * a.st[d] } else { 0 };
                shape.push(len);
                st.push(a.st[d] * step as isize);
                d += 1;
            }
            Ix::New => {
                shape.push(1);
                st.push(0);
            }
            Ix::Ell => {
                let k = a.ndim() - n_used;
                for _ in 0..k {
                    shape.push(a.shape[d]);
                    st.push(a.st[d]);
                    d += 1;
                }
            }
            Ix::Adv(m) => {
                adv.push((shape.len(), m));
                shape.push(a.shape[d]);
                st.push(a.st[d]);
                d += 1;
            }
        }
    }
    while d < a.ndim() {
        shape.push(a.shape[d]);
        st.push(a.st[d]);
        d += 1;
    }
    let view = NdArr { buf: a.buf.clone(), off: off.max(0) as usize, shape, st };
    if adv.is_empty() {
        return Ok(Sel { view, adv: None });
    }
    // integer arrays: broadcast together; their result dimensions go where they are (all next to each other)
    // or first
    let mut bs: Vec<usize> = vec![];
    for (_, m) in &adv {
        bs = np_bshape(&bs, &m.shape).map_err(|_| exc("IndexError", format!("shape mismatch: indexing arrays could not be broadcast together with shapes {}", adv.iter().map(|(_, m)| np_shape_repr(&m.shape).replace(", ", ",")).collect::<Vec<_>>().join(" "))))?;
    }
    let idx: Vec<Vec<i64>> = adv.iter().map(|(_, m)| np_bvals(m, &bs).iv()).collect();
    let adims: Vec<usize> = adv.iter().map(|(k, _)| *k).collect();
    let together = adims.windows(2).all(|w| w[1] == w[0] + 1);
    let rest: Vec<usize> = (0..view.ndim()).filter(|k| !adims.contains(k)).collect();
    let nb: usize = bs.iter().product();
    let mut bases = Vec::with_capacity(nb);
    for b in 0..nb {
        let mut p = view.off as isize;
        for (j, (k, _)) in adv.iter().enumerate() {
            let n = view.shape[*k] as i64;
            let v = idx[j][b];
            let w = if v < 0 { v + n } else { v };
            if w < 0 || w >= n {
                // the axis of the original array this dimension came from
                return Err(exc("IndexError", format!("index {v} is out of bounds for axis {k} with size {n}")));
            }
            p += w as isize * view.st[*k];
        }
        bases.push(p);
    }
    let (pre, post): (Vec<usize>, Vec<usize>) = if together { rest.iter().partition(|k| **k < adims[0]) } else { (vec![], rest.clone()) };
    let pre_shape: Vec<usize> = pre.iter().map(|k| view.shape[*k]).collect();
    let pre_st: Vec<isize> = pre.iter().map(|k| view.st[*k]).collect();
    let post_shape: Vec<usize> = post.iter().map(|k| view.shape[*k]).collect();
    let post_st: Vec<isize> = post.iter().map(|k| view.st[*k]).collect();
    // offsets (relative) of the leading dimensions
    let mut pre_off: Vec<isize> = vec![];
    {
        let n: usize = pre_shape.iter().product();
        let cs = cstrides(&pre_shape);
        for f in 0..n {
            let mut r = f;
            let mut o = 0isize;
            for k in 0..pre_shape.len() {
                let s = cs[k] as usize;
                o += (r / s) as isize * pre_st[k];
                r %= s;
            }
            pre_off.push(o);
        }
    }
    let mut out = vec![];
    for po in &pre_off {
        for b in &bases {
            let base = b + po;
            walk_pos(base as usize, &post_shape, &post_st, &mut out);
            if post_shape.iter().product::<usize>() == 0 {
                continue;
            }
        }
    }
    let mut rshape = pre_shape.clone();
    rshape.extend(bs.iter().cloned());
    rshape.extend(post_shape.iter().cloned());
    Ok(Sel { view, adv: Some((out, rshape)) })
}

fn slice_len(s: &(V, V, V), n: usize) -> Result<usize, E> {
    let (a, b, st) = slice_idx(s, n)?;
    Ok(if st > 0 {
        if b > a {
            ((b - a - 1) / st + 1) as usize
        } else {
            0
        }
    } else if a > b {
        ((a - b - 1) / (-st) + 1) as usize
    } else {
        0
    })
}

pub fn np_getitem(a: &NdArr, i: &V) -> R {
    // a[mask] with a mask of a's shape: the values where it is true
    if let Some(m) = as_np(i) {
        if m.dt() == NT::B && m.shape == a.shape && a.ndim() > 0 {
            let mv = m.bv();
            let vals = a.vals();
            let ps: Vec<usize> = mv.iter().enumerate().filter(|(_, b)| **b).map(|(k, _)| k).collect();
            let n = ps.len();
            return Ok(np_new(vals.pick(&ps), vec![n]));
        }
    }
    let s = np_select(a, i)?;
    match s.adv {
        None => {
            if s.view.ndim() == 0 {
                return Ok(np_sc(s.view.first()));
            }
            Ok(np_arr(s.view))
        }
        Some((ps, shape)) => {
            let b = a.buf.borrow().pick(&ps);
            Ok(np_new(b, shape))
        }
    }
}

pub fn np_setitem(a: &NdArr, i: &V, v: &V) -> Result<(), E> {
    let s = np_select(a, i)?;
    let (ps, shape) = match s.adv {
        None => (s.view.pos(), s.view.shape.clone()),
        Some(x) => x,
    };
    let d = a.dt();
    if d == NT::I32 {
        if let V::Int(i) = v {
            py_i32(*i)?;
        }
    }
    if !d.is_f() {
        if let V::Float(f) = v {
            if f.is_nan() {
                return val_err("cannot convert float NaN to integer".into());
            }
            if f.is_infinite() {
                return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
            }
        }
    }
    let src = match np_opnd(v)? {
        // a list of Python ints stored in an int32 array: each must fit
        Some(_) if d == NT::I32 && seq_items(v).is_some() => np_from(v, Some(NT::I32))?,
        Some(o) => o.a,
        None => return Err(np_unsup(&format!("storing {} in an array", tn(v)))),
    };
    // a value with more leading dimensions of length 1 still fits
    let mut src = src;
    while src.ndim() > shape.len() && src.shape[0] == 1 {
        src = NdArr { buf: src.buf.clone(), off: src.off, shape: src.shape[1..].to_vec(), st: src.st[1..].to_vec() };
    }
    let vals = if src.shape == shape {
        src.vals()
    } else {
        match np_bshape(&src.shape, &shape) {
            Ok(b) if b == shape => np_bvals(&src, &shape),
            _ => {
                if src.ndim() == 1 && shape.len() == 1 {
                    return val_err(format!("NumPy boolean array indexing assignment cannot assign {} input values to the {} output values where the mask is true", src.shape[0], shape[0]).replace("NumPy boolean array indexing assignment cannot assign", "shape mismatch: value array of shape").chars().collect::<String>())
                        .or_else(|_: E| val_err(format!("could not broadcast input array from shape {} into shape {}", np_shape_repr(&src.shape), np_shape_repr(&shape))));
                }
                return val_err(format!("could not broadcast input array from shape {} into shape {}", np_shape_repr(&src.shape), np_shape_repr(&shape)));
            }
        }
    };
    let mut buf = a.buf.borrow_mut();
    for (k, p) in ps.iter().enumerate() {
        buf.set(*p, vals.get(k));
    }
    Ok(())
}

// ---------- reductions ----------

// numpy's pairwise summation (pairwise_sum in loops_utils.h).
pub fn np_pw(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for x in a {
            r += *x;
        }
        r
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_pw(&a[..n2]) + np_pw(&a[n2..])
    }
}

// The same for float32 (FLOAT_pairwise_sum: float32 accumulators).
pub fn np_pw32(a: &[f32]) -> f32 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0f32;
        for x in a {
            r += *x;
        }
        r
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_pw32(&a[..n2]) + np_pw32(&a[n2..])
    }
}

pub fn norm_axis(ax: i64, nd: usize) -> Result<usize, E> {
    let n = nd as i64;
    if ax < -n || ax >= n {
        return Err(exc("AxisError", format!("axis {ax} is out of bounds for array of dimension {nd}")));
    }
    Ok(if ax < 0 { (ax + n) as usize } else { ax as usize })
}

// The values of a, grouped for a reduction over `axis`: (outer shape, groups of the values reduced). Along
// the last axis of a contiguous array the groups are runs numpy sums pairwise; other axes are summed in order.
pub struct Groups {
    pub out_shape: Vec<usize>,
    pub n: usize,
    pub vals: Buf,      // C order of (rest..., axis): group g is vals[g*n..(g+1)*n]
    pub pairwise: bool, // whether numpy sums each group pairwise
}
pub fn np_groups(a: &NdArr, axis: Option<usize>) -> Groups {
    match axis {
        None if a.contig() => Groups { out_shape: vec![], n: a.size(), vals: a.vals(), pairwise: true },
        Some(ax) if ax + 1 == a.ndim() && a.contig() => {
            let out_shape: Vec<usize> = a.shape[..ax].to_vec();
            Groups { out_shape, n: a.shape[ax], vals: a.vals(), pairwise: true }
        }
        None => {
            // numpy reduces in memory order: a contiguous array (or its transpose) is one pairwise run
            let n = a.size();
            let mut order: Vec<usize> = (0..a.ndim()).filter(|k| a.shape[*k] > 1).collect();
            order.sort_by_key(|k| std::cmp::Reverse(a.st[*k].unsigned_abs()));
            let shape: Vec<usize> = order.iter().map(|k| a.shape[*k]).collect();
            let st: Vec<isize> = order.iter().map(|k| a.st[*k]).collect();
            let mut ps = vec![];
            walk_pos(a.off, &shape, &st, &mut ps);
            let dense = st.iter().zip(cstrides(&shape)).all(|(s, c)| s.unsigned_abs() == c as usize);
            let vals = a.buf.borrow().pick(&ps);
            if dense || shape.len() <= 1 {
                Groups { out_shape: vec![], n, vals, pairwise: true }
            } else {
                // rows of the innermost dimension, each pairwise, added in order
                Groups { out_shape: vec![], n, vals, pairwise: false }
            }
        }
        Some(ax) => {
            let mut order: Vec<usize> = (0..a.ndim()).filter(|k| *k != ax).collect();
            order.push(ax);
            let shape: Vec<usize> = order.iter().map(|k| a.shape[*k]).collect();
            let st: Vec<isize> = order.iter().map(|k| a.st[*k]).collect();
            let mut ps = vec![];
            walk_pos(a.off, &shape, &st, &mut ps);
            let out_shape: Vec<usize> = (0..a.ndim()).filter(|k| *k != ax).map(|k| a.shape[k]).collect();
            // pairwise when the reduced axis is the innermost one in memory
            let inner = (0..a.ndim()).filter(|k| a.shape[*k] > 1).min_by_key(|k| a.st[*k].unsigned_abs());
            let pairwise = inner == Some(ax) || out_shape.iter().product::<usize>() <= 1;
            Groups { out_shape, n: a.shape[ax], vals: a.buf.borrow().pick(&ps), pairwise }
        }
    }
}

// Sum of the groups of floats, in numpy's order.
fn fsum_groups(g: &Groups, v: &[f64], a: &NdArr, axis: Option<usize>) -> Vec<f64> {
    let ng: usize = g.out_shape.iter().product::<usize>().max(if g.out_shape.is_empty() { 1 } else { 0 });
    if axis.is_none() && !g.pairwise {
        // innermost rows pairwise, rows added in order
        let mut order: Vec<usize> = (0..a.ndim()).filter(|k| a.shape[*k] > 1).collect();
        order.sort_by_key(|k| std::cmp::Reverse(a.st[*k].unsigned_abs()));
        let inner = a.shape[*order.last().unwrap()];
        let mut s = 0.0;
        for row in v.chunks(inner) {
            s += np_pw(row);
        }
        return vec![s];
    }
    let n = g.n;
    if n == 0 {
        return vec![0.0; ng];
    }
    (0..ng)
        .map(|k| {
            let grp = &v[k * n..(k + 1) * n];
            if g.pairwise {
                // numpy starts the sum at the identity 0.0, so all -0.0 sums to 0.0
                0.0 + np_pw(grp)
            } else {
                let mut s = 0.0;
                for x in grp {
                    s += *x;
                }
                s
            }
        })
        .collect()
}

// fsum_groups in float32.
fn fsum_groups32(g: &Groups, v: &[f32], a: &NdArr, axis: Option<usize>) -> Vec<f32> {
    let ng: usize = g.out_shape.iter().product::<usize>().max(if g.out_shape.is_empty() { 1 } else { 0 });
    if axis.is_none() && !g.pairwise {
        let mut order: Vec<usize> = (0..a.ndim()).filter(|k| a.shape[*k] > 1).collect();
        order.sort_by_key(|k| std::cmp::Reverse(a.st[*k].unsigned_abs()));
        let inner = a.shape[*order.last().unwrap()];
        let mut s = 0.0f32;
        for row in v.chunks(inner) {
            s += np_pw32(row);
        }
        return vec![s];
    }
    let n = g.n;
    if n == 0 {
        return vec![0.0; ng];
    }
    (0..ng)
        .map(|k| {
            let grp = &v[k * n..(k + 1) * n];
            if g.pairwise {
                0.0 + np_pw32(grp)
            } else {
                let mut s = 0.0f32;
                for x in grp {
                    s += *x;
                }
                s
            }
        })
        .collect()
}

pub struct RedOpts {
    pub axis: Option<usize>,
    pub keepdims: bool,
    pub ddof: f64,
}

pub fn red_shape(a: &NdArr, o: &RedOpts, out_shape: Vec<usize>) -> Vec<usize> {
    if !o.keepdims {
        return out_shape;
    }
    match o.axis {
        None => vec![1; a.ndim()],
        Some(ax) => {
            let mut s = a.shape.clone();
            s[ax] = 1;
            s
        }
    }
}

pub fn np_reduce(a: &NdArr, what: &str, o: &RedOpts) -> R {
    // strings: argmax/argmin compare them, any/all/count_nonzero test them for being empty; numpy has no
    // sum, mean, max, ... loop for them
    if let NT::U(_) = a.dt() {
        if !matches!(what, "argmax" | "argmin" | "any" | "all" | "count_nonzero") {
            let uf = match what {
                "max" => "maximum",
                "min" => "minimum",
                "prod" => "multiply",
                _ => return Err(np_str_noloop(what, a.dt()).unwrap_or_else(|| np_unsup(&format!("{what} of a string array")))),
            };
            let d = a.dt().full_name();
            return Err(exc("UFuncTypeError", format!("ufunc '{uf}' did not contain a loop with signature matching types (dtype('{d}'), dtype('{d}')) -> None")));
        }
    }
    if matches!(what, "argmax" | "argmin") && o.axis.is_none() && (a.ndim() != 1 || !a.contig()) {
        // over the flattened array (C order), not memory order
        let fa = NdArr::new(a.vals(), vec![a.size()]);
        let r = np_reduce(&fa, what, &RedOpts { axis: Some(0), keepdims: false, ddof: 0.0 })?;
        if o.keepdims {
            return Ok(np_new(Buf::I(vec![as_npsc(&r).unwrap().i()]), vec![1; a.ndim()]));
        }
        return Ok(r);
    }
    let g = np_groups(a, o.axis);
    let ng: usize = if g.out_shape.is_empty() { 1 } else { g.out_shape.iter().product() };
    let n = g.n;
    let d = a.dt();
    let shape = red_shape(a, o, g.out_shape.clone());
    let empty_err = |what: &str| exc("ValueError", format!("zero-size array to reduction operation {what} which has no identity"));
    let out: Buf = match what {
        "sum" | "mean" | "var" | "std" if d == NT::F32 => {
            // float32 throughout: sums, the mean, the deviations and their squares (numpy's _mean and _var)
            let v = g.vals.clone().f32v();
            let sums = fsum_groups32(&g, &v, a, o.axis);
            if what == "sum" {
                Buf::F32(sums)
            } else {
                let cnt = n as f32;
                if n == 0 {
                    np_warn("Mean of empty slice.");
                    np_warn("invalid value encountered in scalar divide");
                }
                let means: Vec<f32> = sums.iter().map(|s| s / cnt).collect();
                if what == "mean" {
                    Buf::F32(means)
                } else {
                    let dev: Vec<f32> = (0..ng * n).map(|i| {
                        let x = v[i] - means[i / n.max(1)];
                        x * x
                    }).collect();
                    let ss = fsum_groups32(&g, &dev, a, o.axis);
                    let div = (n as f64 - o.ddof).max(0.0);
                    if div <= 0.0 {
                        np_warn("Degrees of freedom <= 0 for slice");
                    }
                    let div = div as f32;
                    let var: Vec<f32> = ss.iter().map(|s| s / div).collect();
                    Buf::F32(if what == "std" { var.iter().map(|x| x.sqrt()).collect() } else { var })
                }
            }
        }
        "sum" | "mean" | "var" | "std" => {
            if what == "sum" && d != NT::F {
                let v = g.vals.ic();
                Buf::I((0..ng).map(|k| v[k * n..(k + 1) * n].iter().fold(0i64, |s, x| s.wrapping_add(*x))).collect())
            } else {
                let v = g.vals.fc();
                let sums = fsum_groups(&g, &v, a, o.axis);
                if what == "sum" {
                    Buf::F(sums)
                } else {
                    let cnt = n as f64;
                    if n == 0 {
                        np_warn("Mean of empty slice.");
                        np_warn("invalid value encountered in scalar divide");
                    }
                    let means: Vec<f64> = sums.iter().map(|s| s / cnt).collect();
                    if what == "mean" {
                        Buf::F(means)
                    } else {
                        // x = a - mean; sum(x*x) / (n - ddof), in the same order
                        let dev: Vec<f64> = (0..ng * n).map(|i| {
                            let x = v[i] - means[i / n.max(1)];
                            x * x
                        }).collect();
                        let ss = fsum_groups(&g, &dev, a, o.axis);
                        let div = (cnt - o.ddof).max(0.0);
                        if div <= 0.0 {
                            np_warn("Degrees of freedom <= 0 for slice");
                        }
                        let var: Vec<f64> = ss.iter().map(|s| s / div).collect();
                        Buf::F(if what == "std" { var.iter().map(|x| x.sqrt()).collect() } else { var })
                    }
                }
            }
        }
        "prod" => match d {
            NT::F => {
                let v = g.vals.fc();
                Buf::F((0..ng).map(|k| v[k * n..(k + 1) * n].iter().fold(1.0, |s, x| s * x)).collect())
            }
            NT::F32 => {
                let v = g.vals.clone().f32v();
                Buf::F32((0..ng).map(|k| v[k * n..(k + 1) * n].iter().fold(1.0f32, |s, x| s * x)).collect())
            }
            _ => {
                let v = g.vals.ic();
                Buf::I((0..ng).map(|k| v[k * n..(k + 1) * n].iter().fold(1i64, |s, x| s.wrapping_mul(*x))).collect())
            }
        },
        "max" | "min" => {
            if n == 0 {
                return Err(empty_err(if what == "max" { "maximum" } else { "minimum" }));
            }
            let mx = what == "max";
            fn fmax<T: Copy + PartialOrd>(grp: &[T], mx: bool, nan: impl Fn(T) -> bool) -> T {
                let mut m = grp[0];
                for x in &grp[1..] {
                    if nan(m) {
                        break;
                    }
                    // ties take the later value (0.0 and -0.0), as numpy's loops do
                    if nan(*x) || (mx && !(m > *x)) || (!mx && !(m < *x)) {
                        m = *x;
                    }
                }
                m
            }
            match &g.vals {
                Buf::F(v) => Buf::F((0..ng).map(|k| fmax(&v[k * n..(k + 1) * n], mx, |x: f64| x.is_nan())).collect()),
                Buf::F32(v) => Buf::F32((0..ng).map(|k| fmax(&v[k * n..(k + 1) * n], mx, |x: f32| x.is_nan())).collect()),
                Buf::I(v) => Buf::I((0..ng).map(|k| { let g = v[k * n..(k + 1) * n].iter(); if mx { *g.max().unwrap() } else { *g.min().unwrap() } }).collect()),
                Buf::I32(v) => Buf::I32((0..ng).map(|k| { let g = v[k * n..(k + 1) * n].iter(); if mx { *g.max().unwrap() } else { *g.min().unwrap() } }).collect()),
                Buf::B(v) => Buf::B((0..ng).map(|k| { let g = &v[k * n..(k + 1) * n]; if mx { g.iter().any(|x| *x) } else { g.iter().all(|x| *x) } }).collect()),
                Buf::U(..) => unreachable!(),
            }
        }
        "argmax" | "argmin" => {
            if n == 0 {
                return val_err(format!("attempt to get {what} of an empty sequence"));
            }
            let mx = what == "argmax";
            // the first best; a NaN wins at once (as numpy's loops)
            fn arg<T: Copy>(g: &[T], better: impl Fn(T, T) -> bool, nan: impl Fn(T) -> bool) -> i64 {
                let (mut best, mut bv) = (0usize, g[0]);
                if nan(bv) {
                    return 0;
                }
                for (j, x) in g.iter().enumerate().skip(1) {
                    if nan(*x) {
                        return j as i64;
                    }
                    if better(*x, bv) {
                        best = j;
                        bv = *x;
                    }
                }
                best as i64
            }
            Buf::I(match &g.vals {
                Buf::F(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: f64, b: f64| if mx { a > b } else { a < b }, |a: f64| a.is_nan())).collect(),
                Buf::I(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: i64, b: i64| if mx { a > b } else { a < b }, |_| false)).collect(),
                Buf::F32(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: f32, b: f32| if mx { a > b } else { a < b }, |a: f32| a.is_nan())).collect(),
                Buf::I32(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: i32, b: i32| if mx { a > b } else { a < b }, |_| false)).collect(),
                Buf::B(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: bool, b: bool| if mx { a & !b } else { !a & b }, |_| false)).collect(),
                Buf::U(v, _) => {
                    let sv: Vec<Rc<str>> = v.iter().map(|i| np_sget(*i)).collect();
                    (0..ng).map(|k| arg(&(k * n..(k + 1) * n).collect::<Vec<usize>>(), |a: usize, b: usize| if mx { sv[a] > sv[b] } else { sv[a] < sv[b] }, |_| false)).collect()
                }
            })
        }
        "any" | "all" => {
            let v = g.vals.bc();
            Buf::B((0..ng).map(|k| { let g = &v[k * n..(k + 1) * n]; if what == "any" { g.iter().any(|x| *x) } else { g.iter().all(|x| *x) } }).collect())
        }
        "count_nonzero" => {
            let v = g.vals.bc();
            Buf::I((0..ng).map(|k| v[k * n..(k + 1) * n].iter().filter(|x| **x).count() as i64).collect())
        }
        _ => return Err(np_unsup(what)),
    };
    if matches!(what, "argmax" | "argmin") && o.axis.is_none() && o.keepdims {
        return Ok(np_new(out, vec![1; a.ndim()]));
    }
    Ok(np_out(out, shape))
}

// cumsum / cumprod (flattened when axis is None).
pub fn np_accum(a: &NdArr, what: &str, axis: Option<usize>) -> R {
    let (src, ax) = match axis {
        None => (NdArr::new(a.vals(), vec![a.size()]), 0),
        Some(x) => (a.clone(), x),
    };
    let g = np_groups(&src, Some(ax));
    let n = g.n;
    let ng = g.vals.len() / n.max(1);
    // bool and int32 accumulate in int64, as numpy's add/multiply.accumulate pick
    let d = if matches!(src.dt(), NT::B | NT::I32) { NT::I } else { src.dt() };
    let res: Buf = match g.vals.clone().cast(d) {
        Buf::F32(v) => {
            let mut o = v.clone();
            for k in 0..ng {
                for j in 1..n {
                    o[k * n + j] = if what == "cumsum" { o[k * n + j - 1] + v[k * n + j] } else { o[k * n + j - 1] * v[k * n + j] };
                }
            }
            Buf::F32(o)
        }
        Buf::F(v) => {
            let mut o = v.clone();
            for k in 0..ng {
                for j in 1..n {
                    o[k * n + j] = if what == "cumsum" { o[k * n + j - 1] + v[k * n + j] } else { o[k * n + j - 1] * v[k * n + j] };
                }
            }
            Buf::F(o)
        }
        Buf::I(v) => {
            let mut o = v.clone();
            for k in 0..ng {
                for j in 1..n {
                    o[k * n + j] = if what == "cumsum" { o[k * n + j - 1].wrapping_add(v[k * n + j]) } else { o[k * n + j - 1].wrapping_mul(v[k * n + j]) };
                }
            }
            Buf::I(o)
        }
        b => b,
    };
    // back from (rest..., axis) order to C order
    np_from_groups(&src, ax, res)
}

// Values in (rest..., axis) order (as np_groups makes them) back into an array of src's shape.
pub fn np_from_groups(src: &NdArr, ax: usize, vals: Buf) -> R {
    let nd = src.ndim();
    let mut order: Vec<usize> = (0..nd).filter(|k| *k != ax).collect();
    order.push(ax);
    let cs = cstrides(&src.shape);
    let gshape: Vec<usize> = order.iter().map(|k| src.shape[*k]).collect();
    let gst: Vec<isize> = order.iter().map(|k| cs[*k]).collect();
    let mut ps = vec![];
    walk_pos(0, &gshape, &gst, &mut ps);
    let mut out = Buf::zeros(vals.dt(), src.size());
    for (i, p) in ps.iter().enumerate() {
        out.set(*p, vals.get(i));
    }
    Ok(np_new(out, src.shape.clone()))
}
