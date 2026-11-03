
// ---------- enum: Enum, auto, unique ----------
// A class with Enum among its bases turns its plain class attributes into members when it is made
// (make_class). Members are instances with _name_/_value_ (also name/value); a value seen before makes an
// alias. The class iterates, has len, `in`, C[name] and C(value) as CPython's EnumType gives it.
thread_local! {
    static ENUM_CLS: RefCell<Option<(Rc<Class>, Rc<Class>)>> = RefCell::new(None);
}
fn enum_classes() -> (Rc<Class>, Rc<Class>) {
    ENUM_CLS.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_none() {
            let en = Class::new("Enum", "enum", vec![bclass("object")], BT::Other);
            en.set("__repr__", builtin("__repr__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), true)));
            en.set("__str__", builtin("__str__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), false)));
            let au = Class::new("auto", "enum", vec![bclass("object")], BT::Other);
            *e = Some((en, au));
        }
        e.clone().unwrap()
    })
}
pub fn enum_base() -> Rc<Class> {
    enum_classes().0
}
pub fn is_enum(c: &Rc<Class>) -> bool {
    let en = enum_base();
    !Rc::ptr_eq(c, &en) && c.is_sub(&en)
}
fn enum_text(v: &V, rep: bool) -> R {
    let V::Obj(o) = v else { return type_err("descriptor needs an Enum member".into()) };
    let name = match o.get("_name_") {
        Some(V::Str(n)) => n.s.clone(),
        _ => String::new(),
    };
    Ok(s(&if rep {
        format!("<{}.{}: {}>", o.cls.name, name, repr(&o.get("_value_").unwrap_or(V::None))?)
    } else {
        format!("{}.{}", o.cls.name, name)
    }))
}
pub fn enum_members(c: &Rc<Class>) -> V {
    c.own("__bv_members__").unwrap_or_else(|| list(vec![]))
}
// Called by make_class for a subclass of Enum.
pub fn enum_setup(c: &Rc<Class>) -> Result<(), E> {
    let au = enum_classes().1;
    if c.mro.borrow().iter().skip(1).any(|b| is_enum(b) && matches!(b.own("__bv_members__"), Some(V::List(l)) if !l.borrow().is_empty())) {
        return type_err(format!("<enum '{}'> cannot extend an enum with members", c.name));
    }
    let attrs = c.attrs.borrow().clone();
    let init = c.lookup("__init__").filter(|f| !is_builtin_init(f));
    let mut members: Vec<V> = vec![];
    let mut values: Vec<V> = vec![];
    let map = dict_new();
    for (n, v) in attrs {
        if n.starts_with('_') || matches!(v, V::Func(_) | V::Class(_) | V::Prop(_) | V::Bound(_) | V::BMeth(_)) {
            continue;
        }
        let val = match &v {
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) => {
                let mut best: Option<V> = None;
                for x in &values {
                    match (&best, x) {
                        (_, V::Int(_) | V::Big(_) | V::Bool(_)) => {
                            if best.as_ref().map_or(Ok(true), |b| lt(b, x))? {
                                best = Some(x.clone());
                            }
                        }
                        _ => return Err(unsupported("enum.auto() after a value that is not an int")),
                    }
                }
                match best {
                    Some(b) => add(&b, &V::Int(1))?,
                    None => V::Int(1),
                }
            }
            _ => v.clone(),
        };
        values.push(val.clone());
        let mut found = None;
        for m in &members {
            if let V::Obj(o) = m {
                let mv = o.get("_value_").unwrap_or(V::None);
                if is(&mv, &val) || eq(&mv, &val)? {
                    found = Some(m.clone());
                    break;
                }
            }
        }
        let m = match found {
            Some(m) => m,
            None => {
                let o = Obj::new(c.clone());
                o.set("_name_", s(n));
                o.set("_value_", val.clone());
                o.set("name", s(n));
                o.set("value", val.clone());
                // class C(str, Enum) / (int, Enum): the member is also the str or int
                if !matches!(c.base_bt(), BT::Object | BT::Other) {
                    *o.inner.borrow_mut() = Some(val.clone());
                }
                let mv = V::Obj(o);
                if let Some(f) = &init {
                    let mut p = vec![mv.clone()];
                    match &val {
                        V::Tuple(t) => p.extend(t.iter().cloned()),
                        _ => p.push(val.clone()),
                    }
                    call(f, Args::p(p))?;
                }
                members.push(mv.clone());
                mv
            }
        };
        c.set(n, m.clone());
        setitem(&map, &s(n), m.clone())?;
    }
    c.set("__bv_members__", list(members));
    c.set("__members__", map.clone());
    c.set("_member_map_", map);
    Ok(())
}
pub fn enum_call(c: &Rc<Class>, a: Args) -> R {
    if !a.k.is_empty() || a.p.len() != 1 {
        return Err(unsupported("calling an Enum class with other than one value"));
    }
    let v = &a.p[0];
    if let V::List(l) = enum_members(c) {
        for m in l.borrow().iter() {
            if is(m, v) {
                return Ok(m.clone());
            }
        }
        for m in l.borrow().iter() {
            if let V::Obj(o) = m {
                if eq(&o.get("_value_").unwrap_or(V::None), v)? {
                    return Ok(m.clone());
                }
            }
        }
    }
    val_err(format!("{} is not a valid {}", repr(v)?, c.name))
}
pub fn enum_getitem(c: &Rc<Class>, k: &V) -> R {
    if let Some(V::Dict(d)) = c.own("__members__") {
        if let Some(m) = d.borrow().get(k)? {
            return Ok(m);
        }
    }
    Err(exc_v("KeyError", k.clone()))
}
pub fn enum_contains(c: &Rc<Class>, x: &V) -> Result<bool, E> {
    if let V::List(l) = enum_members(c) {
        for m in l.borrow().iter() {
            if is(m, x) {
                return Ok(true);
            }
            if let V::Obj(o) = m {
                if eq(&o.get("_value_").unwrap_or(V::None), x)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
pub fn enum_fn(n: &str, a: Args) -> R {
    match n {
        "auto" => Ok(V::Obj(Obj::new(enum_classes().1))),
        "unique" => {
            let c = a.p.first().cloned().unwrap_or(V::None);
            if let V::Class(k) = &c {
                if let (V::List(l), Some(V::Dict(d))) = (enum_members(k), k.own("__members__")) {
                    let ms = l.borrow().clone();
                    let mut dups = vec![];
                    for (name, m) in d.borrow().items() {
                        if let (V::Str(name), V::Obj(o)) = (&name, &m) {
                            if let Some(V::Str(real)) = o.get("_name_") {
                                if real.s != name.s && ms.iter().any(|x| is(x, &m)) {
                                    dups.push(format!("{} -> {}", name.s, real.s));
                                }
                            }
                        }
                    }
                    if !dups.is_empty() {
                        return val_err(format!("duplicate values found in <enum '{}'>: {}", k.name, dups.join(", ")));
                    }
                }
            }
            Ok(c)
        }
        _ => Err(unsupported(&format!("enum.{n}"))),
    }
}
