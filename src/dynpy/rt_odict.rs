// ---------- collections.OrderedDict ----------
//
// An OrderedDict is a Dict of kind 3: its entry table keeps the insertion order, and move_to_end and
// popitem(last=False) reorder it. What CPython's C OrderedDict does on top of dict lives here: its update()
// (MutableMapping's, through __setitem__), its constructor and copy() (a lookup of each key, so a key whose hash
// changed raises KeyError), `|` (the OrderedDict's own type), __reduce__ and its instance __dict__. The
// iterator (odict_iterator) is It::ODict in rt_ops.rs; == also compares the key order there.

// The OrderedDict table of `v`: an OrderedDict, or an instance of a subclass of one.
pub fn odict_of(v: &V) -> Option<Rc<RefCell<Dict>>> {
    match v {
        V::Dict(d) if d.borrow().kind == 3 => Some(d.clone()),
        V::Obj(_) => match inner_of(v) {
            Some(V::Dict(d)) if d.borrow().kind == 3 => Some(d),
            _ => None,
        },
        _ => None,
    }
}
// CPython's mutablemapping_add_pairs: each item of `pairs` unpacked into a key and a value, then self[key] = value
fn odict_add_pairs(o: &V, pairs: &V) -> Result<(), E> {
    for pair in to_vec(pairs)? {
        let mut it = iter(&pair)?;
        let Some(k) = it.next()? else {
            return val_err("need more than 0 values to unpack".into());
        };
        let Some(x) = it.next()? else {
            return val_err("need more than 1 value to unpack".into());
        };
        if it.next()?.is_some() {
            return val_err("too many values to unpack (expected 2)".into());
        }
        setitem(o, &k, x)?;
    }
    Ok(())
}
// CPython's mutablemapping_update_arg: a plain dict gives its items; anything with keys() gives each key and
// arg[key]; anything with items() gives its pairs; any other iterable is a sequence of pairs
pub fn odict_update_arg(o: &V, x: &V) -> Result<(), E> {
    if let V::Dict(y) = x {
        if y.borrow().kind == 0 {
            let items = y.borrow().items();
            for (k, v) in items {
                setitem(o, &k, v)?;
            }
            return Ok(());
        }
    }
    if hasattr(x, "keys")? {
        for k in to_vec(&call_method(x, "keys", Args::p(vec![]))?)? {
            let v = getitem(x, &k)?;
            setitem(o, &k, v)?;
        }
        return Ok(());
    }
    if hasattr(x, "items")? {
        return odict_add_pairs(o, &call_method(x, "items", Args::p(vec![]))?);
    }
    odict_add_pairs(o, x)
}
// OrderedDict.update(*args, **kwds) (CPython's mutablemapping_update): at most one positional argument, then the
// keywords, each through self[key] = value
pub fn odict_update(o: &V, a: Args) -> Result<(), E> {
    if a.p.len() > 1 {
        return type_err(format!("update() takes at most 1 positional argument ({} given)", a.p.len()));
    }
    if let Some(x) = a.p.first() {
        odict_update_arg(o, x)?;
    }
    for (k, v) in a.k {
        setitem(o, &s(k), v)?;
    }
    Ok(())
}
// OrderedDict(*args, **kwds): an empty OrderedDict, then its update (CPython's odict_init)
pub fn odict_new(a: Args) -> R {
    if a.p.len() > 1 {
        return type_err(format!("expected at most 1 arguments, got {}", a.p.len()));
    }
    let mut d = Dict::new();
    d.kind = 3;
    let o = V::Dict(Rc::new(RefCell::new(d)));
    odict_update(&o, a)?;
    Ok(o)
}
// od.__init__(*args, **kwds) again: an update that keeps what is there
pub fn odict_init(o: &V, a: Args) -> R {
    if a.p.len() > 1 {
        return type_err(format!("expected at most 1 arguments, got {}", a.p.len()));
    }
    odict_update(o, a)?;
    Ok(V::None)
}
// od.copy() of an OrderedDict itself: a new one filled key by key in order, each value looked up again
pub fn odict_copy(d: &Rc<RefCell<Dict>>) -> R {
    let mut n = Dict::new();
    n.kind = 3;
    let keys = d.borrow().keys();
    for k in keys {
        match Dict::get_cell(d, &k)? {
            Some(x) => n.set(k, x)?,
            None => return Err(exc_v("KeyError", k)),
        }
    }
    Ok(V::Dict(Rc::new(RefCell::new(n))))
}
// a | b where either side is an OrderedDict (CPython's odict_or): type(the OrderedDict side)(a), updated with b;
// None when the other side is not a dict, so the caller's TypeError follows
pub fn odict_or(a: &V, b: &V) -> Option<R> {
    let is_dict = |v: &V| matches!(v, V::Dict(_)) || matches!(inner_of(v), Some(V::Dict(_))) && matches!(v, V::Obj(_));
    let od_side = if odict_of(a).is_some() { a } else if odict_of(b).is_some() { b } else { return None };
    let other = if std::ptr::eq(od_side, a) { b } else { a };
    if !is_dict(other) {
        return None;
    }
    Some((|| {
        let cls = type_of(od_side)?;
        let new = call(&V::Class(cls), Args::p(vec![a.clone()]))?;
        odict_update_arg(&new, b)?;
        Ok(new)
    })())
}
// od.__reduce__(): (type(od), (), the instance __dict__ or None when it is empty, None, iter(od.items()))
pub fn odict_reduce(o: &V) -> R {
    let state = match o {
        V::Dict(d) => match &d.borrow().attrs {
            Some(a) if a.borrow().n > 0 => V::Dict(a.clone()),
            _ => V::None,
        },
        V::Obj(_) => match getattr(o, "__dict__")? {
            V::Dict(a) if a.borrow().n == 0 => V::None,
            x => x,
        },
        _ => V::None,
    };
    let items = iter_v(&call_method(o, "items", Args::p(vec![]))?)?;
    Ok(tup(vec![V::Class(type_of(o)?), tup(vec![]), state, V::None, items]))
}
// An OrderedDict's instance __dict__, made on first use.
pub fn odict_attrs(d: &Rc<RefCell<Dict>>) -> Rc<RefCell<Dict>> {
    if let Some(a) = &d.borrow().attrs {
        return a.clone();
    }
    let a = Rc::new(RefCell::new(Dict::new()));
    d.borrow_mut().attrs = Some(a.clone());
    a
}
// The methods of dict that add or remove keys. Called through dict on an OrderedDict (dict.pop(od, k)), CPython
// changes the table under the OrderedDict's own linked list, which then no longer matches it; Brevi's
// OrderedDict is one table, so such a call is refused rather than giving an OrderedDict CPython would not have.
pub fn odict_bypass_refused(cname: &str, method: &str, me: &V) -> Result<(), E> {
    if cname == "dict"
        && matches!(method, "__setitem__" | "__delitem__" | "pop" | "popitem" | "setdefault" | "update" | "clear" | "__init__" | "__ior__")
        && odict_of(me).is_some()
    {
        return Err(unsupported(&format!("dict.{method}() called on an OrderedDict (CPython's OrderedDict would keep its old key order apart from the table)")));
    }
    Ok(())
}
