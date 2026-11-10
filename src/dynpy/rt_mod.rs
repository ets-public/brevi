
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

fn lru_cache_wrap(f: V) -> V {
    let cache: Rc<RefCell<Dict>> = Rc::new(RefCell::new(Dict::new()));
    // CPython's update_wrapper copies __name__ and __qualname__ from the wrapped function.
    let (name, qualname) = match &f {
        V::Func(x) => (x.name.get(), x.qualname.get()),
        _ => ("wrapper", "wrapper"),
    };
    let c2 = cache.clone();
    let w = func(name, move |a: Args| {
        let mut key = a.p.clone();
        for (k, v) in &a.k {
            key.push(s(k));
            key.push(v.clone());
        }
        let key = tup(key);
        if let Some(v) = Dict::get_cell(&cache, &key)? {
            return Ok(v);
        }
        let r = call(&f, a)?;
        Dict::set_cell(&cache, key, r.clone())?;
        Ok(r)
    });
    if let V::Func(wf) = &w {
        let c3 = c2.clone();
        wf.attrs.borrow_mut().push(("cache_clear", builtin("cache_clear", move |_a: Args| {
            c3.borrow_mut().clear();
            Ok(V::None)
        })));
    }
    with_qualname(w, qualname)
}
// CPython's functools.update_wrapper: copy __qualname__ and every attribute in the wrapped function's
// __dict__ (__doc__, __module__, __annotations__, user attributes), and set __wrapped__.
fn update_wrapper(w: V, f: &V) -> V {
    let wf = match &w {
        V::Func(wf) => wf.clone(),
        _ => return w,
    };
    let V::Func(x) = f else {
        return w;
    };
    wf.name.set(x.name.get());
    wf.qualname.set(x.qualname.get());
    if !Rc::ptr_eq(&wf, x) {
        let mut a = wf.attrs.borrow_mut();
        for (k, v) in x.attrs.borrow().iter() {
            match a.iter_mut().find(|(n, _)| *n == *k) {
                Some(e) => e.1 = v.clone(),
                None => a.push((*k, v.clone())),
            }
        }
        if !a.iter().any(|(n, _)| *n == "__wrapped__") {
            a.push(("__wrapped__", f.clone()));
        }
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
                Ok(V::Ext(ExtRc::new(CmpKey { f: f.clone(), v: x })))
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
        "wraps" => {
            let f = a1(&a, name)?;
            // CPython's functools.wraps(f) is a decorator that applies update_wrapper to the wrapped def.
            builtin("wraps", move |a: Args| {
                let w = a1(&a, "wraps")?;
                Ok(update_wrapper(w, &f))
            })
        }
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
    shared_iter(move || {
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
        let grouper = shared_iter(move || {
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
            vec_iter(out)
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
            vec_iter(out)
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
            vec_iter(res.into_iter().map(tup).collect())
        }
        // The iterator tools are lazy, as in CPython: they pull from their inputs one item at a time, so they work
        // on infinite inputs, see items added after they were made, and run callbacks in CPython's order.
        "chain" => {
            let srcs = std::mem::take(&mut a.p);
            let mut k = 0;
            let mut cur: Option<It> = None;
            gen_iter(move || loop {
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
            gen_iter(move || loop {
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
            gen_iter(move || {
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
            gen_iter(move || {
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
            let mut its: Vec<Option<It>> = vec![];
            for x in &a.p {
                its.push(Some(iter(x)?));
            }
            let mut active = its.len();
            gen_iter(move || {
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
                Some(n) => vec_iter(vec![x; idx_of(&n)?.max(0) as usize]),
                None => gen_iter(move || Ok(Some(x.clone()))),
            }
        }
        "cycle" => {
            let mut it = Some(iter(&a1(&a, name)?)?);
            let mut saved: Vec<V> = vec![];
            let mut i = 0;
            gen_iter(move || {
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
            gen_iter(move || match it.next()? {
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
            gen_iter(move || loop {
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
            shared_iter(move || {
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
            gen_iter(move || loop {
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
            gen_iter(move || loop {
                let Some(x) = it.next()? else { return Ok(None) };
                let t = if matches!(f, V::None) { truth(&x)? } else { truth(&call(&f, Args::p(vec![x.clone()]))?)? };
                if !t {
                    return Ok(Some(x));
                }
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
                Some(e) => tup(vec![V::Class(type_of(&e)), e, V::None]),
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
        ("sys.stdin.buffer", "__iter__") => Ok(gen_iter(|| Ok(read_line_bytes().map(bytesv)))),
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
fn modfn(m: &'static str, n: &str) -> V {
    let n: Sym = sym(n);
    builtin(n, move |a: Args| module_call(m, n, a))
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
pub fn module_attr(m: &'static str, name: &str) -> R {
    let v = match (m, name) {
        ("sys", "stdin") => V::Module(&ModName("sys.stdin")),
        ("sys", "stdout") => V::Module(&ModName("sys.stdout")),
        ("sys", "stderr") => V::Module(&ModName("sys.stderr")),
        ("sys", "argv") => list(ARGV.with(|a| a.borrow().iter().map(|x| s(x)).collect())),
        ("sys", "maxsize") => V::Int(i64::MAX),
        ("sys", "platform") => s("linux"),
        ("sys", "byteorder") => s("little"),
        ("sys", "version_info") => tup(vec![V::Int(3), V::Int(13), V::Int(0)]),
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
        ("typing" | "typing_extensions" | "collections.abc", _) => typing_attr(name),
        ("itertools", "chain") => {
            let f = modfn("itertools", "chain");
            if let V::Func(ff) = &f {
                ff.attrs.borrow_mut().push(("from_iterable", modfn("itertools", "chain.from_iterable")));
            }
            f
        }
        ("dataclasses", "MISSING" | "KW_ONLY" | "InitVar" | "Field" | "FrozenInstanceError") => dataclass_attr(name).unwrap(),
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
    let module = match a.kw("module") {
        Some(m) => sym(&to_str(&m)?),
        None => "__main__",
    };
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
    if dflt.len() > fields.len() {
        return type_err("Got more default values than field names".into());
    }
    let mut fdd = Dict::new();
    for (k, v) in fields[fields.len() - dflt.len()..].iter().zip(dflt.iter()) {
        fdd.set(s(k), v.clone())?;
    }
    let fd = V::Dict(Rc::new(RefCell::new(fdd)));
    let c = Class::new(name, module, vec![bclass("tuple")], BT::Tuple);
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
        o.set_inner(Some(tup(vals)));
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
    c.set("__doc__", st(format!("{}({})", name, fields.join(", "))));
    c.set("_field_defaults", fd);
    let c6 = c.clone();
    c.set("_make", builtin("_make", move |a: Args| {
        let it = a.p.last().cloned().unwrap_or(V::None);
        instantiate(&c6, Args::p(to_vec(&it)?))
    }));
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
                return val_err(format!("mutable default {} for field {name} is not allowed: use default_factory", repr(&V::Class(type_of(&d)))?));
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
                        if !rich(&getattr(x, f)?, &getattr(y, f)?, "==")? {
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
        ("re", "error") => Some(V::Class(exc_class("error"))),
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
