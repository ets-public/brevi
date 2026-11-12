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
// the type of _brevi_dyn's _BrRed (a value of the program that Python sees only to pickle it), once one has been made
// CPython's re.Pattern type, once a program's compiled pattern has gone into Python: patterns of that type come
// back as the program's own (pickle.loads of a pickled pattern)
static PYD_REPAT: std::sync::atomic::AtomicPtr<std::ffi::c_void> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
static PYD_BRRED: std::sync::atomic::AtomicPtr<std::ffi::c_void> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
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
    def __setstate__(s, state): return _d.setstate(s.b, state)
class _BrRed:
    __slots__ = ('r', 'b')
    def __init__(s, r, b): s.r = r; s.b = b
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
    if file == '<copy>':
        import copy
        file = copy.__file__
    if line == 0:
        return _at_unknown(msg, getattr(_b, cat), module)
    warnings.warn_explicit(msg, getattr(_b, cat), file, line, module, _d.regs.setdefault(module, {}))
# A warning at a line of the program the dynamic build does not know (a statement over several lines), in a
# known module. Only an "error" filter that names no line gives CPython's answer without the line (it raises
# before the registry or the line is looked at); anything else is refused.
def _at_unknown(message, category, module):
    import warnings as _w
    if isinstance(message, Warning):
        text = str(message)
        category = message.__class__
    else:
        text = message
        message = category(message)
    for action, msg, cat, mod, ln in _w.filters:
        if (msg is None or msg.match(text)) and issubclass(category, cat) and (mod is None or mod.match(module)):
            if ln == 0 and action == "error":
                raise message
            break
    raise NotImplementedError("brevi: a warning at a line of the program the dynamic build does not know is not supported in dynamic builds")
_d.regs = {}
_d.warn_at = _warn_at
# The program uses the warnings module. A program frame is no Python frame, so CPython's warnings.warn would
# report a warning that points at the program (warnings.warn called by it, or a library's warn whose
# stacklevel reaches it) at "<sys>":0 in module sys. This warn walks the Python frames of the library call
# only, up to the frame that called into the program (_d.bnd, pushed by each call of a program function from
# Python), and names the program's line (_d.site) when it gets there; a frame further out is not known, so
# that warning is refused rather than placed wrong.
_d.bnd = []
def _bpush():
    try:
        _d.bnd.append(_s._getframe(1))
    except ValueError:
        _d.bnd.append(None)
def _bpop():
    _d.bnd.pop()
_d.bpush = _bpush
_d.bpop = _bpop
def _install_warn(site):
    import warnings as _w
    _d.site = site
    def warn(message, category=None, stacklevel=1, source=None, *, skip_file_prefixes=()):
        if isinstance(message, Warning):
            category = message.__class__
        if category is None:
            category = UserWarning
        if not (isinstance(category, type) and issubclass(category, Warning)):
            raise TypeError("category must be a Warning subclass, not '{:s}'".format(type(category).__name__))
        if not isinstance(skip_file_prefixes, tuple):
            raise TypeError('skip_file_prefixes must be a tuple of strs.')
        if skip_file_prefixes:
            stacklevel = max(2, stacklevel)
        stop = _d.bnd[-1] if _d.bnd else None
        fr = []
        try:
            f = _s._getframe(1)
        except ValueError:
            f = None
        while f is not None and f is not stop:
            fr.append(f)
            f = f.f_back
        # fr[0..P] are the library's frames, newest first; P is the program's frame; past it, not known
        P = len(fr)
        s = _d.site()
        if stacklevel <= 1 or (fr and _w._is_internal_frame(fr[0])):
            i = max(stacklevel, 1) - 1
        else:
            i = 0
            for _ in range(stacklevel - 1):
                i += 1
                while i < P and (_w._is_internal_frame(fr[i]) or _w._is_filename_to_skip(fr[i].f_code.co_filename, skip_file_prefixes)):
                    i += 1
                if i == P and s is not None and _w._is_filename_to_skip(s[0], skip_file_prefixes):
                    i += 1
                if i > P:
                    break
        if i < P:
            fm = fr[i]
            g = fm.f_globals
            module = g['__name__'] if '__name__' in g else '<string>'
            return _w.warn_explicit(message, category, fm.f_code.co_filename, fm.f_lineno, module, g.setdefault('__warningregistry__', {}), g, source)
        if i == P and s is not None:
            file, line, module = s
            if line == 0:
                return _at_unknown(message, category, module)
            return _w.warn_explicit(message, category, file, line, module, _d.regs.setdefault(module, {}), None, source)
        raise NotImplementedError("brevi: a warning at a line of the program the dynamic build does not know is not supported in dynamic builds")
    warn.__doc__ = _w.warn.__doc__
    warn.__module__ = '_warnings'
    warn.__qualname__ = 'warn'
    _w.warn = warn
    # a warning C code issues with no Python frame left lands at "<sys>": refused, never shown or recorded there
    show = _w._showwarnmsg
    def _showwarnmsg(msg):
        if msg.filename == '<sys>':
            raise NotImplementedError("brevi: a warning at a line of the program the dynamic build does not know is not supported in dynamic builds")
        return show(msg)
    _w._showwarnmsg = _showwarnmsg
_d.install_warn = _install_warn
_d.it = _BrIt
def _buf(o):
    try:
        m = memoryview(o)
    except TypeError:
        return None
    with m:
        return m.tobytes()
_d.buf = _buf
_d.red = _BrRed
# The program's own `pickle` module: CPython's, except that a pickle naming an itertools class loads as the
# program's itertools iterator (as pickles the program makes do), not CPython's, whose __setstate__ warns from C
# code with no frame to name. Libraries keep CPython's pickle.
def _pmod():
    m = _d.__dict__.get('_pm')
    if m is not None:
        return m
    import pickle as _p, io as _io
    m = _t.ModuleType('pickle')
    m.__dict__.update(_p.__dict__)
    class Unpickler(_p.Unpickler):
        def find_class(self, module, name):
            obj = super().find_class(module, name)
            if isinstance(obj, type) and obj.__module__ == 'itertools':
                try:
                    return _d._lookup('itertools', obj.__name__)
                except Exception:
                    pass
            return obj
    Unpickler.__module__ = '_pickle'
    Unpickler.__qualname__ = Unpickler.__name__ = 'Unpickler'
    def load(file, *, fix_imports=True, encoding="ASCII", errors="strict", buffers=None):
        return Unpickler(file, fix_imports=fix_imports, encoding=encoding, errors=errors, buffers=buffers).load()
    def loads(data, /, *, fix_imports=True, encoding="ASCII", errors="strict", buffers=None):
        if isinstance(data, str):
            raise TypeError("a bytes-like object is required, not 'str'")
        return Unpickler(_io.BytesIO(data), fix_imports=fix_imports, encoding=encoding, errors=errors, buffers=buffers).load()
    load.__module__ = loads.__module__ = '_pickle'
    m.Unpickler, m.load, m.loads = Unpickler, load, loads
    _d._pm = m
    return m
_d.pmod = _pmod
_d.pyclass = _pyclass
_d._nt = _nt
_d.atexit = _atexit
_d.contains = lambda a, b: b in a
import collections.abc as _cabc
_d.is_array = lambda x: type(x).__module__ == "array" and type(x).__name__ == "array"
# A bytearray of the program goes to a library as a copy, its changes copied back (see pyd_args): only into the
# C code of array, struct and zlib, which reads or writes the buffer and runs no Python code of its own. A class
# with Python code in it (a library's subclass) does not count.
def _native(t):
    return t.__module__ in ("array", "_struct", "zlib") and not any(
        isinstance(v, (_t.FunctionType, classmethod, staticmethod, property)) for c in t.__mro__ for v in vars(c).values())
def _ba_ok(f):
    if isinstance(f, type):
        return _native(f)
    if not isinstance(f, _t.BuiltinFunctionType):  # PyCMethod (array.extend) is a subclass
        return False
    s = getattr(f, "__self__", None)
    if isinstance(s, _t.ModuleType):
        return s.__name__ in ("array", "_struct", "zlib")
    return _native(type(s))
_d.ba_ok = _ba_ok
_d.rc = _s.getrefcount
_d.babytes = _b.bytes
_d.cabc_inst = lambda n, x: _b.isinstance(x, getattr(_cabc, n))
_d.cabc_sub = lambda n, x: _b.issubclass(x, getattr(_cabc, n))
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
def _rebuild(cls, args, state):
    return _d._rebuild_obj(cls, args, state)
_rebuild.__module__ = '_brevi_dyn'
_d._rebuild = _rebuild
def _bf_reduce(f):
    r = _d._ref(f)
    if r is None:
        return f.__reduce__()
    if r[0] == 1:
        import pickle
        raise pickle.PicklingError(r[1])
    if r[0] == 2:
        raise AttributeError(r[1])
    if r[0] == 3:
        t = r[1]
        return (_rebuild, (t[0], t[1], t[2] if len(t) > 2 else None))
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
        BUF_BRIDGE.with(|h| h.set(Some(pyd_buf)));
        PY_BUILTIN.with(|h| h.set(Some(pyd_builtin)));
        ABC_BRIDGE.with(|h| h.set(Some(pyd_abc)));
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
// The program uses the warnings module: CPython's warnings.warn names the program's line (see _install_warn).
static PYD_WARN_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn pyd_warn_install() -> Result<(), E> {
    let site = func("site", |_a: Args| Ok(match warn_site() {
        Some((f, l, m)) => tup(vec![s(f), V::Int(l as i64), s(m)]),
        None => V::None,
    }));
    unsafe {
        let _g = PydGil::take();
        let a = pyd_api();
        let p = pyd_to(&site)?;
        let r = pyd_helper("install_warn", &[p]);
        (a.DecRef)(p);
        r?;
    }
    PYD_WARN_ON.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
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
    if deep {
        if let Some(v) = FROM_MEMO.with(|m| m.borrow().1.get(&(p as usize)).map(|e| e.1.clone())) {
            (a.DecRef)(p);
            return Ok(v);
        }
    }
    FROM_MEMO.with(|m| m.borrow_mut().0 += 1);
    let r = pyd_wrap_new(p, deep);
    let held = FROM_MEMO.with(|m| {
        let mut m = m.borrow_mut();
        m.0 -= 1;
        if m.0 == 0 { m.1.drain().map(|(_, e)| e.0).collect::<Vec<_>>() } else { vec![] }
    });
    held.into_iter().for_each(|q| (a.DecRef)(q));
    r
}

// Remembers `v` as the program's value for the Python object `p` (the memo keeps `p` alive, so its address is
// not reused by another object during the conversion).
unsafe fn from_memo_put(p: P, v: &V) {
    (pyd_api().IncRef)(p);
    FROM_MEMO.with(|m| m.borrow_mut().1.insert(p as usize, (p, v.clone())));
}

unsafe fn pyd_wrap_new(p: P, deep: bool) -> R {
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
    if t == a.TComplex {
        let (re, im) = ((a.ComplexReal)(p), (a.ComplexImag)(p));
        return done(cplx(re, im));
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
        let cell = Rc::new(RefCell::new(Vec::with_capacity(n.max(0) as usize)));
        let me = V::List(cell.clone());
        from_memo_put(p, &me); // before its items: a list holding itself comes back holding itself
        for i in 0..n {
            let x = (a.ListGet)(p, i);
            (a.IncRef)(x);
            match pyd_wrap(x, true) {
                Ok(v) => cell.borrow_mut().push(v),
                Err(e) => {
                    (a.DecRef)(p);
                    return Err(e);
                }
            }
        }
        return done(me);
    }
    if deep && t == a.TDict {
        let cell = Rc::new(RefCell::new(Dict::new()));
        let me = V::Dict(cell.clone());
        from_memo_put(p, &me);
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
            match r.and_then(|(kk, xx)| cell.borrow_mut().set(kk, xx)) {
                Ok(()) => {}
                Err(e) => err = Some(e),
            }
        }
        (a.DecRef)(p);
        return match err {
            Some(e) => Err(e),
            None => Ok(me),
        };
    }
    if deep && (t == a.TSet || t == a.TFrozenSet) {
        let items = pyd_items(p);
        let made = items.and_then(PSet::from_iter).map(|ps| if t == a.TSet { V::Set(Rc::new(RefCell::new(ps))) } else { V::FSet(Rc::new(ps)) });
        if let Ok(me) = &made {
            from_memo_put(p, me);
        }
        (a.DecRef)(p);
        return made;
    }
    if t == PYD_BRIT.load(std::sync::atomic::Ordering::Relaxed) || t == PYD_BRRED.load(std::sync::atomic::Ordering::Relaxed) {
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
    if t == PYD_REPAT.load(std::sync::atomic::Ordering::Relaxed) {
        // CPython's compiled pattern: the program's re.compile(pattern, flags)
        let part = |n: &std::ffi::CStr| -> R {
            let x = (a.GetAttr)(p, n.as_ptr());
            pyd_wrap(pyd_own(x)?, false)
        };
        let r = part(c"pattern").and_then(|pv| part(c"flags").map(|fv| (pv, fv))).and_then(|(pv, fv)| re_fn("compile", Args::p(vec![pv, fv])));
        if let Ok(v) = &r {
            from_memo_put(p, v);
        }
        (a.DecRef)(p);
        return r;
    }
    if t == a.TCFunction {
        if let Some(v) = pyd_callable_v(p) {
            (a.DecRef)(p);
            return Ok(v);
        }
    }
    if deep {
        if let Some(v) = pyd_list_iter(p, t)? {
            (a.DecRef)(p);
            return Ok(v);
        }
    }
    Ok(pyv(p, t == a.TList || t == a.TDict || t == a.TSet))
}

// A list's iterator (or reverse iterator) of Python as the program's own iterator over the program's copy of
// that list, at the same position: copied together with the list (`pickle.loads` of (iter(a), a)), it walks
// the list the program then changes, as in CPython. None for any other object, or a state it cannot read.
unsafe fn pyd_list_iter(p: P, t: P) -> Result<Option<V>, E> {
    let a = pyd_api();
    let name = (a.GetAttr)(t, c"__name__".as_ptr());
    if name.is_null() {
        (a.ErrClear)();
        return Ok(None);
    }
    let text = pyd_text(name).ok();
    (a.DecRef)(name);
    let name = text;
    // 0 list, 1 reversed list, 2 tuple, 3 range, 4 str (ASCII), 5 str
    let kind = match name.as_deref() {
        Some("list_iterator") => 0,
        Some("list_reverseiterator") => 1,
        Some("tuple_iterator") => 2,
        Some("range_iterator") => 3,
        Some("str_ascii_iterator") => 4,
        Some("str_iterator") => 5,
        _ => return Ok(None),
    };
    let rev = kind == 1;
    let red = (a.GetAttr)(p, c"__reduce__".as_ptr());
    if red.is_null() {
        (a.ErrClear)();
        return Ok(None);
    }
    let f = red;
    let red = pyd_call_raw(f, &[]);
    (a.DecRef)(f);
    let red = match red {
        Ok(r) => r,
        Err(_) => {
            (a.ErrClear)();
            return Ok(None);
        }
    };
    // (iter or reversed, (the list,), index), or (iter, (an empty sequence,)) once exhausted
    let got = (|| -> Result<Option<V>, E> {
        if !pyd_type_is(red, a.TTuple) || (a.TupleSize)(red) < 2 {
            return Ok(None);
        }
        let args = (a.TupleGet)(red, 1);
        if !pyd_type_is(args, a.TTuple) || (a.TupleSize)(args) != 1 {
            return Ok(None);
        }
        let seq = (a.TupleGet)(args, 0);
        let ik = [ItKind::ListIter, ItKind::ListReverseIter, ItKind::TupleIter, ItKind::RangeIter, ItKind::StrAsciiIter, ItKind::StrIter][kind];
        // a range iterator: (iter, (range(next, stop, step),), None)
        if kind == 3 {
            let mut parts = [0i64; 3];
            for (k, n) in [c"start", c"stop", c"step"].iter().enumerate() {
                let x = (a.GetAttr)(seq, n.as_ptr());
                if x.is_null() || !pyd_type_is(x, a.TLong) {
                    if x.is_null() { (a.ErrClear)(); } else { (a.DecRef)(x); }
                    return Ok(None);
                }
                let mut over = 0;
                parts[k] = (a.LongAs)(x, &mut over);
                (a.DecRef)(x);
                if over != 0 {
                    return Ok(None);
                }
            }
            return Ok(Some(new_iter(It::Range(parts[0], parts[1], parts[2]), IterClass::Kind(ik))));
        }
        if (a.TupleSize)(red) == 2 {
            return Ok(Some(new_iter(It::Done, IterClass::Kind(ik))));
        }
        let idx = (a.TupleGet)(red, 2);
        if kind >= 2 {
            if !pyd_type_is(idx, a.TLong) {
                return Ok(None);
            }
            let i = (a.LongAs)(idx, &mut 0).max(0) as usize;
            (a.IncRef)(seq);
            return Ok(match pyd_wrap(seq, true)? {
                V::Tuple(t) if kind == 2 => Some(new_iter(It::Tuple(t.clone(), i.min(t.len())), IterClass::Kind(ik))),
                V::Str(x) if kind >= 4 => {
                    let b = x.text().char_indices().nth(i).map_or(x.text().len(), |(b, _)| b);
                    Some(new_iter(It::Str(x.clone(), b), IterClass::Kind(ik)))
                }
                _ => None,
            });
        }
        if !pyd_type_is(seq, a.TList) || !pyd_type_is(idx, a.TLong) {
            return Ok(None);
        }
        let i = (a.LongAs)(idx, &mut 0);
        (a.IncRef)(seq);
        let V::List(l) = pyd_wrap(seq, true)? else { return Ok(None) };
        let n = l.borrow().len() as i64;
        Ok(Some(if rev {
            new_iter(if (0..n).contains(&i) { It::RevList(l, i as usize + 1) } else { It::Done }, IterClass::Kind(ItKind::ListReverseIter))
        } else {
            new_iter(It::List(l, i.max(0) as usize), IterClass::Kind(ItKind::ListIter))
        }))
    })();
    (a.DecRef)(red);
    got
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
    let setstate = func(sym("__setstate__"), |a: Args| match (a.p.first(), a.p.get(1)) {
        (Some(it), Some(st)) => iter_setstate_v(it, st),
        _ => type_err("__setstate__() takes exactly one argument".into()),
    });
    for (n, f) in [(c"_ref", func(sym("_ref"), pickle_ref)), (c"_lookup", func(sym("_lookup"), pickle_lookup_fn)), (c"setstate", setstate), (c"_rebuild_obj", func(sym("_rebuild_obj"), pickle_rebuild))] {
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
    // a functools.partial (or an instance of a subclass of it): its own __reduce__, (class, (func,), state),
    // rebuilt by _brevi_dyn._rebuild in the loading program as the program's own partial again
    if is_partial_v(&v) {
        let r = call_method(&v, "__reduce__", Args::p(vec![]))?;
        return Ok(tup(vec![V::Int(3), r]));
    }
    match &v {
        V::Func(f) if f.kind.get() != 3 => {}
        V::Class(_) => {}
        // a module's function (operator.pow): by reference to that module, as CPython pickles its own
        V::Func(f) if matches!(f.attrs.borrow().iter().find(|(k, _)| *k == "__bv_mod__"), Some((_, V::Str(_)))) => {
            let m = f.attrs.borrow().iter().find(|(k, _)| *k == "__bv_mod__").map(|(_, x)| x.clone()).unwrap_or(V::None);
            let V::Str(m) = m else { return Err(unsupported("pickling a builtin function")) };
            let n = f.name.get();
            return match modfn_cached(m.text(), n) {
                Some(found) if is(&found, &v) => Ok(tup(vec![V::Int(0), st(m.text().to_string()), s(n)])),
                _ => Err(unsupported("pickling a builtin function")),
            };
        }
        V::Func(_) => return Err(unsupported("pickling a builtin function")),
        V::Bound(_) | V::BMeth(_) => return Err(unsupported("pickling a bound method")),
        _ => return Ok(V::None),
    }
    let text = |x: R| -> Result<Option<String>, E> {
        Ok(match x? {
            V::Str(t) => Some(t.text().to_string()),
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
// _brevi_dyn._rebuild(cls, args, state): cls(*args), then its __setstate__(state), as pickle rebuilds an
// object from (cls, args, state); the object stays the program's own.
fn pickle_rebuild(a: Args) -> R {
    // a list, dict or set pickle made is Python's, handed over live: the program's own copy is what it keeps
    let native = |v: &V| -> R {
        match pyv_of(v) {
            Some(py) if unsafe { py.exact() } => unsafe {
                (pyd_api().IncRef)(py.p);
                pyd_wrap(py.p, true)
            },
            _ => Ok(v.clone()),
        }
    };
    let cls = a.p.first().cloned().unwrap_or(V::None);
    let args = match a.p.get(1) {
        Some(V::Tuple(t)) => t.iter().map(native).collect::<Result<Vec<_>, E>>()?,
        _ => vec![],
    };
    let state = match a.p.get(2) {
        Some(V::Tuple(t)) => Some(tup(t.iter().map(native).collect::<Result<Vec<_>, E>>()?)),
        Some(V::None) | None => None,
        Some(x) => Some(native(x)?),
    };
    let o = call(&cls, Args::p(args))?;
    if let Some(st) = state {
        call_method(&o, "__setstate__", Args::p(vec![st]))?;
    }
    Ok(o)
}
fn pickle_lookup_fn(a: Args) -> R {
    let t = |i: usize| match a.p.get(i) {
        Some(V::Str(x)) => Ok(x.text().to_string()),
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
    if let Some(c) = iter_class_named(m, q) {
        return Ok(c);
    }
    if let Some(f) = modfn_cached(m, q) {
        return Ok(f);
    }
    let mut parts = q.split('.');
    let first = parts.next().unwrap_or("");
    let mut v = match m {
        "__main__" => match MAIN_GTAB.with(|g| g.get()) {
            Some(tab) => glob_item(tab, &st(first.to_string())).map_err(|_| missing())?,
            None => return Err(missing()),
        },
        "functools" if first == "partial" => V::Class(partial_class()),
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
    // CPython's frames are under this call: the program's threads may not switch stacks until it returns
    GT_CB.with(|c| c.set(c.get() + 1));
    struct CbGuard;
    impl Drop for CbGuard {
        fn drop(&mut self) {
            GT_CB.with(|c| c.set(c.get() - 1));
        }
    }
    let _cb = CbGuard;
    // a program using warnings: where this library call reached the program (see _install_warn)
    let warn_on = PYD_WARN_ON.load(std::sync::atomic::Ordering::Relaxed);
    if warn_on {
        if let Err(e) = pyd_helper("bpush", &[]) {
            pyd_raise(&e);
            return std::ptr::null_mut();
        }
    }
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
    if warn_on {
        let _ = pyd_helper("bpop", &[]);
    }
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

// What pickle saves for an iterator of the program: its __reduce__ (rt_itred.rs), with Brevi's `iter` and
// `reversed` swapped for CPython's (a list iterator loads as CPython's list iterator, as before). The classes it
// names (itertools.chain, map, ...) pickle by reference to the program's own (pickle_ref), so the pickle loads
// as Brevi's iterator again. The callback runs at pickle time, so it sees the iterator's position then, not
// when the value was handed to the library, and its DeprecationWarning names the line that called pickle.
fn iter_reduce(v: &V) -> R {
    let r = iter_reduce_v(v)?;
    let V::Tuple(t) = &r else { return Ok(r) };
    let mut out = t.to_vec();
    if let Some(V::Func(f)) = out.first() {
        let name = f.name.get();
        if f.kind.get() == 3 && matches!(name, "iter" | "reversed") {
            out[0] = pyd_builtin(name)?;
        }
    }
    // `reversed` is a class now (it_class): its reduce names that class, which pickles as CPython's
    // reversed too, so a reversed list iterator loads as CPython's list_reverseiterator, as before
    if let Some(V::Class(c)) = out.first() {
        if Rc::ptr_eq(c, &it_class(ItKind::Reversed)?) {
            out[0] = pyd_builtin("reversed")?;
        }
    }
    Ok(tup(out))
}

// One conversion's memo, both directions: a list, dict or set reached twice (`[a, a]`, a cycle) becomes one
// object on the other side, as pickle, copy and every library expect. The program-to-Python memo lives for a
// whole library call (pyd_invoke), so a value converted again from a callback during the call (an iterator's
// __reduce__ at pickle time names its list) is the object the call was given. Each entry holds a reference to
// its Python object and a clone of the program's value, so neither address is reused while the memo lives.
thread_local! {
    static TO_MEMO: RefCell<(usize, std::collections::HashMap<usize, (P, V)>)> = RefCell::new((0, std::collections::HashMap::new()));
    static FROM_MEMO: RefCell<(usize, std::collections::HashMap<usize, (P, V)>)> = RefCell::new((0, std::collections::HashMap::new()));
}

fn memo_key(v: &V) -> Option<usize> {
    match v {
        V::List(l) => Some(Rc::as_ptr(l) as *const u8 as usize),
        V::Dict(d) => Some(Rc::as_ptr(d) as *const u8 as usize),
        V::Set(x) => Some(Rc::as_ptr(x) as *const u8 as usize),
        V::FSet(x) => Some(Rc::as_ptr(x) as *const u8 as usize),
        // a tee link: one Python object per pickle, so tees that share it share it again when loaded
        V::Ext(_) => teedata_of(v).map(|l| Rc::as_ptr(&l) as *const u8 as usize),
        _ => None,
    }
}

// Remembers `p` as the Python object made for `v` (a new reference is kept by the memo).
unsafe fn to_memo_put(v: &V, p: P) {
    if let Some(k) = memo_key(v) {
        (pyd_api().IncRef)(p);
        TO_MEMO.with(|m| m.borrow_mut().1.insert(k, (p, v.clone())));
    }
}

// The program's value `v` as a new Python object.
unsafe fn pyd_to(v: &V) -> Result<P, E> {
    let a = pyd_api();
    if let Some(k) = memo_key(v) {
        if let Some(p) = TO_MEMO.with(|m| m.borrow().1.get(&k).map(|e| e.0)) {
            (a.IncRef)(p);
            return Ok(p);
        }
    }
    to_memo_enter();
    let r = pyd_to_new(v);
    to_memo_exit();
    r
}

fn to_memo_enter() {
    TO_MEMO.with(|m| m.borrow_mut().0 += 1);
}

unsafe fn to_memo_exit() {
    let held = TO_MEMO.with(|m| {
        let mut m = m.borrow_mut();
        m.0 -= 1;
        if m.0 == 0 { m.1.drain().map(|(_, e)| e).collect::<Vec<_>>() } else { vec![] }
    });
    held.into_iter().for_each(|(p, _)| (pyd_api().DecRef)(p)); // the program's values drop here, after
}

unsafe fn pyd_to_new(v: &V) -> Result<P, E> {
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
        V::Str(x) => pyd_own(pyd_str_obj(x.text()))?,
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
            // remembered before its items, so a list that holds itself comes out holding itself
            for i in 0..items.len() {
                (a.IncRef)(a.None);
                (a.ListSet)(p, i as isize, a.None);
            }
            to_memo_put(v, p);
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
            to_memo_put(v, p);
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
            let p = if let V::FSet(_) = v {
                let f = (a.FrozenSetNew)(p);
                (a.DecRef)(p);
                pyd_own(f)?
            } else {
                p
            };
            to_memo_put(v, p);
            p
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
            // an itertools._tee_dataobject (in a tee's __reduce__ state): pickled through its own __reduce__,
            // and the program's own value again when it comes back
            if let Some(l) = teedata_of(v) {
                let red = func(sym("__reduce__"), move |_a: Args| teedata_reduce(&l));
                let r = pyd_callable(&red)?;
                let b = pyd_callable(v)?;
                let o = pyd_mod_call("_brevi_dyn", "red", &[r, b]);
                (a.DecRef)(r);
                (a.DecRef)(b);
                let o = o?;
                to_memo_put(v, o);
                let t = (a.Type)(o);
                (a.DecRef)(t);
                PYD_BRRED.store(t, std::sync::atomic::Ordering::Relaxed);
                return Ok(o);
            }
            // a bytearray would arrive as a copy, so the library's changes would not reach the program's
            if bytearray_of(v).is_some() || memview_of(v).is_some() {
                return Err(unsupported("a bytearray or memoryview passed to a library CPython runs"));
            }
            if let Some(b) = bytes_of(v) {
                return pyd_own((a.BytesFrom)(b.as_ptr() as *const c_char, b.len() as isize));
            }
            if let Some((re, im)) = cplx_of(v) {
                return pyd_own((a.ComplexFrom)(re, im));
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
                // a compiled pattern goes in as CPython's re.compile(pattern, flags), which is what it pickles as
                "Pattern" if x.tname() == "re.Pattern" => {
                    let pat = pyd_to(&getattr(v, "pattern")?)?;
                    let fl = match pyd_to(&getattr(v, "flags")?) {
                        Ok(f) => f,
                        Err(e) => {
                            (a.DecRef)(pat);
                            return Err(e);
                        }
                    };
                    let r = pyd_mod_call("re", "compile", &[pat, fl]);
                    (a.DecRef)(pat);
                    (a.DecRef)(fl);
                    let o = r?;
                    to_memo_put(v, o);
                    let t = (a.Type)(o);
                    (a.DecRef)(t);
                    PYD_REPAT.store(t, std::sync::atomic::Ordering::Relaxed);
                    o
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
    // a bytearray of the program handed over as a copy: (the program's bytearray, the copy, the copy's refcount
    // once the arguments hold it); see pyd_ba_sync
    ba: Vec<(V, P, i64)>,
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
            for (_, p, _) in &self.ba {
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

// A bytearray goes to CPython as a copy, so a library's changes would not reach the program's bytearray, and
// the program's later changes would not reach the library. Only for a call into the C code of array, struct or
// zlib (_brevi_dyn.ba_ok) whose other arguments are plain values, so no code of the program runs while the copy
// is out, a bytearray goes as a copy whose changes are copied back after the call (pyd_ba_sync), and the call
// stops if the library kept a reference to the copy. Anything else stays refused (pyd_to_new).
// A bytearray, or an instance of a bytearray subclass whose classes hold only plain values (no methods, so the
// C code, which sees only the buffer, cannot tell it from its bytearray): the bytearray it shares its bytes with.
fn ba_plain(x: &V) -> Option<V> {
    let data = |v: &V| matches!(v, V::None | V::Bool(_) | V::Int(_) | V::Str(_)) || matches!(v, V::Tuple(t) if t.as_slice().iter().all(|e| matches!(e, V::Str(_))));
    match x {
        V::Ext(_) if bytearray_of(x).is_some() => Some(x.clone()),
        V::Obj(o) if is_ba_sub(x) && o.cls.mro.borrow().iter().all(|c| c.module == "builtins" || c.attrs.borrow().iter().all(|(_, v)| data(v))) => ba_val(x),
        _ => None,
    }
}

unsafe fn pyd_ba_call(f: P, pre: &[V], args: &Args) -> bool {
    let all = || pre.iter().chain(args.p.iter()).chain(args.k.iter().map(|(_, x)| x));
    let bas: Vec<V> = all().filter_map(ba_plain).collect();
    if bas.is_empty() {
        return false;
    }
    let plain = |x: &V| matches!(x, V::None | V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_) | V::Str(_)) || is_bytes_v(x) || ba_plain(x).is_some();
    if !all().all(plain) {
        return false;
    }
    // the same bytes twice would go as two copies
    let ds: Vec<_> = bas.iter().map(|b| bytearray_of(b).unwrap().d.clone()).collect();
    if ds.iter().enumerate().any(|(i, d)| ds[..i].iter().any(|e| Rc::ptr_eq(d, e))) {
        return false;
    }
    matches!(pyd_helper("ba_ok", &[f]), Ok(V::Bool(true)))
}

unsafe fn pyd_ba_copy(x: &V, out: &mut Vec<(V, P, i64)>) -> Result<P, E> {
    let a = pyd_api();
    let x = &ba_plain(x).unwrap();
    let b = bytearray_of(x).unwrap().d.borrow().clone();
    let bs = pyd_own((a.BytesFrom)(b.as_ptr() as *const c_char, b.len() as isize))?;
    let k = pyd_builtins_attr("bytearray");
    let k = match k {
        Ok(k) => k,
        Err(e) => {
            (a.DecRef)(bs);
            return Err(e);
        }
    };
    let p = pyd_call_raw(k, &[bs]);
    (a.DecRef)(k);
    (a.DecRef)(bs);
    let p = p?;
    (a.IncRef)(p); // one for the arguments, one held in PydArgs.ba
    out.push((x.clone(), p, 0));
    Ok(p)
}

unsafe fn pyd_refcount(p: P) -> Result<i64, E> {
    match pyd_helper("rc", &[p])? {
        V::Int(n) => Ok(n),
        _ => Err(exc("SystemError", "refcount".into())),
    }
}

// After a call that took bytearray copies: stop if the library still holds one (it would no longer be the
// program's bytearray), else copy the library's changes back into the program's bytearray.
unsafe fn pyd_ba_sync(args: &PydArgs) -> Result<(), E> {
    for (v, p, rc) in &args.ba {
        if pyd_refcount(*p)? != *rc {
            return Err(unsupported("a bytearray that a library CPython runs keeps after the call"));
        }
        let nb = bytes_of(&pyd_helper("babytes", &[*p])?).unwrap_or_default();
        let ba = bytearray_of(v).unwrap();
        if *ba.d.borrow() == nb {
            continue;
        }
        // CPython would raise BufferError resizing an exported bytearray; the copy was not exported
        if ba.d.borrow().len() != nb.len() && ba.ex.get() > 0 {
            return Err(unsupported("a library CPython runs resizing an exported bytearray"));
        }
        *ba.d.borrow_mut() = nb;
    }
    Ok(())
}

unsafe fn pyd_args(f: P, pre: &[V], args: &Args) -> Result<PydArgs, E> {
    let a = pyd_api();
    let n = pre.len() + args.p.len();
    let mut out = PydArgs { t: (a.TupleNew)(n as isize), kw: std::ptr::null_mut(), back: vec![], ba: vec![] };
    let ba_call = pyd_ba_call(f, pre, args);
    // the copies, released if building the arguments fails part way
    struct Hold(Vec<(V, P, i64)>);
    impl Drop for Hold {
        fn drop(&mut self) {
            self.0.iter().for_each(|(_, p, _)| unsafe { (pyd_api().DecRef)(*p) });
        }
    }
    let mut hold = Hold(vec![]);
    let ba = &mut hold.0;
    let mut one = |x: &V, back: &mut Vec<(V, P, Vec<P>)>| -> Result<P, E> {
        if ba_call && ba_plain(x).is_some() {
            return pyd_ba_copy(x, ba);
        }
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
    // what each copy's refcount is while only the arguments and PydArgs hold it
    for e in hold.0.iter_mut() {
        e.2 = pyd_refcount(e.1)?;
    }
    out.ba = std::mem::take(&mut hold.0);
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
    to_memo_enter();
    let pa = pyd_args(f, pre, args);
    let r = match &pa {
        Ok(pa) => (a.Call)(f, pa.t, pa.kw),
        Err(_) => std::ptr::null_mut(),
    };
    to_memo_exit();
    let pa = pa?;
    if r.is_null() {
        let e = pyd_err();
        pyd_ba_sync(&pa)?;
        return Err(e);
    }
    if let Err(e) = pyd_ba_sync(&pa) {
        (a.DecRef)(r);
        return Err(e);
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
        // an array.array next to a bytearray: array's comparison answers NotImplemented and bytearray's compares
        // the two buffers (CPython); the bytearray itself cannot cross to CPython (a copy would not be the object)
        if let Some(theirs) = ba_val(o).and_then(|v| bytes_of(&v)) {
            let is_arr = {
                let _g = PydGil::take();
                matches!(unsafe { pyd_helper("is_array", &[self.p]) }, Ok(V::Bool(true)))
            };
            if is_arr {
                if let Some(mine) = pyd_buf(me) {
                    let (l, r) = if right { (theirs, mine) } else { (mine, theirs) };
                    let c = l.cmp(&r);
                    use std::cmp::Ordering::*;
                    return Some(Ok(V::Bool(match op {
                        "==" => c == Equal,
                        "!=" => c != Equal,
                        "<" => c == Less,
                        "<=" => c != Greater,
                        ">" => c == Greater,
                        ">=" => c != Less,
                        _ => return self.binop(me, op, o, right),
                    })));
                }
            }
        }
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
        if m == "pickle" {
            return pyv_ok(pyd_mod_call("_brevi_dyn", "pmod", &[]));
        }
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
        if m == "pickle" && matches!(n, "load" | "loads" | "Unpickler") {
            let pm = pyd_mod_call("_brevi_dyn", "pmod", &[])?;
            let cn = CString::new(n).unwrap();
            let x = (a.GetAttr)(pm, cn.as_ptr());
            (a.DecRef)(pm);
            return pyd_wrap(pyd_own(x)?, false);
        }
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
// isinstance / issubclass of a library class or value against CPython's collections.abc class `name`
// (ABC_BRIDGE in rt_typing.rs): a bool, or the exception CPython raised.
pub fn pyd_abc(name: &str, v: &V, sub: bool) -> Option<Result<bool, E>> {
    let p = pyv_of(v)?;
    let _g = PydGil::take();
    let n = unsafe { pyd_str_obj(name) };
    let r = unsafe { pyd_helper(if sub { "cabc_sub" } else { "cabc_inst" }, &[n, p.p]) };
    unsafe { (pyd_api().DecRef)(n) };
    Some(match r {
        Ok(V::Bool(b)) => Ok(b),
        Ok(x) => truth(&x),
        Err(e) => Err(e),
    })
}
// The bytes of a library object's buffer (memoryview(o).tobytes() in CPython), or None when it has none.
fn pyd_buf(v: &V) -> Option<Vec<u8>> {
    let p = pyv_of(v)?;
    let _g = PydGil::take();
    match unsafe { pyd_helper("buf", &[p.p]) } {
        Ok(b) => bytes_of(&b),
        Err(_) => None,
    }
}
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
