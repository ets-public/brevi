
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
            prepend(&mut a.p, b.0.clone());
            call(&b.1, a)
        }
        V::BMeth(b) => call_method(&b.0, b.1, a),
        V::Class(c) => instantiate(c, a),
        V::Obj(o) => match o.cls.lookup("__call__") {
            Some(m) => {
                let mut a = a;
                prepend(&mut a.p, f.clone());
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
#[inline]
pub fn call1(f: &V, x: V) -> R {
    if let V::Func(g) = f {
        if let FastFn::F1(h) = g.fast.get() {
            return h(x);
        }
    }
    call(f, Args::p(vec![x]))
}
#[inline]
pub fn call2(f: &V, x: V, y: V) -> R {
    if let V::Func(g) = f {
        if let FastFn::F2(h) = g.fast.get() {
            return h(x, y);
        }
    }
    call(f, Args::p(vec![x, y]))
}
#[inline]
pub fn call3(f: &V, x: V, y: V, z: V) -> R {
    if let V::Func(g) = f {
        if let FastFn::F3(h) = g.fast.get() {
            return h(x, y, z);
        }
    }
    call(f, Args::p(vec![x, y, z]))
}
// f(me, *a.p): a method called on an instance
#[inline]
pub fn call_self(f: &V, me: V, mut a: Args) -> R {
    if let V::Func(g) = f {
        if a.k.is_empty() {
            match (g.fast.get(), a.p.len()) {
                (FastFn::F1(h), 0) => return h(me),
                (FastFn::F2(h), 1) => return h(me, a.p.pop().unwrap()),
                (FastFn::F3(h), 2) => {
                    let y = a.p.pop().unwrap();
                    let x = a.p.pop().unwrap();
                    return h(me, x, y);
                }
                (FastFn::F4(h), 3) => {
                    let z = a.p.pop().unwrap();
                    let y = a.p.pop().unwrap();
                    let x = a.p.pop().unwrap();
                    return h(me, x, y, z);
                }
                _ => {}
            }
        }
    }
    let mut p = Vec::with_capacity(a.p.len() + 1);
    p.push(me);
    p.append(&mut a.p);
    a.p = p;
    call(f, a)
}

// Binds call arguments to a def's parameters: (names, number of positional-or-keyword params,
// number of positional-only params at the front, defaults for the last params, *args?,
// keyword-only names and their defaults, **kwargs?).
pub struct Sig {
    pub name: Sym,
    pub params: &'static [Sym],
    pub npos: usize,
    pub npo: usize,
    pub star: bool,
    pub kwstar: bool,
}
// self (or the class) in front of a call's arguments, reallocating once when the Vec is full
#[inline]
pub fn prepend(p: &mut Vec<V>, v: V) {
    if p.len() == p.capacity() {
        let mut n = Vec::with_capacity(p.len() + 2);
        n.push(v);
        n.append(p);
        *p = n;
    } else {
        p.insert(0, v);
    }
}
pub fn bind(sig: &Sig, dflt: &[V], mut a: Args) -> Result<Vec<V>, E> {
    let np = sig.params.len();
    // the common call: positional arguments only, the rest from defaults
    if a.k.is_empty() && !sig.star && !sig.kwstar && np == sig.npos && a.p.len() <= np && (a.p.len()..np).all(|i| dflt.get(i).is_some_and(|d| !matches!(d, V::Unbound))) {
        let mut out = std::mem::take(&mut a.p);
        for i in out.len()..np {
            out.push(dflt[i].clone());
        }
        return Ok(out);
    }
    let mut out: Vec<V> = vec![V::Unbound; np + sig.star as usize + sig.kwstar as usize];
    let npos = sig.npos;
    let npo = sig.npo;
    // A keyword naming a positional-only parameter is an error before anything else, as in CPython;
    // with **kwargs such keywords go into that dict instead, so this check is skipped.
    if !sig.kwstar && npo > 0 {
        let mut po: Vec<Sym> = vec![];
        for (k, _) in a.k.iter() {
            if sig.params[..npo].contains(k) && !po.contains(k) {
                po.push(*k);
            }
        }
        if !po.is_empty() {
            let list = po.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ");
            return type_err(format!("{}() got some positional-only arguments passed as keyword arguments: '{}'", sig.name, list));
        }
    }
    let given = a.p.len();
    let mut extra = vec![];
    for (i, v) in a.p.drain(..).enumerate() {
        if i < npos {
            out[i] = v;
        } else {
            extra.push(v);
        }
    }
    let mut kw = Dict::new();
    // keyword-only parameters given a value (CPython names their count in the too-many-positional error)
    let mut kwonly_given = 0usize;
    for (k, v) in a.k.drain(..) {
        match sig.params.iter().position(|p| *p == k) {
            Some(i) if i < npo => {
                // a positional-only name passed by keyword goes into **kwargs (as in CPython)
                kw.set(s(k), v)?;
            }
            Some(i) => {
                if !matches!(out[i], V::Unbound) {
                    return type_err(format!("{}() got multiple values for argument '{}'", sig.name, k));
                }
                if i >= npos {
                    kwonly_given += 1;
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
    if given > npos && !sig.star {
        let req = npos - dflt.iter().take(npos).filter(|d| !matches!(d, V::Unbound)).count();
        let takes = if req == npos {
            format!("takes {} positional argument{}", npos, if npos == 1 { "" } else { "s" })
        } else {
            format!("takes from {} to {} positional arguments", req, npos)
        };
        if kwonly_given > 0 {
            let k = kwonly_given;
            return type_err(format!("{}() {} but {} positional argument{} (and {} keyword-only argument{}) were given", sig.name, takes, given, if given == 1 { "" } else { "s" }, k, if k == 1 { "" } else { "s" }));
        }
        return type_err(if req == npos {
            format!("{}() {} but {} {} given", sig.name, takes, given, if given == 1 { "was" } else { "were" })
        } else {
            format!("{}() {} but {} were given", sig.name, takes, given)
        });
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
    // positional ones first, as CPython reports them; then keyword-only ones
    let kwonly = missing.iter().any(|m| sig.params.iter().position(|p| p == m).is_some_and(|i| i >= npos));
    if kwonly && missing.iter().all(|m| sig.params.iter().position(|p| p == m).is_some_and(|i| i >= npos)) {
        let names: Vec<String> = missing.iter().map(|m| format!("'{m}'")).collect();
        let list = match names.len() {
            1 => names[0].clone(),
            2 => format!("{} and {}", names[0], names[1]),
            _ => format!("{}, and {}", names[..names.len() - 1].join(", "), names.last().unwrap()),
        };
        return type_err(format!("{}() missing {} required keyword-only argument{}: {}", sig.name, missing.len(), if missing.len() == 1 { "" } else { "s" }, list));
    }
    missing.retain(|m| sig.params.iter().position(|p| p == m).is_some_and(|i| i < npos));
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
    make_class_f(name, module, bases, attrs, vec![])
}
// As make_class, with the names the class body annotates (dataclass and NamedTuple fields).
pub fn make_class_f(name: Sym, module: Sym, mut bases: Vec<V>, attrs: Vec<(Sym, V)>, fields: Vec<Sym>) -> R {
    if let Some(r) = typing_class(name, &mut bases, &attrs, &fields) {
        return r;
    }
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
    *c.fields.borrow_mut() = fields;
    // __set_name__ is not supported (rare)
    if is_enum(&c) {
        enum_setup(&c)?;
    }
    // the first __init_subclass__ of a base (an implicit classmethod), called with the new class
    let hook = c.mro.borrow().iter().skip(1).find_map(|b| b.own("__init_subclass__"));
    if let Some(f) = hook {
        let f = match &f {
            V::Bound(b) => b.1.clone(),
            f => f.clone(),
        };
        call(&f, Args::p(vec![V::Class(c.clone())]))?;
    }
    Ok(V::Class(c))
}
// A def's number of positional parameters, for Brevi's pydantic (which calls a validator with info only when
// it takes it, as pydantic reads the signature).
pub fn with_np(f: V, n: i64) -> V {
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__bv_np__", V::Int(n)));
    }
    f
}
// A def's docstring, as its __doc__.
pub fn with_doc(f: V, doc: &str) -> V {
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__doc__", s(doc)));
    }
    f
}
// a class of the program's own, of object or another such class: calling it makes an instance and runs __init__
fn plain_new(c: &Rc<Class>) -> bool {
    let ep = CLASS_EPOCH.with(|e| e.get());
    let (e, b) = c.plain_new.get();
    if e == ep {
        return b;
    }
    let b = !matches!(c.module, "typing" | "enum" | "builtins" | "collections")
        && !c.name.starts_with("numpy.")
        && !matches!(c.name, "decimal.Decimal" | "fractions.Fraction" | "datetime.date" | "datetime.datetime" | "datetime.timedelta" | "datetime.time" | "datetime.timezone")
        && c.lookup("__bv_typeddict__").is_none()
        && !is_enum(c)
        && !matches!(c.lookup("__new__"), Some(V::Func(_)))
        && matches!(c.base_bt(), BT::Object | BT::Other);
    c.plain_new.set((ep, b));
    b
}
pub fn instantiate(c: &Rc<Class>, mut a: Args) -> R {
    if plain_new(c) {
        let me = V::Obj(Obj::new(c.clone()));
        match c.lookup("__init__") {
            Some(init) if !is_builtin_init(&init) => {
                let r = call_self(&init, me.clone(), a)?;
                if !matches!(r, V::None) {
                    return type_err(format!("__init__() should return None, not '{}'", tn(&r)));
                }
            }
            Some(_) => {}
            None => {
                if !a.p.is_empty() || !a.k.is_empty() {
                    return type_err(format!("{}() takes no arguments", c.name));
                }
            }
        }
        return Ok(me);
    }
    if c.module == "typing" {
        return typing_call(c.name, a);
    }
    if let Some(r) = typed_dict_call(c, Args { p: a.p.clone(), k: a.k.clone() }) {
        return r;
    }
    if c.module == "enum" || is_enum(c) {
        if is_enum(c) {
            return enum_call(c, a);
        }
        return Err(unsupported(&format!("enum.{}()", c.name)));
    }
    if c.name.starts_with("numpy.") {
        if let Some(r) = np_construct(c.name, a) {
            return r;
        }
        return type_err(format!("cannot create '{}' instances", c.name));
    }
    if matches!(c.name, "decimal.Decimal" | "fractions.Fraction" | "datetime.date" | "datetime.datetime" | "datetime.timedelta" | "datetime.time" | "datetime.timezone" | "zoneinfo.ZoneInfo") {
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
                        if !is_builtin_init(&init) {
                            prepend(&mut a.p, o.clone());
                            call(&init, a)?;
                        }
                    }
                }
            }
            return Ok(o);
        }
    }
    if c.base_bt() == BT::Exc && c.is_sub(&exc_class("BaseExceptionGroup")) {
        let g = eg_new(c, &a)?;
        if let Some(init) = c.lookup("__init__") {
            if !is_builtin_init(&init) {
                prepend(&mut a.p, g.clone());
                call(&init, a)?;
            }
        }
        return Ok(g);
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
        o.set_inner(Some(inner));
    }
    if let Some(init) = c.lookup("__init__") {
        if !is_builtin_init(&init) {
            prepend(&mut a.p, me.clone());
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
    if name == "__new__" {
        return Ok(builtin("__new__", |mut a: Args| {
            let Some(V::Class(k)) = a.p.first().cloned() else {
                return type_err("__new__(X): X is not a type object".into());
            };
            a.p.remove(0);
            let o = Obj::new(k.clone());
            let bb = k.base_bt();
            if bb == BT::Exc {
                o.set("args", tup(a.p));
            } else if bb != BT::Object && bb != BT::Other {
                // subclass of a builtin container: the instance wraps the builtin value
                let base = builtin_of_bt(bb);
                if let Some(r) = builtin_type_call(&base, &mut a) {
                    o.set_inner(Some(r?));
                }
            }
            Ok(V::Obj(o))
        }));
    }
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
            if o.has_inner() {
                let me2 = me.clone();
                return Ok(builtin("__init__", move |mut a: Args| {
                    if let V::Obj(o) = &me2 {
                        let bb = o.cls.base_bt();
                        let base = builtin_of_bt(bb);
                        if let Some(r) = builtin_type_call(&base, &mut a) {
                            o.set_inner(Some(r?));
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
                // str.__repr__(self) / super().__str__() on a subclass of a builtin: the builtin value's text
                if let Some(i) = o.inner() {
                    return Ok(st(if rep { repr(&i)? } else { to_str(&i)? }));
                }
                return Ok(st(format!("<{}.{} object at 0x{:x}>", o.cls.module, o.cls.name, Rc::as_ptr(o) as *const u8 as usize)));
            }
            Ok(s(""))
        }));
    }
    if name == "__eq__" {
        let me2 = me.clone();
        // super().__eq__(other) on a subclass of str, int or tuple compares the values
        if let Some(inner) = inner_of(me) {
            return Ok(builtin("__eq__", move |a: Args| Ok(V::Bool(eq(&inner, &a.p[0])?))));
        }
        return Ok(builtin("__eq__", move |a: Args| Ok(if is(&me2, &a.p[0]) { V::Bool(true) } else { V::NotImpl })));
    }
    if name == "__hash__" {
        let me2 = me.clone();
        if let Some(inner) = inner_of(me) {
            return Ok(builtin("__hash__", move |_a: Args| Ok(V::Int(py_hash(&inner)?))));
        }
        return Ok(builtin("__hash__", move |_a: Args| Ok(V::Int(match &me2 {
            V::Obj(o) => (Rc::as_ptr(o) as usize >> 4) as i64,
            _ => 0,
        }))));
    }
    if name == "__init_subclass__" {
        return Ok(builtin("__init_subclass__", |_a: Args| Ok(V::None)));
    }
    if name == "__setattr__" {
        let me2 = me.clone();
        return Ok(builtin("__setattr__", move |a: Args| object_setattr(&me2, &a.p)));
    }
    if let Some(inner) = inner_of(me) {
        return getattr(&inner, name);
    }
    Err(exc("AttributeError", format!("'super' object has no attribute '{name}'")))
}
// object.__setattr__(me, name, value): the store without the class's own __setattr__, but through a property
pub fn object_setattr(me: &V, a: &[V]) -> R {
    if a.len() != 2 {
        // CPython names no function here: its message starts with a space
        return type_err(format!(" expected 2 arguments, got {}", a.len()));
    }
    let V::Str(n) = &a[0] else {
        return type_err(format!("attribute name must be string, not '{}'", tn(&a[0])));
    };
    let V::Obj(o) = me else {
        return Ok(V::None);
    };
    let name = sym(&n.s);
    if o.cls.is_prop(name) {
        if let Some(V::Prop(p)) = o.cls.lookup(name) {
            if matches!(p.1, V::None) {
                return Err(exc("AttributeError", format!("property '{}' of '{}' object has no setter", name, o.cls.name)));
            }
            call(&p.1, Args::p(vec![me.clone(), a[1].clone()]))?;
            return Ok(V::None);
        }
    }
    o.set_res(name, a[1].clone())?;
    Ok(V::None)
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

// o.name at one place in the program: the index the name had in the last instance seen there (an inline
// cache). A hit is the same instance attribute getattr would find first; anything else takes getattr.
#[inline]
#[inline]
pub fn getattr_ic(o: &V, name: Sym, ic: &std::sync::atomic::AtomicUsize) -> R {
    if let V::Obj(ob) = o {
        let d = ob.d.borrow();
        // an instance with a live __dict__ always takes the general path (rare)
        if d.dict.is_some() {
            drop(d);
            return getattr(o, name);
        }
        // the same name's text can sit at another address in another part of the program
        if let Some(v) = d.attrs.at_named(ic.load(std::sync::atomic::Ordering::Relaxed), name) {
            return Ok(v.clone());
        }
    }
    getattr_ic_slow(o, name, ic)
}
#[inline(never)]
fn getattr_ic_slow(o: &V, name: Sym, ic: &std::sync::atomic::AtomicUsize) -> R {
    if let V::Obj(ob) = o {
        let d = ob.d.borrow();
        if d.dict.is_some() {
            drop(d);
            return getattr(o, name);
        }
        if let Some(i) = d.attrs.find(name) {
            ic.store(i, std::sync::atomic::Ordering::Relaxed);
            return Ok(d.attrs.get(i).unwrap().1.clone());
        }
    }
    getattr(o, name)
}
// o.name = v at one place in the program, cached as getattr_ic is (only where setattr would just store)
#[inline]
pub fn setattr_ic(o: &V, name: Sym, v: V, ic: &std::sync::atomic::AtomicUsize) -> Result<(), E> {
    if let V::Obj(ob) = o {
        if ob.cls.plain_set(name) {
            let mut d = ob.d.borrow_mut();
            if d.dict.is_some() {
                drop(d);
                return setattr(o, name, v);
            }
            let i = ic.load(std::sync::atomic::Ordering::Relaxed);
            if let Some(e) = d.attrs.at_named_mut(i, name) {
                *e = v;
                return Ok(());
            }
            // a new attribute (as in __init__): the layout already has a child for this name, so it is not set yet
            if i == d.attrs.len() {
                match d.attrs.push_known(name, v) {
                    Ok(()) => return Ok(()),
                    Err(v) => {
                        drop(d);
                        return setattr_ic_slow(o, name, v, ic);
                    }
                }
            }
        }
    }
    setattr_ic_slow(o, name, v, ic)
}
#[inline(never)]
fn setattr_ic_slow(o: &V, name: Sym, v: V, ic: &std::sync::atomic::AtomicUsize) -> Result<(), E> {
    // __dict__ is never stored inline; this is the only slow path that could, and it is rare.
    if name == "__dict__" {
        return setattr(o, name, v);
    }
    if let V::Obj(ob) = o {
        if ob.cls.plain_set(name) {
            let mut d = ob.d.borrow_mut();
            if d.dict.is_some() {
                drop(d);
                return setattr(o, name, v);
            }
            let i = match d.attrs.find(name) {
                Some(i) => {
                    *d.attrs.get_mut(i).unwrap().1 = v;
                    i
                }
                None => {
                    d.attrs.push((name, v));
                    d.attrs.len() - 1
                }
            };
            ic.store(i, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }
    }
    setattr(o, name, v)
}
pub fn getattr(o: &V, name: &str) -> R {
    match o {
        V::Obj(ob) => {
            // a class attribute named __dict__ (a property) wins over the object's own dict, as in CPython
            if name == "__dict__" && ob.cls.lookup(name).is_none() {
                return ob.dict();
            }
            if let Some(v) = ob.get_attr(name)? {
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
                "args" if ob.cls.base_bt() == BT::Exc => return Ok(tup(vec![])),
                "__cause__" | "__context__" if ob.cls.base_bt() == BT::Exc => return Ok(V::None),
                "__suppress_context__" if ob.cls.base_bt() == BT::Exc => return Ok(V::Bool(false)),
                // `raise SystemExit(2)` sets only args: code is the one arg, None for none, else the args tuple.
                "code" if ob.cls.base_bt() == BT::Exc && is_exc(o, "SystemExit") => {
                    return Ok(match ob.get("args") {
                        Some(V::Tuple(t)) if t.len() == 1 => t[0].clone(),
                        Some(V::Tuple(t)) if t.is_empty() => V::None,
                        Some(a) => a,
                        None => V::None,
                    });
                }
                // StopIteration.value: the first constructor argument, or None for no arguments.
                "value" if ob.cls.base_bt() == BT::Exc && is_exc(o, "StopIteration") => {
                    return Ok(match ob.get("args") {
                        Some(V::Tuple(t)) if !t.is_empty() => t[0].clone(),
                        _ => V::None,
                    });
                }
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
                // numpy's classes: float64, str_ (their module is numpy)
                "__name__" | "__qualname__" => Ok(s(c.name.strip_prefix("numpy.").filter(|n| !n.contains('.')).unwrap_or(c.name))),
                "__module__" => Ok(s(c.module)),
                "__mro__" => Ok(tup(c.mro.borrow().iter().map(|k| V::Class(k.clone())).collect())),
                "__bases__" => Ok(tup(c.bases.iter().map(|k| V::Class(k.clone())).collect())),
                "__doc__" => Ok(V::None),
                // the class's own names in the order its body bound them (CPython's is a read-only view
                // with a few more dunder names)
                "__dict__" => {
                    let mut d = Dict::new();
                    d.set(s("__module__"), s(c.module))?;
                    for (k, v) in c.attrs.borrow().iter() {
                        if !k.starts_with("__bv_") {
                            d.set(s(k), v.clone())?;
                        }
                    }
                    if c.own("__doc__").is_none() {
                        d.set(s("__doc__"), V::None)?;
                    }
                    Ok(V::View(Rc::new((V::Dict(Rc::new(RefCell::new(d))), 3))))
                }
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
            "__call__" => Ok(o.clone()),
            // an assigned __code__ is kept in attrs; otherwise one is made from the def's flags
            "__code__" if f.attrs.borrow().iter().all(|(k, _)| *k != "__code__") => match f.code_flags.get() {
                0 => Err(exc("AttributeError", format!("'{}' object has no attribute '__code__'", if f.kind.get() == 3 { "builtin_function_or_method" } else { "function" }))),
                fl => Ok(mk_code(f.name, fl)),
            },
            // 0 function, 1 staticmethod, 2 classmethod (Brevi's pydantic reads it)
            "__bv_kind__" => Ok(V::Int(f.kind.get() as i64)),
            "__doc__" => Ok(f.attrs.borrow().iter().find(|(k, _)| *k == "__doc__").map_or(V::None, |(_, v)| v.clone())),
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
        V::Prop(p) if name == "fget" => Ok(p.0.clone()),
        V::Prop(p) if name == "fset" => Ok(p.1.clone()),
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
        V::View(v) if name == "mapping" && v.1 != 3 => match &v.0 {
            V::Dict(_) => Ok(V::View(Rc::new((v.0.clone(), 3)))),
            _ => Err(exc("AttributeError", format!("'{}' object has no attribute 'mapping'", tn(o)))),
        },
        V::View(v) if v.1 == 3 && matches!(name, "get" | "keys" | "values" | "items" | "copy") => {
            Ok(V::BMeth(Rc::new((o.clone(), sym(name)))))
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
        V::Obj(ob) if ob.cls.plain_set(name) => {
            ob.set_res(name, v)?;
            Ok(())
        }
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
            ob.set_res(name, v)?;
            Ok(())
        }
        V::Class(c) => {
            c.set(name, v);
            Ok(())
        }
        V::Module(&ModName("decimal.context")) => decimal_context_set(name, &v),
        V::Ext(x) => match x.set_attr(o, name, v) {
            Some(r) => r,
            None => Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", tn(o), name))),
        },
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
            if let Some(da) = ob.cls.lookup("__delattr__") {
                call(&da, Args::p(vec![o.clone(), s(name)]))?;
                return Ok(());
            }
            if ob.del_attr(name)? {
                Ok(())
            } else {
                Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, name)))
            }
        }
        V::Class(c) if c.del(name) => Ok(()),
        V::Class(c) => Err(exc("AttributeError", format!("type object '{}' has no attribute '{}'", c.name, name))),
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
// o.name(x): the commonest calls on builtin containers without building Args
#[inline]
pub fn call_method1(o: &V, name: &str, x: V) -> R {
    match o {
        V::List(l) if name == "append" => {
            l.borrow_mut().push(x);
            Ok(V::None)
        }
        V::Set(st) if name == "add" => {
            st.borrow_mut().add(x)?;
            Ok(V::None)
        }
        _ => call_method(o, name, Args::p(vec![x])),
    }
}
pub fn call_method(o: &V, name: &str, mut a: Args) -> R {
    // the commonest calls on builtin containers, before the general dispatch
    if a.p.len() == 1 && a.k.is_empty() {
        match (o, name) {
            (V::List(l), "append") => {
                l.borrow_mut().push(a.p.pop().unwrap());
                return Ok(V::None);
            }
            (V::Set(st), "add") => {
                st.borrow_mut().add(a.p.pop().unwrap())?;
                return Ok(V::None);
            }
            _ => {}
        }
    }
    match o {
        V::Obj(ob) => {
            if let Some(v) = ob.get_attr(name)? {
                return call(&v, a);
            }
            if let Some(v) = ob.cls.lookup(name) {
                return match &v {
                    V::Func(f) => match f.kind.get() {
                        1 => call(&v, a),
                        2 => {
                            let mut a = a;
                            prepend(&mut a.p, V::Class(ob.cls.clone()));
                            call(&v, a)
                        }
                        _ => call_self(&v, o.clone(), a),
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
            if ob.cls.base_bt() == BT::Exc && name == "add_note" {
                if a.p.len() != 1 || !a.k.is_empty() {
                    return Err(exc("TypeError", format!("BaseException.add_note() takes exactly one argument ({} given)", a.p.len())));
                }
                if !matches!(a.p[0], V::Str(_)) {
                    return Err(exc("TypeError", format!("note must be a str, not '{}'", type_name(&a.p[0]))));
                }
                match ob.get("__notes__") {
                    Some(V::List(l)) => l.borrow_mut().push(a.p[0].clone()),
                    Some(_) => return Err(exc("TypeError", "Cannot add note: __notes__ is not a list".into())),
                    None => ob.set(sym("__notes__"), list(vec![a.p[0].clone()])),
                }
                return Ok(V::None);
            }
            let f = getattr(o, name)?;
            call(&f, a)
        }
        V::Class(c) => {
            if let Some(v) = c.lookup(name) {
                return match &v {
                    V::Func(f) if f.kind.get() == 2 => {
                        let mut a = a;
                        prepend(&mut a.p, o.clone());
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
