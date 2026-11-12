// ---------- class __dict__: vars(C), C.__dict__ and dir() ----------
//
// A class's __dict__ is what CPython's type dict holds, in its order. A program class (a class statement, or
// type(name, bases, ns)) keeps its namespace in Class.attrs; the entries CPython's type_new adds that Brevi
// keeps elsewhere are made here: __firstlineno__ and __static_attributes__ (from the compiler, for a class
// statement), the member descriptors of __slots__ (mangled, sorted), and the __dict__ and __weakref__ getset
// descriptors when the class adds them, at the "__bv_tnew__" mark make_class leaves after the namespace. A
// builtin type answers from CPython's own table (rt_tdict_data.rs, tools/gen_type_dicts.py). Anything else
// refuses: a class Brevi's runtime has changed in its own way (an Enum, a dataclass, ...), a type with no
// table, a class whose layout depends on one.

pub struct TInfo {
    // (__firstlineno__, __static_attributes__) of a class statement; None for type(name, bases, ns)
    pub stmt: Option<(i64, V)>,
    // CPython's tp_base: the base best_base() picks (None for object)
    pub base: Option<Rc<Class>>,
    pub add_dict: bool,
    pub add_weak: bool,
    // the class's own __slots__ names, mangled and sorted, without __dict__ and __weakref__
    pub slots: Vec<Sym>,
    // names Brevi's runtime keeps on the class that CPython's dict does not hold (namedtuple's helpers)
    pub hidden: Vec<Sym>,
    // set once make_class is done: from then on only the program changes the class exactly
    pub sealed: Cell<bool>,
    // why the dict can no longer be given exactly
    pub inexact: Cell<Option<&'static str>>,
}

pub const TNEW: &str = "__bv_tnew__";

thread_local! {
    static TROWS: RefCell<Option<HashMap<(&'static str, &'static str), usize>>> = RefCell::new(None);
    static TDESCS: RefCell<HashMap<(usize, Sym), V>> = RefCell::new(HashMap::new());
}

fn trow(c: &Class) -> Option<&'static (&'static str, &'static str, &'static str, u8, (&'static str, &'static str), &'static [TEnt])> {
    TROWS.with(|t| {
        let mut t = t.borrow_mut();
        let m = t.get_or_insert_with(|| TYPE_DICTS.iter().enumerate().map(|(i, r)| ((r.0, r.1), i)).collect());
        m.get(&(c.module, c.name)).map(|i| &TYPE_DICTS[*i])
    })
}

// whether CPython's builtin type `name` (or object, which it inherits from) has the attribute `attr`
pub fn cpy_builtin_has(name: &str, attr: &str) -> bool {
    let has = |n: &str| TYPE_DICTS.iter().any(|r| r.0 == "builtins" && r.1 == n && r.5.iter().any(|e| e.0 == attr));
    has(name) || has("object")
}

pub fn tinfo(c: &Class) -> Option<Rc<TInfo>> {
    c.tinfo.borrow().clone()
}

// the class changed outside the program's own setattr/delattr (Brevi's runtime did it): its dict is no longer
// CPython's
pub fn tdict_native_change(c: &Class) {
    if let Some(t) = c.tinfo.borrow().as_ref() {
        if t.sealed.get() && t.inexact.get().is_none() {
            t.inexact.set(Some("changed by Brevi's runtime"));
        }
    }
}
pub fn tdict_inexact(c: &Class, why: &'static str) {
    if let Some(t) = c.tinfo.borrow().as_ref() {
        if t.inexact.get().is_none() {
            t.inexact.set(Some(why));
        }
    }
}

// (instances have a __dict__, a __weakref__ slot, the type is variable-sized), as CPython's layout gives them
fn layout(c: &Rc<Class>) -> Option<(bool, bool, bool)> {
    if let Some(t) = tinfo(c) {
        let (d, w, v) = match &t.base {
            Some(b) => layout(b)?,
            None => (false, false, false),
        };
        return Some((d || t.add_dict, w || t.add_weak, v));
    }
    let r = trow(c)?;
    Some((r.3 & 1 != 0, r.3 & 2 != 0, r.3 & 4 != 0))
}

// CPython's solid_base(): the nearest class in the base chain that adds instance fields
fn solid(c: &Rc<Class>) -> Option<Rc<Class>> {
    if let Some(t) = tinfo(c) {
        if !t.slots.is_empty() {
            return Some(c.clone());
        }
        return match &t.base {
            Some(b) => solid(b),
            None => Some(bclass("object")),
        };
    }
    let r = trow(c)?;
    let (m, n) = r.4;
    c.mro.borrow().iter().find(|k| k.module == m && k.name == n).cloned()
}

// CPython's best_base(): None when a base's layout is unknown here (or the bases conflict, which CPython
// refuses and Brevi's classes do not check)
fn best_base(bases: &[Rc<Class>]) -> Option<Rc<Class>> {
    let mut base: Option<Rc<Class>> = None;
    let mut winner: Option<Rc<Class>> = None;
    for b in bases {
        let cand = solid(b)?;
        match &winner {
            None => {
                winner = Some(cand);
                base = Some(b.clone());
            }
            Some(w) if w.is_sub(&cand) => {}
            Some(w) if cand.is_sub(w) => {
                winner = Some(cand);
                base = Some(b.clone());
            }
            Some(_) => return None,
        }
    }
    base
}

fn slot_list(v: &V) -> Vec<Sym> {
    let mut out = vec![];
    match v {
        V::Str(x) => out.push(sym(&x.s)),
        V::Tuple(_) | V::List(_) | V::Dict(_) => {
            if let Ok(xs) = to_vec(v) {
                for x in xs {
                    if let V::Str(x) = x {
                        out.push(sym(&x.s));
                    }
                }
            }
        }
        _ => {}
    }
    out
}

// _Py_Mangle for a __slots__ name of class `cls`
pub fn mangle_name(cls: &str, n: &str) -> Sym {
    if !n.starts_with("__") || n.ends_with("__") || n.contains('.') {
        return sym(n);
    }
    let c = cls.trim_start_matches('_');
    if c.is_empty() {
        return sym(n);
    }
    sym(&format!("_{c}{n}"))
}

// The record make_class keeps for a new class (its own bases given): what type_new adds to its dict.
pub fn tdict_setup(c: &Rc<Class>, bases: &[Rc<Class>], stmt: Option<(i64, Vec<Sym>)>) {
    let mut inexact: Option<&'static str> = None;
    let base = best_base(bases);
    let known = base.as_ref().and_then(layout);
    if known.is_none() {
        inexact = Some("a base whose layout Brevi does not know");
    }
    let (bd, bw, bv) = known.unwrap_or((false, false, false));
    let (mut add_dict, mut add_weak) = (!bd, !bw && !bv);
    let mut slots: Vec<Sym> = vec![];
    if let Some(sv) = c.own("__slots__") {
        let names = slot_list(&sv);
        add_dict = false;
        add_weak = false;
        for n in names {
            if n == "__dict__" {
                if bd || add_dict {
                    inexact = Some("a __dict__ slot CPython refuses");
                }
                add_dict = true;
            } else if n == "__weakref__" {
                if bw || bv || add_weak {
                    inexact = Some("a __weakref__ slot CPython refuses");
                }
                add_weak = true;
            } else {
                let m = mangle_name(c.name, n);
                if !slots.contains(&m) {
                    slots.push(m);
                }
            }
        }
        slots.sort();
    }
    let stmt = stmt.map(|(l, xs)| (l, tup(xs.iter().map(|x| s(x)).collect())));
    let t = TInfo { stmt, base, add_dict, add_weak, slots, hidden: vec![], sealed: Cell::new(false), inexact: Cell::new(inexact) };
    c.attrs.borrow_mut().push((TNEW, V::None));
    *c.tinfo.borrow_mut() = Some(Rc::new(t));
}
pub fn tdict_seal(c: &Class) {
    if let Some(t) = c.tinfo.borrow().as_ref() {
        t.sealed.set(true);
    }
}
// namedtuple's class: made by type() in CPython, the namespace already on the class; `hidden` are the names
// Brevi adds for itself
pub fn tdict_typemade(c: &Rc<Class>, hidden: Vec<Sym>) {
    let t = TInfo { stmt: None, base: Some(c.bases[0].clone()), add_dict: false, add_weak: false, slots: vec![], hidden, sealed: Cell::new(true), inexact: Cell::new(None) };
    c.attrs.borrow_mut().push((TNEW, V::None));
    *c.tinfo.borrow_mut() = Some(Rc::new(t));
}

// The descriptor objects of a type dict: method, slot wrapper, getset, member and classmethod descriptors,
// a builtin __new__, a builtin staticmethod. Each one is made once (`is` holds as in CPython).
pub struct TDesc {
    pub kind: u8,
    pub owner: Rc<Class>,
    pub name: Sym,
}
fn tdesc(kind: u8, owner: &Rc<Class>, name: Sym) -> V {
    let key = (Rc::as_ptr(owner) as *const u8 as usize, name);
    if let Some(v) = TDESCS.with(|m| m.borrow().get(&key).cloned()) {
        return v;
    }
    let v = V::Ext(ExtRc::new(TDesc { kind, owner: owner.clone(), name }));
    TDESCS.with(|m| m.borrow_mut().insert(key, v.clone()));
    v
}
// Whether CPython's own dict of the builtin type `c` holds `n` (None: Brevi has no table for the type), and
// the type's tp_name, for subscripting a class (rt_typing.rs class_subscript).
pub fn cpy_dict_has(c: &Class, n: &str) -> Option<(bool, &'static str)> {
    trow(c).map(|r| (r.5.iter().any(|e| e.0 == n), r.2))
}
// Whether CPython's type lookup finds `n` on the builtin type `c` (#470): in the dict of a class on its MRO, or
// on `type`, its metaclass. None when a class on the MRO has no table, so Brevi cannot tell.
pub fn cpy_type_has(c: &Rc<Class>, n: &str) -> Option<bool> {
    for k in c.mro.borrow().iter() {
        if trow(k)?.5.iter().any(|e| e.0 == n) {
            return Some(true);
        }
    }
    let ty = TYPE_DICTS.iter().find(|r| r.0 == "builtins" && r.1 == "type")?;
    Some(ty.5.iter().any(|e| e.0 == n))
}
fn tname_of(c: &Class) -> String {
    match trow(c) {
        Some(r) => r.2.to_string(),
        None => dotted_tname(c).to_string(),
    }
}
fn dotted_tname(c: &Class) -> &'static str {
    if c.module == "builtins" {
        if let Some((_, n)) = c.name.rsplit_once('.') {
            return n;
        }
    }
    c.name
}
impl TDesc {
    fn fail(&self) -> E {
        unsupported(&format!("{}.{}", tname_of(&self.owner), self.name))
    }
    // the attribute this descriptor stands for, read off `o` the way Brevi's runtime gives it
    fn read(&self, o: &V) -> R {
        match getattr(o, self.name) {
            Err(e) if is_exc(&e.0, "AttributeError") && matches!(self.kind, 0 | 1 | 4 | 5 | 10) => Err(self.fail()),
            r => r,
        }
    }
    fn check_self(&self, x: &V) -> Result<(), E> {
        let k = type_of(x)?;
        if k.is_sub(&self.owner) {
            return Ok(());
        }
        type_err(format!("descriptor '{}' for '{}' objects doesn't apply to a '{}' object", self.name, tname_of(&self.owner), tn(x)))
    }
}
impl Ext for TDesc {
    fn tname(&self) -> &'static str {
        match self.kind {
            0 => "method_descriptor",
            1 => "wrapper_descriptor",
            2 => "getset_descriptor",
            3 => "member_descriptor",
            4 => "classmethod_descriptor",
            5 => "builtin_function_or_method",
            _ => "staticmethod",
        }
    }
    fn repr(&self) -> String {
        let t = tname_of(&self.owner);
        let at = Rc::as_ptr(&self.owner) as *const u8 as usize;
        match self.kind {
            0 | 4 => format!("<method '{}' of '{t}' objects>", self.name),
            1 => format!("<slot wrapper '{}' of '{t}' objects>", self.name),
            2 => format!("<attribute '{}' of '{t}' objects>", self.name),
            3 => format!("<member '{}' of '{t}' objects>", self.name),
            5 => format!("<built-in method {} of type object at 0x{at:x}>", self.name),
            _ => format!("<staticmethod(<built-in method {} of type object at 0x{at:x}>)>", self.name),
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, me: &V, n: &str) -> Option<R> {
        Some(match n {
            "__name__" => Ok(s(self.name)),
            "__qualname__" => Ok(st(format!("{}.{}", if trow(&self.owner).is_some() { tname_of(&self.owner) } else { self.owner.qualname.get().to_string() }, self.name))),
            "__objclass__" if self.kind <= 4 => Ok(V::Class(self.owner.clone())),
            "__self__" if self.kind == 5 => Ok(V::Class(self.owner.clone())),
            "__func__" | "__wrapped__" if self.kind == 10 => self.read(&V::Class(self.owner.clone())),
            "__doc__" => Err(unsupported(&format!("__doc__ of {}", self.repr()))),
            "__get__" | "__set__" | "__delete__" | "__call__" => return None,
            _ => {
                let _ = me;
                return None;
            }
        })
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let p = std::mem::take(&mut a.p);
        let k = std::mem::take(&mut a.k);
        let owner = V::Class(self.owner.clone());
        Some(match n {
            "__get__" => {
                let obj = p.first().cloned().unwrap_or(V::None);
                match self.kind {
                    5 => return None,
                    4 => self.read(p.get(1).filter(|t| !matches!(t, V::None)).unwrap_or(&owner)),
                    10 => self.read(&owner),
                    _ if matches!(obj, V::None) => Ok(me.clone()),
                    _ => self.check_self(&obj).and_then(|_| self.read(&obj)),
                }
            }
            "__set__" if matches!(self.kind, 2 | 3) => {
                if p.len() != 2 {
                    return Some(type_err(format!("expected 2 arguments, got {}", p.len())));
                }
                self.check_self(&p[0]).and_then(|_| setattr(&p[0], self.name, p[1].clone())).map(|_| V::None)
            }
            "__delete__" if matches!(self.kind, 2 | 3) => {
                if p.len() != 1 {
                    return Some(type_err(format!("expected 1 argument, got {}", p.len())));
                }
                self.check_self(&p[0]).and_then(|_| delattr(&p[0], self.name)).map(|_| V::None)
            }
            "__call__" => match self.kind {
                0 | 1 => {
                    let Some(x) = p.first().cloned() else {
                        return Some(type_err(format!("unbound method {}.{}() needs an argument", tname_of(&self.owner), self.name)));
                    };
                    match self.check_self(&x).and_then(|_| self.read(&x)) {
                        Ok(f) => call(&f, Args { p: p[1..].to_vec(), k }),
                        Err(e) => Err(e),
                    }
                }
                4 | 5 | 10 => match self.read(&owner) {
                    Ok(f) => {
                        let rest = if self.kind == 4 && matches!(p.first(), Some(V::Class(_))) { p[1..].to_vec() } else { p };
                        call(&f, Args { p: rest, k })
                    }
                    Err(e) => Err(e),
                },
                _ => type_err(format!("'{}' object is not callable", self.tname())),
            },
            _ => return None,
        })
    }
}

// The entries of c.__dict__, in CPython's order, or the refusal.
pub fn class_dict(c: &Rc<Class>) -> Result<Vec<(Sym, V)>, E> {
    class_dict_with(c, true)
}
// x.__dict__ as Brevi's own module copies read it: a class's namespace as Brevi keeps it, never refused
pub fn own_dict_of(x: &V) -> R {
    let V::Class(c) = x else { return getattr(x, "__dict__") };
    let items = match class_dict_with(c, false) {
        Ok(v) => v,
        Err(_) => {
            let mut v: Vec<(Sym, V)> = vec![("__module__", s(c.module))];
            v.extend(c.attrs.borrow().iter().filter(|(k, _)| !k.starts_with("__bv_")).cloned());
            if c.own("__doc__").is_none() {
                v.push(("__doc__", V::None));
            }
            v
        }
    };
    let mut d = Dict::new();
    for (k, v) in items {
        d.set(s(k), v)?;
    }
    Ok(V::View(Rc::new((V::Dict(Rc::new(RefCell::new(d))), 3))))
}
fn class_dict_with(c: &Rc<Class>, strict: bool) -> Result<Vec<(Sym, V)>, E> {
    let qn = c.qualname.get();
    if let Some(t) = tinfo(c) {
        if let Some(why) = t.inexact.get().filter(|_| strict) {
            return Err(unsupported(&format!("the __dict__ of class {qn} ({why})")));
        }
        let mut out: Vec<(Sym, V)> = vec![];
        let module = c.own("__module__").unwrap_or_else(|| s(c.module));
        if let Some((line, _)) = &t.stmt {
            out.push(("__module__", module.clone()));
            out.push(("__firstlineno__", c.own("__firstlineno__").unwrap_or(V::Int(*line))));
        }
        let attrs = c.attrs.borrow().clone();
        for (k, v) in attrs {
            if k == TNEW {
                if let Some((_, sa)) = &t.stmt {
                    out.push(("__static_attributes__", sa.clone()));
                } else if !out.iter().any(|(n, _)| *n == "__module__") {
                    out.push(("__module__", module.clone()));
                }
                for n in &t.slots {
                    out.push((*n, tdesc(3, c, *n)));
                }
                if t.add_dict {
                    out.push(("__dict__", tdesc(2, c, "__dict__")));
                }
                if t.add_weak {
                    out.push(("__weakref__", tdesc(2, c, "__weakref__")));
                }
                continue;
            }
            if k.starts_with("__bv_") || k == "__qualname__" || t.hidden.contains(&k) || (t.stmt.is_some() && (k == "__module__" || k == "__firstlineno__")) {
                continue;
            }
            // type_new makes a plain __new__ a staticmethod, and __init_subclass__ and __class_getitem__
            // classmethods
            let v = match (&v, k) {
                (V::Func(f), "__new__") if f.kind.get() == 0 => bi_staticmethod(Args::p(vec![v.clone()]))?,
                (V::Func(f), "__init_subclass__" | "__class_getitem__") if f.kind.get() == 0 => bi_classmethod(Args::p(vec![v.clone()]))?,
                _ => v,
            };
            out.push((k, v));
        }
        return Ok(out);
    }
    let Some(r) = trow(c) else {
        return Err(unsupported(&format!("the __dict__ of the {} type", dotted_tname(c))));
    };
    let mut out = vec![];
    for (n, kind, text) in r.5.iter() {
        let v = match kind {
            6 => V::None,
            7 => s(text),
            8 => V::Int(text.parse().unwrap_or(0)),
            9 => tup(if text.is_empty() { vec![] } else { text.split(',').map(s).collect() }),
            k => tdesc(*k, c, n),
        };
        out.push((*n, v));
    }
    Ok(out)
}

pub fn class_dict_view(c: &Rc<Class>) -> R {
    let mut d = Dict::new();
    for (k, v) in class_dict(c)? {
        d.set(s(k), v)?;
    }
    Ok(V::View(Rc::new((V::Dict(Rc::new(RefCell::new(d))), 3))))
}

// A class attribute only the type dict holds (no entry in Class.attrs): __firstlineno__,
// __static_attributes__, a slot's member descriptor, the __weakref__ getset; found along the MRO as
// CPython's type lookup finds them.
pub fn tdict_attr(c: &Rc<Class>, name: &str) -> Option<V> {
    for k in c.mro.borrow().iter() {
        let Some(t) = tinfo(k) else { continue };
        match name {
            "__firstlineno__" => {
                if let Some((l, _)) = &t.stmt {
                    return Some(V::Int(*l));
                }
            }
            "__static_attributes__" => {
                if let Some((_, sa)) = &t.stmt {
                    return Some(sa.clone());
                }
            }
            "__weakref__" if t.add_weak => return Some(tdesc(2, k, "__weakref__")),
            _ => {
                if let Some(n) = t.slots.iter().find(|n| **n == name) {
                    return Some(tdesc(3, k, n));
                }
            }
        }
    }
    None
}

fn merge_class_names(c: &Rc<Class>, out: &mut Vec<Sym>) -> Result<(), E> {
    for (k, _) in class_dict(c)? {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    for b in c.bases.iter() {
        merge_class_names(b, out)?;
    }
    Ok(())
}

// dir(x): x's own __dir__ when its class defines one, else CPython's object.__dir__ and type.__dir__ over
// the exact dicts above. A module refuses (Brevi's module namespaces are not CPython's).
pub fn bi_dir(a: Args) -> R {
    if a.p.is_empty() && a.k.is_empty() {
        return Err(unsupported("dir() without an argument"));
    }
    let x = a1(&a, "dir")?;
    let cls = type_of(&x)?;
    let user_dir = cls.mro.borrow().iter().find_map(|k| if tinfo(k).is_some() { k.own("__dir__") } else { None });
    let mut names: Vec<V> = if let Some(f) = user_dir {
        let r = call(&f, Args::p(vec![x.clone()]))?;
        to_vec(&r)?
    } else {
        let mut ns: Vec<Sym> = vec![];
        match &x {
            V::Class(c) => merge_class_names(c, &mut ns)?,
            V::Module(_) => return Err(unsupported("dir() of a module")),
            _ => {
                let own = match &x {
                    V::Obj(o) if o.cls.has_dict() => Some(o.dict()?),
                    V::Func(f) if f.kind.get() != 3 => Some(getattr(&x, "__dict__")?),
                    _ => None,
                };
                if let Some(d) = own {
                    for k in to_vec(&d)? {
                        let k = to_str(&k)?;
                        let k = sym(&k);
                        if !ns.contains(&k) {
                            ns.push(k);
                        }
                    }
                }
                // object.__dir__ reads __class__, which only an instance of a program class can change
                let k = match &x {
                    V::Obj(_) => match getattr(&x, "__class__")? {
                        V::Class(k) => k,
                        _ => cls,
                    },
                    _ => cls,
                };
                merge_class_names(&k, &mut ns)?;
            }
        }
        ns.into_iter().map(|n| s(n)).collect()
    };
    let l = list(std::mem::take(&mut names));
    call_method(&l, "sort", Args::p(vec![]))?;
    Ok(l)
}

// ABCMeta's _abc_impl: CPython's _abc._abc_data (Brevi keeps the registry and caches on the class itself)
pub struct AbcData;
impl Ext for AbcData {
    fn tname(&self) -> &'static str {
        "_abc._abc_data"
    }
    fn repr(&self) -> String {
        format!("<_abc._abc_data object at 0x{:x}>", self as *const AbcData as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}
