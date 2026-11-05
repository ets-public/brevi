// ---------- numpy, natively ----------
// numpy arrays and scalars for dynamic builds: float64, int64 and bool arrays of any shape over a shared buffer
// (slices are views, as in numpy), numpy 2's type promotion (NEP 50: Python numbers are "weak"), broadcasting,
// ufuncs, reductions in numpy's summation order (pairwise along the contiguous axis), indexing (basic, boolean
// and integer arrays) and numpy's printing (rt_npfmt.rs). What it does not do is refused when the program is
// built (emit.rs np_* tables), so the program goes to CPython's numpy instead; a value it does not know at run
// time (a string array) raises NotImplementedError naming it.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum NT {
    B,
    I,
    F,
}
impl NT {
    pub fn name(self) -> &'static str {
        match self {
            NT::B => "bool",
            NT::I => "int64",
            NT::F => "float64",
        }
    }
    fn size(self) -> i64 {
        match self {
            NT::B => 1,
            _ => 8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Sc {
    B(bool),
    I(i64),
    F(f64),
}
impl Sc {
    pub fn dt(self) -> NT {
        match self {
            Sc::B(_) => NT::B,
            Sc::I(_) => NT::I,
            Sc::F(_) => NT::F,
        }
    }
    pub fn f(self) -> f64 {
        match self {
            Sc::B(b) => b as i64 as f64,
            Sc::I(i) => i as f64,
            Sc::F(x) => x,
        }
    }
    pub fn i(self) -> i64 {
        match self {
            Sc::B(b) => b as i64,
            Sc::I(i) => i,
            Sc::F(x) => f_to_i(x),
        }
    }
    pub fn b(self) -> bool {
        match self {
            Sc::B(b) => b,
            Sc::I(i) => i != 0,
            Sc::F(x) => x != 0.0,
        }
    }
    pub fn cast(self, d: NT) -> Sc {
        match d {
            NT::B => Sc::B(self.b()),
            NT::I => Sc::I(self.i()),
            NT::F => Sc::F(self.f()),
        }
    }
}
fn f_to_i(x: f64) -> i64 {
    if !(x >= -9.223372036854775808e18 && x < 9.223372036854775808e18) {
        np_warn("invalid value encountered in cast");
        return i64::MIN;
    }
    x as i64
}

#[derive(Clone)]
pub enum Buf {
    B(Vec<bool>),
    I(Vec<i64>),
    F(Vec<f64>),
}
impl Buf {
    pub fn dt(&self) -> NT {
        match self {
            Buf::B(_) => NT::B,
            Buf::I(_) => NT::I,
            Buf::F(_) => NT::F,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Buf::B(v) => v.len(),
            Buf::I(v) => v.len(),
            Buf::F(v) => v.len(),
        }
    }
    pub fn get(&self, p: usize) -> Sc {
        match self {
            Buf::B(v) => Sc::B(v[p]),
            Buf::I(v) => Sc::I(v[p]),
            Buf::F(v) => Sc::F(v[p]),
        }
    }
    pub fn set(&mut self, p: usize, x: Sc) {
        match self {
            Buf::B(v) => v[p] = x.b(),
            Buf::I(v) => v[p] = x.i(),
            Buf::F(v) => v[p] = x.f(),
        }
    }
    pub fn zeros(d: NT, n: usize) -> Buf {
        match d {
            NT::B => Buf::B(vec![false; n]),
            NT::I => Buf::I(vec![0; n]),
            NT::F => Buf::F(vec![0.0; n]),
        }
    }
    pub fn from_sc(d: NT, xs: &[Sc]) -> Buf {
        match d {
            NT::B => Buf::B(xs.iter().map(|x| x.b()).collect()),
            NT::I => Buf::I(xs.iter().map(|x| x.i()).collect()),
            NT::F => Buf::F(xs.iter().map(|x| x.f()).collect()),
        }
    }
    pub fn pick(&self, ps: &[usize]) -> Buf {
        match self {
            Buf::B(v) => Buf::B(ps.iter().map(|p| v[*p]).collect()),
            Buf::I(v) => Buf::I(ps.iter().map(|p| v[*p]).collect()),
            Buf::F(v) => Buf::F(ps.iter().map(|p| v[*p]).collect()),
        }
    }
    pub fn slice(&self, a: usize, n: usize) -> Buf {
        match self {
            Buf::B(v) => Buf::B(v[a..a + n].to_vec()),
            Buf::I(v) => Buf::I(v[a..a + n].to_vec()),
            Buf::F(v) => Buf::F(v[a..a + n].to_vec()),
        }
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
            (b, _) => b,
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
        None => {
            return Err(match v {
                V::Str(_) => np_unsup("an array of strings"),
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
            Some(d) => b.cast(d),
            None => b,
        }, a.shape.clone()));
    }
    let mut shape = vec![];
    let mut out = vec![];
    np_walk(v, 0, &mut shape, &mut out)?;
    let d = dt.unwrap_or_else(|| out.iter().map(|x| x.dt()).max().unwrap_or(NT::F));
    if dt == Some(NT::I) || dt == Some(NT::B) {
        if let (Some(NT::I), Some(x)) = (dt, out.iter().find(|x| matches!(x, Sc::F(f) if !f.is_finite()))) {
            let _ = x;
            return Err(exc("ValueError", "cannot convert float NaN to integer".into()));
        }
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
        return match a.first() {
            Sc::B(v) => Buf::B(vec![v; n]),
            Sc::I(v) => Buf::I(vec![v; n]),
            Sc::F(v) => Buf::F(vec![v; n]),
        };
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
    if seq_items(v).is_some() {
        return Ok(Some(Opnd { a: np_from(v, None)?, weak: false, scalar: false }));
    }
    Ok(None)
}
// The result type of a ufunc over these operands.
pub fn np_rtype(ops: &[&Opnd]) -> NT {
    let strong = ops.iter().filter(|o| !o.weak).map(|o| o.a.dt()).max();
    let mut d = strong.unwrap_or_else(|| ops.iter().map(|o| o.a.dt()).max().unwrap_or(NT::F));
    for o in ops.iter().filter(|o| o.weak) {
        let w = o.a.dt();
        if w == NT::F && d < NT::F {
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
            if matches!(op, "/" | "//" | "%") || (op == "**" && a == 0.0) {
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
fn kern_i(op: &str, x: &[i64], y: &[i64], scalar: bool) -> Result<Buf, E> {
    let n = x.len();
    let mut r = Vec::with_capacity(n);
    let sc = if scalar { "scalar " } else { "" };
    let mut ov = false;
    let mut dz = false;
    for i in 0..n {
        let (a, b) = (x[i], y[i]);
        r.push(match op {
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
        });
    }
    if dz {
        np_warn(&format!("divide by zero encountered in {sc}{}", ufunc_name(op)));
    }
    if ov && scalar {
        np_warn(&format!("overflow encountered in {sc}{}", ufunc_name(op)));
    }
    Ok(Buf::I(r))
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
        let d = np_rtype(&[&x, &y]);
        if (d == NT::F || op == "/") && matches!(op, "&" | "|" | "^" | "<<" | ">>") {
            return Err(no_loop(op));
        }
        let shape = np_bshape(&x.a.shape, &y.a.shape)?;
        let cmp = matches!(op, "<" | ">" | "<=" | ">=" | "==" | "!=");
        let r = if d == NT::F || op == "/" {
            if matches!(op, "&" | "|" | "^" | "<<" | ">>") {
                return Err(no_loop(op));
            }
            // a one-value side stays one value (the kernel spreads it); a float array of the
            // result's shape is read in place
            np_with_f(&x.a, &shape, |xv| {
                np_with_f(&y.a, &shape, |yv| {
                    let r = kern_f(op, xv, yv)?;
                    if let Buf::F(rv) = &r {
                        if rv.iter().any(|v| !v.is_finite()) {
                            let full = |v: &[f64]| if v.len() == rv.len() { v.to_vec() } else { vec![v[0]; rv.len()] };
                            fwarn(op, scalar, &full(xv), &full(yv), rv);
                        }
                    }
                    Ok(r)
                })
            })?
        } else if d == NT::I {
            if matches!(op, "<<" | ">>") || cmp || true {
                kern_i(op, &np_bvals(&x.a, &shape).iv(), &np_bvals(&y.a, &shape).iv(), scalar)?
            } else {
                unreachable!()
            }
        } else {
            if matches!(op, "<<" | ">>") {
                kern_i(op, &np_bvals(&x.a, &shape).iv(), &np_bvals(&y.a, &shape).iv(), scalar)?
            } else {
                kern_b(op, &np_bvals(&x.a, &shape).bv(), &np_bvals(&y.a, &shape).bv())?
            }
        };
        Ok(np_out(r, shape))
    })())
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
    if rb.dt() > d {
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
    let r = match (op, vals) {
        ("neg", Buf::B(_)) => return type_err("The numpy boolean negative, the `-` operator, is not supported, use the `~` operator or the logical_not function instead.".into()),
        ("neg", Buf::I(x)) => Buf::I(x.iter().map(|i| i.wrapping_neg()).collect()),
        ("neg", Buf::F(x)) => Buf::F(x.iter().map(|i| -i).collect()),
        ("pos", Buf::B(_)) => return type_err("ufunc 'positive' did not contain a loop with signature matching types <class 'numpy.dtypes.BoolDType'> -> None".into()),
        ("pos", b) => b,
        ("invert", Buf::B(x)) => Buf::B(x.iter().map(|b| !b).collect()),
        ("invert", Buf::I(x)) => Buf::I(x.iter().map(|i| !i).collect()),
        ("invert", Buf::F(_)) => return Err(no_loop("invert")).map_err(|_| exc("TypeError", "ufunc 'invert' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''".into())),
        ("abs", Buf::B(x)) => Buf::B(x),
        ("abs", Buf::I(x)) => Buf::I(x.iter().map(|i| i.wrapping_abs()).collect()),
        ("abs", Buf::F(x)) => Buf::F(x.iter().map(|i| i.abs()).collect()),
        _ => return Err(np_unsup(op)),
    };
    Ok(np_out(r, o.a.shape.clone()))
}

// a OP b for comparisons, as a value: arrays compare elementwise.
pub fn cmp_v(a: &V, b: &V, op: &str) -> R {
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
    Ok(V::Bool(cmp_op(a, b, op)?))
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

fn dot_f(x: &[f64], y: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..x.len() {
        s += x[i] * y[i];
    }
    s
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
    let mut d = x.dt().max(y.dt());
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
    let d = x.dt().max(y.dt());
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
            let mut o = vec![0.0; m * n];
            let mut col = vec![0.0; k];
            for j in 0..n {
                for t in 0..k {
                    col[t] = yv[t * n + j];
                }
                for i in 0..m {
                    o[i * n + j] = dot_f(&xv[i * k..(i + 1) * k], &col);
                }
            }
            Buf::F(o)
        }
        NT::I | NT::B if d == NT::I => {
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
            Buf::I(o)
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
                if a.size() == 0 && a.dt() == NT::F {
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
                        Sc::I(k) => Ix::Int(k),
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
            if a.dt() == NT::F {
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
    if d != NT::F {
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
                np_pw(grp)
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
            match &g.vals {
                Buf::F(v) => Buf::F(
                    (0..ng)
                        .map(|k| {
                            let grp = &v[k * n..(k + 1) * n];
                            let mut m = grp[0];
                            for x in &grp[1..] {
                                if m.is_nan() {
                                    break;
                                }
                                // ties take the later value (0.0 and -0.0), as numpy's loops do
                                if x.is_nan() || (mx && !(m > *x)) || (!mx && !(m < *x)) {
                                    m = *x;
                                }
                            }
                            m
                        })
                        .collect(),
                ),
                Buf::I(v) => Buf::I((0..ng).map(|k| { let g = v[k * n..(k + 1) * n].iter(); if mx { *g.max().unwrap() } else { *g.min().unwrap() } }).collect()),
                Buf::B(v) => Buf::B((0..ng).map(|k| { let g = &v[k * n..(k + 1) * n]; if mx { g.iter().any(|x| *x) } else { g.iter().all(|x| *x) } }).collect()),
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
                Buf::B(v) => (0..ng).map(|k| arg(&v[k * n..(k + 1) * n], |a: bool, b: bool| if mx { a & !b } else { !a & b }, |_| false)).collect(),
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
    let d = if src.dt() == NT::B { NT::I } else { src.dt() };
    let res: Buf = match g.vals.clone().cast(d) {
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
