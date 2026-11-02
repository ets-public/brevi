
// ---------- calls ----------

thread_local! {
    pub static USER_MOD: Cell<Option<fn(&str, &str) -> Option<R>>> = Cell::new(None);
    pub static DEPTH: Cell<usize> = Cell::new(0);
    pub static REC_LIMIT: Cell<usize> = Cell::new(1000);
}
pub fn call(f: &V, a: Args) -> R {
    match f {
        V::Func(fun) => (fun.f)(a),
        V::Bound(b) => {
            let mut a = a;
            a.p.insert(0, b.0.clone());
            call(&b.1, a)
        }
        V::BMeth(b) => call_method(&b.0, b.1, a),
        V::Class(c) => instantiate(c, a),
        V::Obj(o) => match o.cls.lookup("__call__") {
            Some(m) => {
                let mut a = a;
                a.p.insert(0, f.clone());
                call(&m, a)
            }
            None => type_err(format!("'{}' object is not callable", tn(f))),
        },
        V::Ext(x) => match x.method(f, "__call__", &mut { a }) {
            Some(r) => r,
            None => type_err(format!("'{}' object is not callable", tn(f))),
        },
        _ => type_err(format!("'{}' object is not callable", tn(f))),
    }
}
pub fn call0(f: &V) -> R {
    call(f, Args::p(vec![]))
}
pub fn call1(f: &V, x: V) -> R {
    call(f, Args::p(vec![x]))
}

// Binds call arguments to a def's parameters: (names, number of positional-or-keyword params,
// defaults for the last params, *args?, keyword-only names and their defaults, **kwargs?).
pub struct Sig {
    pub name: Sym,
    pub params: &'static [Sym],
    pub npos: usize,
    pub star: bool,
    pub kwstar: bool,
}
pub fn bind(sig: &Sig, dflt: &[V], mut a: Args) -> Result<Vec<V>, E> {
    let np = sig.params.len();
    let mut out: Vec<V> = vec![V::Unbound; np + sig.star as usize + sig.kwstar as usize];
    let npos = sig.npos;
    let given = a.p.len();
    if given > npos && !sig.star {
        let req = npos - dflt.iter().take(npos).filter(|d| !matches!(d, V::Unbound)).count();
        return type_err(if req == npos {
            format!("{}() takes {} positional argument{} but {} {} given", sig.name, npos, if npos == 1 { "" } else { "s" }, given, if given == 1 { "was" } else { "were" })
        } else {
            format!("{}() takes from {} to {} positional arguments but {} were given", sig.name, req, npos, given)
        });
    }
    let mut extra = vec![];
    for (i, v) in a.p.drain(..).enumerate() {
        if i < npos {
            out[i] = v;
        } else {
            extra.push(v);
        }
    }
    let mut kw = Dict::new();
    for (k, v) in a.k.drain(..) {
        match sig.params.iter().position(|p| *p == k) {
            Some(i) => {
                if !matches!(out[i], V::Unbound) {
                    return type_err(format!("{}() got multiple values for argument '{}'", sig.name, k));
                }
                out[i] = v;
            }
            None => {
                if sig.kwstar {
                    kw.set(s(k), v)?;
                } else {
                    return type_err(format!("{}() got an unexpected keyword argument '{}'", sig.name, k));
                }
            }
        }
    }
    let mut missing = vec![];
    for i in 0..np {
        if matches!(out[i], V::Unbound) {
            match dflt.get(i) {
                Some(d) if !matches!(d, V::Unbound) => out[i] = d.clone(),
                _ => missing.push(sig.params[i]),
            }
        }
    }
    if !missing.is_empty() {
        let names: Vec<String> = missing.iter().map(|m| format!("'{m}'")).collect();
        let list = match names.len() {
            1 => names[0].clone(),
            2 => format!("{} and {}", names[0], names[1]),
            _ => format!("{}, and {}", names[..names.len() - 1].join(", "), names.last().unwrap()),
        };
        return type_err(format!("{}() missing {} required positional argument{}: {}", sig.name, missing.len(), if missing.len() == 1 { "" } else { "s" }, list));
    }
    let mut k = np;
    if sig.star {
        out[k] = tup(extra);
        k += 1;
    }
    if sig.kwstar {
        out[k] = V::Dict(Rc::new(RefCell::new(kw)));
    }
    Ok(out)
}
// f(*xs, **kw) at a call site
pub fn star_args(a: &mut Args, v: &V) -> Result<(), E> {
    a.p.extend(to_vec(v)?);
    Ok(())
}
pub fn star_kwargs(a: &mut Args, v: &V) -> Result<(), E> {
    for (k, x) in dict_items_of(v)? {
        match &k {
            V::Str(n) => a.k.push((sym(&n.s), x)),
            _ => return type_err("keywords must be strings".into()),
        }
    }
    Ok(())
}

pub fn enter() -> Result<(), E> {
    let d = DEPTH.with(|d| {
        let n = d.get() + 1;
        d.set(n);
        n
    });
    if d > REC_LIMIT.with(|l| l.get()) + 50 {
        DEPTH.with(|x| x.set(x.get() - 1));
        return Err(exc("RecursionError", "maximum recursion depth exceeded".into()));
    }
    Ok(())
}
pub fn leave() {
    DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
}
// One per running Python function: RecursionError past sys.getrecursionlimit(), as in CPython, instead of
// running out of stack.
pub struct Depth;
impl Depth {
    #[inline]
    pub fn new() -> Result<Depth, E> {
        enter()?;
        Ok(Depth)
    }
}
impl Drop for Depth {
    #[inline]
    fn drop(&mut self) {
        leave();
    }
}

// ---------- classes ----------

pub fn make_class(name: Sym, module: Sym, bases: Vec<V>, attrs: Vec<(Sym, V)>) -> R {
    let mut bs = vec![];
    for b in &bases {
        match b {
            V::Class(c) => bs.push(c.clone()),
            _ => return type_err("bases must be types".into()),
        }
    }
    if bs.is_empty() {
        bs.push(bclass("object"));
    }
    let bt = if bs.iter().any(|b| b.base_bt() == BT::Exc) { BT::Exc } else { BT::Other };
    let c = Class::new(name, module, bs, bt);
    for (n, v) in attrs {
        c.set(n, v);
    }
    // __init_subclass__ / __set_name__ are not supported (rare)
    Ok(V::Class(c))
}
pub fn instantiate(c: &Rc<Class>, mut a: Args) -> R {
    if matches!(c.name, "decimal.Decimal" | "fractions.Fraction" | "datetime.date" | "datetime.datetime" | "datetime.timedelta") {
        return ext_construct(c.name, a).unwrap();
    }
    // builtin types called as constructors
    if c.module == "builtins" || c.module == "collections" {
        if let Some(r) = builtin_type_call(c, &mut a) {
            return r;
        }
    }
    if let Some(nf) = c.lookup("__new__") {
        if let V::Func(_) = nf {
            let mut a2 = Args { p: a.p.clone(), k: a.k.clone() };
            a2.p.insert(0, V::Class(c.clone()));
            let o = call(&nf, a2)?;
            if let V::Obj(ob) = &o {
                if ob.cls.is_sub(c) {
                    if let Some(init) = c.lookup("__init__") {
                        a.p.insert(0, o.clone());
                        call(&init, a)?;
                    }
                }
            }
            return Ok(o);
        }
    }
    let o = Obj::new(c.clone());
    let me = V::Obj(o.clone());
    let bb = c.base_bt();
    if bb == BT::Exc {
        o.set("args", tup(a.p.clone()));
    } else if bb != BT::Object && bb != BT::Other {
        // subclass of a builtin container: the instance wraps a builtin value
        let mut a0 = Args { p: vec![], k: vec![] };
        if c.lookup("__init__").map_or(true, |f| is_builtin_init(&f)) {
            a0 = Args { p: a.p.clone(), k: a.k.clone() };
        }
        let base = builtin_of_bt(bb);
        let inner = match builtin_type_call(&base, &mut a0) {
            Some(r) => r?,
            None => V::None,
        };
        *o.inner.borrow_mut() = Some(inner);
    }
    if let Some(init) = c.lookup("__init__") {
        if !is_builtin_init(&init) {
            a.p.insert(0, me.clone());
            let r = call(&init, a)?;
            if !matches!(r, V::None) {
                return type_err(format!("__init__() should return None, not '{}'", tn(&r)));
            }
        }
    } else if bb == BT::Object || bb == BT::Other {
        if !a.p.is_empty() || !a.k.is_empty() {
            return type_err(format!("{}() takes no arguments", c.name));
        }
    }
    Ok(me)
}
fn is_builtin_init(f: &V) -> bool {
    matches!(f, V::Func(x) if x.kind.get() == 3 && x.name == "__init__")
}
fn builtin_of_bt(bt: BT) -> Rc<Class> {
    bclass(match bt {
        BT::List => "list",
        BT::Dict => "dict",
        BT::Set => "set",
        BT::Str => "str",
        BT::Int => "int",
        BT::Float => "float",
        BT::Tuple => "tuple",
        BT::DefaultDict => "defaultdict",
        BT::Counter => "Counter",
        BT::OrderedDict => "OrderedDict",
        BT::Deque => "deque",
        _ => "object",
    })
}

// super().name for a method defined in class `cls`, called on `me`.
pub fn super_attr(cls: &V, me: &V, name: &str) -> R {
    let c = match cls {
        V::Class(c) => c.clone(),
        _ => return type_err("super() needs a class".into()),
    };
    let target = match me {
        V::Obj(o) => o.cls.clone(),
        V::Class(k) => k.clone(),
        _ => return type_err("super(): bad self".into()),
    };
    let mro = target.mro.borrow().clone();
    let start = mro.iter().position(|x| Rc::ptr_eq(x, &c)).map(|i| i + 1).unwrap_or(mro.len());
    for k in &mro[start..] {
        if let Some(v) = k.own(name) {
            return Ok(bind_attr(v, me, true));
        }
    }
    // object / builtin bases
    if name == "__init__" {
        if let V::Obj(o) = me {
            if o.cls.base_bt() == BT::Exc {
                let me2 = me.clone();
                return Ok(builtin("__init__", move |a: Args| {
                    if let V::Obj(o) = &me2 {
                        o.set("args", tup(a.p));
                    }
                    Ok(V::None)
                }));
            }
            if o.inner.borrow().is_some() {
                let me2 = me.clone();
                return Ok(builtin("__init__", move |mut a: Args| {
                    if let V::Obj(o) = &me2 {
                        let bb = o.cls.base_bt();
                        let base = builtin_of_bt(bb);
                        if let Some(r) = builtin_type_call(&base, &mut a) {
                            *o.inner.borrow_mut() = Some(r?);
                        }
                    }
                    Ok(V::None)
                }));
            }
        }
        return Ok(builtin("__init__", |_a: Args| Ok(V::None)));
    }
    if name == "__str__" || name == "__repr__" {
        let me2 = me.clone();
        let rep = name == "__repr__";
        return Ok(builtin("__str__", move |_a: Args| {
            if let V::Obj(o) = &me2 {
                if o.cls.base_bt() == BT::Exc {
                    let args = o.get("args").unwrap_or(tup(vec![]));
                    if rep {
                        return Ok(st(format!("{}{}", o.cls.name, repr(&args)?)));
                    }
                    return Ok(st(match &args {
                        V::Tuple(t) if t.len() == 1 => to_str(&t[0])?,
                        V::Tuple(t) if t.is_empty() => String::new(),
                        _ => repr(&args)?,
                    }));
                }
                return Ok(st(format!("<{}.{} object at 0x{:x}>", o.cls.module, o.cls.name, Rc::as_ptr(o) as *const u8 as usize)));
            }
            Ok(s(""))
        }));
    }
    if name == "__eq__" {
        let me2 = me.clone();
        return Ok(builtin("__eq__", move |a: Args| Ok(if is(&me2, &a.p[0]) { V::Bool(true) } else { V::NotImpl })));
    }
    if name == "__hash__" {
        let me2 = me.clone();
        return Ok(builtin("__hash__", move |_a: Args| Ok(V::Int(match &me2 {
            V::Obj(o) => (Rc::as_ptr(o) as usize >> 4) as i64,
            _ => 0,
        }))));
    }
    if name == "__setattr__" {
        let me2 = me.clone();
        return Ok(builtin("__setattr__", move |a: Args| {
            if let (V::Obj(o), V::Str(n)) = (&me2, &a.p[0]) {
                o.set(sym(&n.s), a.p[1].clone());
            }
            Ok(V::None)
        }));
    }
    if let Some(inner) = inner_of(me) {
        return getattr(&inner, name);
    }
    Err(exc("AttributeError", format!("'super' object has no attribute '{name}'")))
}
fn bind_attr(v: V, me: &V, from_class_lookup: bool) -> V {
    match &v {
        V::Func(f) => match f.kind.get() {
            1 => v.clone(),
            2 => {
                let cls = match me {
                    V::Obj(o) => V::Class(o.cls.clone()),
                    x => x.clone(),
                };
                V::Bound(Rc::new((cls, v.clone())))
            }
            3 if !from_class_lookup => v.clone(),
            _ => V::Bound(Rc::new((me.clone(), v.clone()))),
        },
        _ => v,
    }
}

// ---------- attributes ----------

pub fn getattr(o: &V, name: &str) -> R {
    match o {
        V::Obj(ob) => {
            if let Some(v) = ob.get(name) {
                return Ok(v);
            }
            if let Some(v) = ob.cls.lookup(name) {
                return match &v {
                    V::Func(f) => Ok(match f.kind.get() {
                        1 => v.clone(),
                        2 => V::Bound(Rc::new((V::Class(ob.cls.clone()), v.clone()))),
                        _ => V::Bound(Rc::new((o.clone(), v.clone()))),
                    }),
                    V::Prop(p) => call(&p.0, Args::p(vec![o.clone()])),
                    _ => Ok(v),
                };
            }
            match name {
                "__class__" => return Ok(V::Class(ob.cls.clone())),
                "__dict__" => {
                    let mut d = Dict::new();
                    for (k, v) in ob.d.borrow().iter() {
                        d.set(s(k), v.clone())?;
                    }
                    return Ok(V::Dict(Rc::new(RefCell::new(d))));
                }
                "args" if ob.cls.base_bt() == BT::Exc => return Ok(tup(vec![])),
                _ => {}
            }
            if let Some(ga) = ob.cls.lookup("__getattr__") {
                return call(&ga, Args::p(vec![o.clone(), s(name)]));
            }
            if let Some(inner) = inner_of(o) {
                if has_method(&inner, name) {
                    return Ok(V::BMeth(Rc::new((inner, sym(name)))));
                }
            }
            Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, name)))
        }
        V::Class(c) => {
            if let Some(v) = c.lookup(name) {
                return Ok(match &v {
                    V::Func(f) if f.kind.get() == 2 => V::Bound(Rc::new((o.clone(), v.clone()))),
                    _ => v,
                });
            }
            if c.name.contains('.') {
                if let Some(v) = ext_class_attr(c.name, name) {
                    return Ok(v);
                }
            }
            match name {
                "__name__" | "__qualname__" => Ok(s(c.name)),
                "__module__" => Ok(s(c.module)),
                "__mro__" => Ok(tup(c.mro.borrow().iter().map(|k| V::Class(k.clone())).collect())),
                "__bases__" => Ok(tup(c.bases.iter().map(|k| V::Class(k.clone())).collect())),
                "__doc__" => Ok(V::None),
                _ => {
                    if c.module == "builtins" || c.module == "collections" {
                        if let Some(v) = builtin_class_attr(c, name) {
                            return Ok(v);
                        }
                    }
                    Err(exc("AttributeError", format!("type object '{}' has no attribute '{}'", c.name, name)))
                }
            }
        }
        V::Module(m) => {
            if let Some(f) = USER_MOD.with(|u| u.get()) {
                if let Some(r) = f(m, name) {
                    return r;
                }
            }
            module_attr(m, name)
        }
        V::Func(f) => match name {
            "__name__" | "__qualname__" => Ok(s(f.name)),
            "__doc__" => Ok(V::None),
            _ => {
                if let Some(v) = f.attrs.borrow().iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()) {
                    return Ok(v);
                }
                Err(exc("AttributeError", format!("'function' object has no attribute '{name}'")))
            }
        },
        V::Bound(b) => match name {
            "__self__" => Ok(b.0.clone()),
            "__func__" => Ok(b.1.clone()),
            _ => getattr(&b.1, name),
        },
        V::Ext(x) => match x.attr(o, name) {
            Some(r) => r,
            None => {
                if has_method(o, name) {
                    return Ok(V::BMeth(Rc::new((o.clone(), sym(name)))));
                }
                Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", x.tname(), name)))
            }
        },
        V::Int(_) | V::Big(_) | V::Bool(_) if matches!(name, "real" | "numerator") => Ok(o.clone()),
        V::Int(_) | V::Big(_) | V::Bool(_) if matches!(name, "denominator") => Ok(V::Int(1)),
        V::Int(_) | V::Big(_) | V::Bool(_) if name == "imag" => Ok(V::Int(0)),
        V::Float(_) if name == "real" => Ok(o.clone()),
        V::Float(_) if name == "imag" => Ok(V::Float(0.0)),
        V::Range(r) if matches!(name, "start" | "stop" | "step") => Ok(V::Int(match name {
            "start" => r.0,
            "stop" => r.1,
            _ => r.2,
        })),
        V::Slice(sl) if matches!(name, "start" | "stop" | "step") => Ok(match name {
            "start" => sl.0.clone(),
            "stop" => sl.1.clone(),
            _ => sl.2.clone(),
        }),
        V::Prop(p) if matches!(name, "setter" | "getter" | "deleter") => {
            let p = p.clone();
            let which = name.to_string();
            Ok(builtin("setter", move |a: Args| {
                let f = a1(&a, "setter")?;
                Ok(V::Prop(Rc::new(match which.as_str() {
                    "setter" => (p.0.clone(), f),
                    "getter" => (f, p.1.clone()),
                    _ => (p.0.clone(), p.1.clone()),
                })))
            }))
        }
        V::Dict(d) if name == "default_factory" => Ok(d.borrow().dflt.clone().unwrap_or(V::None)),
        V::Deque(d) if name == "maxlen" => Ok(d.borrow().max.map_or(V::None, |m| V::Int(m as i64))),
        _ => {
            if has_method(o, name) {
                return Ok(V::BMeth(Rc::new((o.clone(), sym(name)))));
            }
            Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", tn(o), name)))
        }
    }
}
pub fn setattr(o: &V, name: Sym, v: V) -> Result<(), E> {
    match o {
        V::Obj(ob) => {
            if let Some(V::Prop(p)) = ob.cls.lookup(name) {
                if matches!(p.1, V::None) {
                    return Err(exc("AttributeError", format!("property '{}' of '{}' object has no setter", name, ob.cls.name)));
                }
                call(&p.1, Args::p(vec![o.clone(), v]))?;
                return Ok(());
            }
            if let Some(sa) = ob.cls.lookup("__setattr__") {
                call(&sa, Args::p(vec![o.clone(), s(name), v]))?;
                return Ok(());
            }
            if let Some(V::Tuple(slots)) = ob.cls.lookup("__frozen__") {
                let _ = slots;
                return Err(exc("FrozenInstanceError", format!("cannot assign to field '{name}'")));
            }
            ob.set(name, v);
            Ok(())
        }
        V::Class(c) => {
            c.set(name, v);
            Ok(())
        }
        V::Module("decimal.context") => decimal_context_set(name, &v),
        V::Func(f) => {
            let mut a = f.attrs.borrow_mut();
            match a.iter_mut().find(|(k, _)| *k == name) {
                Some(e) => e.1 = v,
                None => a.push((name, v)),
            }
            Ok(())
        }
        _ => Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", tn(o), name))),
    }
}
pub fn delattr(o: &V, name: &str) -> Result<(), E> {
    match o {
        V::Obj(ob) => {
            if ob.del(name) {
                Ok(())
            } else {
                Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, name)))
            }
        }
        _ => Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", tn(o), name))),
    }
}
pub fn hasattr(o: &V, name: &str) -> Result<bool, E> {
    match getattr(o, name) {
        Ok(_) => Ok(true),
        Err(E(e)) if is_exc(&e, "AttributeError") => Ok(false),
        Err(e) => Err(e),
    }
}

// obj.name(args): methods of builtin types run directly; objects look the method up.
pub fn call_method(o: &V, name: &str, a: Args) -> R {
    match o {
        V::Obj(ob) => {
            if let Some(v) = ob.get(name) {
                return call(&v, a);
            }
            if let Some(v) = ob.cls.lookup(name) {
                return match &v {
                    V::Func(f) => match f.kind.get() {
                        1 => call(&v, a),
                        2 => {
                            let mut a = a;
                            a.p.insert(0, V::Class(ob.cls.clone()));
                            call(&v, a)
                        }
                        _ => {
                            let mut a = a;
                            a.p.insert(0, o.clone());
                            (f.f)(a)
                        }
                    },
                    V::Prop(p) => call(&call(&p.0, Args::p(vec![o.clone()]))?, a),
                    _ => call(&v, a),
                };
            }
            if let Some(inner) = inner_of(o) {
                return call_method(&inner, name, a);
            }
            if ob.cls.base_bt() == BT::Exc && name == "with_traceback" {
                return Ok(o.clone());
            }
            let f = getattr(o, name)?;
            call(&f, a)
        }
        V::Class(c) => {
            if let Some(v) = c.lookup(name) {
                return match &v {
                    V::Func(f) if f.kind.get() == 2 => {
                        let mut a = a;
                        a.p.insert(0, o.clone());
                        call(&v, a)
                    }
                    _ => call(&v, a),
                };
            }
            let f = getattr(o, name)?;
            call(&f, a)
        }
        V::Module(m) => {
            if let Some(f) = USER_MOD.with(|u| u.get()) {
                if let Some(r) = f(m, name) {
                    return call(&r?, a);
                }
            }
            module_call(m, name, a)
        }
        V::Func(_) | V::Bound(_) => {
            let f = getattr(o, name)?;
            call(&f, a)
        }
        _ => builtin_method(o, name, a),
    }
}
