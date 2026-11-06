// ---------- numpy functions, methods and value hooks ----------

fn npa(a: &Args, i: usize, name: &str) -> Option<V> {
    if let Some((_, v)) = a.k.iter().find(|(n, _)| *n == name) {
        return Some(v.clone());
    }
    a.p.get(i).cloned()
}
fn npa_kw_ok(a: &Args, f: &str, ok: &[&str]) -> Result<(), E> {
    for (n, _) in &a.k {
        if !ok.contains(n) {
            return type_err(format!("{f}() got an unexpected keyword argument '{n}'"));
        }
    }
    Ok(())
}
fn np_int_arg(v: &V) -> Result<i64, E> {
    match as_npsc(v) {
        Some(Sc::I(i)) => Ok(i),
        Some(Sc::B(b)) => Ok(b as i64),
        Some(Sc::F(_)) => type_err("'numpy.float64' object cannot be interpreted as an integer".into()),
        None => match v {
            V::Float(_) => type_err("'float' object cannot be interpreted as an integer".into()),
            _ => idx_of(v),
        },
    }
}
fn np_f_arg(v: &V) -> Result<f64, E> {
    match as_npsc(v) {
        Some(s) => Ok(s.f()),
        None => to_float(v),
    }
}
fn np_bool_arg(v: &V) -> Result<bool, E> {
    truth(v)
}
fn np_shape_arg(v: &V) -> Result<Vec<usize>, E> {
    let xs = match v {
        V::Tuple(_) | V::List(_) => to_vec(v)?,
        _ => vec![v.clone()],
    };
    let mut out = vec![];
    for x in xs {
        let n = np_int_arg(&x)?;
        if n < 0 {
            return val_err("negative dimensions are not allowed".into());
        }
        out.push(n as usize);
    }
    Ok(out)
}
fn np_axis_arg(v: Option<V>, nd: usize) -> Result<Option<usize>, E> {
    match v {
        None | Some(V::None) => Ok(None),
        Some(V::Tuple(t)) if t.len() == 1 => Ok(Some(norm_axis(np_int_arg(&t[0])?, nd)?)),
        Some(V::Tuple(_)) => Err(np_unsup("a tuple of axes")),
        Some(x) => Ok(Some(norm_axis(np_int_arg(&x)?, nd)?)),
    }
}
fn shape_tuple(s: &[usize]) -> V {
    tup(s.iter().map(|x| V::Int(*x as i64)).collect())
}
fn np_py(x: Sc) -> V {
    match x {
        Sc::B(b) => V::Bool(b),
        Sc::I(i) => V::Int(i),
        Sc::F(f) => V::Float(f),
    }
}
fn np_tolist(vals: &Buf, shape: &[usize], off: usize) -> V {
    if shape.is_empty() {
        return np_py(vals.get(off));
    }
    let inner: usize = shape[1..].iter().product();
    list((0..shape[0]).map(|i| np_tolist(vals, &shape[1..], off + i * inner)).collect())
}
fn np_view(a: &NdArr, shape: Vec<usize>, st: Vec<isize>, off: usize) -> NdArr {
    NdArr { buf: a.buf.clone(), off, shape, st }
}
fn np_transpose(a: &NdArr, axes: Option<Vec<usize>>) -> Result<NdArr, E> {
    let p = axes.unwrap_or_else(|| (0..a.ndim()).rev().collect());
    if p.len() != a.ndim() {
        return val_err("axes don't match array".into());
    }
    Ok(np_view(a, p.iter().map(|k| a.shape[*k]).collect(), p.iter().map(|k| a.st[*k]).collect(), a.off))
}
fn np_reshape(a: &NdArr, dims: &[i64]) -> Result<NdArr, E> {
    let n = a.size();
    let known: i64 = dims.iter().filter(|d| **d >= 0).product();
    let neg = dims.iter().filter(|d| **d < 0).count();
    if neg > 1 {
        return val_err("can only specify one unknown dimension".into());
    }
    let shape: Vec<usize> = dims.iter().map(|d| if *d < 0 { if known == 0 { 0 } else { (n as i64 / known) as usize } } else { *d as usize }).collect();
    if shape.iter().product::<usize>() != n || (neg == 1 && known != 0 && n as i64 % known != 0) {
        return val_err(format!("cannot reshape array of size {n} into shape {}", np_shape_repr_py(dims)));
    }
    Ok(a.reshaped(shape))
}
fn np_shape_repr_py(d: &[i64]) -> String {
    match d.len() {
        1 => format!("({},)", d[0]),
        _ => format!("({})", d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
    }
}
fn np_dims_args(a: &Args, from: usize) -> Result<Vec<i64>, E> {
    let xs: Vec<V> = if a.p.len() == from + 1 && matches!(a.p[from], V::Tuple(_) | V::List(_)) { to_vec(&a.p[from])? } else { a.p[from..].to_vec() };
    xs.iter().map(np_int_arg).collect()
}

// Sorting: numpy's order, NaN last.
fn np_lt(a: Sc, b: Sc) -> bool {
    match (a, b) {
        (Sc::F(x), Sc::F(y)) => x < y || (y.is_nan() && !x.is_nan()),
        (x, y) if x.dt() == NT::F || y.dt() == NT::F => {
            let (x, y) = (x.f(), y.f());
            x < y || (y.is_nan() && !x.is_nan())
        }
        (x, y) => x.i() < y.i(),
    }
}
fn np_argsort_vals(v: &Buf, lo: usize, n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    match v {
        Buf::F(x) => idx.sort_by(|i, j| {
            let (a, b) = (x[lo + i], x[lo + j]);
            if np_lt(Sc::F(a), Sc::F(b)) {
                std::cmp::Ordering::Less
            } else if np_lt(Sc::F(b), Sc::F(a)) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        }),
        Buf::I(x) => idx.sort_by_key(|i| x[lo + i]),
        Buf::B(x) => idx.sort_by_key(|i| x[lo + i]),
    }
    idx
}
fn np_sorted(a: &NdArr, axis: Option<usize>, arg: bool) -> R {
    let (src, ax) = match axis {
        None => (NdArr::new(a.vals(), vec![a.size()]), 0),
        Some(x) => (a.clone(), x),
    };
    if src.ndim() == 0 {
        return Err(exc("AxisError", "axis -1 is out of bounds for array of dimension 0".into()));
    }
    let g = np_groups(&src, Some(ax));
    let n = g.n;
    let ng = if n == 0 { 0 } else { g.vals.len() / n };
    let mut out = if arg { Buf::I(vec![0; g.vals.len()]) } else { Buf::zeros(g.vals.dt(), g.vals.len()) };
    for k in 0..ng {
        let idx = np_argsort_vals(&g.vals, k * n, n);
        for (j, i) in idx.iter().enumerate() {
            out.set(k * n + j, if arg { Sc::I(*i as i64) } else { g.vals.get(k * n + i) });
        }
    }
    np_from_groups(&src, ax, out)
}

fn np_concat(xs: &[NdArr], axis: Option<usize>) -> R {
    if xs.is_empty() {
        return val_err("need at least one array to concatenate".into());
    }
    let d = xs.iter().map(|a| a.dt()).max().unwrap();
    let xs: Vec<NdArr> = match axis {
        None => xs.iter().map(|a| NdArr::new(a.vals(), vec![a.size()])).collect(),
        Some(_) => xs.to_vec(),
    };
    let ax = axis.unwrap_or(0);
    let nd = xs[0].ndim();
    if nd == 0 {
        return val_err("zero-dimensional arrays cannot be concatenated".into());
    }
    if ax >= nd {
        return Err(exc("AxisError", format!("axis {ax} is out of bounds for array of dimension {nd}")));
    }
    for (i, a) in xs.iter().enumerate() {
        if a.ndim() != nd {
            return val_err(format!("all the input array dimensions except for the concatenation axis must match exactly, but along dimension 0, the array at index 0 has {} dimension(s) and the array at index {i} has {} dimension(s)", nd, a.ndim()).replace("all the input array dimensions except for the concatenation axis must match exactly, but along dimension 0, the", "all the input arrays must have same number of dimensions, but the"));
        }
        for k in 0..nd {
            if k != ax && a.shape[k] != xs[0].shape[k] {
                return val_err(format!("all the input array dimensions except for the concatenation axis must match exactly, but along dimension {k}, the array at index 0 has size {} and the array at index {i} has size {}", xs[0].shape[k], a.shape[k]));
            }
        }
    }
    let mut shape = xs[0].shape.clone();
    shape[ax] = xs.iter().map(|a| a.shape[ax]).sum();
    let outer: usize = shape[..ax].iter().product();
    let mut out: Vec<Sc> = Vec::with_capacity(shape.iter().product());
    let parts: Vec<Buf> = xs.iter().map(|a| a.vals().cast(d)).collect();
    for o in 0..outer {
        for (a, b) in xs.iter().zip(&parts) {
            let chunk: usize = a.shape[ax..].iter().product();
            for j in 0..chunk {
                out.push(b.get(o * chunk + j));
            }
        }
    }
    Ok(np_new(Buf::from_sc(d, &out), shape))
}
fn np_seq_arrays(v: &V) -> Result<Vec<NdArr>, E> {
    to_vec(v)?.iter().map(np_as).collect()
}
fn atleast(a: &NdArr, n: usize) -> NdArr {
    match (a.ndim(), n) {
        (0, 1) => a.reshaped(vec![1]),
        (0, 2) => a.reshaped(vec![1, 1]),
        (1, 2) => a.reshaped(vec![1, a.shape[0]]),
        _ => a.clone(),
    }
}

// Elementwise math on floats (ints become float64; bools would be float16, which Brevi does not have).
fn np_ufunc1(name: &str, v: &V) -> R {
    let o = match np_opnd(v)? {
        Some(o) => o,
        None => return type_err(format!("ufunc '{name}' not supported for the input types")),
    };
    let shape = o.a.shape.clone();
    let d = o.a.dt();
    // the ones that keep integers
    match name {
        "absolute" | "abs" => return np_unary(&np_out(o.a.vals(), shape.clone()).clone_or(v), "abs"),
        "negative" => return np_unary(v, "neg"),
        "positive" => return np_unary(v, "pos"),
        "square" if d != NT::F => {
            if d == NT::B {
                return Err(np_unsup("np.square of a bool array (numpy gives int8)"));
            }
            return Ok(np_out(Buf::I(o.a.iv().iter().map(|x| x.wrapping_mul(*x)).collect()), shape));
        }
        "sign" if d != NT::F => {
            if d == NT::B {
                return Err(exc("UFuncTypeError", "ufunc 'sign' did not contain a loop with signature matching types <class 'numpy.dtypes.BoolDType'> -> None".into()));
            }
            return Ok(np_out(Buf::I(o.a.iv().iter().map(|x| x.signum()).collect()), shape));
        }
        "floor" | "ceil" | "trunc" | "rint" | "fix" if d != NT::F => return Ok(np_out(o.a.vals(), shape)),
        "isnan" | "isinf" | "isfinite" => {
            let v = o.a.fv();
            return Ok(np_out(
                Buf::B(v.iter().map(|x| match name {
                    "isnan" => x.is_nan(),
                    "isinf" => x.is_infinite(),
                    _ => x.is_finite(),
                }).collect()),
                shape,
            ));
        }
        "logical_not" => return Ok(np_out(Buf::B(o.a.bv().iter().map(|b| !b).collect()), shape)),
        _ => {}
    }
    if d == NT::B {
        return Err(np_unsup(&format!("np.{name} of a bool array (numpy gives float16)")));
    }
    let shp = o.a.shape.clone();
    np_with_f(&o.a, &shp, |x| np_ufunc1_f(name, x, shape))
}
fn np_ufunc1_f(name: &str, x: &[f64], shape: Vec<usize>) -> R {
    let f: fn(f64) -> f64 = match name {
        "sqrt" => f64::sqrt,
        "exp" => f64::exp,
        "exp2" => f64::exp2,
        "expm1" => f64::exp_m1,
        "log" => f64::ln,
        "log10" => f64::log10,
        "log2" => f64::log2,
        "log1p" => f64::ln_1p,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "arcsin" => f64::asin,
        "arccos" => f64::acos,
        "arctan" => f64::atan,
        "sinh" => f64::sinh,
        "cosh" => f64::cosh,
        "tanh" => f64::tanh,
        "arcsinh" => f64::asinh,
        "arccosh" => f64::acosh,
        "arctanh" => f64::atanh,
        "cbrt" => f64::cbrt,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        "trunc" | "fix" => f64::trunc,
        "rint" => rint,
        "square" => |x| x * x,
        "reciprocal" => |x| 1.0 / x,
        "sign" => |x: f64| if x.is_nan() { f64::NAN } else if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 },
        "degrees" | "rad2deg" => f64::to_degrees,
        "radians" | "deg2rad" => f64::to_radians,
        _ => return Err(np_unsup(&format!("np.{name}"))),
    };
    let r: Vec<f64> = x.iter().map(|v| f(*v)).collect();
    let (mut dz, mut inv, mut ov) = (false, false, false);
    for (a, b) in x.iter().zip(&r) {
        if b.is_finite() {
            continue;
        }
        if b.is_nan() && !a.is_nan() {
            inv = true;
        } else if b.is_infinite() && a.is_finite() {
            if matches!(name, "log" | "log10" | "log2" | "log1p" | "reciprocal" | "arctanh") {
                dz = true;
            } else {
                ov = true;
            }
        }
    }
    if dz {
        np_warn(&format!("divide by zero encountered in {name}"));
    }
    if ov {
        np_warn(&format!("overflow encountered in {name}"));
    }
    if inv {
        np_warn(&format!("invalid value encountered in {name}"));
    }
    Ok(np_out(Buf::F(r), shape))
}
trait CloneOr {
    fn clone_or(&self, v: &V) -> V;
}
impl CloneOr for V {
    fn clone_or(&self, _v: &V) -> V {
        self.clone()
    }
}

// Two-argument ufuncs by name.
fn np_ufunc2(name: &str, x: &V, y: &V) -> R {
    let op = match name {
        "add" => "+",
        "subtract" => "-",
        "multiply" => "*",
        "divide" | "true_divide" => "/",
        "floor_divide" => "//",
        "mod" | "remainder" => "%",
        "power" => "**",
        "equal" => "==",
        "not_equal" => "!=",
        "less" => "<",
        "less_equal" => "<=",
        "greater" => ">",
        "greater_equal" => ">=",
        "bitwise_and" => "&",
        "bitwise_or" => "|",
        "bitwise_xor" => "^",
        "left_shift" => "<<",
        "right_shift" => ">>",
        _ => "",
    };
    if !op.is_empty() {
        let (xa, ya) = (if is_np(x) || is_np(y) { x.clone() } else { np_arr0(x)? }, y.clone());
        return np_binop(&xa, &ya, op).unwrap();
    }
    let (a, b) = match (np_opnd(x)?, np_opnd(y)?) {
        (Some(a), Some(b)) => (a, b),
        _ => return type_err(format!("ufunc '{name}' not supported for the input types")),
    };
    let shape = np_bshape(&a.a.shape, &b.a.shape)?;
    let scalar = a.scalar && b.scalar;
    if matches!(name, "logical_and" | "logical_or" | "logical_xor") {
        let (u, v) = (np_bvals(&a.a, &shape).bv(), np_bvals(&b.a, &shape).bv());
        let r: Vec<bool> = u.iter().zip(&v).map(|(p, q)| match name {
            "logical_and" => *p && *q,
            "logical_or" => *p || *q,
            _ => p ^ q,
        }).collect();
        return Ok(np_out(Buf::B(r), shape));
    }
    let d = np_rtype(&[&a, &b]);
    let _ = scalar;
    match name {
        "maximum" | "minimum" | "fmax" | "fmin" => {
            let mx = name.ends_with("max") || name == "maximum";
            let fnan = name.starts_with('f');
            let r = match d {
                NT::F => {
                    let (u, v) = (np_bvals(&a.a, &shape).fv(), np_bvals(&b.a, &shape).fv());
                    Buf::F(u.iter().zip(&v).map(|(p, q)| {
                        let (p, q) = (*p, *q);
                        if p.is_nan() || q.is_nan() {
                            if fnan {
                                if p.is_nan() { q } else { p }
                            } else if p.is_nan() {
                                p
                            } else {
                                q
                            }
                        } else if mx {
                            if p > q { p } else { q }
                        } else if p < q {
                            p
                        } else {
                            q
                        }
                    }).collect())
                }
                NT::I => {
                    let (u, v) = (np_bvals(&a.a, &shape).iv(), np_bvals(&b.a, &shape).iv());
                    Buf::I(u.iter().zip(&v).map(|(p, q)| if mx { *p.max(q) } else { *p.min(q) }).collect())
                }
                NT::B => {
                    let (u, v) = (np_bvals(&a.a, &shape).bv(), np_bvals(&b.a, &shape).bv());
                    Buf::B(u.iter().zip(&v).map(|(p, q)| if mx { *p || *q } else { *p && *q }).collect())
                }
            };
            Ok(np_out(r, shape))
        }
        "arctan2" | "hypot" | "copysign" | "fmod" | "logaddexp" => {
            if d == NT::B {
                return Err(np_unsup(&format!("np.{name} of bool arrays")));
            }
            let (u, v) = (np_bvals(&a.a, &shape).fv(), np_bvals(&b.a, &shape).fv());
            let r: Vec<f64> = u.iter().zip(&v).map(|(p, q)| match name {
                "arctan2" => p.atan2(*q),
                "hypot" => p.hypot(*q),
                "copysign" => p.copysign(*q),
                "fmod" => p % q,
                _ => {
                    let m = p.max(*q);
                    if m.is_infinite() { m } else { m + ((p - m).exp() + (q - m).exp()).ln() }
                }
            }).collect();
            Ok(np_out(Buf::F(r), shape))
        }
        _ => Err(np_unsup(&format!("np.{name}"))),
    }
}
fn np_arr0(x: &V) -> R {
    // two Python numbers given to a ufunc: numpy treats them as 0-d arrays of the default types
    let a = np_from(x, None)?;
    Ok(np_arr(a))
}

fn np_isclose(a: &V, b: &V, rtol: f64, atol: f64, equal_nan: bool) -> R {
    let (x, y) = (np_opnd(a)?.ok_or_else(|| exc("TypeError", "isclose: bad input".into()))?, np_opnd(b)?.ok_or_else(|| exc("TypeError", "isclose: bad input".into()))?);
    let shape = np_bshape(&x.a.shape, &y.a.shape)?;
    let (u, v) = (np_bvals(&x.a, &shape).fv(), np_bvals(&y.a, &shape).fv());
    let r: Vec<bool> = u
        .iter()
        .zip(&v)
        .map(|(p, q)| {
            let (p, q) = (*p, *q);
            if p.is_nan() || q.is_nan() {
                return equal_nan && p.is_nan() && q.is_nan();
            }
            if p.is_infinite() || q.is_infinite() {
                return p == q;
            }
            (p - q).abs() <= atol + rtol * q.abs()
        })
        .collect();
    Ok(np_out(Buf::B(r), shape))
}

fn np_round_buf(b: Buf, dec: i64) -> Buf {
    match b {
        Buf::B(v) => Buf::B(v),
        Buf::F(v) => {
            if dec >= 0 {
                let f = 10f64.powi(dec as i32);
                Buf::F(v.iter().map(|x| rint(x * f) / f).collect())
            } else {
                let f = 10f64.powi((-dec) as i32);
                Buf::F(v.iter().map(|x| rint(x / f) * f).collect())
            }
        }
        Buf::I(v) if dec < 0 => {
            let f = 10f64.powi((-dec) as i32);
            Buf::I(v.iter().map(|x| (rint(*x as f64 / f) * f) as i64).collect())
        }
        b => b,
    }
}

fn np_linspace(a: &Args) -> R {
    npa_kw_ok(a, "linspace", &["start", "stop", "num", "endpoint", "retstep", "dtype"])?;
    let start = np_f_arg(&npa(a, 0, "start").ok_or_else(|| exc("TypeError", "linspace() missing required argument 'start' (pos 0)".into()))?)?;
    let stop = np_f_arg(&npa(a, 1, "stop").ok_or_else(|| exc("TypeError", "linspace() missing required argument 'stop' (pos 1)".into()))?)?;
    let num = match npa(a, 2, "num") {
        Some(v) => np_int_arg(&v)?,
        None => 50,
    };
    if num < 0 {
        return val_err(format!("Number of samples, {num}, must be non-negative."));
    }
    let endpoint = match npa(a, 3, "endpoint") {
        Some(v) => np_bool_arg(&v)?,
        None => true,
    };
    let retstep = match npa(a, 4, "retstep") {
        Some(v) => np_bool_arg(&v)?,
        None => false,
    };
    let dt = match npa(a, 5, "dtype") {
        Some(v) => np_dtype_arg(&v)?,
        None => None,
    };
    let n = num as usize;
    let div = if endpoint { n.saturating_sub(1) } else { n };
    let delta = stop - start;
    let mut y: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let step;
    if div > 0 {
        step = delta / div as f64;
        if step == 0.0 {
            for v in y.iter_mut() {
                *v = *v / div as f64 * delta;
            }
        } else {
            for v in y.iter_mut() {
                *v *= step;
            }
        }
    } else {
        step = f64::NAN;
        for v in y.iter_mut() {
            *v *= delta;
        }
    }
    for v in y.iter_mut() {
        *v += start;
    }
    if endpoint && n > 1 {
        y[n - 1] = stop;
    }
    let b = match dt {
        Some(NT::I) => Buf::I(y.iter().map(|x| x.floor() as i64).collect()),
        Some(NT::B) => Buf::F(y).cast(NT::B),
        _ => Buf::F(y),
    };
    let arr = np_new(b, vec![n]);
    if retstep {
        return Ok(tup(vec![arr, np_sc(Sc::F(step))]));
    }
    Ok(arr)
}

fn np_arange(a: &Args) -> R {
    npa_kw_ok(a, "arange", &["start", "stop", "step", "dtype"])?;
    let dt = match a.k.iter().find(|(n, _)| *n == "dtype") {
        Some((_, v)) => np_dtype_arg(v)?,
        None => None,
    };
    let mut ps: Vec<V> = a.p.clone();
    for n in ["start", "stop", "step"] {
        if let Some((_, v)) = a.k.iter().find(|(k, _)| *k == n) {
            ps.push(v.clone());
        }
    }
    let (start, stop, step) = match ps.len() {
        1 => (V::Int(0), ps[0].clone(), V::Int(1)),
        2 => (ps[0].clone(), ps[1].clone(), V::Int(1)),
        3 => (ps[0].clone(), ps[1].clone(), ps[2].clone()),
        _ => return type_err("arange() requires stop to be specified.".into()),
    };
    let ops: Vec<Opnd> = [&start, &stop, &step].iter().map(|v| np_opnd(v).ok().flatten().ok_or_else(|| exc("TypeError", "unsupported arange argument".into()))).collect::<Result<_, _>>()?;
    let d = dt.unwrap_or_else(|| {
        let m = ops.iter().map(|o| o.a.dt()).max().unwrap();
        if m == NT::B {
            NT::I
        } else {
            m
        }
    });
    let (s0, s1, st) = (ops[0].a.first(), ops[1].a.first(), ops[2].a.first());
    if st.f() == 0.0 {
        return Err(exc("ZeroDivisionError", "division by zero".into()));
    }
    let len = ((s1.f() - s0.f()) / st.f()).ceil();
    let n = if len.is_nan() || len <= 0.0 { 0 } else { len as usize };
    let b = if d == NT::I && ops.iter().all(|o| o.a.dt() != NT::F) {
        let (a0, s) = (s0.i(), st.i());
        Buf::I((0..n).map(|i| a0.wrapping_add((i as i64).wrapping_mul(s))).collect())
    } else {
        let a0 = s0.f();
        let delta = (a0 + st.f()) - a0;
        let mut v: Vec<f64> = (0..n).map(|i| a0 + i as f64 * delta).collect();
        if n > 1 {
            v[1] = a0 + st.f();
        }
        Buf::F(v).cast(d)
    };
    Ok(np_new(b, vec![n]))
}

fn np_full(shape: Vec<usize>, x: Sc, d: NT) -> V {
    let n: usize = shape.iter().product();
    np_new(Buf::from_sc(d, &vec![x; n]), shape)
}

fn np_eye(a: &Args) -> R {
    npa_kw_ok(a, "eye", &["N", "M", "k", "dtype"])?;
    let n = np_int_arg(&npa(a, 0, "N").ok_or_else(|| exc("TypeError", "eye() missing required argument 'N' (pos 0)".into()))?)? as usize;
    let m = match npa(a, 1, "M") {
        Some(V::None) | None => n,
        Some(v) => np_int_arg(&v)? as usize,
    };
    let k = match npa(a, 2, "k") {
        Some(v) => np_int_arg(&v)?,
        None => 0,
    };
    let d = match npa(a, 3, "dtype") {
        Some(v) => np_dtype_arg(&v)?.unwrap_or(NT::F),
        None => NT::F,
    };
    let mut b = Buf::zeros(d, n * m);
    for i in 0..n {
        let j = i as i64 + k;
        if j >= 0 && (j as usize) < m {
            b.set(i * m + j as usize, Sc::I(1));
        }
    }
    Ok(np_new(b, vec![n, m]))
}

fn np_where(a: &Args) -> R {
    let c = np_as(&a.p[0])?;
    if a.p.len() == 1 {
        return np_nonzero(&c);
    }
    if a.p.len() != 3 {
        return val_err("either both or neither of x and y should be given".into());
    }
    let (x, y) = (np_opnd(&a.p[1])?.ok_or_else(|| np_unsup("np.where with these values"))?, np_opnd(&a.p[2])?.ok_or_else(|| np_unsup("np.where with these values"))?);
    let shape = np_bshape(&np_bshape(&c.shape, &x.a.shape)?, &y.a.shape)?;
    let d = np_rtype(&[&x, &y]);
    let cv = np_bvals(&c, &shape).bv();
    let (xv, yv) = (np_bvals(&x.a, &shape).cast(d), np_bvals(&y.a, &shape).cast(d));
    let mut out = Buf::zeros(d, cv.len());
    for (i, b) in cv.iter().enumerate() {
        out.set(i, if *b { xv.get(i) } else { yv.get(i) });
    }
    Ok(np_new(out, shape))
}
fn np_nonzero(c: &NdArr) -> R {
    if c.ndim() == 0 {
        return val_err("Calling nonzero on 0d arrays is not allowed. Use np.atleast_1d(scalar).nonzero() instead. If the context of this error is of the form `arr[nonzero(cond)]`, just use `arr[cond]`.".into());
    }
    let bv = c.bv();
    let cs = cstrides(&c.shape);
    let mut cols: Vec<Vec<i64>> = vec![vec![]; c.ndim()];
    for (p, b) in bv.iter().enumerate() {
        if *b {
            let mut r = p;
            for k in 0..c.ndim() {
                let s = cs[k] as usize;
                cols[k].push((r / s) as i64);
                r %= s;
            }
        }
    }
    Ok(tup(cols.into_iter().map(|v| { let n = v.len(); np_new(Buf::I(v), vec![n]) }).collect()))
}

fn np_unique(a: &Args) -> R {
    npa_kw_ok(a, "unique", &["ar", "return_index", "return_inverse", "return_counts", "axis", "equal_nan"])?;
    let x = np_as(&npa(a, 0, "ar").unwrap())?;
    if matches!(npa(a, 4, "axis"), Some(v) if !matches!(v, V::None)) {
        return Err(np_unsup("np.unique with axis"));
    }
    let flag = |i: usize, n: &str| -> Result<bool, E> { npa(a, i, n).map_or(Ok(false), |v| truth(&v)) };
    let (ri, rv, rc) = (flag(1, "return_index")?, flag(2, "return_inverse")?, flag(3, "return_counts")?);
    let vals = x.vals();
    let n = vals.len();
    let order = np_argsort_vals(&vals, 0, n);
    let mut uniq: Vec<usize> = vec![]; // first index (in sorted order) of each group
    let same = |p: Sc, q: Sc| match (p, q) {
        (Sc::F(a), Sc::F(b)) => a == b || (a.is_nan() && b.is_nan()),
        (a, b) => a.i() == b.i(),
    };
    for (j, i) in order.iter().enumerate() {
        if j == 0 || !same(vals.get(order[j - 1]), vals.get(*i)) {
            uniq.push(j);
        }
    }
    let u: Vec<Sc> = uniq.iter().map(|j| vals.get(order[*j])).collect();
    let nu = u.len();
    let mut outs = vec![np_new(Buf::from_sc(vals.dt(), &u), vec![nu])];
    if ri {
        outs.push(np_new(Buf::I(uniq.iter().map(|j| order[*j] as i64).collect()), vec![nu]));
    }
    if rv {
        let mut inv = vec![0i64; n];
        let mut g = 0usize;
        for (j, i) in order.iter().enumerate() {
            if g + 1 < nu && j == uniq[g + 1] {
                g += 1;
            }
            inv[*i] = g as i64;
        }
        outs.push(np_new(Buf::I(inv), if x.ndim() == 0 { vec![1] } else { vec![n] }));
    }
    if rc {
        let mut c = vec![];
        for (g, j) in uniq.iter().enumerate() {
            let end = if g + 1 < nu { uniq[g + 1] } else { n };
            c.push((end - j) as i64);
        }
        outs.push(np_new(Buf::I(c), vec![nu]));
    }
    Ok(if outs.len() == 1 { outs.pop().unwrap() } else { tup(outs) })
}

// median / percentile / quantile, numpy's "linear" method.
fn np_quantile(x: &NdArr, qs: &V, axis: Option<usize>, scale: f64, keepdims: bool) -> R {
    if x.dt() == NT::B {
        return type_err("numpy boolean subtract, the `-` operator, is not supported, use the bitwise_xor, the `^` operator, or the logical_xor function instead.".into());
    }
    let qo = np_as(qs)?;
    let qv: Vec<f64> = qo.fv().iter().map(|q| q / scale).collect();
    if qv.iter().any(|q| !(*q >= 0.0 && *q <= 1.0)) {
        return val_err(if scale == 100.0 { "Percentiles must be in the range [0, 100]".into() } else { "Quantiles must be in the range [0, 1]".into() });
    }
    let (src, ax) = match axis {
        None => (NdArr::new(x.vals(), vec![x.size()]), 0),
        Some(a) => (x.clone(), a),
    };
    let g = np_groups(&src, Some(ax));
    let n = g.n;
    let v = g.vals.clone().fv();
    let ng = if n == 0 { 0 } else { v.len() / n };
    let mut res: Vec<Vec<f64>> = vec![vec![]; qv.len()];
    for k in 0..ng {
        let mut grp = v[k * n..(k + 1) * n].to_vec();
        grp.sort_by(|a, b| if np_lt(Sc::F(*a), Sc::F(*b)) { std::cmp::Ordering::Less } else if np_lt(Sc::F(*b), Sc::F(*a)) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal });
        let has_nan = grp.last().is_some_and(|x| x.is_nan());
        for (qi, q) in qv.iter().enumerate() {
            if has_nan {
                res[qi].push(f64::NAN);
                continue;
            }
            let virt = q * (n as f64 - 1.0);
            let lo = virt.floor();
            let gamma = virt - lo;
            let li = (lo as usize).min(n - 1);
            let hi = (li + 1).min(n - 1);
            let (a, b) = (grp[li], grp[hi]);
            let d = b - a;
            let r = if gamma >= 0.5 { b - d * (1.0 - gamma) } else { a + d * gamma };
            res[qi].push(if a == b { a } else { r });
        }
    }
    let mut out_shape = g.out_shape.clone();
    if keepdims {
        out_shape = match axis {
            None => vec![1; x.ndim()],
            Some(a) => {
                let mut s = x.shape.clone();
                s[a] = 1;
                s
            }
        };
    }
    if qo.ndim() == 0 {
        return Ok(np_out(Buf::F(res.pop().unwrap()), out_shape));
    }
    let mut shape = qo.shape.clone();
    shape.extend(out_shape);
    Ok(np_out(Buf::F(res.concat()), shape))
}
fn np_median(x: &NdArr, axis: Option<usize>, keepdims: bool) -> R {
    // numpy: the mean of the middle one or two values
    let (src, ax) = match axis {
        None => (NdArr::new(x.vals(), vec![x.size()]), 0),
        Some(a) => (x.clone(), a),
    };
    let g = np_groups(&src, Some(ax));
    let n = g.n;
    if n == 0 {
        np_warn("Mean of empty slice.");
        np_warn("invalid value encountered in scalar divide");
    }
    let v = g.vals.clone().fv();
    let ng = if n == 0 { g.out_shape.iter().product::<usize>().max(1) } else { v.len() / n };
    let mut out = vec![];
    for k in 0..ng {
        if n == 0 {
            out.push(f64::NAN);
            continue;
        }
        let mut grp = v[k * n..(k + 1) * n].to_vec();
        grp.sort_by(|a, b| if np_lt(Sc::F(*a), Sc::F(*b)) { std::cmp::Ordering::Less } else if np_lt(Sc::F(*b), Sc::F(*a)) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal });
        if grp[n - 1].is_nan() {
            out.push(f64::NAN);
        } else if n % 2 == 1 {
            out.push(0.0 + grp[n / 2]);
        } else {
            out.push((0.0 + grp[n / 2 - 1] + grp[n / 2]) / 2.0);
        }
    }
    let mut shape = g.out_shape.clone();
    if keepdims {
        shape = match axis {
            None => vec![1; x.ndim()],
            Some(a) => {
                let mut s = x.shape.clone();
                s[a] = 1;
                s
            }
        };
    }
    Ok(np_out(Buf::F(out), shape))
}

fn np_nanred(x: &NdArr, what: &str, o: &RedOpts) -> R {
    if x.dt() != NT::F {
        return np_reduce(x, &what[3..], o);
    }
    let v = x.fv();
    match what {
        "nansum" | "nanmean" | "nanvar" | "nanstd" => {
            let z = NdArr::new(Buf::F(v.iter().map(|f| if f.is_nan() { 0.0 } else { *f }).collect()), x.shape.clone());
            let s = np_reduce(&z, "sum", &RedOpts { axis: o.axis, keepdims: true, ddof: 0.0 })?;
            if what == "nansum" {
                return np_reduce(&z, "sum", o);
            }
            let cnt = NdArr::new(Buf::F(v.iter().map(|f| if f.is_nan() { 0.0 } else { 1.0 }).collect()), x.shape.clone());
            let c = np_reduce(&cnt, "sum", &RedOpts { axis: o.axis, keepdims: true, ddof: 0.0 })?;
            let mean = np_binop(&s, &c, "/").unwrap()?;
            if what == "nanmean" {
                let r = np_as(&mean)?;
                let shape = red_shape(x, o, match o.axis { None => vec![], Some(a) => (0..x.ndim()).filter(|k| *k != a).map(|k| x.shape[k]).collect() });
                return Ok(np_out(r.vals(), shape));
            }
            let dev = np_binop(&np_arr(z.clone()), &mean, "-").unwrap()?;
            let dz = np_as(&dev)?;
            let sq: Vec<f64> = dz.fv().iter().zip(&v).map(|(d, f)| if f.is_nan() { 0.0 } else { d * d }).collect();
            let ss = np_reduce(&NdArr::new(Buf::F(sq), x.shape.clone()), "sum", o)?;
            let cr = np_reduce(&cnt, "sum", o)?;
            let dd = np_binop(&cr, &V::Float(o.ddof), "-").unwrap()?;
            let var = np_binop(&ss, &dd, "/").unwrap()?;
            if what == "nanstd" {
                return np_ufunc1("sqrt", &var);
            }
            Ok(var)
        }
        "nanmax" | "nanmin" => {
            let fill = if what == "nanmax" { f64::NEG_INFINITY } else { f64::INFINITY };
            let z = NdArr::new(Buf::F(v.iter().map(|f| if f.is_nan() { fill } else { *f }).collect()), x.shape.clone());
            np_reduce(&z, &what[3..], o)
        }
        _ => Err(np_unsup(what)),
    }
}

fn red_opts(a: &Args, x: &NdArr, axis_pos: usize) -> Result<RedOpts, E> {
    let axis = np_axis_arg(npa(a, axis_pos, "axis"), x.ndim())?;
    let keepdims = match a.k.iter().find(|(n, _)| *n == "keepdims") {
        Some((_, v)) => truth(v)?,
        None => false,
    };
    let ddof = match a.k.iter().find(|(n, _)| *n == "ddof") {
        Some((_, v)) => np_f_arg(v)?,
        None => 0.0,
    };
    if let Some((_, v)) = a.k.iter().find(|(n, _)| *n == "dtype") {
        if !matches!(v, V::None) {
            return Err(np_unsup("dtype= in a reduction"));
        }
    }
    Ok(RedOpts { axis, keepdims, ddof })
}

// Methods of arrays (x is the array); module functions taking an array first use them too.
pub fn np_method(x: &NdArr, me: &V, n: &str, a: &mut Args) -> R {
    let nd = x.ndim();
    match n {
        "sum" | "mean" | "std" | "var" | "prod" | "max" | "min" | "argmax" | "argmin" | "any" | "all" => {
            npa_kw_ok(a, n, &["axis", "keepdims", "ddof", "dtype", "out"])?;
            if a.k.iter().any(|(k, v)| *k == "out" && !matches!(v, V::None)) {
                return Err(np_unsup("out="));
            }
            let o = red_opts(a, x, 0)?;
            if matches!(n, "argmax" | "argmin") && o.keepdims {
                // fine
            }
            np_reduce(x, n, &o)
        }
        "cumsum" | "cumprod" => np_accum(x, n, np_axis_arg(npa(a, 0, "axis"), nd)?),
        "round" => {
            if x.dt() == NT::B {
                return Err(exc("UFuncTypeError", "ufunc 'rint' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''".into()));
            }
            let d = npa(a, 0, "decimals").map_or(Ok(0), |v| np_int_arg(&v))?;
            Ok(np_out(np_round_buf(x.vals(), d), x.shape.clone()))
        }
        "clip" => np_clip(me, npa(a, 0, "min").unwrap_or(V::None), npa(a, 1, "max").unwrap_or(V::None)),
        "reshape" => {
            let dims = np_dims_args(a, 0)?;
            Ok(np_arr(np_reshape(x, &dims)?))
        }
        "ravel" => Ok(np_arr(x.reshaped(vec![x.size()]))),
        "flatten" => Ok(np_new(x.vals(), vec![x.size()])),
        "copy" => Ok(np_arr(x.copy())),
        "transpose" => {
            let axes = if a.p.is_empty() {
                None
            } else {
                Some(np_dims_args(a, 0)?.iter().map(|k| norm_axis(*k, nd)).collect::<Result<Vec<_>, E>>()?)
            };
            Ok(np_arr(np_transpose(x, axes)?))
        }
        "astype" => {
            let d = np_dtype_arg(&npa(a, 0, "dtype").unwrap_or(V::None))?.unwrap_or(NT::F);
            Ok(np_new(x.vals().cast(d), x.shape.clone()))
        }
        "tolist" => Ok(np_tolist(&x.vals(), &x.shape, 0)),
        "item" => {
            if a.p.is_empty() {
                if x.size() != 1 {
                    return val_err("can only convert an array of size 1 to a Python scalar".into());
                }
                return Ok(np_py(x.first_c()));
            }
            let i = np_int_arg(&a.p[0])?;
            let n = x.size() as i64;
            let j = if i < 0 { i + n } else { i };
            if j < 0 || j >= n {
                return Err(exc("IndexError", format!("index {i} is out of bounds for size {n}")));
            }
            Ok(np_py(x.vals().get(j as usize)))
        }
        "fill" => {
            let v = a.p.first().cloned().unwrap_or(V::None);
            let s = np_opnd(&v)?.ok_or_else(|| np_unsup("fill with this value"))?.a.first();
            let mut b = x.buf.borrow_mut();
            for p in x.pos() {
                b.set(p, s);
            }
            Ok(V::None)
        }
        "sort" => {
            let ax = np_axis_arg(Some(npa(a, 0, "axis").unwrap_or(V::Int(-1))), nd)?;
            let s = np_sorted(x, ax, false)?;
            let sv = as_np(&s).unwrap().vals();
            let mut b = x.buf.borrow_mut();
            for (i, p) in x.pos().iter().enumerate() {
                b.set(*p, sv.get(i));
            }
            Ok(V::None)
        }
        "argsort" => np_sorted(x, np_axis_arg(Some(npa(a, 0, "axis").unwrap_or(V::Int(-1))), nd)?, true),
        "nonzero" => np_nonzero(x),
        "dot" => np_dot(me, &a.p[0]),
        "squeeze" => {
            let ax = np_axis_arg(npa(a, 0, "axis"), nd)?;
            let keep: Vec<usize> = (0..nd).filter(|k| match ax {
                Some(q) => *k != q,
                None => x.shape[*k] != 1,
            }).collect();
            if let Some(q) = ax {
                if x.shape[q] != 1 {
                    return val_err("cannot select an axis to squeeze out which has size not equal to one".into());
                }
            }
            Ok(np_arr(np_view(x, keep.iter().map(|k| x.shape[*k]).collect(), keep.iter().map(|k| x.st[*k]).collect(), x.off)))
        }
        "diagonal" => {
            if nd != 2 {
                return Err(np_unsup("diagonal of a non-2-D array"));
            }
            let k = npa(a, 0, "offset").map_or(Ok(0), |v| np_int_arg(&v))?;
            let (r, c) = (x.shape[0] as i64, x.shape[1] as i64);
            let (r0, c0) = if k >= 0 { (0, k) } else { (-k, 0) };
            let len = (r - r0).min(c - c0).max(0) as usize;
            let off = if len > 0 { (x.off as isize + r0 as isize * x.st[0] + c0 as isize * x.st[1]) as usize } else { x.off };
            Ok(np_arr(np_view(x, vec![len], vec![x.st[0] + x.st[1]], off)))
        }
        "trace" => {
            let d = np_method(x, me, "diagonal", &mut Args::p(vec![]))?;
            let dv = as_np(&d).unwrap();
            match dv.dt() {
                NT::F => Ok(np_sc(Sc::F(np_pw(&dv.fv())))),
                _ => Ok(np_sc(Sc::I(dv.iv().iter().fold(0i64, |s, v| s.wrapping_add(*v))))),
            }
        }
        "repeat" => np_repeat(x, &a.p.first().cloned().unwrap_or(V::Int(1)), np_axis_arg(npa(a, 1, "axis"), nd)?),
        "conjugate" | "conj" => Ok(np_arr(x.copy())),
        "__abs__" => np_unary(me, "abs"),
        "__round__" => {
            let d = a.p.first().map_or(Ok(0), |v| np_int_arg(v))?;
            Ok(np_out(np_round_buf(x.vals(), d), x.shape.clone()))
        }
        "__int__" | "__float__" | "__index__" => {
            if x.size() != 1 {
                return type_err("only length-1 arrays can be converted to Python scalars".into());
            }
            let s = x.first_c();
            Ok(match n {
                "__int__" => V::Int(s.i()),
                "__float__" => V::Float(s.f()),
                _ => {
                    if x.ndim() > 0 || s.dt() == NT::F {
                        return type_err("only integer scalar arrays can be converted to a scalar index".into());
                    }
                    V::Int(s.i())
                }
            })
        }
        "__contains__" => {
            let r = np_binop(me, &a.p[0], "==").unwrap()?;
            Ok(V::Bool(np_as(&r)?.bv().iter().any(|b| *b)))
        }
        "__getitem__" => np_getitem(x, &a.p[0]),
        "__setitem__" => {
            np_setitem(x, &a.p[0], &a.p[1])?;
            Ok(V::None)
        }
        "__iop__" => {
            let op = to_str(&a.p[1])?;
            np_iop(x, me, &a.p[0], &op)
        }
        _ => Err(exc("AttributeError", format!("'numpy.ndarray' object has no attribute '{n}'"))),
    }
}
impl NdArr {
    // the first element in C order
    pub fn first_c(&self) -> Sc {
        self.buf.borrow().get(self.off)
    }
}

fn np_repeat(x: &NdArr, r: &V, axis: Option<usize>) -> R {
    let (src, ax) = match axis {
        None => (NdArr::new(x.vals(), vec![x.size()]), 0),
        Some(a) => (x.clone(), a),
    };
    let n = src.shape[ax];
    let reps: Vec<usize> = match np_opnd(r)? {
        Some(o) if o.a.ndim() == 0 => vec![o.a.first().i().max(0) as usize; n],
        Some(o) => o.a.iv().iter().map(|v| (*v).max(0) as usize).collect(),
        None => return type_err("repeats must be integers".into()),
    };
    if reps.len() != n {
        return val_err(format!("operands could not be broadcast together with shape ({n},) ({},)", reps.len()));
    }
    let mut parts = vec![];
    for (i, k) in reps.iter().enumerate() {
        let one = np_getitem(&src, &tup((0..src.ndim()).map(|d| if d == ax { slice(V::Int(i as i64), V::Int(i as i64 + 1), V::None) } else { slice(V::None, V::None, V::None) }).collect()))?;
        for _ in 0..*k {
            parts.push(as_np(&one).unwrap().clone());
        }
    }
    if parts.is_empty() {
        let mut s = src.shape.clone();
        s[ax] = 0;
        return Ok(np_new(Buf::zeros(src.dt(), 0), s));
    }
    np_concat(&parts, Some(ax))
}

fn np_clip(x: &V, lo: V, hi: V) -> R {
    let mut r = x.clone();
    if !matches!(lo, V::None) {
        r = np_ufunc2("maximum", &r, &lo)?;
    }
    if !matches!(hi, V::None) {
        r = np_ufunc2("minimum", &r, &hi)?;
    }
    if matches!(lo, V::None) && matches!(hi, V::None) {
        return val_err("One of max or min must be given".into());
    }
    Ok(r)
}

fn np_dot(a: &V, b: &V) -> R {
    let (x, y) = match (np_opnd(a)?, np_opnd(b)?) {
        (Some(x), Some(y)) => (x, y),
        _ => return type_err("unsupported operand for dot".into()),
    };
    if x.a.ndim() == 0 || y.a.ndim() == 0 {
        return np_binop(&np_out(x.a.vals(), x.a.shape.clone()), &np_out(y.a.vals(), y.a.shape.clone()), "*").unwrap();
    }
    np_dot2(&x.a, &y.a, false)
}

pub fn np_module_attr(m: &str, n: &str) -> Option<V> {
    Some(match (m, n) {
        ("numpy", "pi") => V::Float(std::f64::consts::PI),
        ("numpy", "e") => V::Float(std::f64::consts::E),
        ("numpy", "inf") => V::Float(f64::INFINITY),
        ("numpy", "nan") => V::Float(f64::NAN),
        ("numpy", "euler_gamma") => V::Float(0.5772156649015329),
        ("numpy", "newaxis") => V::None,
        ("numpy", "random") => V::Module("numpy.random"),
        ("numpy", "linalg") => V::Module("numpy.linalg"),
        ("numpy.linalg", "LinAlgError") => V::Class(exc_class("LinAlgError")),
        ("numpy", "True_") => np_sc(Sc::B(true)),
        ("numpy", "False_") => np_sc(Sc::B(false)),
        ("numpy", "float64" | "double") => V::Class(ext_class("numpy.float64")),
        ("numpy", "int64" | "int_" | "intp") => V::Class(ext_class("numpy.int64")),
        ("numpy", "bool_" | "bool") => V::Class(ext_class("numpy.bool")),
        ("numpy", "ndarray") => V::Class(ext_class("numpy.ndarray")),
        ("numpy", "integer" | "signedinteger" | "floating" | "number" | "generic" | "inexact") => V::Class(ext_class(match n {
            "integer" => "numpy.integer",
            "signedinteger" => "numpy.signedinteger",
            "floating" => "numpy.floating",
            "number" => "numpy.number",
            "inexact" => "numpy.inexact",
            _ => "numpy.generic",
        })),
        _ => return None,
    })
}

pub fn np_construct(cls: &str, a: Args) -> Option<R> {
    let x = a.p.first().cloned();
    Some((|| -> R {
        match cls {
            "numpy.float64" => {
                let x = x.unwrap_or(V::Float(0.0));
                if let Some(arr) = as_np(&x) {
                    return Ok(np_out(arr.vals().cast(NT::F), arr.shape.clone()));
                }
                Ok(np_sc(Sc::F(match &x {
                    V::Str(_) => to_float(&x)?,
                    _ => np_f_arg(&x)?,
                })))
            }
            "numpy.int64" => {
                let x = x.unwrap_or(V::Int(0));
                if let Some(arr) = as_np(&x) {
                    return Ok(np_out(arr.vals().cast(NT::I), arr.shape.clone()));
                }
                let v = match as_npsc(&x) {
                    Some(s) => s.i(),
                    None => match &x {
                        V::Float(f) => Sc::F(*f).i(),
                        V::Big(_) => return Err(exc("OverflowError", "Python int too large to convert to C long".into())),
                        _ => match to_int(&x)? {
                            V::Int(i) => i,
                            _ => return Err(exc("OverflowError", "Python int too large to convert to C long".into())),
                        },
                    },
                };
                Ok(np_sc(Sc::I(v)))
            }
            "numpy.bool" => Ok(np_sc(Sc::B(match &x {
                None => false,
                Some(v) => truth(v)?,
            }))),
            "numpy.ndarray" => Err(np_unsup("np.ndarray(shape)")),
            _ => type_err(format!("cannot create '{cls}' instances")),
        }
    })())
}

pub fn np_module_call(m: &str, n: &str, mut a: Args) -> R {
    if m == "numpy.random" {
        return np_random_call(n, &mut a);
    }
    if m == "numpy.linalg" {
        return np_linalg_call(n, &mut a);
    }
    if m == "numpy" {
        match n {
            "histogram" => return np_histogram(&a),
            "cov" => return np_cov(&a),
            "corrcoef" => return np_corrcoef(&a),
            "trapezoid" => return np_trapezoid(&a),
            "polyfit" => return np_polyfit(&a),
            "polyval" => return np_polyval(&a),
            _ => {}
        }
        if let Some((uf, me)) = n.split_once('.') {
            return np_ufunc_method(uf, me, &a);
        }
    }
    if m != "numpy" {
        return Err(np_unsup(&format!("{m}.{n}")));
    }
    if let Some(V::Class(c)) = np_module_attr(m, n) {
        return np_construct(c.name, a).unwrap();
    }
    let first = |a: &Args| -> Result<NdArr, E> {
        match a.p.first() {
            Some(v) => np_as(v),
            None => type_err(format!("{n}() missing required argument 'a' (pos 0)")),
        }
    };
    match n {
        "array" | "asarray" | "asanyarray" | "copy" => {
            npa_kw_ok(&a, n, &["object", "a", "dtype", "copy", "ndmin"])?;
            let v = npa(&a, 0, if n == "copy" { "a" } else { "object" }).ok_or_else(|| exc("TypeError", format!("{n}() missing required argument 'object' (pos 0)")))?;
            let dt = match npa(&a, 1, "dtype") {
                Some(d) => np_dtype_arg(&d)?,
                None => None,
            };
            let mut r = if n == "asarray" || n == "asanyarray" {
                match (as_np(&v), dt) {
                    (Some(x), None) => x.clone(),
                    (Some(x), Some(d)) if x.dt() == d => x.clone(),
                    _ => np_from(&v, dt)?,
                }
            } else {
                np_from(&v, dt)?
            };
            if let Some(k) = npa(&a, 99, "ndmin") {
                let k = np_int_arg(&k)? as usize;
                while r.ndim() < k {
                    let mut s = r.shape.clone();
                    s.insert(0, 1);
                    r = r.reshaped(s);
                }
            }
            Ok(np_arr(r))
        }
        "zeros" | "ones" | "empty" | "full" => {
            npa_kw_ok(&a, n, &["shape", "fill_value", "dtype"])?;
            let shape = np_shape_arg(&npa(&a, 0, "shape").ok_or_else(|| exc("TypeError", format!("{n}() missing required argument 'shape' (pos 0)")))?)?;
            let (fill, di) = if n == "full" {
                let f = npa(&a, 1, "fill_value").ok_or_else(|| exc("TypeError", "full() missing required argument 'fill_value' (pos 1)".into()))?;
                (np_opnd(&f)?.ok_or_else(|| np_unsup("np.full with this value"))?.a.first(), 2)
            } else {
                (Sc::I((n == "ones") as i64), 1)
            };
            let d = match npa(&a, di, "dtype") {
                Some(v) => np_dtype_arg(&v)?,
                None => None,
            }
            .unwrap_or(if n == "full" { fill.dt() } else { NT::F });
            Ok(np_full(shape, fill, d))
        }
        "zeros_like" | "ones_like" | "empty_like" | "full_like" => {
            let x = first(&a)?;
            let (fill, di) = if n == "full_like" { (np_opnd(&npa(&a, 1, "fill_value").unwrap_or(V::Int(0)))?.unwrap().a.first(), 2) } else { (Sc::I((n == "ones_like") as i64), 1) };
            let d = match npa(&a, di, "dtype") {
                Some(v) => np_dtype_arg(&v)?,
                None => None,
            }
            .unwrap_or(x.dt());
            Ok(np_full(x.shape.clone(), fill, d))
        }
        "arange" => np_arange(&a),
        "linspace" => np_linspace(&a),
        "eye" => np_eye(&a),
        "identity" => np_eye(&Args { p: vec![a.p[0].clone()], k: a.k.iter().filter(|(k, _)| *k == "dtype").cloned().collect() }),
        "set_printoptions" => {
            npa_kw_ok(&a, n, &["precision", "suppress", "linewidth", "threshold", "edgeitems"])?;
            NPOPT.with(|o| -> Result<(), E> {
                let mut o = o.borrow_mut();
                for (k, v) in &a.k {
                    if matches!(v, V::None) {
                        continue;
                    }
                    match *k {
                        "precision" => o.precision = np_int_arg(v)? as usize,
                        "suppress" => o.suppress = truth(v)?,
                        "linewidth" => o.linewidth = np_int_arg(v)? as usize,
                        "threshold" => {
                            o.threshold = match as_npsc(v).map(|s| s.f()).or_else(|| num_f(v)) {
                                Some(f) if f.is_infinite() || f > 1e18 => usize::MAX,
                                Some(f) => f as usize,
                                None => np_int_arg(v)? as usize,
                            }
                        }
                        "edgeitems" => o.edgeitems = np_int_arg(v)? as usize,
                        _ => {}
                    }
                }
                if let Some(p) = a.p.first() {
                    o.precision = np_int_arg(p)? as usize;
                }
                Ok(())
            })?;
            Ok(V::None)
        }
        "shape" => Ok(shape_tuple(&first(&a)?.shape)),
        "ndim" => Ok(V::Int(first(&a)?.ndim() as i64)),
        "size" => Ok(V::Int(first(&a)?.size() as i64)),
        "dtype" => Ok(V::Ext(Rc::new(NpDtype(np_dtype_arg(&a.p[0])?.unwrap_or(NT::F))))),
        "isscalar" => Ok(V::Bool(as_npsc(&a.p[0]).is_some() || matches!(a.p[0], V::Int(_) | V::Float(_) | V::Bool(_) | V::Big(_) | V::Str(_)))),
        "issubdtype" => {
            let d = match as_npdt(&a.p[0]) {
                Some(d) => d,
                None => np_dtype_arg(&a.p[0])?.unwrap_or(NT::F),
            };
            let k = match &a.p[1] {
                V::Class(c) => c.name,
                _ => return Err(np_unsup("issubdtype with this type")),
            };
            Ok(V::Bool(match k {
                "numpy.number" | "numpy.generic" => d != NT::B || k == "numpy.generic",
                "numpy.integer" | "numpy.signedinteger" | "numpy.int64" | "int" => d == NT::I,
                "numpy.floating" | "numpy.inexact" | "numpy.float64" | "float" => d == NT::F,
                "numpy.bool" | "bool" => d == NT::B,
                _ => false,
            }))
        }
        "sum" | "mean" | "std" | "var" | "prod" | "max" | "min" | "amax" | "amin" | "argmax" | "argmin" | "any" | "all" | "cumsum" | "cumprod" | "round" | "around" | "clip" | "reshape" | "ravel" | "transpose" | "squeeze" | "trace" | "diagonal" | "nonzero" | "argsort" | "repeat" => {
            let v = a.p.remove(0);
            let x = np_as(&v)?;
            let me = np_arr(x.clone());
            let name = match n {
                "amax" => "max",
                "amin" => "min",
                "around" => "round",
                _ => n,
            };
            if name == "clip" {
                let lo = npa(&a, 0, "a_min").or_else(|| npa(&a, 99, "min")).unwrap_or(V::None);
                let hi = npa(&a, 1, "a_max").or_else(|| npa(&a, 99, "max")).unwrap_or(V::None);
                return np_clip(&me, lo, hi);
            }
            if name == "round" && x.ndim() == 0 && x.dt() != NT::B {
                let d = npa(&a, 0, "decimals").map_or(Ok(0), |v| np_int_arg(&v))?;
                return Ok(np_out(np_round_buf(x.vals(), d), vec![]));
            }
            if name == "reshape" {
                let s = npa(&a, 0, "shape").or_else(|| npa(&a, 0, "newshape")).unwrap();
                let dims: Vec<i64> = match &s {
                    V::Tuple(_) | V::List(_) => to_vec(&s)?.iter().map(np_int_arg).collect::<Result<_, _>>()?,
                    v => vec![np_int_arg(v)?],
                };
                return Ok(np_arr(np_reshape(&x, &dims)?));
            }
            if name == "transpose" {
                if let Some(ax) = npa(&a, 0, "axes").filter(|v| !matches!(v, V::None)) {
                    let p: Vec<usize> = to_vec(&ax)?.iter().map(|k| norm_axis(np_int_arg(k)?, x.ndim())).collect::<Result<_, _>>()?;
                    return Ok(np_arr(np_transpose(&x, Some(p))?));
                }
                return Ok(np_arr(np_transpose(&x, None)?));
            }
            if name == "argsort" && npa(&a, 0, "axis").is_none() {
                return np_method(&x, &me, name, &mut Args::p(vec![V::Int(-1)]));
            }
            if matches!(name, "sum" | "mean" | "std" | "var" | "prod" | "max" | "min" | "any" | "all") && x.ndim() == 0 && as_npsc(&v).is_none() && as_np(&v).is_none() {
                // a Python number: 0-d
            }
            np_method(&x, &me, name, &mut a)
        }
        "sort" => {
            let x = first(&a)?;
            let ax = match npa(&a, 1, "axis") {
                Some(V::None) => None,
                Some(v) => Some(norm_axis(np_int_arg(&v)?, x.ndim())?),
                None => if x.ndim() == 0 { None } else { Some(x.ndim() - 1) },
            };
            np_sorted(&x, ax, false)
        }
        "median" => {
            let x = first(&a)?;
            let ax = np_axis_arg(npa(&a, 1, "axis"), x.ndim())?;
            let kd = npa(&a, 99, "keepdims").map_or(Ok(false), |v| truth(&v))?;
            np_median(&x, ax, kd)
        }
        "percentile" | "quantile" => {
            npa_kw_ok(&a, n, &["a", "q", "axis", "keepdims", "method"])?;
            if let Some(m) = npa(&a, 99, "method") {
                if to_str(&m)? != "linear" {
                    return Err(np_unsup(&format!("{n} method={}", to_str(&m)?)));
                }
            }
            let x = first(&a)?;
            let q = npa(&a, 1, "q").unwrap();
            let ax = np_axis_arg(npa(&a, 2, "axis"), x.ndim())?;
            let kd = npa(&a, 99, "keepdims").map_or(Ok(false), |v| truth(&v))?;
            np_quantile(&x, &q, ax, if n == "percentile" { 100.0 } else { 1.0 }, kd)
        }
        "nansum" | "nanmean" | "nanstd" | "nanvar" | "nanmax" | "nanmin" => {
            let x = first(&a)?;
            a.p.remove(0);
            let o = red_opts(&a, &x, 0)?;
            np_nanred(&x, n, &o)
        }
        "average" => {
            npa_kw_ok(&a, n, &["a", "axis", "weights", "returned", "keepdims"])?;
            let x = first(&a)?;
            let ax = np_axis_arg(npa(&a, 1, "axis"), x.ndim())?;
            match npa(&a, 2, "weights").filter(|w| !matches!(w, V::None)) {
                None => np_reduce(&x, "mean", &RedOpts { axis: ax, keepdims: false, ddof: 0.0 }),
                Some(w) => {
                    let mut wa = np_as(&w)?;
                    if wa.shape != x.shape {
                        let ax = match ax {
                            Some(k) if wa.ndim() == 1 => k,
                            None => return type_err("Axis must be specified when shapes of a and weights differ.".into()),
                            _ => return type_err("1D weights expected when shapes of a and weights differ.".into()),
                        };
                        if wa.shape[0] != x.shape[ax] {
                            return val_err("Length of weights not compatible with specified axis.".into());
                        }
                        let mut s = vec![1; x.ndim()];
                        s[ax] = wa.shape[0];
                        wa = wa.reshaped(s);
                    }
                    let prod = np_binop(&np_arr(x.clone()), &np_arr(wa.clone()), "*").unwrap()?;
                    let num = np_reduce(&np_as(&prod)?, "sum", &RedOpts { axis: ax, keepdims: false, ddof: 0.0 })?;
                    let wf = NdArr::new(Buf::F(wa.fv()), wa.shape.clone());
                    let den = np_reduce(&wf, "sum", &RedOpts { axis: ax, keepdims: false, ddof: 0.0 })?;
                    if np_as(&den)?.fv().iter().any(|d| *d == 0.0) {
                        return Err(exc("ZeroDivisionError", "Weights sum to zero, can't be normalized".into()));
                    }
                    np_binop(&num, &den, "/").unwrap()
                }
            }
        }
        "count_nonzero" => {
            let x = first(&a)?;
            let ax = np_axis_arg(npa(&a, 1, "axis"), x.ndim())?;
            let r = np_reduce(&x, "count_nonzero", &RedOpts { axis: ax, keepdims: false, ddof: 0.0 })?;
            if ax.is_none() {
                return Ok(V::Int(as_npsc(&r).unwrap().i()));
            }
            Ok(r)
        }
        "where" => np_where(&a),
        "argwhere" => {
            let x = first(&a)?;
            let nz = np_nonzero(&if x.ndim() == 0 { x.reshaped(vec![1]) } else { x.clone() })?;
            let cols: Vec<NdArr> = to_vec(&nz)?.iter().map(|c| as_np(c).unwrap().clone()).collect();
            let k = cols.first().map_or(0, |c| c.size());
            let nd = cols.len();
            let mut v = vec![0i64; k * nd];
            for (j, c) in cols.iter().enumerate() {
                for (i, x) in c.iv().iter().enumerate() {
                    v[i * nd + j] = *x;
                }
            }
            Ok(np_new(Buf::I(v), vec![k, nd]))
        }
        "flatnonzero" => {
            let x = first(&a)?;
            let f = NdArr::new(x.vals(), vec![x.size()]);
            Ok(to_vec(&np_nonzero(&f)?)?.remove(0))
        }
        "unique" => np_unique(&a),
        "concatenate" => {
            let xs = np_seq_arrays(&a.p[0])?;
            let ax = match npa(&a, 1, "axis") {
                Some(V::None) => None,
                Some(v) => Some(norm_axis(np_int_arg(&v)?, xs.first().map_or(1, |x| x.ndim()))?),
                None => Some(0),
            };
            np_concat(&xs, ax)
        }
        "stack" => {
            let xs = np_seq_arrays(&a.p[0])?;
            if xs.is_empty() {
                return val_err("need at least one array to stack".into());
            }
            if xs.iter().any(|x| x.shape != xs[0].shape) {
                return val_err("all input arrays must have the same shape".into());
            }
            let ax = norm_axis(npa(&a, 1, "axis").map_or(Ok(0), |v| np_int_arg(&v))?, xs[0].ndim() + 1)?;
            let ys: Vec<NdArr> = xs.iter().map(|x| {
                let mut s = x.shape.clone();
                s.insert(ax, 1);
                x.reshaped(s)
            }).collect();
            np_concat(&ys, Some(ax))
        }
        "vstack" | "row_stack" => {
            let xs: Vec<NdArr> = np_seq_arrays(&a.p[0])?.iter().map(|x| atleast(x, 2)).collect();
            np_concat(&xs, Some(0))
        }
        "hstack" => {
            let xs: Vec<NdArr> = np_seq_arrays(&a.p[0])?.iter().map(|x| atleast(x, 1)).collect();
            let ax = if xs.iter().all(|x| x.ndim() == 1) { 0 } else { 1 };
            np_concat(&xs, Some(ax))
        }
        "column_stack" => {
            let xs: Vec<NdArr> = np_seq_arrays(&a.p[0])?.iter().map(|x| if x.ndim() < 2 { let n = x.size(); NdArr::new(x.vals(), vec![n, 1]) } else { x.clone() }).collect();
            np_concat(&xs, Some(1))
        }
        "atleast_1d" | "atleast_2d" => {
            let x = first(&a)?;
            Ok(np_arr(atleast(&x, if n == "atleast_1d" { 1 } else { 2 })))
        }
        "expand_dims" => {
            let x = first(&a)?;
            let ax = norm_axis(np_int_arg(&npa(&a, 1, "axis").unwrap())?, x.ndim() + 1)?;
            let mut s = x.shape.clone();
            s.insert(ax, 1);
            Ok(np_arr(x.reshaped(s)))
        }
        "flip" | "fliplr" | "flipud" => {
            let x = first(&a)?;
            let axes: Vec<usize> = match n {
                "fliplr" => vec![1],
                "flipud" => vec![0],
                _ => match npa(&a, 1, "axis") {
                    None | Some(V::None) => (0..x.ndim()).collect(),
                    Some(v) => vec![norm_axis(np_int_arg(&v)?, x.ndim())?],
                },
            };
            let mut v = x.clone();
            for k in axes {
                if k >= v.ndim() {
                    return val_err("Input must be >= 2-d.".into());
                }
                if v.shape[k] > 0 {
                    v.off = (v.off as isize + (v.shape[k] as isize - 1) * v.st[k]) as usize;
                }
                v.st[k] = -v.st[k];
            }
            Ok(np_arr(v))
        }
        "roll" => {
            let x = first(&a)?;
            let sh = np_int_arg(&npa(&a, 1, "shift").unwrap())?;
            let ax = np_axis_arg(npa(&a, 2, "axis"), x.ndim())?;
            let (src, k) = match ax {
                None => (NdArr::new(x.vals(), vec![x.size()]), 0),
                Some(k) => (x.clone(), k),
            };
            let len = src.shape[k] as i64;
            if len == 0 {
                return Ok(np_arr(x.copy()));
            }
            let s = sh.rem_euclid(len);
            let idx: Vec<i64> = (0..len).map(|i| (i - s).rem_euclid(len)).collect();
            let key = tup((0..src.ndim()).map(|d| if d == k { np_new(Buf::I(idx.clone()), vec![len as usize]) } else { slice(V::None, V::None, V::None) }).collect());
            let r = np_getitem(&src, &key)?;
            if ax.is_none() {
                return Ok(np_arr(as_np(&r).unwrap().reshaped(x.shape.clone())));
            }
            Ok(r)
        }
        "diff" => {
            npa_kw_ok(&a, n, &["a", "n", "axis", "prepend", "append"])?;
            let mut x = first(&a)?;
            let times = npa(&a, 1, "n").map_or(Ok(1), |v| np_int_arg(&v))?;
            let ax = norm_axis(npa(&a, 2, "axis").map_or(Ok(-1), |v| np_int_arg(&v))?, x.ndim().max(1))?;
            for (key, front) in [("prepend", true), ("append", false)] {
                if let Some(v) = a.k.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()) {
                    let mut p = np_as(&v)?;
                    if p.ndim() == 0 {
                        let mut s = x.shape.clone();
                        s[ax] = 1;
                        p = NdArr::new(np_bvals(&p, &s), s);
                    }
                    x = np_as(&np_concat(&if front { vec![p, x] } else { vec![x, p] }, Some(ax))?)?;
                }
            }
            let mut cur = np_arr(x);
            for _ in 0..times {
                let c = as_np(&cur).unwrap().clone();
                let nd = c.ndim();
                let sl = |lo: Option<i64>, hi: Option<i64>| tup((0..nd).map(|d| if d == ax { slice(lo.map_or(V::None, V::Int), hi.map_or(V::None, V::Int), V::None) } else { slice(V::None, V::None, V::None) }).collect());
                let (hi, lo) = (np_getitem(&c, &sl(Some(1), None))?, np_getitem(&c, &sl(None, Some(-1)))?);
                cur = if c.dt() == NT::B { np_binop(&hi, &lo, "!=").unwrap()? } else { np_binop(&hi, &lo, "-").unwrap()? };
                if as_np(&cur).is_none() {
                    break;
                }
            }
            Ok(cur)
        }
        "outer" => {
            let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
            let xc = NdArr::new(x.vals(), vec![x.size(), 1]);
            let yr = NdArr::new(y.vals(), vec![y.size()]);
            np_binop(&np_arr(xc), &np_arr(yr), "*").unwrap()
        }
        "dot" | "matmul" | "inner" | "vdot" => {
            if n == "matmul" {
                return np_matmul(&a.p[0], &a.p[1]);
            }
            if n == "vdot" {
                let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
                return np_dot(&np_arr(NdArr::new(x.vals(), vec![x.size()])), &np_arr(NdArr::new(y.vals(), vec![y.size()])));
            }
            if n == "inner" {
                let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
                if x.ndim() <= 1 && y.ndim() <= 1 {
                    return np_dot(&a.p[0], &a.p[1]);
                }
                if y.ndim() == 2 {
                    return np_dot(&a.p[0], &np_arr(np_transpose(&y, None)?));
                }
                return Err(np_unsup("np.inner of these shapes"));
            }
            np_dot(&a.p[0], &a.p[1])
        }
        "isclose" | "allclose" => {
            npa_kw_ok(&a, n, &["a", "b", "rtol", "atol", "equal_nan"])?;
            let rtol = npa(&a, 2, "rtol").map_or(Ok(1e-5), |v| np_f_arg(&v))?;
            let atol = npa(&a, 3, "atol").map_or(Ok(1e-8), |v| np_f_arg(&v))?;
            let en = npa(&a, 4, "equal_nan").map_or(Ok(false), |v| truth(&v))?;
            let r = np_isclose(&a.p[0], &a.p[1], rtol, atol, en)?;
            if n == "allclose" {
                let b = match as_np(&r) {
                    Some(x) => x.bv().iter().all(|b| *b),
                    None => as_npsc(&r).unwrap().b(),
                };
                return Ok(V::Bool(b));
            }
            Ok(r)
        }
        "array_equal" => {
            let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
            if x.shape != y.shape {
                return Ok(V::Bool(false));
            }
            let r = np_binop(&np_arr(x), &np_arr(y), "==").unwrap()?;
            Ok(V::Bool(match as_np(&r) {
                Some(x) => x.bv().iter().all(|b| *b),
                None => as_npsc(&r).unwrap().b(),
            }))
        }
        "searchsorted" => {
            let x = first(&a)?;
            let side = npa(&a, 2, "side").map_or(Ok("left".to_string()), |v| to_str(&v))?;
            let v = np_opnd(&npa(&a, 1, "v").unwrap())?.unwrap();
            let xs = x.vals();
            let right = side == "right";
            let find = |q: Sc| -> i64 {
                let (mut lo, mut hi) = (0usize, xs.len());
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    let m = xs.get(mid);
                    let go = if right { !np_lt(q, m) } else { np_lt(m, q) };
                    if go {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                lo as i64
            };
            let qv = v.a.vals();
            let r: Vec<i64> = (0..qv.len()).map(|i| find(qv.get(i))).collect();
            Ok(np_out(Buf::I(r), v.a.shape.clone()))
        }
        "tile" => {
            let x = first(&a)?;
            let reps = np_shape_arg(&a.p[1])?;
            let mut cur = x.clone();
            let nd = reps.len().max(cur.ndim());
            while cur.ndim() < nd {
                let mut s = cur.shape.clone();
                s.insert(0, 1);
                cur = cur.reshaped(s);
            }
            let mut reps = reps;
            while reps.len() < nd {
                reps.insert(0, 1);
            }
            for (k, r) in reps.iter().enumerate() {
                let parts = vec![cur.clone(); *r];
                if parts.is_empty() {
                    let mut s = cur.shape.clone();
                    s[k] = 0;
                    cur = NdArr::new(Buf::zeros(cur.dt(), 0), s);
                    continue;
                }
                cur = np_as(&np_concat(&parts, Some(k))?)?;
            }
            Ok(np_arr(cur))
        }
        "meshgrid" => {
            npa_kw_ok(&a, n, &["indexing"])?;
            let ij = npa(&a, 99, "indexing").map_or(Ok("xy".to_string()), |v| to_str(&v))? == "ij";
            let xs: Vec<NdArr> = a.p.iter().map(|v| np_as(v).map(|x| NdArr::new(x.vals(), vec![x.size()]))).collect::<Result<_, _>>()?;
            let mut shape: Vec<usize> = xs.iter().map(|x| x.size()).collect();
            if !ij && shape.len() >= 2 {
                shape.swap(0, 1);
            }
            let k = xs.len();
            let mut outs = vec![];
            for (i, x) in xs.iter().enumerate() {
                let mut s = vec![1; k];
                let pos = if !ij && k >= 2 && i < 2 { 1 - i } else { i };
                s[pos] = x.size();
                let r = x.reshaped(s);
                outs.push(np_new(np_bvals(&r, &shape), shape.clone()));
            }
            Ok(list(outs))
        }
        "diag" => {
            let x = first(&a)?;
            let k = npa(&a, 1, "k").map_or(Ok(0), |v| np_int_arg(&v))?;
            if x.ndim() == 1 {
                let m = x.size() + k.unsigned_abs() as usize;
                let mut b = Buf::zeros(x.dt(), m * m);
                let v = x.vals();
                for i in 0..x.size() {
                    let (r, c) = if k >= 0 { (i, i + k as usize) } else { (i + (-k) as usize, i) };
                    b.set(r * m + c, v.get(i));
                }
                return Ok(np_new(b, vec![m, m]));
            }
            let me = np_arr(x.clone());
            let d = np_method(&x, &me, "diagonal", &mut Args::p(vec![V::Int(k)]))?;
            Ok(np_arr(np_as(&d)?.copy()))
        }
        "fill_diagonal" => {
            let x = first(&a)?;
            let s = np_opnd(&a.p[1])?.unwrap().a;
            let sv = s.vals();
            let m = x.shape.iter().cloned().min().unwrap_or(0);
            let step: isize = x.st.iter().sum();
            let mut b = x.buf.borrow_mut();
            for i in 0..m {
                b.set((x.off as isize + i as isize * step) as usize, sv.get(i % sv.len().max(1)));
            }
            Ok(V::None)
        }
        "unravel_index" => {
            let shape = np_shape_arg(&a.p[1])?;
            let idx = np_opnd(&a.p[0])?.unwrap().a;
            let size: usize = shape.iter().product();
            let cs = cstrides(&shape);
            let iv = idx.iv();
            let mut cols: Vec<Vec<i64>> = vec![vec![]; shape.len()];
            for v in &iv {
                if *v < 0 || *v as usize >= size {
                    return val_err(format!("index {v} is out of bounds for array with size {size}"));
                }
                let mut r = *v as usize;
                for k in 0..shape.len() {
                    cols[k].push((r / cs[k] as usize) as i64);
                    r %= cs[k] as usize;
                }
            }
            Ok(tup(cols.into_iter().map(|c| np_out(Buf::I(c), idx.shape.clone())).collect()))
        }
        "cross" => {
            let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
            if x.shape != [3] || y.shape != [3] {
                return Err(np_unsup("np.cross of these shapes"));
            }
            let d = x.dt().max(y.dt());
            if d == NT::F {
                let (u, v) = (x.fv(), y.fv());
                return Ok(np_new(Buf::F(vec![u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]), vec![3]));
            }
            let (u, v) = (x.iv(), y.iv());
            Ok(np_new(Buf::I(vec![u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]), vec![3]))
        }
        "convolve" => {
            let (x, y) = (np_as(&a.p[0])?, np_as(&a.p[1])?);
            let mode = npa(&a, 2, "mode").map_or(Ok("full".to_string()), |v| to_str(&v))?;
            let (u, v) = (x.vals(), y.vals());
            let (n, m) = (u.len(), v.len());
            if n == 0 || m == 0 {
                return val_err(if n == 0 { "a cannot be empty".into() } else { "v cannot be empty".into() });
            }
            let d = x.dt().max(y.dt());
            // numpy: correlate(a, v[::-1]) with the longer one first
            let (p, q) = if m > n { (v.clone(), u.clone()) } else { (u.clone(), v.clone()) };
            let (pn, qn) = (p.len(), q.len());
            let full = pn + qn - 1;
            let mut out: Vec<Sc> = vec![];
            for k in 0..full {
                let lo = k.saturating_sub(qn - 1);
                let hi = k.min(pn - 1);
                match d {
                    NT::F => {
                        let mut s = 0.0;
                        for i in lo..=hi {
                            s += p.get(i).f() * q.get(k - i).f();
                        }
                        out.push(Sc::F(s));
                    }
                    _ => {
                        let mut s = 0i64;
                        for i in lo..=hi {
                            s = s.wrapping_add(p.get(i).i().wrapping_mul(q.get(k - i).i()));
                        }
                        out.push(Sc::I(s));
                    }
                }
            }
            let (start, len) = match mode.as_str() {
                "full" => (0, full),
                "same" => ((qn - 1) / 2, pn),
                "valid" => (qn - 1, pn - qn + 1),
                _ => return val_err(format!("mode must be one of 'valid', 'same', or 'full' (got {})", str_repr(&mode))),
            };
            let d = if d == NT::B { NT::I } else { d };
            Ok(np_new(Buf::from_sc(d, &out[start..start + len]), vec![len]))
        }
        "abs" | "absolute" | "fabs" | "negative" | "positive" | "sqrt" | "exp" | "exp2" | "expm1" | "log" | "log10" | "log2" | "log1p" | "sin" | "cos" | "tan" | "arcsin" | "arccos" | "arctan" | "sinh" | "cosh" | "tanh" | "arcsinh" | "arccosh" | "arctanh" | "cbrt" | "floor" | "ceil" | "trunc" | "rint" | "fix" | "square" | "reciprocal" | "sign" | "degrees" | "radians" | "deg2rad" | "rad2deg" | "isnan" | "isinf" | "isfinite" | "logical_not" => {
            npa_kw_ok(&a, n, &[])?;
            let x = a.p.first().cloned().ok_or_else(|| exc("TypeError", format!("{n}() takes from 1 to 2 positional arguments but 0 were given")))?;
            let x = if is_np(&x) { x } else { np_arr0(&x)? };
            if n == "fabs" {
                let o = np_as(&x)?;
                return Ok(np_out(Buf::F(o.fv().iter().map(|v| v.abs()).collect()), o.shape.clone()));
            }
            np_ufunc1(if n == "abs" { "absolute" } else { n }, &x)
        }
        "add" | "subtract" | "multiply" | "divide" | "true_divide" | "floor_divide" | "mod" | "remainder" | "power" | "maximum" | "minimum" | "fmax" | "fmin" | "arctan2" | "hypot" | "copysign" | "fmod" | "logaddexp" | "equal" | "not_equal" | "less" | "less_equal" | "greater" | "greater_equal" | "logical_and" | "logical_or" | "logical_xor" | "bitwise_and" | "bitwise_or" | "bitwise_xor" | "left_shift" | "right_shift" => {
            npa_kw_ok(&a, n, &[])?;
            if a.p.len() != 2 {
                return type_err(format!("{n}() takes from 2 to 3 positional arguments but {} were given", a.p.len()));
            }
            np_ufunc2(n, &a.p[0], &a.p[1])
        }
        _ => Err(np_unsup(&format!("np.{n}"))),
    }
}

// ---------- the values ----------

impl Ext for NpArr {
    fn tname(&self) -> &'static str {
        "numpy.ndarray"
    }
    fn repr(&self) -> String {
        np_array_repr(&self.0.vals(), &self.0.shape)
    }
    fn str_(&self) -> String {
        np_array_str(&self.0.vals(), &self.0.shape)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn binop(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        match op {
            "neg" | "pos" | "invert" => return Some(np_unary(me, op)),
            _ => {}
        }
        if right {
            np_binop(o, me, op)
        } else {
            np_binop(me, o, op)
        }
    }
    fn rich(&self, me: &V, op: &str, o: &V, right: bool) -> Option<Result<bool, E>> {
        let r = if right { np_binop(o, me, op)? } else { np_binop(me, o, op)? };
        Some(r.and_then(|v| truth(&v)))
    }
    fn try_truth(&self) -> Option<Result<bool, E>> {
        Some(np_truth(&self.0))
    }
    fn to_f64(&self) -> Option<f64> {
        if self.0.size() == 1 {
            return Some(self.0.first_c().f());
        }
        None
    }
    fn len(&self) -> Option<usize> {
        self.0.shape.first().cloned()
    }
    fn iter(&self) -> Option<Vec<V>> {
        let a = &self.0;
        if a.ndim() == 0 {
            return None;
        }
        Some((0..a.shape[0]).map(|i| np_getitem(a, &V::Int(i as i64)).unwrap_or(V::None)).collect())
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.str_())));
        }
        if self.0.ndim() == 0 {
            return Some(format_spec(&np_py(self.0.first_c()), spec).map(st));
        }
        Some(type_err("unsupported format string passed to numpy.ndarray.__format__".into()))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let a = &self.0;
        Some(Ok(match n {
            "shape" => shape_tuple(&a.shape),
            "ndim" => V::Int(a.ndim() as i64),
            "size" => V::Int(a.size() as i64),
            "dtype" => V::Ext(Rc::new(NpDtype(a.dt()))),
            "T" | "mT" => match np_transpose(a, None) {
                Ok(t) => np_arr(t),
                Err(e) => return Some(Err(e)),
            },
            "itemsize" => V::Int(a.dt().size()),
            "nbytes" => V::Int(a.dt().size() * a.size() as i64),
            "real" => np_arr(a.clone()),
            "strides" => tup(a.st.iter().map(|s| V::Int(*s as i64 * a.dt().size())).collect()),
            _ => return None,
        }))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(np_method(&self.0, me, n, a))
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(np_unsup(&format!("setting ndarray.{n}"))))
    }
    fn isinst(&self, cls: &Class) -> Option<bool> {
        Some(cls.name == "numpy.ndarray")
    }
}

impl Ext for NpScal {
    fn tname(&self) -> &'static str {
        match self.0 {
            Sc::F(_) => "numpy.float64",
            Sc::I(_) => "numpy.int64",
            Sc::B(_) => "numpy.bool",
        }
    }
    fn repr(&self) -> String {
        np_sc_repr(self.0)
    }
    fn str_(&self) -> String {
        np_sc_str(self.0)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(match self.0 {
            Sc::F(f) => hash_float(f),
            Sc::I(i) => hash_int(i),
            Sc::B(b) => b as i64,
        })
    }
    fn eq(&self, o: &V) -> Option<bool> {
        let r = np_binop(&np_sc(self.0), o, "==")?.ok()?;
        match as_npsc(&r) {
            Some(s) => Some(s.b()),
            None => None,
        }
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        let x = self.0.f();
        let y = match as_npsc(o) {
            Some(s) => s.f(),
            None => num_f(o)?,
        };
        x.partial_cmp(&y)
    }
    fn rich(&self, me: &V, op: &str, o: &V, right: bool) -> Option<Result<bool, E>> {
        let r = if right { np_binop(o, me, op)? } else { np_binop(me, o, op)? };
        Some(r.and_then(|v| truth(&v)))
    }
    fn binop(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        // "#" * np.int64(3): the sequence repeats, as Python's own does through __index__
        if op == "*" && matches!(o, V::Str(_) | V::List(_) | V::Tuple(_)) && self.0.dt() != NT::F {
            return Some(binop(o, &V::Int(self.0.i()), "*"));
        }
        match op {
            "neg" | "pos" | "invert" => return Some(np_unary(me, op)),
            _ => {}
        }
        if right {
            np_binop(o, me, op)
        } else {
            np_binop(me, o, op)
        }
    }
    fn truth(&self) -> bool {
        self.0.b()
    }
    fn to_f64(&self) -> Option<f64> {
        Some(self.0.f())
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if spec.is_empty() {
            return Some(Ok(st(self.str_())));
        }
        Some(format_spec(&np_py(self.0), spec).map(st))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "dtype" => V::Ext(Rc::new(NpDtype(self.0.dt()))),
            "shape" => tup(vec![]),
            "ndim" => V::Int(0),
            "size" => V::Int(1),
            "itemsize" | "nbytes" => V::Int(self.0.dt().size()),
            "real" => np_sc(self.0),
            "imag" => np_sc(Sc::F(0.0).cast(self.0.dt())),
            _ => return None,
        }))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let x = self.0;
        Some(match n {
            "item" | "tolist" => Ok(np_py(x)),
            "__int__" => Ok(V::Int(x.i())),
            "__float__" => Ok(V::Float(x.f())),
            "__index__" => match x {
                Sc::F(_) => type_err("'numpy.float64' object cannot be interpreted as an integer".into()),
                _ => Ok(V::Int(x.i())),
            },
            "__abs__" => np_unary(me, "abs"),
            "__round__" if matches!(x, Sc::B(_)) && a.p.first().is_some_and(|v| !matches!(v, V::None)) => Err(exc("TypeError", "ufunc 'rint' not supported for the input types, and the inputs could not be safely coerced to any supported types according to the casting rule ''safe''".into())),
            "__round__" => match a.p.first() {
                None | Some(V::None) => match x {
                    Sc::F(f) => to_int(&V::Float(rint(f))),
                    _ => Ok(V::Int(x.i())),
                },
                Some(d) => {
                    let d = match np_int_arg(d) {
                        Ok(d) => d,
                        Err(e) => return Some(Err(e)),
                    };
                    Ok(np_sc(np_round_buf(Buf::from_sc(x.dt(), &[x]), d).get(0)))
                }
            },
            "is_integer" => match x {
                Sc::F(f) => Ok(V::Bool(f.is_finite() && f.fract() == 0.0)),
                _ => Ok(V::Bool(true)),
            },
            "as_integer_ratio" => call_method(&np_py(x), "as_integer_ratio", Args::p(vec![])),
            "conjugate" => Ok(np_sc(x)),
            _ => {
                // array methods (round, astype, sum, ...) work on numpy scalars too
                let arr = NdArr::new(Buf::from_sc(x.dt(), &[x]), vec![]);
                let me2 = np_arr(arr.clone());
                np_method(&arr, &me2, n, a)
            }
        })
    }
    fn isinst(&self, cls: &Class) -> Option<bool> {
        Some(match (self.0, cls.name) {
            (_, "numpy.generic") => true,
            (Sc::F(_), "float" | "numpy.float64" | "numpy.floating" | "numpy.inexact" | "numpy.number") => true,
            (Sc::I(_), "numpy.int64" | "numpy.integer" | "numpy.signedinteger" | "numpy.number") => true,
            (Sc::B(_), "numpy.bool") => true,
            (_, n) if n == self.tname() => true,
            (_, "object") => true,
            _ => false,
        })
    }
}

impl Ext for NpDtype {
    fn tname(&self) -> &'static str {
        "numpy.dtype"
    }
    fn repr(&self) -> String {
        format!("dtype('{}')", self.0.name())
    }
    fn str_(&self) -> String {
        self.0.name().into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(self.0 as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if let Some(d) = as_npdt(o) {
            return Some(d == self.0);
        }
        Some(np_dtype_arg(o).ok().flatten() == Some(self.0))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "name" => s(self.0.name()),
            "kind" => s(match self.0 {
                NT::B => "b",
                NT::I => "i",
                NT::F => "f",
            }),
            "char" => s(match self.0 {
                NT::B => "?",
                NT::I => "l",
                NT::F => "d",
            }),
            "itemsize" => V::Int(self.0.size()),
            "type" => V::Class(ext_class(match self.0 {
                NT::B => "numpy.bool",
                NT::I => "numpy.int64",
                NT::F => "numpy.float64",
            })),
            _ => return None,
        }))
    }
}
