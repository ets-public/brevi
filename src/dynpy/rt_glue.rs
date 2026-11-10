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
// `globals()[k]`, `globals().get(k[, d])` and `k in globals()` read the module's
// global slots through a name -> slot table. Anything else on globals() is refused
// when the program is built. Keys the table cannot answer (dunder names CPython
// sets itself, keys of other types) stop the run rather than give a different answer.
fn glob_slot(tab: fn(&str) -> Option<usize>, k: &V) -> Result<Option<usize>, E> {
    match k {
        V::Str(x) => {
            if let Some(i) = tab(&x.s) {
                return Ok(match gref(i) {
                    V::Unbound => None,
                    _ => Some(i),
                });
            }
            let n = &x.s;
            if n.len() > 4 && n.starts_with("__") && n.ends_with("__") {
                return Err(unsupported(&format!("globals() lookup of '{n}'")));
            }
            Ok(None)
        }
        V::Obj(_) | V::Ext(_) => {
            py_hash(k)?;
            Err(unsupported("a globals() key that is not a str"))
        }
        _ => {
            py_hash(k)?;
            Ok(None)
        }
    }
}
pub fn glob_item(tab: fn(&str) -> Option<usize>, k: &V) -> R {
    match glob_slot(tab, k)? {
        Some(i) => Ok(gr(i)),
        None => Err(exc_v("KeyError", k.clone())),
    }
}
pub fn glob_get(tab: fn(&str) -> Option<usize>, k: &V, d: V) -> R {
    Ok(match glob_slot(tab, k)? {
        Some(i) => gr(i),
        None => d,
    })
}
pub fn glob_has(tab: fn(&str) -> Option<usize>, k: &V) -> Result<bool, E> {
    Ok(glob_slot(tab, k)?.is_some())
}
