
// ---------- equality and ordering ----------

pub fn num_f(v: &V) -> Option<f64> {
    match v {
        V::Int(i) => Some(*i as f64),
        V::Bool(b) => Some(*b as i64 as f64),
        V::Float(f) => Some(*f),
        V::Big(b) => Some(b.to_f64()),
        _ => None,
    }
}
fn is_intlike(v: &V) -> bool {
    matches!(v, V::Int(_) | V::Bool(_) | V::Big(_))
}
fn ival(v: &V) -> Option<i64> {
    match v {
        V::Int(i) => Some(*i),
        V::Bool(b) => Some(*b as i64),
        _ => None,
    }
}
// int vs float compared exactly (as CPython does).
fn cmp_int_float(i: &V, f: f64) -> Option<std::cmp::Ordering> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() {
        return Some(if f > 0.0 { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
    }
    let b = big_of(i).unwrap();
    let fl = f.floor();
    let fb = Big::from_f64(fl);
    match b.cmp(&fb) {
        std::cmp::Ordering::Equal => {
            if f > fl {
                Some(std::cmp::Ordering::Less)
            } else {
                Some(std::cmp::Ordering::Equal)
            }
        }
        o => Some(o),
    }
}

// `d.keys()` / `d.items()` beside a set or another such view: compared as sets, as in CPython.
fn view_pair(a: &V, b: &V) -> Result<Option<(V, V)>, E> {
    let setlike = |v: &V| matches!(v, V::Set(_) | V::FSet(_)) || matches!(v, V::View(x) if x.1 == 0 || x.1 == 2);
    let is_view = |v: &V| matches!(v, V::View(x) if x.1 == 0 || x.1 == 2);
    if !(setlike(a) && setlike(b) && (is_view(a) || is_view(b))) {
        return Ok(None);
    }
    let as_set = |v: &V| -> Result<V, E> {
        Ok(match v {
            V::Set(_) | V::FSet(_) => v.clone(),
            _ => V::FSet(Rc::new(PSet::from_iter(to_vec(v)?)?)),
        })
    };
    Ok(Some((as_set(a)?, as_set(b)?)))
}
// The dict a dict_items view wraps (`kind` 2). Two dict_items views compare item by item in
// CPython (`d1.items() <= d2.items()`), never by hashing the items the way set views do.
fn items_view_dict(v: &V) -> Option<Rc<RefCell<Dict>>> {
    if let V::View(x) = v {
        if x.1 != 2 {
            return None;
        }
        if let V::Dict(d) = &x.0 {
            return Some(d.clone());
        }
        if let Some(i) = inner_of(&x.0) {
            if let V::Dict(d) = i {
                return Some(d);
            }
        }
    }
    None
}
// CPython's dict_items.__contains__: look the key up in the dict, then compare the stored value
// with the given one, the stored value on the left; an identical value is a hit (no __eq__).
fn items_view_contains(d: &Rc<RefCell<Dict>>, x: &V) -> Result<bool, E> {
    // CPython accepts tuple subclasses here, which Brevi holds as an object around a tuple.
    let x = match inner_of(x) {
        Some(i) => i,
        None => x.clone(),
    };
    let (k, v) = match &x {
        V::Tuple(t) if t.len() == 2 => (t[0].clone(), t[1].clone()),
        _ => return Ok(false),
    };
    // The borrow ends before __eq__ runs: a user __eq__ may change the dict.
    let found = d.borrow().get(&k)?;
    match found {
        Some(w) => Ok(is(&w, &v) || eq(&w, &v)?),
        None => Ok(false),
    }
}
fn items_view_eq(a: &Rc<RefCell<Dict>>, b: &Rc<RefCell<Dict>>) -> Result<bool, E> {
    if Rc::ptr_eq(a, b) {
        return Ok(true);
    }
    let items = {
        let a = a.borrow();
        if a.n != b.borrow().n {
            return Ok(false);
        }
        a.items()
    };
    for (k, v) in items {
        if !items_view_contains(b, &tup(vec![k, v]))? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn items_view_subset(sub: &Rc<RefCell<Dict>>, sup: &Rc<RefCell<Dict>>) -> Result<bool, E> {
    let items = sub.borrow().items();
    for (k, v) in items {
        if !items_view_contains(sup, &tup(vec![k, v]))? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn items_view_rich(a: &Rc<RefCell<Dict>>, b: &Rc<RefCell<Dict>>, op: &str) -> Result<bool, E> {
    let (na, nb) = (a.borrow().n, b.borrow().n);
    if Rc::ptr_eq(a, b) {
        return Ok(matches!(op, "<=" | ">="));
    }
    Ok(match op {
        "<" => {
            if na >= nb {
                return Ok(false);
            }
            items_view_subset(a, b)?
        }
        "<=" => {
            if na > nb {
                return Ok(false);
            }
            items_view_subset(a, b)?
        }
        ">" => {
            if na <= nb {
                return Ok(false);
            }
            items_view_subset(b, a)?
        }
        ">=" => {
            if na < nb {
                return Ok(false);
            }
            items_view_subset(b, a)?
        }
        _ => unreachable!(),
    })
}
pub fn eq(a: &V, b: &V) -> Result<bool, E> {
    if matches!(a, V::View(_)) || matches!(b, V::View(_)) {
        if let (Some(x), Some(y)) = (items_view_dict(a), items_view_dict(b)) {
            return items_view_eq(&x, &y);
        }
        if let Some((x, y)) = view_pair(a, b)? {
            return eq(&x, &y);
        }
    }
    Ok(match (a, b) {
        (V::Int(x), V::Int(y)) => x == y,
        (V::Str(x), V::Str(y)) => Rc::ptr_eq(x, y) || x.s == y.s,
        (V::None, V::None) => true,
        (V::None, _) | (_, V::None) if !matches!(a, V::Obj(_)) && !matches!(b, V::Obj(_)) => false,
        (V::Float(x), V::Float(y)) => x == y,
        (x, y) if is_intlike(x) && is_intlike(y) => match (ival(x), ival(y)) {
            (Some(p), Some(q)) => p == q,
            _ => big_of(x).unwrap() == big_of(y).unwrap(),
        },
        (x, V::Float(f)) if is_intlike(x) => cmp_int_float(x, *f) == Some(std::cmp::Ordering::Equal),
        (V::Float(f), y) if is_intlike(y) => cmp_int_float(y, *f) == Some(std::cmp::Ordering::Equal),
        (V::List(x), V::List(y)) => {
            if Rc::ptr_eq(x, y) {
                return Ok(true);
            }
            if x.borrow().len() != y.borrow().len() {
                return Ok(false);
            }
            // item by item against the live lists: an item's __eq__ may change either list (CPython checks
            // the lengths again when the items differ)
            let mut i = 0;
            loop {
                let (p, q) = {
                    let (bx, by) = (x.borrow(), y.borrow());
                    if i >= bx.len() || i >= by.len() {
                        return Ok(bx.len() == by.len());
                    }
                    (bx[i].clone(), by[i].clone())
                };
                if !(is(&p, &q) || eq(&p, &q)?) {
                    let (bx, by) = (x.borrow(), y.borrow());
                    return Ok((i >= bx.len() || i >= by.len()) && bx.len() == by.len());
                }
                i += 1;
            }
        }
        (V::Tuple(x), V::Tuple(y)) => Tup::ptr_eq(x, y) || seq_eq(x, y)?,
        (V::Dict(x), V::Dict(y)) => {
            if Rc::ptr_eq(x, y) {
                return Ok(true);
            }
            if x.borrow().n != y.borrow().n {
                return Ok(false);
            }
            // no borrow is held while a key's or value's __eq__ runs: it may clear either dict (bpo-27945)
            let items = x.borrow().items();
            let mut ok = true;
            for (k, v) in items {
                let found = y.borrow().get(&k)?;
                match found {
                    Some(w) => {
                        if !eq(&v, &w)? {
                            ok = false;
                            break;
                        }
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            ok
        }
        (V::View(x), V::View(y)) if x.1 == 3 && y.1 == 3 => eq(&x.0, &y.0)?,
        (V::View(x), V::Dict(_)) if x.1 == 3 => eq(&x.0, b)?,
        (V::Dict(_), V::View(y)) if y.1 == 3 => eq(a, &y.0)?,
        (V::Set(_) | V::FSet(_), V::Set(_) | V::FSet(_)) => {
            let (x, y) = (set_of(a).unwrap(), set_of(b).unwrap());
            x.used == y.used && x.keys().iter().all(|k| y.contains(k).unwrap_or(false))
        }
        (V::Range(x), V::Range(y)) => {
            let (lx, ly) = (range_len(a), range_len(b));
            lx == ly && (lx == 0 || (x.0 == y.0 && (lx == 1 || x.2 == y.2)))
        }
        (V::Deque(x), V::Deque(y)) => {
            let (x, y): (Vec<V>, Vec<V>) = (x.borrow().v.iter().cloned().collect(), y.borrow().v.iter().cloned().collect());
            seq_eq(&x, &y)?
        }
        (V::Obj(_), _) | (_, V::Obj(_)) => {
            if let V::Obj(o) = a {
                if let Some(f) = o.cls.lookup("__eq__") {
                    let r = call(&f, Args::p(vec![a.clone(), b.clone()]))?;
                    if !matches!(r, V::NotImpl) {
                        return truth(&r);
                    }
                } else if let Some(i) = inner_of(a) {
                    return eq(&i, b);
                }
            }
            if let V::Obj(o) = b {
                if let Some(f) = o.cls.lookup("__eq__") {
                    let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                    if !matches!(r, V::NotImpl) {
                        return truth(&r);
                    }
                } else if let Some(i) = inner_of(b) {
                    return eq(a, &i);
                }
            }
            is(a, b)
        }
        (V::Ext(x), _) => match x.eq(b) {
            Some(r) => r,
            None => match b {
                V::Ext(y) => y.eq(a).unwrap_or(false),
                _ => false,
            },
        },
        (_, V::Ext(y)) => y.eq(a).unwrap_or(false),
        _ => is(a, b),
    })
}
fn seq_eq(x: &[V], y: &[V]) -> Result<bool, E> {
    if x.len() != y.len() {
        return Ok(false);
    }
    for (p, q) in x.iter().zip(y) {
        if !(is(p, q) || eq(p, q)?) {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn ne(a: &V, b: &V) -> Result<bool, E> {
    if let V::Obj(o) = a {
        if let Some(f) = o.cls.lookup("__ne__") {
            return truth(&call(&f, Args::p(vec![a.clone(), b.clone()]))?);
        }
    }
    Ok(!eq(a, b)?)
}
pub fn is(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::None, V::None) => true,
        (V::Bool(x), V::Bool(y)) => x == y,
        (V::Int(x), V::Int(y)) => x == y && (-5..=256).contains(x),
        (V::Str(x), V::Str(y)) => Rc::ptr_eq(x, y) || (x.s == y.s && x.s.len() <= 1),
        (V::List(x), V::List(y)) => Rc::ptr_eq(x, y),
        (V::Tuple(x), V::Tuple(y)) => Tup::ptr_eq(x, y) || (x.is_empty() && y.is_empty()),
        (V::Dict(x), V::Dict(y)) => Rc::ptr_eq(x, y),
        (V::Set(x), V::Set(y)) => Rc::ptr_eq(x, y),
        (V::FSet(x), V::FSet(y)) => Rc::ptr_eq(x, y),
        (V::Func(x), V::Func(y)) => Rc::ptr_eq(x, y),
        (V::Class(x), V::Class(y)) => Rc::ptr_eq(x, y),
        (V::Obj(x), V::Obj(y)) => Rc::ptr_eq(x, y),
        (V::Deque(x), V::Deque(y)) => Rc::ptr_eq(x, y),
        (V::Iter(x), V::Iter(y)) => Rc::ptr_eq(x, y),
        (V::Ext(x), V::Ext(y)) => ExtRc::ptr_eq(x, y),
        (V::Module(x), V::Module(y)) => x == y,
        (V::NotImpl, V::NotImpl) | (V::Ellipsis, V::Ellipsis) => true,
        _ => false,
    }
}

fn cmp_err(op: &str, a: &V, b: &V) -> E {
    exc("TypeError", format!("'{op}' not supported between instances of '{}' and '{}'", tn(a), tn(b)))
}
// Ordering of two values for <, <=, >, >= (None when unordered, like NaN).
pub fn order(a: &V, b: &V, op: &str) -> Result<Option<std::cmp::Ordering>, E> {
    use std::cmp::Ordering::*;
    Ok(match (a, b) {
        (V::Int(x), V::Int(y)) => Some(x.cmp(y)),
        (V::Float(x), V::Float(y)) => x.partial_cmp(y),
        (V::Str(x), V::Str(y)) => Some(x.s.cmp(&y.s)),
        (x, y) if is_intlike(x) && is_intlike(y) => match (ival(x), ival(y)) {
            (Some(p), Some(q)) => Some(p.cmp(&q)),
            _ => Some(big_of(x).unwrap().cmp(&big_of(y).unwrap())),
        },
        (x, V::Float(f)) if is_intlike(x) => cmp_int_float(x, *f),
        (V::Float(f), y) if is_intlike(y) => cmp_int_float(y, *f).map(|o| o.reverse()),
        (V::List(x), V::List(y)) => {
            let (x, y) = (x.borrow().clone(), y.borrow().clone());
            seq_order(&x, &y, op)?
        }
        (V::Tuple(x), V::Tuple(y)) => seq_order(x, y, op)?,
        (V::Set(_) | V::FSet(_), V::Set(_) | V::FSet(_)) => {
            let (x, y) = (set_of(a).unwrap(), set_of(b).unwrap());
            let sub = x.keys().iter().all(|k| y.contains(k).unwrap_or(false));
            let sup = y.keys().iter().all(|k| x.contains(k).unwrap_or(false));
            match (sub, sup) {
                (true, true) => Some(Equal),
                (true, false) => Some(Less),
                (false, true) => Some(Greater),
                _ => None,
            }
        }
        (V::Obj(o), _) | (_, V::Obj(o)) => {
            // rich comparison through __lt__/__gt__/__le__/__ge__
            return Err(cmp_err(op, a, b));
        }
        (V::Ext(x), _) => match x.cmp(b) {
            Some(o) => Some(o),
            None => match b {
                V::Ext(y) => match y.cmp(a) {
                    Some(o) => Some(o.reverse()),
                    None => return Err(cmp_err(op, a, b)),
                },
                _ => return Err(cmp_err(op, a, b)),
            },
        },
        (_, V::Ext(y)) => match y.cmp(a) {
            Some(o) => Some(o.reverse()),
            None => return Err(cmp_err(op, a, b)),
        },
        _ => return Err(cmp_err(op, a, b)),
    })
}
fn seq_order(x: &[V], y: &[V], op: &str) -> Result<Option<std::cmp::Ordering>, E> {
    for (p, q) in x.iter().zip(y) {
        if is(p, q) || eq(p, q)? {
            continue;
        }
        // first differing item decides, through the operator itself
        let lt = rich(p, q, "<")?;
        if lt {
            return Ok(Some(std::cmp::Ordering::Less));
        }
        let gt = rich(p, q, ">")?;
        return Ok(if gt { Some(std::cmp::Ordering::Greater) } else { None });
    }
    Ok(Some(x.len().cmp(&y.len())))
}
fn swap_op(op: &str) -> &'static str {
    match op {
        "<" => ">",
        ">" => "<",
        "<=" => ">=",
        _ => "<=",
    }
}
fn dunder(op: &str) -> &'static str {
    match op {
        "<" => "__lt__",
        ">" => "__gt__",
        "<=" => "__le__",
        _ => "__ge__",
    }
}
// a OP b for the four ordering operators, with __lt__ & co. on objects.
pub fn rich(a: &V, b: &V, op: &str) -> Result<bool, E> {
    if let (V::Int(x), V::Int(y)) = (a, b) {
        return Ok(match op {
            "<" => x < y,
            ">" => x > y,
            "<=" => x <= y,
            _ => x >= y,
        });
    }
    if matches!(a, V::View(_)) || matches!(b, V::View(_)) {
        if let (Some(x), Some(y)) = (items_view_dict(a), items_view_dict(b)) {
            return items_view_rich(&x, &y, op);
        }
        if let Some((x, y)) = view_pair(a, b)? {
            return rich(&x, &y, op);
        }
    }
    if let V::Ext(x) = a {
        if let Some(r) = x.rich(a, op, b, false) {
            return r;
        }
    }
    if let V::Ext(y) = b {
        if let Some(r) = y.rich(b, op, a, true) {
            return r;
        }
    }
    if matches!(a, V::Obj(_)) || matches!(b, V::Obj(_)) {
        if let V::Obj(o) = a {
            if let Some(f) = o.cls.lookup(dunder(op)) {
                let r = call(&f, Args::p(vec![a.clone(), b.clone()]))?;
                if !matches!(r, V::NotImpl) {
                    return truth(&r);
                }
            }
        }
        if let V::Obj(o) = b {
            if let Some(f) = o.cls.lookup(dunder(swap_op(op))) {
                let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                if !matches!(r, V::NotImpl) {
                    return truth(&r);
                }
            }
        }
        let (ia, ib) = (inner_of(a), inner_of(b));
        if ia.is_some() || ib.is_some() {
            return rich(&ia.unwrap_or(a.clone()), &ib.unwrap_or(b.clone()), op);
        }
        return Err(cmp_err(op, a, b));
    }
    use std::cmp::Ordering::*;
    let o = order(a, b, op)?;
    Ok(match (op, o) {
        (_, None) => false,
        ("<", Some(o)) => o == Less,
        (">", Some(o)) => o == Greater,
        ("<=", Some(o)) => o != Greater,
        (_, Some(o)) => o != Less,
    })
}
pub fn lt(a: &V, b: &V) -> Result<bool, E> {
    rich(a, b, "<")
}
pub fn cmp_op(a: &V, b: &V, op: &str) -> Result<bool, E> {
    match op {
        "==" => eq(a, b),
        "!=" => ne(a, b),
        "<" | ">" | "<=" | ">=" => rich(a, b, op),
        "in" => contains(b, a),
        "not in" => Ok(!contains(b, a)?),
        "is" => Ok(is(a, b)),
        _ => Ok(!is(a, b)),
    }
}

// ---------- arithmetic ----------

fn op_err(op: &str, a: &V, b: &V) -> E {
    exc("TypeError", format!("unsupported operand type(s) for {op}: '{}' and '{}'", tn(a), tn(b)))
}
fn odunder(op: &str) -> (&'static str, &'static str) {
    match op {
        "+" => ("__add__", "__radd__"),
        "-" => ("__sub__", "__rsub__"),
        "*" => ("__mul__", "__rmul__"),
        "/" => ("__truediv__", "__rtruediv__"),
        "//" => ("__floordiv__", "__rfloordiv__"),
        "%" => ("__mod__", "__rmod__"),
        "**" => ("__pow__", "__rpow__"),
        "&" => ("__and__", "__rand__"),
        "|" => ("__or__", "__ror__"),
        "^" => ("__xor__", "__rxor__"),
        "<<" => ("__lshift__", "__rlshift__"),
        ">>" => ("__rshift__", "__rrshift__"),
        "@" => ("__matmul__", "__rmatmul__"),
        _ => ("", ""),
    }
}
fn obj_binop(a: &V, b: &V, op: &str) -> Option<R> {
    let (l, r) = odunder(op);
    if let V::Obj(o) = a {
        if let Some(f) = o.cls.lookup(l) {
            match call2(&f, a.clone(), b.clone()) {
                Ok(V::NotImpl) => {}
                x => return Some(x),
            }
        }
    }
    if let V::Obj(o) = b {
        if let Some(f) = o.cls.lookup(r) {
            match call2(&f, b.clone(), a.clone()) {
                Ok(V::NotImpl) => {}
                x => return Some(x),
            }
        }
    }
    if let V::Ext(x) = a {
        if let Some(r) = x.binop(a, op, b, false) {
            return Some(r);
        }
    }
    if let V::Ext(x) = b {
        if let Some(r) = x.binop(b, op, a, true) {
            return Some(r);
        }
    }
    None
}
// Borrows a big int instead of copying it (its limbs can be megabytes).
fn big_cow(a: &V) -> std::borrow::Cow<'_, Big> {
    match a {
        V::Big(b) => std::borrow::Cow::Borrowed(&**b),
        _ => std::borrow::Cow::Owned(big_of(a).unwrap()),
    }
}
fn big2<'a>(a: &'a V, b: &'a V) -> (std::borrow::Cow<'a, Big>, std::borrow::Cow<'a, Big>) {
    (big_cow(a), big_cow(b))
}
fn zdiv(f: bool) -> E {
    if f {
        exc("ZeroDivisionError", "float division by zero".into())
    } else {
        exc("ZeroDivisionError", "integer division or modulo by zero".into())
    }
}
fn fmod(x: f64, y: f64) -> f64 {
    let m = x % y;
    if m != 0.0 && ((m < 0.0) != (y < 0.0)) {
        m + y
    } else if m == 0.0 {
        0.0f64.copysign(y)
    } else {
        m
    }
}
fn ffloordiv(x: f64, y: f64) -> f64 {
    let m = fmod(x, y);
    let d = (x - m) / y;
    if d == 0.0 {
        0.0f64.copysign(x / y)
    } else {
        let f = d.floor();
        if d - f > 0.5 {
            f + 1.0
        } else {
            f
        }
    }
}
// float arithmetic, the inner loop of numeric code: inline at the call, where `op` is a constant
#[inline(always)]
fn float_fast(a: &V, b: &V, op: &str) -> Option<V> {
    let (x, y) = match (a, b) {
        (V::Float(x), V::Float(y)) => (*x, *y),
        (V::Float(x), V::Int(y)) => (*x, *y as f64),
        (V::Int(x), V::Float(y)) => (*x as f64, *y),
        _ => return None,
    };
    match op {
        "+" => Some(V::Float(x + y)),
        "-" => Some(V::Float(x - y)),
        "*" => Some(V::Float(x * y)),
        "/" if y != 0.0 => Some(V::Float(x / y)),
        _ => None,
    }
}
#[inline]
pub fn binop(a: &V, b: &V, op: &str) -> R {
    if let Some(r) = float_fast(a, b, op) {
        return Ok(r);
    }
    binop_slow(a, b, op)
}
#[inline(never)]
fn binop_slow(a: &V, b: &V, op: &str) -> R {
    // fast paths
    if let (V::Int(x), V::Int(y)) = (a, b) {
        let (x, y) = (*x, *y);
        match op {
            "+" => {
                if let Some(r) = x.checked_add(y) {
                    return Ok(V::Int(r));
                }
            }
            "-" => {
                if let Some(r) = x.checked_sub(y) {
                    return Ok(V::Int(r));
                }
            }
            "*" => {
                if let Some(r) = x.checked_mul(y) {
                    return Ok(V::Int(r));
                }
            }
            "//" => {
                if y == 0 {
                    return Err(zdiv(false));
                }
                if !(x == i64::MIN && y == -1) {
                    let q = x / y;
                    return Ok(V::Int(if (x % y != 0) && ((x < 0) != (y < 0)) { q - 1 } else { q }));
                }
            }
            "%" => {
                if y == 0 {
                    return Err(zdiv(false));
                }
                if y == -1 {
                    return Ok(V::Int(0));
                }
                let m = x % y;
                return Ok(V::Int(if m != 0 && ((m < 0) != (y < 0)) { m + y } else { m }));
            }
            "/" => {
                if y == 0 {
                    return Err(exc("ZeroDivisionError", "division by zero".into()));
                }
                if x.unsigned_abs() < (1 << 53) && y.unsigned_abs() < (1 << 53) {
                    return Ok(V::Float(x as f64 / y as f64));
                }
            }
            "&" => return Ok(V::Int(x & y)),
            "|" => return Ok(V::Int(x | y)),
            "^" => return Ok(V::Int(x ^ y)),
            ">>" => {
                if y < 0 {
                    return val_err("negative shift count".into());
                }
                return Ok(V::Int(if y >= 64 { if x < 0 { -1 } else { 0 } } else { x >> y }));
            }
            "<<" => {
                if y < 0 {
                    return val_err("negative shift count".into());
                }
                if y < 63 {
                    if let Some(r) = x.checked_mul(1i64 << y) {
                        return Ok(V::Int(r));
                    }
                }
            }
            "**" => {
                if y >= 0 {
                    if let Ok(e) = u32::try_from(y) {
                        if let Some(r) = x.checked_pow(e) {
                            return Ok(V::Int(r));
                        }
                    }
                } else {
                    return Ok(V::Float((x as f64).powf(y as f64)));
                }
            }
            _ => {}
        }
    }
    if let (V::Float(x), V::Float(y)) = (a, b) {
        return float_op(*x, *y, op, a, b);
    }
    match (a, b) {
        (x, y) if is_intlike(x) && is_intlike(y) => {
            let (pc, qc) = big2(x, y);
            let (p, q): (&Big, &Big) = (&pc, &qc);
            match op {
                "+" => return Ok(p.add(&q).norm()),
                "-" => return Ok(p.sub(&q).norm()),
                "*" => return Ok(p.mul(&q).norm()),
                "//" => {
                    if q.is_zero() {
                        return Err(zdiv(false));
                    }
                    return Ok(p.divmod(&q).0.norm());
                }
                "%" => {
                    if q.is_zero() {
                        return Err(zdiv(false));
                    }
                    return Ok(p.divmod(&q).1.norm());
                }
                "/" => {
                    if q.is_zero() {
                        return Err(exc("ZeroDivisionError", "division by zero".into()));
                    }
                    return Ok(V::Float(big_truediv(&p, &q)?));
                }
                "**" => {
                    if q.neg {
                        return Ok(V::Float(p.to_f64().powf(q.to_f64())));
                    }
                    let e = q.to_i64().ok_or_else(|| exc("OverflowError", "exponent too large".into()))?;
                    // 2 ** n (and 4 ** n, ...) is a shift: squaring a huge number over and over is slow.
                    if let Some(b) = p.to_i64().filter(|b| *b > 0 && (*b as u64).is_power_of_two()) {
                        let k = (b as u64).trailing_zeros() as u64;
                        return Ok(Big::from_i64(1).shl((k * e as u64) as usize).norm());
                    }
                    return Ok(p.pow(e as u64).norm());
                }
                "<<" => {
                    let n = q.to_i64().unwrap_or(i64::MAX);
                    if n < 0 {
                        return val_err("negative shift count".into());
                    }
                    return Ok(p.shl(n as usize).norm());
                }
                ">>" => {
                    let n = q.to_i64().unwrap_or(i64::MAX);
                    if n < 0 {
                        return val_err("negative shift count".into());
                    }
                    // Past the top bit: 0, or -1 for a negative number.
                    if n as u64 >= p.bits() as u64 + 1 {
                        return Ok(V::Int(if p.neg { -1 } else { 0 }));
                    }
                    return Ok(p.shr(n as usize).norm());
                }
                "&" | "|" | "^" => {
                    if let (Some(x), Some(y)) = (p.to_i128(), q.to_i128()) {
                        let r = match op {
                            "&" => x & y,
                            "|" => x | y,
                            _ => x ^ y,
                        };
                        return Ok(Big::from_i128(r).norm());
                    }
                    return Ok(p.bitop(&q, op).norm());
                }
                _ => {}
            }
        }
        (x, y) if num_f(x).is_some() && num_f(y).is_some() && !matches!(x, V::Ext(_)) && !matches!(y, V::Ext(_)) && (matches!(x, V::Float(_)) || matches!(y, V::Float(_))) => {
            if matches!(op, "&" | "|" | "^" | "<<" | ">>") {
                return Err(op_err(op, a, b));
            }
            let fx = num_f(x).unwrap();
            let fy = num_f(y).unwrap();
            if let V::Big(bb) = x {
                if fx.is_infinite() {
                    return Err(exc("OverflowError", "int too large to convert to float".into()));
                }
            }
            return float_op(fx, fy, op, a, b);
        }
        (V::Str(x), V::Str(y)) if op == "+" => {
            let mut s = String::with_capacity(x.s.len() + y.s.len());
            s.push_str(&x.s);
            s.push_str(&y.s);
            return Ok(V::Str(Rc::new(S { s, ascii: x.ascii && y.ascii })));
        }
        (V::List(x), V::List(y)) if op == "+" => {
            let mut v = x.borrow().clone();
            v.extend(y.borrow().iter().cloned());
            return Ok(list(v));
        }
        (V::Tuple(x), V::Tuple(y)) if op == "+" => {
            let mut v = x.to_vec();
            v.extend(y.iter().cloned());
            return Ok(tup(v));
        }
        (V::Str(_) | V::List(_) | V::Tuple(_), n) | (n, V::Str(_) | V::List(_) | V::Tuple(_)) if op == "*" && is_intlike(n) => {
            let (sq, n) = if is_intlike(a) { (b, a) } else { (a, b) };
            let k = ival(n).unwrap_or(0).max(0) as usize;
            let len = match sq {
                V::Str(x) => x.s.len(),
                V::List(x) => x.borrow().len(),
                V::Tuple(x) => x.len(),
                _ => 0,
            };
            // `[0] * sys.maxsize`: CPython raises MemoryError instead of trying.
            if len.checked_mul(k).map_or(true, |t| t > (1usize << 40)) {
                return Err(exc("MemoryError", String::new()));
            }
            return Ok(match sq {
                V::Str(x) => V::Str(Rc::new(S { s: x.s.repeat(k), ascii: x.ascii })),
                V::List(x) => {
                    let x = x.borrow();
                    let mut v = Vec::with_capacity(x.len() * k);
                    for _ in 0..k {
                        v.extend(x.iter().cloned());
                    }
                    list(v)
                }
                V::Tuple(x) => {
                    let mut v = Vec::with_capacity(x.len() * k);
                    for _ in 0..k {
                        v.extend(x.iter().cloned());
                    }
                    tup(v)
                }
                _ => unreachable!(),
            });
        }
        (V::Str(f), _) if op == "%" => return Ok(st(percent_format(&f.s, b)?)),
        (V::Set(_) | V::FSet(_), V::Set(_) | V::FSet(_)) if matches!(op, "|" | "&" | "-" | "^") => {
            let r = set_op(&set_of(a).unwrap(), &set_of(b).unwrap(), op)?;
            return Ok(if matches!(a, V::FSet(_)) { V::FSet(Rc::new(r)) } else { V::Set(Rc::new(RefCell::new(r))) });
        }
        // CPython's dictviews_or: build the left side into an empty set, then
        // update it with the right side. The left side is built by iterating
        // (a set's table is copied, a plain dict's keys reuse its stored
        // hashes); the right side is merged table-wise only when it is a real
        // set, with the same fast path for a plain dict. Materialising a view
        // into a table first and bulk-merging it gives the wrong iteration
        // order, e.g. {1:1, 2:2}.items() | {1:1, 2:2, 3:3}.items().
        (V::View(x), _) | (_, V::View(x)) if (x.1 == 0 || x.1 == 2) && op == "|" => {
            let mut r = match a {
                V::Set(_) | V::FSet(_) => set_of(a).unwrap().copy(),
                V::View(v) if v.1 == 0 => match &v.0 {
                    V::Dict(d) if d.borrow().kind == 0 => {
                        let ks = d.borrow().keys_h();
                        PSet::from_dict(ks)
                    }
                    _ => PSet::from_iter(to_vec(a)?)?,
                },
                _ => PSet::from_iter(to_vec(a)?)?,
            };
            match b {
                V::Set(_) | V::FSet(_) => r.merge(&set_of(b).unwrap()),
                V::Dict(d) if d.borrow().kind == 0 => {
                    let ks = d.borrow().keys_h();
                    r.update_dict(ks)
                }
                _ => {
                    for k in to_vec(b)? {
                        r.add(k)?;
                    }
                }
            }
            return Ok(V::Set(Rc::new(RefCell::new(r))));
        }
        (V::View(x), _) | (_, V::View(x)) if (x.1 == 0 || x.1 == 2) && matches!(op, "&" | "-" | "^") => {
            let to_set = |v: &V| -> Result<PSet, E> {
                match v {
                    V::Set(_) | V::FSet(_) => Ok(set_of(v).unwrap()),
                    _ => PSet::from_iter(to_vec(v)?),
                }
            };
            let r = set_op(&to_set(a)?, &to_set(b)?, op)?;
            return Ok(V::Set(Rc::new(RefCell::new(r))));
        }
        (V::Dict(x), V::Dict(y)) if op == "|" => {
            let mut d = x.borrow().copy();
            for (k, v) in y.borrow().items() {
                d.set(k, v)?;
            }
            return Ok(V::Dict(Rc::new(RefCell::new(d))));
        }
        (V::View(x), _) if x.1 == 3 && op == "|" => return binop(&x.0, b, op),
        (_, V::View(y)) if y.1 == 3 && op == "|" => return binop(a, &y.0, op),
        (V::Dict(x), V::Dict(y)) if (op == "+" || op == "-") && x.borrow().kind == 2 => {
            return counter_op(&x.borrow(), &y.borrow(), op);
        }
        (V::Deque(x), V::Deque(y)) if op == "+" => {
            let mut v = x.borrow().v.clone();
            v.extend(y.borrow().v.iter().cloned());
            return Ok(V::Deque(Rc::new(RefCell::new(Deq { v, max: x.borrow().max }))));
        }
        _ => {}
    }
    if let Some(r) = obj_binop(a, b, op) {
        return r;
    }
    let (ia, ib) = (inner_of(a), inner_of(b));
    if ia.is_some() || ib.is_some() {
        return binop(&ia.unwrap_or(a.clone()), &ib.unwrap_or(b.clone()), op);
    }
    if op == "+" && matches!((a, b), (V::Str(_), _) | (_, V::Str(_))) {
        if let V::Str(_) = a {
            return type_err(format!("can only concatenate str (not \"{}\") to str", tn(b)));
        }
    }
    if op == "+" && matches!(a, V::List(_)) {
        return type_err(format!("can only concatenate list (not \"{}\") to list", tn(b)));
    }
    if op == "+" && matches!(a, V::Tuple(_)) {
        return type_err(format!("can only concatenate tuple (not \"{}\") to tuple", tn(b)));
    }
    let pop = if op == "**" { "** or pow()" } else { op };
    Err(op_err(pop, a, b))
}
fn big_truediv(p: &Big, q: &Big) -> Result<f64, E> {
    let (fp, fq) = (p.to_f64(), q.to_f64());
    if fp.is_finite() && fq.is_finite() && p.bits() <= 53 && q.bits() <= 53 {
        return Ok(fp / fq);
    }
    // scale for precision
    let (qq, r) = p.divmod(q);
    let r = qq.to_f64() + r.to_f64() / fq;
    if r.is_infinite() {
        return Err(exc("OverflowError", "integer division result too large for a float".into()));
    }
    Ok(r)
}
fn float_op(x: f64, y: f64, op: &str, a: &V, b: &V) -> R {
    Ok(V::Float(match op {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "/" => {
            if y == 0.0 {
                return Err(exc("ZeroDivisionError", if matches!(a, V::Float(_)) || matches!(b, V::Float(_)) { "float division by zero" } else { "division by zero" }.into()));
            }
            x / y
        }
        "//" => {
            if y == 0.0 {
                return Err(exc("ZeroDivisionError", "float floor division by zero".into()));
            }
            ffloordiv(x, y)
        }
        "%" => {
            if y == 0.0 {
                return Err(exc("ZeroDivisionError", "float modulo by zero".into()));
            }
            fmod(x, y)
        }
        "**" => {
            if x == 0.0 && y < 0.0 {
                return Err(exc("ZeroDivisionError", "zero to a negative power".into()));
            }
            if x < 0.0 && y.fract() != 0.0 {
                return Err(unsupported("complex numbers"));
            }
            let r = x.powf(y);
            if r.is_infinite() && x.is_finite() && y.is_finite() {
                return Err(exc("OverflowError", "(34, 'Numerical result out of range')".into()));
            }
            r
        }
        _ => return Err(op_err(op, a, b)),
    }))
}
pub fn add(a: &V, b: &V) -> R {
    if let (V::Int(x), V::Int(y)) = (a, b) {
        if let Some(r) = x.checked_add(*y) {
            return Ok(V::Int(r));
        }
    }
    binop(a, b, "+")
}
pub fn sub(a: &V, b: &V) -> R {
    if let (V::Int(x), V::Int(y)) = (a, b) {
        if let Some(r) = x.checked_sub(*y) {
            return Ok(V::Int(r));
        }
    }
    binop(a, b, "-")
}
// a OP= b: lists, sets, dicts and deques change in place; other values make a new value.
#[inline]
pub fn iop(a: &V, b: &V, op: &str) -> R {
    if let Some(r) = float_fast(a, b, op) {
        return Ok(r);
    }
    iop_slow(a, b, op)
}
#[inline(never)]
fn iop_slow(a: &V, b: &V, op: &str) -> R {
    if let Some(x) = as_np(a) {
        return np_iop(x, a, b, op);
    }
    if let V::Obj(o) = a {
        let name = match op {
            "+" => "__iadd__",
            "-" => "__isub__",
            "*" => "__imul__",
            "|" => "__ior__",
            "&" => "__iand__",
            "@" => "__imatmul__",
            "/" => "__itruediv__",
            "//" => "__ifloordiv__",
            "%" => "__imod__",
            "**" => "__ipow__",
            "^" => "__ixor__",
            "<<" => "__ilshift__",
            ">>" => "__irshift__",
            _ => "",
        };
        if !name.is_empty() {
            if let Some(f) = o.cls.lookup(name) {
                return call(&f, Args::p(vec![a.clone(), b.clone()]));
            }
        }
    }
    match (a, op) {
        (V::List(l), "+") => {
            let add = to_vec(b)?;
            l.borrow_mut().extend(add);
            return Ok(a.clone());
        }
        (V::List(l), "*") => {
            let k = ival(b).ok_or_else(|| exc("TypeError", format!("can't multiply sequence by non-int of type '{}'", tn(b))))?.max(0) as usize;
            if l.borrow().len().checked_mul(k).map_or(true, |t| t > (1usize << 40)) {
                return Err(exc("MemoryError", String::new()));
            }
            let v = l.borrow().clone();
            let mut l = l.borrow_mut();
            l.clear();
            for _ in 0..k {
                l.extend(v.iter().cloned());
            }
            return Ok(a.clone());
        }
        (V::Set(s), "|" | "&" | "-" | "^") if matches!(b, V::Set(_) | V::FSet(_)) => {
            // In place and in time with the other set's size, as CPython's are (`seen |= new` in a loop
            // must not copy seen each time).
            let other = set_of(b).unwrap();
            match op {
                "|" => s.borrow_mut().merge(&other),
                "-" => {
                    let mut me = s.borrow_mut();
                    for k in other.keys() {
                        me.discard(&k)?;
                    }
                }
                "&" => {
                    // remove the others (keeps table positions)
                    let mut me = s.borrow_mut();
                    for k in me.keys() {
                        if !other.contains(&k)? {
                            me.discard(&k)?;
                        }
                    }
                }
                _ => {
                    let r = set_op(&s.borrow(), &other, op)?;
                    *s.borrow_mut() = r;
                }
            }
            return Ok(a.clone());
        }
        (V::Dict(d), "|") => {
            let items = dict_items_of(b)?;
            for (k, v) in items {
                d.borrow_mut().set(k, v)?;
            }
            return Ok(a.clone());
        }
        (V::Dict(d), "+" | "-") if d.borrow().kind == 2 => {
            let r = binop(a, b, op)?;
            if let V::Dict(r) = r {
                let r = r.borrow().copy();
                *d.borrow_mut() = r;
                d.borrow_mut().kind = 2;
            }
            return Ok(a.clone());
        }
        (V::Deque(d), "+") => {
            let add = to_vec(b)?;
            let mut d = d.borrow_mut();
            for x in add {
                d.v.push_back(x);
                if let Some(m) = d.max {
                    if d.v.len() > m {
                        d.v.pop_front();
                    }
                }
            }
            return Ok(a.clone());
        }
        _ => {}
    }
    binop(a, b, op)
}
fn counter_op(x: &Dict, y: &Dict, op: &str) -> R {
    let mut d = Dict::new();
    d.kind = 2;
    for (k, v) in x.items() {
        let w = y.get(&k)?.unwrap_or(V::Int(0));
        let r = binop(&v, &w, op)?;
        if rich(&r, &V::Int(0), ">")? {
            d.set(k, r)?;
        }
    }
    if op == "+" {
        for (k, v) in y.items() {
            if !x.contains(&k)? && rich(&v, &V::Int(0), ">")? {
                d.set(k, v)?;
            }
        }
    }
    Ok(V::Dict(Rc::new(RefCell::new(d))))
}
pub fn set_of(v: &V) -> Option<PSet> {
    match v {
        V::Set(s) => Some(s.borrow().clone()),
        V::FSet(s) => Some((**s).clone()),
        _ => None,
    }
}
// CPython's set algebra, so results come out in the same table order.
pub fn set_op(a: &PSet, b: &PSet, op: &str) -> Result<PSet, E> {
    Ok(match op {
        "|" => {
            let mut r = a.copy();
            r.merge(b);
            r
        }
        "&" => {
            let (small, big) = if a.used > b.used { (b, a) } else { (a, b) };
            let mut r = PSet::new();
            for (k, h) in small.keys_h() {
                if big.find(&k, h).is_some() {
                    r.add_h(k, h);
                }
            }
            r
        }
        "-" => {
            if (a.used >> 2) > b.used {
                let mut r = a.copy();
                for k in b.keys() {
                    r.discard(&k)?;
                }
                r
            } else {
                let mut r = PSet::new();
                for (k, h) in a.keys_h() {
                    if b.find(&k, h).is_none() {
                        r.add_h(k, h);
                    }
                }
                r
            }
        }
        _ => {
            let mut r = a.copy();
            for (k, h) in b.keys_h() {
                match r.find(&k, h) {
                    Some(i) => {
                        r.t[i] = Slot::Dummy;
                        r.used -= 1;
                    }
                    None => r.add_h(k, h),
                }
            }
            r
        }
    })
}
pub fn neg(a: &V) -> R {
    match a {
        V::Int(i) => Ok(match i.checked_neg() {
            Some(r) => V::Int(r),
            None => Big::from_i64(*i).neg().norm(),
        }),
        V::Bool(b) => Ok(V::Int(-(*b as i64))),
        V::Big(b) => Ok(b.neg().norm()),
        V::Float(f) => Ok(V::Float(-f)),
        V::Obj(o) => match o.cls.lookup("__neg__") {
            Some(f) => call(&f, Args::p(vec![a.clone()])),
            None => match inner_of(a) {
                Some(i) => neg(&i),
                None => type_err(format!("bad operand type for unary -: '{}'", tn(a))),
            },
        },
        V::Ext(x) => match x.binop(a, "neg", &V::None, false) {
            Some(r) => r,
            None => type_err(format!("bad operand type for unary -: '{}'", tn(a))),
        },
        _ => type_err(format!("bad operand type for unary -: '{}'", tn(a))),
    }
}
pub fn pos(a: &V) -> R {
    match a {
        V::Int(_) | V::Big(_) | V::Float(_) => Ok(a.clone()),
        V::Bool(b) => Ok(V::Int(*b as i64)),
        V::Ext(x) => match x.binop(a, "pos", &V::None, false) {
            Some(r) => r,
            None => type_err(format!("bad operand type for unary +: '{}'", tn(a))),
        },
        V::Obj(o) => match o.cls.lookup("__pos__") {
            Some(f) => call(&f, Args::p(vec![a.clone()])),
            None => match inner_of(a) {
                Some(i) => pos(&i),
                None => type_err(format!("bad operand type for unary +: '{}'", tn(a))),
            },
        },
        _ => type_err(format!("bad operand type for unary +: '{}'", tn(a))),
    }
}
pub fn invert(a: &V) -> R {
    match a {
        V::Int(i) => Ok(V::Int(!i)),
        V::Bool(b) => Ok(V::Int(!(*b as i64))),
        V::Big(b) => Ok(b.neg().sub(&Big::from_i64(1)).norm()),
        V::Ext(x) => match x.binop(a, "invert", &V::None, false) {
            Some(r) => r,
            None => type_err(format!("bad operand type for unary ~: '{}'", tn(a))),
        },
        V::Obj(o) => match o.cls.lookup("__invert__") {
            Some(f) => call(&f, Args::p(vec![a.clone()])),
            None => match inner_of(a) {
                Some(i) => invert(&i),
                None => type_err(format!("bad operand type for unary ~: '{}'", tn(a))),
            },
        },
        _ => type_err(format!("bad operand type for unary ~: '{}'", tn(a))),
    }
}

// ---------- len, contains, indexing ----------

pub fn len(v: &V) -> Result<usize, E> {
    Ok(match v {
        V::Class(c) if is_enum(c) => match enum_members(c) {
            V::List(l) => l.borrow().len(),
            _ => 0,
        },
        V::Str(x) => x.nchars(),
        V::List(l) => l.borrow().len(),
        V::Tuple(t) => t.len(),
        V::Dict(d) => d.borrow().n,
        V::Set(x) => x.borrow().used,
        V::FSet(x) => x.used,
        V::Range(_) => range_len(v),
        V::Deque(d) => d.borrow().v.len(),
        V::View(x) => match &x.0 {
            V::Dict(d) => d.borrow().n,
            _ => 0,
        },
        V::Obj(o) => match o.cls.lookup("__len__") {
            Some(f) => match call(&f, Args::p(vec![v.clone()]))? {
                V::Int(i) if i >= 0 => i as usize,
                _ => return type_err("'__len__' returned a bad value".into()),
            },
            None => match inner_of(v) {
                Some(i) => len(&i)?,
                None => return type_err(format!("object of type '{}' has no len()", tn(v))),
            },
        },
        V::Ext(x) => match x.len() {
            Some(n) => n,
            None => return type_err(format!("object of type '{}' has no len()", tn(v))),
        },
        _ => return type_err(format!("object of type '{}' has no len()", tn(v))),
    })
}
pub fn contains(c: &V, x: &V) -> Result<bool, E> {
    // `x in seen` for a set first: the special cases below are all for str containers
    if let V::Set(s) = c {
        return s.borrow().contains(x);
    }
    // a library value standing for a str (numpy's np.str_) is that str here
    if let V::Ext(e) = x {
        if matches!(c, V::Str(_)) {
            if let Some(n) = e.native() {
                return contains(c, &n?);
            }
        }
    }
    // a str subclass (StrEnum) in a str
    if let (V::Str(_), V::Obj(_)) = (c, x) {
        if let Some(i @ V::Str(_)) = inner_of(x) {
            return contains(c, &i);
        }
    }
    Ok(match c {
        V::Class(k) if is_enum(k) => enum_contains(k, x)?,
        V::Ext(e) if e.is_py() || e.any().is::<BytesV>() => match e.method(c, "__contains__", &mut Args::p(vec![x.clone()])) {
            Some(r) => truth(&r?)?,
            None => false,
        },
        V::Str(s) => match x {
            V::Str(n) => s.s.contains(n.s.as_str()),
            _ => return type_err(format!("'in <string>' requires string as left operand, not {}", tn(x))),
        },
        V::List(l) => {
            let l = l.borrow().clone();
            for y in l.iter() {
                if is(y, x) || eq(y, x)? {
                    return Ok(true);
                }
            }
            false
        }
        V::Tuple(t) => {
            for y in t.iter() {
                if is(y, x) || eq(y, x)? {
                    return Ok(true);
                }
            }
            false
        }
        V::Dict(d) => d.borrow().contains(x)?,
        V::Set(s) => s.borrow().contains(x)?,
        V::FSet(s) => s.contains(x)?,
        V::Range(r) => match x {
            V::Int(i) => {
                let (a, b, st) = **r;
                if st > 0 {
                    *i >= a && *i < b && (i - a) % st == 0
                } else {
                    *i <= a && *i > b && (a - i) % (-st) == 0
                }
            }
            _ => to_vec(c)?.iter().any(|y| eq(y, x).unwrap_or(false)),
        },
        V::View(v) => match (&v.0, v.1) {
            (V::Dict(d), 0 | 3) => d.borrow().contains(x)?,
            (V::Dict(d), 2) => return items_view_contains(d, x),
            _ => to_vec(c)?.iter().any(|y| eq(y, x).unwrap_or(false)),
        },
        V::Deque(d) => {
            let v: Vec<V> = d.borrow().v.iter().cloned().collect();
            v.iter().any(|y| eq(y, x).unwrap_or(false))
        }
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__contains__") {
                return truth(&call(&f, Args::p(vec![c.clone(), x.clone()]))?);
            }
            if let Some(i) = inner_of(c) {
                return contains(&i, x);
            }
            for y in to_vec(c)? {
                if eq(&y, x)? {
                    return Ok(true);
                }
            }
            false
        }
        V::Iter(_) => {
            let mut it = iter(c)?;
            while let Some(y) = it.next()? {
                if eq(&y, x)? {
                    return Ok(true);
                }
            }
            false
        }
        V::Ext(e) => match e.method(c, "__contains__", &mut Args::p(vec![x.clone()])) {
            Some(r) => truth(&r?)?,
            None => match e.native() {
                Some(n) => contains(&n?, x)?,
                None => return type_err(format!("argument of type '{}' is not iterable", tn(c))),
            },
        },
        _ => return type_err(format!("argument of type '{}' is not iterable", tn(c))),
    })
}

pub fn idx_of(i: &V) -> Result<i64, E> {
    match i {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("IndexError", "cannot fit 'int' into an index-sized integer".into())),
        V::Obj(o) if o.cls.lookup("__index__").is_some() => idx_of(&call(&o.cls.lookup("__index__").unwrap(), Args::p(vec![i.clone()]))?),
        // an int subclass (IntEnum)
        V::Obj(_) if matches!(inner_of(i), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => idx_of(&inner_of(i).unwrap()),
        V::Ext(x) if is_np(i) => match x.method(i, "__index__", &mut Args::p(vec![])) {
            Some(r) => idx_of(&r?),
            None => Err(exc("TypeError", format!("list indices must be integers or slices, not {}", tn(i)))),
        },
        _ => Err(exc("TypeError", format!("list indices must be integers or slices, not {}", tn(i)))),
    }
}
fn norm_idx(i: i64, n: usize, what: &str) -> Result<usize, E> {
    let j = if i < 0 { i + n as i64 } else { i };
    if j < 0 || j >= n as i64 {
        return Err(exc("IndexError", format!("{what} index out of range")));
    }
    Ok(j as usize)
}
// Python's slice.indices
pub fn slice_idx(sl: &(V, V, V), n: usize) -> Result<(i64, i64, i64), E> {
    let step = match &sl.2 {
        V::None => 1,
        // `xs[::1 << 333]`: as CPython, a step past 64 bits is the largest one.
        V::Big(b) => if b.neg { -(i64::MAX / 4) } else { i64::MAX / 4 },
        x => idx_of(x)?,
    };
    if step == 0 {
        return val_err("slice step cannot be zero".into());
    }
    let n = n as i64;
    let clamp = |v: &V, dflt: i64| -> Result<i64, E> {
        Ok(match v {
            V::None => dflt,
            x => {
                let mut i = match x {
                    V::Big(b) => {
                        if b.neg {
                            i64::MIN / 2
                        } else {
                            i64::MAX / 2
                        }
                    }
                    _ => idx_of(x)?,
                };
                if i < 0 {
                    i += n;
                    if i < 0 {
                        i = if step < 0 { -1 } else { 0 };
                    }
                } else if i >= n {
                    i = if step < 0 { n - 1 } else { n };
                }
                i
            }
        })
    };
    let start = clamp(&sl.0, if step < 0 { n - 1 } else { 0 })?;
    let stop = clamp(&sl.1, if step < 0 { -1 } else { n })?;
    Ok((start, stop, step))
}
fn slice_positions(sl: &(V, V, V), n: usize) -> Result<Vec<usize>, E> {
    let (a, b, st) = slice_idx(sl, n)?;
    let mut out = vec![];
    let mut i = a;
    // `xs[3::sys.maxsize]`: the step can be too big to add to a position, which ends the slice.
    if st > 0 {
        while i < b {
            out.push(i as usize);
            i = match i.checked_add(st) { Some(x) => x, None => break };
        }
    } else {
        while i > b {
            out.push(i as usize);
            i = match i.checked_add(st) { Some(x) => x, None => break };
        }
    }
    Ok(out)
}
pub fn str_chars(s: &S) -> Vec<char> {
    s.s.chars().collect()
}
pub fn getitem(c: &V, i: &V) -> R {
    match (c, i) {
        (V::List(l), V::Int(k)) => {
            let l = l.borrow();
            let j = norm_idx(*k, l.len(), "list")?;
            return Ok(l[j].clone());
        }
        (V::Tuple(t), V::Int(k)) => {
            let j = norm_idx(*k, t.len(), "tuple")?;
            return Ok(t[j].clone());
        }
        (V::Dict(d), _) => {
            let got = d.borrow().get(i)?;
            if let Some(v) = got {
                return Ok(v);
            }
            let (kind, dflt) = {
                let d = d.borrow();
                (d.kind, d.dflt.clone())
            };
            if kind == 2 {
                return Ok(V::Int(0));
            }
            if let Some(f) = dflt {
                let v = call(&f, Args::p(vec![]))?;
                d.borrow_mut().set(i.clone(), v.clone())?;
                return Ok(v);
            }
            return Err(exc_v("KeyError", i.clone()));
        }
        (V::Str(s), V::Int(k)) => {
            if s.ascii {
                let j = norm_idx(*k, s.s.len(), "string")?;
                return Ok(ch1(s.s.as_bytes()[j] as char));
            }
            let n = s.nchars();
            let j = norm_idx(*k, n, "string")?;
            return Ok(ch1(s.s.chars().nth(j).unwrap()));
        }
        (V::Str(sx), V::Slice(sl)) => {
            if sx.ascii {
                let b = sx.s.as_bytes();
                let (a, e, stp) = slice_idx(sl, b.len())?;
                if stp == 1 {
                    let (a, e) = (a.max(0) as usize, e.max(0) as usize);
                    return Ok(if a >= e { s("") } else { st_ascii(&sx.s[a..e]) });
                }
                let ps = slice_positions(sl, b.len())?;
                return Ok(st(ps.iter().map(|&p| b[p] as char).collect()));
            }
            let cs = str_chars(sx);
            let ps = slice_positions(sl, cs.len())?;
            return Ok(st(ps.iter().map(|&p| cs[p]).collect()));
        }
        (V::List(l), V::Slice(sl)) => {
            let l = l.borrow();
            let (a, e, stp) = slice_idx(sl, l.len())?;
            if stp == 1 {
                let (a, e) = (a.max(0) as usize, e.max(0) as usize);
                return Ok(list(if a >= e { vec![] } else { l[a..e].to_vec() }));
            }
            let ps = slice_positions(sl, l.len())?;
            return Ok(list(ps.iter().map(|&p| l[p].clone()).collect()));
        }
        (V::Tuple(t), V::Slice(sl)) => {
            let ps = slice_positions(sl, t.len())?;
            return Ok(tup(ps.iter().map(|&p| t[p].clone()).collect()));
        }
        (V::Range(_), V::Int(k)) => {
            let n = range_len(c);
            let j = norm_idx(*k, n, "range object")?;
            if let V::Range(r) = c {
                return Ok(V::Int(r.0 + r.2 * j as i64));
            }
        }
        (V::Range(r), V::Slice(sl)) => {
            let n = range_len(c);
            let (a, e, stp) = slice_idx(sl, n)?;
            let start = r.0 + r.2 * a;
            let stop = r.0 + r.2 * e;
            return Ok(V::Range(Rc::new((start, stop, r.2 * stp))));
        }
        (V::Deque(d), _) => {
            let d = d.borrow();
            let j = norm_idx(idx_of(i)?, d.v.len(), "deque")?;
            return Ok(d.v[j].clone());
        }
        (V::View(x), _) if x.1 == 3 => return getitem(&x.0, i),
        (V::Obj(o), _) => {
            if let Some(f) = o.cls.lookup("__getitem__") {
                return call(&f, Args::p(vec![c.clone(), i.clone()]));
            }
            if let Some(inn) = inner_of(c) {
                // A dict subclass with `__missing__`: a missing key calls it (`class D(dict)`).
                if let (V::Dict(d), Some(f)) = (&inn, o.cls.lookup("__missing__")) {
                    let got = d.borrow().get(i)?;
                    return match got {
                        Some(v) => Ok(v),
                        None => call(&f, Args::p(vec![c.clone(), i.clone()])),
                    };
                }
                return getitem(&inn, i);
            }
            return type_err(format!("'{}' object is not subscriptable", tn(c)));
        }
        (V::Class(k), _) if is_enum(k) => return enum_getitem(k, i),
        (V::Class(_), _) => return Ok(c.clone()), // typing: list[int]
        (V::Ext(x), _) => {
            if let Some(r) = x.method(c, "__getitem__", &mut Args::p(vec![i.clone()])) {
                return r;
            }
        }
        _ => {}
    }
    match (c, i) {
        (V::List(_) | V::Tuple(_) | V::Str(_) | V::Range(_), V::Bool(_) | V::Big(_)) => getitem(c, &V::Int(idx_of(i)?)),
        (V::List(_) | V::Tuple(_) | V::Str(_), V::Obj(_)) => getitem(c, &V::Int(idx_of(i)?)),
        (V::List(_), _) => type_err(format!("list indices must be integers or slices, not {}", tn(i))),
        (V::Tuple(_), _) => type_err(format!("tuple indices must be integers or slices, not {}", tn(i))),
        (V::Str(_), _) => type_err(format!("string indices must be integers, not '{}'", tn(i))),
        _ => type_err(format!("'{}' object is not subscriptable", tn(c))),
    }
}
fn st_ascii(x: &str) -> V {
    if x.len() == 1 {
        return ch1(x.as_bytes()[0] as char);
    }
    V::Str(Rc::new(S { s: x.to_string(), ascii: true }))
}
pub fn setitem(c: &V, i: &V, v: V) -> Result<(), E> {
    match (c, i) {
        (V::List(l), V::Int(k)) => {
            let mut l = l.borrow_mut();
            let j = norm_idx(*k, l.len(), "list assignment")?;
            l[j] = v;
            Ok(())
        }
        (V::Dict(d), _) => d.borrow_mut().set(i.clone(), v),
        (V::List(l), V::Slice(sl)) => {
            let new = to_vec(&v)?;
            let n = l.borrow().len();
            let (a, e, stp) = slice_idx(sl, n)?;
            if stp == 1 {
                let a = a.max(0) as usize;
                let e = (e.max(0) as usize).max(a);
                l.borrow_mut().splice(a..e, new);
                return Ok(());
            }
            let ps = slice_positions(sl, n)?;
            if ps.len() != new.len() {
                return val_err(format!("attempt to assign sequence of size {} to extended slice of size {}", new.len(), ps.len()));
            }
            let mut l = l.borrow_mut();
            for (p, x) in ps.into_iter().zip(new) {
                l[p] = x;
            }
            Ok(())
        }
        (V::Deque(d), _) => {
            let mut d = d.borrow_mut();
            let j = norm_idx(idx_of(i)?, d.v.len(), "deque")?;
            d.v[j] = v;
            Ok(())
        }
        (V::Obj(o), _) => {
            if let Some(f) = o.cls.lookup("__setitem__") {
                call(&f, Args::p(vec![c.clone(), i.clone(), v]))?;
                return Ok(());
            }
            if let Some(inn) = inner_of(c) {
                return setitem(&inn, i, v);
            }
            type_err(format!("'{}' object does not support item assignment", tn(c)))
        }
        (V::Ext(x), _) => match x.method(c, "__setitem__", &mut Args::p(vec![i.clone(), v])) {
            Some(r) => r.map(|_| ()),
            None => type_err(format!("'{}' object does not support item assignment", tn(c))),
        },
        (V::List(_), V::Bool(_)) => setitem(c, &V::Int(idx_of(i)?), v),
        (V::List(_), _) => type_err(format!("list indices must be integers or slices, not {}", tn(i))),
        _ => type_err(format!("'{}' object does not support item assignment", tn(c))),
    }
}
pub fn delitem(c: &V, i: &V) -> Result<(), E> {
    match (c, i) {
        (V::List(l), V::Int(k)) => {
            let mut l = l.borrow_mut();
            let j = norm_idx(*k, l.len(), "list assignment")?;
            l.remove(j);
            Ok(())
        }
        (V::List(l), V::Slice(sl)) => {
            let n = l.borrow().len();
            let mut ps = slice_positions(sl, n)?;
            ps.sort();
            let mut l = l.borrow_mut();
            for p in ps.into_iter().rev() {
                l.remove(p);
            }
            Ok(())
        }
        (V::Dict(d), _) => match d.borrow_mut().del(i)? {
            Some(_) => Ok(()),
            None => Err(exc_v("KeyError", i.clone())),
        },
        (V::Deque(d), _) => {
            let mut d = d.borrow_mut();
            let j = norm_idx(idx_of(i)?, d.v.len(), "deque")?;
            d.v.remove(j);
            Ok(())
        }
        (V::Obj(o), _) => {
            if let Some(f) = o.cls.lookup("__delitem__") {
                call(&f, Args::p(vec![c.clone(), i.clone()]))?;
                return Ok(());
            }
            if let Some(inn) = inner_of(c) {
                return delitem(&inn, i);
            }
            type_err(format!("'{}' object doesn't support item deletion", tn(c)))
        }
        (V::Ext(x), _) => match x.method(c, "__delitem__", &mut Args::p(vec![i.clone()])) {
            Some(r) => r.map(|_| ()),
            None => type_err(format!("'{}' object doesn't support item deletion", tn(c))),
        },
        _ => type_err(format!("'{}' object doesn't support item deletion", tn(c))),
    }
}
pub fn slice(a: V, b: V, c: V) -> V {
    V::Slice(Rc::new((a, b, c)))
}

// ---------- iteration ----------

pub enum It {
    List(Rc<RefCell<Vec<V>>>, usize),
    Tuple(Tup, usize),
    Vec(std::vec::IntoIter<V>),
    Str(Rc<S>, usize),
    Range(i64, i64, i64),
    Dict(Rc<RefCell<Dict>>, usize, u8, u64, usize),
    Gen(Box<dyn FnMut() -> Result<Option<V>, E>>),
    // A generator expression: like Gen, but an error escapes to the caller once and then the iterator is
    // exhausted, as in CPython.
    GenExp(Box<dyn FnMut() -> Result<Option<V>, E>>, bool),
    Coro(Rc<RefCell<Box<Coro>>>),
    Obj(V),
    Done,
}
pub struct Iter {
    pub it: It,
}
impl It {
    // lists and tuples first, inline in the loop that runs them; the rest through one match
    #[inline(always)]
    pub fn next(&mut self) -> Result<Option<V>, E> {
        if let It::Tuple(t, i) = self {
            return Ok(t.get(*i).map(|x| {
                *i += 1;
                x.clone()
            }));
        }
        if let It::List(l, i) = self {
            let got = l.borrow().get(*i).cloned();
            if got.is_some() {
                *i += 1;
                return Ok(got);
            }
            // An exhausted list iterator stays exhausted, as in CPython, even if the list grows afterwards.
            *self = It::Done;
            return Ok(None);
        }
        self.next_slow()
    }
    #[inline(never)]
    fn next_slow(&mut self) -> Result<Option<V>, E> {
        match self {
            It::List(l, i) => {
                let got = l.borrow().get(*i).cloned();
                if got.is_some() {
                    *i += 1;
                    Ok(got)
                } else {
                    *self = It::Done;
                    Ok(None)
                }
            }
            It::Tuple(t, i) => {
                if *i < t.len() {
                    *i += 1;
                    Ok(Some(t[*i - 1].clone()))
                } else {
                    Ok(None)
                }
            }
            It::Vec(v) => Ok(v.next()),
            It::Str(s, i) => {
                if *i >= s.s.len() {
                    return Ok(None);
                }
                if s.ascii {
                    *i += 1;
                    return Ok(Some(ch1(s.s.as_bytes()[*i - 1] as char)));
                }
                let c = s.s[*i..].chars().next().unwrap();
                *i += c.len_utf8();
                Ok(Some(ch1(c)))
            }
            It::Range(a, b, st) => {
                if (*st > 0 && *a < *b) || (*st < 0 && *a > *b) {
                    let v = *a;
                    *a += *st;
                    Ok(Some(V::Int(v)))
                } else {
                    Ok(None)
                }
            }
            It::Dict(d, i, kind, ver, n0) => {
                let d = d.borrow();
                if d.n != *n0 {
                    return Err(exc("RuntimeError", "dictionary changed size during iteration".into()));
                }
                while *i < d.ents.len() {
                    *i += 1;
                    if let Some((k, v)) = &d.ents[*i - 1] {
                        // `ver` counts the items given: one more than the dict held when the loop began means a key
                        // was deleted and another added (the size is the same), which CPython stops on.
                        if *ver as usize >= *n0 {
                            return Err(exc("RuntimeError", "dictionary keys changed during iteration".into()));
                        }
                        *ver += 1;
                        return Ok(Some(match kind {
                            0 | 3 => k.clone(),
                            1 => v.clone(),
                            _ => tup(vec![k.clone(), v.clone()]),
                        }));
                    }
                }
                Ok(None)
            }
            It::Gen(f) => f(),
            It::GenExp(f, done) => {
                if *done {
                    return Ok(None);
                }
                match f() {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        *done = true;
                        Err(e)
                    }
                }
            }
            It::Coro(c) => coro_next(c, V::None),
            It::Obj(o) => {
                let f = match o {
                    V::Obj(ob) => ob.cls.lookup("__next__"),
                    _ => None,
                };
                match f {
                    Some(f) => match call(&f, Args::p(vec![o.clone()])) {
                        Ok(v) => Ok(Some(v)),
                        Err(E(e)) if is_exc(&e, "StopIteration") => Ok(None),
                        Err(e) => Err(e),
                    },
                    None => type_err(format!("'{}' object is not an iterator", tn(o))),
                }
            }
            It::Done => Ok(None),
        }
    }
}
pub fn is_exc(e: &V, name: &str) -> bool {
    match e {
        V::Obj(o) => {
            let c = exc_class(name);
            o.cls.is_sub(&c)
        }
        _ => false,
    }
}
pub fn iter(v: &V) -> Result<It, E> {
    Ok(match v {
        V::Class(c) if is_enum(c) => match enum_members(c) {
            V::List(l) => It::List(l, 0),
            _ => It::Done,
        },
        V::List(l) => It::List(l.clone(), 0),
        V::Tuple(t) => It::Tuple(t.clone(), 0),
        V::Str(s) => It::Str(s.clone(), 0),
        V::Range(r) => It::Range(r.0, r.1, r.2),
        V::Dict(d) => {
            let n = d.borrow().n;
            It::Dict(d.clone(), 0, 0, 0, n)
        }
        V::View(x) => match &x.0 {
            V::Dict(d) => {
                let n = d.borrow().n;
                It::Dict(d.clone(), 0, x.1, 0, n)
            }
            _ => It::Done,
        },
        V::Set(s) => It::Vec(s.borrow().keys().into_iter()),
        V::FSet(s) => It::Vec(s.keys().into_iter()),
        V::Deque(d) => It::Vec(d.borrow().v.iter().cloned().collect::<Vec<_>>().into_iter()),
        V::Iter(it) => {
            let it = it.clone();
            It::Gen(Box::new(move || it.borrow_mut().next()))
        }
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__iter__") {
                let r = call(&f, Args::p(vec![v.clone()]))?;
                if let V::Obj(_) = &r {
                    return Ok(It::Obj(r));
                }
                return iter(&r);
            }
            if let Some(i) = inner_of(v) {
                return iter(&i);
            }
            if let Some(f) = o.cls.lookup("__getitem__") {
                let me = v.clone();
                let mut k = 0i64;
                return Ok(It::Gen(Box::new(move || match call(&f, Args::p(vec![me.clone(), V::Int(k)])) {
                    Ok(x) => {
                        k += 1;
                        Ok(Some(x))
                    }
                    Err(E(e)) if is_exc(&e, "IndexError") || is_exc(&e, "StopIteration") => Ok(None),
                    Err(e) => Err(e),
                })));
            }
            return type_err(format!("'{}' object is not iterable", tn(v)));
        }
        V::Module(&ModName("sys.stdin")) => It::Gen(Box::new(|| Ok(read_line().map(st)))),
        V::Module(&ModName("sys.stdin.buffer")) => It::Gen(Box::new(|| Ok(read_line_bytes().map(bytesv)))),
        V::Ext(x) => match x.lazy_iter() {
            Some(r) => return r,
            None => match x.iter() {
                Some(v) => It::Vec(v.into_iter()),
                None => return type_err(format!("'{}' object is not iterable", tn(v))),
            },
        },
        _ => return type_err(format!("'{}' object is not iterable", tn(v))),
    })
}
pub fn iter_v(v: &V) -> R {
    if let V::Iter(_) = v {
        return Ok(v.clone());
    }
    if let V::Obj(o) = v {
        if let Some(f) = o.cls.lookup("__iter__") {
            let r = call(&f, Args::p(vec![v.clone()]))?;
            if let V::Obj(_) = &r {
                return Ok(V::Iter(Rc::new(RefCell::new(It::Obj(r)))));
            }
            return iter_v(&r);
        }
    }
    Ok(V::Iter(Rc::new(RefCell::new(iter(v)?))))
}
pub fn to_vec(v: &V) -> Result<Vec<V>, E> {
    Ok(match v {
        V::List(l) => l.borrow().clone(),
        V::Tuple(t) => t.to_vec(),
        _ => {
            let mut it = iter(v)?;
            let mut out = vec![];
            while let Some(x) = it.next()? {
                out.push(x);
            }
            out
        }
    })
}
pub fn gen_iter(f: impl FnMut() -> Result<Option<V>, E> + 'static) -> V {
    V::Iter(Rc::new(RefCell::new(It::Gen(Box::new(f)))))
}
pub fn genexp_iter(f: impl FnMut() -> Result<Option<V>, E> + 'static) -> V {
    V::Iter(Rc::new(RefCell::new(It::GenExp(Box::new(f), false))))
}
pub fn vec_iter(v: Vec<V>) -> V {
    V::Iter(Rc::new(RefCell::new(It::Vec(v.into_iter()))))
}
// a, b = x
pub fn unpack(v: &V, n: usize) -> Result<Vec<V>, E> {
    let xs = match v {
        V::Tuple(t) if t.len() == n => return Ok(t.to_vec()),
        V::List(l) if l.borrow().len() == n => return Ok(l.borrow().clone()),
        _ => to_vec(v)?,
    };
    if xs.len() > n {
        return val_err(format!("too many values to unpack (expected {n})"));
    }
    if xs.len() < n {
        return val_err(format!("not enough values to unpack (expected {n}, got {})", xs.len()));
    }
    Ok(xs)
}
// a, b = x without building a Vec (a tuple nobody else holds gives up its items)
pub fn unpack2(v: V) -> Result<(V, V), E> {
    if let V::Tuple(t) = &v {
        if t.len() == 2 {
            return Ok((t[0].clone(), t[1].clone()));
        }
    }
    let mut it = unpack(&v, 2)?.into_iter();
    Ok((it.next().unwrap(), it.next().unwrap()))
}
// a, *b, c = x
pub fn unpack_star(v: &V, before: usize, after: usize) -> Result<Vec<V>, E> {
    let xs = to_vec(v)?;
    if xs.len() < before + after {
        return val_err(format!("not enough values to unpack (expected at least {}, got {})", before + after, xs.len()));
    }
    let mut out: Vec<V> = xs[..before].to_vec();
    out.push(list(xs[before..xs.len() - after].to_vec()));
    out.extend(xs[xs.len() - after..].iter().cloned());
    Ok(out)
}
pub fn dict_items_of(v: &V) -> Result<Vec<(V, V)>, E> {
    if let V::Ext(x) = v {
        if let Some(n) = x.native() {
            return dict_items_of(&n?);
        }
    }
    match v {
        V::Dict(d) => Ok(d.borrow().items()),
        V::View(x) if x.1 == 3 => dict_items_of(&x.0),
        V::Obj(o) if o.cls.lookup("keys").is_some() => {
            let ks = to_vec(&call_method(v, "keys", Args::p(vec![]))?)?;
            let mut out = vec![];
            for k in ks {
                let x = getitem(v, &k)?;
                out.push((k, x));
            }
            Ok(out)
        }
        // `dict(d)` of a dict subclass that keeps dict's own keys: its items.
        V::Obj(_) if matches!(inner_of(v), Some(V::Dict(_))) => dict_items_of(&inner_of(v).unwrap()),
        _ => {
            let mut out = vec![];
            for (i, x) in to_vec(v)?.into_iter().enumerate() {
                let p = to_vec(&x).map_err(|_| exc("TypeError", format!("cannot convert dictionary update sequence element #{i} to a sequence")))?;
                if p.len() != 2 {
                    return val_err(format!("dictionary update sequence element #{i} has length {}; 2 is required", p.len()));
                }
                out.push((p[0].clone(), p[1].clone()));
            }
            Ok(out)
        }
    }
}
