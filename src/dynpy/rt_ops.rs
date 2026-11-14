
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
    let found = Dict::get_cell(d, &k)?;
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
// a library value next to a bytearray or memoryview: the library's comparison decides (pyglue's cmp_value
// compares an array.array's buffer; anything else refuses the bytearray, which cannot cross as itself)
fn py_next_to_buffer(a: &V, b: &V) -> bool {
    let py = |x: &V| matches!(x, V::Ext(e) if e.is_py());
    let buf = |x: &V| ba_val(x).is_some() || memview_of(x).is_some();
    (py(a) && buf(b)) || (buf(a) && py(b))
}
// CPython's _odict_keys_equal: the keys of two OrderedDicts compared pairwise in order, an error if either
// OrderedDict's key set changed during a comparison
fn odict_keys_equal(x: &Rc<RefCell<Dict>>, y: &Rc<RefCell<Dict>>) -> Result<bool, E> {
    let (kx, vx) = { let d = x.borrow(); (d.keys(), d.ver) };
    let (ky, vy) = { let d = y.borrow(); (d.keys(), d.ver) };
    let mut i = 0;
    loop {
        match (kx.get(i), ky.get(i)) {
            (None, None) => return Ok(true),
            (None, _) | (_, None) => return Ok(false),
            (Some(p), Some(q)) => {
                let same = is(p, q) || eq(p, q)?;
                if x.borrow().ver != vx || y.borrow().ver != vy {
                    return Err(exc("RuntimeError", "OrderedDict mutated during iteration".into()));
                }
                if !same {
                    return Ok(false);
                }
            }
        }
        i += 1;
    }
}
pub fn eq(a: &V, b: &V) -> Result<bool, E> {
    if let Some(r) = meta_cmp(a, b, "==") {
        return truth(&r?);
    }
    if py_next_to_buffer(a, b) {
        return truth(&cmp_v(a, b, "==")?);
    }
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
        (V::Str(x), V::Str(y)) => Rc::ptr_eq(x, y) || x.same(y),
        (V::None, V::None) => true,
        (V::None, _) | (_, V::None)
            if !matches!(a, V::Obj(_))
                && !matches!(b, V::Obj(_))
                && !matches!(a, V::Ext(_))
                && !matches!(b, V::Ext(_)) =>
        {
            false
        }
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
            if x.borrow().kind == 2 && y.borrow().kind == 2 {
                return Ok(counter_eq(x, y)?);
            }
            if Rc::ptr_eq(x, y) {
                return Ok(true);
            }
            let n = x.borrow().n;
            if y.borrow().n != n {
                return Ok(false);
            }
            let items = x.borrow().items();
            let mut ok = true;
            for (k, v) in items {
                match Dict::get_cell(y, &k)? {
                    Some(w) => {
                        if !(is(&v, &w) || eq(&v, &w)?) {
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
            // two OrderedDicts: equal contents, then the same key order (CPython's odict_richcompare)
            ok && (x.borrow().kind != 3 || y.borrow().kind != 3 || odict_keys_equal(x, y)?)
        }
        (V::View(x), V::View(y)) if x.1 == 3 && y.1 == 3 => eq(&x.0, &y.0)?,
        (V::View(x), V::Dict(_)) if x.1 == 3 => eq(&x.0, b)?,
        (V::Dict(_), V::View(y)) if y.1 == 3 => eq(a, &y.0)?,
        // a cell compares by its contents, exactly as CPython's cell type does (an empty cell raises)
        (V::Cell(x), V::Cell(y)) => {
            let (xv, yv) = {
                let (xb, yb) = (x.borrow(), y.borrow());
                let xv = match &*xb {
                    V::Unbound => return Err(exc("ValueError", "Cell is empty".into())),
                    v => v.clone(),
                };
                let yv = match &*yb {
                    V::Unbound => return Err(exc("ValueError", "Cell is empty".into())),
                    v => v.clone(),
                };
                (xv, yv)
            };
            eq(&xv, &yv)?
        }
        (V::Set(_) | V::FSet(_), V::Set(_) | V::FSet(_)) => {
            let (x, y) = (set_of(a).unwrap(), set_of(b).unwrap());
            x.used == y.used && pset_issubset(&x, &y)?
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
            let first = reflected_first(a, b);
            if first {
                if let V::Obj(o) = b {
                    if let Some(f) = o.cls.lookup("__eq__") {
                        let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                        if !matches!(r, V::NotImpl) {
                            return truth(&r);
                        }
                    }
                }
            }
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
                    // already tried first when b's class is a strict subclass of a's
                    if !first {
                        let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                        if !matches!(r, V::NotImpl) {
                            return truth(&r);
                        }
                    }
                } else if let Some(i) = inner_of(b) {
                    return eq(a, &i);
                }
            }
            is(a, b)
        }
        (V::Ext(x), _) => match x.eq_rich(a, b, false) {
            Some(r) => r?,
            None => match x.eq(b) {
                Some(r) => r,
                None => match b {
                    V::Ext(y) => match y.eq_rich(b, a, true) {
                        Some(r) => r?,
                        None => y.eq(a).unwrap_or(false),
                    },
                    _ => false,
                },
            },
        },
        (_, V::Ext(y)) => match y.eq_rich(b, a, true) {
            Some(r) => r?,
            None => y.eq(a).unwrap_or(false),
        },
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
// CPython's do_richcompare: when the right operand's class is a strict subclass of the left operand's,
// the right operand's reflected comparison runs first (and is not tried again after the left one).
pub fn reflected_first(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Obj(x), V::Obj(y)) => !Rc::ptr_eq(&x.cls, &y.cls) && y.cls.is_sub(&x.cls),
        _ => false,
    }
}
// One side of `x != y` for an object, as CPython's slot does it: x's own __ne__ (its value, whatever it is:
// a polars expression, say), else object.__ne__, which inverts the truth of x's __eq__. None for
// NotImplemented (or no __eq__ either: object's own answers NotImplemented unless x is y).
fn ne_side(x: &V, y: &V) -> Result<Option<V>, E> {
    let V::Obj(o) = x else { return Ok(None) };
    if let Some(f) = o.cls.lookup("__ne__") {
        let r = call(&f, Args::p(vec![x.clone(), y.clone()]))?;
        return Ok(if matches!(r, V::NotImpl) { None } else { Some(r) });
    }
    if let Some(f) = o.cls.lookup("__eq__") {
        let r = call(&f, Args::p(vec![x.clone(), y.clone()]))?;
        return Ok(if matches!(r, V::NotImpl) { None } else { Some(V::Bool(!truth(&r)?)) });
    }
    Ok(if is(x, y) { Some(V::Bool(false)) } else { None })
}
// `a != b` as a value when an operand is an object and neither wraps a builtin value: CPython's order (the
// right operand first when its class is a strict subclass), then identity. None leaves it to the general path.
pub fn obj_ne(a: &V, b: &V) -> Result<Option<V>, E> {
    if !(matches!(a, V::Obj(_)) || matches!(b, V::Obj(_))) || inner_of(a).is_some() || inner_of(b).is_some() {
        return Ok(None);
    }
    let first = reflected_first(a, b);
    if first {
        if let Some(r) = ne_side(b, a)? {
            return Ok(Some(r));
        }
    }
    if let Some(r) = ne_side(a, b)? {
        return Ok(Some(r));
    }
    if !first {
        if let Some(r) = ne_side(b, a)? {
            return Ok(Some(r));
        }
    }
    Ok(Some(V::Bool(!is(a, b))))
}
pub fn ne(a: &V, b: &V) -> Result<bool, E> {
    if let Some(r) = meta_cmp(a, b, "!=") {
        return truth(&r?);
    }
    if py_next_to_buffer(a, b) {
        return truth(&cmp_v(a, b, "!=")?);
    }
    if let Some(r) = obj_ne(a, b)? {
        return truth(&r);
    }
    if let V::Obj(o) = a {
        if let Some(f) = o.cls.lookup("__ne__") {
            let r = call(&f, Args::p(vec![a.clone(), b.clone()]))?;
            if !matches!(r, V::NotImpl) {
                return truth(&r);
            }
        }
    }
    Ok(!eq(a, b)?)
}
pub fn is(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::None, V::None) => true,
        (V::Bool(x), V::Bool(y)) => x == y,
        // an unboxed int is its value, one object per value (the language allows it for immutable types), so
        // `x is x` holds for any int; id() gives each value one id to match (rt_lib::bi_id)
        (V::Int(x), V::Int(y)) => x == y,
        (V::Big(x), V::Big(y)) => Rc::ptr_eq(x, y),
        // NaN identity comes from its payload; other floats compare by their bits (an unboxed float
        // cannot tell two equal values apart, so `x is x` holds and `1.5 is 1.5` is true, as on main).
        (V::Float(x), V::Float(y)) => {
            if x.is_nan() || y.is_nan() {
                same_nan(*x, *y)
            } else {
                x.to_bits() == y.to_bits()
            }
        }
        // CPython keeps one object for each 1-char latin-1 str; a lone surrogate is not one of them
        (V::Str(x), V::Str(y)) => Rc::ptr_eq(x, y) || (!x.is_lone() && !y.is_lone() && x.text() == y.text() && x.text().len() <= 1),
        (V::List(x), V::List(y)) => Rc::ptr_eq(x, y),
        (V::Tuple(x), V::Tuple(y)) => Tup::ptr_eq(x, y) || (x.is_empty() && y.is_empty()),
        (V::Dict(x), V::Dict(y)) => Rc::ptr_eq(x, y),
        (V::Set(x), V::Set(y)) => Rc::ptr_eq(x, y),
        (V::FSet(x), V::FSet(y)) => Rc::ptr_eq(x, y),
        (V::Cell(x), V::Cell(y)) => Rc::ptr_eq(x, y),
        (V::Func(x), V::Func(y)) => Rc::ptr_eq(x, y),
        (V::Class(x), V::Class(y)) => Rc::ptr_eq(x, y),
        (V::Obj(x), V::Obj(y)) => Rc::ptr_eq(x, y),
        (V::Deque(x), V::Deque(y)) => Rc::ptr_eq(x, y),
        (V::Iter(x), V::Iter(y)) => Rc::ptr_eq(x, y),
        (V::Ext(x), V::Ext(y)) => ExtRc::ptr_eq(x, y),
        (V::Module(x), V::Module(y)) => x == y,
        (V::NTF(x), V::NTF(y)) => Rc::ptr_eq(x, y),
        (V::Bound(x), V::Bound(y)) => Rc::ptr_eq(x, y),
        (V::BMeth(x), V::BMeth(y)) => Rc::ptr_eq(x, y),
        (V::Range(x), V::Range(y)) => Rc::ptr_eq(x, y),
        (V::Slice(x), V::Slice(y)) => Rc::ptr_eq(x, y),
        (V::View(x), V::View(y)) => Rc::ptr_eq(x, y),
        (V::Prop(x), V::Prop(y)) => Rc::ptr_eq(x, y),
        (V::NotImpl, V::NotImpl) | (V::Ellipsis, V::Ellipsis) => true,
        _ => false,
    }
}

pub fn cmp_err(op: &str, a: &V, b: &V) -> E {
    exc("TypeError", format!("'{op}' not supported between instances of '{}' and '{}'", tn(a), tn(b)))
}
// Ordering of two values for <, <=, >, >= (None when unordered, like NaN).
pub fn order(a: &V, b: &V, op: &str) -> Result<Option<std::cmp::Ordering>, E> {
    use std::cmp::Ordering::*;
    Ok(match (a, b) {
        (V::Int(x), V::Int(y)) => Some(x.cmp(y)),
        (V::Float(x), V::Float(y)) => x.partial_cmp(y),
        (V::Str(x), V::Str(y)) => Some(x.order(y)),
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
        (V::Deque(_), V::Deque(_)) => {
            let (x, y) = deque_pair(a, b).unwrap();
            seq_order(&x, &y, op)?
        }
        (V::Set(_) | V::FSet(_), V::Set(_) | V::FSet(_)) => {
            let (x, y) = (set_of(a).unwrap(), set_of(b).unwrap());
            let sub = pset_issubset(&x, &y)?;
            let sup = pset_issubset(&y, &x)?;
            match (sub, sup) {
                (true, true) => Some(Equal),
                (true, false) => Some(Less),
                (false, true) => Some(Greater),
                _ => None,
            }
        }
        // a cell orders by its contents, as CPython's cell type does
        (V::Cell(x), V::Cell(y)) => {
            let (xv, yv) = {
                let (xb, yb) = (x.borrow(), y.borrow());
                let xv = match &*xb {
                    V::Unbound => return Err(exc("ValueError", "Cell is empty".into())),
                    v => v.clone(),
                };
                let yv = match &*yb {
                    V::Unbound => return Err(exc("ValueError", "Cell is empty".into())),
                    v => v.clone(),
                };
                (xv, yv)
            };
            order(&xv, &yv, op)?
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
// Counter ordering helpers. The counts are copied out first: comparing them may run user __eq__/__lt__, which
// may change either counter.
fn counter_pair(a: &V, b: &V) -> Option<(Rc<RefCell<Dict>>, Rc<RefCell<Dict>>)> {
    match (a, b) {
        (V::Dict(x), V::Dict(y)) if x.borrow().kind == 2 && y.borrow().kind == 2 => Some((x.clone(), y.clone())),
        _ => None,
    }
}
fn counter_eq(x: &Rc<RefCell<Dict>>, y: &Rc<RefCell<Dict>>) -> Result<bool, E> {
    if Rc::ptr_eq(x, y) {
        return Ok(true);
    }
    let xs = x.borrow().items();
    let ys = y.borrow().items();
    for (k, v) in xs {
        let w = Dict::get_cell(y, &k)?.unwrap_or(V::Int(0));
        if !eq(&v, &w)? {
            return Ok(false);
        }
    }
    for (k, v) in ys {
        let w = Dict::get_cell(x, &k)?.unwrap_or(V::Int(0));
        if !eq(&v, &w)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn counter_rich(x: &Rc<RefCell<Dict>>, y: &Rc<RefCell<Dict>>, op: &str) -> Result<bool, E> {
    if op == "<" || op == ">" {
        let incl = counter_rich(x, y, if op == "<" { "<=" } else { ">=" })?;
        return Ok(incl && !counter_eq(x, y)?);
    }
    let xs = x.borrow().items();
    let ys = y.borrow().items();
    for (k, v) in xs {
        let w = Dict::get_cell(y, &k)?.unwrap_or(V::Int(0));
        if !rich(&v, &w, op)? {
            return Ok(false);
        }
    }
    for (k, v) in ys {
        let w = Dict::get_cell(x, &k)?.unwrap_or(V::Int(0));
        if !rich(&w, &v, op)? {
            return Ok(false);
        }
    }
    Ok(true)
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
    if let Some(r) = meta_cmp(a, b, op) {
        return truth(&r?);
    }
    if py_next_to_buffer(a, b) {
        return truth(&cmp_v(a, b, op)?);
    }
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
        let first = reflected_first(a, b);
        if first {
            if let V::Obj(o) = b {
                if let Some(f) = o.cls.lookup(dunder(swap_op(op))) {
                    let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                    if !matches!(r, V::NotImpl) {
                        return truth(&r);
                    }
                }
            }
        }
        if let V::Obj(o) = a {
            if let Some(f) = o.cls.lookup(dunder(op)) {
                let r = call(&f, Args::p(vec![a.clone(), b.clone()]))?;
                if !matches!(r, V::NotImpl) {
                    return truth(&r);
                }
            }
        }
        if let (V::Obj(o), false) = (b, first) {
            if let Some(f) = o.cls.lookup(dunder(swap_op(op))) {
                let r = call(&f, Args::p(vec![b.clone(), a.clone()]))?;
                if !matches!(r, V::NotImpl) {
                    return truth(&r);
                }
            }
        }
        // a bytearray subclass instance orders against a bytes-like value only; the error names the subclass
        let not_buf = |x: &V| bytes_of(x).is_none() || memview_of(x).is_some();
        if (is_ba_sub(a) && not_buf(b)) || (is_ba_sub(b) && not_buf(a)) {
            return Err(cmp_err(op, a, b));
        }
        let (ia, ib) = (inner_of(a), inner_of(b));
        if ia.is_some() || ib.is_some() {
            return rich(&ia.unwrap_or(a.clone()), &ib.unwrap_or(b.clone()), op);
        }
        return Err(cmp_err(op, a, b));
    }
    if let Some((x, y)) = counter_pair(a, b) {
        return Ok(counter_rich(&x, &y, op)?);
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
    // CPython's order (binary_op1 and SLOT1BINFULL): the reflected method of the right operand runs first
    // when its class is a strict subclass of the left operand's class and gives that method a different
    // value (its own or an intermediate class's); it is then not tried a second time. Two operands of the
    // same class never try the reflected method.
    let mut right_done = false;
    let same = match (a, b) {
        (V::Obj(x), V::Obj(y)) => {
            let same = Rc::ptr_eq(&x.cls, &y.cls);
            if !same && y.cls.is_sub(&x.cls) {
                if let Some(f) = y.cls.lookup(r) {
                    if x.cls.lookup(r).is_none_or(|g| !is(&f, &g)) {
                        right_done = true;
                        match call2(&f, b.clone(), a.clone()) {
                            Ok(V::NotImpl) => {}
                            x => return Some(x),
                        }
                    }
                }
            }
            same
        }
        _ => false,
    };
    if let V::Obj(o) = a {
        if let Some(f) = o.cls.lookup(l) {
            match call2(&f, a.clone(), b.clone()) {
                Ok(V::NotImpl) => {}
                x => return Some(x),
            }
        }
    }
    if let (V::Obj(o), false, false) = (b, same, right_done) {
        if let Some(f) = o.cls.lookup(r) {
            match call2(&f, b.clone(), a.clone()) {
                Ok(V::NotImpl) => {}
                x => return Some(x),
            }
        }
    }
    // a bytearray subclass instance (no method of its own answered): bytearray's + and * on its value, so
    // the result is a bytearray, as CPython's is (before bytes' reflected + could make it bytes)
    if is_ba_sub(a) || is_ba_sub(b) {
        // the count of a repeat: an int, or what __index__ gives (an int subclass, a class with __index__)
        fn count(v: &V) -> Option<R> {
            match v {
                V::Int(_) | V::Bool(_) | V::Big(_) => Some(Ok(v.clone())),
                // an int subclass is its value (PyNumber_Index never calls its __index__)
                V::Obj(o) if matches!(o.inner(), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => o.inner().map(Ok),
                V::Obj(o) if !is_ba_sub(v) => {
                    let f = o.cls.lookup("__index__")?;
                    Some(call(&f, Args::p(vec![v.clone()])).and_then(|r| match &r {
                        V::Int(_) | V::Bool(_) | V::Big(_) => Ok(r),
                        V::Obj(k) if matches!(k.inner(), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => Ok(k.inner().unwrap()),
                        _ => type_err(format!("__index__ returned non-int (type {})", tn(&r))),
                    }))
                }
                _ => None,
            }
        }
        match op {
            "+" if is_ba_sub(a) && bytes_of(b).is_some() => return Some(binop(&ba_val(a).unwrap(), b, op)),
            "*" if is_ba_sub(a) => {
                if let Some(n) = count(b) {
                    return Some(n.and_then(|n| binop(&ba_val(a).unwrap(), &n, op)));
                }
            }
            "*" if is_ba_sub(b) => {
                if let Some(n) = count(a) {
                    return Some(n.and_then(|n| binop(&n, &ba_val(b).unwrap(), op)));
                }
            }
            _ => {}
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
    // nothing else takes a bytearray subclass instance: CPython's errors, naming the subclass
    if is_ba_sub(a) || is_ba_sub(b) {
        // (other operators: binop's own errors, which do not unwrap the instance)
        return match op {
            "+" if is_ba_sub(a) => Some(type_err(format!("can't concat {} to {}", tn(b), tn(a)))),
            "*" if is_ba_sub(a) => Some(type_err(format!("can't multiply sequence by non-int of type '{}'", tn(b)))),
            "*" => Some(type_err(format!("can't multiply sequence by non-int of type '{}'", tn(a)))),
            _ => None,
        };
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
        "+" => Some(V::Float(fresh_float(x + y))),
        "-" => Some(V::Float(fresh_float(x - y))),
        "*" => Some(V::Float(fresh_float(x * y))),
        "/" if y != 0.0 => Some(V::Float(fresh_float(x / y))),
        _ => None,
    }
}
// int OP int that cannot overflow or raise; op is a constant at almost every call site, so this folds away
#[inline(always)]
fn int_fast(a: &V, b: &V, op: &str) -> Option<V> {
    let (V::Int(x), V::Int(y)) = (a, b) else {
        return None;
    };
    let (x, y) = (*x, *y);
    match op {
        "+" => x.checked_add(y).map(V::Int),
        "-" => x.checked_sub(y).map(V::Int),
        "*" => x.checked_mul(y).map(V::Int),
        // Python's floor modulo: the result takes the divisor's sign
        "%" if y > 0 => Some(V::Int(x.rem_euclid(y))),
        "//" if y > 0 => Some(V::Int(x.div_euclid(y))),
        _ => None,
    }
}
#[inline]
pub fn binop(a: &V, b: &V, op: &str) -> R {
    if let Some(r) = int_fast(a, b, op) {
        return Ok(r);
    }
    if let Some(r) = float_fast(a, b, op) {
        return Ok(r);
    }
    binop_slow(a, b, op)
}
#[inline(never)]
fn binop_slow(a: &V, b: &V, op: &str) -> R {
    // `int | str`, `list[int] | None`, `typing.List[int] | str`: a union of types
    if op == "|" && (matches!(a, V::Class(_) | V::Ext(_) | V::None) && matches!(b, V::Class(_) | V::Ext(_) | V::None)) {
        if let Some(r) = union_or(a, b) {
            return r;
        }
    }
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
                    if x == 0 {
                        return Err(exc("ZeroDivisionError", "0.0 cannot be raised to a negative power".into()));
                    }
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
                        if p.is_zero() {
                            return Err(exc("ZeroDivisionError", "0.0 cannot be raised to a negative power".into()));
                        }
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
        // CPython: an empty left gives the right str, else an empty right gives the left (the str itself)
        (V::Str(x), V::Str(_)) if op == "+" && x.nchars() == 0 => return Ok(b.clone()),
        (V::Str(_), V::Str(y)) if op == "+" && y.nchars() == 0 => return Ok(a.clone()),
        (V::Str(x), V::Str(y)) if op == "+" => return Ok(V::Str(Rc::new(x.concat(y)))),
        (V::List(x), V::List(y)) if op == "+" => {
            let mut v = x.borrow().clone();
            v.extend(y.borrow().iter().cloned());
            return Ok(list(v));
        }
        // an empty right tuple gives the left tuple itself, else an empty left gives the right one
        (V::Tuple(_), V::Tuple(y)) if op == "+" && y.is_empty() => return Ok(a.clone()),
        (V::Tuple(x), V::Tuple(_)) if op == "+" && x.is_empty() => return Ok(b.clone()),
        (V::Tuple(x), V::Tuple(y)) if op == "+" => {
            let mut v = x.to_vec();
            v.extend(y.iter().cloned());
            return Ok(tup(v));
        }
        (V::Str(_) | V::List(_) | V::Tuple(_), n) | (n, V::Str(_) | V::List(_) | V::Tuple(_)) if op == "*" && is_intlike(n) => {
            let (sq, n) = if is_intlike(a) { (b, a) } else { (a, b) };
            let k = ival(n).unwrap_or(0).max(0) as usize;
            // * 1 on an exact str or tuple is the object itself
            if k == 1 && matches!(sq, V::Str(_) | V::Tuple(_)) && matches!(n, V::Int(_) | V::Bool(_)) {
                return Ok(sq.clone());
            }
            let len = match sq {
                V::Str(x) if x.is_lone() => x.nchars() * 4,
                V::Str(x) => x.text().len(),
                V::List(x) => x.borrow().len(),
                V::Tuple(x) => x.len(),
                _ => 0,
            };
            // `[0] * sys.maxsize`: CPython raises MemoryError instead of trying.
            if len.checked_mul(k).map_or(true, |t| t > (1usize << 40)) {
                return Err(exc("MemoryError", String::new()));
            }
            return Ok(match sq {
                V::Str(x) if x.is_lone() => st_cps(x.lone().unwrap().repeat(k)),
                V::Str(x) => V::Str(Rc::new(S::with_ascii(x.text().repeat(k), x.ascii()))),
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
        // str/list/tuple raise their own TypeError for a non-int multiplier ('x' * b'y' says "of type 'bytes'"),
        // after the other operand's __rmul__ had its turn: an instance or library value goes on to the reflected path.
        (V::Str(_) | V::List(_) | V::Tuple(_), _) if op == "*" && (bytes_of(b).is_some() || !matches!(b, V::Obj(_) | V::Ext(_))) => {
            return type_err(format!("can't multiply sequence by non-int of type '{}'", tn(b)));
        }
        // "%s" % s and "%s" % (s,) with an exact str: the str itself
        (V::Str(f), _) if op == "%" && f.text() == "%s" && !f.is_lone() && pct_one_str(b).is_some() => return Ok(pct_one_str(b).unwrap()),
        (V::Str(f), _) if op == "%" => return Ok(V::Str(Rc::new(percent_format(f, b)?))),
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
                        PSet::from_dict(ks)?
                    }
                    _ => PSet::from_iter(to_vec(a)?)?,
                },
                _ => PSet::from_iter(to_vec(a)?)?,
            };
            match b {
                V::Set(_) | V::FSet(_) => r.merge(&set_of(b).unwrap())?,
                V::Dict(d) if d.borrow().kind == 0 => {
                    let ks = d.borrow().keys_h();
                    r.update_dict(ks)?
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
        (V::Dict(x), V::Dict(y)) if matches!(op, "+" | "-" | "|" | "&") && x.borrow().kind == 2 && y.borrow().kind == 2 => {
            return counter_op(x, y, op);
        }
        (V::Dict(x), V::Dict(y)) if op == "|" && (x.borrow().kind == 3 || y.borrow().kind == 3) => {
            return odict_or(a, b).unwrap();
        }
        (V::Dict(x), V::Dict(y)) if op == "|" => {
            let mut d = x.borrow().copy();
            let items = y.borrow().items();
            for (k, v) in items {
                d.set(k, v)?;
            }
            return Ok(V::Dict(Rc::new(RefCell::new(d))));
        }
        (V::View(x), _) if x.1 == 3 && op == "|" => return binop(&x.0, b, op),
        (_, V::View(y)) if y.1 == 3 && op == "|" => return binop(a, &y.0, op),
        // deque + deque keeps the left one's maxlen; deque * n and n * deque keep the last maxlen items
        (V::Deque(x), V::Deque(_)) if op == "+" => return deque_concat(x, b),
        (V::Deque(x), n) | (n, V::Deque(x)) if op == "*" && is_intlike(n) => return deque_mul(x, n),
        // (an instance or library value on the right gets its reflected method first)
        (V::Deque(x), _) if op == "+" && !matches!(b, V::Obj(_) | V::Ext(_)) => return deque_concat(x, b),
        (V::Deque(_), _) if op == "*" && !matches!(b, V::Obj(_) | V::Ext(_)) => {
            return type_err(format!("can't multiply sequence by non-int of type '{}'", tn(b)));
        }
        _ => {}
    }
    if let Some(r) = obj_binop(a, b, op) {
        return r;
    }
    if let Some(r) = deque_sub_binop(a, b, op) {
        return r;
    }
    if op == "|" {
        if let Some(r) = odict_or(a, b) {
            return r;
        }
    }
    let (ia, ib) = (inner_of(a), inner_of(b));
    // (a bytearray subclass instance is not unwrapped: obj_binop took what bytearray takes, and errors name the subclass)
    if (ia.is_some() || ib.is_some()) && !is_ba_sub(a) && !is_ba_sub(b) {
        let (xa, xb) = (ia.clone().unwrap_or(a.clone()), ib.clone().unwrap_or(b.clone()));
        let mut r = binop(&xa, &xb, op)?;
        if let Some(i) = &ia {
            r = sub_res(r, i, a, op);
        }
        if let Some(i) = &ib {
            r = sub_res(r, i, b, op);
        }
        // CPython's date arithmetic gives back type(self)(...), an instance of the subclass (date_sub_refused)
        if (is_date_sub(a) || is_date_sub(b)) && is_date_v(&r) {
            return Err(unsupported("date arithmetic on an Enum member that wraps a date (CPython builds the Enum's class from the result)"));
        }
        return Ok(r);
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
    Ok(match op {
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
                return Err(exc("ZeroDivisionError", "0.0 cannot be raised to a negative power".into()));
            }
            if x < 0.0 && y.fract() != 0.0 {
                // float_pow: a negative number to a fractional power is complex_pow's
                if x.is_finite() && y.is_finite() {
                    return c_pow((x, 0.0), (y, 0.0));
                }
                return Err(unsupported("a negative float to an infinite or NaN power"));
            }
            let r = x.powf(y);
            if r.is_infinite() && x.is_finite() && y.is_finite() {
                return Err(exc("OverflowError", "(34, 'Numerical result out of range')".into()));
            }
            r
        }
        _ => return Err(op_err(op, a, b)),
    }).map(|r| V::Float(fresh_float(r)))
}
// the one exact str that a "%s" argument is (the str, or a 1-tuple of it)
fn pct_one_str(b: &V) -> Option<V> {
    match b {
        V::Str(_) => Some(b.clone()),
        V::Tuple(t) if t.len() == 1 && matches!(t[0], V::Str(_)) => Some(t[0].clone()),
        _ => None,
    }
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
    if let Some(r) = int_fast(a, b, op) {
        return Ok(r);
    }
    if let Some(r) = float_fast(a, b, op) {
        return Ok(r);
    }
    iop_slow(a, b, op)
}
#[inline(never)]
fn iop_slow(a: &V, b: &V, op: &str) -> R {
    if let Some(r) = bytearray_iop(a, b, op) {
        return r;
    }
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
    // od |= x (CPython's odict_inplace_or): an update through __setitem__
    if op == "|" && odict_of(a).is_some() {
        odict_update_arg(a, b)?;
        return Ok(a.clone());
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
            // No borrow of s is held while a key's __eq__ runs: it may change s (or b).
            let other = set_of(b).unwrap();
            match op {
                "|" => PSet::cell_merge(s, &other)?,
                "-" => {
                    for (k, h) in other.keys_h() {
                        PSet::cell_discard_h(s, &k, h)?;
                    }
                }
                "&" => {
                    // remove the others (keeps table positions)
                    let mine = s.borrow().keys_h();
                    for (k, h) in mine {
                        if other.find(&k, h)?.is_none() {
                            PSet::cell_discard_h(s, &k, h)?;
                        }
                    }
                }
                _ => {
                    let cur = s.borrow().clone();
                    let r = set_op(&cur, &other, op)?;
                    PSet::cell_replace(s, r);
                }
            }
            return Ok(a.clone());
        }
        (V::Dict(d), cop @ ("+" | "-" | "|" | "&")) if d.borrow().kind == 2 => {
            let r = binop(a, b, cop)?;
            if let V::Dict(r) = r {
                let r = r.borrow().copy();
                *d.borrow_mut() = r;
                d.borrow_mut().kind = 2;
            }
            return Ok(a.clone());
        }
        (V::Dict(d), "|") => {
            let items = dict_items_of(b)?;
            for (k, v) in items {
                Dict::set_cell(d, k, v)?;
            }
            return Ok(a.clone());
        }
        (V::Deque(d), "*") if !matches!(b, V::Obj(_) | V::Ext(_)) => {
            deque_imul(d, b)?;
            return Ok(a.clone());
        }
        (V::Deque(d), "+") => {
            deque_extend(d, b, false)?;
            return Ok(a.clone());
        }
        _ => {}
    }
    binop(a, b, op)
}
fn counter_op(x: &Rc<RefCell<Dict>>, y: &Rc<RefCell<Dict>>, op: &str) -> R {
    let mut d = Dict::new();
    d.kind = 2;
    let xs = x.borrow().items();
    let ys = y.borrow().items();
    for (k, v) in xs {
        let w = Dict::get_cell(y, &k)?.unwrap_or(V::Int(0));
        let r = match op {
            "+" => binop(&v, &w, "+")?,
            "-" => binop(&v, &w, "-")?,
            "|" => if rich(&v, &w, "<")? { w } else { v },
            "&" => if rich(&v, &w, "<")? { v } else { w },
            _ => return Err(op_err(op, &V::Dict(x.clone()), &V::Dict(y.clone()))),
        };
        if rich(&r, &V::Int(0), ">")? {
            d.set(k, r)?;
        }
    }
    // The right operand's keys the left does not have: + keeps positive counts, - keeps flipped negative
    // counts, | keeps positive counts, & has nothing (the left missing count is zero, the minimum).
    if matches!(op, "+" | "|" | "-") {
        for (k, v) in ys {
            if !Dict::contains_cell(x, &k)? {
                let r = if op == "-" { neg(&v)? } else { v };
                if rich(&r, &V::Int(0), ">")? {
                    d.set(k, r)?;
                }
            }
        }
    }
    Ok(V::Dict(Rc::new(RefCell::new(d))))
}
// unary + and -: a new Counter with only positive counts (+), or the flipped positive counts (-).
fn counter_unary(x: &Rc<RefCell<Dict>>, negate: bool) -> R {
    let mut d = Dict::new();
    d.kind = 2;
    for (k, v) in x.borrow().items() {
        let r = if negate { neg(&v)? } else { v };
        if rich(&r, &V::Int(0), ">")? {
            d.set(k, r)?;
        }
    }
    Ok(V::Dict(Rc::new(RefCell::new(d))))
}
// a set's snapshot, a plain dict's keys with their stored hashes (CPython hashes neither again), or the
// items of any other iterable
pub fn pset_of_iterable(v: &V) -> Result<PSet, E> {
    match v {
        V::Set(_) | V::FSet(_) => Ok(set_of(v).unwrap()),
        V::Dict(d) if d.borrow().kind == 0 => {
            let ks = d.borrow().keys_h();
            PSet::from_dict(ks)
        }
        _ => PSet::from_iter(to_vec(v)?),
    }
}
pub fn set_of(v: &V) -> Option<PSet> {
    match v {
        V::Set(s) => Some(s.borrow().clone()),
        V::FSet(s) => Some((**s).clone()),
        _ => None,
    }
}
// every key of x is in y (CPython's set_issubset; a key's __eq__ error is passed on)
pub fn pset_issubset(x: &PSet, y: &PSet) -> Result<bool, E> {
    if x.used > y.used {
        return Ok(false);
    }
    for (k, h) in x.keys_h() {
        if y.find(&k, h)?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}
// CPython's set algebra, so results come out in the same table order.
pub fn set_op(a: &PSet, b: &PSet, op: &str) -> Result<PSet, E> {
    Ok(match op {
        "|" => {
            let mut r = a.copy();
            r.merge(b)?;
            r
        }
        "&" => {
            let (small, big) = if a.used > b.used { (b, a) } else { (a, b) };
            let mut r = PSet::new();
            for (k, h) in small.keys_h() {
                if big.find(&k, h)?.is_some() {
                    r.add_h(k, h)?;
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
                    if b.find(&k, h)?.is_none() {
                        r.add_h(k, h)?;
                    }
                }
                r
            }
        }
        _ => {
            let mut r = a.copy();
            for (k, h) in b.keys_h() {
                match r.probe(&k, h)? {
                    Probe::Found(i) => r.remove_at(i),
                    Probe::Absent(at, reuse) => r.insert_at(at, reuse, k, h),
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
        V::Float(f) => {
            let r = -*f;
            Ok(V::Float(fresh_float(r)))
        }
        V::Dict(d) if d.borrow().kind == 2 => counter_unary(d, true),
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
        V::Dict(d) if d.borrow().kind == 2 => counter_unary(d, false),
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
    if let Some(__p) = proxy_deref(v) {
        return len(&__p?);
    }
    Ok(match v {
        V::Class(c) if c.meta.borrow().is_some() => return meta_len(c, v),
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
            None if memview_of(v).is_some() => return Err(exc("ValueError", "operation forbidden on released memoryview object".into())),
            None if bigrange_of(v).is_some() => match call_method(v, "__len__", Args::p(vec![]))? {
                V::Int(i) => i as usize,
                _ => return type_err("'__len__' returned a bad value".into()),
            },
            None => return type_err(format!("object of type '{}' has no len()", tn(v))),
        },
        _ => return type_err(format!("object of type '{}' has no len()", tn(v))),
    })
}
pub fn contains(c: &V, x: &V) -> Result<bool, E> {
    if let Some(__p) = proxy_deref(c) {
        return contains(&__p?, x);
    }
    // `x in seen` for a set first: the special cases below are all for str containers
    if let V::Set(s) = c {
        return PSet::cell_contains(s, x);
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
            V::Str(n) => s.has(n),
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
        V::Dict(d) => Dict::contains_cell(d, x)?,
        V::Set(s) => PSet::cell_contains(s, x)?,
        V::FSet(s) => s.find(&as_set_key(x), set_key_hash(x)?)?.is_some(),
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
            (V::Dict(d), 0 | 3) => Dict::contains_cell(d, x)?,
            (V::Dict(d), 2) => return items_view_contains(d, x),
            // CPython's dict_values.__contains__ compares with the identity shortcut: an identical
            // value (a NaN object, say) is a hit before __eq__ runs.
            _ => to_vec(c)?.iter().any(|y| is(y, x) || eq(y, x).unwrap_or(false)),
        },
        V::Deque(d) => deque_contains(d, x)?,
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__contains__") {
                return truth(&call(&f, Args::p(vec![c.clone(), x.clone()]))?);
            }
            if let Some(i) = inner_of(c) {
                return contains(&i, x);
            }
            for y in to_vec(c)? {
                if is(&y, x) || eq(&y, x)? {
                    return Ok(true);
                }
            }
            false
        }
        V::Iter(_) => {
            let mut it = iter(c)?;
            while let Some(y) = it.next()? {
                if is(&y, x) || eq(&y, x)? {
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
    // a proxy's referent never has __index__ (weakref.proxy refuses one that does), so CPython's
    // PyNumber_Index of the referent raises
    if let Some(__p) = proxy_deref(i) {
        return type_err(format!("'{}' object cannot be interpreted as an integer", tn(&__p?)));
    }
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
    s.text().chars().collect()
}
// A class subscripted in a dynamic build (list[int], a namedtuple's Group[int, list[int]]): the origin
// class, the args, and the few attributes CPython's types.GenericAlias exposes.
struct GAlias {
    origin: Rc<Class>,
    args: Vec<V>,
}
// cls[item] for a class whose __class_getitem__ is types.GenericAlias
pub fn class_alias(k: &Rc<Class>, i: &V) -> V {
    let args = match i {
        V::Tuple(t) => t.to_vec(),
        other => vec![other.clone()],
    };
    V::Ext(ExtRc::new(GAlias { origin: k.clone(), args }))
}
fn alias_arg_repr(v: &V) -> String {
    match v {
        V::Class(k) if k.module == "builtins" => k.name.to_string(),
        V::Class(k) => format!("{}.{}", k.module, k.name),
        _ => repr(v).unwrap_or_default(),
    }
}
impl Ext for GAlias {
    fn tname(&self) -> &'static str {
        "types.GenericAlias"
    }
    fn repr(&self) -> String {
        let oname = if self.origin.module == "builtins" {
            self.origin.name.to_string()
        } else {
            format!("{}.{}", self.origin.module, self.origin.name)
        };
        format!("{oname}[{}]", self.args.iter().map(alias_arg_repr).collect::<Vec<_>>().join(", "))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "__origin__" => Ok(V::Class(self.origin.clone())),
            "__args__" => Ok(tup(self.args.clone())),
            "__parameters__" => Ok(tup(vec![])),
            "__class__" => return None,
            // CPython's ga_getattro keeps these on the alias; every other name is the origin class's
            "__unpacked__" | "__typing_unpacked_tuple_args__" | "__mro_entries__" | "__reduce_ex__" | "__reduce__" | "__copy__" | "__deepcopy__" => {
                Err(unsupported(&format!("types.GenericAlias.{n}")))
            }
            _ => getattr(&V::Class(self.origin.clone()), n),
        })
    }
    fn eq(&self, o: &V) -> Option<bool> {
        let V::Ext(e) = o else { return Some(false) };
        let Some(b) = e.any().downcast_ref::<GAlias>() else { return Some(false) };
        Some(Rc::ptr_eq(&self.origin, &b.origin) && self.args.len() == b.args.len() && self.args.iter().zip(&b.args).all(|(x, y)| eq(x, y).unwrap_or(false)))
    }
    fn hash(&self) -> Option<i64> {
        Some(py_hash(&V::Class(self.origin.clone())).ok()? ^ py_hash(&tup(self.args.clone())).ok()?)
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        if n == "__call__" {
            return Some(instantiate(&self.origin, Args { p: a.p.clone(), k: a.k.clone() }));
        }
        None
    }
    fn instancecheck(&self, _x: &V) -> Option<Result<bool, E>> {
        Some(type_err("isinstance() argument 2 cannot be a parameterized generic".into()))
    }
}

pub fn getitem(c: &V, i: &V) -> R {
    if let Some(__p) = proxy_deref(c) {
        return getitem(&__p?, i);
    }
    if matches!(c, V::List(_) | V::Tuple(_) | V::Str(_) | V::Range(_)) {
        if proxy_deref(i).is_some() {
            idx_of(i)?;
        }
    }
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
            let got = Dict::get_cell(d, i)?;
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
                Dict::set_cell(d, i.clone(), v.clone())?;
                return Ok(v);
            }
            return Err(exc_v("KeyError", i.clone()));
        }
        (V::Str(s), V::Int(k)) if s.is_lone() => {
            let v = s.lone().unwrap();
            let j = norm_idx(*k, v.len(), "string")?;
            return Ok(cp1(v[j]));
        }
        (V::Str(sx), V::Slice(sl)) if sx.is_lone() => {
            let v = sx.lone().unwrap();
            let ps = slice_positions(sl, v.len())?;
            return Ok(st_cps(ps.iter().map(|&p| v[p]).collect()));
        }
        (V::Str(s), V::Int(k)) => {
            if s.ascii() {
                let j = norm_idx(*k, s.text().len(), "string")?;
                return Ok(ch1(s.text().as_bytes()[j] as char));
            }
            let n = s.nchars();
            let j = norm_idx(*k, n, "string")?;
            return Ok(ch1(s.text().chars().nth(j).unwrap()));
        }
        (V::Str(sx), V::Slice(sl)) => {
            if sx.ascii() {
                let b = sx.text().as_bytes();
                let (a, e, stp) = slice_idx(sl, b.len())?;
                if stp == 1 {
                    // the whole str: the str itself
                    if a <= 0 && e >= b.len() as i64 {
                        return Ok(c.clone());
                    }
                    let (a, e) = (a.max(0) as usize, e.max(0) as usize);
                    return Ok(if a >= e { s("") } else { st_ascii(&sx.text()[a..e]) });
                }
                let ps = slice_positions(sl, b.len())?;
                return Ok(st(ps.iter().map(|&p| b[p] as char).collect()));
            }
            let cs = str_chars(sx);
            let (a0, e0, st0) = slice_idx(sl, cs.len())?;
            if st0 == 1 && a0 <= 0 && e0 >= cs.len() as i64 {
                return Ok(c.clone());
            }
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
            // the whole tuple: the tuple itself
            let (a0, e0, st0) = slice_idx(sl, t.len())?;
            if st0 == 1 && a0 <= 0 && e0 >= t.len() as i64 {
                return Ok(c.clone());
            }
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
        (V::Deque(d), _) => return deque_getitem(d, i),
        (V::View(x), _) if x.1 == 3 => return getitem(&x.0, i),
        (V::Obj(o), _) => {
            if let Some(f) = o.cls.lookup("__getitem__") {
                return call(&f, Args::p(vec![c.clone(), i.clone()]));
            }
            if let Some(inn) = inner_of(c) {
                // A dict subclass with `__missing__`: a missing key calls it (`class D(dict)`).
                if let (V::Dict(d), Some(f)) = (&inn, o.cls.lookup("__missing__")) {
                    let got = Dict::get_cell(d, i)?;
                    return match got {
                        Some(v) => Ok(v),
                        None => call(&f, Args::p(vec![c.clone(), i.clone()])),
                    };
                }
                let r = getitem(&inn, i)?;
                return Ok(sub_res(r, &inn, c, "__getitem__"));
            }
            return type_err(format!("'{}' object is not subscriptable", tn(c)));
        }
        (V::Class(k), _) if is_enum(k) => return enum_getitem(k, i),
        (V::Class(k), _) if k.lookup("__bv_namedtuple__").is_some() || k.lookup("__bv_galias__").is_some() => {
            let args = match i {
                V::Tuple(t) => t.to_vec(),
                other => vec![other.clone()],
            };
            return Ok(V::Ext(ExtRc::new(GAlias { origin: k.clone(), args })));
        }
        (V::Class(k), _) if k.module == "builtins" && matches!(k.name, "list" | "dict" | "set" | "frozenset" | "tuple") => {
            let args = match i {
                V::Tuple(t) => t.to_vec(),
                other => vec![other.clone()],
            };
            return Ok(V::Ext(ExtRc::new(GAlias { origin: k.clone(), args })));
        }
        // collections.abc.Mapping[str, int] (or a subclass of an ABC): a types.GenericAlias, as
        // __class_getitem__ = classmethod(GenericAlias) makes it. Callable's alias flattens its arguments
        // (_CallableGenericAlias), which this one does not do, so Callable[...] is refused.
        (V::Class(k), _) if k.lookup("__class_getitem__").is_some() => {
            if let Some(g) = k.lookup("__class_getitem__") {
                // classmethod(GenericAlias): the alias itself (Brevi's GenericAlias type is not callable)
                if let V::Func(gf) = &g {
                    if gf.kind.get() == 2 && matches!(gf.attrs.borrow().iter().find(|(n, _)| *n == "__func__"), Some((_, V::Class(t))) if t.name == "types.GenericAlias") {
                        return Ok(class_alias(k, i));
                    }
                    // a plain def __class_getitem__ is an implicit classmethod (type_new wraps it)
                    if gf.kind.get() == 0 {
                        return call(&g, Args::p(vec![c.clone(), i.clone()]));
                    }
                }
                let f = getattr(c, "__class_getitem__")?;
                return call(&f, Args::p(vec![i.clone()]));
            }
            if k.mro.borrow().iter().any(|b| b.module == "collections.abc" && b.name == "Callable") {
                return Err(exc("NotImplementedError", "brevi: collections.abc.Callable[...] is not supported in dynamic builds".into()));
            }
            let args = match i {
                V::Tuple(t) => t.to_vec(),
                other => vec![other.clone()],
            };
            return Ok(V::Ext(ExtRc::new(GAlias { origin: k.clone(), args })));
        }
        (V::Class(k), _) if k.module == "typing" => return typing_getitem(k, i),
        (V::Class(k), _) => return class_subscript(k, i),
        // list[int][str], typing.List[T][int] and the like: substitution CPython checks and Brevi does not make
        (V::Ext(x), _) if matches!(x.tname(), "types.GenericAlias" | "typing._GenericAlias" | "typing._UnionGenericAlias" | "types.UnionType") => {
            return Err(unsupported(&format!("subscripting a {}", x.tname())));
        }
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
        // a builtin of the runtime may be a class in CPython (enumerate, zip, property), which may subscript
        (V::Func(_), _) if tn(c) != "function" => Err(unsupported(&format!("subscripting {}", tn(c)))),
        _ => type_err(format!("'{}' object is not subscriptable", tn(c))),
    }
}
fn st_ascii(x: &str) -> V {
    if x.len() == 1 {
        return ch1(x.as_bytes()[0] as char);
    }
    V::Str(Rc::new(S::new_ascii(x.to_string())))
}
pub fn setitem(c: &V, i: &V, v: V) -> Result<(), E> {
    if let Some(__p) = proxy_deref(c) {
        return setitem(&__p?, i, v);
    }
    if matches!(c, V::List(_)) {
        if proxy_deref(i).is_some() {
            idx_of(i)?;
        }
    }
    match (c, i) {
        (V::List(l), V::Int(k)) => {
            let mut l = l.borrow_mut();
            let j = norm_idx(*k, l.len(), "list assignment")?;
            l[j] = v;
            Ok(())
        }
        (V::Dict(d), _) => Dict::set_cell(d, i.clone(), v),
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
        (V::Deque(d), _) => deque_setitem(d, i, v),
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
        (V::Ext(_), _) if is_bytes_v(c) => type_err("'bytes' object does not support item assignment".into()),
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
    if let Some(__p) = proxy_deref(c) {
        return delitem(&__p?, i);
    }
    if matches!(c, V::List(_)) {
        if proxy_deref(i).is_some() {
            idx_of(i)?;
        }
    }
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
        (V::Dict(d), _) => {
            // Counter.__delitem__ ignores missing keys; dict.__delitem__ raises KeyError.
            if d.borrow().kind == 2 && !Dict::contains_cell(d, i)? {
                return Ok(());
            }
            match Dict::del_cell(d, i)? {
                Some(_) => Ok(()),
                None => Err(exc_v("KeyError", i.clone())),
            }
        }
        (V::Deque(d), _) => deque_delitem(d, i),
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
        (V::Ext(_), _) if is_bytes_v(c) => type_err("'bytes' object doesn't support item deletion".into()),
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

// The Python class of an iterator value. `It`'s variants do not all map one-to-one onto CPython's
// iterator types (several builtins produce the same It::Vec / It::Gen / It::Shared shape), so the
// constructor that makes an exposed V::Iter records the kind next to the cell. Obj resolves to the
// wrapped iterator's own class; Unknown means Brevi cannot say which class CPython would report, and
// type() of such an iterator raises instead of guessing.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ItKind {
    ListIter,
    TupleIter,
    StrAsciiIter,
    StrIter,
    RangeIter,
    DictKeys,
    DictValues,
    DictItems,
    SetIter,
    ListReverseIter,
    Reversed,
    DictReverseKeys,
    DictReverseValues,
    DictReverseItems,
    Map,
    Filter,
    Zip,
    Enumerate,
    Generator,
    CallableIterator,
    BytesIter,
    DequeIter,
    // an OrderedDict's iterators (its keys, values, items, and reversed): CPython's one odict_iterator type
    OdictIter,
    Iterator,
    ItCount,
    ItChain,
    ItIslice,
    ItRepeat,
    ItCycle,
    ItAccumulate,
    ItZipLongest,
    ItStarmap,
    ItTakewhile,
    ItDropwhile,
    ItPairwise,
    ItCompress,
    ItFilterfalse,
    ItGroupby,
    ItGrouper,
    ItProduct,
    ItPermutations,
    ItCombinations,
    ItCombinationsWithReplacement,
    ItTee,
    ItBatched,
    Unknown,
}

pub enum It {
    List(Rc<RefCell<Vec<V>>>, usize),
    // reversed() of a list: live like CPython's list_reverseiterator (it sees later writes, and stops early
    // if the list shrinks); the number is the next index plus one, so 0 means exhausted
    RevList(Rc<RefCell<Vec<V>>>, usize),
    Tuple(Tup, usize),
    Vec(std::vec::IntoIter<V>),
    // itertools.repeat: the element and the calls left (None = forever), so repr() shows CPython's
    // remaining count even after the iterator is exhausted.
    Repeat(V, Option<i64>),
    Str(Rc<S>, usize),
    Range(i64, i64, i64),
    Dict(Rc<RefCell<Dict>>, usize, u8, u64, usize),
    // reversed() of a dict or one of its views, live like CPython's dict reverse iterators: the next entry
    // index plus one (0: none left), the view kind, the dict's size when it began (-1 once a size change was
    // seen, so later next() calls raise too, as CPython's di_used = -1) and its key version then
    RevDict(Rc<RefCell<Dict>>, usize, u8, i64, u64),
    // an OrderedDict or one of its views, as CPython's odict_iterator: the next entry's index (usize::MAX once
    // done), the view kind (0 keys, 1 values, 2 items; 4 added when reversed), and the dict's key version and size
    // when it began (the size -1 once a change was seen, so later next() calls raise too)
    ODict(Rc<RefCell<Dict>>, usize, u8, u64, i64),
    Set(Rc<RefCell<PSet>>, usize, i64),
    // a deque's iterator or reverse iterator: next position, items left, the deque's state when it began, reversed
    Deq(Rc<RefCell<Deq>>, usize, usize, u64, bool),
    Gen(Box<dyn FnMut() -> Result<Option<V>, E>>),
    // A generator expression: like Gen, but an error escapes to the caller once and then the iterator is
    // exhausted, as in CPython. `name` is the genexp's dotted __qualname__ (f.<locals>.<genexpr>).
    GenExp(Box<dyn FnMut() -> Result<Option<V>, E>>, Sym, bool),
    Coro(Rc<RefCell<Box<Coro>>>),
    Obj(V),
    // another iterator object, stepped through iter_step so that re-entrant calls work
    Ref(Rc<RefCell<It>>),
    // an iterator whose state lives behind its own cells and holds no borrow while it calls user code, so
    // it may be re-entered (from a key function or __eq__, say), as CPython's C iterators may
    Shared(Rc<dyn Fn() -> Result<Option<V>, E>>),
    // one iterator of an itertools.tee: its state (position) and the shared source live in TeePos/TeeData,
    // stepped through tee_step so re-entrant calls are seen there, as CPython's tee sees them
    Tee(Rc<TeePos>),
    // an iterator whose state CPython pickles and copies (most of itertools, map, filter, zip, enumerate):
    // the state lives behind its own cells, so stepping holds no borrow while it calls user code, and
    // __reduce__/__setstate__ read and write the same state (see rt_itred.rs)
    Red(Rc<dyn RedIt>),
    Done,
}
pub struct Iter {
    pub it: It,
}
// The Python class of an exposed iterator: one of the static kinds above, or a class resolved when the
// iterator was made (a user iterator's own class, or the type CPython gives iter() of a bridged value).
pub enum IterClass {
    Kind(ItKind),
    Class(Rc<Class>),
}
// A V::Iter holds the cell behind an IterCell whose class survives whatever state changes It goes
// through (exhaustion, GC clearing), so type() keeps reporting the class it was created with. The
// inner Rc is shared with It::Ref and re-entrant builtins (zip, map), which need a cell of their own.
pub struct IterCell {
    pub kind: IterClass,
    pub it: Rc<RefCell<It>>,
}
pub fn new_iter(it: It, kind: IterClass) -> V {
    V::Iter(Rc::new(IterCell { kind, it: Rc::new(RefCell::new(it)) }))
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
            It::RevList(l, i) => {
                let got = if *i > 0 { l.borrow().get(*i - 1).cloned() } else { None };
                if got.is_some() {
                    *i -= 1;
                    Ok(got)
                } else {
                    *self = It::Done;
                    Ok(None)
                }
            }
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
            It::Deq(d, i, left, st, rev) => deque_iter_next(d, i, left, *st, *rev),
            It::Repeat(x, n) => match n {
                Some(k) if *k == 0 => Ok(None),
                Some(k) => {
                    *n = Some(*k - 1);
                    Ok(Some(x.clone()))
                }
                None => Ok(Some(x.clone())),
            },
            // a str with a lone surrogate: `i` counts code points
            It::Str(s, i) if s.is_lone() => {
                let v = s.lone().unwrap();
                if *i >= v.len() {
                    return Ok(None);
                }
                *i += 1;
                Ok(Some(cp1(v[*i - 1])))
            }
            It::Str(s, i) => {
                if *i >= s.text().len() {
                    return Ok(None);
                }
                if s.ascii() {
                    *i += 1;
                    return Ok(Some(ch1(s.text().as_bytes()[*i - 1] as char)));
                }
                let c = s.text()[*i..].chars().next().unwrap();
                *i += c.len_utf8();
                Ok(Some(ch1(c)))
            }
            It::Range(a, b, st) => {
                if (*st > 0 && *a < *b) || (*st < 0 && *a > *b) {
                    let v = *a;
                    // a step past the i64 range is past stop too: the range is done
                    *a = a.checked_add(*st).unwrap_or(*b);
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
            It::ODict(d, i, kind, ver0, n0) => {
                // CPython's odictiter_iternext: done stays done; a changed key set (od_state: a key added,
                // deleted or moved) stops the iterator; a changed size raises on every later call; values and
                // items look each key up again, so a key whose hash changed raises KeyError
                if *i == usize::MAX {
                    return Ok(None);
                }
                let k = {
                    let dd = d.borrow();
                    if dd.ver != *ver0 {
                        *i = usize::MAX;
                        return Err(exc("RuntimeError", "OrderedDict mutated during iteration".into()));
                    }
                    if *n0 < 0 || dd.n as i64 != *n0 {
                        *n0 = -1;
                        return Err(exc("RuntimeError", "OrderedDict changed size during iteration".into()));
                    }
                    let Some(Some((k, _))) = dd.ents.get(*i) else {
                        *i = usize::MAX;
                        return Err(exc("RuntimeError", "OrderedDict mutated during iteration".into()));
                    };
                    let k = k.clone();
                    let rev = *kind & 4 != 0;
                    *i = if rev { if *i == 0 { usize::MAX } else { odict_scan(&dd, *i - 1, true) } } else { odict_scan(&dd, *i + 1, false) };
                    k
                };
                if *kind & 3 == 0 {
                    return Ok(Some(k));
                }
                let Some(val) = Dict::get_cell(d, &k)? else {
                    *i = usize::MAX;
                    return Err(exc_v("KeyError", k));
                };
                Ok(Some(if *kind & 3 == 1 { val } else { tup(vec![k, val]) }))
            }
            It::RevDict(d, i, kind, used, ver0) => {
                let item = {
                    let d = d.borrow();
                    // CPython checks the size before anything else, so a change after the last item still raises
                    if *used < 0 || d.n as i64 != *used {
                        *used = -1;
                        return Err(exc("RuntimeError", "dictionary changed size during iteration".into()));
                    }
                    // A key deleted and another added (same size): CPython goes on reading its entry table, so
                    // what comes next depends on when its table was compacted, which Brevi's dict does not copy.
                    if d.ver != *ver0 {
                        return Err(unsupported("reversed() of a dict whose keys changed during iteration"));
                    }
                    let mut got = None;
                    while *i > 0 {
                        *i -= 1;
                        if let Some((k, v)) = &d.ents[*i] {
                            got = Some(match kind {
                                0 | 3 => k.clone(),
                                1 => v.clone(),
                                _ => tup(vec![k.clone(), v.clone()]),
                            });
                            break;
                        }
                    }
                    got
                };
                if item.is_none() {
                    *self = It::Done;
                }
                Ok(item)
            }
            It::Set(s, i, n0) => {
                let s = s.borrow();
                // CPython only counts entries (add + remove keeps it), and once it has raised, every later
                // next() on that iterator raises too (n0 is turned negative to stay failed).
                if *n0 < 0 || s.used as i64 != *n0 {
                    *n0 = -1;
                    return Err(exc("RuntimeError", "Set changed size during iteration".into()));
                }
                while *i < s.t.len() {
                    *i += 1;
                    if let Slot::Full(k, _) = &s.t[*i - 1] {
                        return Ok(Some(k.clone()));
                    }
                }
                Ok(None)
            }
            It::Gen(f) => f(),
            It::GenExp(f, _, done) => {
                if *done {
                    return Ok(None);
                }
                match f() {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        *done = true;
                        Err(pep479(e, "generator", false))
                    }
                }
            }
            It::Coro(c) => coro_next(c, V::None),
            It::Obj(o) => obj_next(o),
            It::Ref(r) => iter_step(r),
            It::Shared(f) => f(),
            It::Red(r) => r.step(),
            It::Tee(p) => tee_step(p),
            It::Done => Ok(None),
        }
    }
}
fn obj_next(o: &V) -> Result<Option<V>, E> {
    let f = match o {
        V::Obj(ob) => ob.cls.lookup("__next__"),
        _ => None,
    };
    match f {
        Some(f) => match call1(&f, o.clone()) {
            Ok(v) => Ok(Some(v)),
            Err(E(e)) if is_exc(&e, "StopIteration") => Ok(None),
            Err(e) => Err(e),
        },
        None => type_err(format!("'{}' object is not an iterator", tn(o))),
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
// The first live entry of an OrderedDict at or after `from` (at or before, reversed); usize::MAX when none is left.
pub fn odict_scan(d: &Dict, from: usize, rev: bool) -> usize {
    if rev {
        (0..=from.min(d.ents.len().wrapping_sub(1))).rev().find(|&j| d.ents[j].is_some()).unwrap_or(usize::MAX)
    } else {
        (from..d.ents.len()).find(|&j| d.ents[j].is_some()).unwrap_or(usize::MAX)
    }
}
// An OrderedDict's iterator over its keys (view kind 0), values (1) or items (2); add 4 for reversed.
pub fn odict_iter(d: &Rc<RefCell<Dict>>, kind: u8) -> It {
    let dd = d.borrow();
    let start = if kind & 4 != 0 {
        if dd.ents.is_empty() { usize::MAX } else { odict_scan(&dd, dd.ents.len() - 1, true) }
    } else {
        odict_scan(&dd, 0, false)
    };
    It::ODict(d.clone(), start, kind, dd.ver, dd.n as i64)
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
        V::Dict(d) if d.borrow().kind == 3 => odict_iter(d, 0),
        V::Dict(d) => {
            let n = d.borrow().n;
            It::Dict(d.clone(), 0, 0, 0, n)
        }
        V::View(x) => match &x.0 {
            V::Dict(d) if x.1 <= 2 && d.borrow().kind == 3 => odict_iter(d, x.1),
            V::Dict(d) => {
                let n = d.borrow().n;
                It::Dict(d.clone(), 0, x.1, 0, n)
            }
            _ => It::Done,
        },
        V::Set(s) => {
            let n = s.borrow().used as i64;
            It::Set(s.clone(), 0, n)
        }
        V::FSet(s) => It::Vec(s.keys().into_iter()),
        V::Deque(d) => deque_iter(d, false),
        V::Iter(it) => It::Ref(it.it.clone()),
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__iter__") {
                let r = call(&f, Args::p(vec![v.clone()]))?;
                if let V::Obj(ro) = &r {
                    if ro.cls.lookup("__next__").is_none() {
                        return type_err(format!("iter() returned non-iterator of type '{}'", tn(&r)));
                    }
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
// The iterator class an iter(v) would report when wrapped for exposure. Only the It shapes that several
// builtins share (Vec, Gen) need the source value; the rest map one-to-one from `v` and match what
// iter(v) actually built (a list subclass iterates through its inner list, so it is a list_iterator).
fn iter_kind(v: &V) -> ItKind {
    match v {
        V::List(_) => ItKind::ListIter,
        V::Tuple(_) => ItKind::TupleIter,
        V::Str(s) => if s.ascii() { ItKind::StrAsciiIter } else { ItKind::StrIter },
        V::Range(_) => ItKind::RangeIter,
        V::Dict(d) if d.borrow().kind == 3 => ItKind::OdictIter,
        V::Dict(_) => ItKind::DictKeys,
        V::View(x) if x.1 <= 2 && matches!(&x.0, V::Dict(d) if d.borrow().kind == 3) => ItKind::OdictIter,
        // view kinds 0, 1, 2, 3: keys, values, items, mappingproxy (which iterates keys too)
        V::View(x) => match x.1 {
            0 | 3 => ItKind::DictKeys,
            1 => ItKind::DictValues,
            _ => ItKind::DictItems,
        },
        V::Set(_) | V::FSet(_) => ItKind::SetIter,
        V::Deque(_) => ItKind::DequeIter,
        V::Class(c) if is_enum(c) => ItKind::Generator,
        V::Obj(_) => match inner_of(v) {
            // iter() of a str subclass goes through its __iter__, which CPython gives str_iterator
            // even for ASCII strings (the ascii fast path only applies to str itself).
            Some(V::Str(_)) => ItKind::StrIter,
            Some(i) => iter_kind(&i),
            // a sequence reached through __getitem__: CPython's plain `iterator`
            None => ItKind::Iterator,
        },
        _ => ItKind::Unknown,
    }
}
fn iter_class_of(v: &V) -> IterClass {
    if let V::Ext(e) = v {
        if !is_bytes_v(v) {
            // a bridged value (PyV) can ask CPython for the type of its iterator; other Ext iterators
            // stay Unknown and type() of them raises rather than printing a wrong class.
            if let Some(c) = e.iter_class(v) {
                return IterClass::Class(c);
            }
            return IterClass::Kind(ItKind::Unknown);
        }
        return IterClass::Kind(ItKind::BytesIter);
    }
    IterClass::Kind(iter_kind(v))
}
pub fn iter_v(v: &V) -> R {
    if let Some(p) = proxy_deref(v) {
        return iter_v(&p?);
    }
    if let V::Iter(_) = v {
        return Ok(v.clone());
    }
    if let V::Obj(o) = v {
        if let Some(f) = o.cls.lookup("__iter__") {
            let r = call(&f, Args::p(vec![v.clone()]))?;
            if let V::Obj(ro) = &r {
                if ro.cls.lookup("__next__").is_none() {
                    return type_err(format!("iter() returned non-iterator of type '{}'", tn(&r)));
                }
                // the object __iter__ returned, itself (`iter(it) is it` for an iterator class, as in CPython)
                return Ok(r);
            }
            if !matches!(r, V::Iter(_) | V::Ext(_)) {
                return type_err(format!("iter() returned non-iterator of type '{}'", tn(&r)));
            }
            return iter_v(&r);
        }
    }
    Ok(new_iter(iter(v)?, iter_class_of(v)))
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
pub fn gen_iter(kind: ItKind, f: impl FnMut() -> Result<Option<V>, E> + 'static) -> V {
    new_iter(It::Gen(Box::new(f)), IterClass::Kind(kind))
}
pub fn genexp_iter(name: Sym, f: impl FnMut() -> Result<Option<V>, E> + 'static) -> V {
    new_iter(It::GenExp(Box::new(f), name, false), IterClass::Kind(ItKind::Generator))
}
pub fn vec_iter(kind: ItKind, v: Vec<V>) -> V {
    new_iter(It::Vec(v.into_iter()), IterClass::Kind(kind))
}
// unpacking's own message for a value whose type cannot be iterated at all (CPython's unpack_iterable): iter()
// said "'X' object is not iterable" and the type has neither __iter__ nor __getitem__ (a TypeError that the
// type's own __iter__ raised stays as it is)
fn unpack_items(v: &V) -> Result<Vec<V>, E> {
    to_vec(v).map_err(|e| {
        let own = match v {
            V::Obj(o) => o.cls.lookup("__iter__").is_none() && o.cls.lookup("__getitem__").is_none(),
            V::Class(_) | V::Ext(_) => false,
            _ => true,
        };
        let plain = format!("'{}' object is not iterable", tn(v));
        let same = is_exc(&e.0, "TypeError")
            && matches!(&e.0, V::Obj(o) if matches!(o.get("args"), Some(V::Tuple(a)) if a.len() == 1 && matches!(&a[0], V::Str(m) if m.text() == plain)));
        if own && same {
            exc("TypeError", format!("cannot unpack non-iterable {} object", tn(v)))
        } else {
            e
        }
    })
}
// a, b = x
pub fn unpack(v: &V, n: usize) -> Result<Vec<V>, E> {
    let xs = match v {
        V::Tuple(t) if t.len() == n => return Ok(t.to_vec()),
        V::List(l) if l.borrow().len() == n => return Ok(l.borrow().clone()),
        _ => unpack_items(v)?,
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
    let xs = unpack_items(v)?;
    if xs.len() < before + after {
        return val_err(format!("not enough values to unpack (expected at least {}, got {})", before + after, xs.len()));
    }
    let mut out: Vec<V> = xs[..before].to_vec();
    out.push(list(xs[before..xs.len() - after].to_vec()));
    out.extend(xs[xs.len() - after..].iter().cloned());
    Ok(out)
}
pub fn dict_items_of(v: &V) -> Result<Vec<(V, V)>, E> {
    if let Some(p) = proxy_deref(v) {
        return dict_items_of(&p?);
    }
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

// A class body's namespace dict as the class's attribute list: every key must be a string, as
// __build_class__ requires.
pub fn dict_sym_items(v: &V) -> Result<Vec<(Sym, V)>, E> {
    let mut out = vec![];
    for (k, x) in dict_items_of(v)? {
        match k {
            V::Str(sv) => out.push((sym(&sv.text()), x)),
            _ => return type_err("__build_class__: keys must be strings".into()),
        }
    }
    Ok(out)
}

// One step of a shared iterator. An iterator whose own step calls back into it (a map function that calls next()
// on the zip reading that map) cannot be stepped twice at once here: that raises ValueError, as CPython does for a
// generator already executing, instead of a RefCell panic.
pub fn iter_step(it: &Rc<RefCell<It>>) -> Result<Option<V>, E> {
    let Ok(mut g) = it.try_borrow_mut() else {
        return Err(exc("ValueError", "iterator already executing".into()));
    };
    // these hold no state of their own here: let go of the cell before running them, so they may re-enter
    match &*g {
        It::Shared(f) => {
            let f = f.clone();
            drop(g);
            f()
        }
        It::Ref(r) => {
            let r = r.clone();
            drop(g);
            iter_step(&r)
        }
        It::Obj(o) => {
            let o = o.clone();
            drop(g);
            obj_next(&o)
        }
        It::Tee(p) => {
            let p = p.clone();
            drop(g);
            tee_step(&p)
        }
        It::Red(r) => {
            let r = r.clone();
            drop(g);
            r.step()
        }
        _ => g.next(),
    }
}
pub fn shared_iter(kind: ItKind, f: impl Fn() -> Result<Option<V>, E> + 'static) -> V {
    new_iter(It::Shared(Rc::new(f)), IterClass::Kind(kind))
}
// an input of a re-entrant iterator: its own cell, so stepping it borrows nothing shared
pub fn shared_input(v: &V) -> Result<Rc<RefCell<It>>, E> {
    Ok(match v {
        V::Iter(it) => it.it.clone(),
        _ => Rc::new(RefCell::new(iter(v)?)),
    })
}
