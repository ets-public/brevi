
// ---------- enum: Enum, auto, unique ----------
// A class with Enum among its bases turns its plain class attributes into members when it is made
// (make_class). Members are instances with _name_/_value_ (also name/value); a value seen before makes an
// alias. The class iterates, has len, `in`, C[name] and C(value) as CPython's EnumType gives it.
thread_local! {
    static ENUM_CLS: RefCell<Option<(Rc<Class>, Rc<Class>)>> = RefCell::new(None);
    static REPR_ENUM: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    // IntEnum, StrEnum, Flag, IntFlag
    static ENUM_MORE: RefCell<Option<[Rc<Class>; 4]>> = RefCell::new(None);
}
fn enum_classes() -> (Rc<Class>, Rc<Class>) {
    ENUM_CLS.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_none() {
            let en = Class::new("Enum", "enum", vec![bclass("object")], BT::Other);
            en.set("__repr__", builtin("__repr__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), true)));
            en.set("__str__", builtin("__str__", |a: Args| enum_text(a.p.first().unwrap_or(&V::None), false)));
            en.set("_member_type_", V::Class(bclass("object")));
            // Enum.__new__ (what super().__new__ in an enum's __new__ reaches): a by-value lookup, which fails
            // while the class is still being made
            en.set("__new__", builtin("__new__", |a: Args| {
                let Some(V::Class(c)) = a.p.first() else { return type_err("Enum.__new__(X): X is not a type object".into()) };
                let has = matches!(c.own("_member_map_"), Some(V::Dict(d)) if d.borrow().len() > 0);
                if !has && c.own("__bv_in_progress__").is_some() {
                    return type_err("do not use `super().__new__; call the appropriate __new__ directly".into());
                }
                if !has {
                    return type_err(format!("{} has no members defined", repr(&V::Class(c.clone()))?));
                }
                match a.p.get(1) {
                    Some(v) if a.p.len() == 2 => enum_lookup(c, v),
                    _ => Err(unsupported("Enum.__new__ with other than one value")),
                }
            }));
            // Enum.__copy__ and __deepcopy__: a member is its own copy
            en.set("__copy__", builtin("__copy__", |a: Args| Ok(a.p.first().cloned().unwrap_or(V::None))));
            en.set("__deepcopy__", builtin("__deepcopy__", |a: Args| Ok(a.p.first().cloned().unwrap_or(V::None))));
            let au = Class::new("auto", "enum", vec![bclass("object")], BT::Other);
            *e = Some((en.clone(), au));
            drop(e);
            // Enum.name and Enum.value are enum.property descriptors (enum.py), not instance attributes: a
            // member's name cannot be set or deleted, and a member may itself be called `name` or `value`
            for (n, f) in [("name", "_name_"), ("value", "_value_")] {
                let g = builtin(n, move |a: Args| match a.p.first() {
                    Some(V::Obj(o)) => Ok(o.get(f).unwrap_or(V::None)),
                    _ => type_err("descriptor needs an Enum member".into()),
                });
                let p = make_eprop(g, V::None, V::None, V::None);
                if let V::Obj(o) = &p {
                    o.set("name", s(n));
                    o.set("clsname", s("Enum"));
                }
                en.set(n, p);
            }
            return en_pair();
        }
        e.clone().unwrap()
    })
}
fn en_pair() -> (Rc<Class>, Rc<Class>) {
    ENUM_CLS.with(|e| e.borrow().clone().unwrap())
}
// ---------- enum.property ----------
// enum.py's property (a DynamicClassAttribute): an instance reads as property() does; through the class it is
// the member it stands in for (`member`), or AttributeError. A member whose name a base's descriptor already
// uses is put on the class as one of these (enum_add_member), as EnumType._add_member_ does.
thread_local! {
    static EPROP: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
pub fn eprop_class() -> Rc<Class> {
    if let Some(c) = EPROP.with(|e| e.borrow().clone()) {
        return c;
    }
    let c = Class::new("property", "enum", vec![bclass("object")], BT::Other);
    fn me(a: &Args) -> Result<Rc<Obj>, E> {
        match a.p.first() {
            Some(V::Obj(o)) => Ok(o.clone()),
            _ => type_err("descriptor needs an enum.property".into()),
        }
    }
    fn field(o: &Rc<Obj>, n: &str) -> V {
        o.get(n).unwrap_or(V::None)
    }
    c.set("__get__", builtin("__get__", |a: Args| {
        let p = me(&a)?;
        let inst = a.p.get(1).cloned().unwrap_or(V::None);
        let owner = a.p.get(2).cloned().unwrap_or(V::None);
        let name = field(&p, "name");
        let no_attr = || -> R { Err(exc("AttributeError", format!("{} has no attribute {}", repr(&owner)?, repr(&name)?))) };
        if matches!(inst, V::None) {
            let m = field(&p, "member");
            return if matches!(m, V::None) { no_attr() } else { Ok(m) };
        }
        let fget = field(&p, "fget");
        if !matches!(fget, V::None) {
            return call(&fget, Args::p(vec![inst]));
        }
        let nm = to_str(&name)?;
        let at = match field(&p, "_attr_type") {
            V::Str(t) => t.text().to_string(),
            _ => String::new(),
        };
        match at.as_str() {
            "attr" => return getattr(&field(&p, "_cls_type"), sym(&nm)),
            "desc" => return getattr(&getattr(&inst, "_value_")?, sym(&nm)),
            _ => {}
        }
        match getattr(&owner, "_member_map_") {
            Ok(mm) => match getitem(&mm, &name) {
                Ok(v) => Ok(v),
                Err(_) => no_attr(),
            },
            Err(_) => no_attr(),
        }
    }));
    c.set("__set__", builtin("__set__", |a: Args| {
        let p = me(&a)?;
        let f = field(&p, "fset");
        if !matches!(f, V::None) {
            return call(&f, Args::p(vec![a.p.get(1).cloned().unwrap_or(V::None), a.p.get(2).cloned().unwrap_or(V::None)]));
        }
        Err(exc("AttributeError", format!("<enum {}> cannot set attribute {}", repr(&field(&p, "clsname"))?, repr(&field(&p, "name"))?)))
    }));
    c.set("__delete__", builtin("__delete__", |a: Args| {
        let p = me(&a)?;
        let f = field(&p, "fdel");
        if !matches!(f, V::None) {
            return call(&f, Args::p(vec![a.p.get(1).cloned().unwrap_or(V::None)]));
        }
        Err(exc("AttributeError", format!("<enum {}> cannot delete attribute {}", repr(&field(&p, "clsname"))?, repr(&field(&p, "name"))?)))
    }));
    c.set("__set_name__", builtin("__set_name__", |a: Args| {
        let p = me(&a)?;
        let owner = a.p.get(1).cloned().unwrap_or(V::None);
        p.set("name", a.p.get(2).cloned().unwrap_or(V::None));
        p.set("clsname", getattr(&owner, "__name__")?);
        Ok(V::None)
    }));
    // DynamicClassAttribute's getter/setter/deleter: a copy with that function replaced
    for (n, slot) in [("getter", "fget"), ("setter", "fset"), ("deleter", "fdel")] {
        c.set(n, builtin(n, move |a: Args| {
            let p = me(&a)?;
            let f = a.p.get(1).cloned().unwrap_or(V::None);
            let mut parts = [field(&p, "fget"), field(&p, "fset"), field(&p, "fdel")];
            parts[["fget", "fset", "fdel"].iter().position(|x| *x == slot).unwrap()] = f;
            let [g, st_, d] = parts;
            let doc = if matches!(p.get("overwrite_doc"), Some(V::Bool(true))) { V::None } else { field(&p, "__doc__") };
            Ok(make_eprop(g, st_, d, doc))
        }));
    }
    USER_DATA_DESC.store(true, std::sync::atomic::Ordering::Relaxed);
    EPROP.with(|e| *e.borrow_mut() = Some(c.clone()));
    c
}
// enum.property(fget, fset, fdel, doc), as DynamicClassAttribute.__init__ sets it up
pub fn make_eprop(fget: V, fset: V, fdel: V, doc: V) -> V {
    let o = Obj::new(eprop_class());
    let overwrite = matches!(doc, V::None);
    let doc = if overwrite && !matches!(fget, V::None) { getattr(&fget, "__doc__").unwrap_or(V::None) } else { doc };
    o.set("fget", fget);
    o.set("fset", fset);
    o.set("fdel", fdel);
    o.set("__doc__", doc);
    o.set("overwrite_doc", V::Bool(overwrite));
    o.set("member", V::None);
    o.set("_attr_type", V::None);
    o.set("_cls_type", V::None);
    V::Obj(o)
}
pub fn eprop_new(mut a: Args) -> R {
    let g = a.p.first().cloned().or(a.kw("fget")).unwrap_or(V::None);
    let st_ = a.p.get(1).cloned().or(a.kw("fset")).unwrap_or(V::None);
    let d = a.p.get(2).cloned().or(a.kw("fdel")).unwrap_or(V::None);
    let doc = a.p.get(3).cloned().or(a.kw("doc")).unwrap_or(V::None);
    if a.p.len() > 4 || !a.k.is_empty() {
        return type_err("DynamicClassAttribute.__init__() takes from 1 to 5 positional arguments".into());
    }
    Ok(make_eprop(g, st_, d, doc))
}
fn is_eprop(v: &V) -> bool {
    matches!(v, V::Obj(o) if EPROP.with(|e| e.borrow().as_ref().is_some_and(|c| Rc::ptr_eq(c, &o.cls))))
}
// EnumType._add_member_: the member goes on the class as itself, or (when a base already has a descriptor of that
// name) as an enum.property standing in for it, so the class reads the member and its instances the descriptor.
fn enum_add_member(c: &Rc<Class>, name: Sym, m: &V) -> Result<(), E> {
    let mut found: Option<V> = None;
    let mut dtype: Option<&str> = None;
    let mut ctype: Option<Rc<Class>> = None;
    for b in c.mro.borrow().iter().skip(1) {
        let attr = match b.own(name) {
            Some(a) => Some(a),
            // a builtin mixin's own attribute (int.numerator, str.title): a descriptor of its type
            None if b.module == "builtins" && b.name != "object" && !matches!(b.base_bt(), BT::Object | BT::Other) => match getattr(&V::Class(b.clone()), name) {
                Ok(x) => Some(match x {
                    V::Obj(_) | V::Func(_) | V::Prop(_) | V::Bound(_) | V::BMeth(_) | V::NTF(_) => x,
                    _ => V::Prop(Rc::new((V::None, V::None))),
                }),
                Err(_) => None,
            },
            None => None,
        };
        let Some(attr) = attr else { continue };
        if is_eprop(&attr) {
            found = Some(attr);
            ctype = Some(b.clone());
            dtype = Some("enum");
            break;
        } else if is_descriptor(&attr) {
            found = Some(attr);
            dtype = dtype.or(Some("desc"));
            ctype = ctype.or(Some(b.clone()));
        } else {
            dtype = Some("attr");
            ctype = Some(b.clone());
        }
    }
    let Some(fd) = found else {
        c.set(name, m.clone());
        return Ok(());
    };
    let r = make_eprop(V::None, V::None, V::None, V::None);
    let V::Obj(ro) = &r else { unreachable!() };
    ro.set("member", m.clone());
    ro.set("name", s(name));
    ro.set("clsname", s(c.name));
    if matches!(dtype, Some("enum" | "desc")) {
        if let V::Obj(fo) = &fd {
            if is_eprop(&fd) {
                for k in ["fget", "fset", "fdel"] {
                    ro.set(k, fo.get(k).unwrap_or(V::None));
                }
            }
        }
    }
    ro.set("_attr_type", dtype.map_or(V::None, s));
    ro.set("_cls_type", ctype.map_or(V::None, V::Class));
    c.set(name, r);
    Ok(())
}
// IntEnum, StrEnum, Flag and IntFlag, as enum.py builds them on Enum.
fn enum_more() -> [Rc<Class>; 4] {
    ENUM_MORE.with(|e| {
        if let Some(k) = e.borrow().as_ref() {
            return k.clone();
        }
        let en = enum_base();
        // ReprEnum: str() and format() are the mixed-in type's
        let mix_str = || builtin("__str__", |a: Args| match a.p.first().and_then(inner_of) {
            Some(i) => Ok(st(to_str(&i)?)),
            None => Ok(s("")),
        });
        let re_ = Class::new("ReprEnum", "enum", vec![en.clone()], BT::Other);
        re_.set("__bv_reprenum__", V::Bool(true));
        re_.set("__str__", mix_str());
        REPR_ENUM.with(|r| *r.borrow_mut() = Some(re_.clone()));
        let en = re_;
        let ie = Class::new("IntEnum", "enum", vec![bclass("int"), en.clone()], BT::Other);
        ie.set("_member_type_", V::Class(bclass("int")));
        ie.set("__str__", mix_str());
        ie.set("__bv_reprenum__", V::Bool(true));
        let se = Class::new("StrEnum", "enum", vec![bclass("str"), en.clone()], BT::Other);
        se.set("_member_type_", V::Class(bclass("str")));
        se.set("__str__", mix_str());
        se.set("__bv_reprenum__", V::Bool(true));
        let fl = Class::new("Flag", "enum", vec![enum_base()], BT::Other);
        flag_methods(&fl);
        let ifl = Class::new("IntFlag", "enum", vec![bclass("int"), en.clone(), fl.clone()], BT::Other);
        ifl.set("_member_type_", V::Class(bclass("int")));
        // Flag's operators, not int's (IntFlag sets them in its own body too)
        flag_methods(&ifl);
        ifl.set("__str__", mix_str());
        ifl.set("__bv_reprenum__", V::Bool(true));
        ifl.set("__bv_keep__", V::Bool(true));
        let k = [ie, se, fl, ifl];
        *e.borrow_mut() = Some(k.clone());
        k
    })
}
pub fn enum_class(n: &str) -> Option<Rc<Class>> {
    let [ie, se, fl, ifl] = enum_more();
    Some(match n {
        "ReprEnum" => REPR_ENUM.with(|r| r.borrow().clone().unwrap()),
        "IntEnum" => ie,
        "StrEnum" => se,
        "Flag" => fl,
        "IntFlag" => ifl,
        _ => return None,
    })
}
fn is_flag(c: &Rc<Class>) -> bool {
    ENUM_MORE.with(|e| e.borrow().as_ref().is_some_and(|k| c.is_sub(&k[2])))
}
fn is_str_enum(c: &Rc<Class>) -> bool {
    ENUM_MORE.with(|e| e.borrow().as_ref().is_some_and(|k| c.is_sub(&k[1])))
}
// IntEnum, StrEnum, IntFlag: format() is the value's, not str(member)'s.
pub fn is_repr_enum(c: &Rc<Class>) -> bool {
    c.lookup("__bv_reprenum__").is_some()
}
pub fn enum_base() -> Rc<Class> {
    enum_classes().0
}
pub fn is_enum(c: &Rc<Class>) -> bool {
    let en = enum_base();
    !Rc::ptr_eq(c, &en) && c.module != "enum" && c.is_sub(&en)
}
fn enum_text(v: &V, rep: bool) -> R {
    let V::Obj(o) = v else { return type_err("descriptor needs an Enum member".into()) };
    let name = match o.get("_name_") {
        Some(V::Str(n)) => n.text().to_string(),
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
// Names in an Enum body that CPython does not make members: _sunder_ and __dunder__ names, and private
// names (_Cls__x, as the class body sees `__x`). Any other name starting with `_` is a member.
fn enum_not_member(cls: &str, n: &str) -> bool {
    let b = n.as_bytes();
    let sunder = b.len() > 2 && b[0] == b'_' && b[1] != b'_' && b[b.len() - 1] == b'_' && b[b.len() - 2] != b'_';
    let dunder = b.len() > 4 && n.starts_with("__") && n.ends_with("__") && b[2] != b'_' && b[b.len() - 3] != b'_';
    let private = n.strip_prefix('_').and_then(|r| r.strip_prefix(cls.trim_start_matches('_'))).map_or(false, |r| r.starts_with("__"));
    sunder || dunder || private || n.starts_with("__")
}
// What the class being defined will be, from its bases (before make_class builds it): kind bit 1 Enum, bit 2 a
// Flag subclass, bit 4 a StrEnum subclass; the bases' own _generate_next_value_, and the one the body defines
// (as CPython's EnumDict keeps it). The class body uses this to give each auto() its value as soon as the member
// is assigned, as EnumDict does.
pub struct EnumCtx {
    cls: String,
    kind: i64,
    gnv: Option<V>,
    count: usize,
}
pub fn enum_auto_ctx(cls: &str, bases: &[&V]) -> EnumCtx {
    let en = enum_base();
    let mut ctx = EnumCtx { cls: cls.to_string(), kind: 0, gnv: None, count: 0 };
    for b in bases {
        if let V::Class(c) = b {
            if c.is_sub(&en) {
                ctx.kind |= 1;
            }
        }
    }
    if ctx.kind == 0 {
        return ctx;
    }
    let k = enum_more();
    for b in bases {
        if let V::Class(c) = b {
            if c.is_sub(&k[2]) {
                ctx.kind |= 2;
            }
            if c.is_sub(&k[1]) {
                ctx.kind |= 4;
            }
        }
    }
    // the first enum base's _generate_next_value_ (enum.py's _get_mixins_: the last base that is an Enum)
    if let Some(V::Class(c)) = bases.iter().rev().find(|b| matches!(b, V::Class(c) if c.is_sub(&en))) {
        ctx.gnv = user_gnv(&c.mro.borrow());
    }
    ctx
}
// The value auto() gets in an Enum class body: 1, then the next value for Enum; the next power of two for
// Flag; the lower-cased member name for StrEnum. `last` holds the values assigned so far, in order.
fn auto_next(ctx: i64, name: &str, last: &[V]) -> R {
    if ctx & 2 != 0 {
        let mut best: Option<i64> = None;
        for x in last {
            match x {
                V::Int(i) => best = Some(best.map_or(*i, |b| b.max(*i))),
                V::Bool(b) => best = Some(best.map_or(*b as i64, |m| m.max(*b as i64))),
                x => return type_err(format!("invalid flag value {}", repr(x)?)),
            }
        }
        let b = best.unwrap_or(0);
        let hb = 64 - (b.max(0) as u64).leading_zeros() as i64 - 1;
        if hb >= 61 {
            return Err(unsupported("Flag values past 2**62"));
        }
        return Ok(V::Int(1i64 << (hb + 1)));
    }
    if ctx & 4 != 0 {
        return Ok(st(name.to_lowercase()));
    }
    let mut best: Option<V> = None;
    for x in last {
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
        Some(b) => add(&b, &V::Int(1)),
        None => Ok(V::Int(1)),
    }
}
// Every name store in the body of an Enum class goes through here: the members' values are kept in order,
// and an auto() gets (and memoises, as CPython does) the value for its position.
pub fn enum_auto_store(last: &mut Vec<V>, ctx: &mut EnumCtx, name: &str, v: V) -> R {
    if ctx.kind == 0 {
        return Ok(v);
    }
    if name == "_generate_next_value_" {
        if ctx.count > 0 {
            return type_err("_generate_next_value_ must be defined before members".into());
        }
        ctx.gnv = Some(v.clone());
        return Ok(v);
    }
    // EnumDict.__setitem__: a private name is a plain attribute; a _sunder_ name must be one enum.py knows
    if enum_is_private(&ctx.cls, name) {
        return Ok(v);
    }
    enum_name_check(name)?;
    if (name.starts_with('_') && enum_not_member(&ctx.cls, name)) || is_descriptor(&v) || matches!(&v, V::Unbound | V::Class(_)) {
        return Ok(v);
    }
    let au = enum_classes().1;
    match &v {
        V::Obj(o) if Rc::ptr_eq(&o.cls, &au) => {
            if let Some(x) = o.get("value") {
                // the same auto() assigned again: CPython reuses the value it was given first
                last.push(x.clone());
                ctx.count += 1;
                return Ok(x);
            }
            let x = gen_next(&ctx.gnv, ctx.kind, name, &V::Int(1), ctx.count, last)?;
            o.set("value", x.clone());
            last.push(x.clone());
            ctx.count += 1;
            Ok(x)
        }
        _ => {
            last.push(v.clone());
            ctx.count += 1;
            Ok(v)
        }
    }
}
// EnumType._find_data_type_: the type the members are instances of (object for a plain Enum).
fn find_data_type(c: &Rc<Class>) -> Result<Rc<Class>, E> {
    let en = enum_base();
    let mut types: Vec<Rc<Class>> = vec![];
    for chain in c.bases.iter() {
        let mut cand: Option<Rc<Class>> = None;
        for b in chain.mro.borrow().iter() {
            if b.module == "builtins" && b.name == "object" {
                continue;
            }
            if b.is_sub(&en) {
                if let Some(V::Class(mt)) = b.lookup("_member_type_") {
                    if !(mt.module == "builtins" && mt.name == "object") && !types.iter().any(|t| Rc::ptr_eq(t, &mt)) {
                        types.push(mt);
                    }
                }
                break;
            }
            if b.module == "builtins" || b.own("__new__").is_some() || b.own("__dataclass_fields__").is_some() || !matches!(b.bt, BT::Object | BT::Other) {
                let t = cand.clone().unwrap_or_else(|| b.clone());
                if !types.iter().any(|x| Rc::ptr_eq(x, &t)) {
                    types.push(t);
                }
                break;
            }
            cand = cand.or(Some(b.clone()));
        }
    }
    if types.len() > 1 {
        let names = types.iter().map(|t| repr(&V::Class(t.clone()))).collect::<Result<Vec<_>, _>>()?;
        return type_err(format!("too many data types for {}: {{{}}}", repr(&s(c.name))?, names.join(", ")));
    }
    Ok(types.pop().unwrap_or_else(|| bclass("object")))
}
// Called by make_class for a subclass of Enum.
pub fn enum_setup(c: &Rc<Class>) -> Result<(), E> {
    let au = enum_classes().1;
    c.set("_member_type_", V::Class(find_data_type(c)?));
    if c.mro.borrow().iter().skip(1).any(|b| is_enum(b) && matches!(b.own("__bv_members__"), Some(V::List(l)) if !l.borrow().is_empty())) {
        return type_err(format!("<enum '{}'> cannot extend an enum with members", c.name));
    }
    let attrs = c.attrs.borrow().clone();
    let init = c.lookup("__init__").filter(|f| !is_builtin_init(f));
    let mt = match c.own("_member_type_") {
        Some(V::Class(m)) => m,
        _ => bclass("object"),
    };
    let mt_object = mt.module == "builtins" && mt.name == "object";
    // EnumType._find_new_: the class's own __new__, else a base enum's saved __new_member__, else the data
    // type's own __new__ when the program defines it
    let own_new = c.own("__new__");
    let new_fn = own_new.clone().or_else(|| c.mro.borrow().iter().skip(1).find_map(|b| b.own("__new_member__"))).or_else(|| {
        if mt.module == "builtins" {
            None
        } else {
            mt.mro.borrow().iter().find_map(|b| if b.module == "builtins" { None } else { b.own("__new__") })
        }
    });
    let mut members: Vec<V> = vec![];
    let mut values: Vec<V> = vec![];
    let mut v2m: Vec<(V, V)> = vec![];
    let map = dict_new();
    // EnumType.__new__: 'mro' and '' are refused as member names, before any member is made
    let bad: Vec<String> = attrs.iter().filter(|(n, v)| matches!(*n, "mro" | "") && !is_descriptor(v) && !matches!(v, V::Class(_))).map(|(n, _)| format!("'{n}'")).collect();
    if !bad.is_empty() {
        return val_err(format!("invalid enum member name(s) {}", bad.join(",")));
    }
    // the members so far are seen while the next one is made (a __new__ reading len(cls.__members__))
    c.set("_member_map_", map.clone());
    c.set("__members__", map.clone());
    c.set("__bv_in_progress__", V::Bool(true));
    struct Building;
    impl Drop for Building {
        fn drop(&mut self) {
            BUILDING.with(|b| b.borrow_mut().pop());
        }
    }
    BUILDING.with(|b| b.borrow_mut().push(c.clone()));
    let building = Building;
    for (n, v) in attrs {
        if (n.starts_with('_') && enum_not_member(c.name, n)) || is_descriptor(&v) || matches!(v, V::Class(_)) {
            continue;
        }
        let val = match &v {
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) && is_str_enum(c) => auto_next(4, n, &values)?,
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) && is_flag(c) => auto_next(2, n, &values)?,
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) => auto_next(1, n, &values)?,
            _ => v.clone(),
        };
        values.push(val.clone());
        // _proto_member.__set_name__: the value as the arguments of __new__, _value_ and __init__
        let mut args: Vec<V> = match &val {
            V::Tuple(t) => t.iter().cloned().collect(),
            _ => vec![val.clone()],
        };
        if mt.module == "builtins" && mt.name == "tuple" {
            args = vec![tup(args)];
        }
        let mv = match &new_fn {
            Some(f) => {
                let f = match f {
                    V::Obj(o) if o.cls.name == "staticmethod" => o.get("__func__").unwrap_or_else(|| f.clone()),
                    _ => f.clone(),
                };
                let mut p = vec![V::Class(c.clone())];
                p.extend(args.iter().cloned());
                call(&f, Args::p(p))?
            }
            None => V::Obj(Obj::new(c.clone())),
        };
        let V::Obj(o) = &mv else {
            return Err(unsupported("an Enum __new__ that does not return an instance of the class"));
        };
        if !Rc::ptr_eq(&o.cls, c) {
            return Err(unsupported("an Enum __new__ that does not return an instance of the class"));
        }
        if o.get("_value_").is_none() {
            let value = if is_str_enum(c) && new_fn.is_none() {
                // StrEnum.__new__: the values must already be str
                if args.len() > 3 {
                    return type_err(format!("too many arguments for str(): {}", repr(&tup(args.clone()))?));
                }
                if args.len() == 1 && !matches!(args[0], V::Str(_)) {
                    return type_err(format!("{} is not a string", repr(&args[0])?));
                }
                if args.len() > 1 {
                    return Err(unsupported("StrEnum values given as several arguments"));
                }
                args[0].clone()
            } else if mt_object {
                val.clone()
            } else {
                match call(&V::Class(mt.clone()), Args::p(args.clone())) {
                    Ok(x) => x,
                    Err(e) => {
                        let ne = exc("TypeError", "_value_ not set in __new__, unable to create it".into());
                        if let V::Obj(x) = &ne.0 {
                            x.set("__cause__", e.0.clone());
                            x.set("__context__", e.0.clone());
                            x.set("__suppress_context__", V::Bool(true));
                        }
                        return Err(ne);
                    }
                }
            };
            o.set("_value_", value);
        }
        let val = o.get("_value_").unwrap_or(V::None);
        o.set("_name_", s(n));
        // class C(str, Enum) / (int, Enum): the member is also the str or int (unless its own __new__ made it so)
        if !matches!(c.base_bt(), BT::Object | BT::Other) && o.inner().is_none() {
            if new_fn.is_some() {
                return Err(unsupported("an Enum __new__ that does not make the member through its data type's __new__"));
            }
            o.set_inner(Some(val.clone()));
        }
        if let Some(f) = &init {
            let mut p = vec![mv.clone()];
            p.extend(args.iter().cloned());
            call(f, Args::p(p))?;
        }
        if is_flag(c) && matches!(val, V::Big(_)) {
            return Err(unsupported("Flag values past 2**62"));
        }
        let mut found = None;
        for (pv, pm) in &v2m {
            if is(pv, &val) || eq(pv, &val)? {
                found = Some(pm.clone());
                break;
            }
        }
        let m = match found {
            Some(m) => m,
            None => {
                let alias = is_flag(c) && !matches!(val, V::Int(i) if i.count_ones() == 1);
                if !alias {
                    members.push(mv.clone());
                }
                v2m.push((val.clone(), mv.clone()));
                mv
            }
        };
        if let Some(V::Dict(d)) = Some(map.clone()) {
            if let Some(prev) = Dict::get_cell(&d, &s(n))? {
                return Err(exc("NameError", format!("{} is already bound: {}", repr(&s(n))?, repr(&prev)?)));
            }
        }
        enum_add_member(c, n, &m)?;
        setitem(&map, &s(n), m.clone())?;
    }
    if is_flag(c) {
        let (mut fm, mut sm) = (0i64, 0i64);
        for (v, _) in &v2m {
            if let V::Int(i) = v {
                fm |= i;
                if i.count_ones() == 1 {
                    sm |= i;
                }
            }
        }
        let all = if fm <= 0 { 0 } else { ((1i128 << (64 - (fm as u64).leading_zeros())) - 1) as i64 };
        c.set("_flag_mask_", V::Int(fm));
        c.set("_singles_mask_", V::Int(sm));
        c.set("_all_bits_", V::Int(all));
        let vals: Vec<i64> = members.iter().filter_map(|m| if let V::Obj(o) = m { if let Some(V::Int(i)) = o.get("_value_") { Some(i) } else { None } } else { None }).collect();
        let mut sorted = vals.clone();
        sorted.sort();
        c.set("__bv_bydef__", V::Bool(sorted != vals));
        let d = dict_new();
        for (v, m) in &v2m {
            if let V::Dict(dd) = &d {
                if dd.borrow().get(v)?.is_none() {
                    dd.borrow_mut().set(v.clone(), m.clone())?;
                }
            }
        }
        c.set("_value2member_map_", d);
    }
    drop(building);
    // _order_ (or __order__): the canonical member names, in definition order, as the class lists them
    let order = c.own("_order_").or_else(|| c.own("__order__"));
    c.del_exact("_order_");
    c.del_exact("__order__");
    if let Some(ov) = order.filter(|v| !matches!(v, V::None)) {
        let mut ord: Vec<String> = match &ov {
            V::Str(x) => x.text().replace(',', " ").split_whitespace().map(|w| w.to_string()).collect(),
            other => to_vec(other)?.iter().map(to_str).collect::<Result<_, _>>()?,
        };
        let names: Vec<String> = members.iter().map(|m| match m {
            V::Obj(o) => to_str(&o.get("_name_").unwrap_or(V::None)),
            _ => Ok(String::new()),
        }).collect::<Result<_, _>>()?;
        if !ord.is_empty() {
            let mut keep = vec![];
            for o in ord {
                let m = Dict::get_cell(&match &map { V::Dict(d) => d.clone(), _ => unreachable!() }, &s(&o))?;
                let ok = match &m {
                    None => true,
                    Some(mv) => {
                        let single = !is_flag(c) || matches!(mv, V::Obj(x) if matches!(x.get("_value_"), Some(V::Int(i)) if i.count_ones() == 1));
                        single && names.contains(&o)
                    }
                };
                if ok {
                    keep.push(o);
                }
            }
            ord = keep;
            if ord != names {
                let r = |v: &[String]| -> Result<String, E> { repr(&list(v.iter().map(|x| st(x.clone())).collect())) };
                return type_err(format!("member order does not match _order_:\n  {}\n  {}", r(&names)?, r(&ord)?));
            }
        }
    }
    c.del_exact("__bv_in_progress__");
    // the class's own __new__ is kept as __new_member__ and the class gets Enum.__new__ (a by-value lookup)
    if let Some(f) = own_new {
        c.set("__new_member__", f);
        c.del_exact("__new__");
    }
    c.set("__bv_members__", list(members));
    c.set("__members__", map.clone());
    c.set("_member_map_", map);
    Ok(())
}

// ---------- Flag ----------
// Flag operators on a member whose value is not an int: CPython's errors are not modelled.
fn int_flag_value(v: &V) -> Result<(), E> {
    match v {
        V::Obj(o) if !matches!(o.get("_value_"), Some(V::Int(_))) => Err(unsupported("Flag members whose values are not ints")),
        _ => Ok(()),
    }
}
fn flag_int(v: &V) -> i64 {
    match v {
        V::Obj(o) => match o.get("_value_") {
            Some(V::Int(i)) => i,
            _ => 0,
        },
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        _ => 0,
    }
}
fn cls_int(c: &Rc<Class>, n: &str) -> i64 {
    match c.lookup(n) {
        Some(V::Int(i)) => i,
        _ => 0,
    }
}
// The members in `value`'s bits: by value, or in definition order when the members' values are not sorted.
fn flag_iter(c: &Rc<Class>, value: i64) -> Result<Vec<V>, E> {
    let v2m = c.lookup("_value2member_map_").unwrap_or(V::None);
    let mut out = vec![];
    let mut n = value & cls_int(c, "_flag_mask_");
    while n != 0 {
        let b = n & n.wrapping_neg();
        out.push(match &v2m {
            V::Dict(d) => Dict::get_cell(d, &V::Int(b))?.unwrap_or(V::None),
            _ => V::None,
        });
        n ^= b;
    }
    if matches!(c.lookup("__bv_bydef__"), Some(V::Bool(true))) {
        let order = enum_members(c);
        let pos = |m: &V| -> usize {
            match &order {
                V::List(l) => l.borrow().iter().position(|x| is(x, m)).unwrap_or(usize::MAX),
                _ => usize::MAX,
            }
        };
        out.sort_by_key(|m| pos(m));
    }
    Ok(out)
}
fn flag_bin(num: i64, max_bits: usize) -> String {
    // enum.bin: two's complement with a sign digit
    let bits = 64 - (if num >= 0 { num as u64 } else { !(num as u64) }).leading_zeros() as usize;
    let ceiling: i128 = 1i128 << bits;
    let body = if num >= 0 { format!("{:b}", num as i128 + ceiling) } else { format!("{:b}", (!(num as i128) ^ (ceiling - 1)) + ceiling) };
    let (sign, mut digits) = if num >= 0 { ("0".to_string(), body[1..].to_string()) } else { ("1".to_string(), body[1..].to_string()) };
    if digits.len() < max_bits {
        let pad: String = sign.repeat(max_bits);
        digits = format!("{pad}{digits}");
        digits = digits[digits.len() - max_bits..].to_string();
    }
    format!("0b{sign} {digits}")
}
// Flag's _missing_: the member for `value`, made (and kept) as CPython makes it.
fn flag_member(c: &Rc<Class>, value: i64) -> R {
    let v2m = match c.lookup("_value2member_map_") {
        Some(V::Dict(d)) => d,
        _ => return val_err(format!("{value} is not a valid {}", c.name)),
    };
    if let Some(m) = Dict::get_cell(&v2m, &V::Int(value))? {
        return Ok(m);
    }
    let (fm, sm, all) = (cls_int(c, "_flag_mask_"), cls_int(c, "_singles_mask_"), cls_int(c, "_all_bits_"));
    let keep = c.lookup("__bv_keep__").is_some();
    let mut value = value;
    let mut neg = None;
    if !(!all <= value && value <= all) || value & (all ^ fm) != 0 {
        if !keep {
            let bl = |x: i64| 64 - (if x >= 0 { x as u64 } else { !(x as u64) }).leading_zeros() as usize;
            let mb = bl(value).max(bl(fm));
            return val_err(format!("<flag '{}'> invalid value {}\n    given {}\n  allowed {}", c.name, value, flag_bin(value, mb), flag_bin(fm, mb)));
        }
        if value < 0 {
            let bl = 64 - (!(value as u64)).leading_zeros();
            if bl >= 62 {
                return Err(unsupported("IntFlag values past 2**62"));
            }
            value += (all + 1).max(1i64 << bl);
        }
    }
    if value < 0 {
        neg = Some(value);
        value = all + 1 + value;
    }
    let unknown0 = value & !fm;
    let aliases = value & !sm;
    let member_value = value & sm;
    if unknown0 != 0 && !keep {
        return val_err(format!("{}({value}) -->  unknown values {unknown0} [{}]", c.name, bin_text(unknown0)));
    }
    let o = Obj::new(c.clone());
    o.set("_value_", V::Int(value));
    if !matches!(c.base_bt(), BT::Object | BT::Other) {
        o.set_inner(Some(V::Int(value)));
    }
    let mut name = V::None;
    if member_value != 0 || aliases != 0 {
        let mut members = flag_iter(c, member_value)?;
        let mut combined = members.iter().fold(0, |a, m| a | flag_int(m));
        let mut value2 = value;
        if aliases != 0 {
            value2 = member_value | aliases;
            if let Some(V::Dict(map)) = c.own("_member_map_") {
                for (_, pm) in map.borrow().items() {
                    let pv = flag_int(&pm);
                    if !members.iter().any(|m| is(m, &pm)) && pv != 0 && pv & value2 == pv {
                        combined |= pv;
                        members.push(pm);
                    }
                }
            }
        }
        let unknown = value2 ^ combined;
        let mut names: Vec<String> = vec![];
        for m in &members {
            if let V::Obj(mo) = m {
                names.push(to_str(&mo.get("_name_").unwrap_or(V::None))?);
            }
        }
        if combined == 0 {
            name = V::None;
        } else if unknown != 0 && !keep {
            return val_err(format!("<flag '{}'>: no members with value {unknown}", c.name));
        } else if unknown != 0 {
            name = st(format!("{}|{unknown}", names.join("|")));
        } else {
            name = st(names.join("|"));
        }
    }
    o.set("_name_", name);
    let m = V::Obj(o);
    Dict::set_cell(&v2m, V::Int(value), m.clone())?;
    if let Some(n) = neg {
        Dict::set_cell(&v2m, V::Int(n), m.clone())?;
    }
    Ok(m)
}
fn bin_text(n: i64) -> String {
    if n < 0 {
        format!("-0b{:b}", n.unsigned_abs())
    } else {
        format!("0b{n:b}")
    }
}
fn flag_methods(fl: &Rc<Class>) {
    fn me_cls(a: &Args) -> Result<Rc<Class>, E> {
        match a.p.first() {
            Some(V::Obj(o)) => Ok(o.cls.clone()),
            _ => type_err("descriptor needs a Flag member".into()),
        }
    }
    // _get_value: a member of the same class, or (IntFlag) an int
    fn other_val(c: &Rc<Class>, x: &V) -> Option<i64> {
        match x {
            V::Obj(o) if o.cls.is_sub(c) => Some(flag_int(x)),
            V::Int(i) if !matches!(c.base_bt(), BT::Object | BT::Other) => Some(*i),
            V::Bool(b) if !matches!(c.base_bt(), BT::Object | BT::Other) => Some(*b as i64),
            _ => None,
        }
    }
    for (name, op) in [("__or__", '|'), ("__ror__", '|'), ("__and__", '&'), ("__rand__", '&'), ("__xor__", '^'), ("__rxor__", '^')] {
        fl.set(name, builtin(name, move |a: Args| {
            let c = me_cls(&a)?;
            let Some(o) = a.p.get(1).and_then(|x| other_val(&c, x)) else { return Ok(V::NotImpl) };
            int_flag_value(&a.p[0])?;
            int_flag_value(&a.p[1])?;
            let v = flag_int(&a.p[0]);
            flag_member(&c, match op {
                '|' => v | o,
                '&' => v & o,
                _ => v ^ o,
            })
        }));
    }
    fl.set("__invert__", builtin("__invert__", |a: Args| {
        let c = me_cls(&a)?;
        int_flag_value(&a.p[0])?;
        let v = flag_int(&a.p[0]);
        if c.lookup("__bv_keep__").is_some() {
            flag_member(&c, !v)
        } else {
            flag_member(&c, cls_int(&c, "_singles_mask_") & !v)
        }
    }));
    fl.set("__contains__", builtin("__contains__", |a: Args| {
        let c = me_cls(&a)?;
        let other = a.p.get(1).cloned().unwrap_or(V::None);
        match &other {
            V::Obj(o) if o.cls.is_sub(&c) => {
                int_flag_value(&a.p[0])?;
                int_flag_value(&other)?;
                let ov = flag_int(&other);
                Ok(V::Bool(ov & flag_int(&a.p[0]) == ov))
            }
            x => type_err(format!("unsupported operand type(s) for 'in': '{}' and '{}'", tn(x), c.name)),
        }
    }));
    fl.set("__iter__", builtin("__iter__", |a: Args| {
        let c = me_cls(&a)?;
        Ok(list(flag_iter(&c, flag_int(&a.p[0]))?))
    }));
    fl.set("__len__", builtin("__len__", |a: Args| Ok(V::Int(flag_int(&a.p[0]).count_ones() as i64))));
    fl.set("__bool__", builtin("__bool__", |a: Args| Ok(V::Bool(flag_int(&a.p[0]) != 0))));
    fl.set("__repr__", builtin("__repr__", |a: Args| {
        let c = me_cls(&a)?;
        let V::Obj(o) = &a.p[0] else { return Ok(s("")) };
        let v = repr(&o.get("_value_").unwrap_or(V::None))?;
        Ok(st(match o.get("_name_") {
            Some(V::Str(n)) => format!("<{}.{}: {v}>", c.name, n.text()),
            _ => format!("<{}: {v}>", c.name),
        }))
    }));
    fl.set("__str__", builtin("__str__", |a: Args| {
        let c = me_cls(&a)?;
        let V::Obj(o) = &a.p[0] else { return Ok(s("")) };
        Ok(st(match o.get("_name_") {
            Some(V::Str(n)) => format!("{}.{}", c.name, n.text()),
            _ => format!("{}({})", c.name, repr(&o.get("_value_").unwrap_or(V::None))?),
        }))
    }));
}
// EnumType.__call__: a value lookup when the class has members; otherwise the functional API
// (`Enum('Color', 'RED GREEN')`), which makes a new enum class on this one.
pub fn enum_call(c: &Rc<Class>, a: Args) -> R {
    let (mut module, mut qualname, mut ty, mut start, mut boundary) = (V::None, V::None, V::None, V::Int(1), V::None);
    for (k, v) in a.k {
        match k {
            "module" => module = v,
            "qualname" => qualname = v,
            "type" => ty = v,
            "start" => start = v,
            "boundary" => boundary = v,
            "names" if a.p.len() == 1 => return enum_call(c, Args { p: vec![a.p[0].clone(), v], k: vec![] }.with_k(module, qualname, ty, start, boundary)),
            _ => return type_err(format!("EnumType.__call__() got an unexpected keyword argument '{k}'")),
        }
    }
    let Some(value) = a.p.first().cloned() else { return type_err("EnumType.__call__() missing 1 required positional argument: 'value'".into()) };
    let has_members = matches!(c.own("_member_map_"), Some(V::Dict(d)) if d.borrow().len() > 0);
    if has_members {
        let value = if a.p.len() > 1 { tup(a.p.clone()) } else { value };
        return enum_lookup(c, &value);
    }
    if a.p.len() > 2 {
        return Err(unsupported("calling an Enum class without members with more than two values"));
    }
    if a.p.len() == 1 && matches!(ty, V::None) {
        return type_err(format!("{} has no members; specify `names=()` if you meant to create a new, empty, enum", to_str(&V::Class(c.clone()))?));
    }
    if !matches!(boundary, V::None) {
        return Err(unsupported("the boundary argument of the Enum functional API"));
    }
    enum_create(c, &value, a.p.get(1).cloned().unwrap_or(V::None), module, qualname, ty, start)
}
trait WithK {
    fn with_k(self, module: V, qualname: V, ty: V, start: V, boundary: V) -> Args;
}
impl WithK for Args {
    fn with_k(mut self, module: V, qualname: V, ty: V, start: V, boundary: V) -> Args {
        for (k, v) in [("module", module), ("qualname", qualname), ("type", ty), ("boundary", boundary)] {
            if !matches!(v, V::None) {
                self.k.push((k, v));
            }
        }
        if !matches!(start, V::Int(1)) {
            self.k.push(("start", start));
        }
        self
    }
}
// The _generate_next_value_ the class (or the class being made on `bases`) would call: a function of the program's
// own, or None for enum.py's own (computed by auto_value).
fn user_gnv(mro: &[Rc<Class>]) -> Option<V> {
    for k in mro {
        if let Some(f) = k.own("_generate_next_value_") {
            return if k.module == "enum" { None } else { Some(f) };
        }
    }
    None
}
// _generate_next_value_(name, start, count, last_values) as enum.py's Enum, Flag and StrEnum define it, or the
// program's own. `kind` is enum_auto_ctx's bits (2 Flag, 4 StrEnum).
fn gen_next(gnv: &Option<V>, kind: i64, name: &str, start: &V, count: usize, last: &[V]) -> R {
    if let Some(f) = gnv {
        let f = match f {
            V::Obj(o) if o.cls.name == "staticmethod" => o.get("__func__").unwrap_or(f.clone()),
            f => f.clone(),
        };
        return call(&f, Args::p(vec![s(name), start.clone(), V::Int(count as i64), list(last.to_vec())]));
    }
    if kind & 4 != 0 {
        return Ok(st(name.to_lowercase()));
    }
    if kind & 2 != 0 {
        if count == 0 {
            return Ok(if matches!(start, V::None) { V::Int(1) } else { start.clone() });
        }
        return auto_next(2, name, last);
    }
    if last.is_empty() {
        return Ok(start.clone());
    }
    let sorted = match bi_sorted(Args::p(vec![list(last.to_vec())])) {
        Ok(V::List(l)) => l.borrow().last().cloned().unwrap_or(V::None),
        Ok(_) => V::None,
        Err(e) if matches!(&e.0, V::Obj(o) if o.cls.name == "TypeError") => return type_err("unable to sort non-numeric values".into()),
        Err(e) => return Err(e),
    };
    match add(&sorted, &V::Int(1)) {
        Err(e) if matches!(&e.0, V::Obj(o) if o.cls.name == "TypeError") => type_err(format!("unable to increment {}", repr(&sorted)?)),
        r => r,
    }
}
// EnumType._create_: the class `Name(type, cls)` with `names` as its members.
fn enum_create(c: &Rc<Class>, name: &V, names: V, module: V, qualname: V, ty: V, start: V) -> R {
    let V::Str(cn) = name else { return type_err(format!("type.__new__() argument 1 must be str, not {}", tn(name))) };
    let cname = sym(&cn.text());
    let mut bases = vec![];
    if !matches!(ty, V::None) {
        bases.push(ty.clone());
    }
    bases.push(V::Class(c.clone()));
    let mut kind = 1;
    if is_flag(c) {
        kind |= 2;
    }
    if is_str_enum(c) {
        kind |= 4;
    }
    let gnv = user_gnv(&c.mro.borrow());
    let mut pairs: Vec<(V, V)> = vec![];
    let names = match &names {
        V::Str(t) => list(t.text().replace(',', " ").split_whitespace().map(s).collect()),
        _ => names,
    };
    let seq = match &names {
        V::List(l) => Some(l.borrow().clone()),
        V::Tuple(t) => Some(t.to_vec()),
        _ => None,
    };
    match (&names, seq) {
        (_, Some(items)) if matches!(items.first(), Some(V::Str(_))) => {
            let mut last: Vec<V> = vec![];
            for (count, n) in items.iter().enumerate() {
                let V::Str(nt) = n else { return type_err("cannot unpack non-iterable object".into()) };
                let v = gen_next(&gnv, kind, &nt.text(), &start, count, &last)?;
                last.push(v.clone());
                pairs.push((n.clone(), v));
            }
        }
        (V::None, _) => {}
        (V::Dict(_), _) => {
            for k in to_vec(&names)? {
                let v = getitem(&names, &k)?;
                pairs.push((k, v));
            }
        }
        _ => {
            for item in to_vec(&names)? {
                if let V::Str(_) = item {
                    let v = getitem(&names, &item)?;
                    pairs.push((item, v));
                    continue;
                }
                let xs = to_vec(&item)?;
                if xs.len() != 2 {
                    return val_err(format!("{} values to unpack (expected 2{})", if xs.len() > 2 { "too many" } else { "not enough" }, if xs.len() > 2 { "".to_string() } else { format!(", got {}", xs.len()) }));
                }
                pairs.push((xs[0].clone(), xs[1].clone()));
            }
        }
    }
    let mut attrs: Vec<(Sym, V)> = vec![];
    let mut last: Vec<V> = vec![];
    let au = enum_classes().1;
    for (k, v) in pairs {
        let V::Str(kt) = &k else { return Err(unsupported("an Enum member name that is not a str")) };
        let kn = sym(&kt.text());
        // the namespace's checks (EnumDict.__setitem__) for the names a class body could not have made
        enum_name_check(kn)?;
        if attrs.iter().any(|(n, _)| *n == kn) {
            return type_err(format!("'{kn}' already defined as {}", repr(&attrs.iter().find(|(n, _)| *n == kn).unwrap().1)?));
        }
        // auto() among the values gets its value as the class body would give it
        let v = match &v {
            V::Obj(o) if Rc::ptr_eq(&o.cls, &au) && !enum_not_member(cname, kn) => {
                let x = match o.get("value") {
                    Some(x) => x,
                    None => {
                        let x = gen_next(&gnv, kind, kn, &V::Int(1), last.len(), &last)?;
                        o.set("value", x.clone());
                        x
                    }
                };
                x
            }
            _ => v,
        };
        if !enum_not_member(cname, kn) && !is_descriptor(&v) {
            last.push(v.clone());
        }
        attrs.push((kn, v));
    }
    let modname: Sym = match &module {
        V::None => "__main__",
        V::Str(m) => sym(&m.text()),
        _ => return Err(unsupported("an Enum functional API module that is not a str")),
    };
    let qn: Sym = match &qualname {
        V::None => cname,
        V::Str(q) => sym(&q.text()),
        _ => return type_err(format!("type __qualname__ must be a str, not {}", tn(&qualname))),
    };
    attrs.insert(0, ("__module__", s(modname)));
    make_class_f(cname, qn, modname, bases, attrs, vec![])
}
// enum.py's _is_descriptor: an object with __get__, __set__ or __delete__ is never a member.
pub fn is_descriptor(v: &V) -> bool {
    match v {
        V::Obj(o) => ["__get__", "__set__", "__delete__"].iter().any(|n| o.cls.lookup(n).is_some()),
        V::Func(_) | V::Prop(_) | V::Bound(_) | V::BMeth(_) | V::NTF(_) => true,
        _ => false,
    }
}
// enum.py's _is_private: _Cls__name (not ending in __), as a class body's __name becomes.
fn enum_is_private(cls: &str, n: &str) -> bool {
    let pat = format!("_{cls}__");
    let b = n.as_bytes();
    n.len() > pat.len() && n.starts_with(&pat) && (b[b.len() - 1] != b'_' || b[b.len() - 2] != b'_')
}
// EnumDict.__setitem__'s refusals of a name: _sunder_ names enum.py does not know, and the reserved ones.
fn enum_name_check(n: &str) -> Result<(), E> {
    let b = n.as_bytes();
    let sunder = b.len() > 2 && b[0] == b'_' && b[1] != b'_' && b[b.len() - 1] == b'_' && b[b.len() - 2] != b'_';
    if sunder && !matches!(n, "_order_" | "_generate_next_value_" | "_numeric_repr_" | "_missing_" | "_ignore_" | "_iter_member_" | "_iter_member_by_value_" | "_iter_member_by_def_" | "_add_alias_" | "_add_value_alias_") {
        if n.starts_with("_repr_") || n == "_settle_" {
            return Err(unsupported(&format!("the Enum name {n}")));
        }
        return val_err(format!("_sunder_ names, such as '{n}', are reserved for future Enum use"));
    }
    Ok(())
}
// Enum.__new__'s lookup of a value: the member, or what the class's own _missing_ gives, as enum.py runs it.
pub fn enum_lookup(c: &Rc<Class>, v: &V) -> R {
    if let V::Obj(o) = v {
        if Rc::ptr_eq(&o.cls, c) {
            return Ok(v.clone());
        }
    }
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
    if is_flag(c) && user_missing(c).is_none() {
        match v {
            V::Int(i) => return flag_member(c, *i),
            V::Bool(b) => return flag_member(c, *b as i64),
            V::Big(_) => return Err(unsupported("Flag values past 2**62")),
            _ => {}
        }
    }
    let ve = || -> Result<E, E> { Ok(exc_v("ValueError", st(format!("{} is not a valid {}", repr(v)?, c.qualname.get())))) };
    let Some(f) = user_missing(c) else { return Err(ve()?) };
    if is_flag(c) {
        return Err(unsupported("a Flag's own _missing_"));
    }
    match call(&f, Args::p(vec![V::Class(c.clone()), v.clone()])) {
        Ok(V::None) => Err(ve()?),
        Ok(r) if matches!(&r, V::Obj(o) if o.cls.is_sub(c)) => Ok(r),
        Ok(r) => type_err(format!("error in {}._missing_: returned {} instead of None or a valid member", c.name, repr(&r)?)),
        Err(e) => Err(e),
    }
}
// The class's own _missing_ (a classmethod of the program), when it has one.
fn user_missing(c: &Rc<Class>) -> Option<V> {
    for k in c.mro.borrow().iter() {
        if k.own("_missing_").is_some() {
            if k.module == "enum" {
                return None;
            }
            return match getattr(&V::Class(c.clone()), "_missing_") {
                Ok(V::Bound(b)) => Some(b.1.clone()),
                Ok(f) => Some(f),
                Err(_) => None,
            };
        }
    }
    None
}
pub fn enum_getitem(c: &Rc<Class>, k: &V) -> R {
    if let Some(V::Dict(d)) = c.own("__members__") {
        if let Some(m) = Dict::get_cell(&d, k)? {
            return Ok(m);
        }
    }
    Err(exc_v("KeyError", k.clone()))
}
fn is_val_err(e: &E) -> bool {
    matches!(&e.0, V::Obj(o) if o.cls.name == "ValueError")
}
// EnumType.__contains__: an instance of the class is in it; a Flag also accepts any value its _missing_
// turns into a member (pseudo-members included); finally a value equal to a member's value is in it.
pub fn enum_contains(c: &Rc<Class>, x: &V) -> Result<bool, E> {
    if let V::Obj(o) = x {
        if o.cls.is_sub(c) {
            return Ok(true);
        }
    }
    if is_flag(c) {
        let keep = c.lookup("__bv_keep__").is_some();
        let v = match x {
            V::Int(i) => Some(*i),
            V::Bool(b) => Some(*b as i64),
            V::Obj(_) => match inner_of(x) {
                Some(V::Int(i)) => Some(i),
                Some(V::Bool(b)) => Some(b as i64),
                Some(V::Big(_)) if keep => return Err(unsupported("IntFlag values past 2**62")),
                _ => None,
            },
            V::Big(_) if keep => return Err(unsupported("IntFlag values past 2**62")),
            _ => None,
        };
        if let Some(v) = v {
            // _missing_ raises ValueError for values it cannot make a member of; EnumType.__contains__ then
            // falls through to the member-value check below (and returns False there).
            return match flag_member(c, v) {
                Ok(_) => Ok(true),
                Err(e) if is_val_err(&e) => Ok(false),
                Err(e) => Err(e),
            };
        }
    }
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
        "property" => eprop_new(a),
        "unique" => {
            let c = a.p.first().cloned().unwrap_or(V::None);
            if let V::Class(k) = &c {
                if let (V::List(l), Some(V::Dict(d))) = (enum_members(k), k.own("__members__")) {
                    let ms = l.borrow().clone();
                    let mut dups = vec![];
                    for (name, m) in d.borrow().items() {
                        if let (V::Str(name), V::Obj(o)) = (&name, &m) {
                            if let Some(V::Str(real)) = o.get("_name_") {
                                if real.text() != name.text() && ms.iter().any(|x| is(x, &m)) {
                                    dups.push(format!("{} -> {}", name.text(), real.text()));
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
// ---------- re.RegexFlag ----------
// re.IGNORECASE and the rest are members of an IntFlag, as in CPython, so they print as `re.IGNORECASE` and
// combine as `re.IGNORECASE|re.DOTALL`, while still working as ints.
const RE_FLAGS: [(Sym, Sym, i64); 9] = [
    ("NOFLAG", "", 0),
    ("ASCII", "A", 256),
    ("IGNORECASE", "I", 2),
    ("LOCALE", "L", 4),
    ("UNICODE", "U", 32),
    ("MULTILINE", "M", 8),
    ("DOTALL", "S", 16),
    ("VERBOSE", "X", 64),
    ("DEBUG", "", 128),
];
thread_local! {
    static RE_FLAG: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
// CPython's re.global_flag_repr over a flags value: the members in definition order, then the unknown bits in hex
pub fn re_flag_names(v: i64) -> String {
    if v == 0 {
        return "re.NOFLAG".into();
    }
    let mut parts = vec![];
    let mut rest = v;
    for (n, _, b) in RE_FLAGS {
        if b != 0 && v & b == b {
            parts.push(format!("re.{n}"));
            rest &= !b;
        }
    }
    if rest > 0 {
        parts.push(format!("{rest:#x}"));
    } else if rest < 0 {
        parts.push(format!("-{:#x}", rest.unsigned_abs()));
    }
    parts.join("|")
}
pub fn re_flag_class() -> Rc<Class> {
    if let Some(c) = RE_FLAG.with(|r| r.borrow().clone()) {
        return c;
    }
    let ifl = enum_more()[3].clone();
    let c = Class::new("RegexFlag", "re", vec![ifl], BT::Other);
    for (n, a, b) in RE_FLAGS {
        c.set(n, V::Int(b));
        if !a.is_empty() {
            c.set(a, V::Int(b));
        }
    }
    let rep = builtin("__repr__", |a: Args| {
        let v = match a.p.first() {
            Some(V::Obj(o)) => o.get("_value_").unwrap_or(V::Int(0)),
            Some(x) => x.clone(),
            None => V::Int(0),
        };
        match v {
            V::Int(i) => Ok(s(&re_flag_names(i))),
            x => Ok(s(&repr(&x)?)),
        }
    });
    c.set("__repr__", rep.clone());
    c.set("__str__", rep);
    let _ = enum_setup(&c);
    RE_FLAG.with(|r| *r.borrow_mut() = Some(c.clone()));
    c
}
// re.IGNORECASE, re.I, ..., re.RegexFlag
pub fn re_flag(n: &str) -> Option<V> {
    if n == "RegexFlag" {
        return Some(V::Class(re_flag_class()));
    }
    if !RE_FLAGS.iter().any(|(a, b, _)| *a == n || *b == n) {
        return None;
    }
    re_flag_class().lookup(n)
}
// ---------- re._constants._NamedIntConstant ----------
// re._constants.MAXREPEAT (also re._compiler's and re._parser's) is a _NamedIntConstant, as in CPython: an int
// (4294967295, _sre.MAXREPEAT) whose str and repr are its name. One instance, so `is` holds across modules.
thread_local! {
    static NAMED_INT: RefCell<Option<(Rc<Class>, V)>> = RefCell::new(None);
}
fn named_int_new(a: Args) -> R {
    let sig = Sig { name: sym("__new__"), params: &["cls", "value", "name"], npos: 3, npo: 0, star: false, kwstar: false };
    let v = bind(&sig, &[V::Unbound, V::Unbound, V::Unbound], a)?;
    // only the class itself or a subclass: CPython's other outcomes (an int with no name slot) are not modelled
    let cls = match &v[0] {
        V::Class(c) if c.is_sub(&named_int_class()) => c.clone(),
        _ => return Err(unsupported("re._constants._NamedIntConstant.__new__ on another class")),
    };
    let iv = builtin_type_call(&bclass("int"), &mut Args::p(vec![v[1].clone()])).unwrap()?;
    Ok(named_int_make(cls, iv, v[2].clone()))
}
fn named_int_make(cls: Rc<Class>, iv: V, name: V) -> V {
    let o = Obj::new(cls);
    o.set_inner(Some(iv));
    o.set("name", name);
    V::Obj(o)
}
pub fn named_int_class() -> Rc<Class> {
    if let Some((c, _)) = NAMED_INT.with(|r| r.borrow().clone()) {
        return c;
    }
    let c = Class::new("_NamedIntConstant", "re._constants", vec![bclass("int")], BT::Other);
    c.set("__new__", builtin("__new__", named_int_new));
    let name = builtin("__repr__", |a: Args| match a.p.first() {
        Some(me) => getattr(me, "name"),
        None => type_err("descriptor '__repr__' of '_NamedIntConstant' object needs an argument".into()),
    });
    c.set("__repr__", name.clone());
    c.set("__str__", name);
    let m = named_int_make(c.clone(), V::Int(0xFFFF_FFFF), s("MAXREPEAT"));
    NAMED_INT.with(|r| *r.borrow_mut() = Some((c.clone(), m)));
    c
}
// re._constants.MAXREPEAT: the one _NamedIntConstant instance
pub fn re_maxrepeat() -> V {
    named_int_class();
    NAMED_INT.with(|r| r.borrow().as_ref().unwrap().1.clone())
}

// EnumType.__setattr__: a member in the class's own _member_map_ cannot be reassigned.
pub fn enum_setattr_block(c: &Rc<Class>, name: &str) -> Option<E> {
    if !is_enum(c) {
        return None;
    }
    let Some(V::Dict(map)) = c.own("_member_map_") else { return None };
    match Dict::contains_cell(&map, &s(name)) {
        Ok(true) => Some(exc("AttributeError", format!("cannot reassign member '{name}'"))),
        Ok(false) => None,
        Err(e) => Some(e),
    }
}
// EnumType.__delattr__: a member in the class's _member_map_ (its own or a base's) cannot be deleted.
pub fn enum_delattr_block(c: &Rc<Class>, name: &str) -> Option<E> {
    if !is_enum(c) {
        return None;
    }
    let Some(V::Dict(map)) = c.lookup("_member_map_") else { return None };
    match Dict::contains_cell(&map, &s(name)) {
        Ok(true) => Some(exc("AttributeError", format!("'{}' cannot delete member '{name}'.", c.name))),
        Ok(false) => None,
        Err(e) => Some(e),
    }
}

// EnumType.__dir__ and Enum.__dir__ (enum.py), for dir() of an enum class or member; None when `x` is neither,
// or a program class on the way defines its own __dir__.
pub fn enum_dir(x: &V) -> Option<R> {
    let c = match x {
        V::Class(c) if is_enum(c) && c.meta.borrow().is_none() => c.clone(),
        V::Obj(o) if is_enum(&o.cls) => o.cls.clone(),
        _ => return None,
    };
    if c.mro.borrow().iter().any(|k| k.module != "enum" && k.module != "builtins" && k.own("__dir__").is_some()) {
        return None;
    }
    Some(enum_dir1(x, &c))
}
fn enum_dir1(x: &V, c: &Rc<Class>) -> R {
    let mt = match c.lookup("_member_type_") {
        Some(V::Class(m)) => m,
        _ => bclass("object"),
    };
    let mt_object = mt.module == "builtins" && mt.name == "object";
    let member_map = match c.lookup("_member_map_") {
        Some(V::Dict(d)) => Some(d),
        _ => None,
    };
    let in_map = |n: &str| -> Result<bool, E> {
        match &member_map {
            Some(d) => Dict::contains_cell(d, &s(n)),
            None => Ok(false),
        }
    };
    let mut names: Vec<String> = vec![];
    let add = |names: &mut Vec<String>, n: &str| {
        if !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    if let V::Class(_) = x {
        for n in ["__class__", "__contains__", "__doc__", "__getitem__", "__init_subclass__", "__iter__", "__len__", "__members__", "__module__", "__name__", "__qualname__"] {
            add(&mut names, n);
        }
        for m in to_vec(&enum_members(c))? {
            if let V::Obj(o) = &m {
                add(&mut names, &to_str(&o.get("_name_").unwrap_or(V::None))?);
            }
        }
        // _new_member_ is not object.__new__: the class's own or a base's __new__, or the data type's
        if !mt_object || c.lookup("__new_member__").is_some() {
            add(&mut names, "__new__");
        }
        if !mt_object {
            for n in to_vec(&crate_dir(&V::Class(mt.clone()))?)? {
                add(&mut names, &to_str(&n)?);
            }
        }
    } else {
        if !mt_object {
            return Err(unsupported("dir() of a member of an Enum with a data type"));
        }
        let V::Obj(o) = x else { unreachable!() };
        if o.cls.has_dict() {
            for k in to_vec(&o.dict()?)? {
                let k = to_str(&k)?;
                if !k.starts_with('_') && !in_map(&k)? {
                    add(&mut names, &k);
                }
            }
        }
        let mro = c.mro.borrow().clone();
        for k in mro.iter() {
            // the enum module's classes: only Enum has public names (its name and value properties)
            let own: Vec<(String, V)> = if k.module == "enum" {
                if Rc::ptr_eq(k, &enum_base()) {
                    vec![("name".into(), V::None), ("value".into(), V::None)]
                } else {
                    vec![]
                }
            } else if k.module == "builtins" {
                if k.name != "object" {
                    return Err(unsupported("dir() of a member of an Enum with a builtin base"));
                }
                vec![]
            } else {
                k.attrs.borrow().iter().map(|(n, v)| (n.to_string(), v.clone())).collect()
            };
            for (n, v) in own {
                if n.starts_with('_') {
                    continue;
                }
                if k.module == "enum" {
                    add(&mut names, &n);
                } else if is_eprop(&v) {
                    let has_fget = matches!(&v, V::Obj(p) if !matches!(p.get("fget"), None | Some(V::None)));
                    if has_fget || !in_map(&n)? {
                        add(&mut names, &n);
                    } else {
                        names.retain(|x| *x != n);
                    }
                } else if !in_map(&n)? {
                    add(&mut names, &n);
                }
            }
        }
        for n in ["__class__", "__doc__", "__eq__", "__hash__", "__module__"] {
            add(&mut names, n);
        }
    }
    names.sort();
    Ok(list(names.into_iter().map(st).collect()))
}
fn crate_dir(x: &V) -> R {
    bi_dir(Args::p(vec![x.clone()]))
}
