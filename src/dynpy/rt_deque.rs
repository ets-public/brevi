
// ---------- collections.deque ----------
// CPython's _collectionsmodule.c deque: maxlen trimming on every add, a mutation counter (`state`) that its
// iterators and its count/index/remove/__contains__ loops check after each step, and the errors CPython raises.

impl Deq {
    pub fn new(v: VecDeque<V>, max: Option<usize>) -> Deq {
        Deq { v, max, state: 0, sub: false }
    }
    // one append (right) or appendleft: the item that maxlen pushes out comes back, so the caller drops it
    // after the borrow is released (dropping can run user code)
    pub fn push(&mut self, x: V, left: bool) -> Option<V> {
        self.state = self.state.wrapping_add(1);
        if left {
            self.v.push_front(x);
        } else {
            self.v.push_back(x);
        }
        match self.max {
            Some(m) if self.v.len() > m => {
                if left {
                    self.v.pop_back()
                } else {
                    self.v.pop_front()
                }
            }
            _ => None,
        }
    }
    pub fn bump(&mut self) {
        self.state = self.state.wrapping_add(1);
    }
}

pub fn deq_v(v: VecDeque<V>, max: Option<usize>) -> V {
    V::Deque(Rc::new(RefCell::new(Deq::new(v, max))))
}

// an integer argument (rotate's n, a repeat count): an int, or a value with __index__
fn deq_int(x: &V) -> Result<i64, E> {
    match x {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        V::Obj(o) if o.cls.lookup("__index__").is_some() || matches!(inner_of(x), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => idx_of(x),
        _ => type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
    }
}

// deque(iterable=(), maxlen=None) and deque.__init__: maxlen must be an int (no __index__), at least 0
fn deque_args(mut a: Args) -> Result<(Option<V>, Option<usize>), E> {
    let it_kw = a.kw("iterable");
    let max_kw = a.kw("maxlen");
    if let Some((k, _)) = a.k.first() {
        return type_err(format!("deque() got an unexpected keyword argument '{k}'"));
    }
    if a.p.len() > 2 {
        return type_err(format!("deque() takes at most 2 arguments ({} given)", a.p.len()));
    }
    if (it_kw.is_some() && !a.p.is_empty()) || (max_kw.is_some() && a.p.len() > 1) {
        let which = if it_kw.is_some() && !a.p.is_empty() { "iterable" } else { "maxlen" };
        return type_err(format!("argument for deque() given by name ('{which}') and position ({})", if which == "iterable" { 1 } else { 2 }));
    }
    let src = it_kw.or_else(|| a.p.first().cloned());
    let max = match max_kw.or_else(|| a.p.get(1).cloned()) {
        None | Some(V::None) => None,
        Some(m) => {
            let n = match &m {
                V::Int(i) => *i,
                V::Bool(b) => *b as i64,
                V::Big(_) => return Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
                V::Obj(_) => match inner_of(&m) {
                    Some(V::Int(i)) => i,
                    Some(V::Bool(b)) => b as i64,
                    _ => return type_err("an integer is required".into()),
                },
                _ => return type_err("an integer is required".into()),
            };
            if n < 0 {
                return val_err("maxlen must be non-negative".into());
            }
            Some(n as usize)
        }
    };
    Ok((src, max))
}

pub fn col_deque(a: Args) -> R {
    let (src, max) = deque_args(a)?;
    let d = Rc::new(RefCell::new(Deq::new(VecDeque::new(), max)));
    if let Some(x) = src {
        deque_extend(&d, &x, false)?;
    }
    Ok(V::Deque(d))
}

fn deque_extend(d: &Rc<RefCell<Deq>>, x: &V, left: bool) -> Result<(), E> {
    // (a deque extended by itself reads a copy first, as CPython's deque_extend does)
    let xs = to_vec(x)?;
    for y in xs {
        let old = d.borrow_mut().push(y, left);
        drop(old);
    }
    Ok(())
}

// `d * n` and `d *= n`: the last maxlen items of n copies
fn deque_repeated(d: &Deq, n: i64) -> Result<VecDeque<V>, E> {
    let len = d.v.len();
    if n <= 0 || len == 0 {
        return Ok(VecDeque::new());
    }
    let total = (len as u128) * (n as u128);
    let keep = match d.max {
        Some(m) => total.min(m as u128),
        None => total,
    };
    if keep > (1u128 << 40) {
        return Err(exc("MemoryError", String::new()));
    }
    let start = total - keep;
    let mut out = VecDeque::with_capacity(keep as usize);
    for i in start..total {
        out.push_back(d.v[(i % len as u128) as usize].clone());
    }
    Ok(out)
}

// a repeat count, as sq_repeat takes it (PyNumber_AsSsize_t with OverflowError)
fn deque_count(n: &V) -> Result<i64, E> {
    match n {
        V::Big(_) => Err(exc("OverflowError", "cannot fit 'int' into an index-sized integer".into())),
        V::Int(_) | V::Bool(_) => deq_int(n),
        V::Obj(o) if o.cls.lookup("__index__").is_some() || matches!(inner_of(n), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => idx_of(n),
        _ => type_err(format!("can't multiply sequence by non-int of type '{}'", tn(n))),
    }
}

pub fn deque_mul(d: &Rc<RefCell<Deq>>, n: &V) -> R {
    let k = deque_count(n)?;
    let b = d.borrow();
    let v = deque_repeated(&b, k)?;
    Ok(deq_v(v, b.max))
}

pub fn deque_imul(d: &Rc<RefCell<Deq>>, n: &V) -> Result<(), E> {
    let k = deque_count(n)?;
    let v = deque_repeated(&d.borrow(), k)?;
    let old = {
        let mut b = d.borrow_mut();
        b.bump();
        std::mem::replace(&mut b.v, v)
    };
    drop(old);
    Ok(())
}

// `sub * n`, `n * sub` and `sub + other` for an instance of a deque subclass whose class does not define the
// operator: CPython's deque_repeat and deque_concat start from deque_copy, type(d)(d) or type(d)(d, maxlen),
// so the result is of the subclass (made by its own __init__), then repeat or extend it in place.
pub fn deque_sub_binop(a: &V, b: &V, op: &str) -> Option<R> {
    let (sub, other) = match op {
        "*" if matches!(inner_of(a), Some(V::Deque(_))) && is_intlike(b) => (a, b),
        "*" if matches!(inner_of(b), Some(V::Deque(_))) && is_intlike(a) => (b, a),
        "+" if matches!(inner_of(a), Some(V::Deque(_))) => (a, b),
        _ => return None,
    };
    Some((|| {
        if op == "+" && !matches!(other, V::Deque(_)) && !matches!(inner_of(other), Some(V::Deque(_))) {
            return type_err(format!("can only concatenate deque (not \"{}\") to deque", tn(other)));
        }
        let k = if op == "*" { Some(deque_count(other)?) } else { None };
        let new = deque_sub_copy(sub).unwrap_or_else(|| type_err("internal: deque subclass copy".into()))?;
        let Some(V::Deque(nd)) = inner_of(&new).or_else(|| Some(new.clone())) else {
            let V::Obj(o) = sub else { return type_err("internal: deque subclass copy".into()) };
            return type_err(format!("{}() must return a deque, not {}", o.cls.name, tn(&new)));
        };
        match k {
            Some(k) => deque_imul(&nd, &V::Int(k))?,
            None => deque_extend(&nd, other, false)?,
        }
        Ok(new)
    })())
}

// `d + other`: a deque of the left one's maxlen; other must be a deque (or a deque subclass instance)
pub fn deque_concat(d: &Rc<RefCell<Deq>>, other: &V) -> R {
    let o = match other {
        V::Deque(o) => o.clone(),
        _ => match inner_of(other) {
            Some(V::Deque(o)) => o,
            _ => return type_err(format!("can only concatenate deque (not \"{}\") to deque", tn(other))),
        },
    };
    let (v, max) = {
        let b = d.borrow();
        (b.v.clone(), b.max)
    };
    let n = Rc::new(RefCell::new(Deq::new(v, max)));
    let add: Vec<V> = o.borrow().v.iter().cloned().collect();
    for y in add {
        let old = n.borrow_mut().push(y, false);
        drop(old);
    }
    Ok(V::Deque(n))
}

// Python's == of an item and x, with the identity shortcut (PyObject_RichCompareBool), then the check that
// the deque did not change under the comparison
fn deque_hit(d: &Rc<RefCell<Deq>>, i: usize, x: &V, start: u64, mutated: &str) -> Result<Option<bool>, E> {
    let y = match d.borrow().v.get(i) {
        Some(y) => y.clone(),
        None => return Ok(None),
    };
    let hit = is(&y, x) || eq(&y, x)?;
    drop(y);
    if d.borrow().state != start {
        let kind = if mutated == "remove" { "IndexError" } else { "RuntimeError" };
        return Err(exc(kind, "deque mutated during iteration".into()));
    }
    Ok(Some(hit))
}

pub fn deque_contains(d: &Rc<RefCell<Deq>>, x: &V) -> Result<bool, E> {
    let (n, start) = {
        let b = d.borrow();
        (b.v.len(), b.state)
    };
    for i in 0..n {
        match deque_hit(d, i, x, start, "contains")? {
            Some(true) => return Ok(true),
            Some(false) => {}
            None => break,
        }
    }
    Ok(false)
}

// d[i] = x and del d[i] (an int index only: deques take no slices)
fn deque_index_arg(i: &V) -> Result<i64, E> {
    match i {
        V::Int(_) | V::Bool(_) | V::Big(_) => idx_of(i),
        V::Obj(o) if o.cls.lookup("__index__").is_some() || matches!(inner_of(i), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => idx_of(i),
        _ => type_err(format!("sequence index must be integer, not '{}'", tn(i))),
    }
}
pub fn deque_getitem(d: &Rc<RefCell<Deq>>, i: &V) -> R {
    let k = deque_index_arg(i)?;
    let b = d.borrow();
    let j = norm_idx(k, b.v.len(), "deque")?;
    Ok(b.v[j].clone())
}
pub fn deque_setitem(d: &Rc<RefCell<Deq>>, i: &V, x: V) -> Result<(), E> {
    let k = deque_index_arg(i)?;
    let old = {
        let mut b = d.borrow_mut();
        let j = norm_idx(k, b.v.len(), "deque")?;
        std::mem::replace(&mut b.v[j], x)
    };
    drop(old);
    Ok(())
}
pub fn deque_delitem(d: &Rc<RefCell<Deq>>, i: &V) -> Result<(), E> {
    let k = deque_index_arg(i)?;
    let old = {
        let mut b = d.borrow_mut();
        let j = norm_idx(k, b.v.len(), "deque")?;
        b.bump();
        b.v.remove(j)
    };
    drop(old);
    Ok(())
}

// iter(d) and reversed(d): live, as CPython's deque iterators (a change of the deque's length or order other
// than d[i] = x makes the next step raise RuntimeError)
pub fn deque_iter(d: &Rc<RefCell<Deq>>, rev: bool) -> It {
    let b = d.borrow();
    It::Deq(d.clone(), 0, b.v.len(), b.state, rev)
}
pub fn deque_iter_next(d: &Rc<RefCell<Deq>>, i: &mut usize, left: &mut usize, start: u64, rev: bool) -> Result<Option<V>, E> {
    let b = d.borrow();
    if b.state != start {
        *left = 0;
        return Err(exc("RuntimeError", "deque mutated during iteration".into()));
    }
    if *left == 0 {
        return Ok(None);
    }
    let n = b.v.len();
    let x = if rev { b.v[n - 1 - *i].clone() } else { b.v[*i].clone() };
    *i += 1;
    *left -= 1;
    Ok(Some(x))
}

// deque(...) == / < / ... deque(...): None when the other side is not a deque (CPython returns NotImplemented)
pub fn deque_pair(a: &V, b: &V) -> Option<(Vec<V>, Vec<V>)> {
    match (a, b) {
        (V::Deque(x), V::Deque(y)) => Some((x.borrow().v.iter().cloned().collect(), y.borrow().v.iter().cloned().collect())),
        _ => None,
    }
}

fn deque_copy_of(d: &Rc<RefCell<Deq>>) -> V {
    let b = d.borrow();
    deq_v(b.v.clone(), b.max)
}

// d.copy() / d.__copy__() of a deque subclass instance: type(d)(d) or type(d)(d, maxlen), as CPython's deque_copy
pub fn deque_sub_copy(o: &V) -> Option<R> {
    let V::Obj(ob) = o else { return None };
    let Some(V::Deque(d)) = inner_of(o) else { return None };
    let max = d.borrow().max;
    let mut p = vec![o.clone()];
    if let Some(m) = max {
        p.push(V::Int(m as i64));
    }
    Some(instantiate(&ob.cls, Args::p(p)))
}

pub fn deque_method(d: &Rc<RefCell<Deq>>, o: &V, name: &str, mut a: Args) -> R {
    if matches!(name, "pop" | "popleft" | "clear" | "copy" | "__copy__" | "reverse" | "__len__" | "__iter__" | "__reversed__" | "__repr__" | "__str__" | "__reduce__") {
        no_args("deque", name, &a)?;
    }
    let one = |a: &Args| -> Result<V, E> {
        if !a.k.is_empty() {
            return type_err(format!("deque.{name}() takes no keyword arguments"));
        }
        if a.p.len() != 1 {
            return type_err(format!("deque.{name}() takes exactly one argument ({} given)", a.p.len()));
        }
        Ok(a.p[0].clone())
    };
    Ok(match name {
        "append" | "appendleft" => {
            let x = one(&a)?;
            let old = d.borrow_mut().push(x, name == "appendleft");
            drop(old);
            V::None
        }
        "extend" | "extendleft" => {
            let x = one(&a)?;
            deque_extend(d, &x, name == "extendleft")?;
            V::None
        }
        "pop" | "popleft" => {
            let got = {
                let mut b = d.borrow_mut();
                let got = if name == "pop" { b.v.pop_back() } else { b.v.pop_front() };
                if got.is_some() {
                    b.bump();
                }
                got
            };
            match got {
                Some(v) => v,
                None => return Err(exc("IndexError", "pop from an empty deque".into())),
            }
        }
        "rotate" => {
            a.no_kw("rotate")?;
            a.want("rotate", 0, 1)?;
            let n = match a.p.first() {
                Some(x) => deq_int(x)?,
                None => 1,
            };
            let mut b = d.borrow_mut();
            let len = b.v.len() as i64;
            if len > 1 {
                b.bump();
                let k = n.rem_euclid(len) as usize;
                b.v.rotate_right(k);
            }
            V::None
        }
        "clear" => {
            let old = {
                let mut b = d.borrow_mut();
                if !b.v.is_empty() {
                    b.bump();
                }
                std::mem::take(&mut b.v)
            };
            drop(old);
            V::None
        }
        "copy" | "__copy__" => deque_copy_of(d),
        "reverse" => {
            d.borrow_mut().v.make_contiguous().reverse();
            V::None
        }
        "count" => {
            let x = one(&a)?;
            let (n, start) = {
                let b = d.borrow();
                (b.v.len(), b.state)
            };
            let mut k = 0;
            for i in 0..n {
                match deque_hit(d, i, &x, start, "count")? {
                    Some(true) => k += 1,
                    Some(false) => {}
                    None => break,
                }
            }
            V::Int(k)
        }
        "index" => {
            a.no_kw("index")?;
            a.want("index", 1, 3)?;
            let x = a.p[0].clone();
            let (n, start) = {
                let b = d.borrow();
                (b.v.len() as i64, b.state)
            };
            // start and stop as list.index() takes them (slice indices: None is not allowed)
            let bound = |i: usize, dflt: i64| -> Result<i64, E> {
                let v = match a.p.get(i) {
                    None => return Ok(dflt),
                    Some(V::Big(b)) => if b.neg { i64::MIN / 4 } else { i64::MAX / 4 },
                    Some(v @ (V::Int(_) | V::Bool(_))) => idx_of(v)?,
                    Some(v @ V::Obj(o)) if o.cls.lookup("__index__").is_some() || inner_of(v).is_some() => idx_of(v)?,
                    Some(_) => return type_err("slice indices must be integers or have an __index__ method".into()),
                };
                Ok(if v < 0 { (v + n).max(0) } else { v.min(n) })
            };
            let lo = bound(1, 0)?;
            let hi = bound(2, n)?;
            for i in lo..hi.max(lo) {
                match deque_hit(d, i as usize, &x, start, "index")? {
                    Some(true) => return Ok(V::Int(i)),
                    Some(false) => {}
                    None => break,
                }
            }
            return val_err(format!("{} is not in deque", repr(&x)?));
        }
        "remove" => {
            let x = one(&a)?;
            let (n, start) = {
                let b = d.borrow();
                (b.v.len(), b.state)
            };
            for i in 0..n {
                match deque_hit(d, i, &x, start, "remove")? {
                    Some(true) => {
                        let old = {
                            let mut b = d.borrow_mut();
                            b.bump();
                            b.v.remove(i)
                        };
                        drop(old);
                        return Ok(V::None);
                    }
                    Some(false) => {}
                    None => break,
                }
            }
            return val_err(format!("{} is not in deque", repr(&x)?));
        }
        "insert" => {
            a.no_kw("insert")?;
            if a.p.len() != 2 {
                return type_err(format!("insert expected 2 arguments, got {}", a.p.len()));
            }
            let i = deq_int(&a.p[0])?;
            let mut b = d.borrow_mut();
            if b.max == Some(b.v.len()) {
                return Err(exc("IndexError", "deque already at its maximum size".into()));
            }
            let n = b.v.len() as i64;
            let j = if i < 0 { (i + n).max(0) } else { i.min(n) } as usize;
            b.bump();
            b.v.insert(j, a.p[1].clone());
            V::None
        }
        "__init__" => {
            let (src, max) = deque_args(a)?;
            let old = {
                let mut b = d.borrow_mut();
                b.max = max;
                if !b.v.is_empty() {
                    b.bump();
                }
                std::mem::take(&mut b.v)
            };
            drop(old);
            if let Some(x) = src {
                deque_extend(d, &x, false)?;
            }
            V::None
        }
        "__len__" => V::Int(d.borrow().v.len() as i64),
        "__getitem__" => deque_getitem(d, &one(&a)?)?,
        "__setitem__" => {
            a.want(name, 2, 2)?;
            deque_setitem(d, &a.p[0], a.p[1].clone())?;
            V::None
        }
        "__delitem__" => {
            deque_delitem(d, &one(&a)?)?;
            V::None
        }
        "__contains__" => V::Bool(deque_contains(d, &one(&a)?)?),
        "__iter__" => new_iter(deque_iter(d, false), IterClass::Kind(ItKind::DequeIter)),
        "__reversed__" => new_iter(deque_iter(d, true), IterClass::Kind(ItKind::Reversed)),
        "__add__" => deque_concat(d, &one(&a)?)?,
        "__mul__" | "__rmul__" => deque_mul(d, &one(&a)?)?,
        "__iadd__" => {
            deque_extend(d, &one(&a)?, false)?;
            o.clone()
        }
        "__imul__" => {
            deque_imul(d, &one(&a)?)?;
            o.clone()
        }
        "__eq__" | "__ne__" | "__lt__" | "__le__" | "__gt__" | "__ge__" => {
            let x = one(&a)?;
            if !matches!(x, V::Deque(_)) {
                return Ok(V::NotImpl);
            }
            let op = match name {
                "__eq__" => "==",
                "__ne__" => "!=",
                "__lt__" => "<",
                "__le__" => "<=",
                "__gt__" => ">",
                _ => ">=",
            };
            V::Bool(cmp_op(o, &x, op)?)
        }
        "__hash__" => return type_err("unhashable type: 'collections.deque'".into()),
        "__repr__" | "__str__" => st(repr(o)?),
        _ => return bad_method(o, name),
    })
}

// collections._deque_iterator(deque, index=0), as CPython's dequeiter_new (keyword arguments are ignored there
// too): a new iterator moved on `index` steps. What its __reduce__ names, so what pickle and copy call.
pub fn deque_iter_new(a: Args) -> R {
    let n = a.p.len();
    if n == 0 {
        return type_err("function takes at least 1 argument (0 given)".into());
    }
    if n > 2 {
        return type_err(format!("function takes at most 2 arguments ({n} given)"));
    }
    let d = match &a.p[0] {
        V::Deque(d) => d.clone(),
        x => match inner_of(x) {
            Some(V::Deque(d)) => d,
            _ => return type_err(format!("argument 1 must be collections.deque, not {}", tn(x))),
        },
    };
    let index = match a.p.get(1) {
        Some(x) => deq_int(x)?,
        None => 0,
    };
    let it = new_iter(deque_iter(&d, false), IterClass::Kind(ItKind::DequeIter));
    for _ in 0..index.max(0) {
        let V::Iter(c) = &it else { break };
        let step = c.it.borrow_mut().next();
        if step?.is_none() {
            break;
        }
    }
    Ok(it)
}

// What pickle needs of a deque, as CPython's deque.__reduce__ gives it: (maxlen, state, a live iterator over
// it), and for an instance of a subclass its class. state is the instance's __dict__ (None when empty) and, if
// its classes have __slots__, (that, the slots that are set) as object.__getstate__ makes it. Refused for a
// class that changes how it pickles (__reduce__, __reduce_ex__, __getstate__, __getnewargs__) or how pickle
// refills it (extend, append): those are not run here.
pub fn deque_pickle_parts(v: &V) -> Result<(Option<Rc<Class>>, Option<usize>, V, V), E> {
    let (d, cls) = match v {
        V::Deque(d) => (d.clone(), None),
        V::Obj(o) => match o.inner() {
            Some(V::Deque(d)) => (d, Some(o.cls.clone())),
            _ => return type_err(format!("cannot pickle '{}' object", tn(v))),
        },
        _ => return type_err(format!("cannot pickle '{}' object", tn(v))),
    };
    let max = d.borrow().max;
    let it = new_iter(deque_iter(&d, false), IterClass::Kind(ItKind::DequeIter));
    let Some(c) = &cls else { return Ok((None, max, V::None, it)) };
    let mro = c.mro.borrow().clone();
    let own = mro.iter().filter(|k| !matches!(k.module, "builtins" | "collections"));
    for k in own.clone() {
        for n in ["__reduce__", "__reduce_ex__", "__getstate__", "__getnewargs__", "__getnewargs_ex__", "extend", "append"] {
            if k.own(n).is_some() {
                return Err(unsupported(&format!("pickling an instance of a deque subclass that defines {n}")));
            }
        }
    }
    let dict = if c.has_dict() {
        match getattr(v, "__dict__")? {
            V::Dict(x) if x.borrow().len() > 0 => V::Dict(x),
            _ => V::None,
        }
    } else {
        V::None
    };
    // copyreg._slotnames: each class's own __slots__, in MRO order, without __dict__ and __weakref__, private
    // names mangled
    let mut slots = Dict::new();
    for k in own {
        let names = match k.own("__slots__") {
            None => continue,
            Some(V::Str(x)) => vec![V::Str(x)],
            Some(x) => to_vec(&x)?,
        };
        for n in names {
            let V::Str(n) = n else { continue };
            let n = n.text();
            if n == "__dict__" || n == "__weakref__" {
                continue;
            }
            let name = if n.starts_with("__") && !n.ends_with("__") { format!("_{}{n}", k.name.trim_start_matches('_')) } else { n.to_string() };
            match getattr(v, &name) {
                Ok(x) => slots.set(st(name), x)?,
                Err(e) if is_exc(&e.0, "AttributeError") => {}
                Err(e) => return Err(e),
            }
        }
    }
    let state = if slots.len() > 0 { tup(vec![dict, V::Dict(Rc::new(RefCell::new(slots)))]) } else { dict };
    Ok((cls, max, state, it))
}

// pickle's BUILD for an instance of a deque subclass a pickle made: its own __setstate__(state), or else the
// __dict__ part set directly and the slots part with setattr, as CPython's load_build.
pub fn deque_pickle_setstate(o: &V, state: V) -> Result<(), E> {
    if let V::Obj(ob) = o {
        if ob.cls.mro.borrow().iter().any(|k| !matches!(k.module, "builtins" | "collections") && k.own("__setstate__").is_some()) {
            call_method(o, "__setstate__", Args::p(vec![state]))?;
            return Ok(());
        }
    }
    let (dict, slots) = match state {
        V::Tuple(t) if t.len() == 2 => (t[0].clone(), t[1].clone()),
        x => (x, V::None),
    };
    let V::Obj(ob) = o else { return Ok(()) };
    // (pickle.UnpicklingError in CPython; a pickle made here never holds one)
    let bad = |what: &str| unsupported(&format!("loading a pickled deque whose {what} is not a dictionary"));
    match &dict {
        V::None => {}
        V::Dict(x) => {
            for (k, x) in x.borrow().items() {
                let V::Str(k) = k else { return Err(unsupported("a pickled __dict__ key that is not a str")) };
                ob.set_res(sym(k.text()), x)?;
            }
        }
        _ => return Err(bad("state")),
    }
    match &slots {
        V::None => {}
        V::Dict(x) => {
            for (k, x) in x.borrow().items() {
                let V::Str(k) = k else { return Err(unsupported("a pickled slot name that is not a str")) };
                setattr(o, sym(k.text()), x)?;
            }
        }
        _ => return Err(bad("slot state")),
    }
    Ok(())
}
