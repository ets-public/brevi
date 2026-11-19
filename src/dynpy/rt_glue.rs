// ---------- glue used by emitted programs ----------

pub enum Flow {
    Normal,
    Ret(V),
    Brk,
    Cont,
    Exc(E),
}
pub static mut GL: Vec<V> = Vec::new();
pub static mut K: Vec<V> = Vec::new();

#[inline]
fn gref(i: usize) -> &'static mut V {
    unsafe { &mut (&mut *std::ptr::addr_of_mut!(GL))[i] }
}
#[inline]
pub fn gl(i: usize, name: &str) -> R {
    match gref(i) {
        V::Unbound => name_err(name),
        v => Ok(v.clone()),
    }
}
// Global slot i of a module read as an attribute of the module object (`m.n`): unbound is the module's
// AttributeError (the emitter's mod_attr_err).
pub fn gla(i: usize, err: impl FnOnce() -> E) -> R {
    match gref(i) {
        V::Unbound => Err(err()),
        v => Ok(v.clone()),
    }
}
// CPython 3.13's AttributeError for name n missing from module m (Objects/moduleobject.c, module_getattro).
// state: the module's import (0 not run, 1 running, 2 done); sub: the same for its submodule named n (0 when n
// names none); shadow: 1 when its file could shadow a library (a top-level module of the script's directory,
// sys.path[0]), 2 when that library is in sys.stdlib_module_names; origin: its file.
pub fn mod_attr_err(m: &str, n: &str, state: u8, sub: u8, shadow: u8, origin: &str) -> E {
    exc(
        "AttributeError",
        if shadow == 2 {
            format!("module '{m}' has no attribute '{n}' (consider renaming '{origin}' since it has the same name as the standard library module named '{m}' and prevents importing that standard library module)")
        } else if state == 1 && shadow == 1 {
            format!("module '{m}' has no attribute '{n}' (consider renaming '{origin}' if it has the same name as a library you intended to import)")
        } else if state == 1 {
            format!("partially initialized module '{m}' has no attribute '{n}' (most likely due to a circular import)")
        } else if sub == 1 {
            format!("cannot access submodule '{n}' of module '{m}' (most likely due to a circular import)")
        } else {
            format!("module '{m}' has no attribute '{n}'")
        },
    )
}
// a global that may shadow a builtin
pub fn glb(i: usize, name: &str) -> R {
    match gref(i) {
        V::Unbound => match builtin_value(name) {
            Some(v) => Ok(v),
            None => {
                let n = match name {
                    "IOError" | "EnvironmentError" => "OSError",
                    x => x,
                };
                if n.ends_with("Error") || n.ends_with("Exception") || n.ends_with("Warning") || matches!(n, "SystemExit" | "KeyboardInterrupt" | "StopIteration" | "GeneratorExit") {
                    Ok(V::Class(exc_class(n)))
                } else {
                    name_err(name)
                }
            }
        },
        v => Ok(v.clone()),
    }
}
#[inline]
pub fn gr(i: usize) -> V {
    gref(i).clone()
}
#[inline]
pub fn gs(i: usize, v: V) {
    *gref(i) = v;
}
/// `del x` on a global: builtins cannot be deleted, and an unbound slot is NameError.
#[inline]
pub fn gdel(i: usize, name: &str) -> Result<(), E> {
    if matches!(gref(i), V::Unbound) {
        return Err(exc("NameError", format!("name '{name}' is not defined")));
    }
    *gref(i) = V::Unbound;
    Ok(())
}
#[inline]
pub fn kc(i: usize) -> V {
    unsafe { (&*std::ptr::addr_of!(K))[i].clone() }
}
#[inline]
pub fn ld(v: &V, name: &str) -> R {
    match v {
        V::Unbound => Err(exc("UnboundLocalError", format!("cannot access local variable '{name}' where it is not associated with a value"))),
        v => Ok(v.clone()),
    }
}
// a local read only to be borrowed (the emitter turns `&ld(..)?` into this)
#[inline(always)]
pub fn ldr<'a>(v: &'a V, name: &str) -> Result<&'a V, E> {
    match v {
        V::Unbound => Err(exc("UnboundLocalError", format!("cannot access local variable '{name}' where it is not associated with a value"))),
        v => Ok(v),
    }
}
// For a nested function that calls itself: it holds its own cell, and the cell holds it, so neither is freed
// (CPython's cycle collector frees them). Dropped as the enclosing function returns: when the cell still holds
// the function the def made (mark), nothing else holds that function, and only the frame, this guard and the
// function itself hold the cell, both are unreachable, so the cell is emptied.
pub struct CellGuard(Rc<RefCell<V>>, Cell<usize>);
impl CellGuard {
    pub fn new(c: &Rc<RefCell<V>>) -> CellGuard {
        CellGuard(Rc::clone(c), Cell::new(0))
    }
    pub fn mark(&self) {
        if let V::Func(f) = &*self.0.borrow() {
            self.1.set(Rc::as_ptr(f) as *const u8 as usize);
        }
    }
}
impl Drop for CellGuard {
    fn drop(&mut self) {
        if Rc::strong_count(&self.0) != 3 {
            return;
        }
        let only = matches!(&*self.0.borrow(), V::Func(f) if Rc::strong_count(f) == 1 && Rc::as_ptr(f) as *const u8 as usize == self.1.get());
        if only {
            let v = std::mem::replace(&mut *self.0.borrow_mut(), V::Unbound);
            drop(v);
        }
    }
}
pub fn cell() -> Rc<RefCell<V>> {
    Rc::new(RefCell::new(V::Unbound))
}
#[inline]
pub fn cget(c: &Rc<RefCell<V>>, name: &str) -> R {
    match &*c.borrow() {
        V::Unbound => Err(exc("NameError", format!("cannot access free variable '{name}' where it is not associated with a value in enclosing scope"))),
        v => Ok(v.clone()),
    }
}
/// A read of a cell this function owns (a local that an inner function captures): unbound is UnboundLocalError.
#[inline]
pub fn cget_own(c: &Rc<RefCell<V>>, name: &str) -> R {
    match &*c.borrow() {
        V::Unbound => Err(exc("UnboundLocalError", format!("cannot access local variable '{name}' where it is not associated with a value"))),
        v => Ok(v.clone()),
    }
}
// a free variable that only its own function assigns, read from a function nested in it: as cget, or borrowed in
// place (cgsr) where the value is only borrowed. Nothing assigns the cell while the nested function runs.
#[inline]
pub fn cgs(c: &Rc<RefCell<V>>, name: &str) -> R {
    cget(c, name)
}
#[inline]
pub fn cgsr<'a>(c: &'a Rc<RefCell<V>>, name: &str) -> Result<std::cell::Ref<'a, V>, E> {
    let r = c.borrow();
    if matches!(&*r, V::Unbound) {
        return Err(exc("NameError", format!("cannot access free variable '{name}' where it is not associated with a value in enclosing scope")));
    }
    Ok(r)
}
// a free variable's cell holds a value (before a direct call of the nested def it names)
#[inline]
pub fn cbound(c: &Rc<RefCell<V>>, name: &str) -> Result<(), E> {
    if matches!(&*c.borrow(), V::Unbound) {
        return Err(exc("NameError", format!("cannot access free variable '{name}' where it is not associated with a value in enclosing scope")));
    }
    Ok(())
}
#[inline]
pub fn cset(c: &Rc<RefCell<V>>, v: V) {
    *c.borrow_mut() = v;
}
pub fn name_err(name: &str) -> R {
    Err(exc("NameError", format!("name '{name}' is not defined")))
}
#[macro_export]
macro_rules! tq {
    ($e:expr, $l:lifetime) => {
        match $e {
            Ok(v) => v,
            Err(x) => break $l Err(x),
        }
    };
}
#[macro_export]
macro_rules! fq {
    ($e:expr, $l:lifetime) => {
        match $e {
            Ok(v) => v,
            Err(x) => break $l Flow::Exc(x),
        }
    };
}
pub fn set_of_vec(v: Vec<V>) -> R {
    let mut p = PSet::new();
    for x in v {
        p.add(x)?;
    }
    Ok(V::Set(Rc::new(RefCell::new(p))))
}
// CPython folds a set display of 3 or more constants into a frozenset, then rebuilds that
// frozenset by inserting its keys in iteration order (merge_consts_recursive), and the
// display copies that rebuilt table.  Reinserting in iteration order can lay the table out
// differently from inserting in source order.
pub fn fset_rebuild(v: V) -> R {
    let keys = match &v {
        V::Set(s) => s.borrow().keys(),
        V::FSet(s) => s.keys(),
        _ => return Err(exc("TypeError", "set display fold needs a set".into())),
    };
    Ok(V::FSet(Rc::new(PSet::from_iter(keys)?)))
}
// CPython rebuilds the frozenset once more when the code object interns one of its strings (codeobject.c
// intern_constants; emit.rs set_rebuilds decides when)
pub fn fset_rebuild2(v: V) -> R {
    fset_rebuild(fset_rebuild(v)?)
}
// Whether CPython rebuilds the frozenset a second time depends on the strings interned before the module was
// compiled, which the build cannot know (a module Brevi copies, imported at run time). With a random hash key any
// layout is one CPython could give; with a fixed one (PYTHONHASHSEED) the program stops when the two layouts
// iterate differently.
pub fn fset_rebuild_unsure(v: V) -> R {
    let one = fset_rebuild(v)?;
    if !HASH_RANDOM.load(std::sync::atomic::Ordering::Relaxed) {
        let two = fset_rebuild(one.clone())?;
        let (V::FSet(a), V::FSet(b)) = (&one, &two) else { unreachable!() };
        if a.keys().iter().zip(b.keys().iter()).any(|(x, y)| !is(x, y)) {
            return Err(unsupported("a set literal of strings in a library module whose order CPython takes from the strings already interned when it imports the module"));
        }
    }
    Ok(one)
}
pub fn make_exc(v: V) -> Result<E, E> {
    match &v {
        V::Class(c) => {
            let o = instantiate(c, Args::p(vec![]))?;
            make_exc_obj(o)
        }
        _ => make_exc_obj(v),
    }
}
// `raise X from Y`
pub fn set_cause(e: &E, c: V) -> Result<(), E> {
    let c = match c {
        V::None => V::None,
        V::Class(_) => make_exc(c)?.0,
        v => match make_exc_obj(v) {
            Ok(x) => x.0,
            Err(_) => return Err(exc("TypeError", "exception causes must derive from BaseException".into())),
        },
    };
    if let V::Obj(o) = &e.0 {
        o.set("__cause__", c);
        o.set("__suppress_context__", V::Bool(true));
    }
    Ok(())
}
fn make_exc_obj(v: V) -> Result<E, E> {
    if isinstance(&v, &V::Class(exc_class("BaseException")))? {
        Ok(E(v))
    } else {
        Err(exc("TypeError", "exceptions must derive from BaseException".into()))
    }
}
pub fn exc_match(e: &V, t: &V) -> Result<bool, E> {
    isinstance(e, t)
}
#[inline]
pub fn range_int(v: &V) -> Result<i64, E> {
    match v {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Obj(_) => match to_int(v)? {
            V::Int(i) => Ok(i),
            _ => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        },
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        _ => Err(exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(v)))),
    }
}

// ---------- globals() ----------
// A module's globals as `globals()` and enum's _convert_ and global_enum see them: the file's global slots through a
// name -> slot table, and, for names no slot has, the keys set at run time (`globals()[k] = v` with a key known only
// then, enum.global_enum exporting members). Keys the table cannot answer (dunder names CPython sets itself, keys
// of other types) stop the run rather than give a different answer. A store by key never changes a name the build
// fixed (`fixed`: every name a def, class or import of the file binds, which the build may call, alias or read
// directly), never adds a name the file reads by name without binding it (`free`: the build compiled those reads as
// builtins or NameError), and is refused in a program with __del__ (whose exit order follows the slots).
pub struct GMod {
    pub name: &'static str,
    pub tab: fn(&str) -> Option<usize>,
    // every slot name, sorted
    pub names: &'static [&'static str],
    pub fixed: &'static [&'static str],
    pub free: &'static [&'static str],
    pub dyn_ok: bool,
}
// keys set at run time that no slot has: (module, name, value), in the order first set
pub static mut GOVER: Vec<(&'static str, String, V)> = Vec::new();
thread_local! {
    pub static GMODS: std::cell::Cell<&'static [&'static GMod]> = const { std::cell::Cell::new(&[]) };
}
fn gover() -> &'static mut Vec<(&'static str, String, V)> {
    unsafe { &mut *std::ptr::addr_of_mut!(GOVER) }
}
pub fn gmod_named(m: &str) -> Option<&'static GMod> {
    GMODS.with(|g| g.get()).iter().copied().find(|g| g.name == m)
}
// a run-time key of module m, as `sys.modules[m].__dict__` holds it
pub fn gover_get(m: &str, n: &str) -> Option<V> {
    gover().iter().find(|(mm, k, _)| *mm == m && k == n).map(|x| x.2.clone())
}
enum GHit {
    Slot(usize),
    Over(V),
    No,
}
fn is_dunder_key(n: &str) -> bool {
    n.len() > 4 && n.starts_with("__") && n.ends_with("__")
}
fn glob_find(g: &GMod, k: &V) -> Result<GHit, E> {
    match k {
        V::Str(x) => {
            let n = &x.text();
            if let Some(i) = (g.tab)(n) {
                return Ok(match gref(i) {
                    V::Unbound => GHit::No,
                    _ => GHit::Slot(i),
                });
            }
            if is_dunder_key(n) {
                return Err(unsupported(&format!("globals() lookup of '{n}'")));
            }
            Ok(match gover_get(g.name, n) {
                Some(v) => GHit::Over(v),
                None => GHit::No,
            })
        }
        V::Obj(_) | V::Ext(_) => {
            py_hash(k)?;
            Err(unsupported("a globals() key that is not a str"))
        }
        _ => {
            py_hash(k)?;
            Ok(GHit::No)
        }
    }
}
pub fn glob_item(g: &GMod, k: &V) -> R {
    match glob_find(g, k)? {
        GHit::Slot(i) => Ok(gr(i)),
        GHit::Over(v) => Ok(v),
        GHit::No => Err(exc_v("KeyError", k.clone())),
    }
}
#[inline]
pub fn glob_get(g: &GMod, k: &V, d: V) -> R {
    Ok(match glob_find(g, k)? {
        GHit::Slot(i) => gr(i),
        GHit::Over(v) => v,
        GHit::No => d,
    })
}
pub fn glob_has(g: &GMod, k: &V) -> Result<bool, E> {
    Ok(!matches!(glob_find(g, k)?, GHit::No))
}
// `globals()[k] = v` (and the stores of enum's _convert_ and global_enum) for a key the build did not give a slot of its own
pub fn glob_set(g: &GMod, k: &V, v: V) -> Result<(), E> {
    let V::Str(x) = k else {
        py_hash(k)?;
        return Err(unsupported("a globals() key that is not a str"));
    };
    let n = x.text().to_string();
    glob_set_name(g, &n, v)
}
pub fn glob_set_name(g: &GMod, n: &str, v: V) -> Result<(), E> {
    if !g.dyn_ok {
        return Err(unsupported(&format!("setting the global '{n}' by key in a program with __del__")));
    }
    if is_dunder_key(n) {
        return Err(unsupported(&format!("globals() store to '{n}'")));
    }
    if g.fixed.binary_search(&n).is_ok() {
        return Err(unsupported(&format!("rebinding '{n}' (bound by a def, class or import, which the build fixed) by key")));
    }
    if let Some(i) = (g.tab)(n) {
        gs(i, v);
        return Ok(());
    }
    if g.free.binary_search(&n).is_ok() {
        return Err(unsupported(&format!("adding the global '{n}' by key, which the program reads by name")));
    }
    let o = gover();
    match o.iter_mut().find(|(mm, kk, _)| *mm == g.name && kk == n) {
        Some(e) => e.2 = v,
        None => o.push((g.name, n.to_string(), v)),
    }
    Ok(())
}
// The module's (name, value) pairs as its __dict__ holds them now, without the dunder names CPython sets itself;
// in no particular order (callers must not let the order show).
pub fn glob_pairs(g: &GMod) -> Vec<(String, V)> {
    let mut out: Vec<(String, V)> = vec![];
    for n in g.names {
        if let Some(i) = (g.tab)(n) {
            if !matches!(gref(i), V::Unbound) {
                out.push((n.to_string(), gr(i)));
            }
        }
    }
    for (m, k, v) in gover().iter() {
        if *m == g.name {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

// ---------- sys._getframe(n).f_globals ----------
// The build lets a frame from sys._getframe(n) (n a literal) be used only as
// `frame.f_globals.get('__warningregistry__'[, d])` (emit.rs frame_vars); the frame is never a value. Which
// module's globals those are is not known here, so the key is answered only while its answer is the same in
// every module: absent, while no module has a registry yet (test.support's check_warnings reads it). Anything
// else stops the run. n must reach no further than the program's own frames (PDEPTH, each of which CPython has too), and
// not past a call from a Python library (whose frames the program does not know).
pub fn getframe_check(n: i64) -> Result<(), E> {
    if GT_CB.with(|c| c.get()) > 0 {
        return Err(unsupported("sys._getframe() in a function a Python library called"));
    }
    // the program's own frames, and __main__'s module frame while its body runs (a thread running then has
    // threading's frames under it instead)
    let known = PDEPTH.with(|d| d.get()) + MAIN_RUNNING.with(|m| m.get()) as usize;
    if n > 0 && n as usize >= known {
        return Err(unsupported("sys._getframe(n) with n past the program's own frames"));
    }
    Ok(())
}
fn fglob_absent(k: &V) -> Result<(), E> {
    match k {
        V::Str(x) if x.text() == "__warningregistry__" => {
            if no_warn_registry()? {
                Ok(())
            } else {
                Err(unsupported("frame.f_globals['__warningregistry__'] once a warning was issued"))
            }
        }
        _ => Err(unsupported("reading a frame's f_globals other than '__warningregistry__'")),
    }
}
pub fn fglob_get(k: &V, d: V) -> R {
    fglob_absent(k)?;
    Ok(d)
}

thread_local! {
    // the classes whose creation is running user code (an Enum's members made by its own __new__ and __init__),
    // innermost last: a zero-argument super() in that code finds its class here before the name is bound
    pub static BUILDING: RefCell<Vec<Rc<Class>>> = RefCell::new(vec![]);
}
// The class of a zero-argument super(): the value of the class's name (`v`, read without a check), else the class
// of that name being made; kind 0 a global, 1 a free variable, 2 a local.
pub fn super_cls(v: V, name: &str, kind: u8) -> R {
    if !matches!(v, V::Unbound) {
        return Ok(v);
    }
    let short = name.rsplit('.').next().unwrap_or(name);
    if let Some(c) = BUILDING.with(|b| b.borrow().iter().rev().find(|c| c.name == short).cloned()) {
        return Ok(V::Class(c));
    }
    match kind {
        0 => name_err(name),
        1 => Err(exc("NameError", format!("cannot access free variable '{name}' where it is not associated with a value in enclosing scope"))),
        _ => Err(exc("UnboundLocalError", format!("cannot access local variable '{name}' where it is not associated with a value"))),
    }
}
pub fn gl_raw(i: usize) -> V {
    gref(i).clone()
}
