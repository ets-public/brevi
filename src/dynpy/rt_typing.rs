// ---------- typing, typing_extensions, collections.abc ----------
// The compiler (dynpy/typing.rs) lets typing names be values only where CPython's behaviour is plain: base
// classes, decorators, aliases, cast, TypeVar, NewType and the like. A typing form is a class of module
// "typing" that subscripts (getitem) and `|`s to itself; make_class turns NamedTuple and TypedDict bases into
// namedtuple and dict classes and drops Generic and Protocol.
thread_local! {
    static TYPING_FORMS: RefCell<Vec<Rc<Class>>> = RefCell::new(vec![]);
    // CPython's isinstance / issubclass against the collections.abc class of the same name, for a library class
    // or value (array.array, say). The bridge sets this when CPython starts; None keeps the runtime's own check.
    pub static ABC_BRIDGE: Cell<Option<fn(&str, &V, bool) -> Option<Result<bool, E>>>> = const { Cell::new(None) };
}
pub fn typing_form(n: &str) -> Rc<Class> {
    TYPING_FORMS.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(c) = t.iter().find(|c| c.name == n) {
            return c.clone();
        }
        let c = Class::new(sym(n), "typing", vec![bclass("object")], BT::Other);
        t.push(c.clone());
        c
    })
}
pub fn typing_attr(n: &str) -> V {
    match n {
        "TYPE_CHECKING" => V::Bool(false),
        "NoDefault" => no_default(),
        "cast" | "overload" | "final" | "override" | "no_type_check" | "runtime_checkable" | "assert_never" | "assert_type" | "reveal_type" | "NewType" | "TypeVar" | "ParamSpec" | "TypeVarTuple" => {
            let n: Sym = sym(n);
            builtin(n, move |a: Args| typing_call(n, a))
        }
        _ => V::Class(typing_form(n)),
    }
}
pub fn typing_call(n: &str, mut a: Args) -> R {
    let first = a.p.first().cloned().unwrap_or(V::None);
    match n {
        "cast" => Ok(a.p.get(1).cloned().unwrap_or(V::None)),
        "assert_type" | "overload" | "final" | "override" | "no_type_check" => Ok(first),
        "runtime_checkable" => runtime_checkable(first),
        "reveal_type" => {
            err_write(&format!("Runtime type is {}\n", repr(&s(&tn(&first)))?));
            Ok(first)
        }
        "assert_never" => {
            let mut v = repr(&first)?;
            if v.chars().count() > 100 {
                v = v.chars().take(100).collect::<String>() + "...";
            }
            Err(exc("AssertionError", format!("Expected code to be unreachable, but got: {v}")))
        }
        // `UserId = NewType("UserId", int)`: UserId(x) is x
        "NewType" => Ok(builtin("NewType", |a: Args| Ok(a.p.first().cloned().unwrap_or(V::None)))),
        "TypeVar" | "ParamSpec" | "TypeVarTuple" => {
            a.k.clear();
            let c = Class::new(sym(&to_str(&first).unwrap_or_default()), "typing", vec![bclass("object")], BT::Other);
            if n != "TypeVar" {
                c.made.set(5);
            }
            Ok(V::Class(c))
        }
        // NamedTuple("Point", [("x", int), ("y", int)])
        "NamedTuple" => {
            let fields: Vec<V> = to_vec(a.p.get(1).unwrap_or(&V::None))?.iter().map(|f| to_vec(f).map(|p| p.first().cloned().unwrap_or(V::None))).collect::<Result<_, _>>()?;
            namedtuple(Args::p(vec![first, list(fields)]))
        }
        _ => type_err(format!("Cannot instantiate typing.{n}")),
    }
}

// A class whose bases hold NamedTuple or TypedDict (make_class calls it first; None: an ordinary class,
// with typing's Generic and Protocol taken out of its bases). `qualname` is the class statement's dotted
// __qualname__; `module` is where the class is defined.
pub fn typing_class(name: Sym, qualname: Sym, module: Sym, bases: &mut Vec<V>, attrs: &[(Sym, V)], fields: &[Sym]) -> Option<R> {
    // the compiler's dataclasses marks ("ClassVar:x"): every annotated name is a NamedTuple or TypedDict key
    let plain: Vec<Sym> = fields.iter().map(|f| f.split_once(':').map_or(*f, |(_, n)| sym(n))).collect();
    let fields = &plain[..];
    let (mut nt, mut td) = (false, false);
    bases.retain(|b| match b {
        V::Class(k) if k.module == "typing" => {
            nt |= k.name == "NamedTuple";
            td |= k.name == "TypedDict";
            false
        }
        V::Class(k) if k.lookup("__bv_typeddict__").is_some() => {
            td = true;
            false
        }
        // typing.List[int] and the like as a base: dropped, as the typing classes they came from were
        V::Ext(e) if e.any().downcast_ref::<TAlias>().is_some() || e.any().downcast_ref::<TUnion>().is_some() || e.any().downcast_ref::<TForm>().is_some() => false,
        _ => true,
    });
    if td {
        let c = Class::with_qualname(name, qualname, module, vec![bclass("dict")], BT::Other);
        c.set("__bv_typeddict__", V::Bool(true));
        return Some(Ok(V::Class(c)));
    }
    if !nt {
        return None;
    }
    Some((|| {
        let mut defaults = vec![];
        let mut default_names: Vec<Sym> = vec![];
        for f in fields {
            match attrs.iter().find(|(n, _)| n == f) {
                Some((_, v)) => {
                    defaults.push(v.clone());
                    default_names.push(f);
                }
                None if !default_names.is_empty() => {
                    return type_err(format!("Non-default namedtuple field {f} cannot follow default field{} {}", if default_names.len() > 1 { "s" } else { "" }, default_names.join(", ")));
                }
                None => {}
            }
        }
        let mut a = Args::p(vec![s(name), list(fields.iter().map(|f| s(f)).collect())]);
        a.k.push(("defaults", list(defaults)));
        // as typing.NamedTuple does: the caller's module, and the class statement's __qualname__
        a.k.push(("module", s(module)));
        let c = namedtuple(a)?;
        if let V::Class(k) = &c {
            k.qualname.set(qualname);
            for (n, v) in attrs {
                if matches!(*n, "_asdict" | "__slots__" | "__new__" | "_replace" | "__init__" | "_field_defaults" | "_fields" | "__getnewargs__" | "_source" | "_make") {
                    return Err(exc("AttributeError", format!("Cannot overwrite NamedTuple attribute {n}")));
                }
                if !fields.contains(n) && !matches!(*n, "__annotations__" | "__module__" | "__name__") && !(*n == "__doc__" && matches!(v, V::None)) {
                    k.set(n, v.clone());
                }
            }
        }
        Ok(c)
    })())
}
// A TypedDict class called: a plain dict, as CPython makes it.
pub fn typed_dict_call(c: &Rc<Class>, a: Args) -> Option<R> {
    if c.lookup("__bv_typeddict__").is_some() {
        return Some(bi_dict(a));
    }
    None
}

// A PEP 695 type parameter (`def f[T]`, `class C[T: int]`, `type A[*Ts, **P = ...]`): CPython's typing.TypeVar
// (kind 0), TypeVarTuple (1) or ParamSpec (2), with infer_variance=True. The bound, constraints and default are
// the annotation scope's lambdas, evaluated on first read and kept once one succeeds (an error, NameError say,
// is raised again on the next read, as CPython's lazy evaluation does). Identity is the object's: equal and
// hashed only as itself.
pub struct TypeVarV {
    pub name: Sym,
    pub kind: u8,
    bound: Lazy,
    cons: Lazy,
    dflt: Lazy,
}
struct Lazy(RefCell<Option<(V, bool)>>);
impl Lazy {
    fn new(v: V) -> Lazy {
        Lazy(RefCell::new(if matches!(v, V::None) { None } else { Some((v, false)) }))
    }
    fn present(&self) -> bool {
        self.0.borrow().is_some()
    }
    fn get(&self) -> Result<Option<V>, E> {
        let (f, done) = match &*self.0.borrow() {
            None => return Ok(None),
            Some((v, done)) => (v.clone(), *done),
        };
        if done {
            return Ok(Some(f));
        }
        let v = call(&f, Args::p(vec![]))?;
        *self.0.borrow_mut() = Some((v.clone(), true));
        Ok(Some(v))
    }
}
pub fn pep695_typevar(n: &str) -> V {
    V::Ext(ExtRc::new(TypeVarV { name: sym(n), kind: 0, bound: Lazy::new(V::None), cons: Lazy::new(V::None), dflt: Lazy::new(V::None) }))
}
// __brevi_typevar(name, kind, bound, constraints, default): the parser's binding of one type parameter
pub fn bi___brevi_typevar(a: Args) -> R {
    a.want("__brevi_typevar", 5, 5)?;
    let kind = match &a.p[1] {
        V::Int(k) => *k as u8,
        _ => 0,
    };
    let name = sym(&to_str(&a.p[0])?);
    Ok(V::Ext(ExtRc::new(TypeVarV { name, kind, bound: Lazy::new(a.p[2].clone()), cons: Lazy::new(a.p[3].clone()), dflt: Lazy::new(a.p[4].clone()) })))
}
// __brevi_tparams(f, (T, ...)): the def made in the annotation scope, with its __type_params__
pub fn bi___brevi_tparams(a: Args) -> R {
    a.want("__brevi_tparams", 2, 2)?;
    Ok(with_tparams(a.p[0].clone(), a.p[1].clone()))
}
// typing.NoDefault, a type parameter's __default__ when it has none
pub struct NoDefaultV;
impl Ext for NoDefaultV {
    fn tname(&self) -> &'static str {
        "typing.NoDefaultType"
    }
    fn repr(&self) -> String {
        "typing.NoDefault".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some(0x4e6f44656661)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if e.any().downcast_ref::<NoDefaultV>().is_some()))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        if n == "__class__" {
            return None;
        }
        Some(Err(unsupported(&format!("typing.NoDefault.{n}"))))
    }
}
thread_local! {
    static NO_DEFAULT: V = V::Ext(ExtRc::new(NoDefaultV));
}
pub fn no_default() -> V {
    NO_DEFAULT.with(|v| v.clone())
}
impl TypeVarV {
    fn addr(&self) -> usize {
        self as *const TypeVarV as usize
    }
    fn kind_name(&self) -> &'static str {
        ["TypeVar", "TypeVarTuple", "ParamSpec"][self.kind.min(2) as usize]
    }
}
// isinstance(x, typing.TypeVar) (or TypeVarTuple, ParamSpec): a PEP 695 parameter knows its kind; an old-style
// TypeVar("T") is a class of module "typing" (is_typevar), and ParamSpec("P") and TypeVarTuple("Ts") are classes
// typing_call marks made = 5 alike, so which of the two such a class is cannot be told: refused.
pub fn typevar_isinstance(x: &V, kind: &str) -> Result<bool, E> {
    let k = match kind {
        "TypeVar" => 0,
        "TypeVarTuple" => 1,
        _ => 2,
    };
    // type(T) of a PEP 695 T is a class of module "typing" too (rt_lib.rs ext_class_mod), and no TypeVar
    let old_style = |c: &Rc<Class>| c.module == "typing" && !is_form(c) && !EXT_ITER_CLASSES.with(|m| m.borrow().values().any(|x| Rc::ptr_eq(x, c)));
    match x {
        V::Ext(e) if e.any().downcast_ref::<TypeVarV>().is_some() => Ok(is_pep695_kind(x, k)),
        V::Class(c) if old_style(c) && c.made.get() == 5 => {
            if k == 0 { Ok(false) } else { Err(unsupported(&format!("isinstance of an old-style ParamSpec or TypeVarTuple with typing.{kind}"))) }
        }
        V::Class(c) if old_style(c) => Ok(k == 0),
        _ => Ok(false),
    }
}
// x is a PEP 695 type parameter of this kind (isinstance(x, typing.TypeVar) and the like)
pub fn is_pep695_kind(x: &V, kind: u8) -> bool {
    matches!(x, V::Ext(e) if e.any().downcast_ref::<TypeVarV>().is_some_and(|t| t.kind == kind))
}
impl Ext for TypeVarV {
    fn tname(&self) -> &'static str {
        ["typing.TypeVar", "typing.TypeVarTuple", "typing.ParamSpec"][self.kind.min(2) as usize]
    }
    // infer_variance: no ~, + or - in front, as CPython prints it (and a TypeVarTuple's is its name too)
    fn repr(&self) -> String {
        self.name.to_string()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some((self.addr() >> 4) as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if e.any().downcast_ref::<TypeVarV>().is_some_and(|t| t.addr() == self.addr())))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let tuple = self.kind == 1;
        Some(match n {
            "__name__" => Ok(s(self.name)),
            "__module__" => Ok(s("typing")),
            "__bound__" if !tuple => self.bound.get().map(|b| b.unwrap_or(V::None)),
            "__constraints__" if self.kind == 0 => self.cons.get().map(|c| c.unwrap_or_else(|| tup(vec![]))),
            "__default__" => self.dflt.get().map(|d| d.unwrap_or_else(no_default)),
            "__covariant__" | "__contravariant__" if !tuple => Ok(V::Bool(false)),
            "__infer_variance__" if !tuple => Ok(V::Bool(true)),
            "has_default" => {
                let h = self.dflt.present();
                Ok(builtin(sym("has_default"), move |a: Args| {
                    a.want("has_default", 0, 0)?;
                    Ok(V::Bool(h))
                }))
            }
            "__class__" => return None,
            // the rest of CPython's TypeVar (substitution hooks, a ParamSpec's args and kwargs, a TypeVarTuple's
            // unpacking) and object's other dunders are not made here
            _ if n.starts_with("__") || (self.kind == 2 && matches!(n, "args" | "kwargs")) => Err(unsupported(&format!("{}.{n}", self.kind_name()))),
            _ => Err(exc("AttributeError", format!("'typing.{}' object has no attribute '{n}'", self.kind_name()))),
        })
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting {}.{n}", self.kind_name()))))
    }
    // T.has_default() and the like: the attribute, called
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        if n == "__call__" {
            return Some(type_err(format!("'typing.{}' object is not callable", self.kind_name())));
        }
        Some(self.attr(me, n)?.and_then(|f| call(&f, Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) })))
    }
    // `T | int`, `int | T`: typing.Union[...] (TypeVar.__or__ and __ror__; a TypeVarTuple has neither)
    fn binop(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        if op != "|" || self.kind == 1 {
            return None;
        }
        Some(if right { typing_union(&[o.clone(), me.clone()]) } else { typing_union(&[me.clone(), o.clone()]) })
    }
}

// `type X[T] = value` (PEP 695): typing.TypeAliasType. __value__ is the annotation scope's lambda, evaluated on
// the first read and kept once it succeeds.
pub struct TypeAliasV {
    name: Sym,
    module: V,
    params: V,
    value: Lazy,
}
pub fn bi___brevi_typealias(a: Args) -> R {
    a.want("__brevi_typealias", 4, 4)?;
    Ok(V::Ext(ExtRc::new(TypeAliasV { name: sym(&to_str(&a.p[0])?), params: a.p[1].clone(), value: Lazy::new(a.p[2].clone()), module: a.p[3].clone() })))
}
impl TypeAliasV {
    fn addr(&self) -> usize {
        self as *const TypeAliasV as usize
    }
}
impl Ext for TypeAliasV {
    fn tname(&self) -> &'static str {
        "typing.TypeAliasType"
    }
    fn repr(&self) -> String {
        self.name.to_string()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some((self.addr() >> 4) as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if e.any().downcast_ref::<TypeAliasV>().is_some_and(|t| t.addr() == self.addr())))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "__name__" => Ok(s(self.name)),
            "__module__" => Ok(self.module.clone()),
            "__type_params__" | "__parameters__" => Ok(self.params.clone()),
            "__value__" => self.value.get().map(|v| v.unwrap_or(V::None)),
            "__class__" => return None,
            // subscripting, `|`, pickling and object's other dunders are not made here
            _ if n.starts_with("__") => Err(unsupported(&format!("TypeAliasType.{n}"))),
            _ => Err(exc("AttributeError", format!("'typing.TypeAliasType' object has no attribute '{n}'"))),
        })
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(match n {
            "__name__" | "__module__" | "__type_params__" | "__parameters__" | "__value__" => exc("AttributeError", format!("readonly attribute")),
            _ => unsupported(&format!("setting TypeAliasType.{n}")),
        }))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        if n == "__call__" {
            return Some(type_err("'typing.TypeAliasType' object is not callable".into()));
        }
        // A.__value__() and the like: the attribute, called
        Some(self.attr(me, n).unwrap_or_else(|| Err(unsupported(&format!("TypeAliasType.{n}")))).and_then(|f| call(&f, Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) })))
    }
    fn binop(&self, _me: &V, op: &str, _o: &V, _right: bool) -> Option<R> {
        Some(Err(unsupported(&format!("a TypeAliasType with {op}"))))
    }
}
// __brevi_generic(T, ...): the typing.Generic[T, ...] a PEP 695 class gets after its bases
pub fn bi___brevi_generic(a: Args) -> R {
    Ok(V::Ext(ExtRc::new(TForm { name: sym("Generic"), params: a.p.len(), origin: None, pep695: Some(a.p.clone()) })))
}

// ---------- typing values: types.UnionType (`int | str`), typing.List[int] and friends, typing.Union ----------
// What CPython prints and compares for them is made here (repr, ==, hash, __args__, isinstance); anything else
// they can do stops with "not supported". An argument this runtime has no CPython repr for (a typing form such as
// Any, a string forward reference, an old-style TypeVar) makes the value opaque: it still stands in an alias, but
// reading it stops.
fn is_none_class(v: &V) -> bool {
    matches!(v, V::Class(k) if k.module == "builtins" && k.name == "NoneType")
}
fn none_class() -> V {
    V::Class(tclass(12, "NoneType"))
}
// an argument whose CPython repr this runtime makes
fn typing_arg_known(v: &V) -> bool {
    match v {
        V::Class(k) => k.module != "typing",
        V::Ellipsis => true,
        V::Tuple(t) => t.iter().all(typing_arg_known),
        V::Ext(e) => {
            let a = e.any();
            a.downcast_ref::<TAlias>().is_some_and(|x| !x.opaque)
                || a.downcast_ref::<TUnion>().is_some_and(|x| !x.opaque)
                || a.downcast_ref::<UnionT>().is_some()
                || a.downcast_ref::<TypeVarV>().is_some()
                || e.tname() == "types.GenericAlias"
        }
        _ => false,
    }
}
fn class_type_repr(k: &Class) -> String {
    if k.module == "builtins" {
        k.qualname.get().to_string()
    } else {
        format!("{}.{}", k.module, k.qualname.get())
    }
}
// typing._type_repr
fn typing_type_repr(v: &V) -> String {
    match v {
        V::Class(k) => class_type_repr(k),
        V::Ellipsis => "...".into(),
        V::Tuple(t) => format!("[{}]", t.iter().map(typing_type_repr).collect::<Vec<_>>().join(", ")),
        _ => repr(v).unwrap_or_default(),
    }
}
fn union_flat(v: &V, out: &mut Vec<V>) -> Result<(), E> {
    let push = |x: V, out: &mut Vec<V>| -> Result<(), E> {
        for y in out.iter() {
            if eq(y, &x)? {
                return Ok(());
            }
        }
        out.push(x);
        Ok(())
    };
    if let V::Ext(e) = v {
        if let Some(u) = e.any().downcast_ref::<UnionT>() {
            for a in &u.args {
                push(a.clone(), out)?;
            }
            return Ok(());
        }
        if let Some(u) = e.any().downcast_ref::<TUnion>() {
            for a in &u.args {
                push(a.clone(), out)?;
            }
            return Ok(());
        }
    }
    push(if matches!(v, V::None) { none_class() } else { v.clone() }, out)
}
fn args_set_hash(args: &[V]) -> Result<i64, E> {
    py_hash(&V::FSet(Rc::new(PSet::from_iter(args.to_vec())?)))
}
fn same_arg_set(a: &[V], b: &[V]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.iter().any(|y| eq(x, y).unwrap_or(false)))
}
fn union_args_of(v: &V) -> Option<Vec<V>> {
    let V::Ext(e) = v else { return None };
    if let Some(u) = e.any().downcast_ref::<UnionT>() {
        return Some(u.args.clone());
    }
    if let Some(u) = e.any().downcast_ref::<TUnion>() {
        return if u.opaque { None } else { Some(u.args.clone()) };
    }
    None
}
// isinstance / issubclass against a union's arguments (classes only, as CPython requires)
fn union_check(args: &[V], x: &V, sub: bool, generic_msg: &str) -> Result<bool, E> {
    for a in args {
        if !matches!(a, V::Class(_)) {
            return type_err(generic_msg.into());
        }
    }
    for a in args {
        let hit = if sub {
            match (x, a) {
                (V::Class(s), V::Class(k)) => s.is_sub(k),
                _ => return type_err("issubclass() arg 1 must be a class".into()),
            }
        } else {
            isinstance(x, a)?
        };
        if hit {
            return Ok(true);
        }
    }
    Ok(false)
}

// types.UnionType: `int | str`, `list[int] | None`
pub struct UnionT {
    pub args: Vec<V>,
}
impl Ext for UnionT {
    fn tname(&self) -> &'static str {
        "types.UnionType"
    }
    fn repr(&self) -> String {
        self.args
            .iter()
            .map(|a| match a {
                _ if is_none_class(a) => "None".to_string(),
                V::Class(k) => class_type_repr(k),
                _ => repr(a).unwrap_or_default(),
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        args_set_hash(&self.args).ok()
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(union_args_of(o).is_some_and(|b| same_arg_set(&self.args, &b)))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "__args__" => Ok(tup(self.args.clone())),
            "__parameters__" => Ok(tup(vec![])),
            "__class__" => return None,
            _ if n.starts_with("__") => Err(unsupported(&format!("types.UnionType.{n}"))),
            _ => Err(exc("AttributeError", format!("'types.UnionType' object has no attribute '{n}'"))),
        })
    }
    fn instancecheck(&self, x: &V) -> Option<Result<bool, E>> {
        Some(union_check(&self.args, x, false, "isinstance() argument 2 cannot be a parameterized generic"))
    }
    fn subclasscheck(&self, sub: &V) -> Option<Result<bool, E>> {
        Some(union_check(&self.args, sub, true, "issubclass() argument 2 cannot be a parameterized generic"))
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(exc("AttributeError", format!("'types.UnionType' object has no attribute '{n}' and no __dict__ for setting new attributes"))))
    }
}

// typing.List[int], typing.Dict[str, int], typing.Tuple[int, ...], typing.Type[C] (typing._GenericAlias)
pub struct TAlias {
    pub name: Sym,
    pub origin: Rc<Class>,
    pub args: Vec<V>,
    pub opaque: bool,
}
impl TAlias {
    fn check(&self) -> Result<(), E> {
        if self.opaque {
            return Err(unsupported(&format!("typing.{}[...] of a typing form, a string or a TypeVar", self.name)));
        }
        Ok(())
    }
}
impl Ext for TAlias {
    fn tname(&self) -> &'static str {
        "typing._GenericAlias"
    }
    fn repr(&self) -> String {
        if self.opaque {
            return format!("typing.{}[...]", self.name);
        }
        if self.args.is_empty() {
            return format!("typing.{}[()]", self.name);
        }
        format!("typing.{}[{}]", self.name, self.args.iter().map(typing_type_repr).collect::<Vec<_>>().join(", "))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        if self.opaque {
            return None;
        }
        let o = py_hash(&V::Class(self.origin.clone())).ok()?;
        let a = py_hash(&tup(self.args.clone())).ok()?;
        Some(o ^ a)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        let V::Ext(e) = o else { return Some(false) };
        let Some(b) = e.any().downcast_ref::<TAlias>() else { return Some(false) };
        if self.opaque || b.opaque {
            return Some(std::ptr::eq(self, b));
        }
        Some(Rc::ptr_eq(&self.origin, &b.origin) && self.args.len() == b.args.len() && self.args.iter().zip(&b.args).all(|(x, y)| eq(x, y).unwrap_or(false)))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        if let Err(e) = self.check() {
            return Some(Err(e));
        }
        Some(match n {
            "__origin__" => Ok(V::Class(self.origin.clone())),
            "__args__" => Ok(tup(self.args.clone())),
            "__parameters__" => Ok(tup(vec![])),
            "_name" | "__name__" | "__qualname__" => Ok(s(self.name)),
            "__module__" => Ok(s("typing")),
            "__class__" => return None,
            _ if n.starts_with("__") => Err(unsupported(&format!("typing.{}[...].{n}", self.name))),
            // typing forwards other names to the origin class
            _ => getattr(&V::Class(self.origin.clone()), n),
        })
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        if n == "__call__" {
            return Some(type_err(format!("Type {} cannot be instantiated; use {}() instead", self.name, self.origin.name)));
        }
        None
    }
    fn instancecheck(&self, _x: &V) -> Option<Result<bool, E>> {
        Some(type_err("Subscripted generics cannot be used with class and instance checks".into()))
    }
    fn subclasscheck(&self, _x: &V) -> Option<Result<bool, E>> {
        Some(type_err("Subscripted generics cannot be used with class and instance checks".into()))
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting typing.{}[...].{n}", self.name))))
    }
}

// typing.Union[...] and typing.Optional[...] (typing._UnionGenericAlias)
pub struct TUnion {
    pub args: Vec<V>,
    pub opaque: bool,
}
impl Ext for TUnion {
    fn tname(&self) -> &'static str {
        "typing._UnionGenericAlias"
    }
    fn repr(&self) -> String {
        if self.opaque {
            return "typing.Union[...]".into();
        }
        if self.args.len() == 2 && self.args.iter().any(is_none_class) {
            let other = self.args.iter().find(|a| !is_none_class(a)).unwrap();
            return format!("typing.Optional[{}]", typing_type_repr(other));
        }
        format!("typing.Union[{}]", self.args.iter().map(typing_type_repr).collect::<Vec<_>>().join(", "))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        if self.opaque {
            return None;
        }
        args_set_hash(&self.args).ok()
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if self.opaque {
            return Some(matches!(o, V::Ext(e) if e.any().downcast_ref::<TUnion>().is_some_and(|b| std::ptr::eq(self, b))));
        }
        Some(union_args_of(o).is_some_and(|b| same_arg_set(&self.args, &b)))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        if self.opaque {
            return Some(Err(unsupported("typing.Union[...] of a typing form, a string or a TypeVar")));
        }
        Some(match n {
            "__args__" => Ok(tup(self.args.clone())),
            "__parameters__" => Ok(tup(vec![])),
            "__class__" => return None,
            _ if n.starts_with("__") => Err(unsupported(&format!("typing.Union.{n}"))),
            _ => Err(unsupported(&format!("typing.Union[...].{n}"))),
        })
    }
    fn method(&self, _me: &V, n: &str, _a: &mut Args) -> Option<R> {
        if n == "__call__" {
            return Some(type_err("Cannot instantiate typing.Union".into()));
        }
        None
    }
    fn instancecheck(&self, x: &V) -> Option<Result<bool, E>> {
        if self.opaque {
            return Some(Err(unsupported("isinstance() with a typing.Union of typing forms")));
        }
        Some(union_check(&self.args, x, false, "Subscripted generics cannot be used with class and instance checks"))
    }
    fn subclasscheck(&self, x: &V) -> Option<Result<bool, E>> {
        if self.opaque {
            return Some(Err(unsupported("issubclass() with a typing.Union of typing forms")));
        }
        Some(union_check(&self.args, x, true, "Subscripted generics cannot be used with class and instance checks"))
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting typing.Union[...].{n}"))))
    }
}
fn typing_valueish(v: &V) -> bool {
    matches!(v, V::Ext(e) if e.any().downcast_ref::<TAlias>().is_some() || e.any().downcast_ref::<TUnion>().is_some() || e.any().downcast_ref::<TypeVarV>().is_some())
}
// typing.Union[args] (typing's own flattening, de-duplication and single-argument collapse)
pub fn typing_union(args: &[V]) -> R {
    let mut out = vec![];
    for a in args {
        union_flat(a, &mut out)?;
    }
    if out.len() == 1 {
        return Ok(out.pop().unwrap());
    }
    let opaque = !out.iter().all(typing_arg_known);
    Ok(V::Ext(ExtRc::new(TUnion { args: out, opaque })))
}
// `a | b` for type-like operands: types.UnionType, or typing.Union when a side is a typing value. None: not a
// union operation (the usual TypeError or __or__ follows).
pub fn union_or(a: &V, b: &V) -> Option<R> {
    let unionable = |v: &V| match v {
        V::None => true,
        V::Class(k) => k.module != "typing",
        V::Ext(e) => e.any().downcast_ref::<UnionT>().is_some() || e.tname() == "types.GenericAlias",
        _ => false,
    };
    if (typing_valueish(a) && (unionable(b) || typing_valueish(b))) || (typing_valueish(b) && unionable(a)) {
        // a TypeVarTuple has no `|` (the usual TypeError follows)
        if [a, b].iter().any(|x| is_pep695_kind(x, 1)) {
            return None;
        }
        return Some(typing_union(&[a.clone(), b.clone()]));
    }
    if !(unionable(a) && unionable(b)) || (matches!(a, V::None) && matches!(b, V::None)) {
        return None;
    }
    let mut out = vec![];
    if let Err(e) = union_flat(a, &mut out).and_then(|_| union_flat(b, &mut out)) {
        return Some(Err(e));
    }
    if out.len() == 1 {
        return Some(Ok(out.pop().unwrap()));
    }
    Some(Ok(V::Ext(ExtRc::new(UnionT { args: out }))))
}
// typing.List[int] and the like, typing.Union[...], typing.Optional[X]: None for any other typing form
pub fn typing_subscript(k: &Rc<Class>, i: &V) -> Option<R> {
    let args = match i {
        V::Tuple(t) => t.to_vec(),
        other => vec![other.clone()],
    };
    let origin = match k.name {
        "List" => bclass("list"),
        "Dict" => bclass("dict"),
        "Set" => bclass("set"),
        "FrozenSet" => bclass("frozenset"),
        "Tuple" => bclass("tuple"),
        "Type" => bclass("type"),
        "Union" => return Some(typing_union(&args)),
        "Optional" => {
            if args.len() != 1 {
                return Some(type_err(format!("typing.Optional requires a single type. Got {}.", repr(i).unwrap_or_default())));
            }
            return Some(typing_union(&[args[0].clone(), V::None]));
        }
        _ => return None,
    };
    let args: Vec<V> = args.into_iter().map(|a| if matches!(a, V::None) { none_class() } else { a }).collect();
    let opaque = !args.iter().all(typing_arg_known) || (k.name != "Tuple" && args.iter().any(|a| matches!(a, V::Ellipsis)));
    Some(Ok(V::Ext(ExtRc::new(TAlias { name: k.name, origin, args, opaque }))))
}

// ---------- subscripts of typing forms and classes, as CPython 3.13 checks them ----------
// An annotation is evaluated when its statement or def runs, so `x: Dict[str]` raises where CPython raises.
// What is made here is CPython's value where this runtime has one (typing_subscript above) and an opaque
// TForm otherwise; what CPython would refuse raises its TypeError, and a check this runtime cannot make
// exactly stops with "not supported" rather than passing.

thread_local! {
    // the modules that are Brevi's copies of CPython modules (emit.rs marks each as its init runs)
    static COPY_MODS: RefCell<Vec<Sym>> = const { RefCell::new(vec![]) };
}
// CPython 3.13's classes (module, qualname) whose own dict holds __class_getitem__, every stdlib module imported
// (typing.Generic aside, which typing_getitem answers)
const OWN_CLASS_GETITEM: &[(&str, &str)] = &[
    ("_asyncio", "Future"), ("_asyncio", "Task"), ("_contextvars", "ContextVar"), ("_contextvars", "Token"), ("_ctypes", "Array"),
    ("_queue", "SimpleQueue"), ("_weakrefset", "WeakSet"), ("array", "array"), ("asyncio.queues", "Queue"),
    ("builtins", "BaseExceptionGroup"), ("builtins", "async_generator"), ("builtins", "coroutine"), ("builtins", "dict"),
    ("builtins", "enumerate"), ("builtins", "frozenset"), ("builtins", "generator"), ("builtins", "list"), ("builtins", "mappingproxy"),
    ("builtins", "set"), ("builtins", "tuple"), ("collections", "defaultdict"), ("collections", "deque"),
    ("collections.abc", "AsyncIterable"), ("collections.abc", "Awaitable"), ("collections.abc", "Callable"), ("collections.abc", "Container"),
    ("collections.abc", "Iterable"), ("collections.abc", "MappingView"), ("contextlib", "AbstractAsyncContextManager"),
    ("contextlib", "AbstractContextManager"), ("csv", "DictReader"), ("csv", "DictWriter"), ("ctypes", "LibraryLoader"),
    ("dataclasses", "Field"), ("dataclasses", "InitVar"), ("difflib", "SequenceMatcher"), ("filecmp", "dircmp"),
    ("fileinput", "FileInput"), ("functools", "cached_property"), ("functools", "partial"), ("functools", "partialmethod"),
    ("graphlib", "TopologicalSorter"), ("itertools", "chain"), ("logging", "LoggerAdapter"), ("logging", "StreamHandler"),
    ("mailbox", "Mailbox"), ("mailbox", "_ProxyFile"), ("os", "PathLike"), ("posix", "DirEntry"), ("queue", "Queue"),
    ("queue", "_PySimpleQueue"), ("re", "Match"), ("re", "Pattern"), ("subprocess", "CompletedProcess"), ("subprocess", "Popen"),
    ("tempfile", "SpooledTemporaryFile"), ("tempfile", "TemporaryDirectory"), ("weakref", "ReferenceType"),
];
pub fn copy_module(m: &str) {
    COPY_MODS.with(|c| c.borrow_mut().push(sym(m)));
}
// typing.Literal["a"], typing.Callable[[int], str], typing.Iterator[int] and the like: a typing value whose
// CPython repr and attributes this runtime does not make. It can stand in an annotation, an alias or a union;
// reading it stops.
pub struct TForm {
    pub name: Sym,
    // Generic[T, ...] and Protocol[T, ...]: how many type parameters (a class made on one takes as many)
    pub params: usize,
    // G[int] of a class the program made on Generic: G, which calling the alias makes an instance of
    pub origin: Option<Rc<Class>>,
    // the Generic[T, ...] a PEP 695 class statement adds after its bases: its type parameters
    pub pep695: Option<Vec<V>>,
}
impl Ext for TForm {
    fn tname(&self) -> &'static str {
        "typing._GenericAlias"
    }
    fn repr(&self) -> String {
        format!("typing.{}[...]", self.name)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if e.any().downcast_ref::<TForm>().is_some_and(|b| std::ptr::eq(self, b))))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        if n == "__class__" {
            return None;
        }
        Some(Err(unsupported(&format!("typing.{}[...].{n}", self.name))))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        // typing._BaseGenericAlias.__call__: an instance of the class, with __orig_class__ set to the alias where
        // the instance takes it (CPython ignores the Exception setting it raises)
        if let (Some(k), "__call__") = (&self.origin, n) {
            let r = call(&V::Class(k.clone()), Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) });
            return Some(r.and_then(|o| match setattr(&o, sym("__orig_class__"), me.clone()) {
                Err(e) if !swallowed(&e) => Err(e),
                _ => Ok(o),
            }));
        }
        Some(Err(unsupported(&format!("typing.{}[...].{n}", self.name))))
    }
    fn instancecheck(&self, _x: &V) -> Option<Result<bool, E>> {
        Some(Err(unsupported(&format!("isinstance() with typing.{}[...]", self.name))))
    }
    fn subclasscheck(&self, _x: &V) -> Option<Result<bool, E>> {
        Some(Err(unsupported(&format!("issubclass() with typing.{}[...]", self.name))))
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting typing.{}[...].{n}", self.name))))
    }
}
// `except Exception: pass` around setting __orig_class__, but never over a refusal of Brevi's own
fn swallowed(e: &E) -> bool {
    let is = |n: &str| exc_match(&e.0, &V::Class(exc_class(n))).unwrap_or(false);
    is("Exception") && !(is("NotImplementedError") && to_str(&e.0).is_ok_and(|m| m.contains("is not supported in dynamic builds")))
}
fn tform(n: Sym) -> R {
    Ok(V::Ext(ExtRc::new(TForm { name: n, params: 0, origin: None, pep695: None })))
}
// a typing form of the runtime's own (typing_form), not a TypeVar or another class of module "typing"
fn is_form(k: &Rc<Class>) -> bool {
    TYPING_FORMS.with(|t| t.borrow().iter().any(|c| Rc::ptr_eq(c, k)))
}
// old-style TypeVar("T") (a class of module "typing" that is no form, and no ParamSpec or TypeVarTuple, which
// typing_call marks made = 5) or a PEP 695 one
fn is_typevar(v: &V) -> bool {
    match v {
        V::Class(k) => k.module == "typing" && !is_form(k) && k.made.get() != 5,
        V::Ext(e) => e.any().downcast_ref::<TypeVarV>().is_some(),
        _ => false,
    }
}
// generic aliases (typing._SpecialGenericAlias): (parameters, how many of them have defaults); 0 parameters:
// "is not a generic class"
fn alias_params(n: &str) -> Option<(usize, usize)> {
    Some(match n {
        "List" | "Set" | "FrozenSet" | "Type" | "Iterable" | "Iterator" | "Sequence" | "MutableSequence" | "MutableSet" | "AbstractSet"
        | "Collection" | "Container" | "Reversible" | "Awaitable" | "AsyncIterator" | "AsyncIterable" | "KeysView" | "ValuesView"
        | "MappingView" | "Counter" | "Deque" | "Pattern" | "Match" => (1, 0),
        "Dict" | "Mapping" | "MutableMapping" | "ItemsView" | "DefaultDict" | "OrderedDict" | "ChainMap" => (2, 0),
        "Coroutine" => (3, 0),
        "Generator" => (3, 2),
        "AsyncGenerator" | "ContextManager" | "AsyncContextManager" => (2, 1),
        "Hashable" | "Sized" => (0, 0),
        _ => return None,
    })
}
// typing._type_check on one argument: Ok when CPython takes it as it is. A string must be a forward reference
// CPython compiles (only dotted names are known to be); a tuple, a bare special form, ClassVar[...] or Final[...]
// inside another form, an instance (whose == CPython would call) and anything else unknown stop here.
fn type_arg_ok(v: &V, what: &str) -> Result<(), E> {
    let ok = match v {
        V::None | V::Int(_) | V::Big(_) | V::Float(_) | V::Bool(_) | V::Ellipsis | V::Func(_) => true,
        V::Str(t) => {
            let t = t.text();
            !t.is_empty() && t.split('.').all(|p| p.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && p.chars().all(|c| c.is_alphanumeric() || c == '_')) && !matches!(t, "None" | "True" | "False") && !is_py_keyword(t)
        }
        V::Class(k) if k.module != "typing" || !is_form(k) => true,
        V::Class(k) => alias_params(k.name).is_some() || matches!(k.name, "Any" | "Tuple" | "Callable" | "NoReturn" | "Never" | "Self" | "LiteralString" | "TypeAlias" | "Text" | "AnyStr"),
        V::Ext(e) => {
            let a = e.any();
            a.downcast_ref::<TAlias>().is_some() || a.downcast_ref::<TUnion>().is_some() || a.downcast_ref::<UnionT>().is_some() || a.downcast_ref::<TypeVarV>().is_some()
                || e.tname() == "types.GenericAlias"
                || a.downcast_ref::<TForm>().is_some_and(|f| !matches!(f.name, "ClassVar" | "Final"))
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(unsupported(&format!("{what} with an argument Brevi cannot check as CPython's typing does")))
    }
}
fn is_py_keyword(t: &str) -> bool {
    matches!(
        t,
        "and" | "as" | "assert" | "async" | "await" | "break" | "class" | "continue" | "def" | "del" | "elif" | "else" | "except" | "finally" | "for"
            | "from" | "global" | "if" | "import" | "in" | "is" | "lambda" | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" | "try"
            | "while" | "with" | "yield"
    )
}
fn arg_repr100(i: &V) -> Result<String, E> {
    let r = repr(i)?;
    Ok(if r.chars().count() > 100 { r.chars().take(100).collect() } else { r })
}
// form[i] for a typing form of the runtime (`k`)
pub fn typing_getitem(k: &Rc<Class>, i: &V) -> R {
    let n = k.name;
    if !is_form(k) {
        return Err(unsupported("subscripting a TypeVar or another typing object"));
    }
    let args: Vec<V> = match i {
        V::Tuple(t) => t.to_vec(),
        other => vec![other.clone()],
    };
    let what = format!("typing.{n}[...]");
    if let Some((np, nd)) = alias_params(n) {
        for a in &args {
            type_arg_ok(a, &what)?;
        }
        let got = args.len();
        if got > np || got + nd < np {
            if np == 0 {
                return type_err(format!("typing.{n} is not a generic class"));
            }
            let expected = if nd > 0 { format!("at least {}", np - nd) } else { np.to_string() };
            return type_err(format!("Too {} arguments for typing.{n}; actual {got}, expected {expected}", if got > np { "many" } else { "few" }));
        }
        return match typing_subscript(k, i) {
            Some(r) => r,
            None => tform(n),
        };
    }
    match n {
        "Tuple" => {
            let body = if args.len() >= 2 && matches!(args.last(), Some(V::Ellipsis)) { &args[..args.len() - 1] } else { &args[..] };
            for a in body {
                type_arg_ok(a, &what)?;
            }
            typing_subscript(k, i).unwrap_or_else(|| tform(n))
        }
        "Union" => {
            if args.is_empty() {
                return type_err("Cannot take a Union of no types.".into());
            }
            for a in &args {
                type_arg_ok(a, &what)?;
            }
            typing_union(&args)
        }
        "Optional" => {
            if let V::Tuple(_) = i {
                return type_err(format!("typing.Optional requires a single type. Got {}.", arg_repr100(i)?));
            }
            type_arg_ok(i, &what)?;
            typing_union(&[i.clone(), V::None])
        }
        "ClassVar" | "Final" | "TypeGuard" | "TypeIs" | "Unpack" | "Required" | "NotRequired" | "ReadOnly" => {
            if let V::Tuple(_) = i {
                let head = if matches!(n, "Required" | "NotRequired" | "ReadOnly") { format!("{n} accepts only a single type.") } else { format!("typing.{n} accepts only single type.") };
                return type_err(format!("{head} Got {}.", arg_repr100(i)?));
            }
            type_arg_ok(i, &what)?;
            tform(n)
        }
        "Literal" => tform(n),
        "Annotated" => {
            if !matches!(i, V::Tuple(t) if t.len() >= 2) {
                return type_err("Annotated[...] should be used with at least two arguments (a type and an annotation).".into());
            }
            type_arg_ok(&args[0], &what)?;
            tform(n)
        }
        "Callable" => {
            if !matches!(i, V::Tuple(t) if t.len() == 2) {
                return type_err("Callable must be used as Callable[[arg, ...], result].".into());
            }
            match &args[0] {
                V::List(l) => {
                    for a in l.borrow().iter() {
                        type_arg_ok(a, &what)?;
                    }
                }
                V::Ellipsis => {}
                a => type_arg_ok(a, &what)?,
            }
            type_arg_ok(&args[1], &what)?;
            tform(n)
        }
        // Generic[T, ...] and Protocol[T, ...] as bases: make_class drops them
        "Generic" | "Protocol" => {
            if args.is_empty() {
                return type_err(format!("Parameter list to {n}[...] cannot be empty"));
            }
            if args.iter().any(|a| matches!(a, V::Class(c) if c.module != "typing")) {
                return type_err(format!("Parameters to {n}[...] must all be type variables or parameter specification variables."));
            }
            if !args.iter().all(is_typevar) {
                return Err(unsupported(&format!("{n}[...] of anything but TypeVars")));
            }
            for (j, a) in args.iter().enumerate() {
                for b in &args[..j] {
                    if eq(a, b)? {
                        return type_err(format!("Parameters to {n}[...] must all be unique"));
                    }
                }
            }
            Ok(V::Ext(ExtRc::new(TForm { name: n, params: args.len(), origin: None, pep695: None })))
        }
        "Self" | "NoReturn" | "Never" | "LiteralString" | "TypeAlias" => type_err(format!("typing.{n} is not subscriptable")),
        "Any" => type_err("type 'Any' is not subscriptable".into()),
        "Text" => type_err("type 'str' is not subscriptable".into()),
        "AnyStr" => type_err("'typing.TypeVar' object is not subscriptable".into()),
        _ => Err(unsupported(&what)),
    }
}
// cls[i] for a class with no __class_getitem__ of its own or inherited that the runtime found: CPython's
// "type 'X' is not subscriptable" when that is sure (a program class on plain classes whose CPython dicts
// Brevi has); otherwise it stops.
pub fn class_subscript(k: &Rc<Class>, i: &V) -> R {
    match k.made.get() {
        2 => return Err(unsupported("subscripting a class made on typing's Generic or Protocol")),
        // typing's _generic_class_getitem on a class with no type parameters
        3 => return type_err(format!("{} is not a generic class", repr(&V::Class(k.clone()))?)),
        m if m >= 10 => {
            let want = (m - 10) as usize;
            let args: Vec<V> = match i {
                V::Tuple(t) => t.to_vec(),
                other => vec![other.clone()],
            };
            // _type_convert: a string becomes a ForwardRef, which CPython compiles
            for a in &args {
                if let V::Str(_) = a {
                    type_arg_ok(a, "a generic class subscript")?;
                }
            }
            let got = args.len();
            let kr = repr(&V::Class(k.clone()))?;
            if got > want {
                return type_err(format!("Too many arguments for {kr}; actual {got}, expected {want}"));
            }
            if got < want {
                return type_err(format!("Too few arguments for {kr}; actual {got}, expected at least {}", got + 1));
            }
            return Ok(V::Ext(ExtRc::new(TForm { name: k.name, params: 0, origin: Some(k.clone()), pep695: None })));
        }
        _ => {}
    }
    if k.meta.borrow().is_some() {
        return Err(unsupported("subscripting a class with a metaclass"));
    }
    // CPython's PyObject_GetItem makes type[...] itself
    if k.module == "builtins" && k.name == "type" {
        return Ok(class_alias(k, i));
    }
    // collections.abc.Callable[[int], str] (Brevi's copy has no __class_getitem__ for it): _CallableGenericAlias's
    // checks, then an opaque value
    if k.mro.borrow().iter().any(|b| b.module == "collections.abc" && b.name == "Callable" && b.made.get() == 1) {
        if !matches!(i, V::Tuple(t) if t.len() == 2) {
            return type_err("Callable must be used as Callable[[arg, ...], result].".into());
        }
        let V::Tuple(t) = i else { unreachable!() };
        return match &t[0] {
            V::Tuple(_) | V::List(_) | V::Ellipsis => tform(sym("Callable")),
            a @ (V::Class(_) | V::Int(_) | V::None | V::Bool(_) | V::Float(_)) if !matches!(a, V::Class(c) if c.module == "typing") => {
                type_err(format!("Expected a list of types, an ellipsis, ParamSpec, or Concatenate. Got {}", to_str(a)?))
            }
            _ => Err(unsupported("collections.abc.Callable[...] of a ParamSpec, Concatenate or another value")),
        };
    }
    let mut tp_name = k.name;
    for (j, b) in k.mro.borrow().iter().enumerate() {
        if b.made.get() == 1 && !COPY_MODS.with(|m| m.borrow().contains(&b.module)) {
            if b.own("__class_getitem__").is_some() {
                return Err(unsupported("a __class_getitem__ Brevi did not find"));
            }
            continue;
        }
        // a builtin type answers from CPython's own dict; a class of Brevi's runtime or of its copy of a module, from
        // the list of CPython's classes that define __class_getitem__ (a copy's class that lookup did not find it on
        // has lost it)
        let own = match cpy_dict_has(b, "__class_getitem__") {
            Some((own, tn)) => {
                if j == 0 {
                    tp_name = tn;
                }
                own
            }
            None if OWN_CLASS_GETITEM.contains(&(b.module, b.qualname.get())) => {
                if b.made.get() == 1 {
                    return Err(unsupported(&format!("subscripting class {}", k.name)));
                }
                true
            }
            None => {
                if j == 0 && b.made.get() == 0 && b.module != "builtins" {
                    tp_name = sym(&format!("{}.{}", b.module, b.name));
                }
                false
            }
        };
        // types.GenericAlias, as `__class_getitem__ = classmethod(GenericAlias)` makes it for the builtin types
        if own {
            return Ok(class_alias(k, i));
        }
    }
    type_err(format!("type '{tp_name}' is not subscriptable"))
}

// ---------- typing.Protocol classes ----------
// A class with Protocol among its own bases is a protocol (CPython's _is_protocol); make_class drops Protocol from
// its bases, so the runtime keeps here what CPython's _ProtocolMeta keeps on the class: __protocol_attrs__ (worked
// out when the class is made, from the dicts and annotations along its MRO, as _get_protocol_attrs does), whether
// @runtime_checkable marked it, and the members that decorator found not callable. isinstance() against one
// follows _ProtocolMeta.__instancecheck__ (rt_lib.rs isinstance); issubclass() against one is refused.
pub struct ProtoInfo {
    attrs: Vec<Sym>,
    // why the attributes could not be worked out exactly; isinstance() then refuses
    inexact: Option<String>,
    runtime: Cell<bool>,
    noncallable: RefCell<Vec<Sym>>,
    // no class along the MRO defines __init__: Protocol.__init_subclass__ gave the class typing's
    // _no_init_or_replace_init, so calling it is the TypeError
    pub no_init: bool,
}
thread_local! {
    static PROTOS: RefCell<Vec<(Rc<Class>, Rc<ProtoInfo>)>> = const { RefCell::new(vec![]) };
}
// typing's _TYPING_INTERNALS and _SPECIAL_NAMES (CPython 3.13): never protocol members
const PROTO_EXCLUDED: &[&str] = &[
    "__parameters__", "__orig_bases__", "__orig_class__", "_is_protocol", "_is_runtime_protocol", "__protocol_attrs__",
    "__non_callable_proto_members__", "__type_params__", "__abstractmethods__", "__annotations__", "__dict__", "__doc__",
    "__init__", "__module__", "__new__", "__slots__", "__subclasshook__", "__weakref__", "__class_getitem__", "__match_args__",
    "__static_attributes__", "__firstlineno__", "_MutableMapping__marker",
];
pub fn proto_info(c: &Rc<Class>) -> Option<Rc<ProtoInfo>> {
    // only a class made on Protocol (Class.made 3, or 10 + n with type parameters) can be one
    let m = c.made.get();
    if m < 3 || m == 5 {
        return None;
    }
    PROTOS.with(|p| p.borrow().iter().find(|(k, _)| Rc::ptr_eq(k, c)).map(|(_, i)| i.clone()))
}
fn is_protocol_form(b: &V) -> bool {
    match b {
        V::Class(k) => k.module == "typing" && k.name == "Protocol",
        V::Ext(e) => e.any().downcast_ref::<TForm>().is_some_and(|f| f.name == "Protocol" && f.origin.is_none()),
        _ => false,
    }
}
// _ProtocolMeta.__new__: whether a class with these bases is a protocol, and the TypeError for a protocol that
// inherits from a class that is not one
pub fn proto_bases_check(bases: &[V]) -> Result<bool, E> {
    if !bases.iter().any(is_protocol_form) {
        return Ok(false);
    }
    for b in bases {
        let ok = match b {
            _ if is_protocol_form(b) => true,
            V::Class(k) if k.module == "builtins" && k.name == "object" => true,
            V::Class(k) if k.module == "typing" && k.name == "Generic" => true,
            V::Class(k) if proto_info(k).is_some() => true,
            // typing's _PROTO_ALLOWLIST: collections.abc and contextlib ABCs, which Brevi's Protocol does not follow
            V::Class(k) if matches!(&*k.module, "collections.abc" | "contextlib" | "_collections_abc") => {
                return Err(unsupported(&format!("a Protocol class on {}.{}", k.module, k.name)));
            }
            V::Class(_) => false,
            V::Ext(e) => match e.any().downcast_ref::<TForm>() {
                Some(f) if f.name == "Generic" && f.origin.is_none() => true,
                Some(TForm { origin: Some(k), .. }) if proto_info(k).is_some() => true,
                Some(TForm { origin: Some(_), .. }) => false,
                _ => return Err(unsupported("a Protocol class on a subscripted typing form")),
            },
            _ => return Err(unsupported("a Protocol class on a base that is not a class")),
        };
        if !ok {
            let shown = match b {
                V::Ext(e) => match e.any().downcast_ref::<TForm>() {
                    Some(TForm { origin: Some(k), .. }) => repr(&V::Class(k.clone()))?,
                    _ => repr(b)?,
                },
                _ => repr(b)?,
            };
            return type_err(format!("Protocols can only inherit from other protocols, got {shown}"));
        }
    }
    Ok(true)
}
// _get_protocol_attrs: every name in the dicts and annotations along the MRO (object left out), less typing's own
pub fn proto_register(c: &Rc<Class>) {
    let mut attrs: Vec<Sym> = vec![];
    let mut inexact = None;
    for k in c.mro.borrow().iter() {
        if k.module == "builtins" && k.name == "object" {
            continue;
        }
        let why = tinfo(k).and_then(|t| t.inexact.get());
        // a class whose annotations Brevi did not keep lacks only __annotations__, which is no member; the
        // annotated names come from Class.fields
        if let Some(w) = why.filter(|w| *w != "annotations Brevi did not keep") {
            inexact = Some(format!("the members of Protocol class {} ({w})", k.qualname.get()));
            continue;
        }
        let keys: Vec<Sym> = match class_dict_with(k, false) {
            Ok(v) => v.into_iter().map(|(n, _)| n).collect(),
            Err(_) => {
                inexact = Some(format!("the members of Protocol class {}", k.qualname.get()));
                continue;
            }
        };
        let anns: Vec<Sym> = k.fields.borrow().iter().map(|f| match f.rfind(':') {
            Some(i) => sym(&f[i + 1..]),
            None => *f,
        }).collect();
        for n in keys.into_iter().chain(anns) {
            if !n.starts_with("_abc_") && !PROTO_EXCLUDED.contains(&n) && !attrs.contains(&n) {
                attrs.push(n);
            }
        }
    }
    let no_init = !c.lookup("__init__").is_some_and(|i| !is_builtin_init(&i));
    let info = Rc::new(ProtoInfo { attrs, inexact, runtime: Cell::new(false), noncallable: RefCell::new(vec![]), no_init });
    PROTOS.with(|p| p.borrow_mut().push((c.clone(), info)));
}
// @runtime_checkable: a protocol class only; the members whose class attribute is not callable are noted, as
// CPython notes them in __non_callable_proto_members__
fn runtime_checkable(cls: V) -> R {
    let p = match &cls {
        V::Class(k) => proto_info(k),
        _ => None,
    };
    let Some(p) = p else {
        return type_err(format!("@runtime_checkable can be only applied to protocol classes, got {}", repr(&cls)?));
    };
    if let Some(w) = &p.inexact {
        return Err(unsupported(w));
    }
    let mut nc = vec![];
    for a in &p.attrs {
        let v = match getattr(&cls, a) {
            Ok(v) => v,
            Err(e) if is_exc(&e.0, "AttributeError") => V::None,
            Err(_) => return type_err(format!("Failed to determine whether protocol member {} is a method member", repr(&s(a))?)),
        };
        if !truth(&bi_callable(Args::p(vec![v]))?)? {
            nc.push(*a);
        }
    }
    *p.noncallable.borrow_mut() = nc;
    p.runtime.set(true);
    Ok(cls)
}
// The entries of a class's __dict__ that a protocol check reads: CPython's exactly, or the refusal
fn proto_dict(k: &Rc<Class>) -> Result<Vec<(Sym, V)>, E> {
    if k.attrs.borrow().iter().any(|(n, _)| *n == "__dict__") {
        return Err(unsupported("isinstance() with a Protocol class on a class that defines __dict__"));
    }
    // a member is never __annotations__, the one entry a class whose annotations Brevi did not keep lacks
    match tinfo(k).and_then(|t| t.inexact.get()) {
        Some("annotations Brevi did not keep") => class_dict_with(k, false),
        _ => class_dict(k),
    }
}
// _ProtocolMeta.__instancecheck__ for a protocol class: the TypeError unless @runtime_checkable marked it, then
// ABCMeta's check with _proto_hook as __subclasshook__, then inspect.getattr_static of each member on the value
pub fn proto_instancecheck(cls: &Rc<Class>, p: &ProtoInfo, x: &V) -> Result<bool, E> {
    if !p.runtime.get() {
        return type_err("Instance and class checks can only be used with @runtime_checkable protocols".into());
    }
    if let Some(w) = &p.inexact {
        return Err(unsupported(w));
    }
    // getattr_static of a class looks through its metaclass too
    match x {
        V::Obj(_) | V::None | V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_) | V::Str(_) | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) | V::FSet(_) => {}
        _ => return Err(unsupported(&format!("isinstance() of a {} value with a Protocol class", tn(x)))),
    }
    let t = type_of(x)?;
    if t.meta.borrow().is_some() {
        return Err(unsupported("isinstance() with a Protocol class of an instance of a class with a metaclass"));
    }
    // ABCMeta's caches, then the hook, then a nominal subclass (protocols take no register() here)
    if abc_has(&cls.abc_cache.borrow(), &t) {
        return Ok(true);
    }
    let token = ABC_TOKEN.load(std::sync::atomic::Ordering::Relaxed);
    if cls.abc_neg_ver.get() < token {
        cls.abc_neg_cache.borrow_mut().clear();
        cls.abc_neg_ver.set(token);
    }
    if !abc_has(&cls.abc_neg_cache.borrow(), &t) {
        let mro: Vec<Rc<Class>> = t.mro.borrow().clone();
        let mut dicts = vec![];
        for k in &mro {
            dicts.push(proto_dict(k)?);
        }
        // _proto_hook(cls, type(x)): every member in some dict along the MRO, the first one found not None
        let mut hook = true;
        for a in &p.attrs {
            match dicts.iter().find_map(|d| d.iter().find(|(n, _)| n == a)) {
                Some((_, V::None)) | None => {
                    hook = false;
                    break;
                }
                Some(_) => {}
            }
        }
        if hook || t.is_sub(cls) {
            cls.abc_cache.borrow_mut().push(t.clone());
            return Ok(true);
        }
        cls.abc_neg_cache.borrow_mut().push(t.clone());
    }
    // inspect.getattr_static, member by member: the instance's own __dict__, then the dicts along the MRO (a data
    // descriptor there comes first); a member found None that is a method member fails the check
    let own = |a: &str| -> Result<Option<V>, E> {
        match x {
            V::Obj(o) => o.get_attr(a),
            _ => Ok(None),
        }
    };
    for a in &p.attrs {
        let mut klass = None;
        for k in t.mro.borrow().iter() {
            if let Some((_, v)) = proto_dict(k)?.into_iter().find(|(n, _)| n == a) {
                klass = Some(v);
                break;
            }
        }
        let inst = own(a)?;
        if let (V::Obj(o), Some(_)) = (x, &inst) {
            if o.cls.is_slot(a) || a.starts_with("__pydantic_") || a.starts_with("__bv_") || (*a == "args" && o.cls.base_bt() == BT::Exc) {
                return Err(unsupported(&format!("isinstance() with a Protocol class reading the slot {a}")));
            }
        }
        let none = match (&inst, &klass) {
            (None, None) => return Ok(false),
            (Some(i), None) => matches!(i, V::None),
            (None, Some(k)) => matches!(k, V::None),
            // whether the class attribute is a data descriptor only matters for an instance value of None
            (Some(V::None), Some(k)) => match k {
                V::None | V::Bool(_) | V::Int(_) | V::Big(_) | V::Float(_) | V::Str(_) | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) | V::FSet(_) | V::Func(_) => true,
                _ => return Err(unsupported(&format!("isinstance() with a Protocol class where the class attribute {a} may be a descriptor"))),
            },
            (Some(_), Some(_)) => false,
        };
        if none && !p.noncallable.borrow().contains(a) {
            return Ok(false);
        }
    }
    Ok(true)
}
// Calling a protocol class: the TypeError of typing's _no_init_or_replace_init (after object.__new__, which does
// nothing a program sees; a __new__ of the program's own is refused rather than run first)
pub fn proto_call_check(c: &Rc<Class>) -> Result<(), E> {
    let Some(p) = proto_info(c) else { return Ok(()) };
    if !p.no_init {
        return Ok(());
    }
    if c.mro.borrow().iter().any(|k| k.made.get() != 0 && k.own("__new__").is_some()) {
        return Err(unsupported("calling a Protocol class that defines __new__"));
    }
    type_err("Protocols cannot be instantiated".into())
}
