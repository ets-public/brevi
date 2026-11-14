
// ---------- calls ----------

thread_local! {
    pub static USER_MOD: Cell<Option<fn(&str, &str) -> Option<R>>> = Cell::new(None);
    // a file of the program by module name -> its absolute path (for the module's repr)
    pub static USER_MOD_FILE: Cell<Option<fn(&str) -> Option<&'static str>>> = Cell::new(None);
    // a library module's repr as the build machine's CPython printed it (emit.rs lib_mod_reprs)
    pub static LIB_MOD_REPR: Cell<Option<fn(&str) -> Option<&'static str>>> = Cell::new(None);
    // __main__'s name -> global slot table (its gtab), for lookups by name at run time (pickle by reference)
    pub static MAIN_GTAB: Cell<Option<fn(&str) -> Option<usize>>> = Cell::new(None);
    pub static DEPTH: Cell<usize> = Cell::new(0);
    pub static REC_LIMIT: Cell<usize> = Cell::new(1000);
}
// CPython merges a call's keywords (explicit and **) into one dict before it binds anything, and
// the first key whose second occurrence arrives raises TypeError. A keyword list is almost always
// tiny, so it is scanned in place; only a very large ** dict goes through a set.
#[inline]
fn kw_dup(k: &[(Sym, V)]) -> Option<Sym> {
    if k.len() < 2 {
        return None;
    }
    if k.len() <= 16 {
        for j in 1..k.len() {
            for i in 0..j {
                if k[i].0 == k[j].0 {
                    return Some(k[j].0);
                }
            }
        }
        return None;
    }
    let mut seen: std::collections::HashSet<Sym> = std::collections::HashSet::with_capacity(k.len());
    for (n, _) in k {
        if !seen.insert(*n) {
            return Some(*n);
        }
    }
    None
}
// CPython names a bound builtin method's owner in duplicate-keyword errors: `dict.update`,
// `collections.Counter.update`. type_name() already has most of these; Counter is the one whose
// repr CPython shortens, so its module is added back here (only on the error path).
fn owner_name(v: &V) -> String {
    if let V::Dict(d) = v {
        if d.borrow().kind == 2 {
            return "collections.Counter".into();
        }
    }
    type_name(v)
}
pub fn call(f: &V, a: Args) -> R {
    match f {
        V::Func(fun) => {
            if let Some(k) = kw_dup(&a.k) {
                return type_err(format!("{}() got multiple values for keyword argument '{}'", fun.fq.get(), k));
            }
            (fun.f)(a)
        }
        V::Bound(b) => call_self(&b.1, b.0.clone(), a),
        V::BMeth(b) => {
            if let Some(k) = kw_dup(&a.k) {
                return type_err(format!("{}.{}() got multiple values for keyword argument '{}'", owner_name(&b.0), b.1, k));
            }
            call_method(&b.0, b.1, a)
        }
        V::Class(c) => {
            if let Some(k) = kw_dup(&a.k) {
                let n = if c.module == "builtins" { c.name.to_string() } else { format!("{}.{}", c.module, c.name) };
                return type_err(format!("{}() got multiple values for keyword argument '{}'", n, k));
            }
            instantiate(c, a)
        }
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
            V::Str(n) => a.k.push((sym(&n.text()), x)),
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
    make_class_f(name, name, module, bases, attrs, vec![])
}
// As make_class, with the names the class body annotates (dataclass and NamedTuple fields). `qualname` is
// CPython's dotted __qualname__ (`outer.<locals>.Name` inside a function, `Outer.Inner` inside a class).
pub fn make_class_f(name: Sym, qualname: Sym, module: Sym, bases: Vec<V>, attrs: Vec<(Sym, V)>, fields: Vec<Sym>) -> R {
    if let Some(m) = bases_meta(&bases)? {
        return meta_from_base(&m, name, qualname, module, bases, attrs, fields, None);
    }
    make_class_impl(name, qualname, module, bases, attrs, fields, None, None)
}
// A class statement's class: as make_class_f, with what CPython's compiler adds to the namespace
// (__firstlineno__, __static_attributes__)
// (and, when not "", why its dict cannot be CPython's: the compiler knows when it is not)
pub fn make_class_s(name: Sym, mut qualname: Sym, module: Sym, bases: Vec<V>, mut attrs: Vec<(Sym, V)>, fields: Vec<Sym>, first: i64, sattrs: &[&'static str], why: &'static str) -> R {
    // `__qualname__ = '...'` in the body is the class's __qualname__ (type.__new__ takes it out of the dict)
    if let Some(i) = attrs.iter().position(|(k, _)| *k == "__qualname__") {
        match &attrs[i].1 {
            V::Str(x) => qualname = sym(&x.text()),
            x => return type_err(format!("type __qualname__ must be a str, not {}", tn(x))),
        }
        attrs.remove(i);
    }
    let c = match bases_meta(&bases)? {
        Some(m) => meta_from_base(&m, name, qualname, module, bases, attrs, fields, Some((first, sattrs)))?,
        None => make_class_impl(name, qualname, module, bases, attrs, fields, Some((first, sattrs.to_vec())), None)?,
    };
    if !why.is_empty() {
        if let V::Class(k) = &c {
            tdict_inexact(k, why);
        }
    }
    Ok(c)
}
// A base made by a program's metaclass: that metaclass makes the class, as __build_class__ has it do, from the
// namespace the class body made (with a class statement's __firstlineno__ and __static_attributes__)
#[allow(clippy::too_many_arguments)]
fn meta_from_base(m: &Rc<Class>, name: Sym, qualname: Sym, module: Sym, bases: Vec<V>, attrs: Vec<(Sym, V)>, fields: Vec<Sym>, stmt: Option<(i64, &[&'static str])>) -> R {
    if meta_lookup(m, "__prepare__").is_some() {
        return Err(unsupported("a metaclass's __prepare__ for a class that inherits the metaclass from a base"));
    }
    let ns = dict_new();
    setitem(&ns, &s("__module__"), s(module))?;
    setitem(&ns, &s("__qualname__"), s(qualname))?;
    if let Some((first, _)) = stmt {
        setitem(&ns, &s("__firstlineno__"), V::Int(first))?;
    }
    for (k, v) in attrs {
        setitem(&ns, &s(k), v)?;
    }
    if let Some((_, sa)) = stmt {
        setitem(&ns, &s("__static_attributes__"), tup(sa.iter().map(|x| s(x)).collect()))?;
    }
    PENDING_FIELDS.with(|p| *p.borrow_mut() = Some(fields));
    meta_call(m, Args::p(vec![s(name), tup(bases), ns]))
}
#[allow(clippy::too_many_arguments)]
fn make_class_impl(name: Sym, qualname: Sym, module: Sym, mut bases: Vec<V>, mut attrs: Vec<(Sym, V)>, fields: Vec<Sym>, stmt: Option<(i64, Vec<Sym>)>, meta: Option<Rc<Class>>) -> R {
    // a class on Protocol is a protocol (rt_typing.rs), which may inherit only from protocols
    let protocol = proto_bases_check(&bases)?;
    // `class C[T](...)` (PEP 695): Generic[T, ...] comes last. As CPython's: a Generic of the class's own among its
    // bases is a TypeError, and so is `object` (no MRO puts it before Generic). __parameters__ is the class's own
    // type parameters (Generic.__init_subclass__ sets it in the class's dict).
    let pep695 = match bases.last() {
        Some(V::Ext(e)) => e.any().downcast_ref::<TForm>().and_then(|f| f.pep695.clone()),
        _ => None,
    };
    if let Some(tv) = pep695 {
        let others = &bases[..bases.len() - 1];
        let generic = |b: &V| match b {
            V::Class(k) => k.module == "typing" && k.name == "Generic",
            V::Ext(e) => e.any().downcast_ref::<TForm>().is_some_and(|f| f.name == "Generic"),
            _ => false,
        };
        if others.iter().any(generic) {
            return type_err("Cannot inherit from Generic[...] multiple times.".into());
        }
        let object = |b: &V| matches!(b, V::Class(k) if k.module == "builtins" && k.name == "object");
        if others.iter().any(object) {
            if others.len() == 1 {
                return type_err("Cannot create a consistent method resolution order (MRO) for bases object, Generic".into());
            }
            return Err(unsupported("a generic class (PEP 695) with object and other bases"));
        }
        if meta.is_some() {
            return Err(unsupported("a generic class (PEP 695) with a metaclass"));
        }
        attrs.push((sym("__parameters__"), tup(tv)));
    } else {
        // a subclass of a PEP 695 class: Generic.__init_subclass__ gives it its own __parameters__, () on a plain
        // base; on a subscripted one (`class D(C[int])`) it is worked out from arguments this runtime does not keep
        let p695 = |k: &Rc<Class>| k.lookup("__parameters__").is_some() && k.lookup("__type_params__").is_some();
        let mut plain = false;
        for b in &bases {
            match b {
                V::Class(k) if p695(k) => plain = true,
                V::Ext(e) if e.any().downcast_ref::<TForm>().is_some_and(|f| f.origin.as_ref().is_some_and(p695)) => {
                    return Err(unsupported("a class on a subscripted generic class (PEP 695)"));
                }
                _ => {}
            }
        }
        if plain && !attrs.iter().any(|(n, _)| *n == "__parameters__") {
            attrs.push((sym("__parameters__"), tup(vec![])));
        }
    }
    // a class on Generic, Protocol or a typing alias (which typing_class takes out of the bases), or on such a class
    // (Class.made: 2 when how its subscript goes is not worked out here, 3 for none of its own type parameters, as
    // with `class B(G)` or `class P(Protocol)`, 10 + n for `class G(Generic[T, ...])` with n of them)
    let made = {
        let (mut own, mut unknown, mut family) = (None, false, false);
        for b in &bases {
            match b {
                V::Class(k) if k.module == "typing" && k.name == "Protocol" => family = true,
                V::Class(k) if k.module == "typing" && k.name == "Generic" => unknown = true,
                V::Class(k) if k.made.get() >= 2 && k.made.get() != 5 => family = true,
                V::Ext(e) => match e.any().downcast_ref::<TForm>() {
                    Some(f) if matches!(f.name, "Generic" | "Protocol") && own.is_none() => own = Some(f.params),
                    _ if matches!(e.tname(), "typing._GenericAlias" | "typing._UnionGenericAlias") => unknown = true,
                    _ => {}
                },
                _ => {}
            }
        }
        match own {
            _ if unknown => 2,
            Some(n) if n < 200 => 10 + n as u8,
            Some(_) => 2,
            None if family => 3,
            None => 1,
        }
    };
    if let Some(r) = typing_class(name, qualname, module, &mut bases, &attrs, &fields) {
        if meta.is_some() {
            return Err(unsupported("a metaclass for a NamedTuple or TypedDict class"));
        }
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
    iter_base_check(&bs)?;
    if name != "ByteString" && bs.iter().any(|b| is_bytestring(b)) {
        bytestring_warn()?;
    }
    let bt = if bs.iter().any(|b| b.base_bt() == BT::Exc) { BT::Exc } else { BT::Other };
    // A collections.abc class, or one built on it, follows ABCMeta's rules.
    let abc = module == "collections.abc" || bs.iter().any(|b| b.abc.get());
    let c = Class::with_qualname(name, qualname, module, bs, bt);
    c.abc.set(abc);
    c.made.set(made);
    let meta_is_some = meta.is_some();
    *c.meta.borrow_mut() = meta;
    for (n, v) in attrs {
        c.set_exact(n, v);
    }
    // what type_new adds after the namespace: slot members, __dict__ and __weakref__ (rt_tdict.rs), then
    // __doc__ (None without a docstring: every class has its own, not its parent's)
    tdict_setup(&c, &c.bases, stmt);
    if c.own("__doc__").is_none() {
        c.set_exact("__doc__", V::None);
    }
    if is_metaclass(&c) {
        meta_class_check(&c)?;
    }
    // type.__new__: a class that defines __eq__ and no __hash__ gets __hash__ = None (unhashable, and so
    // not Hashable), which its subclasses inherit
    if c.own("__eq__").is_some() && c.own("__hash__").is_none() {
        c.set_exact("__hash__", V::None);
    }
    if abc {
        abc_setup(&c)?;
    }
    tdict_seal(&c);
    // chain.from_iterable is a classmethod: C.from_iterable(x) makes a C
    if let Some(base) = iter_base_of(&c) {
        if (base.module, base.name) == ("itertools", "chain") {
            // unless a class of the program defines its own from_iterable before chain in the MRO
            if c.lookup("from_iterable").is_some_and(|f| is_chain_fi(&f)) {
                c.set_exact("from_iterable", chain_from_iterable_for(&c));
            }
        }
    }
    // a __slots__ name that the class body also binds is refused, as CPython refuses it
    if let Some(sv) = c.own("__slots__") {
        let mut own_slots = vec![];
        slot_names(c.name, &sv, &mut own_slots);
        for n in own_slots {
            if n == "__dict__" || n == "__weakref__" {
                continue;
            }
            if c.attrs.borrow().iter().any(|(k, _)| *k == n) {
                return Err(exc("ValueError", format!("'{n}' in __slots__ conflicts with class variable")));
            }
        }
    }
    *c.fields.borrow_mut() = fields;
    // a class of the program that defines __set__ or __delete__ makes its instances data descriptors, which
    // attribute reads must then ask before an instance's own attributes (see getattr)
    if c.attrs.borrow().iter().any(|(k, _)| *k == "__set__" || *k == "__delete__") {
        USER_DATA_DESC.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // type.__new__ calls __set_name__(cls, name) on each attribute whose type defines it, in definition order;
    // an error propagates as it is (CPython 3.12+)
    let hooks: Vec<(Sym, V, V)> = c
        .attrs
        .borrow()
        .iter()
        .filter_map(|(k, v)| match v {
            V::Obj(d) => d.cls.lookup("__set_name__").map(|f| (*k, v.clone(), f)),
            _ => None,
        })
        .collect();
    for (k, v, f) in hooks {
        call(&f, Args::p(vec![v, V::Class(c.clone()), s(k)]))?;
    }
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
    if is_enum(&c) {
        enum_made(&c);
    }
    if protocol {
        if meta_is_some {
            return Err(unsupported("a Protocol class with a metaclass"));
        }
        proto_register(&c);
    }
    Ok(V::Class(c))
}
// ABCMeta's test: getattr(value, "__isabstractmethod__", False) is true. A def has it when @abstractmethod set
// it; a property, classmethod, partialmethod or singledispatchmethod answers for what it wraps.
fn is_abstract_method(v: &V) -> Result<bool, E> {
    match v {
        V::Func(f) => {
            let a = f.attrs.borrow().iter().find(|(k, _)| *k == "__isabstractmethod__").map(|(_, x)| x.clone());
            a.map_or(Ok(false), |x| truth(&x))
        }
        V::None | V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_) | V::Str(_) | V::Tuple(_) | V::List(_) | V::Dict(_) | V::Set(_) | V::FSet(_) => Ok(false),
        // a native value answers only for attributes it has (getattr's fallback would offer any name as a
        // method): the class's own _abc_impl and __abstractmethods__ are never abstract themselves
        V::Ext(x) => match x.attr(v, "__isabstractmethod__") {
            Some(Ok(a)) => truth(&a),
            Some(Err(e)) if is_exc(&e.0, "AttributeError") => Ok(false),
            Some(Err(e)) => Err(e),
            None => Ok(false),
        },
        _ => match getattr(v, "__isabstractmethod__") {
            Ok(x) => truth(&x),
            Err(e) if is_exc(&e.0, "AttributeError") => Ok(false),
            Err(e) => Err(e),
        },
    }
}
// A class made with metaclass=ABCMeta: ABCMeta.__new__ runs type.__new__ (with __set_name__ and
// __init_subclass__) and then its own setup, so the class takes ABCMeta's rules here, after it is made.
pub fn abc_meta(c: V) -> R {
    if let V::Class(k) = &c {
        if !k.abc.get() {
            k.abc.set(true);
            abc_setup(k)?;
        }
    }
    Ok(c)
}
// Whether n, looked up along c's MRO, is still abstract: a builtin base (dict, list, ...) that defines n in
// CPython's type dict supplies a concrete method, as CPython's lookup finds it before an ABC's abstract one.
fn abc_mro_abstract(c: &Rc<Class>, n: &str) -> Result<bool, E> {
    for k in c.mro.borrow().iter() {
        if k.made.get() == 0 && k.tinfo.borrow().is_none() && k.name != "object" {
            if let Some(t) = TYPE_DICTS.iter().find(|t| t.0 == k.module && t.1 == k.name) {
                if t.5.iter().any(|e| e.0 == n) {
                    return Ok(false);
                }
                continue;
            }
        }
        if let Some(v) = k.own(n) {
            return is_abstract_method(&v);
        }
    }
    Ok(false)
}
// ABCMeta.__new__'s class setup: __abstractmethods__, __flags__ and the flag conflict check.
fn abc_setup(c: &Rc<Class>) -> R {
    let mut names: Vec<Sym> = vec![];
    let own: Vec<(Sym, V)> = c.attrs.borrow().iter().map(|(n, v)| (*n, v.clone())).collect();
    for (n, v) in own.iter() {
        if is_abstract_method(v)? && !names.contains(n) {
            names.push(*n);
        }
    }
    for b in c.bases.iter() {
        if let Some(fs) = b.own("__abstractmethods__") {
            if let V::FSet(p) = &fs {
                for k in p.keys() {
                    let n = to_str(&k).unwrap_or_default();
                    let abs = abc_mro_abstract(c, &n)?;
                    if !names.iter().any(|x| same_name(x, &n)) && abs {
                        names.push(sym(&n));
                    }
                }
            }
        }
    }
    names.sort();
    let abs = PSet::from_iter(names.iter().map(|n| s(n)).collect())?;
    c.set_exact("__abstractmethods__", V::FSet(Rc::new(abs)));
    // the Py_TPFLAGS_SEQUENCE/MAPPING bits, kept as Brevi's own __bv_flags__ (CPython keeps them in tp_flags,
    // not the dict); _abc_init removes __abc_tpflags__ from the dict
    // CPython's type creation takes the bits from the first class in the MRO that has them (inherit_patma_flags);
    // an __abc_tpflags__ in the class body replaces them
    let mut flags: i64 = c.mro.borrow().iter().skip(1).map(pm_own_flags).find(|f| *f != 0).unwrap_or(0);
    if let Some(V::Int(f)) = c.own("__abc_tpflags__") {
        if f & 32 != 0 && f & 64 != 0 {
            return Err(exc("TypeError", "__abc_tpflags__ cannot be both Py_TPFLAGS_SEQUENCE and Py_TPFLAGS_MAPPING".into()));
        }
        flags = f & 96;
        c.del_exact("__abc_tpflags__");
    }
    c.set_exact("__bv_flags__", V::Int(flags));
    c.set_exact("_abc_impl", V::Ext(ExtRc::new(AbcData)));
    Ok(V::None)
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
// A def's __annotations__, evaluated when the def was made (see ANN_DEFS in typing.rs): a dict of the
// annotated parameters in order, then "return".
pub fn with_ann(f: V, kv: Vec<(V, V)>) -> R {
    let d = dict_new();
    for (k, v) in kv {
        setitem(&d, &k, v)?;
    }
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__annotations__", d));
    }
    Ok(f)
}
// A PEP 695 def's __type_params__ (one tuple, so every read is the same object).
pub fn with_tparams(f: V, t: V) -> V {
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__type_params__", t));
    }
    f
}
// The private first-annotation descriptor a def carries when it is decorated with a bare `@register`
// (functools.singledispatch reads it; see sd_ann_desc in emit.rs). Classmethod and staticmethod copy it with
// the def's other attributes.
pub fn with_sd_ann(f: V, ann: V) -> V {
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__brevi_sd_ann__", ann));
    }
    f
}
// A def's __module__ when it isn't __main__ (the default getattr gives).
pub fn with_module(f: V, m: &str) -> V {
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__module__", s(m)));
    }
    f
}
// A def's user attributes live in its attrs until its __dict__ is read; from then on that dict holds them, so
// writes through `f.__dict__` and through `f.x = ...` are the same.
fn func_dict(f: &Rc<Func>) -> Option<Rc<RefCell<Dict>>> {
    match f.attrs.borrow().iter().find(|(k, _)| *k == "__bv_dict__") {
        Some((_, V::Dict(d))) => Some(d.clone()),
        _ => None,
    }
}
fn func_dict_make(f: &Rc<Func>) -> Result<Rc<RefCell<Dict>>, E> {
    if let Some(d) = func_dict(f) {
        return Ok(d);
    }
    let mut d = Dict::new();
    let mut a = f.attrs.borrow_mut();
    for (k, v) in a.iter().filter(|(k, _)| !FUNC_SLOT_ATTRS.contains(k)) {
        d.set(s(k), v.clone())?;
    }
    a.retain(|(k, _)| FUNC_SLOT_ATTRS.contains(k));
    let d = Rc::new(RefCell::new(d));
    a.push(("__bv_dict__", V::Dict(d.clone())));
    Ok(d)
}
// A user attribute of a def (None when it has none).
pub fn func_user_attr(f: &Rc<Func>, name: &str) -> Result<Option<V>, E> {
    if let Some(v) = f.attrs.borrow().iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()) {
        return Ok(Some(v));
    }
    match func_dict(f) {
        Some(d) => Dict::get_cell(&d, &s(name)),
        None => Ok(None),
    }
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
    if c.module != "builtins" && is_metaclass(c) {
        return meta_call(c, a);
    }
    proto_call_check(c)?;
    if c.abc.get() {
        if let Some(V::FSet(p)) = c.lookup("__abstractmethods__") {
            let mut names: Vec<String> = p.keys().iter().map(|k| to_str(k).unwrap_or_default()).collect();
            names.sort();
            if !names.is_empty() {
                let many = names.len() > 1;
                return type_err(format!(
                    "Can't instantiate abstract class {} without an implementation for abstract {}{} {}",
                    c.name,
                    if many { "methods" } else { "method" },
                    if names.len() == 1 { "" } else { "" },
                    names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }
    // the classes of Brevi's own iterators (and itertools._tee_dataobject): the builtin or itertools function of the
    // same name; for the others a plain object would be silently wrong
    if c.module != "__main__" && is_iter_class(c) {
        if let Some(r) = iter_class_call(c, a) {
            return r;
        }
        return Err(exc("NotImplementedError", format!("brevi: calling the {}.{} class is not supported in dynamic builds", c.module, c.name)));
    }
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
        if is_enum(c) || c.is_sub(&enum_base()) {
            return enum_call(c, a);
        }
        if Rc::ptr_eq(c, &eprop_class()) {
            return eprop_new(a);
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
                    match c.lookup("__init__") {
                        Some(init) if !is_builtin_init(&init) => {
                            prepend(&mut a.p, o.clone());
                            call(&init, a)?;
                        }
                        // set.__init__ runs after a subclass's own __new__: it refuses keywords, and
                        // refills the set from the arguments
                        _ if c.base_bt() == BT::Set => {
                            if let Some(i @ V::Set(_)) = ob.inner() {
                                set_method(&i, "__init__", a)?;
                            }
                        }
                        _ => {}
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
    // CPython 3.13's OSError is a factory: OSError(errno, strerror, ...) makes the subclass the errno names
    // (PermissionError and so on), so the fields below belong to that subclass.
    let c2 = if c.module == "builtins" && c.name == "OSError" { ose_factory(c, &a.p) } else { c.clone() };
    let o = Obj::new(c2.clone());
    let me = V::Obj(o.clone());
    let bb = c2.base_bt();
    if bb == BT::Exc {
        let builtin_init = c2.lookup("__init__").map_or(true, |f| is_builtin_init(&f));
        if builtin_init {
            // CPython 3.13's builtin exception constructors take no keyword arguments, except
            // ImportError/AttributeError/NameError, which take their C fields by keyword.
            if !a.k.is_empty() {
                exc_kw_init(c, &c2, &o, &mut a)?;
            }
            if c2.is_sub(&ose_class()) {
                ose_init(&o, &a.p);
            } else {
                o.set("args", tup(a.p.clone()));
            }
        } else {
            o.set("args", tup(a.p.clone()));
        }
    } else if bb != BT::Object && bb != BT::Other {
        if let Some(e) = date_sub_refused(&c2) {
            return Err(e);
        }
        // subclass of a builtin container: the instance wraps a builtin value
        let mut a0 = Args { p: vec![], k: vec![] };
        // An immutable builtin is made by its __new__, which takes the arguments whatever __init__ does
        // (HashCountingInt(int) with its own __init__ still wraps the int it was given). frozenset and
        // tuple subclasses ignore keywords their own __init__ takes.
        let own_init = c.lookup("__init__").is_some_and(|f| !is_builtin_init(&f));
        if matches!(bb, BT::FSet | BT::Tuple) {
            if !a.k.is_empty() && !own_init {
                return type_err(format!("{}() takes no keyword arguments", c.name));
            }
            a0 = Args { p: a.p.clone(), k: vec![] };
        } else if matches!(bb, BT::Int | BT::Float | BT::Str | BT::Bytes) {
            a0 = Args { p: a.p.clone(), k: a.k.clone() };
        } else if c.lookup("__init__").map_or(true, |f| is_builtin_init(&f)) {
            a0 = Args { p: a.p.clone(), k: a.k.clone() };
        }
        if bb == BT::Counter && c.lookup("__init__").map_or(true, |f| is_builtin_init(&f)) {
            // Counter's __init__ is self.update(...): through the subclass's overrides
            counter_obj_init(&me, a)?;
            return Ok(me);
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
    matches!(f, V::Func(x) if x.kind.get() == 3 && x.name.get() == "__init__")
}
// CPython 3.13's builtin exception constructors take no keyword arguments, except ImportError (name=,
// path=), AttributeError (name=, obj=) and NameError (name=). Those fields are keyword-only C slots:
// positional arguments all go to args, the fields default to None, and the error messages use the C
// constructor's own name with CPython's keyword-count cap.
fn exc_kw_init(c: &Rc<Class>, c2: &Rc<Class>, o: &Rc<Obj>, a: &Args) -> Result<(), E> {
    let n = a.k.len();
    let (base, max, k1, k2): (&str, usize, &str, Option<&str>) = if c2.is_sub(&exc_class("ImportError")) {
        ("ImportError", 3, "name", Some("path"))
    } else if c2.is_sub(&exc_class("AttributeError")) {
        ("AttributeError", 2, "name", Some("obj"))
    } else if c2.is_sub(&exc_class("NameError")) {
        ("NameError", 1, "name", None)
    } else {
        return type_err(format!("{}() takes no keyword arguments", c.name));
    };
    if n > max {
        let word = if max == 1 { "argument" } else { "arguments" };
        return type_err(format!("{base}() takes at most {max} keyword {word} ({n} given)"));
    }
    for (k, v) in &a.k {
        if *k == k1 || k2 == Some(*k) {
            o.set(sym(k), v.clone());
        } else {
            return type_err(format!("{base}() got an unexpected keyword argument '{k}'"));
        }
    }
    Ok(())
}
// The subclass a plain OSError(errno, ...) really is (OSError itself when the errno names no subclass).
fn ose_factory(c: &Rc<Class>, p: &[V]) -> Rc<Class> {
    if (2..=5).contains(&p.len()) {
        let myerrno = match &p[0] {
            V::Int(i) => *i,
            V::Bool(b) => *b as i64,
            _ => -1,
        };
        let name = ose_errno_name(myerrno);
        if name != "OSError" {
            return exc_class(name);
        }
    }
    c.clone()
}
// OSError(errno, strerror, filename, winerror=None, filename2=None), as CPython 3.13 lays it out: the first
// two arguments are also the args attribute unless a filename is given (it then lives only in the field, so
// args is (errno, strerror)).
fn ose_init(o: &Rc<Obj>, p: &[V]) {
    let n = p.len();
    let mut stored = p.to_vec();
    if n >= 2 && n <= 5 {
        o.set(sym("errno"), p[0].clone());
        o.set(sym("strerror"), p[1].clone());
        if n >= 3 && !matches!(p[2], V::None) {
            o.set(sym("filename"), p[2].clone());
            if n >= 5 && !matches!(p[4], V::None) {
                o.set(sym("filename2"), p[4].clone());
            }
            stored = p[..2].to_vec();
        }
    }
    o.set("args", tup(stored));
}

// An instance of a datetime.date subclass is made only as an Enum member (class Holiday(date, Enum)): CPython's
// date methods and arithmetic give back the subclass (type(self)(...)), which the wrapped date cannot, so any
// other instance is refused where it would be made.
pub fn date_sub_refused(k: &Rc<Class>) -> Option<E> {
    if k.base_bt() == BT::Date && !is_enum(k) {
        return Some(unsupported(&format!("an instance of {}, a subclass of datetime.date that is not an Enum", k.name)));
    }
    None
}
// A date or datetime value (both are DTV; a datetime is a date subclass in CPython too)
pub fn is_date_v(v: &V) -> bool {
    matches!(v, V::Ext(x) if x.any().downcast_ref::<DTV>().is_some())
}
// An Enum member that wraps a date, from a datetime.date mixin (class Holiday(date, Enum)).
pub fn is_date_sub(v: &V) -> bool {
    matches!(v, V::Obj(o) if o.cls.base_bt() == BT::Date)
}

fn builtin_of_bt(bt: BT) -> Rc<Class> {
    bclass(match bt {
        BT::List => "list",
        BT::Dict => "dict",
        BT::Set => "set",
        BT::FSet => "frozenset",
        BT::Str => "str",
        BT::Int => "int",
        BT::Float => "float",
        BT::Tuple => "tuple",
        BT::DefaultDict => "defaultdict",
        BT::Counter => "Counter",
        BT::OrderedDict => "OrderedDict",
        BT::Deque => "deque",
        BT::Bytes => return ext_class("bytes"),
        BT::ByteArray => return ext_class("bytearray"),
        BT::Date => return ext_class("datetime.date"),
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
        // EnumType.__new__ gives every enum class (IntEnum, a program's) Enum.__new__ in its dict
        if name == "__new__" && k.is_sub(&enum_base()) {
            if let Some(v) = enum_base().own("__new__") {
                return Ok(v);
            }
        }
        // a builtin base (int in `class MyInt(int)`) whose CPython dict has __new__ or __init__ is where the
        // lookup stops: the builtin fallback below is that method, and a later base's (Enum.__new__) is not reached
        if matches!(name, "__new__" | "__init__") && k.own(name).is_none() && k.module == "builtins" && k.name != "object" && matches!(cpy_dict_has(k, name), Some((true, _))) {
            break;
        }
        if let Some(v) = k.own(name) {
            // __new__ is a static method (type.__new__ makes it one): super().__new__(cls, ...) passes cls itself
            if name == "__new__" && matches!(v, V::Func(_)) {
                return Ok(v);
            }
            return Ok(bind_attr(v, me, true));
        }
    }
    // a metaclass's super().__new__(mcls, name, bases, ns) and __init__: type's
    if let V::Class(t) = me {
        if is_metaclass(t) && mro[start..].iter().any(|k| is_type_class(k)) {
            if name == "__new__" {
                return Ok(builtin("__new__", |mut a: Args| {
                    let Some(V::Class(k)) = a.p.first().cloned() else {
                        return type_err("type.__new__(X): X is not a type object".into());
                    };
                    if !is_metaclass(&k) && !is_type_class(&k) {
                        return type_err(format!("type.__new__({}): {} is not a subtype of type", k.name, k.name));
                    }
                    a.p.remove(0);
                    type_new(&k, a)
                }));
            }
            if name == "__init__" {
                return Ok(builtin("__init__", |a: Args| {
                    if a.p.len() != 1 && a.p.len() != 3 {
                        return type_err("type.__init__() takes 1 or 3 arguments".into());
                    }
                    Ok(V::None)
                }));
            }
        }
    }
    // object / builtin bases
    if name == "__new__" {
        return Ok(builtin("__new__", |mut a: Args| {
            let Some(V::Class(k)) = a.p.first().cloned() else {
                return type_err("__new__(X): X is not a type object".into());
            };
            a.p.remove(0);
            if let Some(e) = date_sub_refused(&k) {
                return Err(e);
            }
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
                        if !a.k.is_empty() {
                            exc_kw_init(&o.cls, &o.cls, o, &a)?;
                        }
                        if o.cls.is_sub(&ose_class()) {
                            ose_init(o, &a.p);
                        } else {
                            o.set("args", tup(a.p.clone()));
                        }
                    }
                    Ok(V::None)
                }));
            }
            if o.has_inner() {
                let me2 = me.clone();
                return Ok(builtin("__init__", move |mut a: Args| {
                    if let V::Obj(o) = &me2 {
                        if o.cls.base_bt() == BT::Counter {
                            return counter_obj_init(&me2, a);
                        }
                        let bb = o.cls.base_bt();
                        if bb == BT::Date {
                            // a date member is made whole by its Enum; date.__init__ takes the arguments and does nothing
                            return Ok(V::None);
                        }
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
                    if o.cls.is_sub(&ose_class()) {
                        return Ok(st(ose_str(o, &args)?));
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
                return Ok(st(format!("<{}.{} object at 0x{:x}>", o.cls.module, o.cls.qualname.get(), Rc::as_ptr(o) as *const u8 as usize)));
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
    if name == "__delattr__" {
        let me2 = me.clone();
        return Ok(builtin("__delattr__", move |a: Args| {
            if a.p.len() != 1 {
                return type_err(format!("expected 1 argument, got {}", a.p.len()));
            }
            let V::Str(n) = &a.p[0] else {
                return type_err(format!("attribute name must be string, not '{}'", tn(&a.p[0])));
            };
            object_delattr(&me2, &n.text()).map(|_| V::None)
        }));
    }
    if name == "__setattr__" {
        let me2 = me.clone();
        return Ok(builtin("__setattr__", move |a: Args| object_setattr(&me2, &a.p)));
    }
    // Counter's own update/subtract (Counter.update(self, ...), super().update(...)): through self's overrides
    if matches!(name, "update" | "subtract") && counter_inner(me).is_some() {
        let me2 = me.clone();
        let n = sym(name);
        return Ok(builtin(n, move |a: Args| counter_obj_method(&me2, n, a)));
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
    let name = sym(&n.text());
    if o.cls.is_prop(name) {
        if let Some(V::Prop(p)) = o.cls.lookup(name) {
            if matches!(p.1, V::None) {
                return Err(exc("AttributeError", format!("property '{}' of '{}' object has no setter", name, o.cls.name)));
            }
            call(&p.1, Args::p(vec![me.clone(), a[1].clone()]))?;
            return Ok(V::None);
        }
    }
    if !o.cls.may_store(name) {
        if o.cls.lookup(name).is_some() {
            return Err(exc("AttributeError", format!("'{}' object attribute '{}' is read-only", o.cls.name, name)));
        }
        return Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", o.cls.name, name)));
    }
    o.set_res(name, a[1].clone())?;
    Ok(V::None)
}
// A builtin function or a library module's function (len, functools.reduce): not a descriptor in CPython, so
// a class attribute holding one never binds (`class T: reduce = functools.reduce`, then T().reduce(f, xs)).
#[inline]
pub fn nobind(f: &Func) -> bool {
    f.kind.get() == 3 && f.attrs.borrow().iter().any(|(k, _)| *k == "__bv_nobind__")
}
// staticmethod.__get__: the function it wraps (staticmethod(f) read from a class or instance is f)
fn static_get(f: &Rc<Func>, v: &V) -> V {
    f.attrs.borrow().iter().find(|(n, _)| *n == "__bv_wrapped__").map(|x| x.1.clone()).unwrap_or_else(|| v.clone())
}
fn bind_attr(v: V, me: &V, from_class_lookup: bool) -> V {
    match &v {
        V::Func(f) => match f.kind.get() {
            1 => static_get(f, &v),
            3 if nobind(f) => v.clone(),
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

// An inline cache for a namedtuple field read: the top bit tags the value (an instance-attribute index, the
// cache's other use, never has it), the low NTF_IDX_BITS bits are the field's index in the wrapped tuple, and
// the bits between are the CLASS_EPOCH when it was filled. A change to
// any class's dict (P.x = property(...)) bumps the epoch, so the next hit misses and getattr honours the new
// descriptor. Only the lower 64 - NTF_IDX_BITS - 1 epoch bits are kept: the epoch never decreases, so a hit
// is still only possible on the exact epoch that filled it, and after 2^(64 - NTF_IDX_BITS - 1) class
// changes the tag stops hitting (reads stay correct, just take the general path).
const NTF_IDX_BITS: u32 = 24;
const NTF_IDX_MASK: usize = (1 << NTF_IDX_BITS) - 1;
const NTF_TAG: usize = 1 << (usize::BITS - 1);
const NTF_EP_MASK: usize = (NTF_TAG - 1) >> NTF_IDX_BITS;
#[inline]
fn ntf_ic(i: usize, ep: u64) -> usize {
    NTF_TAG | (((ep as usize) & NTF_EP_MASK) << NTF_IDX_BITS) | i
}
// the value behind a namedtuple field descriptor, without Args and no call
#[inline]
fn ntf_get(o: &V, ob: &Obj, idx: i64) -> R {
    let inner = ob.inner.borrow();
    if let Some(b) = inner.as_ref() {
        match getitem(&**b, &V::Int(idx)) {
            Ok(v) => Ok(v),
            // a subclass built a shorter tuple: obj[index] raised IndexError
            Err(E(e)) if is_exc(&e, "IndexError") => Err(exc("AttributeError", format!("Cannot access member at index {idx}"))),
            Err(e) => Err(e),
        }
    } else {
        drop(inner);
        Err(exc("TypeError", format!("'{}' object is not subscriptable", tn(o))))
    }
}
// A cached namedtuple field, when it is still the value getattr would find first; None means "general path".
#[inline]
fn ntf_ic_hit(o: &V, name: Sym, w: usize) -> Option<R> {
    let i = w & NTF_IDX_MASK;
    let ob = match o {
        V::Obj(ob) => ob,
        _ => return None,
    };
    // a class changed since this was cached: getattr must see the new class attribute
    if (w >> NTF_IDX_BITS) & NTF_EP_MASK != (CLASS_EPOCH.with(|e| e.get()) as usize) & NTF_EP_MASK {
        return None;
    }
    // getattr reads an instance attribute before the class descriptor, so one here (only possible after
    // object.__setattr__ bypassed the field) must keep the general path
    let d = ob.d.borrow();
    if d.dict.is_some() || d.attrs.find(name).is_some() {
        return None;
    }
    drop(d);
    // fields sit only on the namedtuple class itself, so an instance of a subclass (whose own class has no
    // fields) and a different type flowing through this site both miss and take the general path
    let fields = ob.cls.fields.borrow();
    if fields.get(i).is_some_and(|f| same_name(f, name)) {
        drop(fields);
        let inner = ob.inner.borrow();
        if let Some(b) = inner.as_ref() {
            if let V::Tuple(t) = &**b {
                if let Some(v) = t.get(i) {
                    return Some(Ok(v.clone()));
                }
            }
        }
    }
    None
}
// o.name at one place in the program: the index the name had in the last instance seen there (an inline
// cache). A hit is the same instance attribute getattr would find first; anything else takes getattr. A
// tagged value is a namedtuple field (see ntf_ic); a miss on it takes the general path below.
#[inline]
pub fn getattr_ic(o: &V, name: Sym, ic: &std::sync::atomic::AtomicUsize) -> R {
    let w = ic.load(std::sync::atomic::Ordering::Relaxed);
    if w & NTF_TAG != 0 {
        if let Some(r) = ntf_ic_hit(o, name, w) {
            return r;
        }
        // a stale or foreign tag: fall through, where the tagged index misses the instance attributes
    }
    if let V::Obj(ob) = o {
        let d = ob.d.borrow();
        // an instance with a live __dict__ always takes the general path (rare)
        if d.dict.is_some() {
            drop(d);
            return getattr(o, name);
        }
        // the same name's text can sit at another address in another part of the program
        if let Some(v) = d.attrs.at_named(w, name) {
            // a data descriptor of the program wins over the instance attribute (see getattr)
            if USER_DATA_DESC.load(std::sync::atomic::Ordering::Relaxed) && ob.cls.lookup(name).is_some_and(|x| is_data_desc(&x)) {
                drop(d);
                return getattr(o, name);
            }
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
            if USER_DATA_DESC.load(std::sync::atomic::Ordering::Relaxed) && ob.cls.lookup(name).is_some_and(|x| is_data_desc(&x)) {
                drop(d);
                return getattr(o, name);
            }
            ic.store(i, std::sync::atomic::Ordering::Relaxed);
            return Ok(d.attrs.get(i).unwrap().1.clone());
        }
        drop(d);
        // a namedtuple field: remember its index here. lookup checks that the descriptor is still V::NTF
        // (P.x = property(...) replaced it), and a subclass instance has no fields of its own.
        let fields = ob.cls.fields.borrow();
        if let Some(i) = fields.iter().position(|f| same_name(f, name)) {
            drop(fields);
            if i <= NTF_IDX_MASK {
                if let Some(V::NTF(n)) = ob.cls.lookup(name) {
                    let idx = n.idx as usize;
                    if idx == i {
                        ic.store(ntf_ic(i, CLASS_EPOCH.with(|e| e.get())), std::sync::atomic::Ordering::Relaxed);
                        return ntf_get(o, ob, idx as i64);
                    }
                }
            }
            return getattr(o, name);
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
            // a tagged namedtuple-field cache is not an instance-attribute index
            if i & NTF_TAG == 0 {
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
// (module, name) of a class Brevi names by its dotted CPython type name, as CPython's type.__module__ and
// __name__ split a static type's tp_name at its last dot; None for every other class.
fn dotted_type_name(c: &Class) -> Option<(&'static str, &'static str)> {
    if c.module != "builtins" {
        return None;
    }
    c.name.rsplit_once('.')
}
// Set once the program makes a class with __set__ or __delete__ (a data descriptor type), so attribute reads
// of programs without one never look at the class first.
pub static USER_DATA_DESC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// an instance of a program class that defines __set__ or __delete__
pub fn is_data_desc(v: &V) -> bool {
    match v {
        V::Obj(d) => d.cls.lookup("__set__").is_some() || d.cls.lookup("__delete__").is_some(),
        _ => false,
    }
}
// descriptor.__get__(obj, cls), or the descriptor itself when its type has no __get__ (a data descriptor
// with only __set__ reads as itself from the class, and from an instance without the attribute)
pub fn desc_get(d: &V, obj: &V, cls: &V) -> R {
    let V::Obj(dd) = d else { return Ok(d.clone()) };
    match dd.cls.lookup("__get__") {
        Some(f) => call(&f, Args::p(vec![d.clone(), obj.clone(), cls.clone()])),
        None => Ok(d.clone()),
    }
}
// A function's __get__(obj, cls), as CPython's function/staticmethod/classmethod method-wrapper does. A
// staticmethod's wrapper holds the wrapped def in __bv_wrapped__, so __get__ returns the def itself; a
// classmethod's __get__ binds the wrapped def to cls (or to type(obj) when cls is omitted).
pub fn func_get(f: &V, obj: V, cls: V) -> R {
    let V::Func(ff) = f else {
        return Err(exc("AttributeError", format!("'{}' object has no attribute '__get__'", tn(f))));
    };
    if matches!(obj, V::None) && matches!(cls, V::None) {
        return Err(exc("TypeError", "__get__(None, None) is invalid".into()));
    }
    let wrapped = ff
        .attrs
        .borrow()
        .iter()
        .find(|(k, _)| *k == "__bv_wrapped__")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| f.clone());
    Ok(match ff.kind.get() {
        0 => if matches!(obj, V::None) { f.clone() } else { V::Bound(Rc::new((obj, f.clone()))) },
        1 => wrapped,
        _ => {
            let owner = if matches!(cls, V::None) { V::Class(type_of(&obj)?) } else { cls };
            V::Bound(Rc::new((owner, wrapped)))
        }
    })
}
// getattr on a class whose type is type (or a metaclass's instance, after meta_getattr's own steps)
fn class_getattr(o: &V, c: &Rc<Class>, name: &str) -> R {
    // annotations are the class's own only, never a base's (CPython's type.__getattribute__).
    // Builtin types keep their old path (object raises AttributeError).
    if name == "__annotations__" && c.module != "builtins" {
        return Ok(c.annotations());
    }
    if let Some(v) = c.lookup(name) {
        return Ok(match &v {
            V::Func(f) if f.kind.get() == 2 => V::Bound(Rc::new((o.clone(), v.clone()))),
            V::Func(f) if f.kind.get() == 1 => static_get(f, &v),
            // a descriptor of the program read from the class: __get__(None, cls)
            V::Obj(d) if d.cls.lookup("__get__").is_some() => return desc_get(&v, &V::None, o),
            _ => {
                if let V::Ext(x) = &v {
                    if let Some(r) = x.descriptor(&v, &V::None, o) {
                        return r;
                    }
                }
                v
            }
        });
    }
    // what only the type dict holds: __firstlineno__, __static_attributes__, slot members, __weakref__
    if let Some(v) = tdict_attr(c, name) {
        return Ok(v);
    }
    if c.abc.get() {
        match name {
            // ABCMeta's register(), a method of the metaclass bound to the class
            "register" => {
                let me = o.clone();
                return Ok(builtin("register", move |a: Args| {
                    let sub = a1(&a, "register")?;
                    abc_register(Args::p(vec![me.clone(), sub]))
                }));
            }
            "__flags__" => {
                if let Some(v) = c.lookup("__bv_flags__") {
                    return Ok(v);
                }
            }
            _ => {}
        }
    }
    if c.name.contains('.') {
        if let Some(v) = ext_class_attr(c.name, name) {
            return Ok(v);
        }
    } else if c.base_bt() == BT::Date {
        // date's class attributes on a subclass: min, max and resolution are plain dates and a timedelta, as in
        // CPython; its alternate constructors (today, fromordinal, ...) build cls(...), which is refused
        if let Some(v) = ext_class_attr("datetime.date", name) {
            if matches!(name, "min" | "max" | "resolution") {
                return Ok(v);
            }
            return Err(unsupported(&format!("datetime.date.{name} on {}, a subclass of datetime.date", c.name)));
        }
    }
    // a builtin type has none (CPython's static types raise)
    if name == "__annotations__" {
        return Err(exc("AttributeError", format!("type object '{}' has no attribute '__annotations__'", c.name)));
    }
    match name {
        // A library type named by its dotted CPython type name ("datetime.date", "numpy.float64"; see
        // ext_class): CPython splits such a name at its last dot into __module__ and __name__.
        "__name__" => Ok(s(dotted_type_name(c).map_or(c.name, |(_, n)| n))),
        "__qualname__" => Ok(s(dotted_type_name(c).map_or(c.qualname.get(), |(_, n)| n))),
        "__module__" => Ok(s(dotted_type_name(c).map_or(c.module, |(m, _)| m))),
        "__mro__" => Ok(tup(c.mro.borrow().iter().map(|k| V::Class(k.clone())).collect())),
        "__bases__" => Ok(tup(c.bases.iter().map(|k| V::Class(k.clone())).collect())),
        "__subclasses__" => {
            // the type class itself exposes the unbound descriptor; every other class gets it bound
            if c.module == "builtins" && c.name == "type" {
                return Ok(builtin("__subclasses__", |mut a: Args| {
                    if a.p.is_empty() {
                        return Err(exc("TypeError", "unbound method type.__subclasses__() needs an argument".into()));
                    }
                    let me = a.p.remove(0);
                    match me {
                        V::Class(k) => {
                            if !a.p.is_empty() {
                                return Err(exc("TypeError", format!("type.__subclasses__() takes no arguments ({} given)", a.p.len())));
                            }
                            if !a.k.is_empty() {
                                return Err(exc("TypeError", "type.__subclasses__() takes no keyword arguments".into()));
                            }
                            Ok(list(k.subs.borrow().iter().filter_map(|w| w.upgrade()).map(V::Class).collect()))
                        }
                        other => Err(exc("TypeError", format!("descriptor '__subclasses__' for 'type' objects doesn't apply to a '{}' object", tn(&other)))),
                    }
                }));
            }
            let c = c.clone();
            Ok(builtin("__subclasses__", move |a: Args| {
                if !a.p.is_empty() {
                    return Err(exc("TypeError", format!("{}.__subclasses__() takes no arguments ({} given)", c.name, a.p.len())));
                }
                if !a.k.is_empty() {
                    return Err(exc("TypeError", format!("{}.__subclasses__() takes no keyword arguments", c.name)));
                }
                Ok(list(c.subs.borrow().iter().filter_map(|w| w.upgrade()).map(V::Class).collect()))
            }))
        }
        "__doc__" => Ok(V::None),
        // a PEP 695 class's own (in its dict), else CPython's empty tuple (type's getter reads only the class's own)
        "__type_params__" => Ok(c.own("__type_params__").unwrap_or_else(|| tup(vec![]))),
        // the class's own names in the order its body bound them (CPython's is a read-only view
        // with a few more dunder names)
        // CPython's type dict, as a read-only view (rt_tdict.rs; refused where Brevi cannot give it exactly)
        "__dict__" => class_dict_view(c),
        _ => {
            // methods of a builtin base (tuple's __getitem__ on a namedtuple class, list's
            // append on a list subclass) live on the builtin class in CPython, not on the subclass
            if c.module == "builtins" || c.module == "collections" {
                // a builtin type has only what CPython's type dicts hold (hasattr(int, "zzz") is False)
                if cpy_type_has(c, name) == Some(false) {
                    return Err(exc("AttributeError", format!("type object '{}' has no attribute '{}'", if c.module == "builtins" { c.name.to_string() } else { format!("{}.{}", c.module, c.name) }, name)));
                }
                if let Some(v) = builtin_class_attr(c, name) {
                    return Ok(v);
                }
            }
            // a bytes subclass: maketrans is bytes' static method; fromhex is a classmethod that builds
            // the subclass from the bytes, as CPython's does (cls(bytes.fromhex(s)))
            // (a bytearray subclass alike: cls(bytearray.fromhex(s)))
            if matches!(c.base_bt(), BT::Bytes | BT::ByteArray) {
                if name == "maketrans" {
                    return Ok(builtin("maketrans", bytes_maketrans));
                }
                if name == "fromhex" {
                    let k = V::Class(c.clone());
                    if c.base_bt() == BT::ByteArray {
                        return Ok(builtin("fromhex", move |a: Args| call(&k, Args::p(vec![bytearray_fromhex(a)?]))));
                    }
                    return Ok(builtin("fromhex", move |a: Args| call(&k, Args::p(vec![bytes_fromhex(a)?]))));
                }
            }
            // int.from_bytes and float.fromhex on a subclass (an IntEnum, a float mixin Enum): CPython calls the
            // subclass with the value the base makes (cls(int.from_bytes(...)), cls(float.fromhex(s)))
            if c.base_bt() == BT::Int && name == "from_bytes" {
                let k = V::Class(c.clone());
                return Ok(builtin("from_bytes", move |a: Args| call(&k, Args::p(vec![int_from_bytes(&V::None, a)?]))));
            }
            if c.base_bt() == BT::Float && name == "fromhex" {
                let k = V::Class(c.clone());
                return Ok(builtin("fromhex", move |a: Args| call(&k, Args::p(vec![float_fromhex(a)?]))));
            }
            // A base's builtin_class_attr answers any name (a generic unbound method), so a base other
            // than object is asked only for a method that type really has.
            for k in c.mro.borrow().iter().skip(1) {
                // a dict or OrderedDict subclass: the methods CPython's type itself defines; fromkeys builds the subclass
                if (k.module == "builtins" && k.name == "dict" || k.module == "collections" && k.name == "OrderedDict") && cpy_type_own_method(k, name) {
                    if name == "fromkeys" {
                        return Ok(dict_fromkeys(c.clone(), k.clone()));
                    }
                    if let Some(v) = builtin_class_attr(k, name) {
                        return Ok(v);
                    }
                }
                if (k.module == "builtins" || k.module == "collections") && k.name != "object" && builtin_type_has(k.name, name) {
                    if let Some(v) = builtin_class_attr(k, name) {
                        return Ok(v);
                    }
                }
            }
            // Last: a plain class inherits object.__init__ (so Formatter.__init__(self) works as it
            // does in CPython); it is not among c.lookup's names because object adds no slot.
            // After the builtin bases, so ValueError.__init__ on an exception subclass stays ValueError's.
            // Every name of object's type dict alike (C.__hash__ is object.__hash__ for a class without __eq__).
            if name == "__init__" || object_type_has(name) {
                if let Some(v) = builtin_class_attr(&bclass("object"), name) {
                    return Ok(v);
                }
            }
            Err(exc("AttributeError", format!("type object '{}' has no attribute '{}'", c.name, name)))
        }
    }
}
pub fn getattr(o: &V, name: &str) -> R {
    match o {
        V::Obj(ob) => {
            // a class attribute named __dict__ (a property) wins over the object's own dict, as in CPython;
            // a class whose __slots__ leave no __dict__ has no instance dict to return
            if name == "__dict__" && ob.cls.lookup(name).is_none() && ob.cls.has_dict() {
                return ob.dict();
            }
            // the __weakref__ getset of the layout: a data descriptor, so it wins over an instance attribute
            if name == "__weakref__" && ob.cls.lookup(name).is_none() {
                return obj_weakref(o, &ob.cls);
            }
            // a data descriptor of the program (its class defines __set__ or __delete__) wins over the
            // instance's own attribute, as in CPython
            if USER_DATA_DESC.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(d) = ob.cls.lookup(name) {
                    if is_data_desc(&d) {
                        return desc_get(&d, o, &V::Class(ob.cls.clone()));
                    }
                }
            }
            if let Some(v) = ob.get_attr(name)? {
                return Ok(v);
            }
            // the class's __module__ (a str in its dict in CPython; Brevi keeps it beside the dict)
            if name == "__module__" && ob.cls.lookup(name).is_none() {
                return Ok(s(ob.cls.module));
            }
            // (An instance's __annotations__ is an ordinary lookup through its class's MRO, so a base's dict is
            // found and a class with none raises AttributeError, as in CPython; only the class itself is special.)
            if let Some(v) = ob.cls.lookup(name) {
                return match &v {
                    // a non-data descriptor of the program (its class defines __get__)
                    V::Obj(d) if d.cls.lookup("__get__").is_some() => desc_get(&v, o, &V::Class(ob.cls.clone())),
                    V::Func(f) => Ok(match f.kind.get() {
                        1 => v.clone(),
                        3 if nobind(f) => v.clone(),
                        2 => V::Bound(Rc::new((V::Class(ob.cls.clone()), v.clone()))),
                        _ => V::Bound(Rc::new((o.clone(), v.clone()))),
                    }),
                    V::Prop(p) => call(&p.0, Args::p(vec![o.clone()])),
                    // a namedtuple field descriptor: index the wrapped tuple directly, no Args and no call
                    V::NTF(n) => ntf_get(o, ob, n.idx),
                    _ => {
                        if let V::Ext(x) = &v {
                            if let Some(r) = x.descriptor(&v, o, &V::Class(ob.cls.clone())) {
                                return r;
                            }
                        }
                        Ok(v)
                    }
                };
            }
            // object's rich comparison slots, which every instance has (an instance wrapping a builtin value
            // gets that type's below): __eq__ is identity or NotImplemented, __ne__ inverts the instance's own
            // __eq__ (NotImplemented passes through), and the four orderings answer NotImplemented.
            if matches!(name, "__lt__" | "__le__" | "__gt__" | "__ge__" | "__eq__" | "__ne__") && ob.inner.borrow().is_none() {
                let me = o.clone();
                let n = sym(name);
                return Ok(builtin(n, move |a: Args| {
                    a.want(n, 1, 1)?;
                    let other = &a.p[0];
                    match n {
                        "__eq__" => Ok(if is(&me, other) { V::Bool(true) } else { V::NotImpl }),
                        "__ne__" => {
                            let r = match &me {
                                V::Obj(x) => match x.cls.lookup("__eq__") {
                                    Some(f) => call(&f, Args::p(vec![me.clone(), other.clone()]))?,
                                    None => if is(&me, other) { V::Bool(true) } else { V::NotImpl },
                                },
                                _ => V::NotImpl,
                            };
                            Ok(if matches!(r, V::NotImpl) { r } else { V::Bool(!truth(&r)?) })
                        }
                        _ => Ok(V::NotImpl),
                    }
                }));
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
                // OSError's constructor-derived fields; once deleted the slot reads None, as in CPython.
                "errno" | "strerror" | "filename" | "filename2" if ob.cls.base_bt() == BT::Exc && ob.cls.is_sub(&ose_class()) => {
                    return Ok(ob.get(name).unwrap_or(V::None));
                }
                // ImportError.name/path, AttributeError.name/obj and NameError.name, likewise.
                "path" if ob.cls.base_bt() == BT::Exc && ob.cls.is_sub(&exc_class("ImportError")) => {
                    return Ok(ob.get(name).unwrap_or(V::None));
                }
                "name" if ob.cls.base_bt() == BT::Exc
                    && (ob.cls.is_sub(&exc_class("ImportError")) || ob.cls.is_sub(&exc_class("AttributeError")) || ob.cls.is_sub(&exc_class("NameError"))) =>
                {
                    return Ok(ob.get(name).unwrap_or(V::None));
                }
                "obj" if ob.cls.base_bt() == BT::Exc && ob.cls.is_sub(&exc_class("AttributeError")) => {
                    return Ok(ob.get(name).unwrap_or(V::None));
                }
                _ => {}
            }
            if let Some(ga) = ob.cls.lookup("__getattr__") {
                return call(&ga, Args::p(vec![o.clone(), s(name)]));
            }
            if matches!(name, "update" | "subtract") && counter_inner(o).is_some() {
                return Ok(V::BMeth(Rc::new((o.clone(), sym(name)))));
            }
            // a bytearray subclass instance's __iadd__ and __imul__ change it and give back the instance itself
            if is_ba_sub(o) && matches!(name, "__iadd__" | "__imul__") {
                let (me, op) = (o.clone(), if name == "__iadd__" { "+" } else { "*" });
                return Ok(builtin(sym(name), move |a: Args| {
                    if a.p.len() != 1 || !a.k.is_empty() {
                        return type_err(format!("expected 1 argument, got {}", a.p.len() + a.k.len()));
                    }
                    bytearray_iop(&me, &a.p[0], op).unwrap_or_else(|| type_err("internal: bytearray in-place operation".into()))
                }));
            }
            if let Some(inner) = inner_of(o) {
                if is_date_sub(o) && matches!(name, "replace" | "__replace__" | "__add__" | "__radd__" | "__sub__" | "__rsub__") {
                    return Err(unsupported(&format!("{name} of an Enum member that wraps a date (CPython builds the Enum's class from the result)")));
                }
                // a date's year, month and day are attributes, its other names methods: the date answers
                if is_date_sub(o) {
                    return getattr(&inner, name);
                }
                if has_method(&inner, name) {
                    return Ok(V::BMeth(Rc::new((inner, sym(name)))));
                }
                // deque's maxlen, a data descriptor of the base class
                if let V::Deque(d) = &inner {
                    if name == "maxlen" {
                        return Ok(d.borrow().max.map_or(V::None, |m| V::Int(m as i64)));
                    }
                }
            }
            Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, name)))
        }
        V::Class(c) => {
            if c.meta.borrow().is_some() {
                return meta_getattr(o, c, name);
            }
            class_getattr(o, c, name)
        }
        V::Module(m) => {
            // a module's annotations (its annotated top-level names) are not kept
            if name == "__annotations__" {
                return Err(unsupported("a module's __annotations__"));
            }
            if let Some(f) = USER_MOD.with(|u| u.get()) {
                if let Some(r) = f(m, name) {
                    return r;
                }
            }
            module_attr(m, name)
        }
        V::Func(f) => match name {
            // one str object per name, as CPython's def keeps its interned __name__ and __qualname__ (so
            // `f.__name__ is f.__name__`, and update_wrapper's copies are the same object)
            "__name__" => Ok(s_sym(f.name.get())),
            "__qualname__" => Ok(s_sym(f.qualname.get())),
            "__module__" => Ok(f.attrs
                .borrow()
                .iter()
                .find(|(k, _)| *k == "__module__")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| s_sym(if f.kind.get() == 3 { "builtins" } else { "__main__" }))),
            // Brevi doesn't evaluate annotations (the typing pass refuses a written `.__annotations__` when the
            // program is built); one assigned to the def reads back, any other read is refused
            "__annotations__" => match f.attrs.borrow().iter().find(|(k, _)| *k == "__annotations__").map(|(_, v)| v.clone()) {
                Some(v) => Ok(v),
                None if f.kind.get() == 3 => Err(exc("AttributeError", "'builtin_function_or_method' object has no attribute '__annotations__'".to_string())),
                None => Err(unsupported("a def's __annotations__")),
            },
            "__type_params__" => match f.attrs.borrow().iter().find(|(k, _)| *k == "__type_params__").map(|(_, v)| v.clone()) {
                Some(v) => Ok(v),
                None if f.kind.get() == 3 => Err(exc("AttributeError", format!("'{}' object has no attribute '__type_params__'", if f.kind.get() == 3 { "builtin_function_or_method" } else { "function" }))),
                None => Ok(tup(vec![])),
            },
            "__call__" => Ok(o.clone()),
            // an assigned __code__ is kept in attrs; otherwise one is made from the def's flags
            "__code__" if f.attrs.borrow().iter().all(|(k, _)| *k != "__code__") => match f.code_flags.get() {
                0 => Err(exc("AttributeError", format!("'{}' object has no attribute '__code__'", if f.kind.get() == 3 { "builtin_function_or_method" } else { "function" }))),
                fl => Ok(match f.tcode.get() {
                    Some(tc) => trace_code_v(tc),
                    None => mk_code(f.name.get(), fl),
                }),
            },
            // the cells the function closes over: None with no free variables, a tuple of cell objects otherwise
            "__closure__" => {
                if f.kind.get() == 3 {
                    return Err(exc("AttributeError", "'builtin_function_or_method' object has no attribute '__closure__'".into()));
                }
                let mut cells = Vec::new();
                let rec = f.closure.borrow();
                // recorded only when the source names __closure__ (and not for a method needing the __class__ cell): an unrecorded one is refused, never guessed
                let Some(rec) = rec.as_ref() else {
                    return Err(unsupported("__closure__ of a function whose cells this build did not record (a method using super(), or a name computed at run time)"));
                };
                for c in rec.iter() {
                    match c.upgrade() {
                        Some(r) => cells.push(V::Cell(r)),
                        // the function's own FnBox holds every captured cell for the function's whole life
                        None => return Err(unsupported("a def's __closure__ after a captured cell was dropped")),
                    }
                }
                if cells.is_empty() { Ok(V::None) } else { Ok(tup(cells)) }
            },
            // the dict of attributes assigned to the function; slot-backed names stay out, as in CPython
            "__dict__" if f.kind.get() != 3 => Ok(V::Dict(func_dict_make(f)?)),
            // 0 function, 1 staticmethod, 2 classmethod (Brevi's pydantic reads it)
            "__bv_kind__" => Ok(V::Int(f.kind.get() as i64)),
            // CPython's def/staticmethod/classmethod method-wrapper; a functools.partial is not a descriptor
            "__get__" if f.kind.get() != 3 && f.attrs.borrow().iter().all(|(k, _)| *k != "__bv_partial__") => {
                let v = o.clone();
                Ok(builtin("__get__", move |mut a: Args| {
                    if !a.k.is_empty() {
                        return type_err("wrapper __get__() takes no keyword arguments".into());
                    }
                    if a.p.is_empty() {
                        return type_err("expected at least 1 argument, got 0".into());
                    }
                    if a.p.len() > 2 {
                        return type_err(format!("expected at most 2 arguments, got {}", a.p.len()));
                    }
                    let obj = a.p.remove(0);
                    let cls = a.p.pop().unwrap_or(V::None);
                    func_get(&v, obj, cls)
                }))
            }
            "__doc__" => Ok(match f.attrs.borrow().iter().find(|(k, _)| *k == "__doc__") {
                Some((_, v)) => v.clone(),
                // a builtin of the builtins module (a module's function carries its __module__): CPython's text
                None if f.kind.get() == 3 && !f.attrs.borrow().iter().any(|(k, _)| *k == "__module__" || *k == "__bv_mod__") => {
                    builtin_doc(&f.name.get()).map_or(V::None, |d| s(d))
                }
                None => V::None,
            }),
            // a staticmethod or classmethod answers for the function it wraps (False when that is not abstract)
            "__isabstractmethod__" if matches!(f.kind.get(), 1 | 2) => Ok(func_user_attr(f, name)?.unwrap_or(V::Bool(false))),
            _ => {
                if let Some(v) = func_user_attr(f, name)? {
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
        V::Cell(c) if name == "cell_contents" => {
            let v = {
                let b = c.borrow();
                match &*b {
                    V::Unbound => return Err(exc("ValueError", "Cell is empty".into())),
                    v => v.clone(),
                }
            };
            Ok(v)
        }
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
        // property.__isabstractmethod__: whether its getter or setter is abstract
        V::Prop(p) if name == "__isabstractmethod__" => Ok(V::Bool(is_abstract_method(&p.0)? || is_abstract_method(&p.1)?)),
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
        // a namedtuple field descriptor: the one attribute it carries is its own __doc__, as in CPython
        V::NTF(n) if name == "__doc__" => {
            let d = n.doc.borrow().clone();
            // after `del P.x.__doc__` the descriptor's getter reads None, as CPython's does
            Ok(match d { V::Unbound => V::None, d => d })
        }
        V::NTF(_) => Err(exc("AttributeError", format!("'collections._tuplegetter' object has no attribute '{}'", name))),
        V::View(v) if name == "mapping" && v.1 != 3 => match &v.0 {
            V::Dict(_) => Ok(V::View(Rc::new((v.0.clone(), 3)))),
            _ => Err(exc("AttributeError", format!("'{}' object has no attribute 'mapping'", tn(o)))),
        },
        V::View(v) if v.1 == 3 && matches!(name, "get" | "keys" | "values" | "items" | "copy" | "__contains__" | "__getitem__") => {
            Ok(V::BMeth(Rc::new((o.clone(), sym(name)))))
        }
        V::Dict(d) if name == "default_factory" => Ok(d.borrow().dflt.clone().unwrap_or(V::None)),
        V::Deque(d) if name == "maxlen" => Ok(d.borrow().max.map_or(V::None, |m| V::Int(m as i64))),
        V::Iter(it) if name == "__qualname__" || name == "__name__" => {
            // only generators and genexprs carry these; the cells are held while the generator body runs,
            // so read them only when free (the running generator's name is remembered elsewhere)
            let qn = it.it.try_borrow().ok().and_then(|i| match &*i {
                It::Coro(c) => c.try_borrow().ok().map(|c| c.name),
                It::GenExp(_, n, _) => Some(*n),
                _ => None,
            })
            .or_else(running_gen_name);
            match qn {
                Some(qn) if name == "__qualname__" => Ok(s(qn)),
                // __name__ is the code object's co_name: the qualname's last component (genexprs: <genexpr>)
                Some(qn) => Ok(s(qn.rsplit('.').next().unwrap_or(qn))),
                None => Err(exc("AttributeError", format!("'{}' object has no attribute '{name}'", tn(o)))),
            }
        }
        // an OrderedDict's instance __dict__, and the attributes set in it
        V::Dict(d) if d.borrow().kind == 3 && name == "__dict__" => Ok(V::Dict(odict_attrs(d))),
        V::Dict(d) if d.borrow().kind == 3 && d.borrow().attrs.as_ref().map_or(false, |x| Dict::contains_cell(x, &s(name)).unwrap_or(false)) => {
            let x = odict_attrs(d);
            Ok(Dict::get_cell(&x, &s(name))?.unwrap_or(V::None))
        }
        // every value's __class__ is its type (has_method would say yes and give a bound method)
        _ if name == "__class__" => Ok(V::Class(type_of(o)?)),
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
            match ob.cls.lookup(name) {
                Some(V::NTF(_)) => return Err(exc("AttributeError", "can't set attribute".into())),
                Some(V::Prop(p)) => {
                    if matches!(p.1, V::None) {
                        return Err(exc("AttributeError", format!("property '{}' of '{}' object has no setter", name, ob.cls.name)));
                    }
                    call(&p.1, Args::p(vec![o.clone(), v]))?;
                    return Ok(());
                }
                // a data descriptor of the program: __set__(obj, value); one with only __delete__ cannot be set
                Some(d @ V::Obj(_)) if is_data_desc(&d) => {
                    let V::Obj(dd) = &d else { unreachable!() };
                    return match dd.cls.lookup("__set__") {
                        Some(f) => call(&f, Args::p(vec![d.clone(), o.clone(), v])).map(|_| ()),
                        None => Err(exc("AttributeError", "__set__".into())),
                    };
                }
                _ => {}
            }
            if let Some(sa) = ob.cls.lookup("__setattr__") {
                call(&sa, Args::p(vec![o.clone(), s(name), v]))?;
                return Ok(());
            }
            if let Some(V::Tuple(slots)) = ob.cls.lookup("__frozen__") {
                let _ = slots;
                return Err(exc("FrozenInstanceError", format!("cannot assign to field '{name}'")));
            }
            if !ob.cls.may_store(name) {
                if ob.cls.lookup(name).is_some() {
                    return Err(exc("AttributeError", format!("'{}' object attribute '{}' is read-only", ob.cls.name, name)));
                }
                return Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", ob.cls.name, name)));
            }
            ob.set_res(name, v)?;
            Ok(())
        }
        V::Class(c) => {
            if let Some(e) = enum_setattr_block(c, &name.to_string()) {
                return Err(e);
            }
            if let Some(r) = meta_setattr(o, c, name, &v) {
                return r;
            }
            if name == "__set__" || name == "__delete__" {
                USER_DATA_DESC.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            c.set_exact(name, v);
            Ok(())
        }
        // each namedtuple field descriptor keeps its own __doc__; nothing else may be set, as in CPython
        V::NTF(n) if name == "__doc__" => {
            *n.doc.borrow_mut() = v;
            Ok(())
        }
        V::NTF(_) => Err(exc("AttributeError", format!("'collections._tuplegetter' object has no attribute '{}' and no __dict__ for setting new attributes", name))),
        V::Module(&ModName("decimal.context")) => decimal_context_set(name, &v),
        V::Module(m) if mod_assignable(m.0, name) => {
            mod_assign(m.0, &name.to_string(), Some(v));
            Ok(())
        }
        V::Module(&ModName("sys")) if matches!(&*name.to_string(), "stdout" | "stderr" | "stdin") => sys_stream_assign(&name.to_string(), v),
        V::Ext(x) => match x.set_attr(o, name, v) {
            Some(r) => r,
            None => Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", tn(o), name))),
        },
        V::Func(f) => {
            // the closure tuple is made from the def's captured cells, as in CPython it is read-only
            if name == "__closure__" {
                return Err(exc("AttributeError", "readonly attribute".into()));
            }
            // a def's name cells are what its repr and errors show, so assignments go there, as in CPython
            if name == "__name__" || name == "__qualname__" {
                let V::Str(t) = &v else {
                    return type_err(format!("{name} must be set to a string object"));
                };
                if name == "__name__" { f.name.set(sym(&t.text())) } else { f.qualname.set(sym(&t.text())) }
                return Ok(());
            }
            if !FUNC_SLOT_ATTRS.contains(&name) {
                if let Some(d) = func_dict(f) {
                    return Dict::set_cell(&d, s(name), v);
                }
            }
            let mut a = f.attrs.borrow_mut();
            match a.iter_mut().find(|(k, _)| *k == name) {
                Some(e) => e.1 = v,
                None => a.push((name, v)),
            }
            Ok(())
        }
        V::Dict(d) if d.borrow().kind == 3 && &*name.to_string() != "__class__" => {
            if &*name.to_string() == "__dict__" {
                let V::Dict(nd) = &v else {
                    return type_err(format!("__dict__ must be set to a dictionary, not a '{}'", tn(&v)));
                };
                d.borrow_mut().attrs = Some(nd.clone());
                return Ok(());
            }
            Dict::set_cell(&odict_attrs(d), s(name), v)
        }
        _ => Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", tn(o), name))),
    }
}
// object.__delattr__: deleting an instance attribute without the class's own __delattr__
pub fn object_delattr(o: &V, name: &str) -> Result<(), E> {
    let V::Obj(ob) = o else {
        return Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", tn(o), name)));
    };
    // a C type's __dict__ descriptor (functools.partial's) cannot be deleted
    if name == "__dict__" && ob.cls.lookup("__bv_nodeldict__").is_some() {
        return type_err("cannot delete __dict__".into());
    }
    // deleting an OSError field empties the slot (getattr then reads None), it does not raise
    if matches!(name, "errno" | "strerror" | "filename" | "filename2")
        && ob.cls.base_bt() == BT::Exc
        && ob.cls.is_sub(&ose_class())
    {
        ob.del_attr(name)?;
        return Ok(());
    }
    if (matches!(name, "path") && ob.cls.is_sub(&exc_class("ImportError"))
        || matches!(name, "obj") && ob.cls.is_sub(&exc_class("AttributeError"))
        || matches!(name, "name")
            && (ob.cls.is_sub(&exc_class("ImportError"))
                || ob.cls.is_sub(&exc_class("AttributeError"))
                || ob.cls.is_sub(&exc_class("NameError"))))
        && ob.cls.base_bt() == BT::Exc
    {
        ob.del_attr(name)?;
        return Ok(());
    }
    // a data descriptor of the program: __delete__(obj); one with only __set__ cannot be deleted
    if USER_DATA_DESC.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(d @ V::Obj(_)) = ob.cls.lookup(name) {
            if is_data_desc(&d) {
                let V::Obj(dd) = &d else { unreachable!() };
                return match dd.cls.lookup("__delete__") {
                    Some(f) => call(&f, Args::p(vec![d.clone(), o.clone()])).map(|_| ()),
                    None => Err(exc("AttributeError", "__delete__".into())),
                };
            }
        }
    }
    if ob.del_attr(name)? {
        Ok(())
    } else if !ob.cls.has_dict() {
        if ob.cls.lookup(name).is_some() {
            Err(exc("AttributeError", format!("'{}' object attribute '{}' is read-only", ob.cls.name, name)))
        } else {
            Err(exc("AttributeError", format!("'{}' object has no attribute '{}' and no __dict__ for setting new attributes", ob.cls.name, name)))
        }
    } else {
        Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, name)))
    }
}
pub fn delattr(o: &V, name: &str) -> Result<(), E> {
    if let V::Dict(d) = o {
        if d.borrow().kind == 3 {
            if Dict::del_cell(&odict_attrs(d), &s(name))?.is_none() {
                return Err(exc("AttributeError", format!("'collections.OrderedDict' object has no attribute '{name}'")));
            }
            return Ok(());
        }
    }
    if let Some(p) = proxy_deref(o) {
        return delattr(&p?, name);
    }
    if let V::Module(m) = o {
        if mod_assignable(m.0, name) {
            if matches!(mod_assigned(m.0, name), Some(None)) {
                return Err(exc("AttributeError", format!("module '{}' has no attribute '{name}'", m.0)));
            }
            mod_assign(m.0, name, None);
            return Ok(());
        }
    }
    match o {
        V::Obj(ob) => {
            if let Some(da) = ob.cls.lookup("__delattr__") {
                call(&da, Args::p(vec![o.clone(), s(name)]))?;
                return Ok(());
            }
            object_delattr(o, name)
        }
        // deleting a namedtuple field descriptor's __doc__ resets it to None, as in CPython
        V::NTF(n) if name == "__doc__" => {
            // Unbound keeps a deleted doc apart from an explicitly set None (repr shows <NULL>)
            *n.doc.borrow_mut() = V::Unbound;
            Ok(())
        }
        V::NTF(_) => Err(exc("AttributeError", format!("'collections._tuplegetter' object has no attribute '{}'", name))),
        // a metaclass's data descriptor would be asked first (CPython's type.__delattr__); not made here
        V::Class(c) if c.meta.borrow().as_ref().is_some_and(|m| meta_lookup(m, name).is_some_and(|d| matches!(d, V::Prop(_)) || is_data_desc(&d))) => {
            Err(unsupported("deleting a class attribute its metaclass defines as a descriptor"))
        }
        V::Class(c) if enum_delattr_block(c, name).is_some() => Err(enum_delattr_block(c, name).unwrap()),
        V::Class(c) if c.del_exact(name) => Ok(()),
        V::Class(c) => Err(exc("AttributeError", format!("type object '{}' has no attribute '{}'", c.name, name))),
        // a def's user attribute: from its __dict__ once that has been made, else from its attrs (as setattr stores it)
        V::Func(f) if !FUNC_SLOT_ATTRS.contains(&name) => {
            let gone = match func_dict(f) {
                Some(d) => Dict::del_cell(&d, &s(name))?.is_some(),
                None => {
                    let mut a = f.attrs.borrow_mut();
                    let n = a.len();
                    a.retain(|(k, _)| *k != name);
                    a.len() != n
                }
            };
            if gone { Ok(()) } else { Err(exc("AttributeError", format!("'function' object has no attribute '{}'", name))) }
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
// o.name(x): the commonest calls on builtin containers without building Args
#[inline]
pub fn call_method1(o: &V, name: &str, x: V) -> R {
    match o {
        V::List(l) if name == "append" => {
            l.borrow_mut().push(x);
            Ok(V::None)
        }
        V::Set(st) if name == "add" => {
            PSet::cell_add(st, x)?;
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
                PSet::cell_add(st, a.p.pop().unwrap())?;
                return Ok(V::None);
            }
            _ => {}
        }
    }
    match o {
        V::Obj(ob) => {
            // obj.name(args) is obj.name, then a call: a data descriptor of the program wins over the instance's
            // own attribute, and a non-data one is asked after it (see getattr)
            if USER_DATA_DESC.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(d) = ob.cls.lookup(name) {
                    if is_data_desc(&d) {
                        let f = desc_get(&d, o, &V::Class(ob.cls.clone()))?;
                        return call(&f, a);
                    }
                }
            }
            if let Some(v) = ob.get_attr(name)? {
                return call(&v, a);
            }
            if let Some(v) = ob.cls.lookup(name) {
                return match &v {
                    V::Obj(d) if d.cls.lookup("__get__").is_some() => {
                        let f = desc_get(&v, o, &V::Class(ob.cls.clone()))?;
                        call(&f, a)
                    }
                    V::Func(f) => match f.kind.get() {
                        1 => call(&v, a),
                        3 if nobind(f) => call(&v, a),
                        2 => {
                            let mut a = a;
                            prepend(&mut a.p, V::Class(ob.cls.clone()));
                            call(&v, a)
                        }
                        _ => call_self(&v, o.clone(), a),
                    },
                    V::Prop(p) => call(&call(&p.0, Args::p(vec![o.clone()]))?, a),
                    V::NTF(_) => {
                        // p.x(args): the field's value is called (CPython evaluates the attribute, then calls it)
                        let f = getattr(o, name)?;
                        call(&f, a)
                    }
                    _ => {
                        if let V::Ext(x) = &v {
                            if let Some(r) = x.descriptor(&v, o, &V::Class(ob.cls.clone())) {
                                return call(&r?, a);
                            }
                        }
                        call(&v, a)
                    }
                };
            }
            // A dict subclass's copy() returns the subclass (a Counter's too). The class defines no copy of its
            // own here; the wrapped builtin value is copied into a new instance of the same class.
            if name == "copy" && ob.cls.own(name).is_none() {
                if odict_of(o).is_some() {
                    return odict_obj_copy(o, &V::Class(ob.cls.clone()));
                }
                if let Some(V::Dict(d)) = inner_of(o) {
                    let inner = V::Dict(Rc::new(RefCell::new(d.borrow().copy())));
                    let no = Obj::new(ob.cls.clone());
                    no.set_inner(Some(inner));
                    return Ok(V::Obj(no));
                }
            }
            // a deque subclass's copy() and __copy__() return the subclass: type(d)(d[, maxlen])
            if matches!(name, "copy" | "__copy__") && ob.cls.own(name).is_none() {
                if let Some(r) = deque_sub_copy(o) {
                    return r;
                }
            }
            if name == "__reduce__" && odict_of(o).is_some() {
                return odict_reduce(o);
            }
            if matches!(name, "update" | "subtract" | "__init__") && counter_inner(o).is_some() {
                return if name == "__init__" { counter_obj_init(o, a) } else { counter_obj_method(o, name, a) };
            }
            // a bytearray subclass instance's __iadd__ and __imul__ give back the instance itself (getattr has the same)
            if is_ba_sub(o) && matches!(name, "__iadd__" | "__imul__") {
                return call(&getattr(o, name)?, a);
            }
            if let Some(inner) = inner_of(o) {
                if is_date_sub(o) && matches!(name, "replace" | "__replace__" | "__add__" | "__radd__" | "__sub__" | "__rsub__") {
                    return Err(unsupported(&format!("{name} of an Enum member that wraps a date (CPython builds the Enum's class from the result)")));
                }
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
                    // a descriptor of the program read from the class: __get__(None, cls), then the call
                    V::Obj(d) if d.cls.lookup("__get__").is_some() => {
                        let f = desc_get(&v, &V::None, o)?;
                        call(&f, a)
                    }
                    _ => {
                        if let V::Ext(x) = &v {
                            if let Some(r) = x.descriptor(&v, &V::None, o) {
                                return call(&r?, a);
                            }
                        }
                        call(&v, a)
                    }
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
        _ => {
            if let Some(k) = kw_dup(&a.k) {
                return type_err(format!("{}.{}() got multiple values for keyword argument '{}'", owner_name(o), name, k));
            }
            builtin_method(o, name, a)
        }
    }
}

// Whether instances of the builtin type `tname` have method `name` (for a subclass reading a base's method
// through the class, as P.__getitem__ on a namedtuple class).
fn builtin_type_has(tname: &str, name: &str) -> bool {
    let sample = match tname {
        "tuple" => tup(vec![]),
        "list" => V::List(Rc::new(RefCell::new(vec![]))),
        "str" => s(""),
        "int" => V::Int(0),
        _ => return false,
    };
    has_method(&sample, name)
        || matches!(name, "__getitem__" | "__contains__" | "__iter__" | "__len__" | "__add__" | "__mul__" | "__rmul__" | "__hash__" | "__repr__" | "__eq__" | "__lt__" | "__le__" | "__gt__" | "__ge__")
}

// ---------- metaclasses ----------
// A program's subclass of type (`class M(type)`) makes classes: M(name, bases, ns), `class C(metaclass=M)`, and
// every class built on a class M made. What M may define is what this runtime asks it for: __new__, __init__ and
// __prepare__ (a dict only), the comparisons, __hash__, __len__, __bool__, __repr__ and __str__, and any plain
// method, property or descriptor, which its classes find as CPython's type.__getattribute__ does. Any other
// special method (__call__, __getattr__, __instancecheck__, __or__, ...) is refused when the metaclass is made.
pub fn is_type_class(k: &Rc<Class>) -> bool {
    k.module == "builtins" && k.name == "type"
}
pub fn is_metaclass(c: &Rc<Class>) -> bool {
    !is_type_class(c) && c.mro.borrow().iter().any(is_type_class)
}
const META_DUNDERS: &[&str] = &[
    "__module__", "__qualname__", "__doc__", "__new__", "__init__", "__prepare__", "__lt__", "__le__", "__gt__", "__ge__", "__eq__", "__ne__",
    "__hash__", "__len__", "__bool__", "__repr__", "__str__", "__annotations__", "__init_subclass__", "__weakref__", "__dict__",
];
pub fn meta_dunder_ok(n: &str) -> bool {
    !(n.len() > 4 && n.starts_with("__") && n.ends_with("__")) || META_DUNDERS.contains(&n)
}
fn is_abc_meta_sub(c: &Rc<Class>) -> bool {
    c.mro.borrow().iter().any(|k| k.module == "abc" && k.name == "ABCMeta")
}
fn meta_class_check(c: &Rc<Class>) -> Result<(), E> {
    // a subclass of abc.ABCMeta may be defined and read; making a class with it is refused (meta_call, type_new)
    if is_abc_meta_sub(c) {
        return Ok(());
    }
    if c.meta.borrow().is_some() {
        return Err(unsupported("a metaclass whose own type is a metaclass"));
    }
    for (n, _) in c.attrs.borrow().iter() {
        if !meta_dunder_ok(n) && !n.starts_with("__bv_") {
            return Err(unsupported(&format!("a metaclass defining {n}")));
        }
    }
    Ok(())
}
thread_local! {
    // the field names of the class statement whose metaclass is being called (type.__new__ takes them)
    static PENDING_FIELDS: RefCell<Option<Vec<Sym>>> = const { RefCell::new(None) };
}
// A name in a metaclass's own classes (type's and object's slots are this runtime's).
pub fn meta_lookup(m: &Rc<Class>, n: &str) -> Option<V> {
    for k in m.mro.borrow().iter() {
        if k.module == "builtins" {
            continue;
        }
        if let Some(v) = k.own(n) {
            return Some(v);
        }
    }
    None
}
pub fn meta_has(c: &Rc<Class>, n: &str) -> bool {
    c.meta.borrow().as_ref().is_some_and(|m| meta_lookup(m, n).is_some())
}
fn meta_method(c: &Rc<Class>, n: &str) -> Option<(Rc<Class>, V)> {
    let m = c.meta.borrow().clone()?;
    let f = meta_lookup(&m, n)?;
    Some((m, f))
}
// A metaclass's function bound to one of its classes: a classmethod binds to the metaclass, a staticmethod not at all.
fn bind_meta(v: V, me: &V, m: &Rc<Class>) -> V {
    match &v {
        V::Func(f) if f.kind.get() == 2 => V::Bound(Rc::new((V::Class(m.clone()), v.clone()))),
        _ => bind_attr(v, me, true),
    }
}
// What a class gets from its metaclass for `name`: a property's getter, a descriptor's __get__(cls, M), a bound method.
fn meta_desc(d: &V, o: &V, m: &Rc<Class>, name: &str) -> R {
    match d {
        V::Prop(p) => {
            if matches!(p.0, V::None) {
                return Err(exc("AttributeError", format!("property '{name}' of '{}' object has no getter", m.name)));
            }
            call(&p.0, Args::p(vec![o.clone()]))
        }
        V::Obj(x) if x.cls.lookup("__get__").is_some() => desc_get(d, o, &V::Class(m.clone())),
        V::Ext(x) => match x.descriptor(d, o, &V::Class(m.clone())) {
            Some(r) => r,
            None => Ok(d.clone()),
        },
        V::Func(_) => Ok(bind_meta(d.clone(), o, m)),
        _ => Ok(d.clone()),
    }
}
fn meta_getattr(o: &V, c: &Rc<Class>, name: &str) -> R {
    let m = c.meta.borrow().clone().unwrap();
    if name == "__class__" {
        return Ok(V::Class(m));
    }
    let mv = meta_lookup(&m, name);
    // a data descriptor of the metaclass wins over the class's own attribute
    if let Some(d) = &mv {
        if matches!(d, V::Prop(_)) || is_data_desc(d) {
            return meta_desc(d, o, &m, name);
        }
    }
    match class_getattr(o, c, name) {
        Err(e) if is_exc(&e.0, "AttributeError") && c.lookup(name).is_none() => match &mv {
            Some(d) => meta_desc(d, o, &m, name),
            None => Err(e),
        },
        r => r,
    }
}
// `C.name = v` for a class made by a metaclass (its property's setter, a descriptor's __set__), and the special
// methods a metaclass may not get later.
fn meta_setattr(o: &V, c: &Rc<Class>, name: &str, v: &V) -> Option<Result<(), E>> {
    if is_metaclass(c) && !meta_dunder_ok(name) {
        return Some(Err(unsupported(&format!("setting {name} on a metaclass"))));
    }
    let m = c.meta.borrow().clone()?;
    let d = meta_lookup(&m, name)?;
    match &d {
        V::Prop(p) => Some(if matches!(p.1, V::None) {
            Err(exc("AttributeError", format!("property '{name}' of '{}' object has no setter", m.name)))
        } else {
            call(&p.1, Args::p(vec![o.clone(), v.clone()])).map(|_| ())
        }),
        V::Obj(x) if is_data_desc(&d) => Some(match x.cls.lookup("__set__") {
            Some(f) => call(&f, Args::p(vec![d.clone(), o.clone(), v.clone()])).map(|_| ()),
            None => Err(exc("AttributeError", "__set__".into())),
        }),
        _ => None,
    }
}
// The most derived metaclass among the bases' types, when one is a program's metaclass (None: all are type).
fn bases_meta(bases: &[V]) -> Result<Option<Rc<Class>>, E> {
    let mut w: Option<Rc<Class>> = None;
    let mut abc = false;
    for b in bases {
        if let V::Class(k) = b {
            abc |= k.abc.get();
            if let Some(m) = k.meta.borrow().clone() {
                w = Some(match w {
                    None => m,
                    Some(x) if m.is_sub(&x) => m,
                    Some(x) if x.is_sub(&m) => x,
                    Some(_) => return Err(meta_conflict()),
                });
            }
        }
    }
    if abc && w.is_some() {
        return Err(meta_conflict());
    }
    Ok(w)
}
fn meta_conflict() -> E {
    exc("TypeError", "metaclass conflict: the metaclass of a derived class must be a (non-strict) subclass of the metaclasses of all its bases".into())
}
// type.__new__(mcls, name, bases, ns): the class, with mcls (or the most derived metaclass of the bases) as its type.
pub fn type_new(mcls: &Rc<Class>, a: Args) -> R {
    if a.p.len() != 3 {
        return type_err(format!("type.__new__() takes exactly 3 arguments ({} given)", a.p.len()));
    }
    if !a.k.is_empty() {
        return Err(unsupported("keyword arguments to type.__new__ (for __init_subclass__)"));
    }
    let name = match &a.p[0] {
        V::Str(x) => sym(&x.text()),
        x => return type_err(format!("type.__new__() argument 1 must be str, not {}", tn(x))),
    };
    let bases = match &a.p[1] {
        V::Tuple(t) => t.to_vec(),
        x => return type_err(format!("type.__new__() argument 2 must be tuple, not {}", tn(x))),
    };
    let ns = match &a.p[2] {
        V::Dict(d) if d.borrow().kind == 0 => a.p[2].clone(),
        V::Dict(_) => return Err(unsupported("a dict subclass as type.__new__'s namespace")),
        x => return type_err(format!("type.__new__() argument 3 must be dict, not {}", tn(x))),
    };
    let mut winner = mcls.clone();
    if is_abc_meta_sub(mcls) {
        return Err(unsupported("a class made by a subclass of abc.ABCMeta"));
    }
    if let Some(bm) = bases_meta(&bases)? {
        if bm.is_sub(&winner) {
            winner = bm;
        } else if !winner.is_sub(&bm) {
            return Err(meta_conflict());
        }
    } else if bases.iter().any(|b| matches!(b, V::Class(k) if k.abc.get())) && !is_type_class(&winner) {
        return Err(meta_conflict());
    }
    if !Rc::ptr_eq(&winner, mcls) && meta_lookup(&winner, "__new__").is_some() {
        return Err(unsupported("type.__new__ handing a class to a more derived metaclass's own __new__"));
    }
    let mut qualname = name;
    let mut module: Sym = "__main__";
    let mut attrs = vec![];
    for (k, v) in dict_sym_items(&ns)? {
        match k {
            "__qualname__" => match &v {
                V::Str(x) => qualname = sym(&x.text()),
                x => return type_err(format!("type __qualname__ must be a str, not {}", tn(x))),
            },
            // kept in the dict where the namespace has it, as type_new keeps it
            "__module__" => match &v {
                V::Str(x) => {
                    module = sym(&x.text());
                    attrs.push((k, v));
                }
                _ => return Err(unsupported("a class __module__ that is not a str")),
            },
            "__classcell__" => {}
            _ => attrs.push((k, v)),
        }
    }
    let fields = PENDING_FIELDS.with(|p| p.borrow_mut().take()).unwrap_or_default();
    let meta = if is_type_class(&winner) { None } else { Some(winner) };
    make_class_impl(name, qualname, module, bases, attrs, fields, None, meta)
}
// Calling a metaclass: M.__new__ (its own, or type's), then M.__init__ when the result is an instance of M.
pub fn meta_call(m: &Rc<Class>, a: Args) -> R {
    if is_abc_meta_sub(m) {
        return Err(unsupported("a class made by a subclass of abc.ABCMeta"));
    }
    if m.meta.borrow().is_some() {
        return Err(unsupported("a metaclass whose own type is a metaclass"));
    }
    let mv = V::Class(m.clone());
    let r = match meta_lookup(m, "__new__") {
        Some(f) => {
            let mut a2 = Args { p: a.p.clone(), k: a.k.clone() };
            a2.p.insert(0, mv.clone());
            call(&f, a2)?
        }
        None => type_new(m, Args { p: a.p.clone(), k: a.k.clone() })?,
    };
    if let V::Class(k) = &r {
        if k.meta.borrow().as_ref().is_some_and(|x| x.is_sub(m)) {
            if let Some(init) = meta_lookup(m, "__init__") {
                let x = call(&bind_meta(init, &r, m), a)?;
                if !matches!(x, V::None) {
                    return type_err(format!("__init__() should return None, not '{}'", tn(&x)));
                }
            }
        }
    }
    Ok(r)
}
// A class statement's namespace with `metaclass=M`: M.__prepare__(name, bases) when M defines it (a plain dict,
// holding at most __annotations__: the class body here reads its other names from the body itself).
pub fn meta_prepare(m: &V, name: &str, bases: &V) -> R {
    let ns = match m {
        V::Class(k) if is_metaclass(k) => {
            let w = match bases_meta(&to_vec(bases)?)? {
                Some(b) if b.is_sub(k) => b,
                Some(b) if !k.is_sub(&b) => return Err(meta_conflict()),
                _ => k.clone(),
            };
            match meta_lookup(&w, "__prepare__") {
                Some(f) => call(&bind_meta(f, &V::Class(w.clone()), &w), Args::p(vec![s(name), bases.clone()]))?,
                None => dict_new(),
            }
        }
        _ => dict_new(),
    };
    match &ns {
        V::Dict(d) if d.borrow().kind == 0 => {
            if dict_sym_items(&ns)?.iter().any(|(k, _)| *k != "__annotations__") {
                return Err(unsupported("a metaclass __prepare__ whose namespace starts with names other than __annotations__"));
            }
        }
        _ => return Err(unsupported("a metaclass __prepare__ that returns something other than a dict")),
    }
    Ok(ns)
}
// CPython's SETUP_ANNOTATIONS on a class namespace dict: the __annotations__ already there, else a new dict.
pub fn ns_annotations(ns: &V) -> R {
    let k = s("__annotations__");
    if let V::Dict(d) = ns {
        if let Some(v) = Dict::get_cell(d, &k)? {
            return Ok(v);
        }
    }
    let d = dict_new();
    setitem(ns, &k, d.clone())?;
    Ok(d)
}
// The end of a class statement with `metaclass=M`: M(name, bases, ns) (type itself: an ordinary class).
pub fn meta_build(m: &V, name: Sym, qualname: Sym, module: Sym, bases: Vec<V>, ns: V, fields: Vec<Sym>) -> R {
    match m {
        V::Class(k) if is_type_class(k) => make_class_f(name, qualname, module, bases, dict_sym_items(&ns)?.into_iter().filter(|(k, _)| !matches!(*k, "__module__" | "__qualname__")).collect(), fields),
        V::Class(k) if is_metaclass(k) => {
            PENDING_FIELDS.with(|p| *p.borrow_mut() = Some(fields));
            let r = meta_call(k, Args::p(vec![s(name), tup(bases), ns]));
            PENDING_FIELDS.with(|p| *p.borrow_mut() = None);
            r
        }
        V::Class(_) => Err(unsupported("a class that is not a subclass of type as a metaclass")),
        f => call(f, Args::p(vec![s(name), tup(bases), ns])),
    }
}
fn meta_truth(c: &Rc<Class>, v: &V) -> Result<bool, E> {
    if let Some((m, f)) = meta_method(c, "__bool__") {
        return match call(&bind_meta(f, v, &m), Args::p(vec![]))? {
            V::Bool(b) => Ok(b),
            x => type_err(format!("__bool__ should return bool, returned {}", tn(&x))),
        };
    }
    if meta_has(c, "__len__") {
        return Ok(meta_len(c, v)? > 0);
    }
    Ok(true)
}
fn meta_len(c: &Rc<Class>, v: &V) -> Result<usize, E> {
    let Some((m, f)) = meta_method(c, "__len__") else {
        return type_err(format!("object of type '{}' has no len()", tn(v)));
    };
    match call(&bind_meta(f, v, &m), Args::p(vec![]))? {
        V::Bool(b) => Ok(b as usize),
        V::Int(i) if i >= 0 => Ok(i as usize),
        V::Int(_) => val_err("__len__() should return >= 0".into()),
        V::Big(b) if b.neg => val_err("__len__() should return >= 0".into()),
        V::Big(_) => Err(exc("OverflowError", "cannot fit 'int' into an index-sized integer".into())),
        x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(&x))),
    }
}
fn meta_hash(c: &Rc<Class>, v: &V) -> Result<i64, E> {
    match meta_method(c, "__hash__") {
        Some((m, V::None)) => type_err(format!("unhashable type: '{}'", m.name)),
        Some((m, f)) => match call(&bind_meta(f, v, &m), Args::p(vec![]))? {
            V::Int(-1) => Ok(-2),
            V::Int(i) => Ok(i),
            V::Bool(b) => Ok(b as i64),
            x @ V::Big(_) => py_hash(&x),
            _ => type_err("__hash__ method should return an integer".into()),
        },
        None => Ok((Rc::as_ptr(c) as usize >> 4) as i64),
    }
}
fn meta_str(c: &Rc<Class>, v: &V, which: &str) -> Result<String, E> {
    let (m, f) = meta_method(c, which).unwrap();
    match call(&bind_meta(f, v, &m), Args::p(vec![]))? {
        V::Str(x) => Ok(x.text().to_string()),
        x => type_err(format!("{which} returned non-string (type {})", tn(&x))),
    }
}
// `a OP b` when a or b is a class a program's metaclass made: the metaclass's comparison methods, in CPython's
// order (the reflected one first when type(b) is a strict subclass of type(a) that has it), with object's
// defaults (== by identity, != inverting __eq__, the orderings a TypeError). None: neither side is such a class.
pub fn meta_cmp(a: &V, b: &V, op: &str) -> Option<R> {
    let is_mc = |x: &V| matches!(x, V::Class(k) if k.meta.borrow().is_some());
    if !is_mc(a) && !is_mc(b) {
        return None;
    }
    let slot = |x: &V, n: &str| -> Option<V> {
        match x {
            V::Class(k) => {
                let m = k.meta.borrow().clone()?;
                meta_lookup(&m, n).map(|f| bind_meta(f, x, &m))
            }
            V::Obj(o) => o.cls.lookup(n).map(|f| V::Bound(Rc::new((x.clone(), f)))),
            _ => None,
        }
    };
    let (l, r) = match op {
        "==" => ("__eq__", "__eq__"),
        "!=" => ("__ne__", "__ne__"),
        "<" => ("__lt__", "__gt__"),
        ">" => ("__gt__", "__lt__"),
        "<=" => ("__le__", "__ge__"),
        ">=" => ("__ge__", "__le__"),
        _ => return None,
    };
    // one side's method: Some(result) unless it answers NotImplemented. A missing __ne__ is object's: the
    // side's own __eq__, inverted.
    let side = |x: &V, y: &V, n: &str| -> Option<R> {
        let f = match slot(x, n) {
            Some(f) => f,
            None if n == "__ne__" => {
                let f = slot(x, "__eq__")?;
                return match call(&f, Args::p(vec![y.clone()])) {
                    Ok(V::NotImpl) => None,
                    Ok(v) => Some(truth(&v).map(|t| V::Bool(!t))),
                    Err(e) => Some(Err(e)),
                };
            }
            None => return None,
        };
        match call(&f, Args::p(vec![y.clone()])) {
            Ok(V::NotImpl) => None,
            r => Some(r),
        }
    };
    let (ta, tb) = match (type_of(a), type_of(b)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
    };
    let first = !Rc::ptr_eq(&ta, &tb) && tb.is_sub(&ta) && slot(b, r).is_some();
    if first {
        if let Some(x) = side(b, a, r) {
            return Some(x);
        }
    }
    if let Some(x) = side(a, b, l) {
        return Some(x);
    }
    if !first {
        if let Some(x) = side(b, a, r) {
            return Some(x);
        }
    }
    Some(match op {
        "==" => Ok(V::Bool(is(a, b))),
        "!=" => Ok(V::Bool(!is(a, b))),
        _ => Err(cmp_err(op, a, b)),
    })
}
