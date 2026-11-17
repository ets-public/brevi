
// ---------- pickle and copy of the program's own objects ----------
// CPython's object.__reduce_ex__ (typeobject.c reduce_newobj, copyreg._reduce_ex), object.__getstate__, set's
// __reduce__, copy._reconstruct and pickle's BUILD, for instances of the program's classes. Only classes whose
// whole MRO is the program's classes plus object, set or frozenset (pk_cat): those are the ones CPython reduces
// with nothing more than these rules. pyglue.rs hands the result to CPython's pickle.

// object, set or frozenset: the builtin base an instance of `c` is reduced on, when every other class of its MRO
// is the program's (made by a class statement, type() or a dataclass) and it is no Enum, exception or namedtuple
pub fn pk_cat(c: &Rc<Class>) -> Option<BT> {
    if is_enum(c) || c.lookup("__bv_namedtuple__").is_some() {
        return None;
    }
    for k in c.mro.borrow().iter() {
        if k.made.get() == 0 && !(k.module == "builtins" && matches!(k.name, "object" | "set" | "frozenset")) {
            return None;
        }
    }
    match c.base_bt() {
        b @ (BT::Object | BT::Set | BT::FSet) => Some(b),
        _ => None,
    }
}
// `n` defined by one of the program's classes in c's MRO (not by object, set or frozenset)
pub fn pk_own(c: &Rc<Class>, n: &str) -> Option<V> {
    c.mro.borrow().iter().filter(|k| k.made.get() != 0).find_map(|k| k.own(n))
}
// copyreg._slotnames: each class's own __slots__, in MRO order, without __dict__ and __weakref__, private names
// mangled
fn pk_slotnames(c: &Rc<Class>) -> Result<Vec<String>, E> {
    let mut out: Vec<String> = vec![];
    for k in c.mro.borrow().iter().filter(|k| k.made.get() != 0) {
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
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    Ok(out)
}
// object.__getstate__ as CPython 3.13's object_getstate_default makes it: the instance __dict__ (None when empty),
// and when the classes have __slots__, (that, {slot: value} of the slots that are set)
fn pk_default_state(v: &V, c: &Rc<Class>) -> R {
    let dict = if c.has_dict() {
        match getattr(v, "__dict__")? {
            V::Dict(x) if x.borrow().len() > 0 => V::Dict(x),
            _ => V::None,
        }
    } else {
        V::None
    };
    let mut slots = Dict::new();
    for name in pk_slotnames(c)? {
        match getattr(v, &name) {
            Ok(x) => slots.set(st(name), x)?,
            Err(e) if is_exc(&e.0, "AttributeError") => {}
            Err(e) => return Err(e),
        }
    }
    Ok(if slots.len() > 0 { tup(vec![dict, V::Dict(Rc::new(RefCell::new(slots)))]) } else { dict })
}
// x.__getstate__(): the class's own, or object's
fn pk_getstate(v: &V, c: &Rc<Class>) -> R {
    if pk_own(c, "__getstate__").is_some() {
        return call_method(v, "__getstate__", Args::p(vec![]));
    }
    pk_default_state(v, c)
}

// How an instance is made again: copyreg.__newobj__(cls, *args) (cls.__new__(cls, *args)), copyreg._reconstructor
// (object.__new__(cls)), or a call of the class (set's __reduce__ gives (cls, (list(self),), state))
pub enum PkHow {
    NewObj,
    Recon,
    Call,
}
// What pickle.dumps saves for the program's object `v` at `proto`: how it is made, the class, the arguments after
// the class, and the state (None: none). Refused for a class that changes how it pickles in ways not run here.
pub fn pk_reduce(v: &V, proto: i64) -> Result<(PkHow, Rc<Class>, Vec<V>, V), E> {
    let V::Obj(o) = v else { return type_err(format!("cannot pickle '{}' object", tn(v))) };
    let c = o.cls.clone();
    let Some(cat) = pk_cat(&c) else { return type_err(format!("cannot pickle '{}' object", tn(v))) };
    for n in ["__reduce_ex__", "__reduce__", "__getnewargs__", "__getnewargs_ex__"] {
        if pk_own(&c, n).is_some() {
            return Err(unsupported(&format!("pickling an instance of a class that defines {n}")));
        }
    }
    match cat {
        // set_reduce: (type(self), (list(self),), self.__getstate__())
        BT::Set | BT::FSet => {
            let items = match o.inner() {
                Some(s @ (V::Set(_) | V::FSet(_))) => to_vec(&s)?,
                _ => return type_err(format!("cannot pickle '{}' object", tn(v))),
            };
            let state = pk_getstate(v, &c)?;
            Ok((PkHow::Call, c, vec![list(items)], state))
        }
        // reduce_newobj: (copyreg.__newobj__, (cls,), state, None, None)
        _ if proto >= 2 => {
            let state = pk_getstate(v, &c)?;
            Ok((PkHow::NewObj, c, vec![], state))
        }
        // copyreg._reduce_ex: base is object, so (_reconstructor, (cls, object, None)) and the state when true
        _ => {
            let state = if pk_own(&c, "__getstate__").is_some() {
                call_method(v, "__getstate__", Args::p(vec![]))?
            } else {
                match getattr(v, "__slots__") {
                    Ok(x) if truth(&x)? => return type_err("a class that defines __slots__ without defining __getstate__ cannot be pickled".into()),
                    Ok(_) => {}
                    Err(e) if is_exc(&e.0, "AttributeError") => {}
                    Err(e) => return Err(e),
                }
                pk_default_state(v, &c)?
            };
            let state = if truth(&state)? { state } else { V::None };
            Ok((PkHow::Recon, c, vec![V::Class(bclass("object")), V::None], state))
        }
    }
}

// Whether giving an instance of `c` its state runs code of the program: its own __setstate__ (other than
// dataclasses' _dataclass_setstate, which only stores), a __getattr__ or __getattribute__ that the lookup of
// __setstate__ goes through, or a __setattr__ that the slots part goes through. pickle.loads builds the
// program's objects only once CPython's Unpickler has finished (pyglue's pyd_stinst), so such a BUILD runs
// after the load: pyglue checks that this changes nothing CPython's order would show.
pub fn pk_build_runs_code(c: &Rc<Class>) -> bool {
    if let Some(f) = pk_own(c, "__setstate__") {
        if !is(&f, &dc_state_fn("_dataclass_setstate")) {
            return true;
        }
    }
    ["__getattr__", "__getattribute__", "__setattr__"].iter().any(|n| pk_own(c, n).is_some())
}

// pickle's BUILD (load_build, `load`) and copy._reconstruct's state step on the program's object `o`, as CPython
// 3.13 runs them: __setstate__(state) when the object has one (pickle: getattr(inst, "__setstate__", None);
// copy: hasattr(y, "__setstate__") and then y.__setstate__, so a __getattr__ of the program runs once or
// twice), or else the __dict__ part stored in the instance's __dict__ and the slots part set with setattr.
pub fn pk_setstate(o: &V, state: V, load: bool) -> Result<(), E> {
    let V::Obj(ob) = o else { return Err(unsupported("setting the pickled state of this object")) };
    let c = ob.cls.clone();
    let own_ga = pk_own(&c, "__getattribute__").is_some();
    if own_ga || pk_own(&c, "__getattr__").is_some() {
        match getattr(o, "__setstate__") {
            Ok(f) => {
                let f = if load { f } else { getattr(o, "__setstate__")? };
                call(&f, Args::p(vec![state]))?;
                return Ok(());
            }
            Err(e) if is_exc(&e.0, "AttributeError") => {}
            Err(e) => return Err(e),
        }
    } else if pk_own(&c, "__setstate__").is_some() {
        call_method(o, "__setstate__", Args::p(vec![state]))?;
        return Ok(());
    }
    let (dict, slots) = match state {
        V::Tuple(t) if t.len() == 2 => (t[0].clone(), t[1].clone()),
        x => (x, V::None),
    };
    match &dict {
        V::None => {}
        // pickle reads inst.__dict__ for any dictionary, copy only for a non-empty one (`if state:`); through a
        // __getattribute__ of the program that read is the program's, and the items go into what it gives
        V::Dict(x) if own_ga => {
            let items = x.borrow().items();
            if load || !items.is_empty() {
                let d = getattr(o, "__dict__")?;
                for (k, x) in items {
                    setitem(&d, &k, x)?;
                }
            }
        }
        V::Dict(x) if ob.cls.has_dict() => {
            for (k, x) in x.borrow().items() {
                let V::Str(k) = k else { return Err(unsupported("a pickled __dict__ key that is not a str")) };
                ob.set_res(sym(k.text()), x)?;
            }
        }
        _ => return Err(unsupported("a pickled state that is not a dictionary for the instance's __dict__")),
    }
    match &slots {
        V::None => {}
        V::Dict(x) => {
            for (k, x) in x.borrow().items() {
                let V::Str(k) = k else { return Err(unsupported("a pickled slot name that is not a str")) };
                setattr(o, sym(k.text()), x)?;
            }
        }
        _ => return Err(unsupported("a pickled slot state that is not a dictionary")),
    }
    Ok(())
}

// dataclasses._dataclass_getstate and _dataclass_setstate (dataclasses.py 3.13), which dataclass(frozen=True,
// slots=True) puts on the class: one function each, shared by every class, as CPython's module functions
thread_local! {
    static DC_STATE_FNS: RefCell<Vec<(&'static str, V)>> = const { RefCell::new(Vec::new()) };
}
pub fn dc_state_fn(n: &'static str) -> V {
    if let Some(f) = DC_STATE_FNS.with(|h| h.borrow().iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone())) {
        return f;
    }
    let get = n == "_dataclass_getstate";
    let f = func(n, move |a: Args| {
        let want = if get { 1 } else { 2 };
        if a.p.len() != want || !a.k.is_empty() {
            let what = if get { "1 positional argument" } else { "2 positional arguments" };
            return type_err(format!("{n}() takes {what} but {} were given", a.p.len()));
        }
        let me = &a.p[0];
        let fields = dataclass_fn("fields", Args::p(vec![me.clone()]))?;
        if get {
            // [getattr(self, f.name) for f in fields(self)]
            let mut out = vec![];
            for f in to_vec(&fields)? {
                let name = getattr(&f, "name")?;
                out.push(getattr(me, &to_str(&name)?)?);
            }
            return Ok(list(out));
        }
        // for field, value in zip(fields(self), state): object.__setattr__(self, field.name, value)
        let values = to_vec(&a.p[1])?;
        for (f, value) in to_vec(&fields)?.into_iter().zip(values) {
            let name = getattr(&f, "name")?;
            object_setattr(me, &[name, value])?;
        }
        Ok(V::None)
    });
    let f = with_module(f, "dataclasses");
    DC_STATE_FNS.with(|h| h.borrow_mut().push((n, f.clone())));
    f
}

// copy.copy and copy.deepcopy (memo: Some) of the program's object `x` through copy._reconstruct, as CPython
// copies it, when its class changes how it reduces or rebuilds (__reduce_ex__, __reduce__, __getstate__,
// __setstate__, __getnewargs__, __getnewargs_ex__, __new__, or __init__ on a set or frozenset). None: copying
// it member by member (rt_mod's shallow_copy and deepcopy) gives CPython's result. Refused for an instance of a
// class with such a method whose reduction is not followed here.
pub fn pk_copy(x: &V, memo: Option<&mut Vec<(usize, V)>>) -> Option<R> {
    let V::Obj(o) = x else { return None };
    let c = o.cls.clone();
    const HOOKS: [&str; 7] = ["__reduce_ex__", "__reduce__", "__getstate__", "__setstate__", "__getnewargs__", "__getnewargs_ex__", "__new__"];
    let hook = HOOKS.iter().find(|n| pk_own(&c, n).is_some());
    let cat = pk_cat(&c);
    let set_init = matches!(cat, Some(BT::Set | BT::FSet)) && pk_own(&c, "__init__").is_some();
    if hook.is_none() && !set_init {
        return None;
    }
    if cat.is_none() {
        return Some(Err(unsupported(&format!("copying an instance of a class that defines {}", hook.unwrap()))));
    }
    Some(pk_copy_red(x, &c, cat.unwrap(), memo))
}
fn pk_copy_red(x: &V, c: &Rc<Class>, cat: BT, mut memo: Option<&mut Vec<(usize, V)>>) -> R {
    // rv = x.__reduce_ex__(4)
    enum How {
        Call(V, Vec<V>, Vec<(Sym, V)>),
        New(Vec<V>, Vec<(Sym, V)>),
    }
    let (how, state, listiter, dictiter) = if pk_own(c, "__reduce_ex__").is_some() || pk_own(c, "__reduce__").is_some() {
        let rv = if pk_own(c, "__reduce_ex__").is_some() {
            call_method(x, "__reduce_ex__", Args::p(vec![V::Int(4)]))?
        } else {
            call_method(x, "__reduce__", Args::p(vec![]))?
        };
        if matches!(rv, V::Str(_)) {
            return Ok(x.clone());
        }
        let V::Tuple(t) = &rv else { return Err(exc("copy.Error", format!("un(deep)copyable object of type {}", c.name))) };
        if t.len() < 2 || t.len() > 5 {
            return Err(unsupported("copying an object whose __reduce__ gives a tuple of this length"));
        }
        let args = to_vec(&t[1])?;
        let opt = |i: usize| t.get(i).cloned().filter(|v| !matches!(v, V::None));
        (How::Call(t[0].clone(), args, vec![]), opt(2), opt(3), opt(4))
    } else {
        match cat {
            BT::Set | BT::FSet => {
                let items = match o_inner(x) {
                    Some(s) => to_vec(&s)?,
                    None => vec![],
                };
                (How::Call(V::Class(c.clone()), vec![list(items)], vec![]), Some(pk_getstate(x, c)?), None, None)
            }
            _ => {
                // reduce_newobj: __getnewargs_ex__ or __getnewargs__ give the arguments of cls.__new__
                let (args, kw) = if pk_own(c, "__getnewargs_ex__").is_some() {
                    let r = call_method(x, "__getnewargs_ex__", Args::p(vec![]))?;
                    let V::Tuple(t) = &r else { return type_err(format!("__getnewargs_ex__ should return a tuple, not '{}'", tn(&r))) };
                    if t.len() != 2 {
                        return val_err(format!("__getnewargs_ex__ should return a tuple of length 2, not {}", t.len()));
                    }
                    let V::Tuple(a) = &t[0] else { return type_err(format!("first item of the tuple returned by __getnewargs_ex__ must be a tuple, not '{}'", tn(&t[0]))) };
                    let V::Dict(k) = &t[1] else { return type_err(format!("second item of the tuple returned by __getnewargs_ex__ must be a dict, not '{}'", tn(&t[1]))) };
                    let mut kw = vec![];
                    for (kk, vv) in k.borrow().items() {
                        let V::Str(kk) = kk else { return Err(unsupported("__getnewargs_ex__ keywords that are not str")) };
                        kw.push((sym(kk.text()), vv));
                    }
                    (Some(a.to_vec()), kw)
                } else if pk_own(c, "__getnewargs__").is_some() {
                    let r = call_method(x, "__getnewargs__", Args::p(vec![]))?;
                    let V::Tuple(t) = &r else { return type_err(format!("__getnewargs__ should return a tuple, not '{}'", tn(&r))) };
                    (Some(t.to_vec()), vec![])
                } else {
                    (None, vec![])
                };
                let state = pk_getstate(x, c)?;
                (How::New(args.unwrap_or_default(), kw), Some(state).filter(|v| !matches!(v, V::None)), None, None)
            }
        }
    };
    // copy._reconstruct
    let deep = memo.is_some();
    let dc = |v: &V, memo: &mut Option<&mut Vec<(usize, V)>>| -> R {
        match memo {
            Some(m) => deepcopy(v, m),
            None => Ok(v.clone()),
        }
    };
    let y = match how {
        How::Call(f, args, kw) => {
            let args = if deep && !args.is_empty() { to_vec(&dc(&tup(args), &mut memo)?)? } else { args };
            call(&f, Args { p: args, k: kw })?
        }
        How::New(args, kw) => {
            let args = if deep && !args.is_empty() { to_vec(&dc(&tup(args), &mut memo)?)? } else { args };
            let cls = V::Class(c.clone());
            let new = getattr(&cls, "__new__")?;
            let mut p = vec![cls];
            p.extend(args);
            call(&new, Args { p, k: kw })?
        }
    };
    if let (Some(m), V::Obj(o)) = (memo.as_mut(), x) {
        m.push((Rc::as_ptr(o) as *const u8 as usize, y.clone()));
    }
    if let Some(s) = state {
        let s = dc(&s, &mut memo)?;
        match &y {
            V::Obj(yo) if pk_own(&yo.cls, "__setstate__").is_some() || pk_cat(&yo.cls).is_some() => pk_setstate(&y, s, false)?,
            _ if getattr(&y, "__setstate__").is_ok() => {
                call_method(&y, "__setstate__", Args::p(vec![s]))?;
            }
            _ => return Err(unsupported("copying an object whose __reduce__ makes an object of another kind with a state")),
        }
    }
    if let Some(li) = listiter {
        for item in to_vec(&li)? {
            let item = dc(&item, &mut memo)?;
            call_method(&y, "append", Args::p(vec![item]))?;
        }
    }
    if let Some(di) = dictiter {
        for kv in to_vec(&di)? {
            let kv = to_vec(&kv)?;
            if kv.len() != 2 {
                return val_err("not enough values to unpack (expected 2)".into());
            }
            let k = dc(&kv[0], &mut memo)?;
            let v = dc(&kv[1], &mut memo)?;
            setitem(&y, &k, v)?;
        }
    }
    Ok(y)
}
fn o_inner(x: &V) -> Option<V> {
    match x {
        V::Obj(o) => o.inner(),
        _ => None,
    }
}
