// ---------- typing, typing_extensions, collections.abc ----------
// The compiler (dynpy/typing.rs) lets typing names be values only where CPython's behaviour is plain: base
// classes, decorators, aliases, cast, TypeVar, NewType and the like. A typing form is a class of module
// "typing" that subscripts (getitem) and `|`s to itself; make_class turns NamedTuple and TypedDict bases into
// namedtuple and dict classes and drops Generic and Protocol.
thread_local! {
    static TYPING_FORMS: RefCell<Vec<Rc<Class>>> = RefCell::new(vec![]);
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
            Ok(V::Class(Class::new(sym(&to_str(&first).unwrap_or_default()), "typing", vec![bclass("object")], BT::Other)))
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
        V::Ext(e) if e.any().downcast_ref::<TAlias>().is_some() || e.any().downcast_ref::<TUnion>().is_some() => false,
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
