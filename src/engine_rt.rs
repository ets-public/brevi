// ------------------------------------------------------------------------------------------- library engine (typed)
// The typed build's library values on Brevi's own engine: the same calls as pyrt.rs (the CPython bridge), over
// the dynamic build's values (brevi_dyn::V) instead of CPython objects. Pasted into a typed program whose
// libraries Brevi all has natively (numpy, httpx, jinja2, ...); the libraries themselves are the dynamic
// build's, compiled into the program's `__dyn` module (engine.rs), so a .bv program gets exactly what a .py
// program gets.

thread_local! {
    static AX_DYN_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AX_DYN_SYMS: std::cell::RefCell<std::collections::HashSet<&'static str>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

// The libraries' module bodies run once, before the first library call.
#[inline]
fn ax_dyn_init() {
    if AX_DYN_ON.with(|o| o.replace(true)) {
        return;
    }
    // Inside a printed list or dict a library value shows its repr, as in Python.
    AX_SHOW_ITEM.get_or_init(|| |x| x.downcast_ref::<AxPy>().map(|p| ax_pyb_repr(p.clone()).unwrap_or_default()));
    if let Err(e) = __dyn::__bv_main() {
        brevi_dyn::out_flush();
        panic!("{}", ax_exc_msg(&ax_dyn_err(e).0));
    }
    brevi_dyn::out_flush();
}

// A name as the engine keeps it (keyword and attribute names are &'static str there).
fn ax_dyn_sym(s: &str) -> &'static str {
    AX_DYN_SYMS.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(x) = t.get(s) {
            return *x;
        }
        let x: &'static str = Box::leak(s.to_string().into_boxed_str());
        t.insert(x);
        x
    })
}

// An exception the engine raised, as a Brevi error that keeps its classes (`except KeyError:` works on it).
fn ax_dyn_err(e: brevi_dyn::E) -> AxErr {
    let v = e.0;
    match &v {
        brevi_dyn::V::Obj(o) => {
            let mut chain: Vec<String> = o.cls.mro.borrow().iter().filter(|c| c.name != "object").map(|c| c.name.to_string()).collect();
            if chain.is_empty() {
                chain.push(o.cls.name.to_string());
            }
            let msg = brevi_dyn::to_str(&v).unwrap_or_default();
            AxErr(format!("\u{1}{}\u{1}{msg}", chain.join(",")))
        }
        x => AxErr(brevi_dyn::repr(x).unwrap_or_default()),
    }
}

// A Brevi error inside a callback the engine runs: the same class when it is a built-in one.
fn ax_dyn_raise(e: &AxErr) -> brevi_dyn::E {
    let (cls, msg) = match e.0.strip_prefix('\u{1}').and_then(|r| r.split_once('\u{1}')) {
        Some((chain, m)) => (chain.split(',').next().unwrap_or("").to_string(), m.to_string()),
        None => (String::new(), e.0.clone()),
    };
    let known = brevi_dyn::BUILTIN_CLASSES.with(|b| b.borrow().contains_key(cls.as_str()));
    brevi_dyn::exc(if known && !cls.is_empty() { ax_dyn_sym(&cls) } else { "RuntimeError" }, msg)
}

#[inline]
fn ax_dyn_r(r: brevi_dyn::R) -> R<AxPy> {
    brevi_dyn::out_flush();
    r.map(AxPy).map_err(ax_dyn_err)
}

// A library value.
#[derive(Clone)]
pub struct AxPy(brevi_dyn::V);
impl Default for AxPy {
    fn default() -> AxPy {
        ax_pyb_none()
    }
}

pub fn ax_pyb_none() -> AxPy {
    AxPy(brevi_dyn::V::None)
}

fn ax_dyn_args(args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> brevi_dyn::Args {
    brevi_dyn::Args { p: args.into_iter().map(|x| x.0).collect(), k: kw.into_iter().map(|(k, v)| (ax_dyn_sym(&k), v.0)).collect() }
}

// A module (its body ran in ax_dyn_init).
pub fn ax_pyb_mod(name: impl AsRef<str>) -> R<AxPy> {
    ax_dyn_init();
    let name: &str = name.as_ref();
    match __dyn::__bv_mod(name) {
        Some(r) => ax_dyn_r(r),
        None => Ok(AxPy(brevi_dyn::module_v(name))),
    }
}

// `from m import a`
pub fn ax_pyb_from(module: impl AsRef<str>, name: impl AsRef<str>) -> R<AxPy> {
    ax_dyn_init();
    let (module, name) = (module.as_ref(), name.as_ref());
    // `dtype=int`, `key=len`: the built-in class or function itself
    if module == "builtins" {
        if brevi_dyn::BUILTIN_CLASSES.with(|b| b.borrow().contains_key(name)) {
            return Ok(AxPy(brevi_dyn::V::Class(brevi_dyn::bclass(name))));
        }
        if let Some(f) = ax_dyn_bi(name) {
            return Ok(AxPy(brevi_dyn::builtin(ax_dyn_sym(name), f)));
        }
    }
    if let Some(r) = __dyn::__bv_from(module, name) {
        return ax_dyn_r(r);
    }
    let m = ax_pyb_mod(module)?;
    ax_pyb_get(m, name)
}

// o.name
pub fn ax_pyb_get(o: AxPy, name: impl AsRef<str>) -> R<AxPy> {
    ax_dyn_r(brevi_dyn::getattr(&o.0, name.as_ref()))
}

// o.name = v
pub fn ax_pyb_set(o: AxPy, name: impl AsRef<str>, v: AxPy) -> R<()> {
    let r = brevi_dyn::setattr(&o.0, ax_dyn_sym(name.as_ref()), v.0);
    brevi_dyn::out_flush();
    r.map_err(ax_dyn_err)
}

fn ax_dyn_invoke(f: Option<&AxPy>, o: &AxPy, name: &str, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    let keys: Vec<usize> = args.iter().chain(kw.iter().map(|x| &x.1)).map(|x| ax_dyn_key(&x.0)).collect();
    let a = ax_dyn_args(args, kw);
    let r = match f {
        Some(f) => brevi_dyn::call(&f.0, a),
        None if name.is_empty() => brevi_dyn::call(&o.0, a),
        None => brevi_dyn::call_method(&o.0, name, a),
    };
    let r = ax_dyn_r(r);
    if AX_PYB_BACK.with(|b| !b.borrow().is_empty()) {
        ax_pyb_backs(&keys);
    }
    r
}

// o.name(args, k=v): name "" calls o itself.
pub fn ax_pyb_call(o: AxPy, name: impl AsRef<str>, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    ax_dyn_init();
    ax_dyn_invoke(None, &o, name.as_ref(), args, kw)
}

// A Python built-in (len, sorted, sum, ...) called on library values: the engine's own.
pub fn ax_pyb_builtin(name: impl AsRef<str>, args: Vec<AxPy>, kw: Vec<(String, AxPy)>) -> R<AxPy> {
    ax_dyn_init();
    let name: &str = name.as_ref();
    let Some(f) = ax_dyn_bi(name) else {
        return Err(AxErr(format!("\u{1}NameError,Exception,BaseException\u{1}name '{name}' is not defined")));
    };
    let keys: Vec<usize> = args.iter().map(|x| ax_dyn_key(&x.0)).collect();
    let r = ax_dyn_r(f(ax_dyn_args(args, kw)));
    if AX_PYB_BACK.with(|b| !b.borrow().is_empty()) {
        ax_pyb_backs(&keys);
    }
    r
}

fn ax_dyn_bi(name: &str) -> Option<fn(brevi_dyn::Args) -> brevi_dyn::R> {
    Some(match name {
        "len" => brevi_dyn::bi_len,
        "repr" => brevi_dyn::bi_repr,
        "str" => brevi_dyn::bi_str,
        "int" => brevi_dyn::bi_int,
        "float" => brevi_dyn::bi_float,
        "bool" => brevi_dyn::bi_bool,
        "list" => brevi_dyn::bi_list,
        "tuple" => brevi_dyn::bi_tuple,
        "set" => brevi_dyn::bi_set,
        "frozenset" => brevi_dyn::bi_frozenset,
        "dict" => brevi_dyn::bi_dict,
        "range" => brevi_dyn::bi_range,
        "abs" => brevi_dyn::bi_abs,
        "min" => brevi_dyn::bi_min,
        "max" => brevi_dyn::bi_max,
        "sum" => brevi_dyn::bi_sum,
        "sorted" => brevi_dyn::bi_sorted,
        "reversed" => brevi_dyn::bi_reversed,
        "enumerate" => brevi_dyn::bi_enumerate,
        "zip" => brevi_dyn::bi_zip,
        "map" => brevi_dyn::bi_map,
        "filter" => brevi_dyn::bi_filter,
        "any" => brevi_dyn::bi_any,
        "all" => brevi_dyn::bi_all,
        "iter" => brevi_dyn::bi_iter,
        "next" => brevi_dyn::bi_next,
        "isinstance" => brevi_dyn::bi_isinstance,
        "type" => brevi_dyn::bi_type,
        "hasattr" => brevi_dyn::bi_hasattr,
        "getattr" => brevi_dyn::bi_getattr,
        "setattr" => brevi_dyn::bi_setattr,
        "callable" => brevi_dyn::bi_callable,
        "ord" => brevi_dyn::bi_ord,
        "chr" => brevi_dyn::bi_chr,
        "round" => brevi_dyn::bi_round,
        "divmod" => brevi_dyn::bi_divmod,
        "pow" => brevi_dyn::bi_pow,
        "hash" => brevi_dyn::bi_hash,
        "id" => brevi_dyn::bi_id,
        "format" => brevi_dyn::bi_format,
        "print" => brevi_dyn::bi_print,
        "bytes" => brevi_dyn::bi_bytes,
        "hex" => brevi_dyn::bi_hex,
        "oct" => brevi_dyn::bi_oct,
        "bin" => brevi_dyn::bi_bin,
        _ => return None,
    })
}

// o[k]
pub fn ax_pyb_idx(o: AxPy, k: AxPy) -> R<AxPy> {
    ax_dyn_r(brevi_dyn::getitem(&o.0, &k.0))
}
// o[k] = v
pub fn ax_pyb_setidx(o: AxPy, k: AxPy, v: AxPy) -> R<()> {
    let r = brevi_dyn::setitem(&o.0, &k.0, v.0);
    brevi_dyn::out_flush();
    r.map_err(ax_dyn_err)
}
// del o[k]
pub fn ax_pyb_delidx(o: AxPy, k: AxPy) -> R<()> {
    brevi_dyn::delitem(&o.0, &k.0).map_err(ax_dyn_err)
}
// a:b:c
pub fn ax_pyb_slice(a: AxPy, b: AxPy, c: AxPy) -> R<AxPy> {
    Ok(AxPy(brevi_dyn::V::Slice(std::rc::Rc::new((a.0, b.0, c.0)))))
}

// a OP b, with Python's meaning.
pub fn ax_pyb_op(op: impl AsRef<str>, a: AxPy, b: AxPy) -> R<AxPy> {
    ax_dyn_init();
    let op: &str = op.as_ref();
    let r = match op {
        "+" => brevi_dyn::add(&a.0, &b.0),
        "-" => brevi_dyn::sub(&a.0, &b.0),
        "*" | "/" | "//" | "%" | "**" | "@" | "<<" | ">>" | "&" | "|" | "^" => brevi_dyn::binop(&a.0, &b.0, ax_dyn_sym(op)),
        "<" | "<=" | ">" | ">=" | "==" | "!=" => brevi_dyn::cmp_v(&a.0, &b.0, ax_dyn_sym(op)),
        "is" => Ok(brevi_dyn::V::Bool(brevi_dyn::is(&a.0, &b.0))),
        "is not" => Ok(brevi_dyn::V::Bool(!brevi_dyn::is(&a.0, &b.0))),
        "in" => brevi_dyn::contains(&b.0, &a.0).map(brevi_dyn::V::Bool),
        "not in" => brevi_dyn::contains(&b.0, &a.0).map(|x| brevi_dyn::V::Bool(!x)),
        "+=" | "-=" | "*=" | "/=" | "//=" | "%=" | "|=" | "&=" | "^=" => brevi_dyn::iop(&a.0, &b.0, ax_dyn_sym(&op[..op.len() - 1])),
        other => return Err(AxErr(format!("operator {other} is not supported on library values"))),
    };
    ax_dyn_r(r)
}

// -a, +a, ~a
pub fn ax_pyb_unary(op: impl AsRef<str>, a: AxPy) -> R<AxPy> {
    ax_dyn_r(match op.as_ref() {
        "-" => brevi_dyn::neg(&a.0),
        "+" => brevi_dyn::pos(&a.0),
        _ => brevi_dyn::invert(&a.0),
    })
}

// format(o, spec): an f-string's `{o:spec}`.
pub fn ax_pyb_format(o: AxPy, spec: impl AsRef<str>) -> R<String> {
    brevi_dyn::format_spec(&o.0, spec.as_ref()).map_err(ax_dyn_err)
}

pub fn ax_pyb_repr(o: AxPy) -> R<String> {
    brevi_dyn::repr(&o.0).map_err(ax_dyn_err)
}

// int(o), float(o), str(o), bool(o), len(o): back to Brevi values, as Python converts.
pub fn ax_pyb_int(o: AxPy) -> R<i64> {
    match brevi_dyn::to_int(&o.0).map_err(ax_dyn_err)? {
        brevi_dyn::V::Int(n) => Ok(n),
        brevi_dyn::V::Bool(b) => Ok(b as i64),
        _ => Err(AxErr("\u{1}OverflowError,ArithmeticError,Exception,BaseException\u{1}int too large for Brevi (more than 64 bits)".into())),
    }
}
pub fn ax_pyb_float(o: AxPy) -> R<f64> {
    brevi_dyn::to_float(&o.0).map_err(ax_dyn_err)
}
pub fn ax_pyb_str(o: AxPy) -> R<String> {
    brevi_dyn::to_str(&o.0).map_err(ax_dyn_err)
}
pub fn ax_pyb_bool(o: AxPy) -> R<bool> {
    brevi_dyn::truth(&o.0).map_err(ax_dyn_err)
}
pub fn ax_pyb_len(o: AxPy) -> R<i64> {
    match brevi_dyn::bi_len(brevi_dyn::Args::p(vec![o.0])).map_err(ax_dyn_err)? {
        brevi_dyn::V::Int(n) => Ok(n),
        _ => Ok(0),
    }
}

// `a, b = o`: o's items, exactly n of them.
pub fn ax_pyb_items(o: AxPy, n: i64) -> R<Vec<AxPy>> {
    let xs = ax_pyb_list(o)?;
    if xs.len() as i64 != n {
        let m = if xs.len() as i64 > n { format!("too many values to unpack (expected {n})") } else { format!("not enough values to unpack (expected {n}, got {})", xs.len()) };
        return Err(AxErr(format!("\u{1}ValueError,Exception,BaseException\u{1}{m}")));
    }
    Ok(xs)
}

// list(o) as a Brevi list of its items.
pub fn ax_pyb_list(o: AxPy) -> R<Vec<AxPy>> {
    let mut out = Vec::new();
    for x in ax_pyb_iter(o)? {
        out.push(x?);
    }
    Ok(out)
}

pub struct AxPyIter(brevi_dyn::It);
impl Iterator for AxPyIter {
    type Item = R<AxPy>;
    fn next(&mut self) -> Option<R<AxPy>> {
        let r = self.0.next();
        brevi_dyn::out_flush();
        match r {
            Ok(Some(v)) => Some(Ok(AxPy(v))),
            Ok(None) => None,
            Err(e) => Some(Err(ax_dyn_err(e))),
        }
    }
}
pub fn ax_pyb_iter(o: AxPy) -> R<AxPyIter> {
    ax_dyn_init();
    brevi_dyn::iter(&o.0).map(AxPyIter).map_err(ax_dyn_err)
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
        brevi_dyn::cmp_op(&self.0, &o.0, "==").unwrap_or(false)
    }
}

// Brevi values passed to the library.
pub trait AxToPy {
    fn ax_to_py(&self) -> AxPy;
}
pub fn ax_pyb<T: AxToPy>(x: T) -> AxPy {
    x.ax_to_py()
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
        AxPy(brevi_dyn::V::Int(*self))
    }
}
impl AxToPy for f64 {
    fn ax_to_py(&self) -> AxPy {
        if *self == f64::MIN && ax_py_on() {
            return ax_pyb_none();
        }
        AxPy(brevi_dyn::V::Float(*self))
    }
}
impl AxToPy for bool {
    fn ax_to_py(&self) -> AxPy {
        AxPy(brevi_dyn::V::Bool(*self))
    }
}
impl AxToPy for String {
    fn ax_to_py(&self) -> AxPy {
        if self == "\u{0}None" {
            return ax_pyb_none();
        }
        AxPy(brevi_dyn::s(self))
    }
}
impl AxToPy for str {
    fn ax_to_py(&self) -> AxPy {
        AxPy(brevi_dyn::s(self))
    }
}
impl AxToPy for () {
    fn ax_to_py(&self) -> AxPy {
        ax_pyb_none()
    }
}
impl<T: AxToPy> AxToPy for Vec<T> {
    fn ax_to_py(&self) -> AxPy {
        AxPy(brevi_dyn::list(self.iter().map(|x| x.ax_to_py().0).collect()))
    }
}
fn ax_dyn_dict(pairs: Vec<(brevi_dyn::V, brevi_dyn::V)>) -> AxPy {
    let mut d = brevi_dyn::Dict::new();
    for (k, v) in pairs {
        if let Err(e) = d.set(k, v) {
            panic!("{}", ax_exc_msg(&ax_dyn_err(e).0));
        }
    }
    AxPy(brevi_dyn::V::Dict(std::rc::Rc::new(std::cell::RefCell::new(d))))
}
impl<K: AxToPy + Hash + Eq + Ord + Clone, V: AxToPy> AxToPy for AxM<K, V> {
    fn ax_to_py(&self) -> AxPy {
        ax_dyn_dict(self.iter().map(|(k, v)| (k.ax_to_py().0, v.ax_to_py().0)).collect())
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
            Json::Tup { v } => AxPy(brevi_dyn::tup(v.iter().map(|x| x.ax_to_py().0).collect())),
            Json::Obj { v } => v.ax_to_py(),
        }
    }
}
impl AxToPy for Dec {
    fn ax_to_py(&self) -> AxPy {
        ax_dyn_init();
        let d = ax_pyb_call(AxPy(brevi_dyn::V::Module(&brevi_dyn::ModName("decimal"))), "Decimal", vec![self.show().ax_to_py()], vec![]);
        d.unwrap_or_else(|e| panic!("{}", ax_exc_msg(&e.0)))
    }
}
macro_rules! ax_pyb_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: AxToPy),+> AxToPy for ($($t,)+) {
            fn ax_to_py(&self) -> AxPy {
                AxPy(brevi_dyn::tup(vec![$(self.$i.ax_to_py().0),+]))
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
        let t = |op: &str| brevi_dyn::cmp_op(&self.0, &o.0, op).unwrap_or(false);
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
// A Python dict made as a Brevi record, given to the library: a dict again.
pub fn ax_pyb_dict(v: Vec<(String, AxPy)>) -> AxPy {
    ax_dyn_dict(v.into_iter().map(|(k, x)| (brevi_dyn::s(&k), x.0)).collect())
}

// ------------------------------------------------------------------------------------------- write-back
// Brevi values the library gives back: what it changed in a list or dict passed to it, and items of
// `a, b = value`.
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
        match &p.0 {
            brevi_dyn::V::None => Ok(i64::MIN),
            brevi_dyn::V::Int(n) => Ok(*n),
            brevi_dyn::V::Bool(b) => Ok(*b as i64),
            brevi_dyn::V::Big(_) => Err(AxErr("int too large for Brevi (more than 64 bits)".into())),
            x => match brevi_dyn::call_method(x, "__index__", brevi_dyn::Args::p(vec![])) {
                Ok(brevi_dyn::V::Int(n)) => Ok(n),
                _ => Err(AxErr(format!("\u{1}TypeError,Exception,BaseException\u{1}'{}' object cannot be interpreted as an integer", brevi_dyn::type_name(x)))),
            },
        }
    }
}
impl AxFromPy for f64 {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        brevi_dyn::to_float(&p.0).map_err(ax_dyn_err)
    }
}
impl AxFromPy for bool {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        ax_pyb_bool(p.clone())
    }
}
impl AxFromPy for String {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        match &p.0 {
            brevi_dyn::V::Str(s) => Ok(s.text().to_string()),
            x => Err(AxErr(format!("\u{1}TypeError,Exception,BaseException\u{1}expected str, got {}", brevi_dyn::type_name(x)))),
        }
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
        let items = brevi_dyn::dict_items_of(&p.0).map_err(ax_dyn_err)?;
        let mut m = AxM::new();
        for (k, v) in items {
            m.insert(K::ax_from_py(&AxPy(k))?, V::ax_from_py(&AxPy(v))?);
        }
        Ok(m)
    }
}
impl AxFromPy for Json {
    fn ax_from_py(p: &AxPy) -> R<Self> {
        ax_dyn_init();
        let s = ax_pyb_call(AxPy(brevi_dyn::V::Module(&brevi_dyn::ModName("json"))), "dumps", vec![p.clone()], vec![])?;
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

// Whether a value of this type can be changed by the library (a list or dict): it is written back.
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
// Which list or dict an argument is (its allocation), to find its write-back after the call.
fn ax_dyn_key(v: &brevi_dyn::V) -> usize {
    match v {
        brevi_dyn::V::List(r) => std::rc::Rc::as_ptr(r) as *const u8 as usize,
        brevi_dyn::V::Dict(r) => std::rc::Rc::as_ptr(r) as *const u8 as usize,
        _ => 0,
    }
}
thread_local! {
    static AX_PYB_BACK: std::cell::RefCell<Vec<(usize, Box<dyn FnOnce()>)>> = std::cell::RefCell::new(vec![]);
}
// `f(xs)` with xs a Brevi list or dict: the library gets a copy, and after the call xs takes what it made of it.
pub fn ax_pyb_argm<T: AxToPy + AxPyBack + 'static>(x: &mut T) -> AxPy {
    let p = x.ax_to_py();
    if x.ax_backs() {
        let ptr: *mut T = x;
        let obj = p.clone();
        let key = ax_dyn_key(&p.0);
        AX_PYB_BACK.with(|b| b.borrow_mut().push((key, Box::new(move || unsafe { (*ptr).ax_back(&obj) }))));
    }
    p
}
// After a call: the write-backs of its own arguments.
fn ax_pyb_backs(args: &[usize]) {
    let mine: Vec<Box<dyn FnOnce()>> = AX_PYB_BACK.with(|b| {
        let mut b = b.borrow_mut();
        let mut out = vec![];
        let mut k = 0;
        while k < b.len() {
            if b[k].0 != 0 && args.contains(&b[k].0) {
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
// A Brevi lambda given to a library: an engine function.
fn ax_pyb_func(f: Box<dyn Fn(Vec<AxPy>) -> AxPy>) -> AxPy {
    AxPy(brevi_dyn::builtin("<lambda>", move |a: brevi_dyn::Args| {
        let argv: Vec<AxPy> = a.p.into_iter().map(AxPy).collect();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(argv))) {
            Ok(v) => Ok(v.0),
            Err(e) => {
                let m = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "a Brevi function failed".into());
                Err(ax_dyn_raise(&AxErr(m)))
            }
        }
    }))
}
pub fn ax_pyb_fn0<T: AxToPy, F: Fn() -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |_a: Vec<AxPy>| f().ax_to_py()))
}
pub fn ax_pyb_fn1<T: AxToPy, F: Fn(AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        f(a.next().unwrap_or_default()).ax_to_py()
    }))
}
pub fn ax_pyb_fn2<T: AxToPy, F: Fn(AxPy, AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        let x = a.next().unwrap_or_default();
        f(x, a.next().unwrap_or_default()).ax_to_py()
    }))
}
pub fn ax_pyb_fn3<T: AxToPy, F: Fn(AxPy, AxPy, AxPy) -> T + 'static>(f: F) -> AxPy {
    ax_pyb_func(Box::new(move |a: Vec<AxPy>| {
        let mut a = a.into_iter();
        let x = a.next().unwrap_or_default();
        let y = a.next().unwrap_or_default();
        f(x, y, a.next().unwrap_or_default()).ax_to_py()
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
        matches!(self.0, brevi_dyn::V::None)
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
    // Python's own sum: its floats are compensated, which adding the items one by one would lose.
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
        brevi_dyn::cmp_op(&self.0, &self.0, "!=").unwrap_or(false)
    }
}
impl Ord for AxPy {
    fn cmp(&self, o: &AxPy) -> std::cmp::Ordering {
        self.partial_cmp(o).unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl std::hash::Hash for AxPy {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        match brevi_dyn::py_hash(&self.0) {
            Ok(v) => h.write_i64(v),
            Err(e) => panic!("{}", ax_exc_msg(&ax_dyn_err(e).0)),
        }
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
