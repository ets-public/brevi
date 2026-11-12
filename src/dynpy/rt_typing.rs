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
        "assert_type" | "overload" | "final" | "override" | "no_type_check" | "runtime_checkable" => Ok(first),
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

// A PEP 695 type parameter (`def f[T]`): CPython's typing.TypeVar with infer_variance=True and no bound,
// constraints or default (the parser refuses those). Identity is the object's: equal and hashed only as itself.
pub struct TypeVarV {
    pub name: Sym,
}
pub fn pep695_typevar(n: &str) -> V {
    V::Ext(ExtRc::new(TypeVarV { name: sym(n) }))
}
impl TypeVarV {
    fn addr(&self) -> usize {
        self as *const TypeVarV as usize
    }
}
impl Ext for TypeVarV {
    fn tname(&self) -> &'static str {
        "typing.TypeVar"
    }
    // infer_variance: no ~, + or - in front, as CPython prints it
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
        Some(match n {
            "__name__" => Ok(s(self.name)),
            "__module__" => Ok(s("typing")),
            "__bound__" => Ok(V::None),
            "__constraints__" => Ok(tup(vec![])),
            "__covariant__" | "__contravariant__" => Ok(V::Bool(false)),
            "__infer_variance__" => Ok(V::Bool(true)),
            "__class__" => return None,
            // the rest of CPython's TypeVar (typing.NoDefault, has_default(), substitution hooks, `T | int`)
            // and object's other dunders are not made here
            _ if n.starts_with("__") || matches!(n, "has_default" | "evaluate_bound" | "evaluate_constraints" | "evaluate_default") => {
                Err(unsupported(&format!("TypeVar.{n}")))
            }
            _ => Err(exc("AttributeError", format!("'typing.TypeVar' object has no attribute '{n}'"))),
        })
    }
    fn set_attr(&self, _me: &V, n: &str, _v: V) -> Option<Result<(), E>> {
        Some(Err(unsupported(&format!("setting TypeVar.{n}"))))
    }
    fn binop(&self, _me: &V, op: &str, _o: &V, _right: bool) -> Option<R> {
        if op == "|" {
            return Some(Err(unsupported("a TypeVar in a typing union")));
        }
        None
    }
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
        if [a, b].iter().any(|x| matches!(x, V::Ext(e) if e.any().downcast_ref::<TypeVarV>().is_some())) {
            return Some(Err(unsupported("a TypeVar in a typing union")));
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
    Ok(V::Ext(ExtRc::new(TForm { name: n, params: 0, origin: None })))
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
            let t = t.s.as_str();
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
            Ok(V::Ext(ExtRc::new(TForm { name: n, params: args.len(), origin: None })))
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
            return Ok(V::Ext(ExtRc::new(TForm { name: k.name, params: 0, origin: Some(k.clone()) })));
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
