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
