// ---------- inspect.Signature ----------
// Signature.from_callable and inspect.signature for the program's own defs and lambdas, the functions that wrap
// them (__wrapped__, as functools.wraps, lru_cache and singledispatch leave it), methods bound to them, the
// __init__ a dataclass writes and dataclasses.dataclass itself. Each def records its parameters when it is made
// (with_sig, while the program uses from_callable or signature; see SIG_DEFS in typing.rs), and its annotations
// are kept (ANN_DEFS). The result renders exactly as CPython 3.13's Signature.__str__ and __repr__, and reads
// its parameters (a mappingproxy over an OrderedDict of inspect.Parameter objects, the same one each time) and
// return_annotation. A Parameter has its name, default, annotation and kind (an inspect._ParameterKind, an
// IntEnum), and renders as CPython's. Everything else about a Signature or a Parameter (other attributes, ==,
// hash, constructing one, replace) and every other kind of callable is refused, never guessed.

// A def's parameters: (name, default or Unbound) for the positional ones (the first npo positional-only),
// then *args, the keyword-only ones and **kwargs.
pub struct SigRec {
    pos: Vec<(&'static str, V)>,
    npo: usize,
    star: Option<&'static str>,
    kw: Vec<(&'static str, V)>,
    kwstar: Option<&'static str>,
}

impl Ext for SigRec {
    fn tname(&self) -> &'static str {
        "brevi.sigrec"
    }
    fn repr(&self) -> String {
        "<brevi.sigrec>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn with_sig(f: V, pos: &[&'static str], npo: usize, star: Option<&'static str>, kw: &[&'static str], kwstar: Option<&'static str>, pd: Vec<V>, kd: Vec<V>) -> V {
    if let V::Func(ff) = &f {
        let rec = SigRec {
            pos: pos.iter().copied().zip(pd).collect(),
            npo,
            star,
            kw: kw.iter().copied().zip(kd).collect(),
            kwstar,
        };
        ff.attrs.borrow_mut().push(("__bv_sig__", V::Ext(ExtRc::new(rec))));
    }
    f
}

// kinds, as inspect._ParameterKind
const P_POSONLY: u8 = 0;
const P_POS: u8 = 1;
const P_STAR: u8 = 2;
const P_KWONLY: u8 = 3;
const P_KWSTAR: u8 = 4;

#[derive(Clone)]
struct Param {
    name: String,
    kind: u8,
    default: Option<V>,
    ann: Option<V>,
}

pub struct SigV {
    params: Vec<Param>,
    ret: Option<V>,
    // sig.parameters, made on the first read and the same object after
    proxy: RefCell<Option<V>>,
}
fn sigv(params: Vec<Param>, ret: Option<V>) -> SigV {
    SigV { params, ret, proxy: RefCell::new(None) }
}

// An inspect.Parameter of a Signature this build made.
pub struct ParamV(Param);

thread_local! {
    // inspect._empty, inspect._ParameterKind with its five members, inspect.Parameter
    static SIG_EXTRA: RefCell<Option<(Rc<Class>, Vec<V>, Rc<Class>)>> = const { RefCell::new(None) };
}
const KINDS: [(&str, &str); 5] = [
    ("POSITIONAL_ONLY", "positional-only"),
    ("POSITIONAL_OR_KEYWORD", "positional or keyword"),
    ("VAR_POSITIONAL", "variadic positional"),
    ("KEYWORD_ONLY", "keyword-only"),
    ("VAR_KEYWORD", "variadic keyword"),
];
fn sig_extra() -> (Rc<Class>, Vec<V>, Rc<Class>) {
    if let Some(x) = SIG_EXTRA.with(|c| c.borrow().clone()) {
        return x;
    }
    let empty = Class::new("_empty", "inspect", vec![bclass("object")], BT::Other);
    empty.set("__doc__", s("Marker object for Signature.empty and Parameter.empty."));
    // class _ParameterKind(enum.IntEnum), its values 0..4 with a description each; str() and format() are the
    // member's name
    let kc = Class::new("_ParameterKind", "inspect", vec![enum_class("IntEnum").expect("IntEnum")], BT::Other);
    for (i, (n, _)) in KINDS.iter().enumerate() {
        kc.set(n, V::Int(i as i64));
    }
    kc.set("__str__", builtin("__str__", |a: Args| match a.p.first() {
        Some(me) => getattr(me, "_name_"),
        None => type_err("descriptor '__str__' of '_ParameterKind' object needs an argument".into()),
    }));
    kc.set("__format__", builtin("__format__", |a: Args| match &a.p[..] {
        [me, spec] => Ok(st(format_spec(&getattr(me, "_name_")?, &to_str(spec)?)?)),
        _ => type_err("__format__() takes exactly one argument".into()),
    }));
    let _ = enum_setup(&kc);
    let mut kinds = vec![];
    for (n, d) in KINDS {
        let m = kc.lookup(n).expect("_ParameterKind member");
        if let V::Obj(o) = &m {
            o.set("description", s(d));
        }
        kinds.push(m);
    }
    let pc = Class::new("Parameter", "inspect", vec![bclass("object")], BT::Other);
    pc.set("empty", V::Class(empty.clone()));
    for ((n, _), k) in KINDS.iter().zip(kinds.iter()) {
        pc.set(n, k.clone());
    }
    for n in [
        "__init__", "__new__", "__call__", "__eq__", "__hash__", "__reduce__", "__setstate__", "__slots__", "__doc__", "_name",
        "_kind", "_default", "_annotation", "name", "default", "annotation", "kind", "replace",
    ] {
        pc.set(n, V::Ext(ExtRc::new(SigRefuse("Parameter", n))));
    }
    let x = (empty, kinds, pc);
    SIG_EXTRA.with(|c| *c.borrow_mut() = Some(x.clone()));
    x
}
pub fn param_class() -> Rc<Class> {
    sig_extra().2
}
fn sig_empty() -> V {
    V::Class(sig_extra().0)
}

impl Ext for ParamV {
    fn tname(&self) -> &'static str {
        "Parameter"
    }
    fn repr(&self) -> String {
        match param_text(&self.0) {
            Ok(t) => format!("<Parameter \"{t}\">"),
            Err(_) => "<Parameter>".into(),
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let p = &self.0;
        Some(match n {
            "name" => Ok(s(&p.name)),
            "default" => Ok(p.default.clone().unwrap_or_else(sig_empty)),
            "annotation" => Ok(p.ann.clone().unwrap_or_else(sig_empty)),
            "kind" => Ok(sig_extra().1[p.kind as usize].clone()),
            "empty" => Ok(sig_empty()),
            _ => match KINDS.iter().position(|(k, _)| *k == n) {
                Some(i) => Ok(sig_extra().1[i].clone()),
                None => Err(unsupported(&format!("inspect.Parameter.{n}"))),
            },
        })
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting inspect.Parameter.{n}"))))
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(Err(unsupported(&format!("inspect.Parameter.{n}"))))
    }
    fn eq_rich(&self, _me: &V, _o: &V, _right: bool) -> Option<Result<bool, E>> {
        Some(Err(unsupported("comparing inspect.Parameter objects")))
    }
    fn rich(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<Result<bool, E>> {
        Some(Err(unsupported("comparing inspect.Parameter objects")))
    }
    fn cmp_value(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<R> {
        Some(Err(unsupported("comparing inspect.Parameter objects")))
    }
    fn binop(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<R> {
        Some(Err(unsupported("operators on inspect.Parameter objects")))
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if !spec.is_empty() {
            return Some(type_err("unsupported format string passed to Parameter.__format__".into()));
        }
        Some(param_text(&self.0).map(st))
    }
}

thread_local! {
    static SIG_CLASS: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
}

// What Signature (the class) and its instances refuse: every name of CPython's class but from_callable.
struct SigRefuse(&'static str, &'static str);
impl Ext for SigRefuse {
    fn tname(&self) -> &'static str {
        "brevi.sigrefuse"
    }
    fn repr(&self) -> String {
        "<brevi.sigrefuse>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn descriptor(&self, _me: &V, _obj: &V, _cls: &V) -> Option<R> {
        Some(Err(unsupported(&format!("inspect.{}.{}", self.0, self.1))))
    }
    fn method(&self, _me: &V, _n: &str, _a: &mut Args) -> Option<R> {
        Some(Err(unsupported(&format!("inspect.{}.{}", self.0, self.1))))
    }
}

pub fn sig_class() -> Rc<Class> {
    if let Some(c) = SIG_CLASS.with(|c| c.borrow().clone()) {
        return c;
    }
    let c = Class::new("Signature", "inspect", vec![bclass("object")], BT::Other);
    c.set("from_callable", builtin(sym("from_callable"), sig_from_callable));
    for n in [
        "__init__", "__new__", "__call__", "__eq__", "__hash__", "__reduce__", "__setstate__", "__slots__", "__doc__", "_bind",
        "_bound_arguments_cls", "_hash_basis", "_parameter_cls", "_parameters", "_return_annotation", "bind", "bind_partial",
        "format", "parameters", "replace", "return_annotation",
    ] {
        c.set(n, V::Ext(ExtRc::new(SigRefuse("Signature", n))));
    }
    c.set("empty", sig_empty());
    SIG_CLASS.with(|s| *s.borrow_mut() = Some(c.clone()));
    c
}

impl Ext for SigV {
    fn tname(&self) -> &'static str {
        "Signature"
    }
    fn repr(&self) -> String {
        match sig_text(self) {
            Ok(t) => format!("<Signature {t}>"),
            Err(_) => "<Signature>".into(),
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "parameters" => self.parameters(),
            "return_annotation" => Ok(self.ret.clone().unwrap_or_else(sig_empty)),
            "empty" => Ok(sig_empty()),
            _ => Err(unsupported(&format!("inspect.Signature.{n}"))),
        })
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting inspect.Signature.{n}"))))
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        Some(Err(unsupported(&format!("inspect.Signature.{n}"))))
    }
    fn eq_rich(&self, _me: &V, _o: &V, _right: bool) -> Option<Result<bool, E>> {
        Some(Err(unsupported("comparing inspect.Signature objects")))
    }
    fn rich(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<Result<bool, E>> {
        Some(Err(unsupported("comparing inspect.Signature objects")))
    }
    fn cmp_value(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<R> {
        Some(Err(unsupported("comparing inspect.Signature objects")))
    }
    fn binop(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<R> {
        Some(Err(unsupported("operators on inspect.Signature objects")))
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if !spec.is_empty() {
            return Some(type_err("unsupported format string passed to Signature.__format__".into()));
        }
        Some(self.text().map(st))
    }
}

impl SigV {
    fn text(&self) -> Result<String, E> {
        sig_text(self)
    }
    // types.MappingProxyType(OrderedDict((p.name, p) for p in params))
    fn parameters(&self) -> R {
        if let Some(v) = self.proxy.borrow().clone() {
            return Ok(v);
        }
        let mut d = Dict::new();
        d.kind = 3;
        for p in &self.params {
            d.set(s(&p.name), V::Ext(ExtRc::new(ParamV(p.clone()))))?;
        }
        let v = V::View(Rc::new((V::Dict(Rc::new(RefCell::new(d))), 3)));
        *self.proxy.borrow_mut() = Some(v.clone());
        Ok(v)
    }
}

// str(sig) and repr(sig), which can raise (a default's __repr__), for to_str and repr
pub fn sig_str(v: &V, as_repr: bool) -> Option<Result<String, E>> {
    if let V::Ext(x) = v {
        if let Some(s) = x.any().downcast_ref::<SigV>() {
            return Some(s.text().map(|t| if as_repr { format!("<Signature {t}>") } else { t }));
        }
        if let Some(p) = x.any().downcast_ref::<ParamV>() {
            return Some(param_text(&p.0).map(|t| if as_repr { format!("<Parameter \"{t}\">") } else { t }));
        }
    }
    None
}

// inspect.formatannotation(annotation) with no base module
fn format_ann(a: &V) -> Result<String, E> {
    match a {
        V::Str(_) | V::None | V::Int(_) | V::Bool(_) | V::Float(_) => repr(a),
        V::Class(_) => {
            let m = getattr(a, "__module__")?;
            if matches!(&m, V::Str(x) if x.text() == "typing") {
                return Err(unsupported("Signature.from_callable with a typing annotation"));
            }
            let q = to_str(&getattr(a, "__qualname__")?)?;
            match &m {
                V::Str(x) if x.text() == "builtins" => Ok(q),
                _ => Ok(format!("{}.{}", to_str(&m)?, q)),
            }
        }
        V::Obj(_) => {
            if let Ok(V::Str(x)) = getattr(a, "__module__") {
                if x.text() == "typing" {
                    return Err(unsupported("Signature.from_callable with a typing annotation"));
                }
            }
            repr(a)
        }
        _ => Err(unsupported(&format!("Signature.from_callable with a '{}' annotation", tn(a)))),
    }
}

fn param_text(p: &Param) -> Result<String, E> {
    let mut f = p.name.clone();
    if let Some(a) = &p.ann {
        f = format!("{f}: {}", format_ann(a)?);
    }
    if let Some(d) = &p.default {
        f = if p.ann.is_some() { format!("{f} = {}", repr(d)?) } else { format!("{f}={}", repr(d)?) };
    }
    Ok(match p.kind {
        P_STAR => format!("*{f}"),
        P_KWSTAR => format!("**{f}"),
        _ => f,
    })
}

// Signature.__str__ of CPython 3.13
fn sig_text(s: &SigV) -> Result<String, E> {
    let mut out: Vec<String> = vec![];
    let mut posonly_sep = false;
    let mut kwonly_sep = true;
    for p in &s.params {
        let t = param_text(p)?;
        if p.kind == P_POSONLY {
            posonly_sep = true;
        } else if posonly_sep {
            out.push("/".into());
            posonly_sep = false;
        }
        if p.kind == P_STAR {
            kwonly_sep = false;
        } else if p.kind == P_KWONLY && kwonly_sep {
            out.push("*".into());
            kwonly_sep = false;
        }
        out.push(t);
    }
    if posonly_sep {
        out.push("/".into());
    }
    let mut r = format!("({})", out.join(", "));
    if let Some(a) = &s.ret {
        r = format!("{r} -> {}", format_ann(a)?);
    }
    Ok(r)
}

fn is_attr_err(e: &E) -> bool {
    is_exc(&e.0, "AttributeError")
}

// getattr(o, n), None on AttributeError (hasattr's rule)
fn attr_opt(o: &V, n: &str) -> Result<Option<V>, E> {
    match getattr(o, n) {
        Ok(v) => Ok(Some(v)),
        Err(e) if is_attr_err(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

fn sig_of(obj: &V, depth: usize) -> Result<SigV, E> {
    if depth > 100 {
        return Err(unsupported("Signature.from_callable through this many wrappers"));
    }
    if !truth(&bi_callable(Args::p(vec![obj.clone()]))?)? {
        return type_err(format!("{} is not a callable object", repr(obj)?));
    }
    // a bound method: its function's signature without the first parameter
    if let V::Bound(b) = obj {
        let mut s = sig_of(&b.1, depth + 1)?;
        match s.params.first().map(|p| p.kind) {
            None | Some(P_KWSTAR) | Some(P_KWONLY) => return val_err("invalid method signature".into()),
            Some(P_STAR) => {}
            Some(_) => {
                s.params.remove(0);
            }
        }
        return Ok(s);
    }
    // inspect.unwrap: follow __wrapped__ until a function with __signature__ or a bound method
    let mut f = obj.clone();
    let mut seen = 0;
    loop {
        if matches!(f, V::Bound(_)) || attr_opt(&f, "__signature__")?.is_some() {
            break;
        }
        match attr_opt(&f, "__wrapped__")? {
            Some(w) => {
                f = w;
                seen += 1;
                if seen > 100 {
                    return Err(unsupported("Signature.from_callable through this many wrappers"));
                }
            }
            None => break,
        }
    }
    if matches!(f, V::Bound(_)) {
        return sig_of(&f, depth + 1);
    }
    if let Some(sv) = attr_opt(&f, "__signature__")? {
        if !matches!(sv, V::None) {
            return Err(unsupported("Signature.from_callable of an object with __signature__"));
        }
    }
    if let V::Func(ff) = &f {
        if ff.attrs.borrow().iter().any(|(k, _)| *k == "__bv_partial__") {
            return Err(unsupported("Signature.from_callable of a functools.partial"));
        }
        let rec = ff.attrs.borrow().iter().find(|(k, _)| *k == "__bv_sig__").map(|(_, v)| v.clone());
        if let Some(V::Ext(r)) = rec {
            if let Some(r) = r.any().downcast_ref::<SigRec>() {
                let ann = match ff.attrs.borrow().iter().find(|(k, _)| *k == "__annotations__").map(|(_, v)| v.clone()) {
                    Some(d @ V::Dict(_)) => d,
                    _ => return Err(unsupported("Signature.from_callable of a def whose annotations this build did not keep")),
                };
                let ann_of = |n: &str| -> Result<Option<V>, E> {
                    match &ann {
                        V::Dict(d) => Dict::get_cell(d, &s(n)),
                        _ => Ok(None),
                    }
                };
                let dflt = |v: &V| if matches!(v, V::Unbound) { None } else { Some(v.clone()) };
                let mut params = vec![];
                for (i, (n, d)) in r.pos.iter().enumerate() {
                    params.push(Param { name: n.to_string(), kind: if i < r.npo { P_POSONLY } else { P_POS }, default: dflt(d), ann: ann_of(n)? });
                }
                if let Some(n) = r.star {
                    params.push(Param { name: n.to_string(), kind: P_STAR, default: None, ann: ann_of(n)? });
                }
                for (n, d) in r.kw.iter() {
                    params.push(Param { name: n.to_string(), kind: P_KWONLY, default: dflt(d), ann: ann_of(n)? });
                }
                if let Some(n) = r.kwstar {
                    params.push(Param { name: n.to_string(), kind: P_KWSTAR, default: None, ann: ann_of(n)? });
                }
                return Ok(sigv(params, ann_of("return")?));
            }
        }
        // builtins whose CPython text signature Brevi knows: lru_cache's cache_info() and cache_clear() ($self, /,
        // bound), functools.cmp_to_key ($module, /, mycmp) and the key type it returns (obj)
        if ff.kind.get() == 3 {
            if matches!(&*ff.qualname.get(), "_lru_cache_wrapper.cache_info" | "_lru_cache_wrapper.cache_clear") {
                return Ok(sigv(vec![], None));
            }
            let one = |n: &str| Ok(sigv(vec![Param { name: n.to_string(), kind: P_POS, default: None, ann: None }], None));
            // dataclasses.dataclass(cls=None, /, *, init=True, repr=True, ...), a def in CPython
            if &*ff.name.get() == "dataclass" && matches!(ff.attrs.borrow().iter().find(|(k, _)| *k == "__bv_mod__"), Some((_, V::Str(m))) if m.text() == "dataclasses") {
                let mut params = vec![Param { name: "cls".into(), kind: P_POSONLY, default: Some(V::None), ann: None }];
                for (n, d) in [("init", true), ("repr", true), ("eq", true), ("order", false), ("unsafe_hash", false), ("frozen", false), ("match_args", true), ("kw_only", false), ("slots", false), ("weakref_slot", false)] {
                    params.push(Param { name: n.into(), kind: P_KWONLY, default: Some(V::Bool(d)), ann: None });
                }
                return Ok(sigv(params, None));
            }
            if &*ff.name.get() == "cmp_to_key" {
                return one("mycmp");
            }
            if let Some((_, V::Str(t))) = ff.attrs.borrow().iter().find(|(k, _)| *k == "__bv_textsig__") {
                return one(&t.text());
            }
        }
    }
    Err(unsupported(&format!("Signature.from_callable of a '{}' object", tn(&f))))
}

pub fn sig_from_callable(mut a: Args) -> R {
    // the defaults only: follow_wrapped=True, globals=None, locals=None, eval_str=False
    for (k, v) in std::mem::take(&mut a.k) {
        let ok = match &*k {
            "follow_wrapped" => matches!(v, V::Bool(true)),
            "globals" | "locals" => matches!(v, V::None),
            "eval_str" => matches!(v, V::Bool(false)),
            _ => return type_err(format!("Signature.from_callable() got an unexpected keyword argument '{k}'")),
        };
        if !ok {
            return Err(unsupported(&format!("Signature.from_callable with {k}=")));
        }
    }
    // called on the class: Signature.from_callable(obj)
    a.want("from_callable", 1, 1)?;
    let s = sig_of(&a.p[0], 0)?;
    Ok(V::Ext(ExtRc::new(s)))
}

// inspect.signature(obj, *, follow_wrapped=True, globals=None, locals=None, eval_str=False), which is
// Signature.from_callable(obj, ...): the defaults only
pub fn sig_signature(a: Args) -> R {
    let sig = Sig { name: sym("signature"), params: &["obj", "follow_wrapped", "globals", "locals", "eval_str"], npos: 1, npo: 0, star: false, kwstar: false };
    let v = bind(&sig, &[V::Unbound, V::Bool(true), V::None, V::None, V::Bool(false)], a)?;
    if !truth(&v[1])? {
        return Err(unsupported("inspect.signature with follow_wrapped="));
    }
    for (i, k) in [(2, "globals"), (3, "locals")] {
        if !matches!(v[i], V::None) {
            return Err(unsupported(&format!("inspect.signature with {k}=")));
        }
    }
    if truth(&v[4])? {
        return Err(unsupported("inspect.signature with eval_str="));
    }
    let s = sig_of(&v[0], 0)?;
    Ok(V::Ext(ExtRc::new(s)))
}

// A def's __defaults__ (kw false) or __kwdefaults__ (kw true) from its recorded signature, as CPython's: a tuple
// of the positional parameters' defaults or a dict of the keyword-only ones', None when there are none. None
// (no answer) when the def recorded no signature (SIG_DEFS off).
pub fn sig_defaults(f: &Func, kw: bool) -> Option<V> {
    let rec = f.attrs.borrow().iter().find(|(k, _)| *k == "__bv_sig__").map(|(_, v)| v.clone())?;
    let V::Ext(e) = &rec else { return None };
    let r = e.any().downcast_ref::<SigRec>()?;
    if kw {
        let mut d = Dict::new();
        for (n, v) in r.kw.iter().filter(|(_, v)| !matches!(v, V::Unbound)) {
            d.set(s(n), v.clone()).ok()?;
        }
        Some(if d.len() == 0 { V::None } else { V::Dict(Rc::new(RefCell::new(d))) })
    } else {
        let ds: Vec<V> = r.pos.iter().filter(|(_, v)| !matches!(v, V::Unbound)).map(|(_, v)| v.clone()).collect();
        Some(if ds.is_empty() { V::None } else { tup(ds) })
    }
}
