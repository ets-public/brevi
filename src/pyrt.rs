// ------------------------------------------------------------------------------------------- Python bridge (typed)
// The typed builds' side of the bridge; pyrt_load.rs comes first.

static AX_PYBAPI: std::sync::OnceLock<Result<ax_pyffi::Api, String>> = std::sync::OnceLock::new();

fn ax_pybapi() -> &'static ax_pyffi::Api {
    match AX_PYBAPI.get_or_init(|| {
        // Inside a printed list or dict a library value shows its repr, as in Python.
        AX_SHOW_ITEM.get_or_init(|| |x| x.downcast_ref::<AxPy>().map(|p| ax_pyb_repr(p.clone()).unwrap_or_default()));
        ax_pyb_load()
    }) {
        Ok(a) => a,
        Err(e) => {
            let fix = if e.starts_with("brevi: --sandbox:") { "change what the message names, or run without --sandbox" } else { "install a CPython 3.13 that has its shared library (libpython), or set BREVI_PYTHON to one" };
            eprintln!("{{\"e\":\"runtime\",\"msg\":{e:?},\"fix\":{fix:?}}}");
            std::process::exit(1)
        }
    }
}

// Holds Python's interpreter lock while alive.
struct AxGil(std::ffi::c_int);
impl AxGil {
    #[inline]
    fn take() -> AxGil {
        AxGil(unsafe { (ax_pybapi().GilEnsure)() })
    }
}
impl Drop for AxGil {
    fn drop(&mut self) {
        unsafe { (ax_pybapi().GilRelease)(self.0) }
    }
}

// A Python object. Each AxPy owns one reference.
pub struct AxPy(ax_pyffi::P);
impl Drop for AxPy {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let _g = AxGil::take();
            unsafe { (ax_pybapi().DecRef)(self.0) }
        }
    }
}
impl Clone for AxPy {
    fn clone(&self) -> AxPy {
        let _g = AxGil::take();
        unsafe { (ax_pybapi().IncRef)(self.0) };
        AxPy(self.0)
    }
}
impl Default for AxPy {
    fn default() -> AxPy {
        ax_pyb_none()
    }
}

// The error Python raised, as a Brevi error that keeps its classes (`except KeyError:` works on it).
fn ax_pyb_err() -> AxErr {
    let _g = AxGil::take();
    let a = ax_pybapi();
    unsafe {
        if (a.ErrOccurred)().is_null() {
            return AxErr("Python failed without an error".into());
        }
        let (mut t, mut v, mut tb) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        (a.ErrFetch)(&mut t, &mut v, &mut tb);
        (a.ErrNormalize)(&mut t, &mut v, &mut tb);
        let ty = AxPy(t);
        let _tb = AxPy(tb);
        let val = AxPy(v);
        let m = AxPy((a.Import)(c"_brevi".as_ptr()));
        let f = if m.0.is_null() { AxPy(std::ptr::null_mut()) } else { AxPy((a.GetAttr)(m.0, c"exc".as_ptr())) };
        let s = if f.0.is_null() || val.0.is_null() { std::ptr::null_mut() } else { (a.Call)(f.0, ax_pyb_tuple_raw(vec![val.clone()]), std::ptr::null_mut()) };
        if s.is_null() {
            (a.ErrClear)();
            let _ = ty;
            return AxErr("Python raised an error".into());
        }
        let s = AxPy(s);
        let text = ax_pyb_text(&s);
        match text.split_once('\u{1}') {
            Some((chain, msg)) => AxErr(format!("\u{1}{chain}\u{1}{msg}")),
            None => AxErr(text),
        }
    }
}

fn ax_pyb_text(p: &AxPy) -> String {
    let _g = AxGil::take();
    let mut n: isize = 0;
    let c = unsafe { (ax_pybapi().UniAs)(p.0, &mut n) };
    if c.is_null() {
        unsafe { (ax_pybapi().ErrClear)() };
        return String::new();
    }
    let b = unsafe { std::slice::from_raw_parts(c as *const u8, n as usize) };
    String::from_utf8_lossy(b).into_owned()
}

#[inline]
fn ax_pyb_own(p: ax_pyffi::P) -> R<AxPy> {
    if p.is_null() { Err(ax_pyb_err()) } else { Ok(AxPy(p)) }
}

fn ax_pyb_tuple_raw(xs: Vec<AxPy>) -> ax_pyffi::P {
    let _g = AxGil::take();
    let a = ax_pybapi();
    unsafe {
        let t = (a.TupleNew)(xs.len() as isize);
        for (i, x) in xs.into_iter().enumerate() {
            (a.TupleSet)(t, i as isize, x.0);
            std::mem::forget(x);
        }
        t
    }
}

pub fn ax_pyb_none() -> AxPy {
    let _g = AxGil::take();
    let a = ax_pybapi();
    unsafe { (a.IncRef)(a.None) };
    AxPy(a.None)
}

fn ax_pyb_attr_raw(o: &AxPy, name: &str) -> R<AxPy> {
    let c = std::ffi::CString::new(name).unwrap();
    ax_pyb_own(unsafe { (ax_pybapi().GetAttr)(o.0, c.as_ptr()) })
}

thread_local! {
    static AX_PYB_MODS: std::cell::RefCell<HashMap<String, AxPy>> = std::cell::RefCell::new(HashMap::new());
}

// A module (imported once).
pub fn ax_pyb_mod(name: impl AsRef<str>) -> R<AxPy> {
    let name: &str = name.as_ref();
    if let Some(m) = AX_PYB_MODS.with(|m| m.borrow().get(name).cloned()) {
        return Ok(m);
    }
    let _g = AxGil::take();
    let c = std::ffi::CString::new(name).unwrap();
    let m = ax_pyb_own(unsafe { (ax_pybapi().Import)(c.as_ptr()) })?;
    // `import urllib.parse` makes urllib.parse reachable from urllib.
    for sub in AX_PYB_SUBS.iter().filter(|s| s.starts_with(&format!("{name}."))) {
        let c = std::ffi::CString::new(*sub).unwrap();
        ax_pyb_own(unsafe { (ax_pybapi().Import)(c.as_ptr()) })?;
    }
    AX_PYB_MODS.with(|x| x.borrow_mut().insert(name.to_string(), m.clone()));
    Ok(m)
}

// `from m import a`
pub fn ax_pyb_from(module: impl AsRef<str>, name: impl AsRef<str>) -> R<AxPy> {
    ax_pyb_fn(module.as_ref(), name.as_ref())
}

fn ax_pyb_fn(module: &str, name: &str) -> R<AxPy> {
    let m = ax_pyb_mod(module)?;
    let _g = AxGil::take();
    // `from PIL import Image`: a submodule not yet imported is imported, as Python's from-import does.
    ax_pyb_attr_raw(&m, name).or_else(|e| {
        let c = std::ffi::CString::new(format!("{module}.{name}")).unwrap();
        ax_pyb_own(unsafe { (ax_pybapi().Import)(c.as_ptr()) }).map_err(|_| e)
    })
}

// o.name
pub fn ax_pyb_get(o: AxPy, name: impl AsRef<str>) -> R<AxPy> {
    let name: &str = name.as_ref();
    let _g = AxGil::take();
    ax_pyb_attr_raw(&o, name)
}

// o.name = v
pub fn ax_pyb_set(o: AxPy, name: impl AsRef<str>, v: AxPy) -> R<()> {
    let name: &str = name.as_ref();
    let _g = AxGil::take();
    let c = std::ffi::CString::new(name).unwrap();
    if unsafe { (ax_pybapi().SetAttr)(o.0, c.as_ptr(), v.0) } < 0 { Err(ax_pyb_err()) } else { Ok(()) }
}

fn ax_pyb_invoke(f: &AxPy, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    let _g = AxGil::take();
    let a = ax_pybapi();
    let keys: Vec<ax_pyffi::P> = args.iter().map(|x| x.0).chain(kw.iter().map(|x| x.1 .0)).collect();
    let t = ax_pyb_tuple_raw(args);
    let d = if kw.is_empty() {
        std::ptr::null_mut()
    } else {
        let d = unsafe { (a.DictNew)() };
        for (k, v) in kw {
            // interned, as CPython's own keyword names are (orjson tells its keywords apart by identity)
            let mut k = ax_pyb_strobj(&k);
            unsafe {
                (a.Intern)(&mut k.0);
                (a.DictSet)(d, k.0, v.0)
            };
        }
        d
    };
    let r = unsafe { (a.Call)(f.0, t, d) };
    unsafe {
        (a.DecRef)(t);
        if !d.is_null() {
            (a.DecRef)(d);
        }
    }
    let r = ax_pyb_own(r);
    if !keys.is_empty() && AX_PYB_BACK.with(|b| !b.borrow().is_empty()) {
        ax_pyb_backs(&keys);
    }
    r
}

// o.name(args, k=v): name "" calls o itself.
pub fn ax_pyb_call(o: AxPy, name: impl AsRef<str>, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    let name: &str = name.as_ref();
    let _ = ax_pyb_flush();
    let _g = AxGil::take();
    let f = if name.is_empty() { o } else { ax_pyb_attr_raw(&o, name)? };
    ax_pyb_invoke(&f, args, kw)
}

// A Python built-in (len, sorted, sum, ...) called on library values.
pub fn ax_pyb_builtin(name: impl AsRef<str>, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    let name: &str = name.as_ref();
    let f = ax_pyb_fn("builtins", name)?;
    let _ = ax_pyb_flush();
    let _g = AxGil::take();
    ax_pyb_invoke(&f, args, kw)
}

fn ax_pyb_flush() -> std::io::Result<()> {
    use std::io::Write;
    std::io::stdout().flush()
}

// o[k]
pub fn ax_pyb_idx(o: AxPy, k: AxPy) -> R<AxPy> {
    let _g = AxGil::take();
    ax_pyb_own(unsafe { (ax_pybapi().GetItem)(o.0, k.0) })
}
// o[k] = v
pub fn ax_pyb_setidx(o: AxPy, k: AxPy, v: AxPy) -> R<()> {
    let _g = AxGil::take();
    if unsafe { (ax_pybapi().SetItem)(o.0, k.0, v.0) } < 0 { Err(ax_pyb_err()) } else { Ok(()) }
}
// del o[k]
pub fn ax_pyb_delidx(o: AxPy, k: AxPy) -> R<()> {
    let _g = AxGil::take();
    if unsafe { (ax_pybapi().DelItem)(o.0, k.0) } < 0 { Err(ax_pyb_err()) } else { Ok(()) }
}
// a:b:c
pub fn ax_pyb_slice(a: AxPy, b: AxPy, c: AxPy) -> R<AxPy> {
    let _g = AxGil::take();
    ax_pyb_own(unsafe { (ax_pybapi().SliceNew)(a.0, b.0, c.0) })
}

// a OP b, with Python's meaning (operator.add, operator.lt, ...).
pub fn ax_pyb_op(op: impl AsRef<str>, a: AxPy, b: AxPy) -> R<AxPy> {
    let op: &str = op.as_ref();
    let (m, f) = match op {
        "+" => ("operator", "add"),
        "-" => ("operator", "sub"),
        "*" => ("operator", "mul"),
        "/" => ("operator", "truediv"),
        "//" => ("operator", "floordiv"),
        "%" => ("operator", "mod"),
        "**" => ("operator", "pow"),
        "@" => ("operator", "matmul"),
        "<<" => ("operator", "lshift"),
        ">>" => ("operator", "rshift"),
        "&" => ("operator", "and_"),
        "|" => ("operator", "or_"),
        "^" => ("operator", "xor"),
        "<" => ("operator", "lt"),
        "<=" => ("operator", "le"),
        ">" => ("operator", "gt"),
        ">=" => ("operator", "ge"),
        "==" => ("operator", "eq"),
        "!=" => ("operator", "ne"),
        "is" => ("operator", "is_"),
        "is not" => ("_brevi", "isnot"),
        "in" => ("_brevi", "contains"),
        "not in" => ("_brevi", "notin"),
        "+=" => ("operator", "iadd"),
        "-=" => ("operator", "isub"),
        "*=" => ("operator", "imul"),
        "/=" => ("operator", "itruediv"),
        "//=" => ("operator", "ifloordiv"),
        "%=" => ("operator", "imod"),
        "|=" => ("operator", "ior"),
        "&=" => ("operator", "iand"),
        "^=" => ("operator", "ixor"),
        other => return Err(AxErr(format!("operator {other} is not supported on Python values"))),
    };
    let f = ax_pyb_fn(m, f)?;
    let _g = AxGil::take();
    ax_pyb_invoke(&f, vec![a, b], vec![])
}

// -a, +a, ~a
pub fn ax_pyb_unary(op: impl AsRef<str>, a: AxPy) -> R<AxPy> {
    let op: &str = op.as_ref();
    let f = match op {
        "-" => "neg",
        "+" => "pos",
        _ => "invert",
    };
    let f = ax_pyb_fn("operator", f)?;
    let _g = AxGil::take();
    ax_pyb_invoke(&f, vec![a], vec![])
}

// format(o, spec): an f-string's `{o:spec}`.
pub fn ax_pyb_format(o: AxPy, spec: impl AsRef<str>) -> R<String> {
    let spec: &str = spec.as_ref();
    let _g = AxGil::take();
    let s = ax_pyb_strobj(spec);
    let r = ax_pyb_own(unsafe { (ax_pybapi().Format)(o.0, s.0) })?;
    Ok(ax_pyb_text(&r))
}

pub fn ax_pyb_repr(o: AxPy) -> R<String> {
    let _g = AxGil::take();
    let r = ax_pyb_own(unsafe { (ax_pybapi().Repr)(o.0) })?;
    Ok(ax_pyb_text(&r))
}

// int(o), float(o), str(o), bool(o), len(o): back to Brevi values, as Python converts.
pub fn ax_pyb_int(o: AxPy) -> R<i64> {
    let f = ax_pyb_fn("builtins", "int")?;
    let _g = AxGil::take();
    let v = ax_pyb_invoke(&f, vec![o], vec![])?;
    let mut over = 0;
    let n = unsafe { (ax_pybapi().LongAs)(v.0, &mut over) };
    if over != 0 {
        return Err(AxErr("\u{1}OverflowError,ArithmeticError,Exception,BaseException\u{1}int too large for Brevi (more than 64 bits)".into()));
    }
    Ok(n)
}
pub fn ax_pyb_float(o: AxPy) -> R<f64> {
    let f = ax_pyb_fn("builtins", "float")?;
    let _g = AxGil::take();
    let v = ax_pyb_invoke(&f, vec![o], vec![])?;
    Ok(unsafe { (ax_pybapi().FloatAs)(v.0) })
}
pub fn ax_pyb_str(o: AxPy) -> R<String> {
    let _g = AxGil::take();
    let r = ax_pyb_own(unsafe { (ax_pybapi().Str)(o.0) })?;
    Ok(ax_pyb_text(&r))
}
pub fn ax_pyb_bool(o: AxPy) -> R<bool> {
    let _g = AxGil::take();
    match unsafe { (ax_pybapi().IsTrue)(o.0) } {
        n if n < 0 => Err(ax_pyb_err()),
        n => Ok(n == 1),
    }
}
pub fn ax_pyb_len(o: AxPy) -> R<i64> {
    let f = ax_pyb_fn("builtins", "len")?;
    let _g = AxGil::take();
    let v = ax_pyb_invoke(&f, vec![o], vec![])?;
    Ok(unsafe { (ax_pybapi().LongAs)(v.0, &mut 0) })
}

// `a, b = o`: o's items, exactly n of them.
pub fn ax_pyb_items(o: AxPy, n: i64) -> R<Vec<AxPy>> {
    let f = ax_pyb_fn("_brevi", "unpack")?;
    let t = ax_pyb_invoke(&f, vec![o, ax_pyb(n)], vec![])?;
    let mut out = Vec::new();
    for x in ax_pyb_iter(t)? {
        out.push(x?);
    }
    Ok(out)
}

// list(o) as a Brevi list of its items.
pub fn ax_pyb_list(o: AxPy) -> R<Vec<AxPy>> {
    let mut out = Vec::new();
    for x in ax_pyb_iter(o)? {
        out.push(x?);
    }
    Ok(out)
}

pub struct AxPyIter(AxPy);
impl Iterator for AxPyIter {
    type Item = R<AxPy>;
    fn next(&mut self) -> Option<R<AxPy>> {
        let _g = AxGil::take();
        let p = unsafe { (ax_pybapi().IterNext)(self.0 .0) };
        if p.is_null() {
            if unsafe { (ax_pybapi().ErrOccurred)() }.is_null() { None } else { Some(Err(ax_pyb_err())) }
        } else {
            Some(Ok(AxPy(p)))
        }
    }
}
pub fn ax_pyb_iter(o: AxPy) -> R<AxPyIter> {
    let _g = AxGil::take();
    Ok(AxPyIter(ax_pyb_own(unsafe { (ax_pybapi().GetIter)(o.0) })?))
}
// `for x in o:` (an error while stepping stops the program).
pub struct AxPyEach(AxPyIter);
impl Iterator for AxPyEach {
    type Item = AxPy;
    fn next(&mut self) -> Option<AxPy> {
        match self.0.next()? {
            Ok(p) => Some(p),
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl AxEach for AxPy {
    type It = AxPyEach;
    fn ax_each(self) -> AxPyEach {
        match ax_pyb_iter(self) {
            Ok(i) => AxPyEach(i),
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl AxTruth for AxPy {
    fn ax_truth(&self) -> bool {
        match ax_pyb_bool(self.clone()) {
            Ok(b) => b,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl Show for AxPy {
    fn show(&self) -> String {
        match ax_pyb_str(self.clone()) {
            Ok(s) => s,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl std::fmt::Debug for AxPy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", ax_pyb_repr(self.clone()).unwrap_or_default())
    }
}
impl PartialEq for AxPy {
    fn eq(&self, o: &AxPy) -> bool {
        ax_pyb_op("==", self.clone(), o.clone()).map(|v| v.ax_truth()).unwrap_or(false)
    }
}

// Brevi values passed to Python.
pub trait AxToPy {
    fn ax_to_py(&self) -> AxPy;
}
pub fn ax_pyb<T: AxToPy>(x: T) -> AxPy {
    x.ax_to_py()
}
fn ax_pyb_strobj(s: &str) -> AxPy {
    let _g = AxGil::take();
    AxPy(unsafe { (ax_pybapi().UniFrom)(s.as_ptr() as *const std::ffi::c_char, s.len() as isize) })
}
impl<T: AxToPy + ?Sized> AxToPy for &T {
    fn ax_to_py(&self) -> AxPy {
        (**self).ax_to_py()
    }
}
impl AxToPy for AxPy {
    fn ax_to_py(&self) -> AxPy {
        self.clone()
    }
}
impl AxToPy for i64 {
    fn ax_to_py(&self) -> AxPy {
        if *self == i64::MIN && ax_py_on() {
            return ax_pyb_none();
        }
        let _g = AxGil::take();
        AxPy(unsafe { (ax_pybapi().LongFrom)(*self) })
    }
}
impl AxToPy for f64 {
    fn ax_to_py(&self) -> AxPy {
        if *self == f64::MIN && ax_py_on() {
            return ax_pyb_none();
        }
        let _g = AxGil::take();
        AxPy(unsafe { (ax_pybapi().FloatFrom)(*self) })
    }
}
impl AxToPy for bool {
    fn ax_to_py(&self) -> AxPy {
        let _g = AxGil::take();
        AxPy(unsafe { (ax_pybapi().BoolFrom)(*self as std::ffi::c_long) })
    }
}
impl AxToPy for String {
    fn ax_to_py(&self) -> AxPy {
        if self == "\u{0}None" {
            return ax_pyb_none();
        }
        ax_pyb_strobj(self)
    }
}
impl AxToPy for str {
    fn ax_to_py(&self) -> AxPy {
        ax_pyb_strobj(self)
    }
}
impl AxToPy for () {
    fn ax_to_py(&self) -> AxPy {
        ax_pyb_none()
    }
}
impl<T: AxToPy> AxToPy for Vec<T> {
    fn ax_to_py(&self) -> AxPy {
        let items: Vec<AxPy> = self.iter().map(|x| x.ax_to_py()).collect();
        let _g = AxGil::take();
        let a = ax_pybapi();
        unsafe {
            let l = (a.ListNew)(items.len() as isize);
            for (i, x) in items.into_iter().enumerate() {
                (a.ListSet)(l, i as isize, x.0);
                std::mem::forget(x);
            }
            AxPy(l)
        }
    }
}
impl<K: AxToPy + Hash + Eq + Ord + Clone, V: AxToPy> AxToPy for AxM<K, V> {
    fn ax_to_py(&self) -> AxPy {
        let pairs: Vec<(AxPy, AxPy)> = self.iter().map(|(k, v)| (k.ax_to_py(), v.ax_to_py())).collect();
        let _g = AxGil::take();
        let a = ax_pybapi();
        unsafe {
            let d = (a.DictNew)();
            for (k, v) in pairs {
                (a.DictSet)(d, k.0, v.0);
            }
            AxPy(d)
        }
    }
}
impl AxToPy for Json {
    fn ax_to_py(&self) -> AxPy {
        match self {
            Json::Null => ax_pyb_none(),
            Json::Bool { v } => v.ax_to_py(),
            Json::Num { v, f: false } if v.fract() == 0.0 && v.abs() < 9e15 => (*v as i64).ax_to_py(),
            Json::Num { v, .. } => v.ax_to_py(),
            Json::Str { v } => v.ax_to_py(),
            Json::Arr { v } => v.ax_to_py(),
            Json::Tup { v } => {
                let items: Vec<AxPy> = v.iter().map(|x| x.ax_to_py()).collect();
                let _g = AxGil::take();
                AxPy(ax_pyb_tuple_raw(items))
            }
            Json::Obj { v } => v.ax_to_py(),
        }
    }
}
impl AxToPy for Dec {
    fn ax_to_py(&self) -> AxPy {
        let f = ax_pyb_fn("decimal", "Decimal").unwrap_or_else(|e| panic!("{}", ax_exc_msg(&e.0)));
        let _g = AxGil::take();
        ax_pyb_invoke(&f, vec![self.show().ax_to_py()], vec![]).unwrap_or_else(|e| panic!("{}", ax_exc_msg(&e.0)))
    }
}
macro_rules! ax_pyb_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: AxToPy),+> AxToPy for ($($t,)+) {
            fn ax_to_py(&self) -> AxPy {
                let items = vec![$(self.$i.ax_to_py()),+];
                let _g = AxGil::take();
                AxPy(ax_pyb_tuple_raw(items))
            }
        }
    };
}
ax_pyb_tuple!(A 0);
ax_pyb_tuple!(A 0, B 1);
ax_pyb_tuple!(A 0, B 1, C 2);
ax_pyb_tuple!(A 0, B 1, C 2, D 3);
ax_pyb_tuple!(A 0, B 1, C 2, D 3, E 4);
ax_pyb_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
impl AxIter for AxPy {
    type I<'a> = AxPyEach;
    fn ax_iter<'a>(&'a self) -> AxPyEach {
        self.clone().ax_each()
    }
}
impl PartialOrd for AxPy {
    fn partial_cmp(&self, o: &AxPy) -> Option<std::cmp::Ordering> {
        let t = |op: &str| ax_pyb_op(op, self.clone(), o.clone()).map(|v| v.ax_truth()).unwrap_or(false);
        if t("<") {
            Some(std::cmp::Ordering::Less)
        } else if t("==") {
            Some(std::cmp::Ordering::Equal)
        } else if t(">") {
            Some(std::cmp::Ordering::Greater)
        } else {
            None
        }
    }
}
// A Python dict made as a Brevi record, given to Python: a dict again.
pub fn ax_pyb_dict(v: Vec<(String, AxPy)>) -> AxPy {
    let _g = AxGil::take();
    let a = ax_pybapi();
    unsafe {
        let d = (a.DictNew)();
        for (k, x) in v {
            let k = ax_pyb_strobj(&k);
            (a.DictSet)(d, k.0, x.0);
        }
        AxPy(d)
    }
}

// ------------------------------------------------------------------------------------------- write-back
// Brevi values Python gets back: what a library changed in a list or dict passed to it (random.shuffle(xs),
// heapq.heappush(h, x)), and items of `a, b = value`.
pub trait AxFromPy: Sized {
    fn ax_from_py(p: &AxPy) -> R<Self>;
}
impl AxFromPy for AxPy {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        Ok(p.clone())
    }
}
impl AxFromPy for i64 {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        if p.0 == ax_pybapi().None {
            return Ok(i64::MIN);
        }
        let _g = AxGil::take();
        let mut over = 0;
        let n = unsafe { (ax_pybapi().LongAs)(p.0, &mut over) };
        if n == -1 && !unsafe { (ax_pybapi().ErrOccurred)() }.is_null() {
            return Err(ax_pyb_err());
        }
        if over != 0 {
            return Err(AxErr("int too large for Brevi (more than 64 bits)".into()));
        }
        Ok(n)
    }
}
impl AxFromPy for f64 {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        let _g = AxGil::take();
        let x = unsafe { (ax_pybapi().FloatAs)(p.0) };
        if x == -1.0 && !unsafe { (ax_pybapi().ErrOccurred)() }.is_null() {
            return Err(ax_pyb_err());
        }
        Ok(x)
    }
}
impl AxFromPy for bool {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        ax_pyb_bool(p.clone())
    }
}
impl AxFromPy for String {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        let _g = AxGil::take();
        let mut n: isize = 0;
        let c = unsafe { (ax_pybapi().UniAs)(p.0, &mut n) };
        if c.is_null() {
            return Err(ax_pyb_err());
        }
        Ok(String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(c as *const u8, n as usize) }).into_owned())
    }
}
impl<T: AxFromPy> AxFromPy for Vec<T> {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        let mut out = vec![];
        for x in ax_pyb_iter(p.clone())? {
            out.push(T::ax_from_py(&x?)?);
        }
        Ok(out)
    }
}
impl<K: AxFromPy + Hash + Eq + Ord + Clone, V: AxFromPy> AxFromPy for AxM<K, V> {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        let items = ax_pyb_call(p.clone(), "items", vec![], vec![])?;
        let mut m = AxM::new();
        for kv in ax_pyb_iter(items)? {
            let kv = ax_pyb_items(kv?, 2)?;
            m.insert(K::ax_from_py(&kv[0])?, V::ax_from_py(&kv[1])?);
        }
        Ok(m)
    }
}
impl AxFromPy for Json {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        let s = ax_pyb_call(ax_pyb_mod("json")?, "dumps", vec![p.clone()], vec![])?;
        json(ax_pyb_str(s)?)
    }
}
macro_rules! ax_pyb_from_tuple {
    ($n:expr; $($t:ident $i:tt),+) => {
        impl<$($t: AxFromPy),+> AxFromPy for ($($t,)+) {
            fn ax_from_py(p: &AxPy) -> R<Self> {
                let xs = ax_pyb_items(p.clone(), $n)?;
                Ok(($($t::ax_from_py(&xs[$i])?,)+))
            }
        }
    };
}
ax_pyb_from_tuple!(1; A 0);
ax_pyb_from_tuple!(2; A 0, B 1);
ax_pyb_from_tuple!(3; A 0, B 1, C 2);
ax_pyb_from_tuple!(4; A 0, B 1, C 2, D 3);

// Whether a value of this type can be changed by Python (a list or dict): it is written back.
pub trait AxPyBack {
    fn ax_back(&mut self, _p: &AxPy) {}
    fn ax_backs(&self) -> bool {
        false
    }
}
impl AxPyBack for i64 {}
impl AxPyBack for f64 {}
impl AxPyBack for bool {}
impl AxPyBack for String {}
impl AxPyBack for AxPy {}
impl AxPyBack for Json {}
impl AxPyBack for Dec {}
impl AxPyBack for () {}
impl<T: AxFromPy> AxPyBack for Vec<T> {
    fn ax_back(&mut self, p: &AxPy) {
        match Self::ax_from_py(p) {
            Ok(v) => *self = v,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
    fn ax_backs(&self) -> bool {
        true
    }
}
impl<K: AxFromPy + Hash + Eq + Ord + Clone, V: AxFromPy> AxPyBack for AxM<K, V> {
    fn ax_back(&mut self, p: &AxPy) {
        match Self::ax_from_py(p) {
            Ok(v) => *self = v,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
    fn ax_backs(&self) -> bool {
        true
    }
}
thread_local! {
    static AX_PYB_BACK: std::cell::RefCell<Vec<(ax_pyffi::P, Box<dyn FnOnce()>)>> = std::cell::RefCell::new(vec![]);
}
// `f(xs)` with xs a Brevi list or dict: Python gets a copy, and after the call xs takes what Python made of it.
pub fn ax_pyb_argm<T: AxToPy + AxPyBack + 'static>(x: &mut T) -> AxPy {
    let p = x.ax_to_py();
    if x.ax_backs() {
        let ptr: *mut T = x;
        let obj = p.clone();
        let key = p.0;
        AX_PYB_BACK.with(|b| b.borrow_mut().push((key, Box::new(move || unsafe { (*ptr).ax_back(&obj) }))));
    }
    p
}
// After a call: the write-backs of its own arguments.
fn ax_pyb_backs(args: &[ax_pyffi::P]) {
    let mine: Vec<Box<dyn FnOnce()>> = AX_PYB_BACK.with(|b| {
        let mut b = b.borrow_mut();
        let mut out = vec![];
        let mut k = 0;
        while k < b.len() {
            if args.contains(&b[k].0) {
                out.push(b.remove(k).1);
            } else {
                k += 1;
            }
        }
        out
    });
    for f in mine {
        f();
    }
}

// ------------------------------------------------------------------------------------------- callbacks
// A Brevi lambda given to Python: a Python function (a C function holding the closure in a capsule).
type AxPyFn = Box<dyn Fn(Vec<AxPy>) -> R<AxPy>>;
static mut AX_PYB_DEF: ax_pyffi::MethodDef = ax_pyffi::MethodDef { name: c"brevi_lambda".as_ptr(), meth: ax_pyb_tramp, flags: 1, doc: std::ptr::null() };
unsafe extern "C" fn ax_pyb_free(cap: ax_pyffi::P) {
    let p = (ax_pybapi().CapsuleGet)(cap, std::ptr::null()) as *mut AxPyFn;
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}
unsafe extern "C" fn ax_pyb_tramp(cap: ax_pyffi::P, args: ax_pyffi::P) -> ax_pyffi::P {
    let a = ax_pybapi();
    let f = (a.CapsuleGet)(cap, std::ptr::null()) as *const AxPyFn;
    let n = (a.TupleSize)(args);
    let argv: Vec<AxPy> = (0..n)
        .map(|i| {
            let x = (a.TupleGet)(args, i);
            (a.IncRef)(x);
            AxPy(x)
        })
        .collect();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (*f)(argv)));
    let msg = match r {
        Ok(Ok(v)) => {
            let p = v.0;
            std::mem::forget(v);
            return p;
        }
        Ok(Err(e)) => ax_exc_msg(&e.0),
        Err(e) => e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "a Brevi function failed".into()),
    };
    let c = std::ffi::CString::new(msg.replace('\0', "")).unwrap();
    (a.ErrSetString)(a.RuntimeError, c.as_ptr());
    std::ptr::null_mut()
}
fn ax_pyb_func(f: AxPyFn) -> AxPy {
    let a = ax_pybapi();
    let _g = AxGil::take();
    unsafe {
        let p = Box::into_raw(Box::new(f));
        let cap = (a.CapsuleNew)(p as ax_pyffi::P, std::ptr::null(), Some(ax_pyb_free));
        let func = (a.CFunctionNew)(std::ptr::addr_of_mut!(AX_PYB_DEF), cap, std::ptr::null_mut());
        (a.DecRef)(cap);
        AxPy(func)
    }
}
pub fn ax_pyb_fn0<T: AxToPy, F: Fn() -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |_a: Vec<AxPy>| Ok(f().ax_to_py())))
}
pub fn ax_pyb_fn1<T: AxToPy, F: Fn(AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        Ok(f(a.next().unwrap_or_default()).ax_to_py())
    }))
}
pub fn ax_pyb_fn2<T: AxToPy, F: Fn(AxPy, AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        let x = a.next().unwrap_or_default();
        Ok(f(x, a.next().unwrap_or_default()).ax_to_py())
    }))
}
pub fn ax_pyb_fn3<T: AxToPy, F: Fn(AxPy, AxPy, AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        let x = a.next().unwrap_or_default();
        let y = a.next().unwrap_or_default();
        Ok(f(x, y, a.next().unwrap_or_default()).ax_to_py())
    }))
}

// Rust operators on library values, for code the compiler emits directly (sum(i * v ...), a + b).
fn ax_pyb_opx(op: &str, a: AxPy, b: AxPy) -> AxPy {
    match ax_pyb_op(op, a, b) {
        Ok(v) => v,
        Err(e) => panic!("{}", ax_exc_msg(&e.0)),
    }
}
macro_rules! ax_pyb_ops {
    ($($tr:ident $f:ident $op:literal),*) => {$(
        impl std::ops::$tr<AxPy> for AxPy { type Output = AxPy; fn $f(self, o: AxPy) -> AxPy { ax_pyb_opx($op, self, o) } }
        impl std::ops::$tr<i64> for AxPy { type Output = AxPy; fn $f(self, o: i64) -> AxPy { ax_pyb_opx($op, self, o.ax_to_py()) } }
        impl std::ops::$tr<f64> for AxPy { type Output = AxPy; fn $f(self, o: f64) -> AxPy { ax_pyb_opx($op, self, o.ax_to_py()) } }
        impl std::ops::$tr<AxPy> for i64 { type Output = AxPy; fn $f(self, o: AxPy) -> AxPy { ax_pyb_opx($op, self.ax_to_py(), o) } }
        impl std::ops::$tr<AxPy> for f64 { type Output = AxPy; fn $f(self, o: AxPy) -> AxPy { ax_pyb_opx($op, self.ax_to_py(), o) } }
    )*};
}
ax_pyb_ops!(Add add "+", Sub sub "-", Mul mul "*", Div div "/", Rem rem "%");
impl std::ops::Neg for AxPy {
    type Output = AxPy;
    fn neg(self) -> AxPy {
        match ax_pyb_unary("-", self) {
            Ok(v) => v,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl Plus for AxPy {
    type Out = AxPy;
    fn plus(self, o: AxPy) -> AxPy {
        ax_pyb_opx("+", self, o)
    }
}
impl AxNone for AxPy {
    fn ax_none() -> Self {
        ax_pyb_none()
    }
    fn ax_is_none(&self) -> bool {
        self.0 == ax_pyb_none().0
    }
}
impl AxToJson for AxPy {
    fn ax_json(&self) -> Json {
        match <Json as AxFromPy>::ax_from_py(self) {
            Ok(j) => j,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl AxSum<AxPy> for Vec<AxPy> {
    // CPython's own sum: its floats are compensated, which adding the items one by one would lose.
    fn ax_sum(&self) -> AxPy {
        match ax_pyb_builtin("sum", vec![self.ax_to_py()], vec![]) {
            Ok(v) => v,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
impl Eq for AxPy {}
// A library value as a map key or set item: a NaN (x != x) stops the typed run, as the prelude's own floats do.
impl AxNaN for AxPy {
    fn ax_has_nan(&self) -> bool {
        ax_pyb_op("!=", self.clone(), self.clone()).map(|v| v.ax_truth()).unwrap_or(false)
    }
}
impl Ord for AxPy {
    fn cmp(&self, o: &AxPy) -> std::cmp::Ordering {
        self.partial_cmp(o).unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl std::hash::Hash for AxPy {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        let _g = AxGil::take();
        let v = unsafe { (ax_pybapi().Hash)(self.0) };
        if v == -1 {
            let e = ax_pyb_err();
            panic!("{}", ax_exc_msg(&e.0));
        }
        h.write_isize(v);
    }
}
// `self.w -= lr * grad` on a field holding a library value: Python's in-place operator.
macro_rules! ax_pyb_assign_ops {
    ($($tr:ident $f:ident $op:literal),*) => {$(
        impl std::ops::$tr<AxPy> for AxPy { fn $f(&mut self, o: AxPy) { *self = ax_pyb_opx($op, self.clone(), o) } }
        impl std::ops::$tr<i64> for AxPy { fn $f(&mut self, o: i64) { *self = ax_pyb_opx($op, self.clone(), o.ax_to_py()) } }
        impl std::ops::$tr<f64> for AxPy { fn $f(&mut self, o: f64) { *self = ax_pyb_opx($op, self.clone(), o.ax_to_py()) } }
    )*};
}
ax_pyb_assign_ops!(AddAssign add_assign "+=", SubAssign sub_assign "-=", MulAssign mul_assign "*=", DivAssign div_assign "/=");
// `{v:.3f}` in a string: Python's format() of the library value.
impl AxSpec for AxPy {
    fn ax_spec(&self, spec: String) -> String {
        match ax_pyb_format(self.clone(), spec) {
            Ok(s) => s,
            Err(e) => panic!("{}", ax_exc_msg(&e.0)),
        }
    }
}
