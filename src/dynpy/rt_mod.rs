
// ---------- library modules ----------

fn f1(a: &Args, f: &str) -> Result<f64, E> {
    a.want(f, 1, 1)?;
    to_float(&a.p[0]).map_err(|_| exc("TypeError", format!("must be real number, not {}", tn(&a.p[0]))))
}
fn math_dom<T>() -> Result<T, E> {
    val_err("math domain error".into())
}
// math.nan is one object for the whole run, so every read of it carries the same NaN payload.
fn math_nan() -> f64 {
    static N: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *N.get_or_init(nan_obj)
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
    let v = match name {
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
            if x.is_infinite() {
                // CPython: fractional part is a signed zero, integer part is x itself
                tup(vec![V::Float(0.0f64.copysign(x)), V::Float(x)])
            } else {
                let ip = x.trunc();
                let mut fp = x - ip;
                if fp == 0.0 {
                    // keep the sign of x (modf(-0.0) and modf(-2.0) give -0.0)
                    fp = fp.copysign(x);
                }
                tup(vec![V::Float(fp), V::Float(ip)])
            }
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
    };
    // A computed float result is a new object in CPython: a NaN it makes (math.fabs(nan), math.sqrt(nan),
    // math.fmod(nan, 1.0), ...) is not the NaN passed in.
    Ok(match v {
        V::Float(r) => V::Float(fresh_float(r)),
        x => x,
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

// heapq.merge: one live entry per input iterator, merged in the order of the values the
// inputs produce. As in CPython the inputs are not sorted here; equal values keep the
// input order, and reverse flips the value order but not that tie-break.
struct MergeEntry {
    v: V,
    k: V,
    ord: i64,
    it: It,
}
fn merge_lt(a: &MergeEntry, b: &MergeEntry, rev: bool) -> Result<bool, E> {
    // CPython's heap entries are [value, order*direction] tuples; tuple
    // comparison moves on to the order only when the values are equal, so
    // unordered values (NaN, say) compare as not-less rather than by order.
    if eq(&a.k, &b.k)? {
        return Ok(if rev { a.ord > b.ord } else { a.ord < b.ord });
    }
    lt(&a.k, &b.k)
}
fn take_merge_entry(h: &mut Vec<MergeEntry>, pos: usize) -> MergeEntry {
    std::mem::replace(&mut h[pos], MergeEntry { v: V::None, k: V::None, ord: 0, it: It::Done })
}
// StopIteration is how iterators signal their end here as well.
fn merge_next(it: &mut It) -> Result<Option<V>, E> {
    match it.next() {
        Ok(v) => Ok(v),
        Err(E(e)) if is_exc(&e, "StopIteration") => Ok(None),
        Err(e) => Err(e),
    }
}
fn merge_key(f: &V, v: &V) -> Result<Option<V>, E> {
    match call(f, Args::p(vec![v.clone()])) {
        Ok(k) => Ok(Some(k)),
        // merge catches StopIteration from key() and treats that input as done.
        Err(E(e)) if is_exc(&e, "StopIteration") => Ok(None),
        Err(e) => Err(e),
    }
}
// heapq._siftdown and its max-heap variant: bubble newitem toward the root.
fn m_siftdown(h: &mut Vec<MergeEntry>, start: usize, mut pos: usize, rev: bool) -> Result<(), E> {
    let item = take_merge_entry(h, pos);
    while pos > start {
        let parent = (pos - 1) >> 1;
        let up = if rev { merge_lt(&h[parent], &item, rev)? } else { merge_lt(&item, &h[parent], rev)? };
        if up {
            h[pos] = take_merge_entry(h, parent);
            pos = parent;
            continue;
        }
        break;
    }
    h[pos] = item;
    Ok(())
}
// heapq._siftup and its max-heap variant: bubble the selected child up to a leaf.
fn m_siftup(h: &mut Vec<MergeEntry>, mut pos: usize, rev: bool) -> Result<(), E> {
    let end = h.len();
    let start = pos;
    let item = take_merge_entry(h, pos);
    let mut child = 2 * pos + 1;
    while child < end {
        let right = child + 1;
        if right < end {
            let pick_right = if rev { !merge_lt(&h[right], &h[child], rev)? } else { !merge_lt(&h[child], &h[right], rev)? };
            if pick_right {
                child = right;
            }
        }
        h[pos] = take_merge_entry(h, child);
        pos = child;
        child = 2 * pos + 1;
    }
    h[pos] = item;
    m_siftdown(h, start, pos, rev)
}
struct MergeState {
    inputs: Vec<V>,
    heap: Vec<MergeEntry>,
    started: bool,
    hole: bool,
    tail: bool,
    revv: V,
    rev: Option<bool>,
}
impl MergeState {
    fn next(&mut self, key: Option<&V>) -> Result<Option<V>, E> {
        if !self.started {
            self.started = true;
            let rev = match self.rev {
                Some(r) => r,
                None => {
                    let r = truth(&self.revv)?;
                    self.rev = Some(r);
                    r
                }
            };
            for (ord, x) in self.inputs.drain(..).enumerate() {
                let mut it = iter(&x)?;
                if let Some(v) = merge_next(&mut it)? {
                    let k = match key {
                        Some(f) => match merge_key(f, &v)? {
                            Some(k) => k,
                            None => continue,
                        },
                        None => v.clone(),
                    };
                    self.heap.push(MergeEntry { v, k, ord: ord as i64, it });
                }
            }
            for i in (0..self.heap.len() / 2).rev() {
                m_siftup(&mut self.heap, i, rev)?;
            }
        }
        let rev = self.rev.unwrap();
        loop {
            if self.heap.is_empty() {
                return Ok(None);
            }
            if self.hole {
                // CPython's last-input fast path is `yield from next.__self__`, which
                // iterates the iterator object again rather than calling its __next__
                // directly, so rewrap an object iterator once the merge is down to it.
                if self.heap.len() == 1 && !self.tail {
                    self.tail = true;
                    let rewrapped = match &self.heap[0].it {
                        It::Obj(o) => Some(iter(o)?),
                        _ => None,
                    };
                    if let Some(it) = rewrapped {
                        self.heap[0].it = it;
                    }
                }
                let nxt = { let e = &mut self.heap[0]; merge_next(&mut e.it)? };
                match nxt {
                    Some(v) => {
                        // CPython stops applying key once one iterator is left and
                        // streams the rest of it straight through.
                        let kv = if self.heap.len() > 1 {
                            match key {
                                Some(f) => match merge_key(f, &v)? {
                                    Some(k) => Some((v, k)),
                                    None => None,
                                },
                                None => {
                                    let k = v.clone();
                                    Some((v, k))
                                }
                            }
                        } else {
                            Some((v, V::None))
                        };
                        match kv {
                            Some((v, k)) => {
                                self.heap[0].v = v;
                                self.heap[0].k = k;
                                self.hole = false;
                                if self.heap.len() > 1 {
                                    m_siftup(&mut self.heap, 0, rev)?;
                                }
                            }
                            None => {
                                self.heap.swap_remove(0);
                                self.hole = false;
                                if self.heap.len() > 1 {
                                    m_siftup(&mut self.heap, 0, rev)?;
                                }
                                continue;
                            }
                        }
                    }
                    None => {
                        self.heap.swap_remove(0);
                        self.hole = false;
                        if self.heap.len() > 1 {
                            m_siftup(&mut self.heap, 0, rev)?;
                        }
                        continue;
                    }
                }
            }
            let out = std::mem::replace(&mut self.heap[0].v, V::None);
            self.hole = true;
            return Ok(Some(out));
        }
    }
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
            let key = a.kw("key").filter(|k| !matches!(k, V::None));
            let revv = a.kw("reverse").unwrap_or(V::None);
            a.no_kw("merge")?;
            let mut st = MergeState { inputs: std::mem::take(&mut a.p), heap: vec![], started: false, hole: false, tail: false, revv, rev: None };
            genexp_iter("merge", move || st.next(key.as_ref()))
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

// a bounded lru_cache's results by slot: (key, value, last use), and the slots by last use
struct LruSlots {
    tick: u64,
    map: std::collections::HashMap<i64, (V, V, u64)>,
    order: std::collections::BTreeMap<u64, i64>,
}
fn lru_cache_wrap(f: V, maxsize: V, typed: bool) -> R {
    let cache: Rc<RefCell<Dict>> = Rc::new(RefCell::new(Dict::new()));
    let hits: Rc<Cell<i64>> = Rc::new(Cell::new(0));
    let misses: Rc<Cell<i64>> = Rc::new(Cell::new(0));
    // CPython's update_wrapper copies __name__ and __qualname__ from the wrapped function.
    let (name, qualname) = match &f {
        V::Func(x) => (x.name.get(), x.qualname.get()),
        _ => ("wrapper", "wrapper"),
    };
    let slots: Rc<RefCell<LruSlots>> = Rc::new(RefCell::new(LruSlots { tick: 0, map: std::collections::HashMap::new(), order: std::collections::BTreeMap::new() }));
    let s3 = slots.clone();
    let c2 = cache.clone();
    let h2 = hits.clone();
    let m2 = misses.clone();
    let f2 = f.clone();
    // CPython's bound: None is unbounded, 0 or less caches nothing, n keeps the n most recently used results
    let bound: Option<usize> = match &maxsize {
        V::None => None,
        V::Int(n) => Some((*n).max(0) as usize),
        V::Bool(b) => Some(*b as usize),
        x => return type_err(format!("Expected first argument to be an integer, a callable, or None, not '{}'", tn(x))),
    };
    let maxsize = match bound {
        Some(n) if matches!(maxsize, V::Int(m) if m < 0) => V::Int(n as i64),
        _ => maxsize,
    };
    let w = func(name, move |a: Args| {
        // positional and keyword arguments in separate tuples, so f(1, "k", 2) and f(1, k=2) are different calls;
        // typed=True also keys on each argument's type, so f(1) and f(1.0) are cached apart
        let mut kw = vec![];
        for (k, v) in &a.k {
            kw.push(s(k));
            kw.push(v.clone());
        }
        let mut parts = vec![tup(a.p.clone()), tup(kw)];
        if typed {
            let mut types = vec![];
            for x in a.p.iter().chain(a.k.iter().map(|(_, v)| v)) {
                types.push(V::Class(type_of(x)?));
            }
            parts.push(tup(types));
        }
        let key = tup(parts);
        // A bounded cache's dict maps each key to a slot number; the slots and their recency live outside the
        // dict, so a hit (which only moves a result to most recently used) never changes the dict, as in
        // CPython, where the order is a linked list: a key's __eq__ may call the cached function again.
        if bound != Some(0) {
            if let Some(v) = Dict::get_cell(&cache, &key)? {
                h2.set(h2.get() + 1);
                if bound.is_none() {
                    return Ok(v);
                }
                let V::Int(id) = v else { unreachable!() };
                let mut sl = slots.borrow_mut();
                let t = sl.tick + 1;
                sl.tick = t;
                let (val, old) = match sl.map.get_mut(&id) {
                    Some(e) => {
                        let old = e.2;
                        e.2 = t;
                        (e.1.clone(), old)
                    }
                    None => unreachable!(),
                };
                sl.order.remove(&old);
                sl.order.insert(t, id);
                return Ok(val);
            }
        }
        m2.set(m2.get() + 1);
        let r = call(&f2, a)?;
        match bound {
            Some(0) => {}
            Some(n) => {
                if Dict::get_cell(&cache, &key)?.is_none() {
                    let id = {
                        let mut sl = slots.borrow_mut();
                        sl.tick += 1;
                        let t = sl.tick;
                        sl.map.insert(t as i64, (key.clone(), r.clone(), t));
                        sl.order.insert(t, t as i64);
                        t as i64
                    };
                    Dict::set_cell(&cache, key, V::Int(id))?;
                    while cache.borrow().n > n {
                        let oldest = {
                            let mut sl = slots.borrow_mut();
                            let first = sl.order.iter().next().map(|(t, id)| (*t, *id));
                            first.and_then(|(t, id)| {
                                sl.order.remove(&t);
                                sl.map.remove(&id).map(|e| e.0)
                            })
                        };
                        match oldest {
                            Some(k) => {
                                Dict::del_cell(&cache, &k)?;
                            }
                            None => break,
                        }
                    }
                }
            }
            None => Dict::set_cell(&cache, key, r.clone())?,
        }
        Ok(r)
    });
    if let V::Func(wf) = &w {
        let c3 = c2.clone();
        let h3 = hits.clone();
        let m3 = misses.clone();
        wf.attrs.borrow_mut().push(("cache_clear", builtin("cache_clear", move |_a: Args| {
            c3.borrow_mut().clear();
            let mut sl = s3.borrow_mut();
            sl.map.clear();
            sl.order.clear();
            h3.set(0);
            m3.set(0);
            Ok(V::None)
        })));
        let c4 = c2.clone();
        let h4 = hits.clone();
        let m4 = misses.clone();
        let ms4 = maxsize.clone();
        let info = cache_info_class()?;
        wf.attrs.borrow_mut().push(("cache_info", builtin("cache_info", move |_a: Args| {
            call(&info, Args::p(vec![V::Int(h4.get()), V::Int(m4.get()), ms4.clone(), V::Int(c4.borrow().n as i64)]))
        })));
        let ms5 = maxsize.clone();
        wf.attrs.borrow_mut().push(("cache_parameters", func("cache_parameters", move |_a: Args| {
            let mut d = Dict::new();
            d.set(s("maxsize"), ms5.clone())?;
            d.set(s("typed"), V::Bool(typed))?;
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        })));
    }
    let w = with_qualname(w, qualname);
    let w = update_wrapper(w, f, wraps_assigned(), wraps_updated())?;
    Ok(w)
}
// functools._CacheInfo: one namedtuple class for every cache's cache_info(), as CPython's module has.
fn cache_info_class() -> R {
    thread_local! {
        static CI: RefCell<Option<V>> = const { RefCell::new(None) };
    }
    if let Some(c) = CI.with(|c| c.borrow().clone()) {
        return Ok(c);
    }
    let c = namedtuple_runtime_type(Args { p: vec![s("CacheInfo"), tup(vec![s("hits"), s("misses"), s("maxsize"), s("currsize")])], k: vec![("module", s("functools"))] })?;
    CI.with(|x| *x.borrow_mut() = Some(c.clone()));
    Ok(c)
}
// functools.wraps' defaults, shared with update_wrapper (CPython 3.13's WRAPPER_ASSIGNMENTS/UPDATES).
fn wraps_assigned() -> V {
    tup(vec![s("__module__"), s("__name__"), s("__qualname__"), s("__doc__"), s("__annotations__"), s("__type_params__")])
}
fn wraps_updated() -> V {
    tup(vec![s("__dict__")])
}
fn wraps_name(x: &V) -> Result<Sym, E> {
    match x {
        V::Str(s) => Ok(sym(&s.s)),
        _ => type_err(format!("attribute name must be string, not '{}'", tn(x))),
    }
}
// functools.update_wrapper: copy the `assigned` attributes from `wrapped` onto `wrapper`, merge the `updated`
// attributes, and point __wrapped__ at `wrapped`, exactly as CPython 3.13 does.
fn update_wrapper(wrapper: V, wrapped: V, assigned: V, updated: V) -> R {
    let mut it = iter(&assigned)?;
    while let Some(x) = it.next()? {
        let n = wraps_name(&x)?;
        // annotations Brevi doesn't keep: the wrapper's stay unkept too (reading either is refused)
        if n == "__annotations__" {
            if let V::Func(wd) = &wrapped {
                if wd.attrs.borrow().iter().all(|(k, _)| *k != "__annotations__") {
                    continue;
                }
            }
        }
        match getattr(&wrapped, &n) {
            Ok(v) => setattr(&wrapper, sym(&n), v)?,
            Err(E(e)) if is_exc(&e, "AttributeError") => {}
            Err(e) => return Err(e),
        }
    }
    let mut it = iter(&updated)?;
    while let Some(x) = it.next()? {
        let n = wraps_name(&x)?;
        // A function's __dict__ is its assigned attributes; update it directly (for any wrapped object with
        // one: another function, a class, a callable instance, a partial, an lru_cache wrapper).
        if n == "__dict__" && matches!(&wrapper, V::Func(_)) {
            let other = match getattr(&wrapped, &n) {
                Ok(v) => v,
                Err(E(e)) if is_exc(&e, "AttributeError") => dict_new(),
                Err(e) => return Err(e),
            };
            for (k, v) in dict_items_of(&other)? {
                if let V::Str(ks) = &k {
                    setattr(&wrapper, sym(&ks.s), v)?;
                }
            }
            continue;
        }
        let d = getattr(&wrapper, &n)?;
        let other = match getattr(&wrapped, &n) {
            Ok(v) => v,
            Err(E(e)) if is_exc(&e, "AttributeError") => dict_new(),
            Err(e) => return Err(e),
        };
        call_method(&d, "update", Args::p(vec![other]))?;
    }
    setattr(&wrapper, sym("__wrapped__"), wrapped)?;
    Ok(wrapper)
}
// ---------- functools.singledispatch / singledispatchmethod ----------
// CPython 3.13's pure-Python implementation, over the dynamic runtime's values. The registry is a dict of
// type keys (exposed read-only as its mappingproxy view); the dispatch cache keys by the concrete class, as
// CPython's WeakKeyDictionary does. A def decorated with a bare `@register` carries its first annotation as a
// private descriptor made by the compiler (emit.rs sd_ann_desc): only that descriptor is evaluated, never the
// annotation itself.
struct SDState {
    registry: Rc<RefCell<Dict>>,
    // keyed by the class's address; each entry holds its class, so the address stays that class's while cached
    cache: RefCell<std::collections::HashMap<usize, (V, Option<V>)>>,
}
fn sd_registry_view(st: &Rc<SDState>) -> V {
    V::View(Rc::new((V::Dict(st.registry.clone()), 3)))
}
fn sd_invalid_first(v: &V) -> E {
    let r = repr(v).unwrap_or_else(|_| "<object>".into());
    exc("TypeError", format!("Invalid first argument to `register()`. {r} is not a class or union type."))
}
fn sd_invalid_ann(arg: &str, rep: &str) -> E {
    exc("TypeError", format!("Invalid annotation for '{arg}'. {rep} is not a class."))
}
fn sd_invalid_ann_union(arg: &str, rep: &str) -> E {
    exc("TypeError", format!("Invalid annotation for '{arg}'. {rep} not all arguments are classes."))
}
fn sd_missing_ann(f: &V) -> E {
    let r = repr(f).unwrap_or_else(|_| "<object>".into());
    exc("TypeError", format!("Invalid first argument to `register()`: {r}. Use either `@register(some_class)` or plain `@register` on an annotated function."))
}
fn sd_desc_at(desc: &V, i: usize) -> Option<V> {
    match desc {
        V::Tuple(t) => t.get(i).cloned(),
        _ => None,
    }
}
// The classes an annotation descriptor names. `union_repr` is the whole union's repr, for its error message.
fn sd_ann_classes(arg: &str, desc: &V, union_repr: Option<String>) -> Result<Vec<V>, E> {
    let tag = match desc {
        V::Tuple(t) if !t.is_empty() => to_str(&t[0]).unwrap_or_default(),
        _ => return Err(unsupported("a singledispatch annotation")),
    };
    match tag.as_str() {
        "type" => {
            let thunk = sd_desc_at(desc, 1).ok_or_else(|| unsupported("a singledispatch annotation"))?;
            let v = call(&thunk, Args::p(vec![]))?;
            if matches!(v, V::Class(_)) {
                Ok(vec![v])
            } else if let Some(full) = union_repr {
                Err(sd_invalid_ann_union(arg, &full))
            } else {
                Err(sd_invalid_ann(arg, &repr(&v).unwrap_or_else(|_| "<object>".into())))
            }
        }
        "none" => Ok(vec![V::Class(type_of(&V::None)?)]),
        "union" => {
            let arms = match sd_desc_at(desc, 1) {
                Some(V::Tuple(t)) => t.to_vec(),
                _ => return Err(unsupported("a singledispatch union annotation")),
            };
            let full = sd_desc_at(desc, 2).map(|v| to_str(&v).unwrap_or_default()).unwrap_or_default();
            let mut out = vec![];
            for arm in arms {
                out.extend(sd_ann_classes(arg, &arm, Some(full.clone()))?);
            }
            Ok(out)
        }
        "invalid" => {
            let rep = sd_desc_at(desc, 1).map(|v| to_str(&v).unwrap_or_default()).unwrap_or_default();
            if let Some(full) = union_repr {
                Err(sd_invalid_ann_union(arg, &full))
            } else {
                Err(sd_invalid_ann(arg, &rep))
            }
        }
        _ => Err(unsupported(&format!("singledispatch on this annotation ({})", tag))),
    }
}
fn sd_register_ann(st: &Rc<SDState>, f: V) -> R {
    let ann = match &f {
        V::Func(_) => match getattr(&f, "__brevi_sd_ann__") {
            Ok(v) => v,
            Err(E(e)) if is_exc(&e, "AttributeError") => return Err(sd_missing_ann(&f)),
            Err(e) => return Err(e),
        },
        _ => return Err(sd_missing_ann(&f)),
    };
    let (arg, desc) = match &ann {
        V::Tuple(t) if t.len() == 2 => (to_str(&t[0]).unwrap_or_default(), t[1].clone()),
        _ => return Err(unsupported("a singledispatch annotation")),
    };
    let classes = sd_ann_classes(&arg, &desc, None)?;
    {
        let mut reg = st.registry.borrow_mut();
        for c in classes {
            reg.set(c, f.clone())?;
        }
    }
    st.cache.borrow_mut().clear();
    Ok(f)
}
fn sd_register_type(st: &Rc<SDState>, cls: &V, func: V) -> R {
    match cls {
        V::Class(k) if k.module == "typing" => {
            // collections.abc as Brevi's typing forms is not a set of real ABCs here; dispatch over its MRO
            // would not do what CPython's virtual subclass registration does.
            return Err(unsupported(&format!("singledispatch on this ABC ({})", k.name)));
        }
        V::Class(_) => {}
        V::Ext(x) if x.subclasscheck(&V::None).is_some() => {
            return Err(unsupported(&format!("singledispatch on this ABC ({})", x.tname())));
        }
        _ => return Err(sd_invalid_first(cls)),
    }
    st.registry.borrow_mut().set(cls.clone(), func.clone())?;
    st.cache.borrow_mut().clear();
    Ok(func)
}
fn sd_register(st: Rc<SDState>, mut a: Args) -> R {
    if a.p.is_empty() {
        return type_err("register() missing 1 required positional argument: 'cls'".into());
    }
    let first = a.p.remove(0);
    if let Some(func) = a.kw("func").or_else(|| a.p.first().cloned()) {
        return sd_register_type(&st, &first, func);
    }
    if matches!(first, V::Class(_)) {
        let st2 = st.clone();
        let c = first.clone();
        return Ok(builtin("register", move |b: Args| sd_register_type(&st2, &c, a1(&b, "register")?)));
    }
    sd_register_ann(&st, first)
}
fn sd_find_impl(cls: &V, reg: &Rc<RefCell<Dict>>) -> Result<Option<V>, E> {
    let V::Class(c) = cls else { return Ok(None) };
    let mro = c.mro.borrow().clone();
    let items = reg.borrow().items();
    for t in mro {
        for (k, v) in &items {
            if is(k, &V::Class(t.clone())) {
                return Ok(Some(v.clone()));
            }
        }
    }
    Ok(None)
}
fn sd_key(cls: &V) -> usize {
    match cls {
        V::Class(c) => Rc::as_ptr(c) as *const u8 as usize,
        _ => 0,
    }
}
fn sd_dispatch_impl(st: &Rc<SDState>, cls: &V) -> R {
    {
        if let Some((_, v)) = st.cache.borrow().get(&sd_key(cls)) {
            return Ok(v.clone().unwrap_or(V::None));
        }
    }
    let found = match Dict::get_cell(&st.registry, cls)? {
        Some(v) => Some(v),
        None => sd_find_impl(cls, &st.registry)?,
    };
    st.cache.borrow_mut().insert(sd_key(cls), (cls.clone(), found.clone()));
    Ok(found.unwrap_or(V::None))
}
fn sd_make_wrapper(st: &Rc<SDState>, f: V) -> R {
    let funcname = match getattr(&f, "__name__") {
        Ok(V::Str(x)) => x.s.clone(),
        _ => "singledispatch function".into(),
    };
    let stw = st.clone();
    let w = func("singledispatch", move |a: Args| {
        if a.p.is_empty() {
            return type_err(format!("{funcname} requires at least 1 positional argument"));
        }
        let cls = V::Class(type_of(&a.p[0])?);
        let implv = sd_dispatch_impl(&stw, &cls)?;
        call(&implv, a)
    });
    st.registry.borrow_mut().set(V::Class(bclass("object")), f.clone())?;
    let st1 = st.clone();
    let st2 = st.clone();
    let st3 = st.clone();
    if let V::Func(wf) = &w {
        let reg = builtin("register", move |a: Args| sd_register(st1.clone(), a));
        let disp = builtin("dispatch", move |a: Args| {
            let c = a1(&a, "dispatch")?;
            match &c {
                V::Class(_) => sd_dispatch_impl(&st2, &c),
                _ => type_err(format!("dispatch() argument must be a class, not '{}'", tn(&c))),
            }
        });
        let clear = builtin("_clear_cache", move |_a: Args| {
            st3.cache.borrow_mut().clear();
            Ok(V::None)
        });
        wf.attrs.borrow_mut().push(("register", reg));
        wf.attrs.borrow_mut().push(("dispatch", disp));
        wf.attrs.borrow_mut().push(("_clear_cache", clear));
        wf.attrs.borrow_mut().push(("registry", sd_registry_view(st)));
    }
    update_wrapper(w, f, wraps_assigned(), wraps_updated())
}
fn singledispatch_impl(f: V) -> R {
    let st = Rc::new(SDState { registry: Rc::new(RefCell::new(Dict::new())), cache: RefCell::new(std::collections::HashMap::new()) });
    sd_make_wrapper(&st, f)
}
fn sd_is_abstract(v: &V) -> bool {
    match getattr(v, "__isabstractmethod__") {
        Ok(x) => truth(&x).unwrap_or(false),
        Err(_) => false,
    }
}
fn sd_bind_impl(implv: &V, obj: &V, cls: &V) -> R {
    match implv {
        V::Func(f) if f.kind.get() == 1 => Ok(implv.clone()),
        V::Func(f) if f.kind.get() == 2 => Ok(V::Bound(Rc::new((cls.clone(), implv.clone())))),
        V::Func(_) => Ok(if matches!(obj, V::None) { implv.clone() } else { V::Bound(Rc::new((obj.clone(), implv.clone()))) }),
        other => match getattr(other, "__get__") {
            Ok(g) => call(&g, Args::p(vec![obj.clone(), cls.clone()])),
            Err(_) => Ok(other.clone()),
        },
    }
}
fn sd_method_get(md: &SDMethod, obj: V, cls: V) -> R {
    let funcname = match getattr(&md.func, "__name__") {
        Ok(V::Str(x)) => x.s.clone(),
        _ => "singledispatchmethod method".into(),
    };
    let st = md.st.clone();
    let orig = md.func.clone();
    let objc = obj.clone();
    let clsc = cls.clone();
    let m = func("_method", move |a: Args| {
        if a.p.is_empty() {
            return type_err(format!("{funcname} requires at least 1 positional argument"));
        }
        let first_cls = V::Class(type_of(&a.p[0])?);
        let implv = sd_dispatch_impl(&st, &first_cls)?;
        let bound = sd_bind_impl(&implv, &objc, &clsc)?;
        call(&bound, a)
    });
    let regst = md.st.clone();
    if let V::Func(mf) = &m {
        mf.attrs.borrow_mut().push(("__isabstractmethod__", V::Bool(sd_is_abstract(&orig))));
        mf.attrs.borrow_mut().push(("register", builtin("register", move |a: Args| sd_register(regst.clone(), a))));
    }
    update_wrapper(m, orig, wraps_assigned(), wraps_updated())
}
struct SDMethod {
    st: Rc<SDState>,
    dispatcher: V,
    func: V,
}
impl Ext for SDMethod {
    fn tname(&self) -> &'static str {
        "functools.singledispatchmethod"
    }
    fn repr(&self) -> String {
        format!("<functools.singledispatchmethod object at 0x{:x}>", self as *const SDMethod as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "register" => {
                let st = self.st.clone();
                Ok(builtin("register", move |a: Args| sd_register(st.clone(), a)))
            }
            "dispatcher" => Ok(self.dispatcher.clone()),
            "__isabstractmethod__" => Ok(V::Bool(sd_is_abstract(&self.func))),
            "__name__" => getattr(&self.func, "__name__").or_else(|_| Ok(s("singledispatchmethod"))),
            "__doc__" => getattr(&self.func, "__doc__").or_else(|_| Ok(V::None)),
            _ => return None,
        })
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        match n {
            "__get__" => {
                let obj = a.p.first().cloned().unwrap_or(V::None);
                let cls = match a.p.get(1) {
                    Some(c) => c.clone(),
                    None => match type_of(&obj) {
                        Ok(c) => V::Class(c),
                        Err(e) => return Some(Err(e)),
                    },
                };
                Some(sd_method_get(self, obj, cls))
            }
            "register" => {
                let st = self.st.clone();
                let a = Args { p: a.p.clone(), k: a.k.clone() };
                Some(sd_register(st, a))
            }
            _ => None,
        }
    }
    fn descriptor(&self, _me: &V, obj: &V, cls: &V) -> Option<R> {
        Some(sd_method_get(self, obj.clone(), cls.clone()))
    }
}
fn singledispatchmethod_impl(f: V) -> R {
    let ok = matches!(f, V::Func(_) | V::Class(_)) || getattr(&f, "__call__").is_ok() || getattr(&f, "__get__").is_ok();
    if !ok {
        let r = repr(&f).unwrap_or_else(|_| "<object>".into());
        return type_err(format!("{r} is not callable or a descriptor"));
    }
    let st = Rc::new(SDState { registry: Rc::new(RefCell::new(Dict::new())), cache: RefCell::new(std::collections::HashMap::new()) });
    let dispatcher = sd_make_wrapper(&st, f.clone())?;
    Ok(V::Ext(ExtRc::new(SDMethod { st, dispatcher, func: f })))
}

pub fn functools_fn(name: &str, mut a: Args) -> R {
    Ok(match name {
        "cached_property" => return call(&V::Class(cached_property_class()), a),
        "partialmethod" => return call(&V::Class(partialmethod_class()), a),
        "_CacheInfo" => return call(&cache_info_class()?, a),
        // CPython's functools.RLock is _thread.RLock
        "RLock" => return threading_fn("RLock", a),
        "lru_cache" | "cache" => {
            // @lru_cache, @lru_cache(), @lru_cache(maxsize=None); cache is lru_cache(maxsize=None)
            match a.p.first() {
                Some(f @ (V::Func(_) | V::Bound(_))) => lru_cache_wrap(f.clone(), if name == "cache" { V::None } else { V::Int(128) }, false)?,
                _ => {
                    let maxsize = a.kw("maxsize").or_else(|| a.p.first().cloned()).unwrap_or(if name == "cache" { V::None } else { V::Int(128) });
                    let typed = a.kw("typed").unwrap_or(V::Bool(false));
                    builtin("lru_cache", move |a: Args| lru_cache_wrap(a1(&a, "lru_cache")?, maxsize.clone(), truth(&typed)?))
                }
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
            // cmp_to_key(mycmp) and K(obj) take their one argument by position or by name, as in CPython
            fn one(mut a: Args, f: &str, n: &str) -> R {
                // CPython's argument clinic: the count first, then the name
                let total = a.p.len() + a.k.len();
                if total > 1 {
                    return type_err(format!("{f}() takes at most 1 argument ({total} given)"));
                }
                if let Some(v) = a.p.pop() {
                    return Ok(v);
                }
                match a.kw(n) {
                    Some(v) => Ok(v),
                    None => type_err(format!("{f}() missing required argument '{n}' (pos 1)")),
                }
            }
            let f = one(a, name, "mycmp")?;
            builtin("K", move |a: Args| {
                let x = one(a, "K", "obj")?;
                Ok(V::Ext(ExtRc::new(CmpKey { f: f.clone(), v: x })))
            })
        }
        "partial" => return call(&V::Class(partial_class()), a),
        "update_wrapper" => {
            if a.p.is_empty() {
                return type_err("update_wrapper() missing 2 required positional arguments: 'wrapper' and 'wrapped'".into());
            }
            let wrapper = a.p[0].clone();
            let wrapped = match a.p.get(1) {
                Some(v) => v.clone(),
                None => return type_err("update_wrapper() missing 1 required positional argument: 'wrapped'".into()),
            };
            let assigned = a.kw("assigned").or_else(|| a.p.get(2).cloned()).unwrap_or_else(wraps_assigned);
            let updated = a.kw("updated").or_else(|| a.p.get(3).cloned()).unwrap_or_else(wraps_updated);
            a.want(name, 2, 4)?;
            a.no_kw(name)?;
            update_wrapper(wrapper, wrapped, assigned, updated)?
        }
        "wraps" => {
            let wrapped = match a.p.first() {
                Some(v) => v.clone(),
                None => return type_err("wraps() missing 1 required positional argument: 'wrapped'".into()),
            };
            let assigned = a.kw("assigned").or_else(|| a.p.get(1).cloned()).unwrap_or_else(wraps_assigned);
            let updated = a.kw("updated").or_else(|| a.p.get(2).cloned()).unwrap_or_else(wraps_updated);
            a.want(name, 1, 3)?;
            a.no_kw(name)?;
            // functools.wraps(f) is a decorator that applies update_wrapper to the wrapped def.
            builtin("wraps", move |a: Args| {
                let w = a1(&a, "wraps")?;
                update_wrapper(w, wrapped.clone(), assigned.clone(), updated.clone())
            })
        }
        "total_ordering" => {
            let c = a1(&a, name)?;
            if let V::Class(k) = &c {
                total_ordering(k)?;
            }
            c
        }
        "singledispatch" => singledispatch_impl(a1(&a, name)?)?,
        "singledispatchmethod" => singledispatchmethod_impl(a1(&a, name)?)?,
        _ => return Err(unsupported(&format!("functools.{name}"))),
    })
}
// functools.total_ordering, as CPython 3.13's: _convert maps each root to the comparisons it derives and
// the helper function whose name, qualname and docstring the derived method keeps; a root is a comparison
// whose lookup differs from object's; __lt__ is preferred over __le__ over __gt__ over __ge__; and each
// missing comparison is filled in with the same short-circuit and NotImplemented propagation as _gt_from_lt
// & co. (the helpers call the root directly, not through the operator, so NotImplemented passes through).
fn total_ordering(k: &Rc<Class>) -> Result<(), E> {
    // root, then (comparison, helper name, helper doc) exactly as CPython's _convert and helpers.
    const CONVERT: &[(&str, &[(&str, &str, &str)])] = &[
        ("__lt__", &[
            ("__gt__", "_gt_from_lt", "Return a > b.  Computed by @total_ordering from (not a < b) and (a != b)."),
            ("__le__", "_le_from_lt", "Return a <= b.  Computed by @total_ordering from (a < b) or (a == b)."),
            ("__ge__", "_ge_from_lt", "Return a >= b.  Computed by @total_ordering from (not a < b)."),
        ]),
        ("__le__", &[
            ("__ge__", "_ge_from_le", "Return a >= b.  Computed by @total_ordering from (not a <= b) or (a == b)."),
            ("__lt__", "_lt_from_le", "Return a < b.  Computed by @total_ordering from (a <= b) and (a != b)."),
            ("__gt__", "_gt_from_le", "Return a > b.  Computed by @total_ordering from (not a <= b)."),
        ]),
        ("__gt__", &[
            ("__lt__", "_lt_from_gt", "Return a < b.  Computed by @total_ordering from (not a > b) and (a != b)."),
            ("__ge__", "_ge_from_gt", "Return a >= b.  Computed by @total_ordering from (a > b) or (a == b)."),
            ("__le__", "_le_from_gt", "Return a <= b.  Computed by @total_ordering from (not a > b)."),
        ]),
        ("__ge__", &[
            ("__le__", "_le_from_ge", "Return a <= b.  Computed by @total_ordering from (not a >= b) or (a == b)."),
            ("__gt__", "_gt_from_ge", "Return a > b.  Computed by @total_ordering from (a >= b) and (a != b)."),
            ("__lt__", "_lt_from_ge", "Return a < b.  Computed by @total_ordering from (not a >= b)."),
        ]),
    ];
    // CPython's roots: {op for op in _convert if getattr(cls, op, None) is not getattr(object, op, None)}.
    // A lookup that is Some holds a comparison the class defines or inherits from a user class (or a
    // namedtuple/dataclass base); None means no entry, i.e. object's own comparison, which is not a root.
    // A builtin base other than object's (list, ...) provides the comparison as CPython's slot wrappers do,
    // but Brevi has no callable for it, so a class deriving from such a root is refused below, never panics.
    // Exception classes only reach object's own comparisons, as in CPython.
    let is_root = |n: &str| match k.lookup(n) {
        Some(v) => !matches!(v, V::None),
        None => k
            .mro
            .borrow()
            .iter()
            .skip(1)
            .any(|c| (c.module == "builtins" || c.module == "collections") && c.name != "object" && c.name != "type" && c.base_bt() != BT::Exc),
    };
    let roots: Vec<&str> = CONVERT.iter().map(|(r, _)| *r).filter(|n| is_root(n)).collect();
    if roots.is_empty() {
        return val_err("must define at least one ordering operation: < > <= >=".into());
    }
    // max: prefer __lt__ to __le__ to __gt__ to __ge__
    let root = *roots.iter().max().unwrap();
    let derived: Vec<(&str, &str, &str)> = CONVERT.iter().find(|(r, _)| *r == root).unwrap().1.iter().filter(|(n, _, _)| !is_root(n)).copied().collect();
    if derived.is_empty() {
        return Ok(());
    }
    if k.lookup(root).is_none() {
        return Err(unsupported(&format!("functools.total_ordering: {root} inherited from a builtin base")));
    }
    for (n, helper, doc) in derived {
        k.set(n, total_ordering_helper(root, n, helper, doc));
    }
    Ok(())
}
// CPython's helpers (_gt_from_lt & co.) are module-level functions of functools, shared by every class the
// decorator fills in (`A.__gt__ is B.__gt__`, pickled by reference as functools._gt_from_lt). Each looks the
// root up on type(self) when called, so a subclass that overrides the root is used, as in CPython.
thread_local! {
    static TO_HELPERS: RefCell<Vec<(&'static str, V)>> = const { RefCell::new(Vec::new()) };
}
pub fn total_ordering_helper_named(helper: &str) -> Option<V> {
    TO_HELPERS.with(|h| h.borrow().iter().find(|(n, _)| *n == helper).map(|(_, v)| v.clone()))
}
fn total_ordering_helper(root: &'static str, n: &'static str, helper: &'static str, doc: &'static str) -> V {
    if let Some(f) = total_ordering_helper_named(helper) {
        // as CPython: opfunc.__name__ = opname on the shared function
        if let V::Func(fv) = &f {
            fv.name.set(sym(n));
        }
        return f;
    }
    let f = func(n, move |a: Args| {
        if a.p.len() != 2 {
            return type_err(format!("{helper}() takes 2 positional arguments but {} were given", a.p.len()));
        }
        let rf = getattr(&V::Class(type_of(&a.p[0])?), root)?;
        let r = call(&rf, Args::p(vec![a.p[0].clone(), a.p[1].clone()]))?;
        if matches!(r, V::NotImpl) {
            return Ok(r);
        }
        let t = truth(&r)?;
        let eqv = || cmp_v(&a.p[0], &a.p[1], "==");
        let nev = || cmp_v(&a.p[0], &a.p[1], "!=");
        match (root, n) {
            ("__lt__", "__gt__") | ("__gt__", "__lt__") => {
                if t { Ok(V::Bool(false)) } else { nev() }
            }
            ("__lt__", "__le__") | ("__gt__", "__ge__") => {
                if t { Ok(r) } else { eqv() }
            }
            ("__le__", "__ge__") | ("__ge__", "__le__") => {
                if !t { Ok(V::Bool(true)) } else { eqv() }
            }
            ("__le__", "__lt__") | ("__ge__", "__gt__") => {
                if t { nev() } else { Ok(r) }
            }
            _ => Ok(V::Bool(!t)),
        }
    });
    if let V::Func(fv) = &f {
        fv.qualname.set(sym(helper));
        fv.attrs.borrow_mut().push(("__doc__", s(doc)));
    }
    let f = with_module(f, "functools");
    TO_HELPERS.with(|h| h.borrow_mut().push((helper, f.clone())));
    f
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
        format!("<functools.KeyWrapper object at 0x{:x}>", self as *const Self as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    // the wrapped value, as CPython's KeyWrapper.obj
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        if n == "obj" {
            return Some(Ok(self.v.clone()));
        }
        None
    }
    fn eq_rich(&self, _me: &V, o: &V, _right: bool) -> Option<Result<bool, E>> {
        Some(match cmp_key_of(o) {
            Some(k) => cmp_key_cmp(&self.f, &self.v, &k.v, "=="),
            None => type_err("other argument must be K instance".into()),
        })
    }
    fn rich(&self, _me: &V, op: &str, o: &V, _right: bool) -> Option<Result<bool, E>> {
        match op {
            "<" | ">" | "<=" | ">=" => Some(match cmp_key_of(o) {
                Some(k) => cmp_key_cmp(&self.f, &self.v, &k.v, op),
                None => type_err("other argument must be K instance".into()),
            }),
            _ => None,
        }
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
fn cmp_key_of(o: &V) -> Option<&CmpKey> {
    match o {
        V::Ext(x) => x.any().downcast_ref::<CmpKey>(),
        _ => None,
    }
}
// CPython's KeyWrapper does `mycmp(self.obj, other.obj) OP 0` for OP in ==, <, >, <=, >=: the
// comparison runs on whatever mycmp returned, so errors and odd result types come out as they do there.
fn cmp_key_cmp(f: &V, x: &V, y: &V, op: &str) -> Result<bool, E> {
    let r = call(f, Args::p(vec![x.clone(), y.clone()]))?;
    if op == "==" {
        eq(&r, &V::Int(0))
    } else {
        rich(&r, &V::Int(0), op)
    }
}

// itertools.groupby, as CPython's: the groups share the parent's position, so advancing the parent ends the
// current group, and no borrow is held across the key function or a user __eq__ (both may re-enter).
struct GroupBy {
    key: Option<V>,
    tgt: Option<V>,
    cur_key: Option<V>,
    cur_val: Option<V>,
    group: u64,
}
fn groupby_step(it: &Rc<RefCell<It>>, st: &Rc<RefCell<GroupBy>>) -> Result<bool, E> {
    let Some(v) = iter_step(it)? else { return Ok(false) };
    let f = st.borrow().key.clone();
    let k = match f {
        Some(f) => call(&f, Args::p(vec![v.clone()]))?,
        None => v.clone(),
    };
    let mut s = st.borrow_mut();
    s.cur_val = Some(v);
    s.cur_key = Some(k);
    Ok(true)
}
fn same_or_eq(a: &V, b: &V) -> Result<bool, E> {
    Ok(same_obj(a, b) || eq(a, b)?)
}
fn groupby_iter(it: Rc<RefCell<It>>, key: Option<V>) -> V {
    let st = Rc::new(RefCell::new(GroupBy { key, tgt: None, cur_key: None, cur_val: None, group: 0 }));
    shared_iter(ItKind::ItGroupby, move || {
        st.borrow_mut().group += 1;
        loop {
            let (tgt, cur) = {
                let s = st.borrow();
                (s.tgt.clone(), s.cur_key.clone())
            };
            match (tgt, cur) {
                (_, None) => {}
                (None, Some(_)) => break,
                (Some(t), Some(c)) => {
                    if !same_or_eq(&t, &c)? {
                        break;
                    }
                }
            }
            if !groupby_step(&it, &st)? {
                return Ok(None);
            }
        }
        // a fresh group: any grouper made before, including by a re-entrant call, is now over
        let (me, k) = {
            let mut s = st.borrow_mut();
            s.group += 1;
            s.tgt = s.cur_key.clone();
            (s.group, s.tgt.clone().unwrap())
        };
        let (it2, st2, tk) = (it.clone(), st.clone(), k.clone());
        let grouper = shared_iter(ItKind::ItGrouper, move || {
            if st2.borrow().group != me {
                return Ok(None);
            }
            let empty = st2.borrow().cur_val.is_none();
            if empty && !groupby_step(&it2, &st2)? {
                return Ok(None);
            }
            let Some(c) = st2.borrow().cur_key.clone() else { return Ok(None) };
            if !same_or_eq(&tk, &c)? {
                return Ok(None);
            }
            let mut s = st2.borrow_mut();
            s.cur_key = None;
            Ok(s.cur_val.take())
        });
        Ok(Some(tup(vec![k, grouper])))
    })
}
// itertools sizes (r, repeat) are PyNumber_AsSsize_t in CPython: ints past 64 bits overflow.
fn it_ssize(v: &V, neg_msg: &str) -> Result<i64, E> {
    if matches!(v, V::Big(_)) || matches!(inner_of(v), Some(V::Big(_))) {
        return Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into()));
    }
    let i = idx_of(v)?;
    if i < 0 {
        return Err(exc("ValueError", neg_msg.into()));
    }
    Ok(i)
}
// islice converts each bound with PyLong_AsSsize_t and, as CPython, folds any conversion
// failure (float, string, an int past 64 bits, ...) into the bound's range ValueError.
fn islice_ssize(v: &V, none: i64) -> i64 {
    if matches!(v, V::None) {
        return none;
    }
    match idx_of(v) {
        Ok(i) => i,
        Err(_) => -1,
    }
}
const ISLICE_STOP_MSG: &str = "Stop argument for islice() must be None or an integer: 0 <= x <= sys.maxsize.";
const ISLICE_START_MSG: &str = "Indices for islice() must be None or an integer: 0 <= x <= sys.maxsize.";
const ISLICE_STEP_MSG: &str = "Step for islice() must be a positive integer or None.";

// PyNumber_AsSsize_t, as itertools.tee and itertools.batched convert their n argument.
fn it_ssize_arg(v: &V) -> Result<i64, E> {
    let x = match v {
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        V::Big(_) => return Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__index__") {
                return it_index_result(&call(&f, Args::p(vec![v.clone()]))?);
            }
            if matches!(inner_of(v), Some(V::Int(_) | V::Bool(_) | V::Big(_))) {
                return it_ssize_arg(&inner_of(v).unwrap());
            }
            return Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v))));
        }
        _ => return Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v)))),
    };
    Ok(x)
}
fn it_index_result(r: &V) -> Result<i64, E> {
    match r {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        V::Obj(_) if matches!(inner_of(r), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => it_index_result(&inner_of(r).unwrap()),
        x => Err(exc("TypeError", format!("__index__ returned non-int (type {})", tn(x)))),
    }
}

// itertools.tee: the tee iterators of one group share a TeeData holding the source and the values fetched
// but not yet read by every live member (CPython frees a link once all tees are past it), and each keeps its
// own absolute position. A re-entrant next() on any tee of a group raises RuntimeError, as CPython does.
pub struct TeeData {
    it: Rc<RefCell<It>>,
    // the buffered values in blocks of TEE_BLOCK (as CPython's teedataobject links blocks of 57): vals[0][0] is
    // the value at absolute position `base`. Blocks keep a long buffer from ever being copied to grow it, and
    // a block is freed once every member is past it.
    vals: RefCell<std::collections::VecDeque<Vec<V>>>,
    base: Cell<usize>,
    members: RefCell<Vec<std::rc::Weak<TeePos>>>,
    active: Cell<bool>,
}
pub struct TeePos {
    data: Rc<TeeData>,
    pos: Cell<usize>,
}
impl Drop for TeePos {
    fn drop(&mut self) {
        // a deleted tee no longer holds values back (test_tee_del_backward)
        tee_trim(&self.data);
    }
}
const TEE_BLOCK: usize = 64;
fn tee_trim(d: &TeeData) {
    let lowest = {
        let mut m = d.members.borrow_mut();
        m.retain(|w| w.strong_count() > 0);
        m.iter().filter_map(|w| w.upgrade()).map(|p| p.pos.get()).min()
    };
    let Some(lowest) = lowest else { return };
    if lowest < d.base.get() + TEE_BLOCK {
        return;
    }
    let mut freed = vec![];
    {
        let mut vals = d.vals.borrow_mut();
        while d.base.get() + TEE_BLOCK <= lowest && vals.front().is_some_and(|b| b.len() == TEE_BLOCK) {
            freed.push(vals.pop_front().unwrap());
            d.base.set(d.base.get() + TEE_BLOCK);
        }
    }
    // values are dropped after the borrow ends: a __del__ may run
    drop(freed);
}
// itertools._tee(iterable), CPython's tee_new: a copy of a tee, else a lone member over iter(iterable)
pub fn tee_new(a: Args) -> R {
    if !a.k.is_empty() {
        return type_err("_tee() takes no keyword arguments".into());
    }
    if a.p.len() != 1 {
        return type_err(format!("_tee expected 1 argument, got {}", a.p.len()));
    }
    let it = iter_v(&a.p[0])?;
    if let Some((data, pos)) = tee_pos_of(&it) {
        return Ok(tee_member(&data, pos));
    }
    let src = shared_input(&it)?;
    let data = Rc::new(TeeData { it: src, vals: RefCell::new(Default::default()), base: Cell::new(0), members: RefCell::new(vec![]), active: Cell::new(false) });
    Ok(tee_member(&data, 0))
}
fn tee_member(data: &Rc<TeeData>, pos: usize) -> V {
    let p = Rc::new(TeePos { data: data.clone(), pos: Cell::new(pos) });
    data.members.borrow_mut().push(Rc::downgrade(&p));
    new_iter(It::Tee(p), IterClass::Kind(ItKind::ItTee))
}
fn tee_step(p: &Rc<TeePos>) -> Result<Option<V>, E> {
    if p.data.active.get() {
        return Err(exc("RuntimeError", "cannot re-enter the tee iterator".into()));
    }
    p.data.active.set(true);
    let r = tee_step_inner(p);
    p.data.active.set(false);
    r
}
fn tee_step_inner(p: &Rc<TeePos>) -> Result<Option<V>, E> {
    let i = p.pos.get();
    let d = &p.data;
    let got = {
        let vals = d.vals.borrow();
        let k = i - d.base.get();
        vals.get(k / TEE_BLOCK).and_then(|b| b.get(k % TEE_BLOCK)).cloned()
    };
    let v = match got {
        Some(v) => v,
        None => match iter_step(&d.it)? {
            Some(v) => {
                let mut vals = d.vals.borrow_mut();
                match vals.back_mut() {
                    Some(b) if b.len() < TEE_BLOCK => b.push(v.clone()),
                    _ => {
                        let mut b = Vec::with_capacity(TEE_BLOCK);
                        b.push(v.clone());
                        vals.push_back(b);
                    }
                }
                v
            }
            None => return Ok(None),
        },
    };
    p.pos.set(i + 1);
    // a block can be freed only once the last member reaches its end, a multiple of TEE_BLOCK
    if (i + 1) % TEE_BLOCK == 0 {
        tee_trim(d);
    }
    Ok(Some(v))
}
fn tee_pos_of(v: &V) -> Option<(Rc<TeeData>, usize)> {
    let V::Iter(cell) = v else { return None };
    let Ok(it) = cell.it.try_borrow() else { return None };
    match &*it {
        It::Tee(p) => Some((p.data.clone(), p.pos.get())),
        _ => None,
    }
}
// copy.copy/deepcopy of a tee iterator make a new tee of the group at the same position, as CPython's
// tee.__copy__ does.
fn tee_copy_iter(v: &V) -> Option<V> {
    tee_pos_of(v).map(|(d, pos)| tee_member(&d, pos))
}
pub fn itertools_fn(name: &str, mut a: Args) -> R {
    Ok(match name {
        "permutations" => {
            let pool = to_vec(a.p.first().ok_or_else(|| exc("TypeError", "permutations() missing iterable".into()))?)?;
            let r = match a.p.get(1).cloned().or(a.kw("r")) {
                Some(V::None) | None => pool.len(),
                Some(x) => it_ssize(&x, "r must be non-negative")? as usize,
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
            vec_iter(ItKind::ItPermutations, out)
        }
        "combinations" | "combinations_with_replacement" => {
            a.want(name, 2, 2)?;
            let pool = to_vec(&a.p[0])?;
            let r = it_ssize(&a.p[1], "r must be non-negative")? as usize;
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
            vec_iter(if rep { ItKind::ItCombinationsWithReplacement } else { ItKind::ItCombinations }, out)
        }
        "product" => {
            let rep = match a.kw("repeat") {
                Some(x) => it_ssize(&x, "repeat argument cannot be negative")? as usize,
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
            vec_iter(ItKind::ItProduct, res.into_iter().map(tup).collect())
        }
        // The iterator tools are lazy, as in CPython: they pull from their inputs one item at a time, so they work
        // on infinite inputs, see items added after they were made, and run callbacks in CPython's order.
        "chain" => {
            let srcs = std::mem::take(&mut a.p);
            let mut k = 0;
            let mut cur: Option<It> = None;
            gen_iter(ItKind::ItChain, move || loop {
                if let Some(it) = &mut cur {
                    if let Some(x) = it.next()? {
                        return Ok(Some(x));
                    }
                    cur = None;
                }
                if k >= srcs.len() {
                    return Ok(None);
                }
                k += 1;
                cur = Some(iter(&srcs[k - 1])?);
            })
        }
        "chain.from_iterable" => {
            let mut outer = iter(&a1(&a, "from_iterable")?)?;
            let mut cur: Option<It> = None;
            let mut done = false;
            gen_iter(ItKind::ItChain, move || loop {
                if let Some(it) = &mut cur {
                    if let Some(x) = it.next()? {
                        return Ok(Some(x));
                    }
                    cur = None;
                }
                if done {
                    return Ok(None);
                }
                match outer.next()? {
                    Some(x) => cur = Some(iter(&x)?),
                    None => {
                        done = true;
                        return Ok(None);
                    }
                }
            })
        }
        "accumulate" => {
            let init = a.kw("initial").filter(|x| !matches!(x, V::None));
            let f = a.p.get(1).cloned().or(a.kw("func")).filter(|f| !matches!(f, V::None));
            let mut it = iter(&a.p.first().cloned().or(a.kw("iterable")).ok_or_else(|| exc("TypeError", "accumulate() missing required argument 'iterable' (pos 1)".into()))?)?;
            let mut acc: Option<V> = None;
            let mut first = init;
            gen_iter(ItKind::ItAccumulate, move || {
                if let Some(i) = first.take() {
                    acc = Some(i.clone());
                    return Ok(Some(i));
                }
                let Some(x) = it.next()? else { return Ok(None) };
                let v = match acc.take() {
                    None => x,
                    Some(p) => match &f {
                        Some(f) => call(f, Args::p(vec![p, x]))?,
                        None => add(&p, &x)?,
                    },
                };
                acc = Some(v.clone());
                Ok(Some(v))
            })
        }
        "groupby" => {
            let key = a.p.get(1).cloned().or(a.kw("key")).filter(|k| !matches!(k, V::None));
            let it = shared_input(&a.p.first().cloned().or(a.kw("iterable")).ok_or_else(|| exc("TypeError", "groupby() missing required argument 'iterable'".into()))?)?;
            groupby_iter(it, key)
        }
        "count" => {
            let mut n = a.p.first().cloned().unwrap_or(V::Int(0));
            let st0 = a.p.get(1).cloned().unwrap_or(V::Int(1));
            gen_iter(ItKind::ItCount, move || {
                let v = n.clone();
                n = add(&n, &st0)?;
                Ok(Some(v))
            })
        }
        "islice" => {
            if !a.k.is_empty() {
                return Err(exc("TypeError", "islice() takes no keyword arguments".into()));
            }
            let n = a.p.len();
            if n < 2 {
                return Err(exc("TypeError", format!("islice expected at least 2 arguments, got {n}")));
            }
            if n > 4 {
                return Err(exc("TypeError", format!("islice expected at most 4 arguments, got {n}")));
            }
            let src = a.p[0].clone();
            // CPython converts and range-checks the bounds in order: stop, start, step.
            let stop = islice_ssize(if n == 2 { &a.p[1] } else { &a.p[2] }, i64::MAX);
            if stop < 0 {
                // CPython's islice folds conversion failures into -1 (reported as the stop
                // argument error) but reports other negative stop values with the indices error.
                let msg = if stop == -1 { ISLICE_STOP_MSG } else { ISLICE_START_MSG };
                return Err(exc("ValueError", msg.into()));
            }
            let start = if n == 2 { 0 } else { islice_ssize(&a.p[1], 0) };
            if start < 0 {
                return Err(exc("ValueError", ISLICE_START_MSG.into()));
            }
            let step = if n == 4 { islice_ssize(&a.p[3], 1) } else { 1 };
            if step < 1 {
                return Err(exc("ValueError", ISLICE_STEP_MSG.into()));
            }
            let mut it = iter(&src)?;
            let mut i = 0i64;
            let mut next_i = start;
            gen_iter(ItKind::ItIslice, move || loop {
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
            let mut its: Vec<Option<It>> = vec![];
            for x in &a.p {
                its.push(Some(iter(x)?));
            }
            let mut active = its.len();
            gen_iter(ItKind::ItZipLongest, move || {
                if active == 0 {
                    return Ok(None);
                }
                let mut row = Vec::with_capacity(its.len());
                for slot in its.iter_mut() {
                    let got = match slot {
                        Some(it) => it.next()?,
                        None => None,
                    };
                    match got {
                        Some(x) => row.push(x),
                        None => {
                            if slot.take().is_some() {
                                active -= 1;
                                if active == 0 {
                                    its.clear();
                                    return Ok(None);
                                }
                            }
                            row.push(fill.clone());
                        }
                    }
                }
                Ok(Some(tup(row)))
            })
        }
        "repeat" => {
            let x = a.p[0].clone();
            match a.p.get(1).cloned().or(a.kw("times")) {
                Some(n) => new_iter(It::Repeat(x, Some(idx_of(&n)?.max(0))), IterClass::Kind(ItKind::ItRepeat)),
                None => new_iter(It::Repeat(x, None), IterClass::Kind(ItKind::ItRepeat)),
            }
        }
        "cycle" => {
            let mut it = Some(iter(&a1(&a, name)?)?);
            let mut saved: Vec<V> = vec![];
            let mut i = 0;
            gen_iter(ItKind::ItCycle, move || {
                if let Some(src) = &mut it {
                    if let Some(x) = src.next()? {
                        saved.push(x.clone());
                        return Ok(Some(x));
                    }
                    it = None;
                }
                if saved.is_empty() {
                    return Ok(None);
                }
                let v = saved[i % saved.len()].clone();
                i += 1;
                Ok(Some(v))
            })
        }
        "starmap" => {
            a.want(name, 2, 2)?;
            let f = a.p[0].clone();
            let mut it = iter(&a.p[1])?;
            gen_iter(ItKind::ItStarmap, move || match it.next()? {
                Some(x) => Ok(Some(call(&f, Args::p(to_vec(&x)?))?)),
                None => Ok(None),
            })
        }
        "takewhile" | "dropwhile" => {
            a.want(name, 2, 2)?;
            let f = a.p[0].clone();
            let mut it = iter(&a.p[1])?;
            let take = name == "takewhile";
            let mut flag = false;
            gen_iter(if take { ItKind::ItTakewhile } else { ItKind::ItDropwhile }, move || loop {
                if take && flag {
                    return Ok(None);
                }
                let Some(x) = it.next()? else { return Ok(None) };
                if !take && flag {
                    return Ok(Some(x));
                }
                let t = truth(&call(&f, Args::p(vec![x.clone()]))?)?;
                if take {
                    if t {
                        return Ok(Some(x));
                    }
                    flag = true;
                } else if !t {
                    flag = true;
                    return Ok(Some(x));
                }
            })
        }
        "pairwise" => {
            // CPython's order, which re-entrant inputs can observe: the first item, then each next one
            let it = shared_input(&a1(&a, name)?)?;
            let st: Rc<RefCell<(Option<V>, bool)>> = Rc::new(RefCell::new((None, false)));
            shared_iter(ItKind::ItPairwise, move || {
                if st.borrow().1 {
                    return Ok(None);
                }
                let prev = st.borrow().0.clone();
                let old = match prev {
                    Some(o) => o,
                    None => match iter_step(&it)? {
                        Some(o) => {
                            st.borrow_mut().0 = Some(o.clone());
                            o
                        }
                        None => {
                            st.borrow_mut().1 = true;
                            return Ok(None);
                        }
                    },
                };
                match iter_step(&it)? {
                    Some(x) => {
                        st.borrow_mut().0 = Some(x.clone());
                        Ok(Some(tup(vec![old, x])))
                    }
                    None => {
                        *st.borrow_mut() = (None, true);
                        Ok(None)
                    }
                }
            })
        }
        "compress" => {
            let data = a.p.first().cloned().or(a.kw("data")).ok_or_else(|| exc("TypeError", "compress() missing required argument 'data' (pos 1)".into()))?;
            let sel = a.p.get(1).cloned().or(a.kw("selectors")).ok_or_else(|| exc("TypeError", "compress() missing required argument 'selectors' (pos 2)".into()))?;
            let mut xs = iter(&data)?;
            let mut ss = iter(&sel)?;
            gen_iter(ItKind::ItCompress, move || loop {
                let Some(x) = xs.next()? else { return Ok(None) };
                let Some(s0) = ss.next()? else { return Ok(None) };
                if truth(&s0)? {
                    return Ok(Some(x));
                }
            })
        }
        "filterfalse" => {
            a.want(name, 2, 2)?;
            let f = a.p[0].clone();
            let mut it = iter(&a.p[1])?;
            gen_iter(ItKind::ItFilterfalse, move || loop {
                let Some(x) = it.next()? else { return Ok(None) };
                let t = if matches!(f, V::None) { truth(&x)? } else { truth(&call(&f, Args::p(vec![x.clone()]))?)? };
                if !t {
                    return Ok(Some(x));
                }
            })
        }
        "tee" => {
            if !a.k.is_empty() {
                return Err(exc("TypeError", "itertools.tee() takes no keyword arguments".into()));
            }
            a.want("tee", 1, 2)?;
            let n = match a.p.get(1) {
                Some(x) => it_ssize_arg(x)?,
                None => 2,
            };
            if n < 0 {
                return Err(exc("ValueError", "n must be >= 0".into()));
            }
            // CPython: it = iter(iterable); when `it` has __copy__, the result is `it` itself followed by
            // n - 1 copies of it; otherwise a new group over `it`.
            let it = iter_v(&a.p[0])?;
            if n == 0 {
                return Ok(tup(vec![]));
            }
            let mut out = vec![it.clone()];
            if let Some((data, pos)) = tee_pos_of(&it) {
                // CPython 3.13 makes every member new for a tee input (gh-123884), at its position
                out = (0..n).map(|_| tee_member(&data, pos)).collect();
            } else if let Some(copy) = match &it {
                V::Obj(o) if o.cls.lookup("__copy__").is_some() => Some(getattr(&it, "__copy__")?),
                _ => None,
            } {
                for _ in 1..n {
                    out.push(call(&copy, Args::p(vec![]))?);
                }
            } else {
                let src = shared_input(&it)?;
                let data = Rc::new(TeeData { it: src, vals: RefCell::new(Default::default()), base: Cell::new(0), members: RefCell::new(vec![]), active: Cell::new(false) });
                out.clear();
                for _ in 0..n {
                    out.push(tee_member(&data, 0));
                }
            }
            tup(out)
        }
        "batched" => {
            // CPython's Argument Clinic checks these in this order before converting anything.
            if a.p.len() > 2 {
                return type_err(format!("batched() takes exactly 2 positional arguments ({} given)", a.p.len()));
            }
            if a.p.len() + a.k.len() > 3 {
                return type_err(format!("batched() takes at most 3 arguments ({} given)", a.p.len() + a.k.len()));
            }
            if a.p.len() >= 1 && a.k.iter().any(|(n, _)| *n == "iterable") {
                return type_err("argument for batched() given by name ('iterable') and position (1)".into());
            }
            if a.p.len() >= 2 && a.k.iter().any(|(n, _)| *n == "n") {
                return type_err("argument for batched() given by name ('n') and position (2)".into());
            }
            for (n, _) in &a.k {
                if *n != "iterable" && *n != "n" && *n != "strict" {
                    return type_err(format!("batched() got an unexpected keyword argument '{n}'"));
                }
            }
            let iterable = match a.p.first().cloned() {
                Some(v) => v,
                None => match a.kw("iterable") {
                    Some(v) => v,
                    None => return type_err("batched() missing required argument 'iterable' (pos 1)".into()),
                },
            };
            let nv = match a.p.get(1).cloned() {
                Some(v) => v,
                None => match a.kw("n") {
                    Some(v) => v,
                    None => return type_err("batched() missing required argument 'n' (pos 2)".into()),
                },
            };
            let n = it_ssize_arg(&nv)?;
            let strict = truth(&a.kw("strict").unwrap_or(V::Bool(false)))?;
            if n <= 0 {
                return Err(exc("ValueError", "n must be at least one".into()));
            }
            let it = shared_input(&iterable)?;
            let dead = Rc::new(RefCell::new(false));
            shared_iter(ItKind::ItBatched, move || {
                if *dead.borrow() {
                    return Ok(None);
                }
                let mut row = vec![];
                for i in 0..n {
                    match iter_step(&it)? {
                        Some(x) => row.push(x),
                        None => {
                            if i == 0 {
                                *dead.borrow_mut() = true;
                                let old = std::mem::replace(&mut *it.borrow_mut(), It::Done);
                                drop(old);
                                return Ok(None);
                            }
                            if strict {
                                *dead.borrow_mut() = true;
                                let old = std::mem::replace(&mut *it.borrow_mut(), It::Done);
                                drop(old);
                                return Err(exc("ValueError", "batched(): incomplete batch".into()));
                            }
                            return Ok(Some(tup(row)));
                        }
                    }
                }
                Ok(Some(tup(row)))
            })
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
            let (attrs, dict) = {
                let d = o.d.borrow();
                (d.attrs.clone(), d.dict.clone())
            };
            *n.d.borrow_mut() = AStore { attrs, dict: None };
            if let Some(dd) = dict {
                let (kind, dflt, items) = {
                    let src = dd.borrow();
                    (src.kind, src.dflt.clone(), src.items())
                };
                let mut nd = Dict::new();
                nd.kind = kind;
                nd.dflt = dflt;
                for (k, v) in items {
                    nd.set(k, v)?;
                }
                n.set_dict(V::Dict(Rc::new(RefCell::new(nd))))?;
            }
            n.set_inner(match o.inner() {
                Some(i) => Some(shallow_copy(&i)?),
                None => None,
            });
            V::Obj(n)
        }
        V::Iter(_) => match tee_copy_iter(x) {
            Some(v) => v,
            None => x.clone(),
        },
        _ => x.clone(),
    })
}
fn ptr_of(x: &V) -> Option<usize> {
    Some(match x {
        V::List(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Dict(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Set(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Obj(l) => Rc::as_ptr(l) as *const u8 as usize,
        V::Tuple(l) => Tup::as_ptr(l) as *const u8 as usize,
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
                Dict::set_cell(&n, k, v)?;
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
            let (attrs, dict) = {
                let d = o.d.borrow();
                (d.attrs.clone(), d.dict.clone())
            };
            if let Some(dd) = dict {
                let (kind, dflt, items) = {
                    let src = dd.borrow();
                    (src.kind, src.dflt.clone(), src.items())
                };
                let mut nd = Dict::new();
                nd.kind = kind;
                nd.dflt = dflt;
                for (k, v) in items {
                    let k = deepcopy(&k, memo)?;
                    let v = deepcopy(&v, memo)?;
                    nd.set(k, v)?;
                }
                for (k, v) in attrs.into_iter() {
                    n.set_res(k, deepcopy(&v, memo)?)?;
                }
                n.set_dict(V::Dict(Rc::new(RefCell::new(nd))))?;
            } else {
                for (k, v) in attrs.into_iter() {
                    n.set_res(k, deepcopy(&v, memo)?)?;
                }
            }
            let inner = o.inner();
            if let Some(i) = inner {
                n.set_inner(Some(deepcopy(&i, memo)?));
            }
            V::Obj(n)
        }
        V::Iter(_) => match tee_copy_iter(x) {
            Some(v) => v,
            None => x.clone(),
        },
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
                    // as `a < b`: an object's __lt__ result is the value (polars, numpy)
                    return cmp_v(&a.p[0], &a.p[1], o);
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
// The exceptions being handled (innermost last), for sys.exception() and sys.exc_info().
// An except block pushes one with ExcGuard and the guard removes it again, however the
// block ends. Entries carry an id so tasks that interleave across awaits remove their own.
thread_local! {
    static EXC_STACK: RefCell<Vec<(u64, V)>> = const { RefCell::new(Vec::new()) };
    static EXC_ID: Cell<u64> = const { Cell::new(0) };
}
pub struct ExcGuard(u64);
impl ExcGuard {
    pub fn push(e: V) -> ExcGuard {
        let id = EXC_ID.with(|c| {
            c.set(c.get() + 1);
            c.get()
        });
        EXC_STACK.with(|s| s.borrow_mut().push((id, e)));
        ExcGuard(id)
    }
}
impl Drop for ExcGuard {
    fn drop(&mut self) {
        let id = self.0;
        EXC_STACK.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(i) = s.iter().rposition(|(k, _)| *k == id) {
                s.remove(i);
            }
        });
    }
}
// `raise X` inside an except block: X.__context__ is the exception being handled (unless it is X itself).
pub fn set_context(e: &E) {
    if let (V::Obj(o), Some(V::Obj(c))) = (&e.0, cur_exc()) {
        if !Rc::ptr_eq(o, &c) && o.get("__context__").is_none() {
            o.set("__context__", V::Obj(c));
        }
    }
}
fn cur_exc() -> Option<V> {
    EXC_STACK.with(|s| s.borrow().last().map(|(_, e)| e.clone()))
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
        "set_int_max_str_digits" => {
            let mut a = a;
            let kw = a.kw("maxdigits");
            let v = match (a.p.first(), kw) {
                (Some(v), None) if a.p.len() == 1 && a.k.is_empty() => v.clone(),
                (None, Some(v)) if a.k.is_empty() => v,
                _ => return type_err("set_int_max_str_digits() takes exactly one argument (maxdigits)".into()),
            };
            set_int_max_digits(&v)?;
            Ok(V::None)
        }
        "get_int_max_str_digits" => {
            a.want(name, 0, 0)?;
            Ok(V::Int(int_max_digits() as i64))
        }
        "intern" => Ok(a1(&a, name)?),
        "getsizeof" => Ok(V::Int(64)),
        "exception" => {
            a.want(name, 0, 0)?;
            Ok(cur_exc().unwrap_or(V::None))
        }
        "exc_info" => {
            a.want(name, 0, 0)?;
            Ok(match cur_exc() {
                Some(e) => tup(vec![V::Class(type_of(&e)?), e, V::None]),
                None => tup(vec![V::None, V::None, V::None]),
            })
        }
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
        // The binary streams (sys.stdin.buffer.read().split(), sys.stdout.buffer.write(b"...")), a common
        // fast-I/O idiom: the same stdin and stdout as the text streams, as bytes.
        ("sys.stdin.buffer", "read") => match a.p.first() {
            Some(V::Int(n)) if *n >= 0 => Ok(bytesv(read_n_bytes(*n as usize))),
            _ => Ok(bytesv(read_all_bytes())),
        },
        ("sys.stdin.buffer", "readline") => Ok(bytesv(read_line_bytes().unwrap_or_default())),
        ("sys.stdin.buffer", "readlines") => {
            let mut v = vec![];
            while let Some(l) = read_line_bytes() {
                v.push(bytesv(l));
            }
            Ok(list(v))
        }
        ("sys.stdin.buffer", "__iter__") => Ok(gen_iter(ItKind::Unknown, || Ok(read_line_bytes().map(bytesv)))),
        ("sys.stdin.buffer", "fileno") => Ok(V::Int(0)),
        ("sys.stdin.buffer" | "sys.stdout.buffer" | "sys.stderr.buffer", "isatty") => Ok(V::Bool(false)),
        ("sys.stdout.buffer" | "sys.stderr.buffer", "write") => {
            let x = a1(&a, "write")?;
            let b = match bytes_of(&x) {
                Some(b) => b,
                None => return type_err(format!("a bytes-like object is required, not '{}'", tn(&x))),
            };
            if stream == "sys.stdout.buffer" {
                out_write_bytes(&b);
            } else {
                out_flush();
                std::io::Write::write_all(&mut std::io::stderr(), &b).ok();
            }
            Ok(V::Int(b.len() as i64))
        }
        ("sys.stdout.buffer", "flush") => {
            out_flush();
            Ok(V::None)
        }
        ("sys.stderr.buffer", "flush") => Ok(V::None),
        _ => Err(unsupported(&format!("{stream}.{name}"))),
    }
}
thread_local! {
    // module functions are the same object every time they are read (CPython: `m.f is m.f`), and creating a
    // V::Func per read would also cost a hot call site on every call, so modfn caches them
    static MODFNS: RefCell<HashMap<(&'static str, Sym), V>> = RefCell::new(HashMap::new());
}
fn modfn(m: &'static str, n: &str) -> V {
    let n: Sym = sym(n);
    if let Some(v) = MODFNS.with(|c| c.borrow().get(&(m, n)).cloned()) {
        return v;
    }
    let v = builtin(n, move |a: Args| module_call(m, n, a));
    // marked as a module's function, so it never reads as the builtins function of its name (math.pow.__doc__)
    if let V::Func(f) = &v {
        f.attrs.borrow_mut().push(("__bv_mod__", s(m)));
        f.attrs.borrow_mut().push(("__bv_nobind__", V::Bool(true)));
    }
    MODFNS.with(|c| {
        c.borrow_mut().insert((m, n), v.clone());
    });
    v
}
// A library str (numpy's np.str_) given to a text module is that str (re.match(p, names[0])); json reads
// it inside lists and dicts too.
fn native_strs(v: &V, deep: bool) -> V {
    match v {
        V::Ext(e) => match e.native() {
            Some(Ok(s @ V::Str(_))) => s,
            _ => v.clone(),
        },
        V::List(l) if deep => list(l.borrow().iter().map(|x| native_strs(x, true)).collect()),
        V::Tuple(t) if deep => tup(t.iter().map(|x| native_strs(x, true)).collect()),
        V::Dict(d) if deep => {
            let mut o = Dict::new();
            for (k, x) in d.borrow().items() {
                let _ = o.set(native_strs(&k, true), native_strs(&x, true));
            }
            V::Dict(Rc::new(RefCell::new(o)))
        }
        _ => v.clone(),
    }
}
pub fn module_call(m: &str, n: &str, a: Args) -> R {
    if let Some(k) = kw_dup(&a.k) {
        return type_err(format!("{m}.{n}() got multiple values for keyword argument '{k}'"));
    }
    let a = if matches!(m, "re" | "json" | "textwrap" | "string" | "os" | "os.path" | "unicodedata" | "hashlib" | "base64" | "difflib" | "fnmatch" | "shlex" | "html" | "urllib.parse")
        && (a.p.iter().any(|x| !matches!(x, V::Str(_) | V::Int(_) | V::Float(_))) || !a.k.is_empty())
    {
        let deep = false;
        Args { p: a.p.iter().map(|x| native_strs(x, deep)).collect(), k: a.k.iter().map(|(k, x)| (*k, native_strs(x, deep))).collect() }
    } else {
        a
    };
    match m {
        // the sqlite3 hot path: skip ext_module_call's module match
        "_brevi_sqlite" => brevi_sqlite_fn(n, a),
        "math" => math_fn(n, a),
        "atexit" => atexit_fn(n, a),
        "heapq" => heapq_fn(n, a),
        "bisect" => bisect_fn(n, a),
        "functools" => functools_fn(n, a),
        "itertools" => itertools_fn(n, a),
        "copy" => copy_fn(n, a),
        "operator" => operator_fn(n, a),
        "sys" => sys_fn(n, a),
        "sys.stdin" | "sys.stdout" | "sys.stderr" | "sys.stdin.buffer" | "sys.stdout.buffer" | "sys.stderr.buffer" => stream_fn(m, n, a),
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
            "time" => Ok(V::Float(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0))),
            "time_ns" => Ok(V::Int(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as i64).unwrap_or(0))),
            // CPython's clocks: perf_counter and monotonic are CLOCK_MONOTONIC, process_time the process's CPU time
            "perf_counter" | "monotonic" => Ok(V::Float(clock_ns(false) as f64 * 1e-9)),
            "perf_counter_ns" | "monotonic_ns" => Ok(V::Int(clock_ns(false))),
            "process_time" => Ok(V::Float(clock_ns(true) as f64 * 1e-9)),
            "process_time_ns" => Ok(V::Int(clock_ns(true))),
            "sleep" => {
                let x = a1(&a, "sleep")?;
                let secs = match &x {
                    V::Int(i) => *i as f64,
                    V::Bool(b) => *b as i64 as f64,
                    V::Float(f) => *f,
                    _ => return Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", type_name(&x)))),
                };
                if secs.is_nan() {
                    return Err(exc("ValueError", "Invalid value NaN (not a number)".into()));
                }
                if secs < 0.0 {
                    return Err(exc("ValueError", "sleep length must be non-negative".into()));
                }
                std::thread::sleep(std::time::Duration::from_secs_f64(secs.min(1e9)));
                Ok(V::None)
            }
            "gmtime" => time_gmtime(a),
            "strftime" => time_strftime(a),
            _ => Err(unsupported(&format!("time.{n}"))),
        },
        "typing" | "typing_extensions" => typing_call(n, a),
        "dataclasses" => dataclass_fn(n, a),
        "abc" => Ok(a.p.first().cloned().unwrap_or(V::None)),
        "gc" => gc_fn(n, a),
        "weakref" => weakref_fn(n, a),
        _ => ext_module_call(m, n, a),
    }
}
// CPython's sys.modules: one dict for the whole program, so every read sees the same object and the imports
// each statement registers (a copy would lose `d = sys.modules; d['x'] = 1; sys.modules['x']`).
thread_local! {
    static SYS_MODS: RefCell<Option<Rc<RefCell<Dict>>>> = const { RefCell::new(None) };
}
pub fn sys_modules() -> V {
    SYS_MODS.with(|s| {
        let mut s = s.borrow_mut();
        V::Dict(s.get_or_insert_with(|| Rc::new(RefCell::new(Dict::new()))).clone())
    })
}
// `import m` records the module object the program bound to `m`, so `sys.modules['m'] is m`.
pub fn sys_modules_set(name: &str, v: &V) -> R {
    if let V::Dict(d) = sys_modules() {
        d.borrow_mut().set(s(name), v.clone())?;
    }
    Ok(V::None)
}
thread_local! {
    static SYS_VERSION_INFO: RefCell<Option<Rc<Obj>>> = const { RefCell::new(None) };
}
// sys.version_info: CPython's tuple subclass, so it indexes, slices, compares and iterates as the 5-tuple,
// while major/minor/micro/releaselevel/serial (and the class's n_fields) read as attributes.
fn sys_version_info() -> V {
    SYS_VERSION_INFO.with(|slot| {
        if let Some(vi) = slot.borrow().as_ref() {
            return V::Obj(vi.clone());
        }
        let c = Class::new("version_info", "sys", vec![bclass("tuple")], BT::Tuple);
        c.set("n_fields", V::Int(5));
        c.set("__match_args__", tup(vec![s("major"), s("minor"), s("micro"), s("releaselevel"), s("serial")]));
        c.set("__hash__", func("__hash__", |a: Args| Ok(V::Int(py_hash(&inner_of(&a.p[0]).unwrap_or(V::None))?))));
        let o = Obj::new(c.clone());
        o.set(sym("major"), V::Int(3));
        o.set(sym("minor"), V::Int(13));
        o.set(sym("micro"), V::Int(0));
        o.set(sym("releaselevel"), s("final"));
        o.set(sym("serial"), V::Int(0));
        o.set_inner(Some(tup(vec![V::Int(3), V::Int(13), V::Int(0), s("final"), V::Int(0)])));
        let cc = c.clone();
        c.set("__repr__", func("__repr__", move |a: Args| {
            let me = a.p.first().cloned().unwrap_or(V::None);
            let f = |n: &str| match &me {
                V::Obj(o) => o.get(n).unwrap_or(V::None),
                _ => V::None,
            };
            Ok(st(format!("{}.{}(major={}, minor={}, micro={}, releaselevel={}, serial={})",
                cc.module, cc.name,
                repr(&f("major"))?, repr(&f("minor"))?, repr(&f("micro"))?,
                repr(&f("releaselevel"))?, repr(&f("serial"))?)))
        }));
        *slot.borrow_mut() = Some(o.clone());
        V::Obj(o)
    })
}
// sys.flags: the interpreter options of a plain `python3 prog.py` run, which is what a built program is.
// Only the options a built program cannot change (no -O, -d, -i, -B, ... flags exist for it) are given;
// the ones that depend on the environment or on CPython's startup (hash_randomization, utf8_mode,
// int_max_str_digits, gil) and the tuple forms (repr, len, iteration, indexing) are refused, never guessed.
fn sys_flags() -> V {
    thread_local! {
        static SYS_FLAGS: RefCell<Option<Rc<Obj>>> = const { RefCell::new(None) };
    }
    SYS_FLAGS.with(|slot| {
        if let Some(f) = slot.borrow().as_ref() {
            return V::Obj(f.clone());
        }
        let c = Class::new("flags", "sys", vec![bclass("object")], BT::Other);
        c.set("__getattr__", func("__getattr__", |a: Args| {
            let n = match a.p.get(1) { Some(V::Str(n)) => n.s.clone(), _ => String::new() };
            Err(unsupported(&format!("sys.flags.{n}")))
        }));
        for m in ["__repr__", "__len__", "__iter__", "__getitem__", "__eq__", "__hash__"] {
            c.set(m, func(m, move |_a: Args| Err(unsupported(&format!("sys.flags.{m}")))));
        }
        let o = Obj::new(c);
        for n in ["debug", "inspect", "interactive", "optimize", "dont_write_bytecode", "no_user_site", "no_site",
                  "ignore_environment", "verbose", "bytes_warning", "quiet", "isolated", "warn_default_encoding"] {
            o.set(sym(n), V::Int(0));
        }
        o.set(sym("dev_mode"), V::Bool(false));
        o.set(sym("safe_path"), V::Bool(false));
        *slot.borrow_mut() = Some(o.clone());
        V::Obj(o)
    })
}
pub fn module_attr(m: &'static str, name: &str) -> R {
    let v = match (m, name) {
        ("sys", "stdin") => V::Module(&ModName("sys.stdin")),
        ("sys", "stdout") => V::Module(&ModName("sys.stdout")),
        ("sys", "stderr") => V::Module(&ModName("sys.stderr")),
        ("sys", "argv") => list(ARGV.with(|a| a.borrow().iter().map(|x| s(x)).collect())),
        ("sys", "maxsize") => V::Int(i64::MAX),
        ("sys", "platform") => s(if cfg!(windows) { "win32" } else if cfg!(target_os = "macos") { "darwin" } else { "linux" }),
        ("sys", "byteorder") => s("little"),
        ("sys", "version_info") => sys_version_info(),
        ("sys", "flags") => sys_flags(),
        ("sys", "version") => s("3.13.0 (brevi)"),
        ("sys", "float_info") => return Err(unsupported("sys.float_info")),
        ("sys", "modules") => sys_modules(),
        // CPython puts the script's folder first; a built program has only itself, so its folder stands in
        ("sys", "path") => list(vec![s(&std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned())).unwrap_or_default())]),
        ("sys.stdin", "buffer") => V::Module(&ModName("sys.stdin.buffer")),
        ("sys.stdout", "buffer") => V::Module(&ModName("sys.stdout.buffer")),
        ("sys.stderr", "buffer") => V::Module(&ModName("sys.stderr.buffer")),
        ("math", "pi") => V::Float(std::f64::consts::PI),
        ("math", "e") => V::Float(std::f64::consts::E),
        ("math", "tau") => V::Float(std::f64::consts::TAU),
        ("math", "inf") => V::Float(f64::INFINITY),
        ("math", "nan") => V::Float(math_nan()),
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
        ("typing" | "typing_extensions" | "collections.abc", _) => typing_attr(name),
        ("itertools", "chain") => {
            let f = modfn("itertools", "chain");
            if let V::Func(ff) = &f {
                if !ff.attrs.borrow().iter().any(|(k, _)| *k == "from_iterable") {
                    ff.attrs.borrow_mut().push(("from_iterable", modfn("itertools", "chain.from_iterable")));
                }
            }
            f
        }
        ("dataclasses", "MISSING" | "KW_ONLY" | "InitVar" | "Field" | "FrozenInstanceError") => dataclass_attr(name).unwrap(),
        ("abc", "ABC") => V::Class(abc_class()),
        ("re._constants", "MAXREPEAT") => V::Int(0xFFFF_FFFE),
        ("re._constants", "MAXGROUPS") => V::Int(0x3FFF_FFFF),
        ("functools", "cached_property") => V::Class(cached_property_class()),
        ("functools", "partialmethod") => V::Class(partialmethod_class()),
        ("functools", "partial") => V::Class(partial_class()),
        // CPython's functools imports RLock from _thread, so it is an attribute of the module
        ("functools", "RLock") => return module_attr("threading", "RLock"),
        ("functools", "_CacheInfo") => cache_info_class()?,
        ("functools", "WRAPPER_ASSIGNMENTS") => wraps_assigned(),
        ("functools", "WRAPPER_UPDATES") => wraps_updated(),
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
thread_local! {
    static CACHED_PROPERTY: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
}
// functools.cached_property, as CPython 3.13's pure-Python class: a non-data descriptor (see getattr) that
// takes its name from __set_name__ and caches the value in the instance's __dict__ under it, so later reads
// find the instance attribute first. A program class may subclass it.
fn cached_property_class() -> Rc<Class> {
    CACHED_PROPERTY.with(|slot| {
        if let Some(c) = slot.borrow().as_ref() {
            return c.clone();
        }
        let c = Class::new("cached_property", "functools", vec![bclass("object")], BT::Other);
        c.set("__init__", func(sym("__init__"), |a: Args| {
            if a.p.len() != 2 || !a.k.is_empty() {
                return type_err(format!("cached_property.__init__() takes 2 positional arguments but {} were given", a.p.len()));
            }
            let (me, f) = (&a.p[0], &a.p[1]);
            setattr(me, sym("func"), f.clone())?;
            setattr(me, sym("attrname"), V::None)?;
            setattr(me, sym("__doc__"), getattr(f, "__doc__")?)?;
            setattr(me, sym("__module__"), getattr(f, "__module__")?)?;
            Ok(V::None)
        }));
        c.set("__set_name__", func(sym("__set_name__"), |a: Args| {
            if a.p.len() != 3 {
                return type_err(format!("cached_property.__set_name__() takes 3 positional arguments but {} were given", a.p.len()));
            }
            let me = &a.p[0];
            let cur = getattr(me, "attrname")?;
            if matches!(cur, V::None) {
                setattr(me, sym("attrname"), a.p[2].clone())?;
            } else if !eq(&a.p[2], &cur)? {
                return type_err(format!("Cannot assign the same cached_property to two different names ({} and {}).", repr(&cur)?, repr(&a.p[2])?));
            }
            Ok(V::None)
        }));
        c.set("__get__", func(sym("__get__"), |a: Args| {
            if a.p.len() < 2 || a.p.len() > 3 {
                return type_err(format!("cached_property.__get__() takes from 2 to 3 positional arguments but {} were given", a.p.len()));
            }
            let (me, inst) = (&a.p[0], &a.p[1]);
            if matches!(inst, V::None) {
                return Ok(me.clone());
            }
            let name = getattr(me, "attrname")?;
            if matches!(name, V::None) {
                return type_err("Cannot use cached_property instance without calling __set_name__ on it.".into());
            }
            let tname = || getattr(&V::Class(type_of(inst)?), "__name__").and_then(|n| repr(&n));
            let cache = match getattr(inst, "__dict__") {
                Ok(d) => d,
                Err(e) if is_exc(&e.0, "AttributeError") => {
                    return type_err(format!("No '__dict__' attribute on {} instance to cache {} property.", tname()?, repr(&name)?));
                }
                Err(e) => return Err(e),
            };
            let found = call(&getattr(&cache, "get")?, Args::p(vec![name.clone()]))?;
            // a cached None is told apart from a missing key as CPython's _NOT_FOUND sentinel does
            if !matches!(found, V::None) || truth(&call(&getattr(&cache, "__contains__")?, Args::p(vec![name.clone()]))?)? {
                return Ok(found);
            }
            let f = getattr(me, "func")?;
            let val = call(&f, Args::p(vec![inst.clone()]))?;
            match setitem(&cache, &name, val.clone()) {
                Ok(()) => Ok(val),
                Err(e) if is_exc(&e.0, "TypeError") => type_err(format!(
                    "The '__dict__' attribute on {} instance does not support item assignment for caching {} property.",
                    tname()?,
                    repr(&name)?
                )),
                Err(e) => Err(e),
            }
        }));
        *slot.borrow_mut() = Some(c.clone());
        c
    })
}
thread_local! {
    static PARTIALMETHOD: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
}
fn is_partial_v(v: &V) -> bool {
    matches!(v, V::Obj(o) if o.cls.is_sub(&partial_class()))
}
thread_local! {
    static PARTIAL: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
    static PARTIAL_REPR: RefCell<Vec<usize>> = const { RefCell::new(vec![]) };
}
// A partial object's C fields (fn, args, kw), kept out of its __dict__; None for an object no
// partial.__new__ made (object.__new__(partial) in CPython refuses to make one).
fn pstate(o: &Obj) -> Result<(V, V, V), E> {
    match (o.get("__bv_slot_func"), o.get("__bv_slot_args"), o.get("__bv_slot_keywords")) {
        (Some(f), Some(a), Some(k)) => Ok((f, a, k)),
        _ => Err(unsupported("a functools.partial object not made by partial.__new__")),
    }
}
fn pstate_of(v: &V, what: &str) -> Result<(Rc<Obj>, V, V, V), E> {
    if let V::Obj(o) = v {
        if o.cls.is_sub(&partial_class()) {
            let (f, a, k) = pstate(o)?;
            return Ok((o.clone(), f, a, k));
        }
    }
    type_err(format!("descriptor '{what}' for 'functools.partial' objects doesn't apply to a '{}' object", tn(v)))
}
fn dict_v(v: &V) -> bool {
    matches!(v, V::Dict(_)) || matches!(v, V::Obj(o) if matches!(o.inner(), Some(V::Dict(_))))
}
fn tuple_v(v: &V) -> bool {
    matches!(v, V::Tuple(_)) || matches!(v, V::Obj(o) if matches!(o.inner(), Some(V::Tuple(_))))
}
// CPython's member_descriptor for one of partial's C fields: read-only, and it wins over the instance dict.
fn partial_member(name: &'static str) -> V {
    let c = Class::new("member_descriptor", "builtins", vec![bclass("object")], BT::Other);
    let slot = sym(&format!("__bv_slot_{name}"));
    c.set("__get__", func(sym("__get__"), move |a: Args| {
        if a.p.len() < 2 || a.p.len() > 3 {
            return type_err(format!("expected 1 or 2 arguments, got {}", a.p.len().saturating_sub(1)));
        }
        if matches!(a.p[1], V::None) && a.p.len() == 3 {
            return Ok(a.p[0].clone());
        }
        let (o, ..) = pstate_of(&a.p[1], name)?;
        o.get(slot).ok_or_else(|| exc("AttributeError", name.to_string()))
    }));
    c.set("__set__", func(sym("__set__"), |_a: Args| Err(exc("AttributeError", "readonly attribute".into()))));
    c.set("__delete__", func(sym("__delete__"), |_a: Args| Err(exc("AttributeError", "readonly attribute".into()))));
    c.set("__repr__", func(sym("__repr__"), move |_a: Args| Ok(st(format!("<member '{name}' of 'functools.partial' objects>")))));
    let d = Obj::new(c);
    d.set(sym("__name__"), s(name));
    d.set(sym("__qualname__"), st(format!("partial.{name}")));
    V::Obj(d)
}
// functools.partial, as CPython 3.13's C type: func/args/keywords are read-only C fields, a partial of a
// partial whose __dict__ was never made is flattened, and a program class may subclass it.
fn partial_class() -> Rc<Class> {
    PARTIAL.with(|slot| {
        if let Some(c) = slot.borrow().as_ref() {
            return c.clone();
        }
        let c = Class::new("partial", "functools", vec![bclass("object")], BT::Other);
        *slot.borrow_mut() = Some(c.clone());
        // its fields are data descriptors (see setattr)
        USER_DATA_DESC.store(true, std::sync::atomic::Ordering::Relaxed);
        let cw = Rc::downgrade(&c);
        c.set("__new__", func(sym("__new__"), move |a: Args| {
            let base = cw.upgrade().unwrap();
            let cls = match a.p.first() {
                Some(V::Class(k)) => k.clone(),
                Some(x) => return type_err(format!("partial.__new__(X): X is not a type object ({})", tn(x))),
                None => return type_err("partial.__new__(): not enough arguments".into()),
            };
            if !cls.is_sub(&base) {
                return type_err(format!("partial.__new__({0}): {0} is not a subtype of partial", cls.name));
            }
            if a.p.len() < 2 {
                return type_err("type 'partial' takes at least one argument".into());
            }
            let mut f = a.p[1].clone();
            let mut args = vec![];
            let kw = Rc::new(RefCell::new(Dict::new()));
            if let V::Obj(o) = &f {
                if o.cls.is_sub(&base) && o.dict_unset() {
                    let (ff, pa, pk) = pstate(o)?;
                    args = to_vec(&pa)?;
                    for (k, v) in dict_items_of(&pk)? {
                        Dict::set_cell(&kw, k, v)?;
                    }
                    f = ff;
                }
            }
            if !truth(&bi_callable(Args::p(vec![f.clone()]))?)? {
                return type_err("the first argument must be callable".into());
            }
            args.extend(a.p[2..].iter().cloned());
            for (n, v) in &a.k {
                Dict::set_cell(&kw, s(n), v.clone())?;
            }
            let o = Obj::new(cls);
            o.set(sym("__bv_slot_func"), f);
            o.set(sym("__bv_slot_args"), tup(args));
            o.set(sym("__bv_slot_keywords"), V::Dict(kw));
            Ok(V::Obj(o))
        }));
        c.set("__call__", func(sym("__call__"), |mut a: Args| {
            if a.p.is_empty() {
                return type_err("descriptor '__call__' of 'functools.partial' object needs an argument".into());
            }
            let me = a.p.remove(0);
            let (_, f, pa, pk) = pstate_of(&me, "__call__")?;
            let mut p = to_vec(&pa)?;
            p.append(&mut a.p);
            let mut k = kw_syms(&pk)?;
            if k.is_empty() {
                k = a.k;
            } else {
                for (n, v) in a.k {
                    match k.iter_mut().find(|(m, _)| *m == n) {
                        Some(e) => e.1 = v,
                        None => k.push((n, v)),
                    }
                }
            }
            call(&f, Args { p, k })
        }));
        c.set("__repr__", func(sym("__repr__"), |a: Args| {
            let me = a.p.first().cloned().unwrap_or(V::None);
            let (o, f, pa, pk) = pstate_of(&me, "__repr__")?;
            let id = Rc::as_ptr(&o) as usize;
            if PARTIAL_REPR.with(|r| r.borrow().contains(&id)) {
                return Ok(s("..."));
            }
            PARTIAL_REPR.with(|r| r.borrow_mut().push(id));
            let r = (|| {
                // what the object holds when repr starts: a repr that changes it (__setstate__) changes nothing here
                let args = to_vec(&pa)?;
                let kws = dict_items_of(&pk)?;
                let mut parts = vec![repr(&f)?];
                for x in &args {
                    parts.push(repr(x)?);
                }
                for (k, v) in &kws {
                    parts.push(format!("{}={}", to_str(k)?, repr(v)?));
                }
                let cls = V::Class(o.cls.clone());
                let module = to_str(&getattr(&cls, "__module__")?)?;
                let qualname = to_str(&getattr(&cls, "__qualname__")?)?;
                Ok(st(format!("{module}.{qualname}({})", parts.join(", "))))
            })();
            PARTIAL_REPR.with(|r| r.borrow_mut().retain(|x| *x != id));
            r
        }));
        c.set("__reduce__", func(sym("__reduce__"), |a: Args| {
            let me = a.p.first().cloned().unwrap_or(V::None);
            let (o, f, pa, pk) = pstate_of(&me, "__reduce__")?;
            let d = if o.dict_unset() { V::None } else { getattr(&me, "__dict__")? };
            Ok(tup(vec![V::Class(o.cls.clone()), tup(vec![f.clone()]), tup(vec![f, pa, pk, d])]))
        }));
        c.set("__setstate__", func(sym("__setstate__"), |a: Args| {
            if a.p.len() != 2 || !a.k.is_empty() {
                return type_err(format!("partial.__setstate__() takes exactly one argument ({} given)", a.p.len().saturating_sub(1)));
            }
            let (o, ..) = pstate_of(&a.p[0], "__setstate__")?;
            let state = &a.p[1];
            if !tuple_v(state) {
                return type_err("argument to __setstate__ must be a tuple".into());
            }
            let st4 = to_vec(state)?;
            if st4.len() != 4 {
                return type_err(format!("expected 4 items in state, got {}", st4.len()));
            }
            let (f, fa, kw, d) = (&st4[0], &st4[1], &st4[2], &st4[3]);
            if !truth(&bi_callable(Args::p(vec![f.clone()]))?)? || !tuple_v(fa) || !(matches!(kw, V::None) || dict_v(kw)) || !(matches!(d, V::None) || dict_v(d)) {
                return type_err("invalid partial state".into());
            }
            let fa = if matches!(fa, V::Tuple(_)) { fa.clone() } else { tup(to_vec(fa)?) };
            let kw = match kw {
                V::None => V::Dict(Rc::new(RefCell::new(Dict::new()))),
                V::Dict(_) => kw.clone(),
                _ => {
                    let nd = Rc::new(RefCell::new(Dict::new()));
                    for (k, v) in dict_items_of(kw)? {
                        Dict::set_cell(&nd, k, v)?;
                    }
                    V::Dict(nd)
                }
            };
            o.set(sym("__bv_slot_func"), f.clone());
            o.set(sym("__bv_slot_args"), fa);
            o.set(sym("__bv_slot_keywords"), kw);
            match d {
                V::None => o.clear_dict(),
                _ => o.set_dict(d.clone())?,
            }
            Ok(V::None)
        }));
        // partial(...) as a class attribute: CPython 3.13 warns that it will bind in a later version, and does not
        c.set("__get__", func(sym("__get__"), |a: Args| {
            if a.p.len() < 2 || a.p.len() > 3 {
                return type_err(format!("expected 1 or 2 arguments, got {}", a.p.len().saturating_sub(1)));
            }
            Ok(a.p[0].clone())
        }));
        let cg = func(sym("__class_getitem__"), |a: Args| {
            if a.p.len() != 2 || !a.k.is_empty() {
                return type_err(format!("partial.__class_getitem__() takes exactly one argument ({} given)", a.p.len().saturating_sub(1)));
            }
            let V::Class(k) = &a.p[0] else { return type_err("__class_getitem__ needs a class".into()) };
            Ok(class_alias(k, &a.p[1]))
        });
        if let V::Func(f) = &cg {
            f.kind.set(2);
        }
        c.set("__class_getitem__", cg);
        c.set("__bv_galias__", V::Bool(true));
        c.set("__bv_nodeldict__", V::Bool(true));
        c.set("__doc__", s("Create a new function with partial application of the given arguments\nand keywords."));
        for n in ["func", "args", "keywords"] {
            c.set(n, partial_member(n));
        }
        c
    })
}
fn kw_syms(v: &V) -> Result<Vec<(Sym, V)>, E> {
    let mut out = vec![];
    for (k, v) in dict_items_of(v)? {
        match &k {
            V::Str(t) => out.push((sym(&t.s), v)),
            _ => return type_err("keywords must be strings".into()),
        }
    }
    Ok(out)
}
// functools.partialmethod, as CPython 3.13's pure-Python class: a non-data descriptor (see getattr) that
// binds func through its own __get__ when it has one, and otherwise binds _make_unbound_method like a
// plain def. A program class may subclass it.
fn partialmethod_class() -> Rc<Class> {
    PARTIALMETHOD.with(|slot| {
        if let Some(c) = slot.borrow().as_ref() {
            return c.clone();
        }
        let c = Class::new("partialmethod", "functools", vec![bclass("object")], BT::Other);
        let cw = Rc::downgrade(&c);
        c.set("__init__", func(sym("__init__"), move |a: Args| {
            if a.p.len() < 2 {
                return type_err("partialmethod.__init__() missing 1 required positional argument: 'func'".into());
            }
            let me = a.p[0].clone();
            let funcv = a.p[1].clone();
            let args = tup(a.p[2..].to_vec());
            let kw = Rc::new(RefCell::new(Dict::new()));
            for (n, v) in &a.k {
                Dict::set_cell(&kw, s(n), v.clone())?;
            }
            let callable = truth(&bi_callable(Args::p(vec![funcv.clone()]))?)?;
            let has_get = match getattr(&funcv, "__get__") {
                Ok(_) => true,
                Err(E(e)) if is_exc(&e, "AttributeError") => false,
                Err(e) => return Err(e),
            };
            if !callable && !has_get {
                return type_err(format!("{} is not callable or a descriptor", repr(&funcv)?));
            }
            let nested = match &funcv {
                V::Obj(o) => cw.upgrade().is_some_and(|k| o.cls.is_sub(&k)),
                _ => false,
            };
            if nested {
                let f = getattr(&funcv, "func")?;
                let mut all_args = to_vec(&getattr(&funcv, "args")?)?;
                all_args.extend(a.p[2..].iter().cloned());
                let all_kw = Rc::new(RefCell::new(Dict::new()));
                for (k, v) in dict_items_of(&getattr(&funcv, "keywords")?)? {
                    Dict::set_cell(&all_kw, k, v)?;
                }
                for (n, v) in &a.k {
                    Dict::set_cell(&all_kw, s(n), v.clone())?;
                }
                setattr(&me, sym("func"), f)?;
                setattr(&me, sym("args"), tup(all_args))?;
                setattr(&me, sym("keywords"), V::Dict(all_kw))?;
            } else {
                setattr(&me, sym("func"), funcv)?;
                setattr(&me, sym("args"), args)?;
                setattr(&me, sym("keywords"), V::Dict(kw))?;
            }
            Ok(V::None)
        }));
        c.set("__repr__", func(sym("__repr__"), move |mut a: Args| {
            if a.p.len() != 1 {
                return type_err(format!("partialmethod.__repr__() takes 1 positional argument but {} were given", a.p.len()));
            }
            a.no_kw("partialmethod.__repr__")?;
            let me = &a.p[0];
            let cls = V::Class(type_of(me)?);
            let module = to_str(&getattr(&cls, "__module__")?)?;
            let qualname = to_str(&getattr(&cls, "__qualname__")?)?;
            let mut parts = vec![repr(&getattr(me, "func")?)?];
            for x in to_vec(&getattr(me, "args")?)? {
                parts.push(repr(&x)?);
            }
            for (k, v) in dict_items_of(&getattr(me, "keywords")?)? {
                parts.push(format!("{}={}", to_str(&k)?, repr(&v)?));
            }
            Ok(st(format!("{module}.{qualname}({})", parts.join(", "))))
        }));
        c.set("_make_unbound_method", func(sym("_make_unbound_method"), move |a: Args| {
            if a.p.len() != 1 || !a.k.is_empty() {
                return type_err(format!("partialmethod._make_unbound_method() takes 1 positional argument but {} were given", a.p.len()));
            }
            let me = a.p[0].clone();
            let funcv = getattr(&me, "func")?;
            let margs = getattr(&me, "args")?;
            let mkw = getattr(&me, "keywords")?;
            let isabs = match getattr(&me, "__isabstractmethod__") {
                Ok(v) => v,
                Err(E(e)) if is_exc(&e, "AttributeError") => V::Bool(false),
                Err(e) => return Err(e),
            };
            let m = func("_method", move |b: Args| {
                if b.p.is_empty() {
                    return type_err("_method() missing 1 required positional argument: 'cls_or_self'".into());
                }
                let mut p = vec![b.p[0].clone()];
                p.extend(to_vec(&margs)?);
                p.extend(b.p[1..].iter().cloned());
                let mut k = kw_syms(&mkw)?;
                for (n, v) in &b.k {
                    match k.iter_mut().find(|(m, _)| *m == *n) {
                        Some(e) => e.1 = v.clone(),
                        None => k.push((*n, v.clone())),
                    }
                }
                call(&funcv, Args { p, k })
            });
            if let V::Func(mf) = &m {
                mf.attrs.borrow_mut().push(("__isabstractmethod__", isabs));
                mf.attrs.borrow_mut().push(("__partialmethod__", me));
            }
            Ok(m)
        }));
        c.set("__get__", func(sym("__get__"), move |mut a: Args| {
            if a.p.len() < 2 || a.p.len() > 3 {
                return type_err(format!("partialmethod.__get__() takes from 2 to 3 positional arguments but {} were given", a.p.len()));
            }
            let me = a.p[0].clone();
            let obj = a.p[1].clone();
            let cls = if a.p.len() == 3 { a.p[2].clone() } else { a.kw("cls").unwrap_or(V::None) };
            a.no_kw("partialmethod.__get__")?;
            let funcv = getattr(&me, "func")?;
            let get = match getattr(&funcv, "__get__") {
                Ok(g) => Some(g),
                Err(E(e)) if is_exc(&e, "AttributeError") => None,
                Err(e) => return Err(e),
            };
            let mut result: Option<V> = None;
            if let Some(g) = get {
                if !is_partial_v(&funcv) {
                    let new_func = call(&g, Args::p(vec![obj.clone(), cls.clone()]))?;
                    if !is(&new_func, &funcv) {
                        let mut p = vec![new_func.clone()];
                        p.extend(to_vec(&getattr(&me, "args")?)?);
                        let part = functools_fn("partial", Args { p, k: kw_syms(&getattr(&me, "keywords")?)? })?;
                        match getattr(&new_func, "__self__") {
                            Ok(sv) => {
                                if let Err(e) = setattr(&part, sym("__self__"), sv) {
                                    if !is_exc(&e.0, "AttributeError") {
                                        return Err(e);
                                    }
                                }
                            }
                            Err(E(e)) if is_exc(&e, "AttributeError") => {}
                            Err(e) => return Err(e),
                        }
                        result = Some(part);
                    }
                }
            }
            if result.is_none() {
                let m = call_method(&me, "_make_unbound_method", Args::p(vec![]))?;
                result = Some(func_get(&m, obj, cls)?);
            }
            Ok(result.unwrap())
        }));
        let getter = func(sym("__isabstractmethod__"), move |a: Args| {
            let f = getattr(&a.p[0], "func")?;
            match getattr(&f, "__isabstractmethod__") {
                Ok(v) => Ok(v),
                Err(E(e)) if is_exc(&e, "AttributeError") => Ok(V::Bool(false)),
                Err(e) => Err(e),
            }
        });
        c.set("__isabstractmethod__", V::Prop(Rc::new((getter, V::None))));
        *slot.borrow_mut() = Some(c.clone());
        c
    })
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

// The same checks CPython's namedtuple makes, on the str values field_names was mapped with str().
fn py_ident(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c == '_' || c.is_alphabetic() => cs.all(|c| c == '_' || c.is_alphanumeric()),
        _ => false,
    }
}
fn py_keyword(s: &str) -> bool {
    matches!(s,
        "False" | "None" | "True" | "and" | "as" | "assert" | "async" | "await" | "break" | "class" |
        "continue" | "def" | "del" | "elif" | "else" | "except" | "finally" | "for" | "from" | "global" |
        "if" | "import" | "in" | "is" | "lambda" | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" |
        "try" | "while" | "with" | "yield")
}

// collections.namedtuple: the class CPython's collections.namedtuple builds, step for step. Fields are
// read-only class descriptors that read from the tuple the instance wraps; only the tuple holds the values
// (never instance attributes), so a subclass's __dict__ starts empty, as in CPython.
pub fn namedtuple(a: Args) -> R {
    namedtuple_with(a, true)
}
// For the runtime's own tuple types whose CPython name is dotted (time.struct_time): the type name is not a
// user's, so it skips the identifier check; the fields are still checked.
pub fn namedtuple_runtime_type(a: Args) -> R {
    namedtuple_with(a, false)
}
fn namedtuple_with(mut a: Args, check_name: bool) -> R {
    let rename = match a.kw("rename") {
        Some(v) => truth(&v)?,
        None => false,
    };
    let defaults = a.kw("defaults");
    let module = a.kw("module");
    let module_sym: Sym = match &module {
        Some(m) => sym(&to_str(m)?),
        None => "__main__",
    };
    a.no_kw("namedtuple")?;
    a.want("namedtuple", 2, 2)?;
    let name = sym(&to_str(&a.p[0])?);
    let mut fields: Vec<Sym> = match &a.p[1] {
        V::Str(x) => x.s.replace(',', " ").split_whitespace().map(sym).collect(),
        x => {
            let mut out = vec![];
            for f in to_vec(x)? {
                out.push(sym(&to_str(&f)?));
            }
            out
        }
    };
    if rename {
        let mut seen: Vec<Sym> = vec![];
        let original = fields.clone();
        for (index, f) in fields.iter_mut().enumerate() {
            if !py_ident(f) || py_keyword(f) || f.starts_with('_') || seen.contains(f) {
                *f = sym(&format!("_{index}"));
            }
            seen.push(original[index]);
        }
    }
    for f in std::iter::once(&name).filter(|_| check_name).chain(fields.iter()) {
        if !py_ident(f) {
            return val_err(format!("Type names and field names must be valid identifiers: {}", str_repr(f)));
        }
        if py_keyword(f) {
            return val_err(format!("Type names and field names cannot be a keyword: {}", str_repr(f)));
        }
    }
    let mut seen: Vec<Sym> = vec![];
    for f in &fields {
        if f.starts_with('_') && !rename {
            return val_err(format!("Field names cannot start with an underscore: {}", str_repr(f)));
        }
        if seen.contains(f) {
            return val_err(format!("Encountered duplicate field name: {}", str_repr(f)));
        }
        seen.push(*f);
    }
    let defaults_given = matches!(&defaults, Some(d) if !matches!(d, V::None));
    let dflt: Vec<V> = match &defaults {
        Some(d) if !matches!(d, V::None) => to_vec(d)?,
        _ => vec![],
    };
    if dflt.len() > fields.len() {
        return type_err("Got more default values than field names".into());
    }
    let mut fdd = Dict::new();
    for (k, v) in fields[fields.len() - dflt.len()..].iter().zip(dflt.iter()) {
        fdd.set(s(k), v.clone())?;
    }
    let fd = V::Dict(Rc::new(RefCell::new(fdd)));
    let arg_list = if fields.len() == 1 {
        format!("{},", fields[0])
    } else {
        fields.join(", ")
    };
    let c = Class::new(name, module_sym, vec![bclass("tuple")], BT::Tuple);
    *c.fields.borrow_mut() = fields.clone();
    c.set("__bv_namedtuple__", V::Bool(true));
    c.set("__slots__", tup(vec![]));
    c.set("_fields", tup(fields.iter().map(|f| s(f)).collect()));
    c.set("__match_args__", tup(fields.iter().map(|f| s(f)).collect()));
    c.set("__doc__", st(format!("{name}({arg_list})")));
    c.set("_field_defaults", fd);
    c.set("__module__", module.unwrap_or_else(|| s("__main__")));

    // __new__: built exactly once; a subclass call passes its own class, so the instance (and its repr)
    // belong to the class the call was made on.
    let cnew = c.clone();
    let fnew = fields.clone();
    let dnew = dflt.clone();
    let nd = dflt.len();
    let nf = fields.len();
    let nv = func("__new__", move |mut a: Args| {
        let cls = a.p.first().cloned().unwrap_or(V::Class(cnew.clone()));
        // CPython's errors name the namedtuple's own __new__ (P.__new__), also when called on a subclass
        let cname = format!("{}.__new__", cnew.name);
        if !a.p.is_empty() {
            a.p.remove(0);
        }
        let mut vals: Vec<V> = vec![V::Unbound; nf];
        if a.p.len() > nf {
            return type_err(format!("{}() takes {} positional arguments but {} were given", cname, nf + 1, a.p.len() + 1));
        }
        for (i, v) in a.p.drain(..).enumerate() {
            vals[i] = v;
        }
        for (k, v) in a.k.drain(..) {
            match fnew.iter().position(|f| *f == k) {
                Some(i) => {
                    if !matches!(vals[i], V::Unbound) {
                        return type_err(format!("{}() got multiple values for argument '{}'", cname, k));
                    }
                    vals[i] = v;
                }
                None => return type_err(format!("{}() got an unexpected keyword argument '{}'", cname, k)),
            }
        }
        let mut missing: Vec<String> = vec![];
        for i in 0..nf {
            if matches!(vals[i], V::Unbound) {
                if i + nd >= nf {
                    vals[i] = dnew[i + nd - nf].clone();
                } else {
                    missing.push(format!("'{}'", fnew[i]));
                }
            }
        }
        if !missing.is_empty() {
            let list = match missing.len() {
                1 => missing[0].clone(),
                2 => format!("{} and {}", missing[0], missing[1]),
                _ => format!("{}, and {}", missing[..missing.len() - 1].join(", "), missing.last().unwrap()),
            };
            return type_err(format!("{}() missing {} required positional argument{}: {}", cname, missing.len(), if missing.len() == 1 { "" } else { "s" }, list));
        }
        let oc = match cls {
            V::Class(k) => k.clone(),
            _ => cnew.clone(),
        };
        let o = Obj::new(oc);
        o.set_inner(Some(tup(vals)));
        Ok(V::Obj(o))
    });
    if let V::Func(f) = &nv {
        let mut fa = f.attrs.borrow_mut();
        fa.push(("__defaults__", if defaults_given { tup(dflt.clone()) } else { V::None }));
        let builtins = V::Dict(Rc::new(RefCell::new(Dict::new())));
        let mut g = Dict::new();
        g.set(s("__builtins__"), builtins.clone())?;
        fa.push(("__globals__", V::Dict(Rc::new(RefCell::new(g)))));
        fa.push(("__builtins__", builtins));
        fa.push(("__doc__", st(format!("Create new instance of {name}({arg_list})"))));
    }
    c.set("__new__", nv);

    // each field is a read-only descriptor holding its index in the wrapped tuple (V::NTF): getattr reads
    // the tuple directly for it, with no Args allocation and no call (see getattr in rt_call.rs).
    for (i, f) in fields.iter().enumerate() {
        c.set(*f, V::NTF(Rc::new(Ntf { idx: i as i64, doc: RefCell::new(st(format!("Alias for field number {i}"))) })));
    }

    let frepr = fields.clone();
    c.set("__repr__", func("__repr__", move |a: Args| {
        let me = a.p.first().cloned().unwrap_or(V::None);
        let cname = match &me {
            V::Obj(o) => o.cls.name.to_string(),
            _ => String::new(),
        };
        let inner = inner_of(&me).unwrap_or_else(|| tup(vec![]));
        let mut parts = vec![];
        for (i, f) in frepr.iter().enumerate() {
            let v = getitem(&inner, &V::Int(i as i64))?;
            parts.push(format!("{}={}", f, repr(&v)?));
        }
        Ok(st(format!("{}({})", cname, parts.join(", "))))
    }));
    let fas = fields.clone();
    c.set("_asdict", func("_asdict", move |a: Args| {
        let mut d = Dict::new();
        let me = a.p.first().cloned().unwrap_or(V::None);
        let inner = inner_of(&me).unwrap_or_else(|| tup(vec![]));
        for (i, f) in fas.iter().enumerate() {
            d.set(s(f), getitem(&inner, &V::Int(i as i64))?)?;
        }
        Ok(V::Dict(Rc::new(RefCell::new(d))))
    }));
    let frep = fields.clone();
    let rv = func("_replace", move |a: Args| {
        let me = a.p.first().cloned().unwrap_or(V::None);
        if a.p.len() > 1 {
            return type_err(format!("_replace() takes 1 positional argument but {} were given", a.p.len()));
        }
        let cls = match &me {
            V::Obj(o) => o.cls.clone(),
            _ => return type_err("descriptor '_replace' for 'tuple' objects doesn't apply to a 'NoneType' object".into()),
        };
        let inner = inner_of(&me).unwrap_or_else(|| tup(vec![]));
        let mut vals = vec![];
        for (i, f) in frep.iter().enumerate() {
            match a.k.iter().find(|(k, _)| k == f) {
                Some((_, v)) => vals.push(v.clone()),
                None => vals.push(getitem(&inner, &V::Int(i as i64))?),
            }
        }
        let unknown: Vec<Sym> = a.k.iter().filter_map(|(k, _)| if frep.contains(k) { None } else { Some(*k) }).collect();
        if !unknown.is_empty() {
            return type_err(format!("Got unexpected field names: {}", repr(&list(unknown.iter().map(|k| s(k)).collect()))?));
        }
        let o = Obj::new(cls);
        o.set_inner(Some(tup(vals)));
        Ok(V::Obj(o))
    });
    c.set("_replace", rv.clone());
    c.set("__replace__", rv);
    let cmk = c.clone();
    let fmk = fields.clone();
    let nfm = fields.len();
    let mk = func("_make", move |a: Args| {
        let cls = a.p.first().cloned().unwrap_or(V::Class(cmk.clone()));
        let it = a.p.get(1).cloned().unwrap_or(V::None);
        let vals = to_vec(&it)?;
        if vals.len() != nfm {
            return type_err(format!("Expected {} arguments, got {}", nfm, vals.len()));
        }
        let oc = match cls {
            V::Class(k) => k.clone(),
            _ => cmk.clone(),
        };
        let o = Obj::new(oc);
        o.set_inner(Some(tup(vals)));
        Ok(V::Obj(o))
    });
    if let V::Func(f) = &mk {
        f.kind.set(2);
    }
    c.set("_make", mk);
    c.set("__getnewargs__", func("__getnewargs__", |a: Args| Ok(inner_of(&a.p[0]).unwrap_or_else(|| tup(vec![])))));
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

// @dataclass, field, fields, asdict, astuple, replace, is_dataclass, KW_ONLY, InitVar, MISSING
// CPython 3.13's dataclasses.py, step for step: the class body's annotated names (Class.fields, with
// "InitVar:" and "KW_ONLY:" marks from the compiler; ClassVar names left out) after the fields of
// dataclass bases. Each class keeps a dict of Field objects as __dataclass_fields__ (InitVars included).
#[derive(Clone)]
struct DcOpts {
    init: bool,
    repr: bool,
    eq: bool,
    order: bool,
    unsafe_hash: bool,
    frozen: bool,
    kw_only: bool,
    slots: bool,
    weakref_slot: bool,
}
pub fn dataclass_fn(n: &str, mut a: Args) -> R {
    match n {
        "dataclass" => {
            let mut flag = |k: &str, d: bool| -> Result<bool, E> { a.kw(k).map(|x| truth(&x)).transpose().map(|v| v.unwrap_or(d)) };
            let o = DcOpts {
                init: flag("init", true)?,
                repr: flag("repr", true)?,
                eq: flag("eq", true)?,
                order: flag("order", false)?,
                unsafe_hash: flag("unsafe_hash", false)?,
                frozen: flag("frozen", false)?,
                kw_only: flag("kw_only", false)?,
                slots: flag("slots", false)?,
                weakref_slot: flag("weakref_slot", false)?,
            };
            let _ = a.kw("match_args");
            if !a.k.is_empty() {
                return type_err(format!("dataclass() got an unexpected keyword argument '{}'", a.k[0].0));
            }
            match a.p.first() {
                Some(V::Class(c)) => {
                    make_dataclass(c, &o)?;
                    Ok(a.p[0].clone())
                }
                _ => Ok(builtin("dataclass", move |a: Args| {
                    if let Some(V::Class(c)) = a.p.first() {
                        make_dataclass(c, &o)?;
                    }
                    Ok(a.p.first().cloned().unwrap_or(V::None))
                })),
            }
        }
        "field" => {
            if !a.p.is_empty() {
                return type_err(format!("field() takes 0 positional arguments but {} were given", a.p.len()));
            }
            let o = Obj::new(field_class());
            let d = a.kw("default");
            let df = a.kw("default_factory");
            if d.is_some() && df.is_some() {
                return val_err("cannot specify both default and default_factory".into());
            }
            o.set("default", d.unwrap_or_else(missing));
            o.set("default_factory", df.unwrap_or_else(missing));
            for (k, dv) in [("init", V::Bool(true)), ("repr", V::Bool(true)), ("hash", V::None), ("compare", V::Bool(true)), ("metadata", V::None), ("kw_only", missing())] {
                o.set(k, a.kw(k).unwrap_or(dv));
            }
            if let Some((k, _)) = a.k.first() {
                return type_err(format!("field() got an unexpected keyword argument '{k}'"));
            }
            if matches!(o.get("metadata"), Some(V::None)) {
                o.set("metadata", dict_new());
            }
            o.set("_field_type", s("_FIELD"));
            Ok(V::Obj(o))
        }
        "fields" => {
            let x = a1(&a, n)?;
            let c = match &x {
                V::Class(c) => c.clone(),
                V::Obj(o) => o.cls.clone(),
                _ => return type_err("must be called with a dataclass type or instance".into()),
            };
            match c.lookup("__dataclass_fields__") {
                Some(V::Dict(d)) => Ok(tup(d.borrow().items().into_iter().map(|(_, f)| f).filter(|f| field_kind(f) == "_FIELD").collect())),
                _ => type_err("must be called with a dataclass type or instance".into()),
            }
        }
        "is_dataclass" => {
            let x = a1(&a, n)?;
            let c = match &x {
                V::Class(c) => c.clone(),
                V::Obj(o) => o.cls.clone(),
                _ => return Ok(V::Bool(false)),
            };
            Ok(V::Bool(matches!(c.lookup("__dataclass_fields__"), Some(V::Dict(_)))))
        }
        "asdict" | "astuple" => {
            let x = a1(&a, n)?;
            if dc_fields(&x).is_none() || matches!(x, V::Class(_)) {
                return type_err(format!("{n}() should be called on dataclass instances"));
            }
            dc_copy(&x, n == "asdict")
        }
        "replace" => {
            let x = a.p.first().cloned().unwrap_or(V::None);
            let Some(fs) = dc_fields(&x).filter(|_| !matches!(x, V::Class(_))) else { return type_err("replace() should be called on dataclass instances".into()) };
            let V::Obj(o) = &x else { unreachable!() };
            let mut k: Vec<(Sym, V)> = a.k.clone();
            for f in fs {
                let name = field_name(&f);
                if !truth(&fget(&f, "init"))? {
                    if k.iter().any(|(n, _)| *n == name) {
                        return type_err(format!("field {name} is declared with init=False, it cannot be specified with replace()"));
                    }
                    continue;
                }
                if !k.iter().any(|(n, _)| *n == name) {
                    if field_kind(&f) == "_FIELD_INITVAR" {
                        if is_missing(&fget(&f, "default")) {
                            return type_err(format!("InitVar '{name}' must be specified with replace()"));
                        }
                        continue;
                    }
                    k.push((name, getattr(&x, name)?));
                }
            }
            instantiate(&o.cls, Args { p: vec![], k })
        }
        _ => Err(unsupported(&format!("dataclasses.{n}"))),
    }
}
thread_local! {
    static FIELD: RefCell<Option<(Rc<Class>, V, V, V)>> = RefCell::new(None);
}
// Field, MISSING, KW_ONLY, InitVar
fn dc_globals() -> (Rc<Class>, V, V, V) {
    FIELD.with(|a| {
        let mut a = a.borrow_mut();
        if a.is_none() {
            let f = Class::new("Field", "dataclasses", vec![bclass("object")], BT::Other);
            // its repr shows the field's type, which Brevi does not keep
            f.set("__repr__", builtin("__repr__", |_a: Args| Err(unsupported("repr() of a dataclasses.Field"))));
            let m = Class::new("_MISSING_TYPE", "dataclasses", vec![bclass("object")], BT::Other);
            let k = Class::new("_KW_ONLY_TYPE", "dataclasses", vec![bclass("object")], BT::Other);
            let iv = Class::new("InitVar", "dataclasses", vec![bclass("object")], BT::Other);
            *a = Some((f, V::Obj(Obj::new(m)), V::Obj(Obj::new(k)), V::Class(iv)));
        }
        a.clone().unwrap()
    })
}
fn field_class() -> Rc<Class> {
    dc_globals().0
}
fn missing() -> V {
    dc_globals().1
}
pub fn dataclass_attr(n: &str) -> Option<V> {
    let g = dc_globals();
    Some(match n {
        "MISSING" => g.1,
        "KW_ONLY" => g.2,
        "InitVar" => g.3,
        "Field" => V::Class(g.0),
        "FrozenInstanceError" => V::Class(exc_class("FrozenInstanceError")),
        _ => return None,
    })
}
fn is_missing(v: &V) -> bool {
    is(v, &missing())
}
fn fget(f: &V, k: &str) -> V {
    match f {
        V::Obj(o) => o.get(k).unwrap_or(V::None),
        _ => V::None,
    }
}
fn field_name(f: &V) -> Sym {
    match fget(f, "name") {
        V::Str(x) => sym(&x.s),
        _ => "",
    }
}
fn field_kind(f: &V) -> String {
    match fget(f, "_field_type") {
        V::Str(x) => x.s.clone(),
        _ => String::new(),
    }
}
// A dataclass instance's (or class's) fields, InitVars included, in order.
fn dc_fields(x: &V) -> Option<Vec<V>> {
    let c = match x {
        V::Class(c) => c.clone(),
        V::Obj(o) => o.cls.clone(),
        _ => return None,
    };
    match c.lookup("__dataclass_fields__") {
        Some(V::Dict(d)) => Some(d.borrow().items().into_iter().map(|(_, f)| f).collect()),
        _ => None,
    }
}
// asdict / astuple: dataclasses, lists, tuples (namedtuples too) and dicts copied deeply.
fn dc_copy(x: &V, as_dict: bool) -> R {
    if let (Some(fs), V::Obj(_)) = (dc_fields(x), x) {
        let mut d = Dict::new();
        let mut t = vec![];
        for f in fs.iter().filter(|f| field_kind(f) == "_FIELD") {
            let name = field_name(f);
            let v = dc_copy(&getattr(x, name)?, as_dict)?;
            if as_dict {
                d.set(s(name), v)?;
            } else {
                t.push(v);
            }
        }
        return Ok(if as_dict { V::Dict(Rc::new(RefCell::new(d))) } else { tup(t) });
    }
    Ok(match x {
        V::Obj(o) if o.cls.lookup("_fields").is_some() && inner_of(x).is_some() => {
            let mut v = vec![];
            for y in to_vec(x)? {
                v.push(dc_copy(&y, as_dict)?);
            }
            instantiate(&o.cls, Args::p(v))?
        }
        V::List(l) => {
            let mut v = vec![];
            for y in l.borrow().clone().iter() {
                v.push(dc_copy(y, as_dict)?);
            }
            list(v)
        }
        V::Tuple(t) => {
            let mut v = vec![];
            for y in t.iter() {
                v.push(dc_copy(y, as_dict)?);
            }
            tup(v)
        }
        V::Dict(d) => {
            let mut n = Dict::new();
            for (k, v) in d.borrow().items() {
                n.set(dc_copy(&k, as_dict)?, dc_copy(&v, as_dict)?)?;
            }
            V::Dict(Rc::new(RefCell::new(n)))
        }
        _ => deepcopy(x, &mut vec![])?,
    })
}
fn make_dataclass(c: &Rc<Class>, o: &DcOpts) -> Result<(), E> {
    let fc = field_class();
    // fields of dataclass bases, in reverse MRO order, then this class's own
    let mut fields: Vec<(Sym, V)> = vec![];
    let mro = c.mro.borrow().clone();
    for b in mro.iter().skip(1).rev() {
        if let Some(V::Dict(d)) = b.own("__dataclass_fields__") {
            for (k, f) in d.borrow().items() {
                let name = field_name(&f);
                let _ = k;
                match fields.iter_mut().find(|(n, _)| *n == name) {
                    Some(slot) => slot.1 = f,
                    None => fields.push((name, f)),
                }
            }
        }
    }
    let own: Vec<Sym> = c.fields.borrow().clone();
    let mut kw_only = o.kw_only;
    let mut seen_kw_only = false;
    for raw in own {
        let (kind, name) = match raw.split_once(':') {
            Some((k, n)) => (k, sym(n)),
            None => ("", raw),
        };
        if kind == "KW_ONLY" {
            if seen_kw_only {
                return type_err(format!("'{name}' is KW_ONLY, but KW_ONLY has already been specified"));
            }
            seen_kw_only = true;
            kw_only = true;
            c.attrs.borrow_mut().retain(|(k, _)| *k != name);
            continue;
        }
        let dflt = c.own(name);
        let f = match &dflt {
            Some(V::Obj(x)) if Rc::ptr_eq(&x.cls, &fc) => dflt.clone().unwrap(),
            _ => {
                let x = Obj::new(fc.clone());
                x.set("default", dflt.clone().unwrap_or_else(missing));
                x.set("default_factory", missing());
                for (k, dv) in [("init", V::Bool(true)), ("repr", V::Bool(true)), ("hash", V::None), ("compare", V::Bool(true)), ("kw_only", missing())] {
                    x.set(k, dv);
                }
                x.set("metadata", dict_new());
                V::Obj(x)
            }
        };
        let V::Obj(fo) = &f else { unreachable!() };
        fo.set("name", s(name));
        fo.set("_field_type", s(if kind == "InitVar" { "_FIELD_INITVAR" } else { "_FIELD" }));
        if is_missing(&fo.get("kw_only").unwrap_or(V::None)) {
            fo.set("kw_only", V::Bool(kw_only));
        }
        if kind == "InitVar" && !is_missing(&fget(&f, "default_factory")) {
            return type_err(format!("field {name} cannot have a default factory"));
        }
        // mutable defaults: anything unhashable
        let d = fget(&f, "default");
        if kind != "InitVar" && !is_missing(&d) {
            let unhashable = match &d {
                V::List(_) | V::Dict(_) | V::Set(_) => true,
                V::Obj(x) => matches!(x.cls.lookup("__hash__"), Some(V::None)),
                _ => false,
            };
            if unhashable {
                return val_err(format!("mutable default {} for field {name} is not allowed: use default_factory", repr(&V::Class(type_of(&d)?))?));
            }
        }
        // a field(...) in the class body leaves its default (or nothing) as the class attribute
        if let Some(V::Obj(x)) = &dflt {
            if Rc::ptr_eq(&x.cls, &fc) {
                c.attrs.borrow_mut().retain(|(k, _)| *k != name);
                if !is_missing(&d) {
                    c.attrs.borrow_mut().push((name, d.clone()));
                }
                class_changed();
            }
        }
        match fields.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = f,
            None => fields.push((name, f)),
        }
    }
    if o.slots {
        // CPython's dataclasses._add_slots: the field names not already stored in a base's slot, plus
        // __weakref__ when asked; the class's own field attributes leave its dict (their defaults still
        // feed the generated __init__ through the Field objects)
        if c.own("__slots__").is_some() {
            return type_err(format!("{} already specifies __slots__", c.name));
        }
        let mro = c.mro.borrow().clone();
        let mut inherited: Vec<Sym> = vec![];
        for b in mro.iter().skip(1).filter(|b| b.bt != BT::Object) {
            if let Some(sv) = b.own("__slots__") {
                slot_names(&sv, &mut inherited);
            }
        }
        let mut seen: Vec<Sym> = vec![];
        let mut new_slots: Vec<V> = vec![];
        for (n, f) in &fields {
            if field_kind(f) != "_FIELD_INITVAR" && !inherited.contains(n) && !seen.contains(n) {
                seen.push(*n);
                new_slots.push(s(*n));
            }
        }
        if o.weakref_slot && !inherited.iter().any(|k| *k == "__weakref__") {
            new_slots.push(s("__weakref__"));
        }
        for (n, f) in &fields {
            if field_kind(f) != "_FIELD_INITVAR" {
                c.attrs.borrow_mut().retain(|(k, _)| *k != *n);
            }
        }
        class_changed();
        c.set("__slots__", tup(new_slots));
    }
    let fd = dict_new();
    if let V::Dict(d) = &fd {
        for (n, f) in &fields {
            d.borrow_mut().set(s(n), f.clone())?;
        }
    }
    c.set("__dataclass_fields__", fd);
    let all: Vec<V> = fields.iter().map(|x| x.1.clone()).collect();
    let real: Vec<V> = all.iter().filter(|f| field_kind(f) == "_FIELD").cloned().collect();
    let names = |fs: &[V], key: &str| -> Result<Vec<Sym>, E> {
        let mut out = vec![];
        for f in fs {
            if truth(&fget(f, key))? {
                out.push(field_name(f));
            }
        }
        Ok(out)
    };
    let cname = c.name;
    if o.init && c.own("__init__").is_none() {
        // positional fields, then keyword-only ones
        let mut pos: Vec<V> = vec![];
        let mut kwo: Vec<V> = vec![];
        for f in &all {
            if truth(&fget(f, "init"))? {
                if truth(&fget(f, "kw_only"))? {
                    kwo.push(f.clone());
                } else {
                    pos.push(f.clone());
                }
            }
        }
        let mut seen: Option<Sym> = None;
        for f in &pos {
            let has = !is_missing(&fget(f, "default")) || !is_missing(&fget(f, "default_factory"));
            if has {
                seen = Some(field_name(f));
            } else if let Some(sd) = seen {
                return type_err(format!("non-default argument '{}' follows default argument '{sd}'", field_name(f)));
            }
        }
        let order: Vec<V> = pos.iter().chain(kwo.iter()).cloned().collect();
        let mut params: Vec<Sym> = vec!["self"];
        params.extend(order.iter().map(field_name));
        let sig = Sig { name: sym(&format!("{cname}.__init__")), params: Box::leak(params.into_boxed_slice()), npos: pos.len() + 1, npo: 0, star: false, kwstar: false };
        // the defaults bind fills in (a factory is called afterwards, once per missing value)
        let mut dflt: Vec<V> = vec![V::Unbound];
        let factory = V::Obj(Obj::new(Class::new("_HAS_DEFAULT_FACTORY_CLASS", "dataclasses", vec![bclass("object")], BT::Other)));
        for f in &order {
            let d = fget(f, "default");
            dflt.push(if !is_missing(&d) { d } else if !is_missing(&fget(f, "default_factory")) { factory.clone() } else { V::Unbound });
        }
        let frozen = o.frozen;
        let post = c.lookup("__post_init__").is_some();
        let all2 = all.clone();
        c.set("__init__", func("__init__", move |a: Args| {
            let vals = bind(&sig, &dflt, a)?;
            let me = vals[0].clone();
            let V::Obj(ob) = &me else { return Ok(V::None) };
            let mut initvars = vec![];
            for f in &all2 {
                let name = field_name(f);
                let given = sig.params.iter().position(|p| *p == name).map(|i| vals[i].clone());
                let v = match given {
                    Some(v) if is(&v, &factory) => call(&fget(f, "default_factory"), Args::p(vec![]))?,
                    Some(v) => v,
                    // init=False: its default, a factory's value, or nothing
                    None => {
                        let d = fget(f, "default");
                        let df = fget(f, "default_factory");
                        if !is_missing(&df) {
                            call(&df, Args::p(vec![]))?
                        } else if !is_missing(&d) {
                            d
                        } else {
                            continue;
                        }
                    }
                };
                if field_kind(f) == "_FIELD_INITVAR" {
                    initvars.push(v);
                    continue;
                }
                if frozen {
                    ob.set(name, v);
                } else {
                    setattr(&me, name, v)?;
                }
            }
            if post {
                call_method(&me, "__post_init__", Args::p(initvars))?;
            }
            Ok(V::None)
        }));
    }
    if o.repr && c.own("__repr__").is_none() {
        let fs = names(&real, "repr")?;
        c.set("__repr__", func("__repr__", move |a: Args| {
            let me = &a.p[0];
            let mut parts = vec![];
            for f in &fs {
                parts.push(format!("{}={}", f, repr(&getattr(me, f)?)?));
            }
            // CPython's dataclass repr uses self.__class__.__qualname__
            let qn = match me {
                V::Obj(o) => o.cls.qualname.get().to_string(),
                _ => tn(me),
            };
            Ok(st(format!("{qn}({})", parts.join(", "))))
        }));
    }
    let cmp = names(&real, "compare")?;
    let key = |fs: &Vec<Sym>, v: &V| -> R { Ok(tup(fs.iter().map(|f| getattr(v, f)).collect::<Result<Vec<_>, _>>()?)) };
    if o.eq && c.own("__eq__").is_none() {
        let fs = cmp.clone();
        c.set("__eq__", func("__eq__", move |a: Args| {
            let (x, y) = (&a.p[0], &a.p[1]);
            if is(x, y) {
                return Ok(V::Bool(true));
            }
            match (x, y) {
                (V::Obj(p), V::Obj(q)) if Rc::ptr_eq(&p.cls, &q.cls) => {
                    for f in &fs {
                        if !eq(&getattr(x, f)?, &getattr(y, f)?)? {
                            return Ok(V::Bool(false));
                        }
                    }
                    Ok(V::Bool(true))
                }
                _ => Ok(V::NotImpl),
            }
        }));
    }
    if o.order {
        for (nm, op) in [("__lt__", "<"), ("__le__", "<="), ("__gt__", ">"), ("__ge__", ">=")] {
            if c.own(nm).is_some() {
                return type_err(format!("Cannot overwrite attribute {nm} in class {cname}. Consider using functools.total_ordering"));
            }
            let fs = cmp.clone();
            c.set(nm, func(nm, move |a: Args| match (&a.p[0], &a.p[1]) {
                (V::Obj(x), V::Obj(y)) if Rc::ptr_eq(&x.cls, &y.cls) => Ok(V::Bool(rich(&key(&fs, &a.p[0])?, &key(&fs, &a.p[1])?, op)?)),
                _ => Ok(V::NotImpl),
            }));
        }
    }
    if o.frozen {
        let fs: Vec<Sym> = real.iter().map(field_name).collect();
        let cls = c.clone();
        let fs2 = fs.clone();
        let cls2 = c.clone();
        c.set("__setattr__", func("__setattr__", move |a: Args| {
            let n = to_str(&a.p[1])?;
            if matches!(&a.p[0], V::Obj(o) if Rc::ptr_eq(&o.cls, &cls)) || fs.iter().any(|f| *f == n) {
                return Err(exc("FrozenInstanceError", format!("cannot assign to field {}", repr(&a.p[1])?)));
            }
            if let V::Obj(o) = &a.p[0] {
                o.set(sym(&n), a.p[2].clone());
            }
            Ok(V::None)
        }));
        c.set("__delattr__", func("__delattr__", move |a: Args| {
            let n = to_str(&a.p[1])?;
            if matches!(&a.p[0], V::Obj(o) if Rc::ptr_eq(&o.cls, &cls2)) || fs2.iter().any(|f| *f == n) {
                return Err(exc("FrozenInstanceError", format!("cannot delete field {}", repr(&a.p[1])?)));
            }
            Err(unsupported("del on a frozen dataclass's subclass"))
        }));
    }
    // __hash__, by CPython's _hash_action table
    let explicit = matches!(c.own("__hash__"), Some(ref h) if !matches!(h, V::None)) || (c.own("__hash__").is_some() && c.own("__eq__").is_none());
    let add = match (o.unsafe_hash, o.eq, o.frozen, explicit) {
        (false, true, false, false) => {
            c.set("__hash__", V::None);
            false
        }
        (false, true, true, false) | (true, _, _, false) => true,
        (true, _, _, true) => return type_err(format!("Cannot overwrite attribute __hash__ in class {cname}")),
        _ => false,
    };
    if add {
        let mut fs = vec![];
        for f in &real {
            let h = fget(f, "hash");
            if if matches!(h, V::None) { truth(&fget(f, "compare"))? } else { truth(&h)? } {
                fs.push(field_name(f));
            }
        }
        c.set("__hash__", func("__hash__", move |a: Args| Ok(V::Int(py_hash(&key(&fs, &a.p[0])?)?))));
    }
    if c.own("__match_args__").is_none() {
        let mut m = vec![];
        for f in &all {
            if truth(&fget(f, "init"))? && !truth(&fget(f, "kw_only"))? {
                m.push(s(field_name(f)));
            }
        }
        c.set("__match_args__", tup(m));
    }
    Ok(())
}

// Library modules written elsewhere (re, json, decimal, ...) register here.
pub fn ext_module_call(m: &str, n: &str, a: Args) -> R {
    if m == "numpy" || m.starts_with("numpy.") {
        return np_module_call(m, n, a);
    }
    if matches!(m, "decimal" | "fractions" | "datetime" | "csv" | "html" | "statistics" | "threading" | "zoneinfo" | "tzlocal" | "importlib.metadata") {
        return ext_module_call2(m, n, a).unwrap();
    }
    match m {
        "re" if matches!(n, "error" | "PatternError") => re_error_new(a),
        "re" => re_fn(n, a),
        "json" | "json.decoder" if n == "JSONDecoder" => call(&V::Class(json_decoder_class()), a),
        "json" | "json.encoder" if n == "JSONEncoder" => call(&V::Class(json_encoder_class()), a),
        "json.decoder" if n == "scanstring" => json_scanstring(a),
        "json" => json_fn(n, a),
        "enum" => enum_fn(n, a),
        "_brevi_mt" => brevi_mt_fn(n, a),
        "_brevi_col" => brevi_col_fn(n, a),
        "_brevi_async" => brevi_async_fn(n, a),
        "_brevi_tok" => brevi_tok_fn(n, a),
        "_brevi_net" => brevi_net_fn(n, a),
        "_brevi_stdio" => brevi_stdio_fn(n, a),
        "_brevi_proc" => brevi_proc_fn(n, a),
        "_brevi_srv" => brevi_srv_fn(n, a),
        "_brevi_fs" => brevi_fs_fn(n, a),
        "_brevi_st" => brevi_st_fn(n, a),
        "_brevi_ort" => brevi_ort_fn(n, a),
        "_brevi_sqlite" => brevi_sqlite_fn(n, a),
        "_brevi_duckdb" => brevi_duckdb_fn(n, a),
        "_brevi_hf" => brevi_hf_fn(n, a),
        "hashlib" => hashlib_fn(n, a),
        "base64" => base64_fn(n, a),
        "binascii" => binascii_fn(n, a),
        "_brevi_term" if n == "columns" => Ok(V::Int(term_columns())),
        _ => Err(unsupported(&format!("{m}.{n}"))),
    }
}
pub fn ext_module_attr(m: &str, n: &str) -> Result<Option<V>, E> {
    if let Some(v) = np_module_attr(m, n) {
        return Ok(Some(v));
    }
    if let Some(v) = ext_module_attr2(m, n) {
        return Ok(Some(v));
    }
    Ok(match (m, n) {
        ("re", _) if re_flag(n).is_some() => re_flag(n),
        ("_brevi_sqlite", "cursor_next") => Some(cursor_next_func()),
        ("_brevi_sqlite", "cursor_fetchone") => Some(cursor_fetchone_func()),
        ("_brevi_sqlite", "conn_execute") => Some(conn_execute_func()),
        ("re", "error" | "PatternError") => Some(V::Class(exc_class("PatternError"))),
        ("json" | "json.decoder", "JSONDecodeError") => Some(V::Class(exc_class("JSONDecodeError"))),
        ("json" | "json.decoder", "JSONDecoder") => Some(V::Class(json_decoder_class())),
        ("json" | "json.encoder", "JSONEncoder") => Some(V::Class(json_encoder_class())),
        ("enum", "Enum") => Some(V::Class(enum_base())),
        ("enum", "IntEnum" | "StrEnum" | "Flag" | "IntFlag") => enum_class(n).map(V::Class),
        ("binascii", "Error") => Some(V::Class(exc_class("binascii.Error"))),
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
// atexit: the callbacks run last-registered first when the program ends (after an uncaught exception's
// traceback, as CPython's finalization runs them); one that raises is reported and the rest still run.
thread_local! {
    // CPython's list: unregister empties a slot (the count stays), the run goes from the last slot down
    static ATEXIT: RefCell<Vec<Option<(V, Args)>>> = RefCell::new(vec![]);
}
fn atexit_fn(n: &str, mut a: Args) -> R {
    match n {
        "register" => {
            if a.p.is_empty() {
                return type_err("register() takes at least 1 argument (0 given)".into());
            }
            let f = a.p.remove(0);
            if !truth(&bi_callable(Args::p(vec![f.clone()]))?)? {
                return type_err("the first argument must be callable".into());
            }
            ATEXIT.with(|x| x.borrow_mut().push(Some((f.clone(), a))));
            Ok(f)
        }
        "unregister" => {
            let f = a1(&a, "unregister")?;
            let n = ATEXIT.with(|x| x.borrow().len());
            for i in 0..n {
                let g = ATEXIT.with(|x| x.borrow().get(i).and_then(|e| e.as_ref().map(|e| e.0.clone())));
                if let Some(g) = g {
                    if eq(&g, &f)? {
                        ATEXIT.with(|x| x.borrow_mut()[i] = None);
                    }
                }
            }
            Ok(V::None)
        }
        "_clear" => {
            ATEXIT.with(|x| x.borrow_mut().clear());
            Ok(V::None)
        }
        "_ncallbacks" => Ok(V::Int(ATEXIT.with(|x| x.borrow().len()) as i64)),
        "_run_exitfuncs" => {
            atexit_run();
            Ok(V::None)
        }
        _ => Err(unsupported(&format!("atexit.{n}"))),
    }
}
fn atexit_run() {
    let n = ATEXIT.with(|x| x.borrow().len());
    for i in (0..n).rev() {
        let Some((f, a)) = ATEXIT.with(|x| x.borrow().get(i).and_then(|e| e.as_ref().map(|(f, a)| (f.clone(), Args { p: a.p.clone(), k: a.k.clone() })))) else { continue };
        if let Err(E(e)) = call(&f, a) {
            out_flush();
            let fr = repr(&f).unwrap_or_default();
            err_write(&format!("Exception ignored in atexit callback {fr}:\nTraceback (most recent call last):\n{}\n", exc_line(&e)));
        }
    }
    ATEXIT.with(|x| x.borrow_mut().clear());
}
pub fn run_main(f: fn() -> Result<(), E>) {
    // glibc hands freed memory back to the kernel and takes it again, zeroed, as soon as a program that
    // builds and drops many objects (an autograd graph each step) grows again: keep it instead.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        unsafe {
            mallopt(-1, i32::MAX); // M_TRIM_THRESHOLD
            mallopt(-3, 1 << 30); // M_MMAP_THRESHOLD
            mallopt(-8, 1); // M_ARENA_MAX: the program's thread uses the main heap
        }
    }
    let child = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        init_builtin_classes();
        ARGV.with(|a| *a.borrow_mut() = std::env::args().collect());
        // Exceptions arrive as Err, so a panic here is a bug in the runtime: keep what the program printed.
        let Ok(r) = std::panic::catch_unwind(f) else {
            out_flush();
            return 1;
        };
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
        atexit_run();
        unsafe { fin_exit(&*std::ptr::addr_of!(GL)) };
        close_live_gens();
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

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}
#[cfg(not(windows))]
extern "C" {
    fn clock_gettime(clk: i32, ts: *mut Timespec) -> i32;
}
// clock_gettime(CLOCK_MONOTONIC or CLOCK_PROCESS_CPUTIME_ID) in nanoseconds
pub fn clock_ns(cpu: bool) -> i64 {
    #[cfg(windows)]
    {
        // Windows: QPC is monotonic; GetProcessTimes gives the process's CPU time (100 ns units).
        #[repr(C)]
        struct Ft {
            lo: u32,
            hi: u32,
        }
        #[repr(C)]
        struct Pt {
            creation: Ft,
            exit: Ft,
            kernel: Ft,
            user: Ft,
        }
        extern "system" {
            fn GetCurrentProcess() -> *mut core::ffi::c_void;
            fn GetProcessTimes(h: *mut core::ffi::c_void, c: *mut Ft, e: *mut Ft, k: *mut Ft, u: *mut Ft) -> i32;
        }
        if cpu {
            let mut p = Pt { creation: Ft { lo: 0, hi: 0 }, exit: Ft { lo: 0, hi: 0 }, kernel: Ft { lo: 0, hi: 0 }, user: Ft { lo: 0, hi: 0 } };
            let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut p.creation, &mut p.exit, &mut p.kernel, &mut p.user) };
            if ok != 0 {
                let t = |f: &Ft| ((f.hi as u64) << 32 | f.lo as u64) as i64;
                return (t(&p.kernel) + t(&p.user)) * 100;
            }
            return 0;
        }
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        return START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64;
    }
    #[cfg(not(windows))]
    {
        #[cfg(target_os = "macos")]
        let clk = if cpu { 12 } else { 6 };
        #[cfg(not(target_os = "macos"))]
        let clk = if cpu { 2 } else { 1 };
        let mut ts = Timespec { sec: 0, nsec: 0 };
        unsafe { clock_gettime(clk, &mut ts) };
        ts.sec * 1_000_000_000 + ts.nsec
    }
}
