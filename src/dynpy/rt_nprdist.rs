// ---------- numpy random distributions (Generator and legacy RandomState), bit-exact ----------
// Ports of numpy/random/src/distributions/distributions.c, legacy-distributions.c, random_hypergeometric.c and
// logfactorial.c (numpy 2.x), with the argument checks of _common.pyx, _generator.pyx and mtrand.pyx.
// Bg's dbl/normal/exponential already switch between the Generator form (ziggurat) and the legacy one
// (polar method, -log(1-U)), so the shared algorithms are written once; where the legacy code differs it is
// a separate branch.

#[derive(Clone, Copy, PartialEq)]
enum Cons {
    No,
    NonNeg,
    Pos,
    PosNN,
    B01,
    BGt01,
    BLt01,
    Gt1,
    Gte1,
    Poi,
    LLong,
}
fn np_lam_max() -> f64 {
    let m = i64::MAX as f64;
    m - m.sqrt() * 10.0
}
fn np_cons1(v: f64, n: &str, c: Cons) -> Result<(), E> {
    let bad = |m: String| val_err::<()>(m);
    match c {
        Cons::No => Ok(()),
        Cons::NonNeg if !v.is_nan() && v.is_sign_negative() => bad(format!("{n} < 0")),
        Cons::PosNN if v.is_nan() => bad(format!("{n} must not be NaN")),
        Cons::Pos | Cons::PosNN if v <= 0.0 => bad(format!("{n} <= 0")),
        Cons::B01 if !(v >= 0.0) || !(v <= 1.0) => bad(format!("{n} < 0, {n} > 1 or {n} is NaN")),
        Cons::BGt01 if !(v > 0.0) || !(v <= 1.0) => bad(format!("{n} <= 0, {n} > 1 or {n} contains NaNs")),
        Cons::BLt01 if !(v >= 0.0) || !(v < 1.0) => bad(format!("{n} < 0, {n} >= 1 or {n} is NaN")),
        Cons::Gt1 if !(v > 1.0) => bad(format!("{n} <= 1 or {n} is NaN")),
        Cons::Gte1 if !(v >= 1.0) => bad(format!("{n} < 1 or {n} is NaN")),
        Cons::Poi if !(v >= 0.0) => bad(format!("{n} < 0 or {n} is NaN")),
        Cons::Poi if !(v <= np_lam_max()) => bad(format!("{n} value too large")),
        Cons::LLong if v < 0.0 => bad(format!("{n} < 0")),
        Cons::LLong if v > i64::MAX as f64 => bad(format!("{n} is out of bounds for long, consider using the new generator API for 64bit integers.")),
        _ => Ok(()),
    }
}
fn np_cons_arr(xs: &[f64], n: &str, c: Cons) -> Result<(), E> {
    let any = |f: &dyn Fn(f64) -> bool| xs.iter().any(|v| f(*v));
    let m = match c {
        Cons::No => None,
        Cons::NonNeg => any(&|v| !v.is_nan() && v.is_sign_negative()).then(|| format!("{n} < 0")),
        Cons::Pos | Cons::PosNN => {
            if c == Cons::PosNN && any(&|v| v.is_nan()) {
                Some(format!("{n} must not be NaN"))
            } else {
                any(&|v| v <= 0.0).then(|| format!("{n} <= 0"))
            }
        }
        Cons::B01 => any(&|v| !(v >= 0.0) || !(v <= 1.0)).then(|| format!("{n} < 0, {n} > 1 or {n} contains NaNs")),
        Cons::BGt01 => any(&|v| !(v > 0.0) || !(v <= 1.0)).then(|| format!("{n} <= 0, {n} > 1 or {n} contains NaNs")),
        Cons::BLt01 => any(&|v| !(v >= 0.0) || !(v < 1.0)).then(|| format!("{n} < 0, {n} >= 1 or {n} contains NaNs")),
        Cons::Gt1 => any(&|v| !(v > 1.0)).then(|| format!("{n} <= 1 or {n} contains NaNs")),
        Cons::Gte1 => any(&|v| !(v >= 1.0)).then(|| format!("{n} < 1 or {n} contains NaNs")),
        Cons::Poi => {
            let mx = np_lam_max();
            if any(&|v| !(v <= mx)) {
                Some(format!("{n} value too large"))
            } else {
                any(&|v| !(v >= 0.0)).then(|| format!("{n} < 0 or {n} contains NaNs"))
            }
        }
        Cons::LLong => {
            if any(&|v| !(v >= 0.0)) {
                Some(format!("{n} < 0"))
            } else {
                any(&|v| !(v <= i64::MAX as f64)).then(|| format!("{n} is out of bounds for long, consider using the new generator API for 64bit integers."))
            }
        }
    };
    match m {
        Some(m) => val_err(m),
        None => Ok(()),
    }
}

// One distribution parameter: a scalar, or an array (numpy's array path: broadcasting, "contains NaNs" messages).
enum DArg {
    S(f64),
    A(NdArr),
}
fn np_darg(v: &V, int: bool) -> Result<DArg, E> {
    if as_np(v).is_some() || seq_items(v).is_some() {
        let a = np_as(v)?;
        if int && a.dt() == NT::F {
            return type_err("Cannot cast array data from dtype('float64') to dtype('int64') according to the rule 'safe'".into());
        }
        if !matches!(a.dt(), NT::F | NT::I | NT::B) {
            return Err(np_unsup("a distribution parameter of this dtype"));
        }
        if a.ndim() == 0 {
            return Ok(DArg::S(a.vals().fv()[0]));
        }
        if int && a.vals().fv().iter().any(|x| x.abs() > 9007199254740992.0) {
            return Err(np_unsup("integer distribution parameters beyond 2**53"));
        }
        return Ok(DArg::A(a));
    }
    if int {
        return Ok(DArg::S(np_trunc_i64(v)? as f64));
    }
    Ok(DArg::S(np_f_arg(v)?))
}
// Cython's <int64_t>x for a Python number: floats truncate toward zero
fn np_trunc_i64(v: &V) -> Result<i64, E> {
    let f = match v {
        V::Float(f) => Some(*f),
        _ => match as_npsc(v) {
            Some(Sc::F(f)) => Some(f),
            _ => None,
        },
    };
    let n = match f {
        Some(f) if f.is_nan() => return val_err("cannot convert float NaN to integer".into()),
        Some(f) if f.is_infinite() => return Err(exc("OverflowError", "cannot convert float infinity to integer".into())),
        Some(f) if f.abs() >= 9223372036854775808.0 => return Err(exc("OverflowError", "Python int too large to convert to C long".into())),
        Some(f) => f.trunc() as i64,
        None => {
            if let V::Big(_) = v {
                return Err(exc("OverflowError", "Python int too large to convert to C long".into()));
            }
            np_int_arg(v)?
        }
    };
    if n.unsigned_abs() > 1 << 53 {
        return Err(np_unsup("integer distribution parameters beyond 2**53"));
    }
    Ok(n)
}

// The parameters of a call, checked as numpy checks them: (values per parameter, result shape or None for one value).
// A parameter's value list has one entry (a scalar) or one per result element.
struct DPar {
    vals: Vec<Vec<f64>>,
    shape: Option<Vec<usize>>,
    scalar: bool,
}
impl DPar {
    fn n(&self) -> usize {
        self.shape.as_ref().map_or(1, |s| s.iter().product())
    }
    fn at(&self, i: usize, xs: &mut [f64]) {
        for (k, v) in self.vals.iter().enumerate() {
            xs[k] = if v.len() == 1 { v[0] } else { v[i] };
        }
    }
}
type PSpec<'a> = (&'a str, Option<f64>, Cons);
fn np_dmissing(f: &str, need: usize, a: &Args) -> E {
    let given = a.p.len() + a.k.len();
    exc("TypeError", format!("{f}() takes at least {need} positional argument{} ({given} given)", if need == 1 { "" } else { "s" }))
}
fn np_dpar(a: &Args, f: &str, specs: &[PSpec], int: bool) -> Result<DPar, E> {
    let need = specs.iter().filter(|s| s.1.is_none()).count();
    let mut args = vec![];
    for (i, (name, def, _)) in specs.iter().enumerate() {
        match (npa(a, i, name), def) {
            (Some(v), _) => args.push(np_darg(&v, int)?),
            (None, Some(d)) => args.push(DArg::S(*d)),
            (None, None) => return Err(np_dmissing(f, need, a)),
        }
    }
    let size = np_size(a, specs.len())?;
    np_dpar_from(args, specs, size)
}
fn np_dpar_from(args: Vec<DArg>, specs: &[PSpec], size: Option<Vec<usize>>) -> Result<DPar, E> {
    let scalar = args.iter().all(|x| matches!(x, DArg::S(_)));
    if scalar {
        let mut vals = vec![];
        for (x, (name, _, c)) in args.iter().zip(specs) {
            if let DArg::S(v) = x {
                np_cons1(*v, name, *c)?;
                vals.push(vec![*v]);
            }
        }
        return Ok(DPar { vals, shape: size, scalar: true });
    }
    for (x, (name, _, c)) in args.iter().zip(specs) {
        match x {
            DArg::S(v) => np_cons_arr(&[*v], name, *c)?,
            DArg::A(a) => np_cons_arr(&a.vals().fv(), name, *c)?,
        }
    }
    let mut shape: Vec<usize> = vec![];
    let mut shapes: Vec<Vec<usize>> = vec![];
    if let Some(s) = &size {
        shapes.push(s.clone());
    }
    for x in &args {
        shapes.push(match x {
            DArg::S(_) => vec![],
            DArg::A(a) => a.shape.clone(),
        });
    }
    for (k, s) in shapes.iter().enumerate() {
        shape = match np_bshape(&shape, s) {
            Ok(b) => b,
            Err(_) => {
                let j = shapes[..k].iter().position(|t| np_bshape(t, s).is_err()).unwrap_or(0);
                return val_err(format!("shape mismatch: objects cannot be broadcast to a single shape.  Mismatch is between arg {j} with shape {} and arg {k} with shape {}.", np_shape_repr(&shapes[j]), np_shape_repr(s)));
            }
        };
    }
    if let Some(s) = &size {
        if shape != *s {
            return val_err(format!("Output size {} is not compatible with broadcast dimensions of inputs {}.", np_shape_repr(s), np_shape_repr(&shape)));
        }
    }
    let vals = args
        .iter()
        .map(|x| match x {
            DArg::S(v) => vec![*v],
            DArg::A(a) => np_bvals(a, &shape).fv(),
        })
        .collect();
    Ok(DPar { vals, shape: Some(shape), scalar: false })
}
fn np_dcont(p: &DPar, mut f: impl FnMut(&[f64]) -> f64) -> V {
    let mut xs = vec![0.0; p.vals.len()];
    match &p.shape {
        None => {
            p.at(0, &mut xs);
            V::Float(f(&xs))
        }
        Some(sh) => {
            let n = p.n();
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                p.at(i, &mut xs);
                out.push(f(&xs));
            }
            np_new(Buf::F(out), sh.clone())
        }
    }
}
fn np_ddisc(p: &DPar, mut f: impl FnMut(&[f64]) -> i64) -> V {
    let mut xs = vec![0.0; p.vals.len()];
    match &p.shape {
        None => {
            p.at(0, &mut xs);
            V::Int(f(&xs))
        }
        Some(sh) => {
            let n = p.n();
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                p.at(i, &mut xs);
                out.push(f(&xs));
            }
            np_new(Buf::I(out), sh.clone())
        }
    }
}

// ----- the algorithms -----
impl Bg {
    fn leg(&self) -> bool {
        matches!(self, Bg::M(_))
    }
    fn std_gamma(&mut self, shape: f64) -> f64 {
        if shape == 1.0 {
            return self.exponential();
        } else if shape == 0.0 {
            return 0.0;
        } else if shape < 1.0 {
            loop {
                let u = self.dbl();
                let v = self.exponential();
                if u <= 1.0 - shape {
                    let x = u.powf(1.0 / shape);
                    if x <= v {
                        return x;
                    }
                } else {
                    let y = -((1.0 - u) / shape).ln();
                    let x = (1.0 - shape + shape * y).powf(1.0 / shape);
                    if x <= v + y {
                        return x;
                    }
                }
            }
        }
        let b = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * b).sqrt();
        loop {
            let (mut x, mut v) = (0.0, 0.0);
            loop {
                x = self.normal();
                v = 1.0 + c * x;
                if !(v <= 0.0) {
                    break;
                }
            }
            v = v * v * v;
            let u = self.dbl();
            if u < 1.0 - 0.0331 * (x * x) * (x * x) {
                return b * v;
            }
            if u.ln() < 0.5 * x * x + b * (1.0 - v + v.ln()) {
                return b * v;
            }
        }
    }
    fn beta(&mut self, a: f64, b: f64) -> f64 {
        if a <= 1.0 && b <= 1.0 {
            if self.leg() {
                loop {
                    let u = self.dbl();
                    let v = self.dbl();
                    let x = u.powf(1.0 / a);
                    let y = v.powf(1.0 / b);
                    if x + y <= 1.0 {
                        if x + y > 0.0 {
                            return x / (x + y);
                        }
                        let (mut lx, mut ly) = (u.ln() / a, v.ln() / b);
                        let lm = if lx > ly { lx } else { ly };
                        lx -= lm;
                        ly -= lm;
                        return (lx - (lx.exp() + ly.exp()).ln()).exp();
                    }
                }
            }
            if a < 3e-103 && b < 3e-103 {
                let u = self.dbl();
                return if (a + b) * u < a { 1.0 } else { 0.0 };
            }
            loop {
                let u = self.dbl();
                let v = self.dbl();
                let x = u.powf(1.0 / a);
                let y = v.powf(1.0 / b);
                let xy = x + y;
                if xy <= 1.0 && u + v > 0.0 {
                    if x > 0.0 && y > 0.0 {
                        return x / xy;
                    }
                    let d = u.ln() / a - v.ln() / b;
                    return if d > 0.0 { (-(-d).exp().ln_1p()).exp() } else { (d - d.exp().ln_1p()).exp() };
                }
            }
        }
        let ga = self.std_gamma(a);
        let gb = self.std_gamma(b);
        ga / (ga + gb)
    }
    fn chisq(&mut self, df: f64) -> f64 {
        2.0 * self.std_gamma(df / 2.0)
    }
    fn nc_chisq(&mut self, df: f64, nonc: f64) -> f64 {
        let leg = self.leg();
        if !leg && nonc.is_nan() {
            return f64::NAN;
        }
        if nonc == 0.0 {
            return self.chisq(df);
        }
        if 1.0 < df {
            let c = self.chisq(df - 1.0);
            let n = self.normal() + nonc.sqrt();
            return c + n * n;
        }
        let i = self.poisson(nonc / 2.0);
        let out = self.chisq(df + 2.0 * i as f64);
        if leg && nonc.is_nan() {
            return f64::NAN;
        }
        out
    }
    fn vonmises(&mut self, mu: f64, kappa: f64) -> f64 {
        use std::f64::consts::PI;
        if kappa.is_nan() {
            return f64::NAN;
        }
        if kappa < 1e-8 {
            return PI * (2.0 * self.dbl() - 1.0);
        }
        let s = if kappa < 1e-5 {
            1.0 / kappa + kappa
        } else if kappa <= 1e6 || self.leg() {
            let r = 1.0 + (1.0 + 4.0 * kappa * kappa).sqrt();
            let rho = (r - (2.0 * r).sqrt()) / (2.0 * kappa);
            (1.0 + rho * rho) / (2.0 * rho)
        } else {
            let mut res = mu + (1.0 / kappa).sqrt() * self.normal();
            if res < -PI {
                res += 2.0 * PI;
            }
            if res > PI {
                res -= 2.0 * PI;
            }
            return res;
        };
        let w = loop {
            let u = self.dbl();
            let z = (PI * u).cos();
            let w = (1.0 + s * z) / (s + z);
            let y = kappa * (s - w);
            let v = self.dbl();
            if y * (2.0 - y) - v >= 0.0 || (y / v).ln() + 1.0 - y >= 0.0 {
                break w;
            }
        };
        let u = self.dbl();
        let mut res = w.acos();
        if u < 0.5 {
            res = -res;
        }
        res += mu;
        let neg = res < 0.0;
        let mut m = res.abs();
        m = (m + PI) % (2.0 * PI) - PI;
        if neg {
            m *= -1.0;
        }
        m
    }
    fn laplace(&mut self, loc: f64, scale: f64) -> f64 {
        loop {
            let u = self.dbl();
            if u >= 0.5 {
                return loc - scale * (2.0 - u - u).ln();
            } else if u > 0.0 {
                return loc + scale * (u + u).ln();
            }
        }
    }
    fn gumbel(&mut self, loc: f64, scale: f64) -> f64 {
        loop {
            let u = 1.0 - self.dbl();
            if u < 1.0 {
                return loc - scale * (-u.ln()).ln();
            }
        }
    }
    fn logistic(&mut self, loc: f64, scale: f64) -> f64 {
        loop {
            let u = self.dbl();
            if u > 0.0 {
                return loc + scale * (u / (1.0 - u)).ln();
            }
        }
    }
    fn wald(&mut self, mean: f64, scale: f64) -> f64 {
        let mut y = self.normal();
        y = mean * y * y;
        let x = if self.leg() {
            let mu_2l = mean / (2.0 * scale);
            mean + mu_2l * (y - (4.0 * scale * y + y * y).sqrt())
        } else {
            let d = 1.0 + (1.0 + 4.0 * scale / y).sqrt();
            mean * (1.0 - 2.0 / d)
        };
        let u = self.dbl();
        if u <= mean / (mean + x) {
            x
        } else {
            mean * mean / x
        }
    }
    fn triangular(&mut self, left: f64, mode: f64, right: f64) -> f64 {
        let base = right - left;
        let lb = mode - left;
        let ratio = lb / base;
        let lp = lb * base;
        let rp = (right - mode) * base;
        let u = self.dbl();
        if u <= ratio {
            left + (u * lp).sqrt()
        } else {
            right - ((1.0 - u) * rp).sqrt()
        }
    }
    fn poisson(&mut self, lam: f64) -> i64 {
        if lam >= 10.0 {
            let slam = lam.sqrt();
            let loglam = lam.ln();
            let b = 0.931 + 2.53 * slam;
            let a = -0.059 + 0.02483 * b;
            let invalpha = 1.1239 + 1.1328 / (b - 3.4);
            let vr = 0.9277 - 3.6224 / (b - 2.0);
            loop {
                let u = self.dbl() - 0.5;
                let v = self.dbl();
                let us = 0.5 - u.abs();
                let k = ((2.0 * a / us + b) * u + lam + 0.43).floor() as i64;
                if us >= 0.07 && v <= vr {
                    return k;
                }
                if k < 0 || (us < 0.013 && v > us) {
                    continue;
                }
                if v.ln() + invalpha.ln() - (a / (us * us) + b).ln() <= -lam + k as f64 * loglam - np_loggam(k as f64 + 1.0) {
                    return k;
                }
            }
        } else if lam == 0.0 {
            return 0;
        }
        let enlam = (-lam).exp();
        let mut x = 0;
        let mut prod = 1.0;
        loop {
            prod *= self.dbl();
            if prod > enlam {
                x += 1;
            } else {
                return x;
            }
        }
    }
    fn binom_inv(&mut self, n: i64, p: f64) -> i64 {
        let q = 1.0 - p;
        let qn = if self.leg() { (n as f64 * q.ln()).exp() } else { (n as f64 * (-p).ln_1p()).exp() };
        let np = n as f64 * p;
        let bound = (n as f64).min(np + 10.0 * (np * q + 1.0).sqrt()) as i64;
        let mut x = 0i64;
        let mut px = qn;
        let mut u = self.dbl();
        while u > px {
            x += 1;
            if x > bound {
                x = 0;
                px = qn;
                u = self.dbl();
            } else {
                u -= px;
                px = ((n - x + 1) as f64 * p * px) / (x as f64 * q);
            }
        }
        x
    }
    fn binom_btpe(&mut self, n: i64, p: f64) -> i64 {
        let nf = n as f64;
        let r = p.min(1.0 - p);
        let q = 1.0 - r;
        let fm = nf * r + r;
        let m = fm.floor() as i64;
        let mf = m as f64;
        let p1 = (2.195 * (nf * r * q).sqrt() - 4.6 * q).floor() + 0.5;
        let xm = mf + 0.5;
        let xl = xm - p1;
        let xr = xm + p1;
        let c = 0.134 + 20.5 / (15.3 + mf);
        let mut a = (fm - xl) / (fm - xl * r);
        let laml = a * (1.0 + a / 2.0);
        a = (xr - fm) / (xr * q);
        let lamr = a * (1.0 + a / 2.0);
        let p2 = p1 * (1.0 + 2.0 * c);
        let p3 = p2 + c / laml;
        let p4 = p3 + c / lamr;
        let mut y: i64;
        'step10: loop {
            let nrq = nf * r * q;
            let u = self.dbl() * p4;
            let mut v = self.dbl();
            if u <= p1 {
                y = (xm - p1 * v + u).floor() as i64;
                break 'step10;
            }
            if u <= p2 {
                let x = xl + (u - p1) / c;
                v = v * c + 1.0 - (mf - x + 0.5).abs() / p1;
                if v > 1.0 {
                    continue 'step10;
                }
                y = x.floor() as i64;
            } else if u <= p3 {
                y = (xl + v.ln() / laml).floor() as i64;
                if y < 0 || v == 0.0 {
                    continue 'step10;
                }
                v = v * (u - p2) * laml;
            } else {
                y = (xr - v.ln() / lamr).floor() as i64;
                if y > n || v == 0.0 {
                    continue 'step10;
                }
                v = v * (u - p3) * lamr;
            }
            // Step50
            let k = (y - m).abs();
            if !(k > 20 && (k as f64) < nrq / 2.0 - 1.0) {
                let s = r / q;
                let a = s * (nf + 1.0);
                let mut f = 1.0;
                if m < y {
                    for i in m + 1..=y {
                        f *= a / i as f64 - s;
                    }
                } else if m > y {
                    for i in y + 1..=m {
                        f /= a / i as f64 - s;
                    }
                }
                if v > f {
                    continue 'step10;
                }
                break 'step10;
            }
            // Step52
            let kf = k as f64;
            let rho = (kf / nrq) * ((kf * (kf / 3.0 + 0.625) + 0.16666666666666666) / nrq + 0.5);
            let t = -kf * kf / (2.0 * nrq);
            let aa = v.ln();
            if aa < t - rho {
                break 'step10;
            }
            if aa > t + rho {
                continue 'step10;
            }
            let x1 = (y + 1) as f64;
            let f1 = mf + 1.0;
            let z = nf + 1.0 - mf;
            let w = nf - y as f64 + 1.0;
            let (x2, f2, z2, w2) = (x1 * x1, f1 * f1, z * z, w * w);
            // numpy 2.x corrected the Stirling terms (13860, the last two subtracted); RandomState keeps the old ones
            let leg = self.leg();
            let c0 = if leg { 13680. } else { 13860. };
            let st = |d: f64, d2: f64| (c0 - (462. - (132. - (99. - 140. / d2) / d2) / d2) / d2) / d / 166320.;
            let bound = if leg {
                xm * (f1 / x1).ln() + (nf - mf + 0.5) * (z / w).ln() + (y - m) as f64 * (w * r / (x1 * q)).ln() + st(f1, f2) + st(z, z2) + st(x1, x2) + st(w, w2)
            } else {
                xm * (f1 / x1).ln() + (nf - mf + 0.5) * (z / w).ln() + (y - m) as f64 * (w * r / (x1 * q)).ln() + st(f1, f2) + st(z, z2) - st(x1, x2) - st(w, w2)
            };
            if aa > bound {
                continue 'step10;
            }
            break 'step10;
        }
        if p > 0.5 {
            y = n - y;
        }
        y
    }
    fn binomial(&mut self, p: f64, n: i64) -> i64 {
        // the legacy RandomState.binomial skips this early exit (and so draws a value for n == 0 or p == 0)
        if !self.leg() && (n == 0 || p == 0.0) {
            return 0;
        }
        self.binomial_any(p, n)
    }
    fn binomial_any(&mut self, p: f64, n: i64) -> i64 {
        if p <= 0.5 {
            if p * n as f64 <= 30.0 {
                self.binom_inv(n, p)
            } else {
                self.binom_btpe(n, p)
            }
        } else {
            let q = 1.0 - p;
            if q * n as f64 <= 30.0 {
                n - self.binom_inv(n, q)
            } else {
                n - self.binom_btpe(n, q)
            }
        }
    }
    fn geometric(&mut self, p: f64) -> i64 {
        if p >= 0.333333333333333333333333 {
            let mut x = 1;
            let mut sum = p;
            let mut prod = p;
            let q = 1.0 - p;
            let u = self.dbl();
            while u > sum {
                prod *= q;
                sum += prod;
                x += 1;
            }
            return x;
        }
        if self.leg() {
            return ((-self.dbl()).ln_1p() / (1.0 - p).ln()).ceil() as i64;
        }
        let z = (-self.exponential() / (-p).ln_1p()).ceil();
        if z >= 9.223372036854776e18 {
            return i64::MAX;
        }
        z as i64
    }
    fn zipf(&mut self, a: f64) -> i64 {
        let leg = self.leg();
        if !leg && a >= 1025.0 {
            return 1;
        }
        let am1 = a - 1.0;
        let b = 2f64.powf(am1);
        let umin = if leg { 0.0 } else { (i64::MAX as f64).powf(-am1) };
        loop {
            let u = if leg {
                1.0 - self.dbl()
            } else {
                let u01 = self.dbl();
                u01 * umin + (1.0 - u01)
            };
            let v = self.dbl();
            let x = u.powf(-1.0 / am1).floor();
            if x > i64::MAX as f64 || x < 1.0 {
                continue;
            }
            let t = (1.0 + 1.0 / x).powf(am1);
            if v * x * (t - 1.0) / (b - 1.0) <= t / b {
                return x as i64;
            }
        }
    }
    fn logseries(&mut self, p: f64) -> i64 {
        let leg = self.leg();
        let r = if leg { (1.0 - p).ln() } else { (-p).ln_1p() };
        loop {
            let v = self.dbl();
            if v >= p {
                return 1;
            }
            let u = self.dbl();
            let q = if leg { 1.0 - (r * u).exp() } else { -(r * u).exp_m1() };
            if v <= q * q {
                let res = (1.0 + v.ln() / q.ln()).floor() as i64;
                if res < 1 || v == 0.0 {
                    continue;
                }
                return res;
            }
            if v >= q {
                return 1;
            }
            return 2;
        }
    }
    fn hypergeometric(&mut self, good: i64, bad: i64, sample: i64) -> i64 {
        if self.leg() {
            if sample > 10 {
                return self.hyper_hrua_leg(good, bad, sample);
            } else if sample > 0 {
                let d1 = bad + good - sample;
                let d2 = good.min(bad) as f64;
                let mut y = d2;
                let mut k = sample;
                while y > 0.0 {
                    let u = self.dbl();
                    y -= (u + y / (d1 + k) as f64).floor() as i64 as f64;
                    k -= 1;
                    if k == 0 {
                        break;
                    }
                }
                let mut z = (d2 - y) as i64;
                if good > bad {
                    z = sample - z;
                }
                return z;
            }
            return 0;
        }
        if sample >= 10 && sample <= good + bad - 10 {
            return self.hyper_hrua(good, bad, sample);
        }
        let total = good + bad;
        let cs = if sample > total / 2 { total - sample } else { sample };
        let (mut rt, mut rg, mut c) = (total, good, cs);
        while c > 0 && rg > 0 && rt > rg {
            rt -= 1;
            if (self.interval(rt as u64) as i64) < rg {
                rg -= 1;
            }
            c -= 1;
        }
        if rt == rg {
            rg -= c;
        }
        if sample > total / 2 {
            rg
        } else {
            good - rg
        }
    }
    fn hyper_hrua(&mut self, good: i64, bad: i64, sample: i64) -> i64 {
        const D1: f64 = 1.7155277699214135;
        const D2: f64 = 0.8989161620588988;
        let popsize = good + bad;
        let cs = sample.min(popsize - sample);
        let mn = good.min(bad);
        let mx = good.max(bad);
        let p = mn as f64 / popsize as f64;
        let q = mx as f64 / popsize as f64;
        let mu = cs as f64 * p;
        let a = mu + 0.5;
        let var = (popsize - cs) as f64 * cs as f64 * p * q / (popsize - 1) as f64;
        let c = (var + 0.5).sqrt();
        let h = D1 * c + D2;
        let m = ((cs + 1) as f64 * (mn + 1) as f64 / (popsize + 2) as f64).floor() as i64;
        let g = np_logfact(m) + np_logfact(mn - m) + np_logfact(cs - m) + np_logfact(mx - cs + m);
        let b = ((cs.min(mn) + 1) as f64).min((a + 16.0 * c).floor());
        let mut k;
        loop {
            let u = self.dbl();
            let v = self.dbl();
            let x = a + h * (v - 0.5) / u;
            if x < 0.0 || x >= b {
                continue;
            }
            k = x.floor() as i64;
            let gp = np_logfact(k) + np_logfact(mn - k) + np_logfact(cs - k) + np_logfact(mx - cs + k);
            let t = g - gp;
            if u * (4.0 - u) - 3.0 <= t {
                break;
            }
            if u * (u - t) >= 1.0 {
                continue;
            }
            if 2.0 * u.ln() <= t {
                break;
            }
        }
        if good > bad {
            k = cs - k;
        }
        if cs < sample {
            k = good - k;
        }
        k
    }
    fn hyper_hrua_leg(&mut self, good: i64, bad: i64, sample: i64) -> i64 {
        const D1: f64 = 1.7155277699214135;
        const D2: f64 = 0.8989161620588988;
        let mn = good.min(bad);
        let popsize = good + bad;
        let mx = good.max(bad);
        let m = sample.min(popsize - sample);
        let d4 = mn as f64 / popsize as f64;
        let d5 = 1.0 - d4;
        let d6 = m as f64 * d4 + 0.5;
        let d7 = ((popsize - m) as f64 * sample as f64 * d4 * d5 / (popsize - 1) as f64 + 0.5).sqrt();
        let d8 = D1 * d7 + D2;
        let d9 = ((m + 1) as f64 * (mn + 1) as f64 / (popsize + 2) as f64).floor() as i64;
        let lg = |x: i64| np_loggam(x as f64);
        let d10 = lg(d9 + 1) + lg(mn - d9 + 1) + lg(m - d9 + 1) + lg(mx - m + d9 + 1);
        let d11 = ((m.min(mn)) as f64 + 1.0).min((d6 + 16.0 * d7).floor());
        let mut z;
        loop {
            let x = self.dbl();
            let y = self.dbl();
            let w = d6 + d8 * (y - 0.5) / x;
            if w < 0.0 || w >= d11 {
                continue;
            }
            z = w.floor() as i64;
            let t = d10 - (lg(z + 1) + lg(mn - z + 1) + lg(m - z + 1) + lg(mx - m + z + 1));
            if x * (4.0 - x) - 3.0 <= t {
                break;
            }
            if x * (x - t) >= 1.0 {
                continue;
            }
            if 2.0 * x.ln() <= t {
                break;
            }
        }
        if good > bad {
            z = m - z;
        }
        if m < sample {
            z = good - z;
        }
        z
    }
}
fn np_loggam(x: f64) -> f64 {
    const A: [f64; 10] = [8.333333333333333e-02, -2.777777777777778e-03, 7.936507936507937e-04, -5.952380952380952e-04, 8.417508417508418e-04, -1.917526917526918e-03, 6.410256410256410e-03, -2.955065359477124e-02, 1.796443723688307e-01, -1.39243221690590e+00];
    if x == 1.0 || x == 2.0 {
        return 0.0;
    }
    let n = if x < 7.0 { (7.0 - x) as i64 } else { 0 };
    let mut x0 = x + n as f64;
    let x2 = (1.0 / x0) * (1.0 / x0);
    let lg2pi = 1.8378770664093453e+00;
    let mut gl0 = A[9];
    for k in (0..=8).rev() {
        gl0 *= x2;
        gl0 += A[k];
    }
    let mut gl = gl0 / x0 + 0.5 * lg2pi + (x0 - 0.5) * x0.ln() - x0;
    if x < 7.0 {
        for _ in 1..=n {
            gl -= (x0 - 1.0).ln();
            x0 -= 1.0;
        }
    }
    gl
}
const NP_LOGFACT: [f64; 126] = [
    0.0, 0.0, 0.69314718055994529, 1.791759469228055, 3.1780538303479458, 4.7874917427820458, 6.5792512120101012, 8.5251613610654147, 10.604602902745251, 12.801827480081469, 15.104412573075516, 17.502307845873887, 19.987214495661885, 22.552163853123425, 25.19122118273868, 27.89927138384089, 30.671860106080672, 33.505073450136891, 36.395445208033053, 39.339884187199495, 42.335616460753485, 45.380138898476908, 48.471181351835227, 51.606675567764377, 54.784729398112319, 58.003605222980518, 61.261701761002001, 64.557538627006338, 67.88974313718154, 71.257038967168015, 74.658236348830158, 78.092223553315307, 81.557959456115043, 85.054467017581516, 88.580827542197682, 92.136175603687093, 95.719694542143202, 99.330612454787428, 102.96819861451381, 106.63176026064346, 110.32063971475739, 114.03421178146171, 117.77188139974507, 121.53308151543864, 125.3172711493569, 129.12393363912722, 132.95257503561632, 136.80272263732635, 140.67392364823425, 144.5657439463449, 148.47776695177302, 152.40959258449735, 156.3608363030788, 160.3311282166309, 164.32011226319517, 168.32744544842765, 172.35279713916279, 176.39584840699735, 180.45629141754378, 184.53382886144948, 188.6281734236716, 192.7390472878449, 196.86618167289001, 201.00931639928152, 205.1681994826412, 209.34258675253685, 213.53224149456327, 217.73693411395422, 221.95644181913033, 226.1905483237276, 230.43904356577696, 234.70172344281826, 238.97838956183432, 243.26884900298271, 247.57291409618688, 251.89040220972319, 256.22113555000954, 260.56494097186322, 264.92164979855278, 269.29109765101981, 273.67312428569369, 278.06757344036612, 282.4742926876304, 286.89313329542699, 291.32395009427029, 295.76660135076065, 300.22094864701415, 304.68685676566872, 309.1641935801469, 313.65282994987905, 318.1526396202093, 322.66349912672615, 327.1852877037752, 331.71788719692847, 336.26118197919845, 340.81505887079902, 345.37940706226686, 349.95411804077025, 354.53908551944079, 359.1342053695754, 363.73937555556347, 368.35449607240474, 372.97946888568902, 377.61419787391867, 382.25858877306001, 386.91254912321756, 391.57598821732961, 396.24881705179155, 400.93094827891576, 405.6222961611449, 410.32277652693733, 415.03230672824964, 419.75080559954472, 424.47819341825709, 429.21439186665157, 433.95932399501481, 438.71291418612117, 443.47508812091894, 448.24577274538461, 453.02489623849613, 457.81238798127816, 462.60817852687489, 467.4121995716082, 472.22438392698058, 477.04466549258564, 481.87297922988796,
];
fn np_logfact(k: i64) -> f64 {
    if (k as u64) < NP_LOGFACT.len() as u64 {
        return NP_LOGFACT[k as usize];
    }
    let kf = k as f64;
    (kf + 0.5) * kf.ln() - kf + (0.9189385332046728 + (1.0 / kf) * (1.0 / 12.0 - 1.0 / (360.0 * kf * kf)))
}
fn np_kahan(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut sum = xs[0];
    let mut c = 0.0;
    for x in &xs[1..] {
        let y = x - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }
    sum
}

// ----- the methods -----
// Some(result) when `n` is one of the distributions here
fn np_dist_call(r: &NpRng, n: &str, a: &Args) -> Option<R> {
    use Cons::*;
    if !NP_DISTS.contains(&n) {
        return None;
    }
    let leg = r.legacy;
    let pd = |specs: &[PSpec]| np_dpar(a, n, specs, false);
    let res = (|| -> R {
        Ok(match n {
            "standard_gamma" => {
                if !leg && (npa(a, 2, "dtype").is_some() || npa(a, 3, "out").is_some()) {
                    return Err(np_unsup("standard_gamma(dtype=/out=)"));
                }
                let p = pd(&[("shape", None, NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.std_gamma(x[0]))
            }
            "gamma" => {
                let p = pd(&[("shape", None, NonNeg), ("scale", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| x[1] * bg.std_gamma(x[0]))
            }
            "beta" => {
                let p = pd(&[("a", None, Pos), ("b", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.beta(x[0], x[1]))
            }
            "chisquare" => {
                let p = pd(&[("df", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.chisq(x[0]))
            }
            "noncentral_chisquare" => {
                let p = pd(&[("df", None, Pos), ("nonc", None, NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.nc_chisq(x[0], x[1]))
            }
            "f" => {
                let p = pd(&[("dfnum", None, Pos), ("dfden", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| (bg.chisq(x[0]) * x[1]) / (bg.chisq(x[1]) * x[0]))
            }
            "noncentral_f" => {
                let p = pd(&[("dfnum", None, Pos), ("dfden", None, Pos), ("nonc", None, NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| {
                    let t = bg.nc_chisq(x[0], x[2]) * x[1];
                    t / (bg.chisq(x[1]) * x[0])
                })
            }
            "standard_cauchy" => {
                let p = pd(&[])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |_| {
                    let a = bg.normal();
                    a / bg.normal()
                })
            }
            "standard_t" => {
                let p = pd(&[("df", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| {
                    let num = bg.normal();
                    let den = bg.std_gamma(x[0] / 2.0);
                    (x[0] / 2.0).sqrt() * num / den.sqrt()
                })
            }
            "vonmises" => {
                let p = pd(&[("mu", None, No), ("kappa", None, NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.vonmises(x[0], x[1]))
            }
            "pareto" => {
                let p = pd(&[("a", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| if leg { (bg.exponential() / x[0]).exp() - 1.0 } else { (bg.exponential() / x[0]).exp_m1() })
            }
            "weibull" => {
                let p = pd(&[("a", None, NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| if x[0] == 0.0 { 0.0 } else { bg.exponential().powf(1.0 / x[0]) })
            }
            "power" => {
                let p = pd(&[("a", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| {
                    let e = bg.exponential();
                    let b = if leg { 1.0 - (-e).exp() } else { -(-e).exp_m1() };
                    b.powf(1.0 / x[0])
                })
            }
            "laplace" | "gumbel" | "logistic" => {
                let p = pd(&[("loc", Some(0.0), No), ("scale", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                match n {
                    "laplace" => np_dcont(&p, |x| bg.laplace(x[0], x[1])),
                    "gumbel" => np_dcont(&p, |x| bg.gumbel(x[0], x[1])),
                    _ => np_dcont(&p, |x| bg.logistic(x[0], x[1])),
                }
            }
            "lognormal" => {
                let p = pd(&[("mean", Some(0.0), No), ("sigma", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| (x[0] + x[1] * bg.normal()).exp())
            }
            "rayleigh" => {
                let p = pd(&[("scale", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| if leg { x[0] * (-2.0 * (-bg.dbl()).ln_1p()).sqrt() } else { x[0] * (2.0 * bg.exponential()).sqrt() })
            }
            "wald" => {
                let p = pd(&[("mean", None, Pos), ("scale", None, Pos)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.wald(x[0], x[1]))
            }
            "triangular" => {
                let p = pd(&[("left", None, No), ("mode", None, No), ("right", None, No)])?;
                let n = p.n();
                let mut xs = [0.0; 3];
                let tot = if p.scalar { 1 } else { n };
                for i in 0..tot {
                    p.at(i, &mut xs);
                    if xs[0] > xs[1] {
                        return val_err("left > mode".into());
                    }
                }
                for i in 0..tot {
                    p.at(i, &mut xs);
                    if xs[1] > xs[2] {
                        return val_err("mode > right".into());
                    }
                }
                for i in 0..tot {
                    p.at(i, &mut xs);
                    if xs[0] == xs[2] {
                        return val_err("left == right".into());
                    }
                }
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| bg.triangular(x[0], x[1], x[2]))
            }
            "poisson" => {
                let p = pd(&[("lam", Some(1.0), Poi)])?;
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.poisson(x[0]))
            }
            "zipf" => {
                let p = pd(&[("a", None, Gt1)])?;
                // numpy's legacy zipf never returns once 2**(a-1) overflows
                if leg && p.vals[0].iter().any(|a| 2f64.powf(a - 1.0).is_infinite()) {
                    return Err(np_unsup("RandomState.zipf with a >= 1025"));
                }
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.zipf(x[0]))
            }
            "geometric" => {
                let p = pd(&[("p", None, BGt01)])?;
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.geometric(x[0]))
            }
            "logseries" => {
                let p = pd(&[("p", None, BLt01)])?;
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.logseries(x[0]))
            }
            "negative_binomial" => {
                let p = if leg { pd(&[("n", None, Pos), ("p", None, B01)])? } else { pd(&[("n", None, PosNN), ("p", None, BGt01)])? };
                let tot = if p.scalar { 1 } else { p.n() };
                let mut xs = [0.0; 2];
                for i in 0..tot {
                    p.at(i, &mut xs);
                    if !leg && (1.0 - xs[1]) / xs[1] * (xs[0] + 10.0 * xs[0].sqrt()) > np_lam_max() {
                        return val_err("n too large or p too small, see Generator.negative_binomial Notes".into());
                    }
                    if leg && (xs[1] == 0.0 || xs[0].is_nan() || xs[0].is_infinite()) {
                        return Err(np_unsup("RandomState.negative_binomial with p == 0 or n not finite"));
                    }
                }
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| {
                    let y = (1.0 - x[1]) / x[1] * bg.std_gamma(x[0]);
                    bg.poisson(y)
                })
            }
            "binomial" => {
                let nv = npa(a, 0, "n").ok_or_else(|| np_dmissing(n, 2, a))?;
                let pv = npa(a, 1, "p").ok_or_else(|| np_dmissing(n, 2, a))?;
                let pa = np_darg(&pv, false)?;
                let na = np_darg(&nv, true)?;
                let size = np_size(a, 2)?;
                let scalar = matches!((&pa, &na), (DArg::S(_), DArg::S(_)));
                let ncons = if leg { LLong } else { NonNeg };
                // numpy checks p, then n; the shape after (one broadcast of p and n)
                let p = if scalar {
                    let (DArg::S(pf), DArg::S(nf)) = (&pa, &na) else { unreachable!() };
                    np_cons1(*pf, "p", B01)?;
                    np_cons1(*nf, "n", ncons)?;
                    DPar { vals: vec![vec![*pf], vec![*nf]], shape: size, scalar: true }
                } else {
                    np_dpar_from(vec![pa, na], &[("p", None, B01), ("n", None, ncons)], size)?
                };
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.binomial(x[0], x[1] as i64))
            }
            "hypergeometric" => {
                let specs: [PSpec; 3] = if leg { [("ngood", None, LLong), ("nbad", None, NonNeg), ("nsample", None, Gte1)] } else { [("ngood", None, NonNeg), ("nbad", None, NonNeg), ("nsample", None, NonNeg)] };
                let mut args = vec![];
                for (i, (nm, _, _)) in specs.iter().enumerate() {
                    let v = npa(a, i, nm).ok_or_else(|| np_dmissing(n, 3, a))?;
                    args.push(np_darg(&v, true)?);
                }
                let size = np_size(a, 3)?;
                let vals = |x: &DArg| match x {
                    DArg::S(v) => vec![*v],
                    DArg::A(a) => a.vals().fv(),
                };
                // the pre-checks run on the unbroadcast values, as numpy's do
                let (g, b, s) = (vals(&args[0]), vals(&args[1]), vals(&args[2]));
                if !leg && (g.iter().any(|x| *x >= 1e9) || b.iter().any(|x| *x >= 1e9)) {
                    return val_err("both ngood and nbad must be less than 1000000000".into());
                }
                let shp = |x: &DArg| match x {
                    DArg::S(_) => vec![],
                    DArg::A(a) => a.shape.clone(),
                };
                let sh = np_bshape(&np_bshape(&shp(&args[0]), &shp(&args[1]))?, &shp(&args[2]))?;
                let tot: usize = sh.iter().product();
                if matches!((&args[0], &args[1], &args[2]), (DArg::S(_), DArg::S(_), DArg::S(_))) {
                    if g[0] + b[0] < s[0] {
                        return val_err("ngood + nbad < nsample".into());
                    }
                } else {
                    let (gb, bb, sb) = (np_bcast_f(&args[0], &sh), np_bcast_f(&args[1], &sh), np_bcast_f(&args[2], &sh));
                    if (0..tot).any(|i| gb[i] + bb[i] < sb[i]) {
                        return val_err("ngood + nbad < nsample".into());
                    }
                }
                let p = np_dpar_from(args, &specs, size)?;
                let mut bg = r.bg.borrow_mut();
                np_ddisc(&p, |x| bg.hypergeometric(x[0] as i64, x[1] as i64, x[2] as i64))
            }
            "multinomial" => {
                let nv = npa(a, 0, "n").ok_or_else(|| np_dmissing(n, 2, a))?;
                let pv = npa(a, 1, "pvals").ok_or_else(|| np_dmissing(n, 2, a))?;
                if as_np(&nv).is_some() || seq_items(&nv).is_some() {
                    return Err(np_unsup("multinomial with an array n"));
                }
                let ni = np_int_arg(&nv)?;
                let parr = np_as(&pv)?;
                if parr.ndim() > 1 {
                    return Err(np_unsup("multinomial with 2-d pvals"));
                }
                if parr.ndim() == 0 {
                    return if leg { type_err("pvals must be a 1-d sequence".into()) } else { val_err("pvals must have at least 1 dimension and the last dimension of pvals must be greater than 0.".into()) };
                }
                let pix = parr.vals().fv();
                let d = pix.len();
                if d == 0 && !leg {
                    return val_err("pvals must have at least 1 dimension and the last dimension of pvals must be greater than 0.".into());
                }
                np_cons_arr(&pix, "pvals", B01)?;
                if d > 0 && np_kahan(&pix[..d - 1]) > 1.0 + 1e-12 {
                    return val_err("sum(pvals[:-1]) > 1.0".into());
                }
                let size = np_size(a, 2)?;
                let mut shape = size.clone().unwrap_or_default();
                shape.push(d);
                np_cons1(ni as f64, "n", NonNeg)?;
                let tot: usize = shape.iter().product();
                let mut out = vec![0i64; tot];
                let mut bg = r.bg.borrow_mut();
                if d > 0 {
                    for row in out.chunks_mut(d) {
                        let mut rem = 1.0;
                        let mut dn = ni;
                        for j in 0..d - 1 {
                            row[j] = bg.binomial(pix[j] / rem, dn);
                            dn -= row[j];
                            if dn <= 0 {
                                break;
                            }
                            rem -= pix[j];
                        }
                        if dn > 0 {
                            row[d - 1] = dn;
                        }
                    }
                }
                np_new(Buf::I(out), shape)
            }
            "dirichlet" => {
                let av = npa(a, 0, "alpha").ok_or_else(|| np_dmissing(n, 1, a))?;
                let arr = np_as(&av)?;
                if arr.ndim() != 1 {
                    return Err(np_unsup("dirichlet with alpha that is not 1-d"));
                }
                let al = arr.vals().fv();
                let k = al.len();
                if leg {
                    if al.iter().any(|x| *x <= 0.0) {
                        return val_err("alpha <= 0".into());
                    }
                } else if al.iter().any(|x| *x < 0.0) {
                    return val_err("alpha < 0".into());
                }
                let size = np_size(a, 1)?;
                let mut shape = size.clone().unwrap_or_default();
                shape.push(k);
                let tot: usize = shape.iter().product();
                let mut out = vec![0.0; tot];
                let mut bg = r.bg.borrow_mut();
                let amax = al.iter().cloned().fold(f64::NEG_INFINITY, |m, x| if x > m || x.is_nan() { x } else { m });
                if k > 0 && !leg && amax < 0.1 {
                    let mut cs = vec![0.0; k];
                    let mut csum = 0.0;
                    for j in (0..k).rev() {
                        csum += al[j];
                        cs[j] = csum;
                    }
                    if csum > 0.0 {
                        for row in out.chunks_mut(k) {
                            let mut acc = 1.0;
                            for j in 0..k - 1 {
                                let v = bg.beta(al[j], cs[j + 1]);
                                row[j] = acc * v;
                                acc *= 1.0 - v;
                                if cs[j + 1] == 0.0 {
                                    break;
                                }
                            }
                            row[k - 1] = acc;
                        }
                    }
                } else if k > 0 {
                    for row in out.chunks_mut(k) {
                        let mut acc = 0.0;
                        for j in 0..k {
                            row[j] = bg.std_gamma(al[j]);
                            acc += row[j];
                        }
                        let inv = 1.0 / acc;
                        for x in row.iter_mut() {
                            *x *= inv;
                        }
                    }
                }
                np_new(Buf::F(out), shape)
            }
            "normal" => {
                let p = pd(&[("loc", Some(0.0), No), ("scale", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| x[0] + x[1] * bg.normal())
            }
            "exponential" => {
                let p = pd(&[("scale", Some(1.0), NonNeg)])?;
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| x[0] * bg.exponential())
            }
            "uniform" => {
                let lo = np_darg(&npa(a, 0, "low").unwrap_or(V::Float(0.0)), false)?;
                let hi = np_darg(&npa(a, 1, "high").unwrap_or(V::Float(1.0)), false)?;
                let size = np_size(a, 2)?;
                let rc = if leg { No } else { NonNeg };
                let p = match (&lo, &hi) {
                    (DArg::S(l), DArg::S(h)) => {
                        let rng = h - l;
                        if !rng.is_finite() {
                            return Err(exc("OverflowError", if leg { "Range exceeds valid bounds".into() } else { "high - low range exceeds valid bounds".into() }));
                        }
                        np_dpar_from(vec![DArg::S(*l), DArg::S(rng)], &[("", None, No), ("high - low", None, rc)], size)?
                    }
                    _ => {
                        let la = match &lo {
                            DArg::S(v) => NdArr::new(Buf::F(vec![*v]), vec![]),
                            DArg::A(a) => a.clone(),
                        };
                        let ha = match &hi {
                            DArg::S(v) => NdArr::new(Buf::F(vec![*v]), vec![]),
                            DArg::A(a) => a.clone(),
                        };
                        let sh = np_bshape(&la.shape, &ha.shape)?;
                        let (lv, hv) = (np_bvals(&la, &sh).fv(), np_bvals(&ha, &sh).fv());
                        let rv: Vec<f64> = hv.iter().zip(&lv).map(|(h, l)| h - l).collect();
                        if rv.iter().any(|x| !x.is_finite()) {
                            return Err(exc("OverflowError", "Range exceeds valid bounds".into()));
                        }
                        let ra = NdArr::new(Buf::F(rv), sh.clone());
                        np_dpar_from(vec![lo, DArg::A(ra)], &[("", None, No), ("high - low", None, rc)], size)?
                    }
                };
                let mut bg = r.bg.borrow_mut();
                np_dcont(&p, |x| x[0] + x[1] * bg.dbl())
            }
            _ => return Err(np_unsup(n)),
        })
    })();
    Some(res)
}
const NP_DISTS: &[&str] = &[
    "standard_gamma", "gamma", "beta", "chisquare", "noncentral_chisquare", "f", "noncentral_f", "standard_cauchy", "standard_t", "vonmises", "pareto", "weibull", "power", "laplace", "gumbel", "logistic", "lognormal", "rayleigh", "wald", "triangular", "poisson", "zipf", "geometric", "logseries", "negative_binomial", "binomial", "hypergeometric", "multinomial", "dirichlet", "normal", "exponential", "uniform",
];
fn np_bcast_f(x: &DArg, sh: &[usize]) -> Vec<f64> {
    let n: usize = sh.iter().product();
    match x {
        DArg::S(v) => vec![*v; n],
        DArg::A(a) => np_bvals(a, sh).fv(),
    }
}
