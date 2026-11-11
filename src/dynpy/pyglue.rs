// ------------------------------------------------------------------------------------------- dynamic-build bridge
// Pasted after pyrt_load.rs into a dynamic build (see mod.rs) that imports Python libraries Brevi does not have
// (requests, pydantic, yaml, ...). Never part of the dynamic runtime itself, which reaches no file, process or
// CPython (caps.rs tests that).
//
// A library value is a `PyV` (an Ext value holding one reference to a Python object), and every operation on
// one runs in CPython: attributes, calls, items, operators, iteration, str/repr/format. Values cross like this:
//   - into Python: None, bool, int, float, str, bytes, tuples, lists, dicts and sets are copied; a list or dict
//     the library changed (`lib.fill(xs)`) is copied back after the call; functions, classes and iterators of
//     the program go in as Python callables that call back into it; exceptions as Python exceptions.
//   - out of Python: None, bool, int, float, str and bytes become the program's own values, and so do tuples
//     and, from a call's result (`r.json()`, `m.model_dump()`), lists, dicts and sets, copied. A list, dict or
//     set reached through an attribute or item (`post.tags`) stays the library's object, so changing it
//     (`post.tags.append(x)`) changes what the library sees, as in CPython.
//   - a Python exception becomes an exception of the program whose class has the Python class's name, module and
//     nearest builtin base (`except KeyError`, `except requests.HTTPError`, `type(e).__name__` all work).
// Classes built on a library's classes (`class Post(BaseModel)`) are run by CPython whole: see pyd_class.

use std::ffi::{c_char, c_int, CString};
use ax_pyffi::P;

static PYD_API: std::sync::OnceLock<Result<ax_pyffi::Api, String>> = std::sync::OnceLock::new();
// the type of _brevi_dyn's _BrIt, once one has been made
static PYD_BRIT: std::sync::atomic::AtomicPtr<std::ffi::c_void> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
static PYD_MAIN: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();
const PYD_CAP: &std::ffi::CStr = c"brevi.dyn.callable";

const PYD_HELPERS: &str = r#"
import builtins as _b, operator as _o, types as _t, sys as _s
_d = _t.ModuleType('_brevi_dyn')
class _BrIt:
    __slots__ = ('f', 'r', 'b')
    def __init__(s, f, r, b): s.f = f; s.r = r; s.b = b
    def __iter__(s): return s
    def __next__(s): return s.f()
    def __reduce__(s): return s.r()
def _pyclass(mod, imports, src, name, file):
    m = _s.modules.get(mod) if mod == '__main__' else _d.mods.get(mod)
    if m is None:
        m = _t.ModuleType(mod)
        _d.mods[mod] = m
    ns = m.__dict__
    if mod not in _d.done:
        _d.done.add(mod)
        exec(compile(imports, file, 'exec'), ns)
    exec(compile(src, file, 'exec'), ns)
    return ns[name]
def _flush_files():
    import gc, io
    for f in gc.get_objects():
        try:
            if isinstance(f, io.IOBase) and not f.closed:
                f.flush()
        except Exception:
            pass
def _atexit():
    try:
        import atexit
        atexit._run_exitfuncs()
    except Exception:
        pass
    _flush_files()
_d.mods = {}
_d.done = set()
_d.nts = {}
def _nt(cls, name, fields, values):
    c = _d.nts.get(cls)
    if c is None:
        import collections as _co
        c = _co.namedtuple(name, fields, module='_brevi_dyn')
        setattr(_d, name, c)
        _d.nts[cls] = c
    return c(*values)
# A warning Brevi's runtime issues (re's DeprecationWarning/FutureWarning) at the program's line, as
# warnings.warn would from that frame: one registry per module, like each module's __warningregistry__.
def _warn_at(msg, cat, file, line, module):
    import warnings
    warnings.warn_explicit(msg, getattr(_b, cat), file, line, module, _d.regs.setdefault(module, {}))
_d.regs = {}
_d.warn_at = _warn_at
_d.it = _BrIt
_d.pyclass = _pyclass
_d._nt = _nt
_d.atexit = _atexit
_d.contains = lambda a, b: b in a
_d.op = lambda n, a, b: getattr(_o, n)(a, b)
_d.un = lambda n, a: getattr(_o, n)(a)
_d.mro = lambda t: [(c.__name__, c.__module__) for c in t.__mro__]
# Pickling a function or class of the program: by reference, as CPython pickles a def or class (its
# __module__ and __qualname__, looked up again when loaded), checked to be the same object when dumped.
# The pickle names _brevi_dyn._glob, which resolves the name in the program, so it loads in this program.
def _glob(m, q):
    return _d._lookup(m, q)
_glob.__module__ = '_brevi_dyn'
_d._glob = _glob
def _bf_reduce(f):
    r = _d._ref(f)
    if r is None:
        return f.__reduce__()
    if r[0] == 1:
        import pickle
        raise pickle.PicklingError(r[1])
    if r[0] == 2:
        raise AttributeError(r[1])
    return (_glob, (r[1], r[2]))
import copyreg as _cr
_cr.dispatch_table[type(len)] = _bf_reduce
_s.modules['_brevi_dyn'] = _d
"#;

extern "C" {
    fn atexit(f: extern "C" fn()) -> c_int;
}

fn pyd_api() -> &'static ax_pyffi::Api {
    match PYD_API.get_or_init(|| {
        PYD_MAIN.set(std::thread::current().id()).ok();
        let a = ax_pyb_load()?;
        let g = unsafe { (a.GilEnsure)() };
        let c = CString::new(PYD_HELPERS).unwrap();
        let ok = unsafe { (a.RunSimple)(c.as_ptr()) } == 0;
        unsafe { (a.GilRelease)(g) };
        if !ok {
            return Err("Python did not start".into());
        }
        if unsafe { !pyd_register(&a) } {
            return Err("Python did not start".into());
        }
        unsafe { atexit(pyd_at_exit) };
        PY_DT.with(|h| h.set(Some(pyd_dt)));
        Ok(a)
    }) {
        Ok(a) => a,
        Err(e) => {
            out_flush();
            let fix = if e.starts_with("brevi: --sandbox:") { "change what the message names, or run without --sandbox" } else { "run the program with python3, or install a Python that has its shared library (libpython)" };
            eprintln!("{{\"e\":\"runtime\",\"msg\":{e:?},\"fix\":{fix:?}}}");
            std::process::exit(1)
        }
    }
}

// What CPython does on the way out: atexit functions, and files still open written out.
extern "C" fn pyd_at_exit() {
    let Some(Ok(a)) = PYD_API.get() else { return };
    unsafe {
        let g = (a.GilEnsure)();
        let m = (a.Import)(c"_brevi_dyn".as_ptr());
        if !m.is_null() {
            let f = (a.GetAttr)(m, c"atexit".as_ptr());
            if !f.is_null() {
                let t = (a.TupleNew)(0);
                let r = (a.Call)(f, t, std::ptr::null_mut());
                if !r.is_null() {
                    (a.DecRef)(r);
                }
                (a.DecRef)(t);
                (a.DecRef)(f);
            }
            (a.DecRef)(m);
        }
        (a.ErrClear)();
        (a.GilRelease)(g);
    }
}

// Holds Python's interpreter lock while alive.
struct PydGil(c_int);
impl PydGil {
    #[inline]
    fn take() -> PydGil {
        // Anything Python prints comes after what the program printed before.
        out_flush();
        PydGil(unsafe { (pyd_api().GilEnsure)() })
    }
}
impl Drop for PydGil {
    fn drop(&mut self) {
        unsafe { (pyd_api().GilRelease)(self.0) }
    }
}

// A Python object (one reference). `live`: a list, dict or set the library holds, kept rather than copied.
pub struct PyV {
    p: P,
    live: bool,
    tn: Cell<Option<Sym>>,
}
impl Drop for PyV {
    fn drop(&mut self) {
        if !self.p.is_null() {
            let _g = PydGil::take();
            unsafe { (pyd_api().DecRef)(self.p) }
        }
    }
}

fn pyv(p: P, live: bool) -> V {
    V::Ext(ExtRc::new(PyV { p, live, tn: Cell::new(None) }))
}
fn pyv_of(v: &V) -> Option<&PyV> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<PyV>(),
        _ => None,
    }
}

// The error Python raised, as an exception of the program.
fn pyd_err() -> E {
    let a = pyd_api();
    let _g = PydGil::take();
    unsafe {
        if (a.ErrOccurred)().is_null() {
            return exc("SystemError", "a Python library failed without an error".into());
        }
        let (mut t, mut v, mut tb) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        (a.ErrFetch)(&mut t, &mut v, &mut tb);
        (a.ErrNormalize)(&mut t, &mut v, &mut tb);
        for x in [t, tb] {
            if !x.is_null() {
                (a.DecRef)(x);
            }
        }
        if v.is_null() {
            return exc("SystemError", "a Python library failed without an error".into());
        }
        pyd_exc_of(v)
    }
}

// The program's exception for Python exception `v` (consumed).
unsafe fn pyd_exc_of(v: P) -> E {
    let a = pyd_api();
    // One of the program's own, raised by a function of it that a library called.
    let b = (a.GetAttr)(v, c"__brevi__".as_ptr());
    if b.is_null() {
        (a.ErrClear)();
    } else {
        let orig = pyd_callable_v(b);
        (a.DecRef)(b);
        if let Some(o) = orig {
            (a.DecRef)(v);
            return E(o);
        }
    }
    let cls = pyd_exc_class(v);
    let o = Obj::new(cls.clone());
    let args = (a.GetAttr)(v, c"args".as_ptr());
    let args = if args.is_null() {
        (a.ErrClear)();
        tup(vec![])
    } else {
        pyd_wrap(args, true).unwrap_or_else(|_| tup(vec![]))
    };
    o.set("args", args);
    if cls.is_sub(&exc_class("SystemExit")) {
        let c = (a.GetAttr)(v, c"code".as_ptr());
        if c.is_null() {
            (a.ErrClear)();
        } else {
            o.set("code", pyd_wrap(c, true).unwrap_or(V::None));
        }
    }
    o.set("__pyexc__", pyv(v, false));
    E(V::Obj(o))
}

thread_local! {
    // Python exception class (name, module) -> the program's class for it.
    static PYD_EXC: RefCell<std::collections::HashMap<(String, String), Rc<Class>>> = RefCell::new(std::collections::HashMap::new());
}

unsafe fn pyd_exc_class(v: P) -> Rc<Class> {
    let a = pyd_api();
    let t = (a.Type)(v);
    let mro = pyd_helper("mro", &[t]).ok().and_then(|m| to_vec(&m).ok()).unwrap_or_default();
    (a.DecRef)(t);
    let names: Vec<(String, String)> = mro.iter().filter_map(|x| to_vec(x).ok()).filter(|p| p.len() == 2).map(|p| (to_str(&p[0]).unwrap_or_default(), to_str(&p[1]).unwrap_or_default())).collect();
    let known = |n: &str| BUILTIN_CLASSES.with(|b| b.borrow().get(n).is_some_and(|c| c.bt == BT::Exc || c.is_sub(&exc_class("BaseException"))));
    let base = names.iter().find(|(n, m)| m == "builtins" && known(n)).map(|(n, _)| exc_class(n)).unwrap_or_else(|| exc_class("Exception"));
    let Some((name, module)) = names.first().cloned() else { return base };
    if module == "builtins" && known(&name) {
        return base;
    }
    if let Some(c) = PYD_EXC.with(|m| m.borrow().get(&(name.clone(), module.clone())).cloned()) {
        return c;
    }
    let c = Class::new(sym(&name), sym(&module), vec![base.clone()], base.bt);
    let py = |o: &V| -> Option<V> {
        match o {
            V::Obj(o) => o.get("__pyexc__"),
            _ => None,
        }
    };
    c.set("__str__", func(sym("__str__"), move |a: Args| match a.p.first().and_then(py) {
        Some(x) => Ok(st(to_str(&x)?)),
        None => Ok(s("")),
    }));
    c.set("__getattr__", func(sym("__getattr__"), move |a: Args| {
        let n = to_str(a.p.get(1).unwrap_or(&V::None))?;
        match a.p.first().and_then(py) {
            Some(x) => getattr(&x, &n),
            None => Err(exc("AttributeError", n)),
        }
    }));
    PYD_EXC.with(|m| m.borrow_mut().insert((name, module), c.clone()));
    c
}

#[inline]
fn pyd_own(p: P) -> Result<P, E> {
    if p.is_null() { Err(pyd_err()) } else { Ok(p) }
}

unsafe fn pyd_str_obj(s: &str) -> P {
    (pyd_api().UniFrom)(s.as_ptr() as *const c_char, s.len() as isize)
}

unsafe fn pyd_text(p: P) -> Result<String, E> {
    let mut n: isize = 0;
    let c = (pyd_api().UniAs)(p, &mut n);
    if c.is_null() {
        return Err(pyd_err());
    }
    Ok(String::from_utf8_lossy(std::slice::from_raw_parts(c as *const u8, n as usize)).into_owned())
}

// A function of _brevi_dyn called with borrowed arguments; its result as the program's value (copied).
unsafe fn pyd_helper(name: &str, args: &[P]) -> R {
    let a = pyd_api();
    let m = pyd_own((a.Import)(c"_brevi_dyn".as_ptr()))?;
    let c = CString::new(name).unwrap();
    let f = (a.GetAttr)(m, c.as_ptr());
    (a.DecRef)(m);
    let f = pyd_own(f)?;
    let t = (a.TupleNew)(args.len() as isize);
    for (i, x) in args.iter().enumerate() {
        (a.IncRef)(*x);
        (a.TupleSet)(t, i as isize, *x);
    }
    let r = (a.Call)(f, t, std::ptr::null_mut());
    (a.DecRef)(t);
    (a.DecRef)(f);
    pyd_wrap(pyd_own(r)?, true)
}

// warnings.warn_explicit(msg, <builtin category>, file, line, module) through CPython's warnings module, so
// its filters, catch_warnings and showwarning see it; a filter that makes it an error raises here.
pub fn pyd_warn_at(cat: &str, msg: &str, file: &str, line: usize, module: &str) -> Result<(), E> {
    unsafe {
        let _g = PydGil::take();
        let a = pyd_api();
        let mut ps = vec![];
        for v in [st(msg.to_string()), st(cat.to_string()), st(file.to_string()), V::Int(line as i64), st(module.to_string())] {
            match pyd_to(&v) {
                Ok(p) => ps.push(p),
                Err(e) => {
                    for p in ps {
                        (a.DecRef)(p);
                    }
                    return Err(e);
                }
            }
        }
        let r = pyd_helper("warn_at", &ps);
        for p in ps {
            (a.DecRef)(p);
        }
        r.map(|_| ())
    }
}
unsafe fn pyd_type_is(p: P, t: P) -> bool {
    let a = pyd_api();
    let x = (a.Type)(p);
    (a.DecRef)(x);
    x == t
}

// (__name__, __module__) of the type CPython gives iter(p), for type() of a Brevi iterator over a
// bridged value (a proxy list iterates as list_iterator, and so on).
unsafe fn pyd_iter_type(p: P) -> Option<(String, String)> {
    let a = pyd_api();
    let it = (a.GetIter)(p);
    if it.is_null() {
        (a.ErrClear)();
        return None;
    }
    let t = (a.Type)(it);
    (a.DecRef)(it);
    let get = |attr: &'static std::ffi::CStr| -> Option<String> {
        let v = (a.GetAttr)(t, attr.as_ptr());
        if v.is_null() {
            (a.ErrClear)();
            return None;
        }
        let s = pyd_text(v).ok();
        (a.DecRef)(v);
        s
    };
    let name = get(c"__name__");
    let module = get(c"__module__").unwrap_or_else(|| "builtins".to_string());
    (a.DecRef)(t);
    name.map(|name| (name, module))
}

// Python object `p` (consumed) as the program's value: copied when it is data (deep: lists, dicts and sets too).
unsafe fn pyd_wrap(p: P, deep: bool) -> R {
    let a = pyd_api();
    if p == a.None {
        (a.DecRef)(p);
        return Ok(V::None);
    }
    let t = (a.Type)(p);
    (a.DecRef)(t);
    let done = |v: V| -> R {
        (a.DecRef)(p);
        Ok(v)
    };
    if t == a.TBool {
        let b = (a.IsTrue)(p) == 1;
        return done(V::Bool(b));
    }
    // A class (its type is `type`, whose type is itself): Python's own exception classes come back as the
    // program's, so `w.category is DeprecationWarning` and issubclass(w.category, Warning) hold as in CPython.
    let tt = (a.Type)(t);
    (a.DecRef)(tt);
    if tt == t {
        if let Some(c) = pyd_builtin_exc(p) {
            return done(V::Class(c));
        }
    }
    if t == a.TLong {
        let mut over = 0;
        let n = (a.LongAs)(p, &mut over);
        if over == 0 && !(n == -1 && !(a.ErrOccurred)().is_null()) {
            return done(V::Int(n));
        }
        (a.ErrClear)();
        let s = (a.Str)(p);
        let txt = pyd_text(pyd_own(s)?);
        (a.DecRef)(s);
        return done(parse_int(&txt?, 10).unwrap_or(V::None));
    }
    if t == a.TFloat {
        let f = (a.FloatAs)(p);
        return done(V::Float(f));
    }
    if t == a.TUni {
        let s = pyd_text(p);
        (a.DecRef)(p);
        return Ok(st(s?));
    }
    if t == a.TBytes {
        let (mut b, mut n): (*mut c_char, isize) = (std::ptr::null_mut(), 0);
        if (a.BytesAs)(p, &mut b, &mut n) < 0 {
            (a.DecRef)(p);
            return Err(pyd_err());
        }
        let v = std::slice::from_raw_parts(b as *const u8, n as usize).to_vec();
        return done(bytesv(v));
    }
    if t == a.TTuple {
        let n = (a.TupleSize)(p);
        let mut out = Vec::with_capacity(n.max(0) as usize);
        for i in 0..n {
            let x = (a.TupleGet)(p, i);
            (a.IncRef)(x);
            match pyd_wrap(x, deep) {
                Ok(v) => out.push(v),
                Err(e) => {
                    (a.DecRef)(p);
                    return Err(e);
                }
            }
        }
        return done(tup(out));
    }
    if deep && t == a.TList {
        let n = (a.ListSize)(p);
        let mut out = Vec::with_capacity(n.max(0) as usize);
        for i in 0..n {
            let x = (a.ListGet)(p, i);
            (a.IncRef)(x);
            match pyd_wrap(x, true) {
                Ok(v) => out.push(v),
                Err(e) => {
                    (a.DecRef)(p);
                    return Err(e);
                }
            }
        }
        return done(list(out));
    }
    if deep && t == a.TDict {
        let mut d = Dict::new();
        let mut pos: isize = 0;
        let (mut k, mut x): (P, P) = (std::ptr::null_mut(), std::ptr::null_mut());
        let mut items = vec![];
        while (a.DictNext)(p, &mut pos, &mut k, &mut x) != 0 {
            (a.IncRef)(k);
            (a.IncRef)(x);
            items.push((k, x));
        }
        let mut err = None;
        for (k, x) in items {
            if err.is_some() {
                (a.DecRef)(k);
                (a.DecRef)(x);
                continue;
            }
            let r = pyd_wrap(k, true).and_then(|kk| pyd_wrap(x, true).map(|xx| (kk, xx)));
            match r.and_then(|(kk, xx)| d.set(kk, xx)) {
                Ok(()) => {}
                Err(e) => err = Some(e),
            }
        }
        (a.DecRef)(p);
        return match err {
            Some(e) => Err(e),
            None => Ok(V::Dict(Rc::new(RefCell::new(d)))),
        };
    }
    if deep && (t == a.TSet || t == a.TFrozenSet) {
        let items = pyd_items(p);
        (a.DecRef)(p);
        let items = items?;
        let ps = PSet::from_iter(items)?;
        return Ok(if t == a.TSet { V::Set(Rc::new(RefCell::new(ps))) } else { V::FSet(Rc::new(ps)) });
    }
    if t == PYD_BRIT.load(std::sync::atomic::Ordering::Relaxed) {
        // an iterator of the program that went into Python (a library returned it, or types.coroutine)
        let b = (a.GetAttr)(p, c"b".as_ptr());
        if !b.is_null() {
            let v = pyd_callable_v(b);
            (a.DecRef)(b);
            if let Some(v) = v {
                (a.DecRef)(p);
                return Ok(v);
            }
        } else {
            (a.ErrClear)();
        }
    }
    if t == a.TCFunction {
        if let Some(v) = pyd_callable_v(p) {
            (a.DecRef)(p);
            return Ok(v);
        }
    }
    Ok(pyv(p, t == a.TList || t == a.TDict || t == a.TSet))
}

// Items of a Python iterable (borrowed), copied.
unsafe fn pyd_items(p: P) -> Result<Vec<V>, E> {
    let a = pyd_api();
    let it = pyd_own((a.GetIter)(p))?;
    let mut out = vec![];
    loop {
        let x = (a.IterNext)(it);
        if x.is_null() {
            break;
        }
        match pyd_wrap(x, true) {
            Ok(v) => out.push(v),
            Err(e) => {
                (a.DecRef)(it);
                return Err(e);
            }
        }
    }
    (a.DecRef)(it);
    if !(a.ErrOccurred)().is_null() {
        return Err(pyd_err());
    }
    Ok(out)
}

// The program's value a callable made by pyd_callable stands for.
unsafe fn pyd_callable_v(f: P) -> Option<V> {
    let a = pyd_api();
    if !pyd_type_is(f, a.TCFunction) {
        return None;
    }
    let me = (a.CFunctionSelf)(f);
    if me.is_null() {
        (a.ErrClear)();
        return None;
    }
    if (a.CapsuleValid)(me, PYD_CAP.as_ptr()) != 1 {
        return None;
    }
    let v = (a.CapsuleGet)(me, PYD_CAP.as_ptr()) as *const V;
    if v.is_null() { None } else { Some((*v).clone()) }
}

#[repr(C)]
struct PydDef {
    name: *const c_char,
    meth: unsafe extern "C" fn(P, P, P) -> P,
    flags: c_int,
    doc: *const c_char,
}
unsafe impl Sync for PydDef {}
static mut PYD_DEF: PydDef = PydDef { name: c"brevi_function".as_ptr(), meth: pyd_tramp, flags: 3, doc: std::ptr::null() };

unsafe extern "C" fn pyd_cap_free(c: P) {
    let a = pyd_api();
    let v = (a.CapsuleGet)(c, PYD_CAP.as_ptr()) as *mut V;
    if !v.is_null() {
        drop(Box::from_raw(v));
    }
}

// A Python callable that calls `v`, a function, class or callable object of the program.
unsafe fn pyd_callable(v: &V) -> Result<P, E> {
    pyd_callable_in(pyd_api(), v)
}
unsafe fn pyd_callable_in(a: &ax_pyffi::Api, v: &V) -> Result<P, E> {
    let b = Box::into_raw(Box::new(v.clone()));
    let cap = (a.CapsuleNew)(b as ax_pyffi::P, PYD_CAP.as_ptr(), Some(pyd_cap_free));
    if cap.is_null() {
        drop(Box::from_raw(b));
        (a.ErrClear)();
        return Err(exc("RuntimeError", "brevi: could not make a Python callable".into()));
    }
    let f = (a.CFunctionNew)(std::ptr::addr_of_mut!(PYD_DEF) as *mut ax_pyffi::MethodDef, cap, std::ptr::null_mut());
    (a.DecRef)(cap);
    if f.is_null() {
        (a.ErrClear)();
        return Err(exc("RuntimeError", "brevi: could not make a Python callable".into()));
    }
    Ok(f)
}

// _brevi_dyn._ref and _lookup (see _bf_reduce in PYD_HELPERS), set while Python starts, before pyd_api() is ready.
unsafe fn pyd_register(a: &ax_pyffi::Api) -> bool {
    let g = (a.GilEnsure)();
    let m = (a.Import)(c"_brevi_dyn".as_ptr());
    let mut ok = !m.is_null();
    for (n, f) in [(c"_ref", func(sym("_ref"), pickle_ref)), (c"_lookup", func(sym("_lookup"), pickle_lookup_fn))] {
        if !ok {
            break;
        }
        match pyd_callable_in(a, &f) {
            Ok(p) => {
                ok = (a.SetAttr)(m, n.as_ptr(), p) == 0;
                (a.DecRef)(p);
            }
            Err(_) => ok = false,
        }
    }
    if !m.is_null() {
        (a.DecRef)(m);
    }
    if !ok {
        (a.ErrClear)();
    }
    (a.GilRelease)(g);
    ok
}

// What pickle saves for a function or class of the program: None for any other callable (CPython's own
// builtins pickle as usual), (0, module, qualname) when that name finds this very object again, else the
// error CPython raises: (1, msg) a PicklingError, (2, msg) an AttributeError (a def local to a function).
fn pickle_ref(a: Args) -> R {
    let v = a.p.first().cloned().unwrap_or(V::None);
    match &v {
        V::Func(f) if f.kind.get() != 3 => {}
        V::Class(_) => {}
        V::Func(_) => return Err(unsupported("pickling a builtin function")),
        V::Bound(_) | V::BMeth(_) => return Err(unsupported("pickling a bound method")),
        _ => return Ok(V::None),
    }
    let text = |x: R| -> Result<Option<String>, E> {
        Ok(match x? {
            V::Str(t) => Some(t.s.to_string()),
            _ => None,
        })
    };
    let (Some(m), Some(q)) = (text(getattr(&v, "__module__"))?, text(getattr(&v, "__qualname__"))?) else {
        return Ok(tup(vec![V::Int(1), st(format!("Can't pickle {}: no __module__ or __qualname__", repr(&v)?))]));
    };
    if q.split('.').any(|p| p == "<locals>") {
        return Ok(tup(vec![V::Int(2), st(format!("Can't get local object '{q}'"))]));
    }
    match pickle_lookup(&m, &q) {
        Ok(found) if is(&found, &v) => Ok(tup(vec![V::Int(0), st(m), st(q)])),
        Ok(_) => Ok(tup(vec![V::Int(1), st(format!("Can't pickle {}: it's not the same object as {m}.{q}", repr(&v)?))])),
        Err(_) => Ok(tup(vec![V::Int(1), st(format!("Can't pickle {}: it's not found as {m}.{q}", repr(&v)?))])),
    }
}
fn pickle_lookup_fn(a: Args) -> R {
    let t = |i: usize| match a.p.get(i) {
        Some(V::Str(x)) => Ok(x.s.to_string()),
        _ => type_err("_glob() takes a module name and a qualified name".into()),
    };
    pickle_lookup(&t(0)?, &t(1)?)
}
// `module.qualname` of the program, as pickle finds a def or class again: the first name from the module's
// globals (__main__, an imported module of the program, or functools' total_ordering helpers), the rest as
// attributes.
fn pickle_lookup(m: &str, q: &str) -> R {
    let missing = || exc("AttributeError", format!("Can't get attribute '{q}' on <module '{m}'>"));
    if q.split('.').any(|p| p == "<locals>") {
        return Err(exc("AttributeError", format!("Can't get local attribute '{q}' on <module '{m}'>")));
    }
    let mut parts = q.split('.');
    let first = parts.next().unwrap_or("");
    let mut v = match m {
        "__main__" => match MAIN_GTAB.with(|g| g.get()) {
            Some(tab) => glob_item(tab, &st(first.to_string())).map_err(|_| missing())?,
            None => return Err(missing()),
        },
        "functools" => total_ordering_helper_named(first).ok_or_else(missing)?,
        "builtins" => builtin_value(first).ok_or_else(missing)?,
        _ => match USER_MOD.with(|u| u.get()).and_then(|f| f(m, first)) {
            Some(r) => r.map_err(|_| missing())?,
            None => return Err(missing()),
        },
    };
    for p in parts {
        v = getattr(&v, p).map_err(|_| missing())?;
    }
    Ok(v)
}

unsafe extern "C" fn pyd_tramp(me: P, args: P, kw: P) -> P {
    let a = pyd_api();
    if PYD_MAIN.get() != Some(&std::thread::current().id()) {
        (a.ErrSetString)(a.RuntimeError, c"brevi: a Python library called a function of this program from another thread, which dynamic builds do not support".as_ptr());
        return std::ptr::null_mut();
    }
    let f = (a.CapsuleGet)(me, PYD_CAP.as_ptr()) as *const V;
    if f.is_null() {
        return std::ptr::null_mut();
    }
    let f = (*f).clone();
    let r = (|| -> R {
        let mut pos = vec![];
        for i in 0..(a.TupleSize)(args) {
            let x = (a.TupleGet)(args, i);
            (a.IncRef)(x);
            pos.push(pyd_wrap(x, false)?);
        }
        let mut kws = vec![];
        if !kw.is_null() {
            let mut at: isize = 0;
            let (mut k, mut x): (P, P) = (std::ptr::null_mut(), std::ptr::null_mut());
            while (a.DictNext)(kw, &mut at, &mut k, &mut x) != 0 {
                let name = pyd_text(k)?;
                (a.IncRef)(x);
                kws.push((sym(&name), pyd_wrap(x, false)?));
            }
        }
        call(&f, Args { p: pos, k: kws })
    })();
    match r.and_then(|v| pyd_to(&v)) {
        Ok(p) => p,
        Err(e) => {
            pyd_raise(&e);
            std::ptr::null_mut()
        }
    }
}

// Sets the Python error for exception `e` of the program.
unsafe fn pyd_raise(e: &E) {
    let a = pyd_api();
    match pyd_to(&e.0) {
        Ok(x) => {
            let t = (a.Type)(x);
            (a.ErrSetObject)(t, x);
            (a.DecRef)(t);
            (a.DecRef)(x);
        }
        Err(_) => {
            let m = CString::new(format!("brevi: {}", exc_line(&e.0)).replace('\0', "")).unwrap();
            (a.ErrSetString)(a.RuntimeError, m.as_ptr());
        }
    }
}

// The program's class for p when p is one of Python's builtin exception classes.
unsafe fn pyd_builtin_exc(p: P) -> Option<Rc<Class>> {
    let a = pyd_api();
    let n = pyd_own((a.GetAttr)(p, c"__name__".as_ptr())).ok()?;
    let name = pyd_text(n);
    (a.DecRef)(n);
    let name = name.ok()?;
    let c = BUILTIN_CLASSES.with(|b| b.borrow().get(name.as_str()).cloned())?;
    if !(c.module == "builtins" && (c.bt == BT::Exc || c.is_sub(&exc_class("BaseException")))) {
        return None;
    }
    let same = match pyd_builtins_attr(&name) {
        Ok(q) => {
            (a.DecRef)(q);
            q == p
        }
        Err(_) => {
            (a.ErrClear)();
            false
        }
    };
    same.then_some(c)
}
unsafe fn pyd_builtins_attr(name: &str) -> Result<P, E> {
    let a = pyd_api();
    let b = pyd_own((a.Import)(c"builtins".as_ptr()))?;
    let c = CString::new(name).unwrap();
    let r = (a.GetAttr)(b, c.as_ptr());
    (a.DecRef)(b);
    pyd_own(r)
}

unsafe fn pyd_call_raw(f: P, args: &[P]) -> Result<P, E> {
    let a = pyd_api();
    let t = (a.TupleNew)(args.len() as isize);
    for (i, x) in args.iter().enumerate() {
        (a.IncRef)(*x);
        (a.TupleSet)(t, i as isize, *x);
    }
    let r = (a.Call)(f, t, std::ptr::null_mut());
    (a.DecRef)(t);
    pyd_own(r)
}

// module.name(*args), args borrowed.
unsafe fn pyd_mod_call(module: &str, name: &str, args: &[P]) -> Result<P, E> {
    let a = pyd_api();
    let m = CString::new(module).unwrap();
    let m = pyd_own((a.Import)(m.as_ptr()))?;
    let n = CString::new(name).unwrap();
    let f = (a.GetAttr)(m, n.as_ptr());
    (a.DecRef)(m);
    let f = pyd_own(f)?;
    let r = pyd_call_raw(f, args);
    (a.DecRef)(f);
    r
}

unsafe fn pyd_str_arg(module: &str, name: &str, text: &str) -> Result<P, E> {
    let a = pyd_api();
    let s = pyd_str_obj(text);
    let r = pyd_mod_call(module, name, &[s]);
    (a.DecRef)(s);
    r
}

// A namedtuple instance as a CPython namedtuple of the same name and fields, registered in _brevi_dyn so
// pickle can find its class again. The class is made once per Brevi class and kept by the helper module.
unsafe fn pyd_namedtuple(v: &V) -> Result<P, E> {
    let o = match v {
        V::Obj(o) => o,
        _ => return type_err("expected a namedtuple".into()),
    };
    let cls = pyd_callable(&V::Class(o.cls.clone()))?;
    let name = pyd_str_obj(o.cls.name);
    let fields = pyd_to(&o.cls.lookup("_fields").unwrap_or_else(|| tup(vec![])))?;
    let values = pyd_to(&inner_of(v).unwrap_or_else(|| tup(vec![])))?;
    let r = pyd_mod_call("_brevi_dyn", "_nt", &[cls, name, fields, values]);
    let a = pyd_api();
    (a.DecRef)(cls);
    (a.DecRef)(name);
    (a.DecRef)(fields);
    (a.DecRef)(values);
    r
}

// What pickle should save for an iterator of the program. CPython's own iterators over lists, tuples, strings,
// ranges, dicts and sets serialize as the source they have left (`iter` of the rest of the list, tuple, ...),
// so the program's iterator does the same. The callback runs at pickle time, so it sees the iterator's position
// then, not when the value was handed to the library.
enum ItPlan {
    // CPython's exact form for list, tuple and str iterators: `(iter, (whole,), index)`, then __setstate__(index).
    At(V, usize),
    Seq(Vec<V>),
    Range(i64, i64, i64),
    Dict(Rc<RefCell<Dict>>, usize, u8, u64, usize),
    Set(Rc<RefCell<PSet>>, usize, i64),
}
fn iter_reduce(v: &V) -> R {
    let V::Iter(cell) = v else {
        return Err(exc("TypeError", format!("cannot pickle '{}' object", tn(v))));
    };
    let plan = {
        let Ok(it) = cell.it.try_borrow() else {
            return Err(exc("ValueError", "iterator already executing".into()));
        };
        match &*it {
            It::List(l, i) => ItPlan::At(V::List(l.clone()), *i),
            It::Tuple(t, i) => ItPlan::At(tup(t.to_vec()), *i),
            // It::Vec holds the rest of reversed(), map(), zip() and others: CPython pickles each of those its own
            // way (`reversed`, `map(f, it)`, ...), so it is refused rather than saved as a plain list iterator.
            // the iterator keeps a byte offset; CPython's index counts characters
            It::Str(s, i) => ItPlan::At(st(s.s.clone()), s.s[..*i].chars().count()),
            It::Range(a, b, st) => ItPlan::Range(*a, *b, *st),
            It::Dict(d, i, kind, ver, n0) => ItPlan::Dict(d.clone(), *i, *kind, *ver, *n0),
            It::Set(s, i, n0) => ItPlan::Set(s.clone(), *i, *n0),
            It::Done => ItPlan::Seq(vec![]),
            // reversed() of a dict or its views keeps the rest as It::Vec; CPython pickles those iterators as
            // iter(list of the rest), as here
            It::Vec(rest) if matches!(cell.kind, IterClass::Kind(ItKind::DictReverseKeys | ItKind::DictReverseValues | ItKind::DictReverseItems)) => {
                ItPlan::Seq(rest.as_slice().to_vec())
            }
            It::Coro(_) | It::GenExp(..) => {
                return Err(exc("TypeError", "cannot pickle 'generator' object".into()))
            }
            _ => return Err(exc("TypeError", format!("cannot pickle '{}' object", tn(v)))),
        }
    };
    let f = pyd_builtin("iter")?;
    let base = match plan {
        ItPlan::At(whole, i) => return Ok(tup(vec![f, tup(vec![whole]), V::Int(i as i64)])),
        ItPlan::Seq(xs) => V::List(Rc::new(RefCell::new(xs))),
        ItPlan::Range(a, b, st) => V::Range(Rc::new((a, b, st))),
        ItPlan::Dict(d, i, kind, ver, n0) => {
            let mut sim = It::Dict(d, i, kind, ver, n0);
            let mut out = vec![];
            while let Some(x) = sim.next()? {
                out.push(x);
            }
            V::List(Rc::new(RefCell::new(out)))
        }
        ItPlan::Set(s, i, n0) => {
            let mut sim = It::Set(s, i, n0);
            let mut out = vec![];
            while let Some(x) = sim.next()? {
                out.push(x);
            }
            V::List(Rc::new(RefCell::new(out)))
        }
    };
    Ok(tup(vec![f, tup(vec![base])]))
}

// The program's value `v` as a new Python object.
unsafe fn pyd_to(v: &V) -> Result<P, E> {
    let a = pyd_api();
    Ok(match v {
        V::None => {
            (a.IncRef)(a.None);
            a.None
        }
        V::Bool(b) => (a.BoolFrom)(*b as std::ffi::c_long),
        V::Int(i) => (a.LongFrom)(*i),
        V::Big(_) => {
            let c = CString::new(repr(v)?).unwrap();
            pyd_own((a.LongFromStr)(c.as_ptr(), std::ptr::null_mut(), 10))?
        }
        V::Float(f) => (a.FloatFrom)(*f),
        V::Str(x) => pyd_own(pyd_str_obj(&x.s))?,
        V::Tuple(t) => {
            let p = (a.TupleNew)(t.len() as isize);
            for (i, x) in t.iter().enumerate() {
                match pyd_to(x) {
                    Ok(q) => {
                        (a.TupleSet)(p, i as isize, q);
                    }
                    Err(e) => {
                        (a.DecRef)(p);
                        return Err(e);
                    }
                }
            }
            p
        }
        V::List(l) => {
            let items = l.borrow().clone();
            let p = (a.ListNew)(items.len() as isize);
            for (i, x) in items.iter().enumerate() {
                match pyd_to(x) {
                    Ok(q) => {
                        (a.ListSet)(p, i as isize, q);
                    }
                    Err(e) => {
                        (a.DecRef)(p);
                        return Err(e);
                    }
                }
            }
            p
        }
        V::Dict(d) => {
            let items = d.borrow().items();
            let p = (a.DictNew)();
            for (k, x) in items {
                let r = pyd_to(&k).and_then(|kk| match pyd_to(&x) {
                    Ok(xx) => Ok((kk, xx)),
                    Err(e) => {
                        (a.DecRef)(kk);
                        Err(e)
                    }
                });
                let (kk, xx) = match r {
                    Ok(t) => t,
                    Err(e) => {
                        (a.DecRef)(p);
                        return Err(e);
                    }
                };
                let bad = (a.DictSet)(p, kk, xx) < 0;
                (a.DecRef)(kk);
                (a.DecRef)(xx);
                if bad {
                    (a.DecRef)(p);
                    return Err(pyd_err());
                }
            }
            p
        }
        V::Set(_) | V::FSet(_) => {
            let items = to_vec(v)?;
            let p = pyd_own((a.SetNew)(std::ptr::null_mut()))?;
            for x in items {
                let q = match pyd_to(&x) {
                    Ok(q) => q,
                    Err(e) => {
                        (a.DecRef)(p);
                        return Err(e);
                    }
                };
                let bad = (a.SetAdd)(p, q) < 0;
                (a.DecRef)(q);
                if bad {
                    (a.DecRef)(p);
                    return Err(pyd_err());
                }
            }
            if let V::FSet(_) = v {
                let f = (a.FrozenSetNew)(p);
                (a.DecRef)(p);
                pyd_own(f)?
            } else {
                p
            }
        }
        V::Range(r) => {
            let xs = [(a.LongFrom)(r.0), (a.LongFrom)(r.1), (a.LongFrom)(r.2)];
            let f = pyd_builtins_attr("range")?;
            let o = pyd_call_raw(f, &xs);
            (a.DecRef)(f);
            xs.iter().for_each(|x| (a.DecRef)(*x));
            o?
        }
        V::Ellipsis => pyd_builtins_attr("Ellipsis")?,
        V::NotImpl => pyd_builtins_attr("NotImplemented")?,
        V::Module(m) => match m.0 {
            "sys.stdout" | "sys.stderr" | "sys.stdin" => {
                out_flush();
                pyd_own((a.GetAttr)(pyd_own((a.Import)(c"sys".as_ptr()))?, CString::new(&m[4..]).unwrap().as_ptr()))?
            }
            "sys.stdout.buffer" | "sys.stderr.buffer" | "sys.stdin.buffer" => {
                out_flush();
                let stream = pyd_own((a.GetAttr)(pyd_own((a.Import)(c"sys".as_ptr()))?, CString::new(&m[4..m.len() - 7]).unwrap().as_ptr()))?;
                let buf = (a.GetAttr)(stream, c"buffer".as_ptr());
                (a.DecRef)(stream);
                pyd_own(buf)?
            }
            _ => {
                let c = CString::new(m.0).unwrap();
                pyd_own((a.Import)(c.as_ptr()))?
            }
        },
        V::Ext(x) => {
            if let Some(p) = x.any().downcast_ref::<PyV>() {
                (a.IncRef)(p.p);
                return Ok(p.p);
            }
            if let Some(b) = bytes_of(v) {
                return pyd_own((a.BytesFrom)(b.as_ptr() as *const c_char, b.len() as isize));
            }
            match x.tname().rsplit('.').next().unwrap_or("") {
                "Decimal" => pyd_str_arg("decimal", "Decimal", &to_str(v)?)?,
                "Fraction" => pyd_str_arg("fractions", "Fraction", &to_str(v)?)?,
                t @ ("datetime" | "date" | "time") => {
                    let iso = to_str(&call_method(v, "isoformat", Args::p(vec![]))?)?;
                    let a2 = pyd_api();
                    let m = pyd_own((a2.Import)(c"datetime".as_ptr()))?;
                    let c = CString::new(t).unwrap();
                    let k = (a2.GetAttr)(m, c.as_ptr());
                    (a2.DecRef)(m);
                    let k = pyd_own(k)?;
                    let f = (a2.GetAttr)(k, c"fromisoformat".as_ptr());
                    (a2.DecRef)(k);
                    let f = pyd_own(f)?;
                    let s = pyd_str_obj(&iso);
                    let r = pyd_call_raw(f, &[s]);
                    (a2.DecRef)(s);
                    (a2.DecRef)(f);
                    r?
                }
                "timedelta" => {
                    let part = |n: &str| -> Result<i64, E> {
                        match getattr(v, n)? {
                            V::Int(i) => Ok(i),
                            _ => Ok(0),
                        }
                    };
                    let xs = [(a.LongFrom)(part("days")?), (a.LongFrom)(part("seconds")?), (a.LongFrom)(part("microseconds")?)];
                    let r = pyd_mod_call("datetime", "timedelta", &xs);
                    xs.iter().for_each(|x| (a.DecRef)(*x));
                    r?
                }
                "timezone" => {
                    let off = pyd_to(&call_method(v, "utcoffset", Args::p(vec![V::None]))?)?;
                    let r = pyd_mod_call("datetime", "timezone", &[off]);
                    (a.DecRef)(off);
                    r?
                }
                t => return type_err(format!("brevi: a {t} value cannot be passed to a Python library in a dynamic build")),
            }
        }
        V::Obj(o) if o.cls.base_bt() == BT::Exc || o.cls.is_sub(&exc_class("BaseException")) => {
            if let Some(p) = o.get("__pyexc__").as_ref().and_then(pyv_of) {
                (a.IncRef)(p.p);
                return Ok(p.p);
            }
            // The nearest builtin exception class, with the same args; the program's own exception rides
            // along (`__brevi__`) and comes back as itself.
            let base = o.cls.mro.borrow().iter().find(|c| c.module == "builtins" && pyd_builtins_attr(c.name).map(|p| (a.DecRef)(p)).map_err(|_| (a.ErrClear)()).is_ok()).map(|c| c.name).unwrap_or("Exception");
            let k = pyd_builtins_attr(base)?;
            let args = match o.get("args") {
                Some(t @ V::Tuple(_)) => t,
                _ => tup(vec![]),
            };
            let args = match (base, o.get("code")) {
                ("SystemExit", Some(c)) => tup(vec![c]),
                _ => args,
            };
            let t = match pyd_to(&args) {
                Ok(t) => t,
                Err(_) => {
                    let m = to_str(v).unwrap_or_default();
                    let t = (a.TupleNew)(1);
                    (a.TupleSet)(t, 0, pyd_str_obj(&m));
                    t
                }
            };
            let e = (a.Call)(k, t, std::ptr::null_mut());
            (a.DecRef)(t);
            (a.DecRef)(k);
            let e = pyd_own(e)?;
            if let Ok(f) = pyd_callable(v) {
                if (a.SetAttr)(e, c"__brevi__".as_ptr(), f) < 0 {
                    (a.ErrClear)();
                }
                (a.DecRef)(f);
            }
            e
        }
        V::Class(c) if c.module == "builtins" && (c.bt == BT::Exc || c.is_sub(&exc_class("BaseException"))) => pyd_builtins_attr(c.name)?,
        V::Obj(o) if o.cls.lookup("__bv_namedtuple__").is_some() => pyd_namedtuple(v)?,
        V::Iter(_) => {
            let next_me = v.clone();
            let red_me = v.clone();
            let next = func(sym("__next__"), move |_a: Args| bi_next(Args::p(vec![next_me.clone()])));
            let red = func(sym("__reduce__"), move |_a: Args| iter_reduce(&red_me));
            let f = pyd_callable(&next)?;
            let r = pyd_callable(&red)?;
            // the iterator itself, so that it comes back out of Python as the program's own iterator
            let b = pyd_callable(v)?;
            let o = pyd_mod_call("_brevi_dyn", "it", &[f, r, b]);
            (a.DecRef)(f);
            (a.DecRef)(r);
            (a.DecRef)(b);
            let o = o?;
            let t = (a.Type)(o);
            (a.DecRef)(t);
            PYD_BRIT.store(t, std::sync::atomic::Ordering::Relaxed);
            o
        }
        V::Func(_) | V::Bound(_) | V::BMeth(_) | V::Class(_) => pyd_callable(v)?,
        V::Obj(o) if o.cls.lookup("__call__").is_some() => pyd_callable(v)?,
        _ => return type_err(format!("brevi: a '{}' object cannot be passed to a Python library in a dynamic build", tn(v))),
    })
}

// Arguments of a call into Python, and what to copy back after it: a list or dict of the program the library
// changed (items compared by identity).
struct PydArgs {
    t: P,
    kw: P,
    back: Vec<(V, P, Vec<P>)>,
}
impl Drop for PydArgs {
    fn drop(&mut self) {
        let a = pyd_api();
        unsafe {
            (a.DecRef)(self.t);
            if !self.kw.is_null() {
                (a.DecRef)(self.kw);
            }
            for (_, p, _) in &self.back {
                (a.DecRef)(*p);
            }
        }
    }
}

unsafe fn pyd_snapshot(p: P) -> Vec<P> {
    let a = pyd_api();
    let mut out = vec![];
    if pyd_type_is(p, a.TList) {
        for i in 0..(a.ListSize)(p) {
            out.push((a.ListGet)(p, i));
        }
    } else {
        let mut pos: isize = 0;
        let (mut k, mut x): (P, P) = (std::ptr::null_mut(), std::ptr::null_mut());
        while (a.DictNext)(p, &mut pos, &mut k, &mut x) != 0 {
            out.push(k);
            out.push(x);
        }
    }
    out
}

unsafe fn pyd_args(pre: &[V], args: &Args) -> Result<PydArgs, E> {
    let a = pyd_api();
    let n = pre.len() + args.p.len();
    let mut out = PydArgs { t: (a.TupleNew)(n as isize), kw: std::ptr::null_mut(), back: vec![] };
    let mut one = |x: &V, back: &mut Vec<(V, P, Vec<P>)>| -> Result<P, E> {
        let p = pyd_to(x)?;
        if matches!(x, V::List(_) | V::Dict(_)) {
            (a.IncRef)(p);
            back.push((x.clone(), p, pyd_snapshot(p)));
        }
        Ok(p)
    };
    for (i, x) in pre.iter().chain(args.p.iter()).enumerate() {
        let p = one(x, &mut out.back)?;
        (a.TupleSet)(out.t, i as isize, p);
    }
    if !args.k.is_empty() {
        out.kw = (a.DictNew)();
        for (k, x) in &args.k {
            let p = one(x, &mut out.back)?;
            // interned, as CPython's own keyword names are (orjson tells its keywords apart by identity)
            let mut kk = pyd_str_obj(k);
            (a.Intern)(&mut kk);
            (a.DictSet)(out.kw, kk, p);
            (a.DecRef)(kk);
            (a.DecRef)(p);
        }
    }
    Ok(out)
}

unsafe fn pyd_write_back(args: &PydArgs) -> Result<(), E> {
    let a = pyd_api();
    for (v, p, snap) in &args.back {
        if pyd_snapshot(*p) == *snap {
            continue;
        }
        (a.IncRef)(*p);
        match (v, pyd_wrap(*p, true)?) {
            (V::List(l), V::List(n)) => {
                let items = n.borrow().clone();
                *l.borrow_mut() = items;
            }
            (V::Dict(d), V::Dict(n)) => {
                let items = n.borrow().items();
                d.borrow_mut().clear();
                for (k, x) in items {
                    Dict::set_cell(d, k, x)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// f(*pre, *args, **kw); the result copied when `deep`.
unsafe fn pyd_invoke(f: P, pre: &[V], args: &Args, deep: bool) -> R {
    let a = pyd_api();
    out_flush();
    let pa = pyd_args(pre, args)?;
    let r = (a.Call)(f, pa.t, pa.kw);
    if r.is_null() {
        return Err(pyd_err());
    }
    pyd_write_back(&pa)?;
    pyd_wrap(r, deep)
}

impl PyV {
    unsafe fn attr_raw(&self, n: &str) -> Result<P, E> {
        let c = CString::new(n).map_err(|_| exc("ValueError", "embedded null character".into()))?;
        pyd_own((pyd_api().GetAttr)(self.p, c.as_ptr()))
    }
    unsafe fn call_named(&self, n: &str, a: &mut Args) -> R {
        let api = pyd_api();
        match n {
            "__call__" => pyd_invoke(self.p, &[], a, true),
            "__getitem__" => {
                let k = pyd_to(a.p.first().unwrap_or(&V::None))?;
                let r = (api.GetItem)(self.p, k);
                (api.DecRef)(k);
                pyd_wrap(pyd_own(r)?, false)
            }
            "__setitem__" => {
                let k = pyd_to(a.p.first().unwrap_or(&V::None))?;
                let x = match pyd_to(a.p.get(1).unwrap_or(&V::None)) {
                    Ok(x) => x,
                    Err(e) => {
                        (api.DecRef)(k);
                        return Err(e);
                    }
                };
                let bad = (api.SetItem)(self.p, k, x) < 0;
                (api.DecRef)(k);
                (api.DecRef)(x);
                if bad { Err(pyd_err()) } else { Ok(V::None) }
            }
            "__delitem__" => {
                let k = pyd_to(a.p.first().unwrap_or(&V::None))?;
                let bad = (api.DelItem)(self.p, k) < 0;
                (api.DecRef)(k);
                if bad { Err(pyd_err()) } else { Ok(V::None) }
            }
            "__contains__" => {
                let k = pyd_to(a.p.first().unwrap_or(&V::None))?;
                let r = pyd_helper("contains", &[self.p, k]);
                (api.DecRef)(k);
                r
            }
            // What a context manager hands the block is usually something it keeps changing (the list
            // catch_warnings(record=True) appends to): kept live, as CPython shares it, not copied.
            "__enter__" => {
                let f = self.attr_raw("__enter__")?;
                let r = pyd_invoke(f, &[], a, false);
                (api.DecRef)(f);
                r
            }
            "__exit__" => {
                // (type, value, traceback) as Python's: the exception as Python's own.
                let f = self.attr_raw("__exit__")?;
                let ev = a.p.get(1).cloned().unwrap_or(V::None);
                let r = if matches!(ev, V::None) {
                    pyd_invoke(f, &[V::None, V::None, V::None], &Args::p(vec![]), true)
                } else {
                    match pyd_to(&ev) {
                        Ok(e) => {
                            let t = (api.Type)(e);
                            let args = [t, e, api.None];
                            let r = pyd_call_raw(f, &args).and_then(|r| pyd_wrap(r, true));
                            (api.DecRef)(t);
                            (api.DecRef)(e);
                            r
                        }
                        Err(_) => pyd_invoke(f, &[V::None, V::None, V::None], &Args::p(vec![]), true),
                    }
                };
                (api.DecRef)(f);
                r
            }
            _ => {
                let f = self.attr_raw(n)?;
                // Methods of a list, dict or set the library holds hand back what it holds (d.setdefault(k, [])).
                let r = pyd_invoke(f, &[], a, !self.live);
                (api.DecRef)(f);
                r
            }
        }
    }
    unsafe fn exact(&self) -> bool {
        let a = pyd_api();
        pyd_type_is(self.p, a.TList) || pyd_type_is(self.p, a.TDict) || pyd_type_is(self.p, a.TSet) || pyd_type_is(self.p, a.TFrozenSet) || pyd_type_is(self.p, a.TTuple)
    }
}

fn pyd_opname(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "add",
        "-" => "sub",
        "*" => "mul",
        "/" => "truediv",
        "//" => "floordiv",
        "%" => "mod",
        "**" => "pow",
        "@" => "matmul",
        "<<" => "lshift",
        ">>" => "rshift",
        "&" => "and_",
        "|" => "or_",
        "^" => "xor",
        // comparisons as values (cmp_v): a library's own result, such as a polars or pandas expression
        "<" => "lt",
        "<=" => "le",
        ">" => "gt",
        ">=" => "ge",
        "==" => "eq",
        "!=" => "ne",
        _ => return None,
    })
}

impl Ext for PyV {
    fn tname(&self) -> &'static str {
        if let Some(t) = self.tn.get() {
            return t;
        }
        let _g = PydGil::take();
        let a = pyd_api();
        let n = unsafe {
            let t = (a.Type)(self.p);
            let n = (a.GetAttr)(t, c"__name__".as_ptr());
            (a.DecRef)(t);
            if n.is_null() {
                (a.ErrClear)();
                "object".to_string()
            } else {
                let s = pyd_text(n).unwrap_or_default();
                (a.DecRef)(n);
                s
            }
        };
        let t = sym(&n);
        self.tn.set(Some(t));
        t
    }
    fn repr(&self) -> String {
        let _g = PydGil::take();
        let a = pyd_api();
        unsafe {
            let r = (a.Repr)(self.p);
            if r.is_null() {
                (a.ErrClear)();
                return String::new();
            }
            let s = pyd_text(r).unwrap_or_default();
            (a.DecRef)(r);
            s
        }
    }
    fn str_(&self) -> String {
        let _g = PydGil::take();
        let a = pyd_api();
        unsafe {
            out_flush();
            let r = (a.Str)(self.p);
            if r.is_null() {
                (a.ErrClear)();
                return String::new();
            }
            let s = pyd_text(r).unwrap_or_default();
            (a.DecRef)(r);
            s
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        let _g = PydGil::take();
        let a = pyd_api();
        let h = unsafe { (a.Hash)(self.p) };
        if h == -1 && unsafe { !(a.ErrOccurred)().is_null() } {
            unsafe { (a.ErrClear)() };
            return None;
        }
        Some(h as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        let _g = PydGil::take();
        let a = pyd_api();
        unsafe {
            let q = match pyd_to(o) {
                Ok(q) => q,
                Err(_) => {
                    (a.ErrClear)();
                    return Some(false);
                }
            };
            let r = (a.RichCompareBool)(self.p, q, 2);
            (a.DecRef)(q);
            if r < 0 {
                (a.ErrClear)();
                return None;
            }
            Some(r == 1)
        }
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        let lt = self.rich(&V::None, "<", o, false)?.ok()?;
        if lt {
            return Some(std::cmp::Ordering::Less);
        }
        let gt = self.rich(&V::None, ">", o, false)?.ok()?;
        Some(if gt { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal })
    }
    fn rich(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<Result<bool, E>> {
        let code = match op {
            "<" => 0,
            "<=" => 1,
            ">" => 4,
            ">=" => 5,
            _ => return None,
        };
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            (|| {
                let q = pyd_to(o)?;
                let r = if right { (a.RichCompareBool)(q, self.p, code) } else { (a.RichCompareBool)(self.p, q, code) };
                (a.DecRef)(q);
                if r < 0 { Err(pyd_err()) } else { Ok(r == 1) }
            })()
        })
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let _g = PydGil::take();
        Some(unsafe { self.attr_raw(n).and_then(|p| pyd_wrap(p, false)) })
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let _g = PydGil::take();
        Some(unsafe { self.call_named(n, a) })
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            (|| {
                if op == "neg" || op == "pos" || op == "invert" {
                    let n = pyd_str_obj(op);
                    let r = pyd_helper("un", &[n, self.p]);
                    (a.DecRef)(n);
                    return r;
                }
                let Some(name) = pyd_opname(op) else { return Ok(V::NotImpl) };
                let q = pyd_to(o)?;
                let n = pyd_str_obj(name);
                let r = if right { pyd_helper("op", &[n, q, self.p]) } else { pyd_helper("op", &[n, self.p, q]) };
                (a.DecRef)(n);
                (a.DecRef)(q);
                r
            })()
        })
    }
    fn cmp_value(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        self.binop(me, op, o, right)
    }
    fn try_truth(&self) -> Option<Result<bool, E>> {
        let _g = PydGil::take();
        let r = unsafe { (pyd_api().IsTrue)(self.p) };
        Some(if r < 0 { Err(pyd_err()) } else { Ok(r == 1) })
    }
    fn to_f64(&self) -> Option<f64> {
        let _g = PydGil::take();
        let a = pyd_api();
        unsafe {
            let f = (a.NumberFloat)(self.p);
            if f.is_null() {
                (a.ErrClear)();
                return None;
            }
            let x = (a.FloatAs)(f);
            (a.DecRef)(f);
            Some(x)
        }
    }
    fn len(&self) -> Option<usize> {
        let _g = PydGil::take();
        let a = pyd_api();
        let n = unsafe { (a.Size)(self.p) };
        if n < 0 {
            unsafe { (a.ErrClear)() };
            return None;
        }
        Some(n as usize)
    }
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        let _g = PydGil::take();
        let a = pyd_api();
        let it = unsafe { (a.GetIter)(self.p) };
        if it.is_null() {
            return Some(Err(pyd_err()));
        }
        let holder = PyV { p: it, live: false, tn: Cell::new(None) };
        Some(Ok(It::Gen(Box::new(move || {
            // The whole holder (not just its pointer) lives in the closure.
            let h = &holder;
            let _g = PydGil::take();
            let a = pyd_api();
            unsafe {
                let x = (a.IterNext)(h.p);
                if x.is_null() {
                    if (a.ErrOccurred)().is_null() {
                        return Ok(None);
                    }
                    return Err(pyd_err());
                }
                pyd_wrap(x, false).map(Some)
            }
        }))))
    }
    fn iter_class(&self, _me: &V) -> Option<Rc<Class>> {
        let _g = PydGil::take();
        let (name, module) = unsafe { pyd_iter_type(self.p)? };
        Some(ext_class_mod(&module, &name))
    }
    fn next_item(&self) -> Option<R> {
        // `next(x)` has CPython's answer even when the library value is not an iterator (a list, a module, ...),
        // so run Python's own next rather than the bridge's iter().
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            let f = match pyd_builtins_attr("next") {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            let r = pyd_call_raw(f, &[self.p]);
            (a.DecRef)(f);
            match r {
                Ok(p) => pyd_wrap(p, false),
                Err(e) => Err(e),
            }
        })
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            let s = pyd_str_obj(spec);
            let r = (a.Format)(self.p, s);
            (a.DecRef)(s);
            pyd_own(r).and_then(|r| pyd_wrap(r, true))
        })
    }
    fn set_attr(&self, _me: &V, n: &str, v: V) -> Option<Result<(), E>> {
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            (|| {
                let x = pyd_to(&v)?;
                let c = CString::new(n).unwrap();
                let bad = (a.SetAttr)(self.p, c.as_ptr(), x) < 0;
                (a.DecRef)(x);
                if bad { Err(pyd_err()) } else { Ok(()) }
            })()
        })
    }
    fn isinst(&self, cls: &Class) -> Option<bool> {
        if cls.module != "builtins" {
            return Some(false);
        }
        let _g = PydGil::take();
        let a = pyd_api();
        unsafe {
            let k = match pyd_builtins_attr(cls.name) {
                Ok(k) => k,
                Err(_) => {
                    (a.ErrClear)();
                    return None;
                }
            };
            let r = (a.IsInstance)(self.p, k);
            (a.DecRef)(k);
            if r < 0 {
                (a.ErrClear)();
                return None;
            }
            Some(r == 1)
        }
    }
    fn instancecheck(&self, x: &V) -> Option<Result<bool, E>> {
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            (|| {
                let xp = match x {
                    V::Obj(o) => match o.get("__pyexc__").as_ref().and_then(pyv_of) {
                        Some(p) => {
                            (a.IncRef)(p.p);
                            p.p
                        }
                        None => match inner_of(x) {
                            Some(inner) => match pyd_to(&inner) {
                                Ok(p) => p,
                                Err(_) => return Ok(false),
                            },
                            None => return Ok(false),
                        },
                    },
                    _ => match pyd_to(x) {
                        Ok(p) => p,
                        Err(_) => return Ok(false),
                    },
                };
                let r = (a.IsInstance)(xp, self.p);
                (a.DecRef)(xp);
                if r < 0 { Err(pyd_err()) } else { Ok(r == 1) }
            })()
        })
    }
    // `issubclass(sub, self)` for a Brevi class: builtin and collections classes have their CPython twins, and a
    // user subclass of a builtin container stands for its builtin base (a user class cannot inherit a CPython ABC).
    fn subclasscheck(&self, sub: &V) -> Option<Result<bool, E>> {
        let _g = PydGil::take();
        let a = pyd_api();
        Some(unsafe {
            (|| {
                let collections_attr = |n: &str| -> Result<P, E> {
                    let m = pyd_own((a.Import)(c"collections".as_ptr()))?;
                    let c = CString::new(n).unwrap();
                    let r = (a.GetAttr)(m, c.as_ptr());
                    (a.DecRef)(m);
                    pyd_own(r)
                };
                let sp = match sub {
                    V::Class(k) if k.module == "builtins" => pyd_builtins_attr(k.name)?,
                    V::Class(k) if k.module == "collections" => collections_attr(k.name)?,
                    V::Class(k) => match k.base_bt() {
                        BT::Bool => pyd_builtins_attr("bool")?,
                        BT::Int => pyd_builtins_attr("int")?,
                        BT::Float => pyd_builtins_attr("float")?,
                        BT::Str => pyd_builtins_attr("str")?,
                        BT::List => pyd_builtins_attr("list")?,
                        BT::Tuple => pyd_builtins_attr("tuple")?,
                        BT::Dict => pyd_builtins_attr("dict")?,
                        BT::Set => pyd_builtins_attr("set")?,
                        BT::DefaultDict => collections_attr("defaultdict")?,
                        BT::Counter => collections_attr("Counter")?,
                        BT::OrderedDict => collections_attr("OrderedDict")?,
                        BT::Deque => collections_attr("deque")?,
                        _ => return Ok(false),
                    },
                    _ => return Ok(false),
                };
                let r = (a.IsSubclass)(sp, self.p);
                (a.DecRef)(sp);
                if r < 0 { Err(pyd_err()) } else { Ok(r == 1) }
            })()
        })
    }
    fn native(&self) -> Option<R> {
        let _g = PydGil::take();
        unsafe {
            if !self.exact() {
                return None;
            }
            (pyd_api().IncRef)(self.p);
            Some(pyd_wrap(self.p, true))
        }
    }
    fn is_py(&self) -> bool {
        true
    }
}

// `import m` (`import a.b` binds a: top), `from m import n`, a builtin CPython has and the dynamic build lacks.
pub fn pyd_import(m: &str, top: bool) -> R {
    let a = pyd_api();
    let _g = PydGil::take();
    unsafe {
        let c = CString::new(m).unwrap();
        let p = pyd_own((a.Import)(c.as_ptr()))?;
        if top && m.contains('.') {
            (a.DecRef)(p);
            let c = CString::new(m.split('.').next().unwrap()).unwrap();
            return Ok(pyv(pyd_own((a.Import)(c.as_ptr()))?, false));
        }
        Ok(pyv(p, false))
    }
}
pub fn pyd_from(m: &str, n: &str) -> R {
    let a = pyd_api();
    let _g = PydGil::take();
    unsafe {
        let c = CString::new(m).unwrap();
        let p = pyd_own((a.Import)(c.as_ptr()))?;
        let cn = CString::new(n).unwrap();
        let x = (a.GetAttr)(p, cn.as_ptr());
        (a.DecRef)(p);
        if x.is_null() {
            // `from PIL import Image`: a submodule not yet imported.
            (a.ErrClear)();
            let c = CString::new(format!("{m}.{n}")).unwrap();
            let s = (a.Import)(c.as_ptr());
            if s.is_null() {
                (a.ErrClear)();
                return Err(exc("ImportError", format!("cannot import name '{n}' from '{m}'")));
            }
            return Ok(pyv(s, false));
        }
        pyd_wrap(x, false)
    }
}
// datetime.<cls>.<meth>(*a) run by CPython, for a tzinfo the native datetime lacks (rt_ext.rs py_dt).
fn pyd_dt(cls: &str, meth: &str, a: Args) -> R {
    let c = pyd_from("datetime", cls)?;
    let f = if meth.is_empty() { c } else { getattr(&c, meth)? };
    call(&f, a)
}
pub fn pyd_builtin(n: &str) -> R {
    let _g = PydGil::take();
    unsafe { pyv_ok(pyd_builtins_attr(n)) }
}
unsafe fn pyv_ok(p: Result<P, E>) -> R {
    Ok(pyv(p?, false))
}

// A class built on a library's class (`class Post(BaseModel)`), run by CPython whole: `imports` is the module's
// import lines, `src` the class's lines (at their own line numbers), in a Python module named as the program's.
pub fn pyd_class(module: &str, imports: &str, src: &str, name: &str, file: &str) -> R {
    let a = pyd_api();
    let _g = PydGil::take();
    unsafe {
        out_flush();
        let xs: Vec<P> = [module, imports, src, name, file].iter().map(|s| pyd_str_obj(s)).collect();
        let r = pyd_mod_call("_brevi_dyn", "pyclass", &xs);
        xs.iter().for_each(|x| (a.DecRef)(*x));
        Ok(pyv(r?, false))
    }
}
