
// ---------- library modules ----------

fn f1(a: &Args, f: &str) -> Result<f64, E> {
    a.want(f, 1, 1)?;
    to_float(&a.p[0]).map_err(|_| exc("TypeError", format!("must be real number, not {}", tn(&a.p[0]))))
}
fn math_dom<T>() -> Result<T, E> {
    val_err("math domain error".into())
}
fn fl_int(f: f64) -> R {
    if f.is_nan() {
        return val_err("cannot convert float NaN to integer".into());
    }
    if f.is_infinite() {
        return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
    }
    to_int(&V::Float(f))
}
fn int_arg(v: &V, f: &str) -> Result<Big, E> {
    big_of(v).ok_or_else(|| exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v))))
}
fn isqrt(n: &Big) -> Big {
    if n.is_zero() {
        return n.clone();
    }
    // Newton
    let mut x = Big::from_i64(1).shl((n.bits() + 1) / 2);
    loop {
        let y = x.add(&n.divmod(&x).0).shr(1);
        if y.cmp(&x) != std::cmp::Ordering::Less {
            return x;
        }
        x = y;
    }
}
pub fn math_fn(name: &str, a: Args) -> R {
    Ok(match name {
        "sqrt" => {
            let x = f1(&a, name)?;
            if x < 0.0 {
                return math_dom();
            }
            V::Float(x.sqrt())
        }
        "isqrt" => {
            let n = int_arg(&a1(&a, name)?, name)?;
            if n.neg {
                return val_err("isqrt() argument must be nonnegative".into());
            }
            isqrt(&n).norm()
        }
        "floor" | "ceil" | "trunc" => {
            let x = a1(&a, name)?;
            match &x {
                V::Int(_) | V::Big(_) | V::Bool(_) => to_int(&x)?,
                V::Float(f) => fl_int(match name {
                    "floor" => f.floor(),
                    "ceil" => f.ceil(),
                    _ => f.trunc(),
                })?,
                V::Obj(o) => {
                    let d = format!("__{name}__");
                    match o.cls.lookup(&d) {
                        Some(f) => call(&f, Args::p(vec![x.clone()]))?,
                        None => return type_err(format!("must be real number, not {}", tn(&x))),
                    }
                }
                V::Ext(e) => match e.method(&x, &format!("__{name}__"), &mut Args::p(vec![])) {
                    Some(r) => r?,
                    None => fl_int({
                        let f = to_float(&x)?;
                        match name {
                            "floor" => f.floor(),
                            "ceil" => f.ceil(),
                            _ => f.trunc(),
                        }
                    })?,
                },
                _ => return type_err(format!("must be real number, not {}", tn(&x))),
            }
        }
        "gcd" | "lcm" => {
            let mut acc = Big::from_i64(if name == "gcd" { 0 } else { 1 });
            for x in &a.p {
                let b = int_arg(x, name)?;
                if name == "gcd" {
                    acc = big_gcd(&acc, &b);
                } else {
                    if b.is_zero() || acc.is_zero() {
                        acc = Big::from_i64(0);
                        continue;
                    }
                    let g = big_gcd(&acc, &b);
                    acc = Big { neg: false, mag: acc.mul(&b).divmod(&g).0.mag };
                }
            }
            acc.norm()
        }
        "factorial" => {
            let n = idx_of(&a1(&a, name)?)?;
            if n < 0 {
                return val_err("factorial() not defined for negative values".into());
            }
            let mut r = Big::from_i64(1);
            for i in 2..=n {
                r = r.mul(&Big::from_i64(i));
            }
            r.norm()
        }
        "comb" | "perm" => {
            a.want(name, 1, 2)?;
            let n = idx_of(&a.p[0])?;
            let k = match a.p.get(1) {
                Some(V::None) | None => n,
                Some(x) => idx_of(x)?,
            };
            if n < 0 || k < 0 {
                return val_err(format!("{} must be a non-negative integer", if n < 0 { "n" } else { "k" }));
            }
            if k > n {
                return Ok(V::Int(0));
            }
            let mut r = Big::from_i64(1);
            if name == "perm" {
                for i in (n - k + 1)..=n {
                    r = r.mul(&Big::from_i64(i));
                }
            } else {
                let k = k.min(n - k);
                for i in 0..k {
                    r = r.mul(&Big::from_i64(n - i)).divmod(&Big::from_i64(i + 1)).0;
                }
            }
            r.norm()
        }
        "fabs" => V::Float(f1(&a, name)?.abs()),
        "exp" => {
            let r = f1(&a, name)?.exp();
            if r.is_infinite() {
                return Err(exc("OverflowError", "math range error".into()));
            }
            V::Float(r)
        }
        "log" => {
            a.want(name, 1, 2)?;
            let x = &a.p[0];
            let lx = match x {
                V::Big(b) => {
                    if b.neg || b.is_zero() {
                        return math_dom();
                    }
                    let sh = b.bits().saturating_sub(60);
                    b.shr(sh).to_f64().ln() + sh as f64 * std::f64::consts::LN_2
                }
                _ => {
                    let f = to_float(x)?;
                    if f <= 0.0 {
                        return math_dom();
                    }
                    f.ln()
                }
            };
            match a.p.get(1) {
                Some(b) => {
                    let b = to_float(b)?;
                    if b <= 0.0 || b == 1.0 {
                        if b == 1.0 {
                            return Err(exc("ZeroDivisionError", "float division by zero".into()));
                        }
                        return math_dom();
                    }
                    V::Float(lx / b.ln())
                }
                None => V::Float(lx),
            }
        }
        "log2" | "log10" | "log1p" => {
            let x = f1(&a, name)?;
            if (x <= 0.0 && name != "log1p") || (name == "log1p" && x <= -1.0) {
                return math_dom();
            }
            V::Float(match name {
                "log2" => x.log2(),
                "log10" => x.log10(),
                _ => x.ln_1p(),
            })
        }
        "pow" => {
            a.want(name, 2, 2)?;
            let (x, y) = (to_float(&a.p[0])?, to_float(&a.p[1])?);
            if x == 0.0 && y < 0.0 {
                return math_dom();
            }
            if x < 0.0 && y.fract() != 0.0 {
                return math_dom();
            }
            V::Float(x.powf(y))
        }
        "sin" => V::Float(f1(&a, name)?.sin()),
        "cos" => V::Float(f1(&a, name)?.cos()),
        "tan" => V::Float(f1(&a, name)?.tan()),
        "asin" => V::Float(f1(&a, name)?.asin()),
        "acos" => V::Float(f1(&a, name)?.acos()),
        "atan" => V::Float(f1(&a, name)?.atan()),
        "sinh" => V::Float(f1(&a, name)?.sinh()),
        "cosh" => V::Float(f1(&a, name)?.cosh()),
        "tanh" => V::Float(f1(&a, name)?.tanh()),
        "degrees" => V::Float(f1(&a, name)?.to_degrees()),
        "radians" => V::Float(f1(&a, name)?.to_radians()),
        "atan2" => {
            a.want(name, 2, 2)?;
            V::Float(to_float(&a.p[0])?.atan2(to_float(&a.p[1])?))
        }
        "hypot" => {
            let mut s2 = 0.0f64;
            let mut vals = vec![];
            for x in &a.p {
                vals.push(to_float(x)?);
            }
            if vals.len() == 2 {
                V::Float(vals[0].hypot(vals[1]))
            } else {
                for v in vals {
                    s2 += v * v;
                }
                V::Float(s2.sqrt())
            }
        }
        "dist" => {
            a.want(name, 2, 2)?;
            let (p, q) = (to_vec(&a.p[0])?, to_vec(&a.p[1])?);
            let mut s2 = 0.0;
            for (x, y) in p.iter().zip(q.iter()) {
                let d = to_float(x)? - to_float(y)?;
                s2 += d * d;
            }
            V::Float(s2.sqrt())
        }
        "isnan" => V::Bool(f1(&a, name)?.is_nan()),
        "isinf" => V::Bool(f1(&a, name)?.is_infinite()),
        "isfinite" => V::Bool(f1(&a, name)?.is_finite()),
        "copysign" => {
            a.want(name, 2, 2)?;
            V::Float(to_float(&a.p[0])?.copysign(to_float(&a.p[1])?))
        }
        "isclose" => {
            let mut a = a;
            let rel = a.kw("rel_tol").map(|x| to_float(&x)).transpose()?.unwrap_or(1e-9);
            let abs_ = a.kw("abs_tol").map(|x| to_float(&x)).transpose()?.unwrap_or(0.0);
            let (x, y) = (to_float(&a.p[0])?, to_float(&a.p[1])?);
            if x == y {
                return Ok(V::Bool(true));
            }
            if x.is_infinite() || y.is_infinite() {
                return Ok(V::Bool(false));
            }
            let d = (x - y).abs();
            V::Bool(d <= (rel * y.abs()).max(rel * x.abs()) || d <= abs_)
        }
        "fsum" => {
            // exact summation (Shewchuk), as CPython
            let mut partials: Vec<f64> = vec![];
            for v in to_vec(&a1(&a, name)?)? {
                let mut x = to_float(&v)?;
                let mut i = 0;
                for j in 0..partials.len() {
                    let mut y = partials[j];
                    if x.abs() < y.abs() {
                        std::mem::swap(&mut x, &mut y);
                    }
                    let hi = x + y;
                    let lo = y - (hi - x);
                    if lo != 0.0 {
                        partials[i] = lo;
                        i += 1;
                    }
                    x = hi;
                }
                partials.truncate(i);
                partials.push(x);
            }
            let mut hi = 0.0;
            if let Some(mut n) = partials.len().checked_sub(1).map(|x| x + 1) {
                hi = partials[n - 1];
                n -= 1;
                let mut lo = 0.0;
                while n > 0 {
                    let x = hi;
                    let y = partials[n - 1];
                    n -= 1;
                    hi = x + y;
                    let yr = hi - x;
                    lo = y - yr;
                    if lo != 0.0 {
                        break;
                    }
                }
                if n > 0 && ((lo < 0.0 && partials[n - 1] < 0.0) || (lo > 0.0 && partials[n - 1] > 0.0)) {
                    let y = lo * 2.0;
                    let x = hi + y;
                    let yr = x - hi;
                    if y == yr {
                        hi = x;
                    }
                }
            }
            V::Float(hi)
        }
        "prod" => {
            let mut a = a;
            let start = a.kw("start").unwrap_or(V::Int(1));
            let mut acc = start;
            for x in to_vec(&a1(&a, name)?)? {
                acc = binop(&acc, &x, "*")?;
            }
            acc
        }
        "modf" => {
            let x = f1(&a, name)?;
            tup(vec![V::Float(x.fract()), V::Float(x.trunc())])
        }
        "frexp" => {
            let x = f1(&a, name)?;
            let (m, e) = frexp(x);
            tup(vec![V::Float(m), V::Int(e as i64)])
        }
        "ldexp" => {
            a.want(name, 2, 2)?;
            V::Float(to_float(&a.p[0])? * 2f64.powi(idx_of(&a.p[1])? as i32))
        }
        "fmod" => {
            a.want(name, 2, 2)?;
            V::Float(to_float(&a.p[0])? % to_float(&a.p[1])?)
        }
        "remainder" => {
            a.want(name, 2, 2)?;
            let (x, y) = (to_float(&a.p[0])?, to_float(&a.p[1])?);
            V::Float(x - (x / y).round_ties_even() * y)
        }
        _ => return Err(unsupported(&format!("math.{name}"))),
    })
}

// heapq: CPython's sift functions, so equal items come out in the same order.
fn heap_list(v: &V) -> Result<Rc<RefCell<Vec<V>>>, E> {
    match v {
        V::List(l) => Ok(l.clone()),
        V::Obj(_) => match inner_of(v) {
            Some(V::List(l)) => Ok(l),
            _ => type_err("heap argument must be a list".into()),
        },
        x => type_err(format!("heap argument must be a list, not {}", tn(x))),
    }
}
fn siftdown(h: &mut Vec<V>, start: usize, mut pos: usize) -> Result<(), E> {
    let item = h[pos].clone();
    while pos > start {
        let parent = (pos - 1) >> 1;
        if lt(&item, &h[parent])? {
            h[pos] = h[parent].clone();
            pos = parent;
            continue;
        }
        break;
    }
    h[pos] = item;
    Ok(())
}
fn siftup(h: &mut Vec<V>, mut pos: usize) -> Result<(), E> {
    let end = h.len();
    let start = pos;
    let item = h[pos].clone();
    let mut child = 2 * pos + 1;
    while child < end {
        let right = child + 1;
        if right < end && !lt(&h[child], &h[right])? {
            child = right;
        }
        h[pos] = h[child].clone();
        pos = child;
        child = 2 * pos + 1;
    }
    h[pos] = item;
    siftdown(h, start, pos)
}
pub fn heapq_fn(name: &str, mut a: Args) -> R {
    Ok(match name {
        "heappush" => {
            a.want(name, 2, 2)?;
            let l = heap_list(&a.p[0])?;
            let mut h = l.borrow_mut();
            h.push(a.p[1].clone());
            let n = h.len() - 1;
            siftdown(&mut h, 0, n)?;
            V::None
        }
        "heappop" => {
            let l = heap_list(&a1(&a, name)?)?;
            let mut h = l.borrow_mut();
            let last = match h.pop() {
                Some(x) => x,
                None => return Err(exc("IndexError", "index out of range".into())),
            };
            if !h.is_empty() {
                let r = std::mem::replace(&mut h[0], last);
                siftup(&mut h, 0)?;
                r
            } else {
                last
            }
        }
        "heapify" => {
            let l = heap_list(&a1(&a, name)?)?;
            let mut h = l.borrow_mut();
            let n = h.len();
            for i in (0..n / 2).rev() {
                siftup(&mut h, i)?;
            }
            V::None
        }
        "heappushpop" => {
            a.want(name, 2, 2)?;
            let l = heap_list(&a.p[0])?;
            let mut h = l.borrow_mut();
            let mut item = a.p[1].clone();
            if !h.is_empty() && lt(&h[0], &item)? {
                item = std::mem::replace(&mut h[0], item);
                siftup(&mut h, 0)?;
            }
            item
        }
        "heapreplace" => {
            a.want(name, 2, 2)?;
            let l = heap_list(&a.p[0])?;
            let mut h = l.borrow_mut();
            if h.is_empty() {
                return Err(exc("IndexError", "index out of range".into()));
            }
            let r = std::mem::replace(&mut h[0], a.p[1].clone());
            siftup(&mut h, 0)?;
            r
        }
        "nlargest" | "nsmallest" => {
            let key = a.kw("key");
            a.want(name, 2, 2)?;
            let n = idx_of(&a.p[0])?.max(0) as usize;
            let mut v = to_vec(&a.p[1])?;
            sort_values(&mut v, key.as_ref(), name == "nlargest")?;
            v.truncate(n);
            list(v)
        }
        "merge" => {
            let key = a.kw("key");
            let rev = a.kw("reverse").map(|r| truth(&r)).transpose()?.unwrap_or(false);
            let mut v = vec![];
            for x in &a.p {
                v.extend(to_vec(x)?);
            }
            sort_values(&mut v, key.as_ref(), rev)?;
            vec_iter(v)
        }
        _ => return Err(unsupported(&format!("heapq.{name}"))),
    })
}
pub fn bisect_fn(name: &str, mut a: Args) -> R {
    let key = a.kw("key");
    let lo = a.kw("lo");
    let hi = a.kw("hi");
    a.want(name, 2, 4)?;
    let l = to_vec(&a.p[0])?;
    let x = a.p[1].clone();
    let mut lo = match a.p.get(2).cloned().or(lo) {
        Some(v) => idx_of(&v)? as usize,
        None => 0,
    };
    let mut hi = match a.p.get(3).cloned().or(hi) {
        Some(V::None) | None => l.len(),
        Some(v) => idx_of(&v)? as usize,
    };
    let kx = |v: &V| -> R {
        match &key {
            Some(k) if !matches!(k, V::None) => call(k, Args::p(vec![v.clone()])),
            _ => Ok(v.clone()),
        }
    };
    let right = name.ends_with("right") || name == "bisect" || name == "insort";
    let xk = if name.starts_with("insort") { kx(&x)? } else { x.clone() };
    while lo < hi {
        let mid = (lo + hi) / 2;
        let m = kx(&l[mid])?;
        let go_right = if right { !lt(&xk, &m)? } else { lt(&m, &xk)? };
        if go_right {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if name.starts_with("insort") {
        if let V::List(ll) = &a.p[0] {
            ll.borrow_mut().insert(lo, x);
        }
        return Ok(V::None);
    }
    Ok(V::Int(lo as i64))
}

fn lru_cache_wrap(f: V) -> V {
    let cache: Rc<RefCell<Dict>> = Rc::new(RefCell::new(Dict::new()));
    let name = match &f {
        V::Func(x) => x.name,
        _ => "wrapper",
    };
    let c2 = cache.clone();
    let w = func(name, move |a: Args| {
        let mut key = a.p.clone();
        for (k, v) in &a.k {
            key.push(s(k));
            key.push(v.clone());
        }
        let key = tup(key);
        if let Some(v) = cache.borrow().get(&key)? {
            return Ok(v);
        }
        let r = call(&f, a)?;
        cache.borrow_mut().set(key, r.clone())?;
        Ok(r)
    });
    if let V::Func(wf) = &w {
        let c3 = c2.clone();
        wf.attrs.borrow_mut().push(("cache_clear", builtin("cache_clear", move |_a: Args| {
            c3.borrow_mut().clear();
            Ok(V::None)
        })));
    }
    w
}
pub fn functools_fn(name: &str, mut a: Args) -> R {
    Ok(match name {
        "lru_cache" | "cache" => {
            // @lru_cache, @lru_cache(), @lru_cache(maxsize=None)
            match a.p.first() {
                Some(f @ (V::Func(_) | V::Bound(_))) => lru_cache_wrap(f.clone()),
                _ => builtin("lru_cache", |a: Args| Ok(lru_cache_wrap(a1(&a, "lru_cache")?))),
            }
        }
        "reduce" => {
            a.want(name, 2, 3)?;
            let f = a.p[0].clone();
            let mut it = iter(&a.p[1])?;
            let mut acc = match a.p.get(2) {
                Some(x) => x.clone(),
                None => match it.next()? {
                    Some(x) => x,
                    None => return type_err("reduce() of empty iterable with no initial value".into()),
                },
            };
            while let Some(x) = it.next()? {
                acc = call(&f, Args::p(vec![acc, x]))?;
            }
            acc
        }
        "cmp_to_key" => {
            let f = a1(&a, name)?;
            builtin("K", move |a: Args| {
                let x = a1(&a, "K")?;
                Ok(V::Ext(Rc::new(CmpKey { f: f.clone(), v: x })))
            })
        }
        "partial" => {
            if a.p.is_empty() {
                return type_err("partial needs a function".into());
            }
            let f = a.p.remove(0);
            let pre = a.p.clone();
            let kw = a.k.clone();
            func("partial", move |b: Args| {
                let mut p = pre.clone();
                p.extend(b.p);
                let mut k = kw.clone();
                for (n, v) in b.k {
                    k.retain(|(m, _)| *m != n);
                    k.push((n, v));
                }
                call(&f, Args { p, k })
            })
        }
        "wraps" => builtin("wraps", |a: Args| Ok(a1(&a, "wraps")?)),
        "total_ordering" => {
            let c = a1(&a, name)?;
            if let V::Class(k) = &c {
                total_ordering(k);
            }
            c
        }
        _ => return Err(unsupported(&format!("functools.{name}"))),
    })
}
fn total_ordering(k: &Rc<Class>) {
    let has = |n: &str| k.lookup(n).is_some() && !matches!(k.lookup(n), Some(V::Func(f)) if f.kind.get() == 3);
    if has("__lt__") {
        let lt_ = k.lookup("__lt__").unwrap();
        let l2 = lt_.clone();
        k.set("__gt__", func("__gt__", move |a: Args| {
            let r = call(&l2, Args::p(vec![a.p[0].clone(), a.p[1].clone()]))?;
            if matches!(r, V::NotImpl) {
                return Ok(r);
            }
            Ok(V::Bool(!truth(&r)? && !eq(&a.p[0], &a.p[1])?))
        }));
        let l3 = lt_.clone();
        k.set("__le__", func("__le__", move |a: Args| {
            let r = call(&l3, Args::p(vec![a.p[0].clone(), a.p[1].clone()]))?;
            if matches!(r, V::NotImpl) {
                return Ok(r);
            }
            Ok(V::Bool(truth(&r)? || eq(&a.p[0], &a.p[1])?))
        }));
        let l4 = lt_.clone();
        k.set("__ge__", func("__ge__", move |a: Args| {
            let r = call(&l4, Args::p(vec![a.p[0].clone(), a.p[1].clone()]))?;
            if matches!(r, V::NotImpl) {
                return Ok(r);
            }
            Ok(V::Bool(!truth(&r)?))
        }));
    }
}
pub struct CmpKey {
    pub f: V,
    pub v: V,
}
impl Ext for CmpKey {
    fn tname(&self) -> &'static str {
        "functools.KeyWrapper"
    }
    fn repr(&self) -> String {
        "<functools.KeyWrapper object>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        if let V::Ext(x) = o {
            if let Some(k) = x.any().downcast_ref::<CmpKey>() {
                let r = call(&self.f, Args::p(vec![self.v.clone(), k.v.clone()])).ok()?;
                let n = match r {
                    V::Int(i) => i,
                    V::Float(f) => {
                        if f < 0.0 {
                            -1
                        } else if f > 0.0 {
                            1
                        } else {
                            0
                        }
                    }
                    V::Bool(b) => b as i64,
                    _ => 0,
                };
                return Some(n.cmp(&0));
            }
        }
        None
    }
}

pub fn itertools_fn(name: &str, mut a: Args) -> R {
    Ok(match name {
        "permutations" => {
            let pool = to_vec(a.p.first().ok_or_else(|| exc("TypeError", "permutations() missing iterable".into()))?)?;
            let r = match a.p.get(1).cloned().or(a.kw("r")) {
                Some(V::None) | None => pool.len(),
                Some(x) => idx_of(&x)? as usize,
            };
            let n = pool.len();
            let mut out = vec![];
            if r <= n {
                let mut idx: Vec<usize> = (0..n).collect();
                let mut cyc: Vec<usize> = (n - r + 1..=n).rev().collect();
                out.push(tup(idx[..r].iter().map(|&i| pool[i].clone()).collect()));
                'outer: loop {
                    if n == 0 {
                        break;
                    }
                    let mut i = r;
                    loop {
                        if i == 0 {
                            break 'outer;
                        }
                        i -= 1;
                        cyc[i] -= 1;
                        if cyc[i] == 0 {
                            let x = idx.remove(i);
                            idx.push(x);
                            cyc[i] = n - i;
                        } else {
                            let j = cyc[i];
                            idx.swap(i, n - j);
                            out.push(tup(idx[..r].iter().map(|&k| pool[k].clone()).collect()));
                            break;
                        }
                    }
                }
            }
            vec_iter(out)
        }
        "combinations" | "combinations_with_replacement" => {
            a.want(name, 2, 2)?;
            let pool = to_vec(&a.p[0])?;
            let r = idx_of(&a.p[1])? as usize;
            let n = pool.len();
            let mut out = vec![];
            let rep = name != "combinations";
            if rep {
                if n > 0 || r == 0 {
                    let mut idx = vec![0usize; r];
                    out.push(tup(idx.iter().map(|&i| pool[i].clone()).collect()));
                    loop {
                        let mut i = r;
                        let mut found = false;
                        while i > 0 {
                            i -= 1;
                            if idx[i] != n - 1 {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            break;
                        }
                        let v = idx[i] + 1;
                        for j in i..r {
                            idx[j] = v;
                        }
                        out.push(tup(idx.iter().map(|&i| pool[i].clone()).collect()));
                    }
                }
            } else if r <= n {
                let mut idx: Vec<usize> = (0..r).collect();
                out.push(tup(idx.iter().map(|&i| pool[i].clone()).collect()));
                loop {
                    let mut i = r;
                    let mut found = false;
                    while i > 0 {
                        i -= 1;
                        if idx[i] != i + n - r {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        break;
                    }
                    idx[i] += 1;
                    for j in i + 1..r {
                        idx[j] = idx[j - 1] + 1;
                    }
                    out.push(tup(idx.iter().map(|&i| pool[i].clone()).collect()));
                }
            }
            vec_iter(out)
        }
        "product" => {
            let rep = match a.kw("repeat") {
                Some(x) => idx_of(&x)? as usize,
                None => 1,
            };
            let mut pools = vec![];
            for x in &a.p {
                pools.push(to_vec(x)?);
            }
            let pools: Vec<Vec<V>> = (0..rep).flat_map(|_| pools.clone()).collect();
            let mut res: Vec<Vec<V>> = vec![vec![]];
            for p in &pools {
                let mut nr = vec![];
                for r in &res {
                    for x in p {
                        let mut y = r.clone();
                        y.push(x.clone());
                        nr.push(y);
                    }
                }
                res = nr;
            }
            vec_iter(res.into_iter().map(tup).collect())
        }
        "chain" => {
            let mut out = vec![];
            for x in &a.p {
                out.extend(to_vec(x)?);
            }
            vec_iter(out)
        }
        "chain.from_iterable" => {
            let mut out = vec![];
            for x in to_vec(&a1(&a, "from_iterable")?)? {
                out.extend(to_vec(&x)?);
            }
            vec_iter(out)
        }
        "accumulate" => {
            let init = a.kw("initial");
            let f = a.p.get(1).cloned().or(a.kw("func"));
            let xs = to_vec(&a.p[0])?;
            let mut out = vec![];
            let mut acc: Option<V> = init.filter(|x| !matches!(x, V::None));
            if let Some(i) = &acc {
                out.push(i.clone());
            }
            for x in xs {
                acc = Some(match acc {
                    None => x,
                    Some(p) => match &f {
                        Some(f) if !matches!(f, V::None) => call(f, Args::p(vec![p, x]))?,
                        _ => add(&p, &x)?,
                    },
                });
                out.push(acc.clone().unwrap());
            }
            vec_iter(out)
        }
        "groupby" => {
            let key = a.p.get(1).cloned().or(a.kw("key")).filter(|k| !matches!(k, V::None));
            let xs = to_vec(&a.p[0])?;
            let mut out: Vec<V> = vec![];
            let mut cur: Option<(V, Vec<V>)> = None;
            for x in xs {
                let k = match &key {
                    Some(f) => call(f, Args::p(vec![x.clone()]))?,
                    None => x.clone(),
                };
                match &mut cur {
                    Some((ck, g)) if eq(ck, &k)? => g.push(x),
                    _ => {
                        if let Some((ck, g)) = cur.take() {
                            out.push(tup(vec![ck, vec_iter(g)]));
                        }
                        cur = Some((k, vec![x]));
                    }
                }
            }
            if let Some((ck, g)) = cur.take() {
                out.push(tup(vec![ck, vec_iter(g)]));
            }
            vec_iter(out)
        }
        "count" => {
            let mut n = a.p.first().cloned().unwrap_or(V::Int(0));
            let st0 = a.p.get(1).cloned().unwrap_or(V::Int(1));
            gen_iter(move || {
                let v = n.clone();
                n = add(&n, &st0)?;
                Ok(Some(v))
            })
        }
        "islice" => {
            let src = a.p.first().cloned().unwrap_or(V::None);
            let (start, stop, step) = match a.p.len() {
                2 => (0, a.p[1].clone(), 1),
                3 => (idx_of(&a.p[1]).unwrap_or(0), a.p[2].clone(), 1),
                _ => (idx_of(&a.p[1]).unwrap_or(0), a.p[2].clone(), idx_of(&a.p[3]).unwrap_or(1)),
            };
            let stop = match stop {
                V::None => i64::MAX,
                x => idx_of(&x)?,
            };
            let mut it = iter(&src)?;
            let mut i = 0i64;
            let mut next_i = start;
            gen_iter(move || loop {
                if i >= stop || next_i >= stop {
                    return Ok(None);
                }
                match it.next()? {
                    None => return Ok(None),
                    Some(x) => {
                        let cur = i;
                        i += 1;
                        if cur == next_i {
                            next_i += step;
                            return Ok(Some(x));
                        }
                    }
                }
            })
        }
        "zip_longest" => {
            let fill = a.kw("fillvalue").unwrap_or(V::None);
            let mut cols = vec![];
            for x in &a.p {
                cols.push(to_vec(x)?);
            }
            let n = cols.iter().map(|c| c.len()).max().unwrap_or(0);
            vec_iter((0..n).map(|i| tup(cols.iter().map(|c| c.get(i).cloned().unwrap_or(fill.clone())).collect())).collect())
        }
        "repeat" => {
            let x = a.p[0].clone();
            match a.p.get(1).cloned().or(a.kw("times")) {
                Some(n) => vec_iter(vec![x; idx_of(&n)?.max(0) as usize]),
                None => gen_iter(move || Ok(Some(x.clone()))),
            }
        }
        "cycle" => {
            let xs = to_vec(&a1(&a, name)?)?;
            let mut i = 0;
            gen_iter(move || {
                if xs.is_empty() {
                    return Ok(None);
                }
                let v = xs[i % xs.len()].clone();
                i += 1;
                Ok(Some(v))
            })
        }
        "starmap" => {
            let f = a.p[0].clone();
            let mut out = vec![];
            for x in to_vec(&a.p[1])? {
                out.push(call(&f, Args::p(to_vec(&x)?))?);
            }
            vec_iter(out)
        }
        "takewhile" | "dropwhile" => {
            let f = a.p[0].clone();
            let mut out = vec![];
            let mut dropping = true;
            for x in to_vec(&a.p[1])? {
                let t = truth(&call(&f, Args::p(vec![x.clone()]))?)?;
                if name == "takewhile" {
                    if !t {
                        break;
                    }
                    out.push(x);
                } else {
                    if dropping && t {
                        continue;
                    }
                    dropping = false;
                    out.push(x);
                }
            }
            vec_iter(out)
        }
        "pairwise" => {
            let xs = to_vec(&a1(&a, name)?)?;
            vec_iter(xs.windows(2).map(|w| tup(w.to_vec())).collect())
        }
        "compress" => {
            let xs = to_vec(&a.p[0])?;
            let sel = to_vec(&a.p[1])?;
            let mut out = vec![];
            for (x, s0) in xs.into_iter().zip(sel) {
                if truth(&s0)? {
                    out.push(x);
                }
            }
            vec_iter(out)
        }
        "filterfalse" => {
            let f = a.p[0].clone();
            let mut out = vec![];
            for x in to_vec(&a.p[1])? {
                let t = if matches!(f, V::None) { truth(&x)? } else { truth(&call(&f, Args::p(vec![x.clone()]))?)? };
                if !t {
                    out.push(x);
                }
            }
            vec_iter(out)
        }
        _ => return Err(unsupported(&format!("itertools.{name}"))),
    })
}
pub fn copy_fn(name: &str, a: Args) -> R {
    let x = a1(&a, name)?;
    if name == "deepcopy" {
        let mut memo: Vec<(usize, V)> = vec![];
        return deepcopy(&x, &mut memo);
    }
    shallow_copy(&x)
}
pub fn shallow_copy(x: &V) -> R {
    Ok(match x {
        V::List(l) => list(l.borrow().clone()),
        V::Dict(d) => V::Dict(Rc::new(RefCell::new(d.borrow().copy()))),
        V::Set(s0) => V::Set(Rc::new(RefCell::new(s0.borrow().copy()))),
        V::Deque(d) => V::Deque(Rc::new(RefCell::new(Deq { v: d.borrow().v.clone(), max: d.borrow().max }))),
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__copy__") {
                return call(&f, Args::p(vec![x.clone()]));
            }
            let n = Obj::new(o.cls.clone());
            *n.d.borrow_mut() = o.d.borrow().clone();
            *n.inner.borrow_mut() = match &*o.inner.borrow() {
                Some(i) => Some(shallow_copy(i)?),
                None => None,
            };
            V::Obj(n)
        }
        _ => x.clone(),
    })
}
fn ptr_of(x: &V) -> Option<usize> {
    Some(match x {
        V::List(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Dict(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Set(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Obj(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Tuple(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Deque(l) => Rc::as_ptr(l) as *const u8 as usize,
        _ => return None,
    })
}
pub fn deepcopy(x: &V, memo: &mut Vec<(usize, V)>) -> R {
    let p = ptr_of(x);
    if let Some(p) = p {
        if let Some((_, v)) = memo.iter().find(|(q, _)| *q == p) {
            return Ok(v.clone());
        }
    }
    Ok(match x {
        V::List(l) => {
            let n = Rc::new(RefCell::new(vec![]));
            memo.push((p.unwrap(), V::List(n.clone())));
            let items = l.borrow().clone();
            for y in items {
                let c = deepcopy(&y, memo)?;
                n.borrow_mut().push(c);
            }
            V::List(n)
        }
        V::Tuple(t) => {
            let mut v = vec![];
            for y in t.iter() {
                v.push(deepcopy(y, memo)?);
            }
            tup(v)
        }
        V::Dict(d) => {
            let n = Rc::new(RefCell::new(Dict::new()));
            {
                let src = d.borrow();
                let mut nn = n.borrow_mut();
                nn.kind = src.kind;
                nn.dflt = src.dflt.clone();
            }
            memo.push((p.unwrap(), V::Dict(n.clone())));
            let items = d.borrow().items();
            for (k, v) in items {
                let k = deepcopy(&k, memo)?;
                let v = deepcopy(&v, memo)?;
                n.borrow_mut().set(k, v)?;
            }
            V::Dict(n)
        }
        V::Set(s0) => {
            let mut n = PSet::new();
            for k in s0.borrow().keys() {
                n.add(deepcopy(&k, memo)?)?;
            }
            V::Set(Rc::new(RefCell::new(n)))
        }
        V::Deque(d) => {
            let mut v = VecDeque::new();
            let items: Vec<V> = d.borrow().v.iter().cloned().collect();
            for y in items {
                v.push_back(deepcopy(&y, memo)?);
            }
            V::Deque(Rc::new(RefCell::new(Deq { v, max: d.borrow().max })))
        }
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__deepcopy__") {
                return call(&f, Args::p(vec![x.clone(), dict_new()]));
            }
            if o.cls.base_bt() == BT::Exc {
                return Ok(x.clone());
            }
            let n = Obj::new(o.cls.clone());
            memo.push((p.unwrap(), V::Obj(n.clone())));
            let attrs = o.d.borrow().clone();
            for (k, v) in attrs {
                let c = deepcopy(&v, memo)?;
                n.set(k, c);
            }
            let inner = o.inner.borrow().clone();
            if let Some(i) = inner {
                *n.inner.borrow_mut() = Some(deepcopy(&i, memo)?);
            }
            V::Obj(n)
        }
        _ => x.clone(),
    })
}
pub fn operator_fn(name: &str, a: Args) -> R {
    match name {
        "itemgetter" => {
            let keys = a.p.clone();
            Ok(builtin("itemgetter", move |b: Args| {
                let x = a1(&b, "itemgetter")?;
                if keys.len() == 1 {
                    return getitem(&x, &keys[0]);
                }
                let mut v = vec![];
                for k in &keys {
                    v.push(getitem(&x, k)?);
                }
                Ok(tup(v))
            }))
        }
        "attrgetter" => {
            let names: Vec<String> = a.p.iter().map(|x| to_str(x).unwrap_or_default()).collect();
            Ok(builtin("attrgetter", move |b: Args| {
                let x = a1(&b, "attrgetter")?;
                let get = |n: &str| -> R {
                    let mut v = x.clone();
                    for part in n.split('.') {
                        v = getattr(&v, part)?;
                    }
                    Ok(v)
                };
                if names.len() == 1 {
                    return get(&names[0]);
                }
                let mut v = vec![];
                for n in &names {
                    v.push(get(n)?);
                }
                Ok(tup(v))
            }))
        }
        "methodcaller" => {
            let mut p = a.p.clone();
            let n = sym(&to_str(&p.remove(0))?);
            let k = a.k.clone();
            Ok(builtin("methodcaller", move |b: Args| call_method(&a1(&b, "methodcaller")?, n, Args { p: p.clone(), k: k.clone() })))
        }
        _ => {
            let op = match name {
                "add" | "__add__" => "+",
                "sub" => "-",
                "mul" => "*",
                "truediv" => "/",
                "floordiv" => "//",
                "mod" => "%",
                "pow" => "**",
                "and_" => "&",
                "or_" => "|",
                "xor" => "^",
                "lshift" => "<<",
                "rshift" => ">>",
                "lt" | "le" | "gt" | "ge" | "eq" | "ne" => {
                    a.want(name, 2, 2)?;
                    let o = match name {
                        "lt" => "<",
                        "le" => "<=",
                        "gt" => ">",
                        "ge" => ">=",
                        "eq" => "==",
                        _ => "!=",
                    };
                    return Ok(V::Bool(cmp_op(&a.p[0], &a.p[1], o)?));
                }
                "neg" => return neg(&a1(&a, name)?),
                "not_" => return not(&a1(&a, name)?),
                "truth" => return Ok(V::Bool(truth(&a1(&a, name)?)?)),
                "getitem" => {
                    a.want(name, 2, 2)?;
                    return getitem(&a.p[0], &a.p[1]);
                }
                "contains" => {
                    a.want(name, 2, 2)?;
                    return Ok(V::Bool(contains(&a.p[0], &a.p[1])?));
                }
                "index" => return to_int(&a1(&a, name)?),
                _ => return Err(unsupported(&format!("operator.{name}"))),
            };
            a.want(name, 2, 2)?;
            binop(&a.p[0], &a.p[1], op)
        }
    }
}
pub fn sys_fn(name: &str, a: Args) -> R {
    match name {
        "exit" => bi_exit(a),
        "setrecursionlimit" => {
            let n = idx_of(&a1(&a, name)?)?;
            REC_LIMIT.with(|l| l.set(n.max(1) as usize));
            Ok(V::None)
        }
        "getrecursionlimit" => Ok(V::Int(REC_LIMIT.with(|l| l.get()) as i64)),
        "set_int_max_str_digits" => Ok(V::None),
        "intern" => Ok(a1(&a, name)?),
        "getsizeof" => Ok(V::Int(64)),
        _ => Err(unsupported(&format!("sys.{name}"))),
    }
}
pub fn stream_fn(stream: &str, name: &str, a: Args) -> R {
    match (stream, name) {
        ("sys.stdin", "read") => Ok(st(read_all())),
        ("sys.stdin", "readline") => Ok(st(read_line().unwrap_or_default())),
        ("sys.stdin", "readlines") => {
            let mut v = vec![];
            while let Some(l) = read_line() {
                v.push(st(l));
            }
            Ok(list(v))
        }
        ("sys.stdin", "__iter__") => Ok(stdin_lines()),
        ("sys.stdin", "isatty") => Ok(V::Bool(false)),
        ("sys.stdin", "fileno") => Ok(V::Int(0)),
        ("sys.stdout", "write") => {
            let t = match &a1(&a, "write")? {
                V::Str(x) => x.s.clone(),
                x => return type_err(format!("write() argument must be str, not {}", tn(x))),
            };
            out_write(&t);
            Ok(V::Int(t.chars().count() as i64))
        }
        ("sys.stderr", "write") => {
            let t = to_str(&a1(&a, "write")?)?;
            err_write(&t);
            Ok(V::Int(t.chars().count() as i64))
        }
        ("sys.stdout", "flush") => {
            out_flush();
            Ok(V::None)
        }
        ("sys.stderr", "flush") => Ok(V::None),
        ("sys.stdout" | "sys.stderr", "writelines") => {
            for x in to_vec(&a1(&a, "writelines")?)? {
                let t = to_str(&x)?;
                if stream == "sys.stdout" {
                    out_write(&t);
                } else {
                    err_write(&t);
                }
            }
            Ok(V::None)
        }
        ("sys.stdout" | "sys.stderr", "isatty") => Ok(V::Bool(false)),
        _ => Err(unsupported(&format!("{stream}.{name}"))),
    }
}
fn modfn(m: &'static str, n: &str) -> V {
    let n: Sym = sym(n);
    builtin(n, move |a: Args| module_call(m, n, a))
}
pub fn module_call(m: &str, n: &str, a: Args) -> R {
    match m {
        "math" => math_fn(n, a),
        "heapq" => heapq_fn(n, a),
        "bisect" => bisect_fn(n, a),
        "functools" => functools_fn(n, a),
        "itertools" => itertools_fn(n, a),
        "copy" => copy_fn(n, a),
        "operator" => operator_fn(n, a),
        "sys" => sys_fn(n, a),
        "sys.stdin" | "sys.stdout" | "sys.stderr" => stream_fn(m, n, a),
        "collections" => match n {
            "defaultdict" => col_defaultdict(a),
            "Counter" => col_counter(a),
            "deque" => col_deque(a),
            "OrderedDict" => instantiate(&bclass("OrderedDict"), a),
            "namedtuple" => namedtuple(a),
            _ => Err(unsupported(&format!("collections.{n}"))),
        },
        "string" => Err(unsupported(&format!("string.{n}"))),
        "time" => match n {
            "time" | "perf_counter" | "monotonic" | "process_time" => Ok(V::Float(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0))),
            "sleep" => Ok(V::None),
            _ => Err(unsupported(&format!("time.{n}"))),
        },
        "typing" => Ok(a.p.first().cloned().unwrap_or(V::None)),
        "dataclasses" => dataclass_fn(n, a),
        "abc" => Ok(a.p.first().cloned().unwrap_or(V::None)),
        _ => ext_module_call(m, n, a),
    }
}
pub fn module_attr(m: &'static str, name: &str) -> R {
    let v = match (m, name) {
        ("sys", "stdin") => V::Module("sys.stdin"),
        ("sys", "stdout") => V::Module("sys.stdout"),
        ("sys", "stderr") => V::Module("sys.stderr"),
        ("sys", "argv") => list(ARGV.with(|a| a.borrow().iter().map(|x| s(x)).collect())),
        ("sys", "maxsize") => V::Int(i64::MAX),
        ("sys", "platform") => s("linux"),
        ("sys", "byteorder") => s("little"),
        ("sys", "version_info") => tup(vec![V::Int(3), V::Int(13), V::Int(0)]),
        ("sys", "version") => s("3.13.0 (brevi)"),
        ("sys", "float_info") => return Err(unsupported("sys.float_info")),
        ("sys", "modules") => dict_new(),
        ("sys", "path") => list(vec![]),
        ("sys.stdin", "buffer") | ("sys.stdout", "buffer") => return Err(unsupported("binary streams")),
        ("math", "pi") => V::Float(std::f64::consts::PI),
        ("math", "e") => V::Float(std::f64::consts::E),
        ("math", "tau") => V::Float(std::f64::consts::TAU),
        ("math", "inf") => V::Float(f64::INFINITY),
        ("math", "nan") => V::Float(f64::NAN),
        ("collections", "defaultdict" | "Counter" | "OrderedDict" | "deque") => V::Class(bclass(name)),
        ("string", "ascii_lowercase") => s("abcdefghijklmnopqrstuvwxyz"),
        ("string", "ascii_uppercase") => s("ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        ("string", "ascii_letters") => s("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        ("string", "digits") => s("0123456789"),
        ("string", "hexdigits") => s("0123456789abcdefABCDEF"),
        ("string", "octdigits") => s("01234567"),
        ("string", "punctuation") => s("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~"),
        ("string", "whitespace") => s(" \t\n\r\x0b\x0c"),
        ("string", "printable") => s("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c"),
        ("typing", _) => V::Class(bclass("object")),
        ("itertools", "chain") => {
            let f = modfn("itertools", "chain");
            if let V::Func(ff) = &f {
                ff.attrs.borrow_mut().push(("from_iterable", modfn("itertools", "chain.from_iterable")));
            }
            f
        }
        ("dataclasses", "MISSING") => V::None,
        ("abc", "ABC") => V::Class(abc_class()),
        _ => {
            if let Some(v) = ext_module_attr(m, name)? {
                return Ok(v);
            }
            modfn(m, name)
        }
    };
    Ok(v)
}
thread_local! {
    static ABC: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
fn abc_class() -> Rc<Class> {
    ABC.with(|a| {
        let mut a = a.borrow_mut();
        if a.is_none() {
            *a = Some(Class::new("ABC", "abc", vec![bclass("object")], BT::Other));
        }
        a.clone().unwrap()
    })
}

// collections.namedtuple: a class whose instances keep their fields in order.
pub fn namedtuple(mut a: Args) -> R {
    let defaults = a.kw("defaults");
    let _ = a.kw("rename");
    let _ = a.kw("module");
    a.no_kw("namedtuple")?;
    a.want("namedtuple", 2, 2)?;
    let name = sym(&to_str(&a.p[0])?);
    let fields: Vec<Sym> = match &a.p[1] {
        V::Str(x) => x.s.replace(',', " ").split_whitespace().map(sym).collect(),
        x => to_vec(x)?.iter().map(|f| sym(&to_str(f).unwrap_or_default())).collect(),
    };
    let dflt: Vec<V> = match defaults {
        Some(d) => to_vec(&d)?,
        None => vec![],
    };
    let c = Class::new(name, "__main__", vec![bclass("tuple")], BT::Tuple);
    *c.fields.borrow_mut() = fields.clone();
    let f2 = fields.clone();
    let cc = c.clone();
    c.set("__new__", func("__new__", move |mut a: Args| {
        a.p.remove(0);
        let mut vals: Vec<V> = vec![V::Unbound; f2.len()];
        let n = a.p.len();
        if n > f2.len() {
            return type_err(format!("{}() takes {} positional arguments but {} were given", cc.name, f2.len() + 1, n + 1));
        }
        for (i, v) in a.p.drain(..).enumerate() {
            vals[i] = v;
        }
        for (k, v) in a.k.drain(..) {
            match f2.iter().position(|f| *f == k) {
                Some(i) => vals[i] = v,
                None => return type_err(format!("{}() got an unexpected keyword argument '{}'", cc.name, k)),
            }
        }
        let nd = dflt.len();
        let nf = f2.len();
        for i in 0..nf {
            if matches!(vals[i], V::Unbound) {
                if i + nd >= nf {
                    vals[i] = dflt[i + nd - nf].clone();
                } else {
                    return type_err(format!("{}() missing 1 required positional argument: '{}'", cc.name, f2[i]));
                }
            }
        }
        let o = Obj::new(cc.clone());
        for (f, v) in f2.iter().zip(vals.iter()) {
            o.set(f, v.clone());
        }
        *o.inner.borrow_mut() = Some(tup(vals));
        Ok(V::Obj(o))
    }));
    let f3 = fields.clone();
    c.set("__repr__", func("__repr__", move |a: Args| {
        let me = &a.p[0];
        if let V::Obj(o) = me {
            let mut parts = vec![];
            for f in &f3 {
                parts.push(format!("{}={}", f, repr(&o.get(f).unwrap_or(V::None))?));
            }
            return Ok(st(format!("{}({})", o.cls.name, parts.join(", "))));
        }
        Ok(s(""))
    }));
    let f4 = fields.clone();
    c.set("_asdict", func("_asdict", move |a: Args| {
        let mut d = Dict::new();
        if let V::Obj(o) = &a.p[0] {
            for f in &f4 {
                d.set(s(f), o.get(f).unwrap_or(V::None))?;
            }
        }
        Ok(V::Dict(Rc::new(RefCell::new(d))))
    }));
    let f5 = fields.clone();
    c.set("_replace", func("_replace", move |mut a: Args| {
        let me = a.p.remove(0);
        if let V::Obj(o) = &me {
            let mut vals = vec![];
            for f in &f5 {
                vals.push(match a.k.iter().find(|(k, _)| k == f) {
                    Some((_, v)) => v.clone(),
                    None => o.get(f).unwrap_or(V::None),
                });
            }
            return instantiate(&o.cls, Args::p(vals));
        }
        Ok(V::None)
    }));
    c.set("_fields", tup(fields.iter().map(|f| s(f)).collect()));
    c.set("__eq__", func("__eq__", |a: Args| {
        let (x, y) = (inner_of(&a.p[0]).unwrap_or(a.p[0].clone()), inner_of(&a.p[1]).unwrap_or(a.p[1].clone()));
        Ok(V::Bool(eq(&x, &y)?))
    }));
    c.set("__hash__", func("__hash__", |a: Args| Ok(V::Int(py_hash(&inner_of(&a.p[0]).unwrap_or(V::None))?))));
    c.set("__lt__", func("__lt__", |a: Args| {
        let (x, y) = (inner_of(&a.p[0]).unwrap_or(a.p[0].clone()), inner_of(&a.p[1]).unwrap_or(a.p[1].clone()));
        Ok(V::Bool(lt(&x, &y)?))
    }));
    c.set("__gt__", func("__gt__", |a: Args| {
        let (x, y) = (inner_of(&a.p[0]).unwrap_or(a.p[0].clone()), inner_of(&a.p[1]).unwrap_or(a.p[1].clone()));
        Ok(V::Bool(lt(&y, &x)?))
    }));
    Ok(V::Class(c))
}

// @dataclass
pub fn dataclass_fn(n: &str, mut a: Args) -> R {
    match n {
        "dataclass" => {
            let order = a.kw("order").map(|x| truth(&x)).transpose()?.unwrap_or(false);
            let frozen = a.kw("frozen").map(|x| truth(&x)).transpose()?.unwrap_or(false);
            let eqf = a.kw("eq").map(|x| truth(&x)).transpose()?.unwrap_or(true);
            let _ = a.kw("slots");
            let _ = a.kw("kw_only");
            let _ = a.kw("repr");
            let _ = a.kw("unsafe_hash");
            match a.p.first() {
                Some(V::Class(c)) => {
                    make_dataclass(c, order, frozen, eqf)?;
                    Ok(a.p[0].clone())
                }
                _ => Ok(builtin("dataclass", move |a: Args| {
                    if let Some(V::Class(c)) = a.p.first() {
                        make_dataclass(c, order, frozen, eqf)?;
                    }
                    Ok(a.p.first().cloned().unwrap_or(V::None))
                })),
            }
        }
        "field" => {
            let df = a.kw("default_factory");
            let d = a.kw("default");
            let _ = a.kw("compare");
            let r = a.kw("repr");
            let _ = a.kw("init");
            let o = Obj::new(field_class());
            if let Some(r) = r {
                o.set("repr", r);
            }
            if let Some(f) = df {
                o.set("default_factory", f);
            }
            if let Some(d) = d {
                o.set("default", d);
            }
            Ok(V::Obj(o))
        }
        "asdict" => {
            let x = a1(&a, n)?;
            asdict(&x)
        }
        "astuple" => {
            let x = a1(&a, n)?;
            if let V::Obj(o) = &x {
                let fs = all_fields(&o.cls);
                return Ok(tup(fs.iter().map(|f| o.get(f).unwrap_or(V::None)).collect()));
            }
            type_err("astuple() should be called on dataclass instances".into())
        }
        "replace" => {
            let x = a.p.first().cloned().unwrap_or(V::None);
            if let V::Obj(o) = &x {
                let fs = all_fields(&o.cls);
                let mut k: Vec<(Sym, V)> = vec![];
                for f in fs {
                    let v = match a.k.iter().find(|(n, _)| *n == f) {
                        Some((_, v)) => v.clone(),
                        None => o.get(f).unwrap_or(V::None),
                    };
                    k.push((f, v));
                }
                return instantiate(&o.cls, Args { p: vec![], k });
            }
            type_err("replace() should be called on dataclass instances".into())
        }
        "fields" => Err(unsupported("dataclasses.fields")),
        _ => Err(unsupported(&format!("dataclasses.{n}"))),
    }
}
fn asdict(x: &V) -> R {
    match x {
        V::Obj(o) if o.cls.lookup("__dataclass_fields__").is_some() => {
            let mut d = Dict::new();
            for f in all_fields(&o.cls) {
                d.set(s(f), asdict(&o.get(f).unwrap_or(V::None))?)?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        V::List(l) => {
            let mut v = vec![];
            for y in l.borrow().iter() {
                v.push(asdict(y)?);
            }
            Ok(list(v))
        }
        V::Tuple(t) => {
            let mut v = vec![];
            for y in t.iter() {
                v.push(asdict(y)?);
            }
            Ok(tup(v))
        }
        V::Dict(d) => {
            let mut n = Dict::new();
            for (k, v) in d.borrow().items() {
                n.set(asdict(&k)?, asdict(&v)?)?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(n))))
        }
        _ => Ok(x.clone()),
    }
}
thread_local! {
    static FIELD: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
fn field_class() -> Rc<Class> {
    FIELD.with(|a| {
        let mut a = a.borrow_mut();
        if a.is_none() {
            *a = Some(Class::new("Field", "dataclasses", vec![bclass("object")], BT::Other));
        }
        a.clone().unwrap()
    })
}
fn all_fields(c: &Rc<Class>) -> Vec<Sym> {
    let mut out: Vec<Sym> = vec![];
    let mro = c.mro.borrow().clone();
    for k in mro.iter().rev() {
        for f in k.fields.borrow().iter() {
            if !out.contains(f) {
                out.push(f);
            }
        }
    }
    out
}
// The class body's annotated names are its fields (the compiler records them in Class.fields).
fn make_dataclass(c: &Rc<Class>, order: bool, frozen: bool, eqf: bool) -> Result<(), E> {
    c.set("__dataclass_fields__", V::Bool(true));
    let fields = all_fields(c);
    // defaults: class attributes with the field names
    let mut defs: Vec<(Sym, Option<V>, Option<V>)> = vec![];
    // field(repr=False): left out of __repr__, here and in subclasses (kept as a class attribute).
    let mut hidden: Vec<V> = match c.lookup("__brevi_norepr__") {
        Some(V::Tuple(t)) => t.as_ref().clone(),
        _ => vec![],
    };
    for f in &fields {
        if let Some(V::Obj(o)) = c.own(f) {
            if Rc::ptr_eq(&o.cls, &field_class()) && matches!(o.get("repr"), Some(r) if !truth(&r)?) {
                hidden.push(s(f));
            }
        }
    }
    c.set("__brevi_norepr__", tup(hidden.clone()));
    for f in &fields {
        let d = c.lookup(f);
        let (dv, df) = match &d {
            Some(V::Obj(o)) if Rc::ptr_eq(&o.cls, &field_class()) => (o.get("default"), o.get("default_factory")),
            Some(v) => (Some(v.clone()), None),
            None => (None, None),
        };
        defs.push((f, dv, df));
    }
    // a field set to field(...) in the class body is not a class attribute afterwards
    for (f, dv, _) in &defs {
        if let Some(V::Obj(o)) = c.own(f) {
            if Rc::ptr_eq(&o.cls, &field_class()) {
                c.attrs.borrow_mut().retain(|(k, _)| k != f);
                if let Some(d) = dv {
                    c.set(f, d.clone());
                }
            }
        }
    }
    let cname = c.name;
    if c.own("__init__").is_none() {
        let defs2 = defs.clone();
        let post = c.lookup("__post_init__").is_some();
        c.set("__init__", func("__init__", move |mut a: Args| {
            let me = a.p.remove(0);
            let o = match &me {
                V::Obj(o) => o.clone(),
                _ => return Ok(V::None),
            };
            let n = a.p.len();
            if n > defs2.len() {
                return type_err(format!("{}.__init__() takes {} positional arguments but {} were given", cname, defs2.len() + 1, n + 1));
            }
            let mut vals: Vec<Option<V>> = vec![None; defs2.len()];
            for (i, v) in a.p.drain(..).enumerate() {
                vals[i] = Some(v);
            }
            for (k, v) in a.k.drain(..) {
                match defs2.iter().position(|d| d.0 == k) {
                    Some(i) => vals[i] = Some(v),
                    None => return type_err(format!("{}.__init__() got an unexpected keyword argument '{}'", cname, k)),
                }
            }
            let mut missing = vec![];
            for (i, (f, dv, df)) in defs2.iter().enumerate() {
                let v = match vals[i].take() {
                    Some(v) => v,
                    None => match (dv, df) {
                        (_, Some(f)) => call(f, Args::p(vec![]))?,
                        (Some(d), None) => d.clone(),
                        _ => {
                            missing.push(format!("'{f}'"));
                            continue;
                        }
                    },
                };
                o.set(f, v);
            }
            if !missing.is_empty() {
                let list = if missing.len() == 1 { missing[0].clone() } else { format!("{} and {}", missing[..missing.len() - 1].join(", "), missing.last().unwrap()) };
                return type_err(format!("{}.__init__() missing {} required positional argument{}: {}", cname, missing.len(), if missing.len() == 1 { "" } else { "s" }, list));
            }
            if post {
                call_method(&me, "__post_init__", Args::p(vec![]))?;
            }
            Ok(V::None)
        }));
    }
    if c.own("__repr__").is_none() {
        let fs: Vec<Sym> = fields.iter().copied().filter(|f| !hidden.iter().any(|h| matches!(h, V::Str(x) if x.s == *f))).collect();
        c.set("__repr__", func("__repr__", move |a: Args| {
            if let V::Obj(o) = &a.p[0] {
                let mut parts = vec![];
                for f in &fs {
                    parts.push(format!("{}={}", f, repr(&o.get(f).unwrap_or(V::None))?));
                }
                return Ok(st(format!("{}({})", o.cls.name, parts.join(", "))));
            }
            Ok(s(""))
        }));
    }
    let key = |fs: &Vec<Sym>, v: &V| -> V {
        if let V::Obj(o) = v {
            return tup(fs.iter().map(|f| o.get(f).unwrap_or(V::None)).collect());
        }
        V::None
    };
    if eqf && c.own("__eq__").is_none() {
        let fs = fields.clone();
        c.set("__eq__", func("__eq__", move |a: Args| {
            match (&a.p[0], &a.p[1]) {
                (V::Obj(x), V::Obj(y)) if Rc::ptr_eq(&x.cls, &y.cls) => Ok(V::Bool(eq(&key(&fs, &a.p[0]), &key(&fs, &a.p[1]))?)),
                _ => Ok(V::NotImpl),
            }
        }));
        if !frozen {
            c.set("__hash__", V::None);
        }
    }
    if frozen {
        let fs = fields.clone();
        c.set("__hash__", func("__hash__", move |a: Args| Ok(V::Int(py_hash(&key(&fs, &a.p[0]))?))));
        c.set("__setattr__", func("__setattr__", move |a: Args| {
            // allowed while __init__ runs: fields not set yet
            if let (V::Obj(o), V::Str(n)) = (&a.p[0], &a.p[1]) {
                if o.get(&n.s).is_none() {
                    o.set(sym(&n.s), a.p[2].clone());
                    return Ok(V::None);
                }
                return Err(exc("AttributeError", format!("cannot assign to field '{}'", n.s)));
            }
            Ok(V::None)
        }));
    }
    if order {
        for (nm, op) in [("__lt__", "<"), ("__le__", "<="), ("__gt__", ">"), ("__ge__", ">=")] {
            let fs = fields.clone();
            c.set(nm, func(nm, move |a: Args| match (&a.p[0], &a.p[1]) {
                (V::Obj(x), V::Obj(y)) if Rc::ptr_eq(&x.cls, &y.cls) => Ok(V::Bool(rich(&key(&fs, &a.p[0]), &key(&fs, &a.p[1]), op)?)),
                _ => Ok(V::NotImpl),
            }));
        }
    }
    Ok(())
}

// Library modules written elsewhere (re, json, decimal, ...) register here.
pub fn ext_module_call(m: &str, n: &str, a: Args) -> R {
    if matches!(m, "decimal" | "fractions" | "datetime" | "csv" | "html" | "statistics" | "threading") {
        return ext_module_call2(m, n, a).unwrap();
    }
    match m {
        "re" => re_fn(n, a),
        "json" | "json.decoder" if n == "JSONDecoder" => {
            if !a.p.is_empty() || !a.k.is_empty() {
                return Err(unsupported("json.JSONDecoder options"));
            }
            Ok(V::Obj(Obj::new(json_decoder_class())))
        }
        "json.decoder" if n == "scanstring" => json_scanstring(a),
        "json" => json_fn(n, a),
        "enum" => enum_fn(n, a),
        "_brevi_mt" => brevi_mt_fn(n, a),
        "_brevi_term" if n == "columns" => Ok(V::Int(term_columns())),
        _ => Err(unsupported(&format!("{m}.{n}"))),
    }
}
pub fn ext_module_attr(m: &str, n: &str) -> Result<Option<V>, E> {
    if let Some(v) = ext_module_attr2(m, n) {
        return Ok(Some(v));
    }
    Ok(match (m, n) {
        ("re", "IGNORECASE" | "I") => Some(V::Int(2)),
        ("re", "MULTILINE" | "M") => Some(V::Int(8)),
        ("re", "DOTALL" | "S") => Some(V::Int(16)),
        ("re", "VERBOSE" | "X") => Some(V::Int(64)),
        ("re", "ASCII" | "A") => Some(V::Int(256)),
        ("re", "error") => Some(V::Class(exc_class("error"))),
        ("json" | "json.decoder", "JSONDecodeError") => Some(V::Class(exc_class("JSONDecodeError"))),
        ("enum", "Enum") => Some(V::Class(enum_base())),
        _ => None,
    })
}

// ---------- program entry ----------

pub fn exc_line(e: &V) -> String {
    match e {
        V::Obj(o) => {
            let msg = to_str(e).unwrap_or_default();
            let name = if o.cls.module == "builtins" || o.cls.module == "__main__" { o.cls.name.to_string() } else { format!("{}.{}", o.cls.module, o.cls.name) };
            if msg.is_empty() {
                name
            } else {
                format!("{name}: {msg}")
            }
        }
        x => repr(x).unwrap_or_default(),
    }
}
pub fn run_main(f: fn() -> Result<(), E>) {
    let child = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        init_builtin_classes();
        ARGV.with(|a| *a.borrow_mut() = std::env::args().collect());
        let r = f();
        let code = match r {
            Ok(()) => 0,
            Err(E(e)) => {
                if is_exc(&e, "SystemExit") {
                    let code = match &e {
                        // `raise SystemExit(3)` sets only args; sys.exit(3) sets code.
                        V::Obj(o) => o.get("code").or_else(|| match o.get("args") {
                            Some(V::Tuple(t)) if t.len() == 1 => Some(t[0].clone()),
                            _ => None,
                        }).unwrap_or(V::None),
                        _ => V::None,
                    };
                    match code {
                        V::None => 0,
                        V::Int(i) => i as i32,
                        V::Bool(b) => b as i32,
                        other => {
                            out_flush();
                            err_write(&format!("{}\n", to_str(&other).unwrap_or_default()));
                            1
                        }
                    }
                } else {
                    out_flush();
                    err_write(&format!("Traceback (most recent call last):\n{}\n", exc_line(&e)));
                    1
                }
            }
        };
        out_flush();
        code
    });
    let code = child.unwrap().join().unwrap_or(1);
    std::process::exit(code);
}

// shutil.get_terminal_size().columns, for Brevi's copy of argparse. The runtime reads no environment (caps.rs
// tests it, for --sandbox), so the program sets this outside the sandbox (dynpy::to_rust adds the reader);
// under it, or unset, help text is 80 columns wide.
pub static TERM_COLS: std::sync::OnceLock<fn() -> i64> = std::sync::OnceLock::new();
fn term_columns() -> i64 {
    TERM_COLS.get().map_or(80, |f| f())
}
