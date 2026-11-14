// ===== match statements (PEP 634): the checks behind the parser's rewrite (parse.rs match_stmt) =====
// Each follows CPython 3.13's ceval (MATCH_SEQUENCE, MATCH_MAPPING, MATCH_KEYS, MATCH_CLASS) and the compiler's
// order of length checks, subscripts and unpacking, so a pattern runs the same __len__, __getitem__, get() and
// attribute lookups CPython runs, in the same order.

// Py_TPFLAGS_SEQUENCE (32) or Py_TPFLAGS_MAPPING (64) as one class holds it (0: none). CPython copies the bits
// from the first class in the MRO that has them, and ABCMeta.register sets them on the registered class.
fn pm_own_flags(k: &Rc<Class>) -> i64 {
    match k.bt {
        BT::List | BT::Tuple | BT::Range | BT::Deque => return 32,
        BT::Dict | BT::DefaultDict | BT::Counter | BT::OrderedDict => return 64,
        _ => {}
    }
    if k.made.get() == 0 && k.tinfo.borrow().is_none() {
        match k.name {
            "memoryview" | "array" | "array.array" | "deque" | "range" => return 32,
            "mappingproxy" => return 64,
            _ => {}
        }
    }
    match k.own("__bv_flags__") {
        Some(V::Int(f)) => f & 96,
        _ => 0,
    }
}
pub fn pm_class_flags(k: &Rc<Class>) -> i64 {
    let f = pm_own_flags(k);
    if f != 0 {
        return f;
    }
    for b in k.mro.borrow().iter().skip(1) {
        let f = pm_own_flags(b);
        if f != 0 {
            return f;
        }
    }
    0
}
// 32 for a sequence, 64 for a mapping, 0 for neither (str, bytes and bytearray are neither)
fn pm_kind(x: &V) -> Result<i64, E> {
    match x {
        V::List(_) | V::Tuple(_) | V::Range(_) | V::Deque(_) => return Ok(32),
        V::Dict(_) => return Ok(64),
        V::Str(_) | V::Int(_) | V::Big(_) | V::Float(_) | V::Bool(_) | V::None | V::Set(_) | V::FSet(_) => return Ok(0),
        V::Ext(e) if e.is_py() => {
            // a CPython library value: its type's own flags
            let t = getattr(x, "__class__")?;
            return match getattr(&t, "__flags__")? {
                V::Int(f) => Ok(f & 96),
                _ => Err(exc("NotImplementedError", "brevi: a match pattern on this library value is not supported".into())),
            };
        }
        _ => {}
    }
    Ok(pm_class_flags(&type_of(x)?))
}

fn pm_usize(v: &V) -> usize {
    match v {
        V::Int(i) => *i as usize,
        _ => 0,
    }
}

// A sequence pattern: (subject, items before the star, items after it, whether there is a star, how the items
// are read: 0 unpacked by iterating, 1 by subscripts (the star is `*_`), 2 not at all). None when it does not
// match; otherwise the items (with a star unpacked, the starred part is one list in its place).
pub fn bi___brevi_pm_seq(a: Args) -> R {
    a.want("__brevi_pm_seq", 5, 5)?;
    let x = &a.p[0];
    let (nb, na, star, how) = (pm_usize(&a.p[1]), pm_usize(&a.p[2]), matches!(a.p[3], V::Bool(true)), pm_usize(&a.p[4]));
    if pm_kind(x)? != 32 {
        return Ok(V::None);
    }
    // CPython checks the length unless the pattern is a lone star (`[*_]`, `[*rest]`)
    if !star || nb + na > 0 {
        let n = len(x)?;
        if (star && n < nb + na) || (!star && n != nb) {
            return Ok(V::None);
        }
    }
    match how {
        2 => Ok(tup(vec![])),
        1 => {
            let mut out = Vec::with_capacity(nb + na);
            for i in 0..nb {
                out.push(getitem(x, &V::Int(i as i64))?);
            }
            for j in 0..na {
                let n = len(x)?;
                out.push(getitem(x, &V::Int(n as i64 - (na - j) as i64))?);
            }
            Ok(tup(out))
        }
        _ if star => Ok(tup(unpack_star(x, nb, na)?)),
        _ => Ok(tup(unpack(x, nb)?)),
    }
}

// A mapping pattern's first checks: the subject is a mapping, with at least as many items as the pattern has keys.
pub fn bi___brevi_pm_ismap(a: Args) -> R {
    a.want("__brevi_pm_ismap", 2, 2)?;
    let x = &a.p[0];
    if pm_kind(x)? != 64 {
        return Ok(V::Bool(false));
    }
    let k = pm_usize(&a.p[1]);
    Ok(V::Bool(k == 0 || len(x)? >= k))
}

// A mapping pattern's keys (MATCH_KEYS): the values the subject's get() gives for them, then with `**rest` a dict
// of the other items; None when a key is missing. A key given twice is a ValueError.
pub fn bi___brevi_pm_map(a: Args) -> R {
    a.want("__brevi_pm_map", 3, 3)?;
    let x = &a.p[0];
    let keys = to_vec(&a.p[1])?;
    let mut out = Vec::with_capacity(keys.len() + 1);
    if !keys.is_empty() {
        let get = getattr(x, "get")?;
        let seen = set_new();
        let dummy = list(vec![]);
        for k in &keys {
            if contains(&seen, k)? {
                return val_err(format!("mapping pattern checks duplicate key ({})", repr(k)?));
            }
            call_method1(&seen, "add", k.clone())?;
            let v = call2(&get, k.clone(), dummy.clone())?;
            if let (V::List(p), V::List(q)) = (&v, &dummy) {
                if Rc::ptr_eq(p, q) {
                    return Ok(V::None);
                }
            }
            out.push(v);
        }
    }
    if matches!(a.p[2], V::Bool(true)) {
        let d = dict_new();
        call_method1(&d, "update", x.clone())?;
        for k in &keys {
            delitem(&d, k)?;
        }
        out.push(d);
    }
    Ok(tup(out))
}

// classes whose one positional sub-pattern matches the subject itself (CPython's _Py_TPFLAGS_MATCH_SELF,
// inherited by subclasses)
fn pm_match_self(k: &Rc<Class>) -> bool {
    let own = |c: &Rc<Class>| {
        matches!(c.bt, BT::Int | BT::Float | BT::Str | BT::Bool | BT::List | BT::Tuple | BT::Dict | BT::DefaultDict | BT::Counter | BT::OrderedDict | BT::Set | BT::FSet | BT::Bytes | BT::ByteArray)
            || (c.made.get() == 0 && matches!(c.name, "bytes" | "bytearray") && c.module == "builtins")
    };
    own(k) || k.mro.borrow().iter().any(own)
}

// A class pattern (MATCH_CLASS): (subject, class, number of positional sub-patterns, keyword names). The
// attribute values the sub-patterns match, positional ones first (through __match_args__); None when the
// subject is not an instance or lacks one of the attributes.
pub fn bi___brevi_pm_cls(a: Args) -> R {
    a.want("__brevi_pm_cls", 4, 4)?;
    let (x, c) = (&a.p[0], &a.p[1]);
    let npos = pm_usize(&a.p[2]);
    let kws = to_vec(&a.p[3])?;
    let k = match c {
        V::Class(k) => k.clone(),
        _ => {
            if isinstance(c, &V::Class(tclass(13, "type")))? {
                return Err(exc("NotImplementedError", "brevi: a class pattern on a library class is not supported".into()));
            }
            return type_err("called match pattern must be a class".into());
        }
    };
    if !isinstance(x, c)? {
        return Ok(V::None);
    }
    let tname = k.name.rsplit('.').next().unwrap_or(k.name).to_string();
    let mut seen: Vec<String> = vec![];
    let mut out = Vec::with_capacity(npos + kws.len());
    // getattr(subject, name), None when it has no such attribute; a name already matched is a TypeError
    let mut attr = |name: &str, seen: &mut Vec<String>| -> Result<Option<V>, E> {
        if seen.iter().any(|n| n == name) {
            return type_err(format!("{tname}() got multiple sub-patterns for attribute {}", repr(&s(name))?));
        }
        seen.push(name.to_string());
        match getattr(x, name) {
            Ok(v) => Ok(Some(v)),
            Err(E(e)) if is_exc(&e, "AttributeError") => Ok(None),
            Err(e) => Err(e),
        }
    };
    if npos > 0 {
        let margs = match getattr(c, "__match_args__") {
            Ok(v) => Some(v),
            Err(E(e)) if is_exc(&e, "AttributeError") => None,
            Err(e) => return Err(e),
        };
        let (names, match_self) = match &margs {
            Some(V::Tuple(t)) => (t.to_vec(), false),
            Some(v) => return type_err(format!("{tname}.__match_args__ must be a tuple (got {})", tn(v))),
            None => (vec![], pm_match_self(&k)),
        };
        let allowed = if match_self { 1 } else { names.len() };
        if allowed < npos {
            return type_err(format!("{tname}() accepts {allowed} positional sub-pattern{} ({npos} given)", if allowed == 1 { "" } else { "s" }));
        }
        if match_self {
            out.push(x.clone());
        } else {
            for n in names.iter().take(npos) {
                let V::Str(_) = n else {
                    return type_err(format!("__match_args__ elements must be strings (got {})", tn(n)));
                };
                match attr(&to_str(n)?, &mut seen)? {
                    Some(v) => out.push(v),
                    None => return Ok(V::None),
                }
            }
        }
    }
    for n in &kws {
        match attr(&to_str(n)?, &mut seen)? {
            Some(v) => out.push(v),
            None => return Ok(V::None),
        }
    }
    Ok(tup(out))
}
