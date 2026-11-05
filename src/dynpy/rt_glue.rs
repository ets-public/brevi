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
pub fn make_exc(v: V) -> Result<E, E> {
    match &v {
        V::Class(c) => {
            let o = instantiate(c, Args::p(vec![]))?;
            make_exc_obj(o)
        }
        _ => make_exc_obj(v),
    }
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
