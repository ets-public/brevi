// typing (and typing_extensions, collections.abc) in dynamic builds.
//
// Annotations are never evaluated by Brevi (the parser drops them), so a typing name in an annotation costs
// nothing. Everywhere else a typing name is a value whose behaviour the runtime only partly has: it can be a
// base class (Generic[T], Protocol, NamedTuple, TypedDict), a decorator (overload, final, ...), a call
// (TypeVar, NewType, cast, NamedTuple("P", [...]), a TypedDict class), the right side of an alias
// (`Vector = List[float]`) or the condition `if TYPE_CHECKING:`. Any other use (printing `List[int]`,
// `isinstance(x, Iterable)`, `get_type_hints`, `Movie.__annotations__`) is refused here, when the program is
// built, so the program goes to CPython instead of printing something else.
use super::parse::{Arg, ClassDef, Ex, FPart, Params, St, Stmt};
use std::collections::{HashMap, HashSet};

// The typing modules: their names are typing forms. collections.abc is a real module in dynamic builds.
pub fn typing_mod(m: &str) -> bool {
    matches!(m, "typing" | "typing_extensions")
}
// As typing_mod, with collections.abc treated as a typing module: only the typed build's isinstance guard
// still looks at programs that way (it must refuse them before the typed build tests against object).
fn typing_mod_for_guard(m: &str, abc_forms: bool) -> bool {
    matches!(m, "typing" | "typing_extensions") || (abc_forms && m == "collections.abc")
}

// Names of typing and typing_extensions the runtime has (anything else is refused when imported).
pub const TYPING_NAMES: &[&str] = &[
    "Any", "AnyStr", "Optional", "Union", "List", "Dict", "Set", "FrozenSet", "Tuple", "Type", "Callable", "Iterable", "Iterator",
    "Generator", "Sequence", "MutableSequence", "Mapping", "MutableMapping", "MutableSet", "AbstractSet", "Collection", "Container",
    "Hashable", "Sized", "Reversible", "Awaitable", "Coroutine", "AsyncIterator", "AsyncIterable", "AsyncGenerator", "ContextManager",
    "AsyncContextManager", "ItemsView", "KeysView", "ValuesView", "MappingView", "DefaultDict", "OrderedDict", "Counter", "Deque",
    "ChainMap", "IO", "TextIO", "BinaryIO", "Pattern", "Match", "ClassVar", "Final", "Literal", "Annotated", "Required", "NotRequired",
    "ReadOnly", "Self", "NoReturn", "Never", "LiteralString", "TypeAlias", "TypeGuard", "TypeIs", "Concatenate", "Unpack",
    "SupportsInt", "SupportsFloat", "SupportsIndex", "SupportsAbs", "SupportsRound", "SupportsBytes", "SupportsComplex", "Text",
    "TypeVar", "ParamSpec", "TypeVarTuple", "NoDefault", "NewType", "Generic", "Protocol", "NamedTuple", "TypedDict", "TYPE_CHECKING", "cast",
    "overload", "final", "override", "no_type_check", "runtime_checkable", "assert_never", "assert_type", "reveal_type",
];
// Names of collections.abc (annotations only: as a value, isinstance needs CPython's ABCs).
pub const ABC_NAMES: &[&str] = &[
    "Iterable", "Iterator", "Generator", "Sequence", "MutableSequence", "Mapping", "MutableMapping", "Set", "MutableSet", "Collection",
    "Container", "Hashable", "Sized", "Reversible", "Callable", "Awaitable", "Coroutine", "AsyncIterator", "AsyncIterable",
    "AsyncGenerator", "ItemsView", "KeysView", "ValuesView", "MappingView", "Buffer",
];
// Decorators that give the def or class back unchanged.
const DECORATORS: &[&str] = &["overload", "final", "override", "no_type_check", "runtime_checkable"];

pub fn module_has(m: &str, n: &str) -> bool {
    match m {
        "collections.abc" => ABC_NAMES.contains(&n),
        _ => TYPING_NAMES.contains(&n),
    }
}

// `TYPE_CHECKING` or `typing.TYPE_CHECKING`: False when the program runs, so its block is left out.
pub fn type_checking(c: &Ex) -> bool {
    matches!(c, Ex::Name(n) if n == "TYPE_CHECKING") || matches!(c, Ex::Attr(_, n) if n == "TYPE_CHECKING")
}

// What a module's names are, as far as typing goes.
#[derive(Default, Clone)]
struct Names {
    // name -> (typing module, typing name), for `from typing import X as name` and aliases (`V = List[int]`:
    // the name of what V stands for is "" then)
    forms: HashMap<String, (String, String)>,
    // names bound to a typing module (`import typing as t`; `collections` after `import collections.abc`)
    mods: HashMap<String, String>,
    generic: HashSet<String>,   // classes with Generic[...] or Protocol[...] among their bases (or a generic parent)
    protocols: HashSet<String>, // classes with Protocol among their bases
    tdicts: HashSet<String>,    // TypedDict classes
    ntuples: HashSet<String>,   // NamedTuple classes (their __annotations__ is not kept)
    classes: HashSet<String>,   // every class of the module, nested ones included
    // classes built on a subscripted generic class (`class D(Base[int, T])`) and no Generic[...] of their own: the
    // runtime does not work out their type parameters, so subscripting one (`D[str]`) is refused here
    on_alias: HashSet<String>,
    lazy: bool,                 // `from __future__ import annotations`: annotations are strings, not kept
    odd_ann: bool,              // a class annotation the runtime may not evaluate (`int | None`, `list[int]`)
}

type Er = (usize, String);
fn err(line: usize, what: &str) -> Er {
    (line, format!("typing: {what} needs CPython's typing"))
}

impl Names {
    // The typing name `e` stands for: (module, name); name "" for an alias of a typing expression.
    fn form(&self, e: &Ex) -> Option<(String, String)> {
        match e {
            Ex::Name(n) => self.forms.get(n).cloned(),
            Ex::Attr(o, a) => match o.as_ref() {
                Ex::Name(m) => self.mods.get(m).filter(|m| *m != "collections").map(|m| (m.clone(), a.clone())),
                Ex::Attr(c, abc) if abc == "abc" && matches!(c.as_ref(), Ex::Name(m) if self.mods.get(m).is_some_and(|x| x == "collections")) => Some(("collections.abc".into(), a.clone())),
                _ => None,
            },
            _ => None,
        }
    }
    fn is_form(&self, e: &Ex, name: &str) -> bool {
        self.form(e).is_some_and(|(_, n)| n == name)
    }
    // A typing expression: a typing name, a subscript of one, `X | None` with one in it.
    fn typing_expr(&self, e: &Ex) -> bool {
        match e {
            Ex::Name(n) => self.forms.contains_key(n) || self.tdicts.contains(n),
            Ex::Attr(..) => self.form(e).is_some(),
            Ex::Index(o, _) => self.typing_expr(o) || matches!(o.as_ref(), Ex::Name(n) if self.generic.contains(n)),
            Ex::Bin("|", a, b) => self.typing_expr(a) || self.typing_expr(b),
            Ex::Call(f, _) => matches!(self.form(f), Some((_, n)) if matches!(n.as_str(), "TypeVar" | "ParamSpec" | "TypeVarTuple" | "NewType")),
            _ => false,
        }
    }
    // `X`, `X[...]` for a generic user class X
    fn generic_class(&self, e: &Ex) -> bool {
        match e {
            Ex::Name(n) => self.generic.contains(n),
            Ex::Index(o, _) => self.generic_class(o),
            _ => false,
        }
    }
}

// Every module's typing uses, checked; Err(module, line, why) for the first one the runtime does not have.
// Set while the program uses Brevi's pydantic (its TypeAdapter takes typing forms as descriptors).
pub static OWN_PYDANTIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// Set while the program's defs keep their annotations (see with_ann in rt_call.rs): one of its modules reads
// `.__annotations__`, no module has `from __future__ import annotations` (strings Brevi does not make), the
// program does not use Brevi's pydantic, and every def's annotation is a plain name, attribute, string or None
// (a type expression such as `int | None` or `list[int]` has no runtime object here). Each def then evaluates its
// annotations when it is made, as CPython 3.13 does; otherwise a read of a def's __annotations__ is refused.
pub static ANN_DEFS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// Set while the program names `from_callable` (inspect.Signature.from_callable) and its defs keep their
// annotations (ANN_DEFS, which the use turns on under the same conditions): every def and lambda then records its
// parameters and defaults (with_sig in rt_sig.rs). Without ANN_DEFS a from_callable call is refused at build time.
pub static SIG_DEFS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn check(mods: &[(&str, &[Stmt])]) -> Result<(), (String, usize, String)> {
    let mut names: HashMap<String, Names> = HashMap::new();
    // twice, for names a module imports from another module of the program
    for _ in 0..2 {
        for (m, body) in mods {
            let mut nm = names.get(*m).cloned().unwrap_or_default();
            collect(body, &mut nm, &names, true, false);
            names.insert(m.to_string(), nm);
        }
    }
    // classes on a subscripted generic class at any depth (collect reads only module-level statements)
    fn deep_alias(body: &[Stmt], nm: &mut Names, gens: &mut HashSet<String>) {
        for s in body {
            if let St::Class(c) = &s.st {
                let generic = |o: &Ex, nm: &Names, gens: &HashSet<String>| nm.generic_class(o) || matches!(o, Ex::Name(n) if gens.contains(n));
                let own = c.bases.iter().any(|b| matches!(b, Ex::Index(o, _) if nm.is_form(o, "Generic") || nm.is_form(o, "Protocol")));
                if !own && c.bases.iter().any(|b| matches!(b, Ex::Index(o, _) if generic(o, nm, gens))) {
                    nm.on_alias.insert(c.name.clone());
                }
                if own || c.bases.iter().any(|b| match b {
                    Ex::Index(o, _) => generic(o, nm, gens),
                    b => generic(b, nm, gens),
                }) {
                    gens.insert(c.name.clone());
                }
            }
            blocks(&s.st, &mut |b| deep_alias(b, nm, gens));
        }
    }
    for (m, body) in mods {
        let nm = names.get_mut(*m).unwrap();
        deep_alias(body, nm, &mut HashSet::new());
        class_names(body, &mut nm.classes);
        nm.lazy = future_annotations(body);
        nm.odd_ann = odd_class_annotation(body, false);
    }
    let reads = mods.iter().any(|(_, b)| {
        let mut r = false;
        walk_all(b, &mut |e, _| {
            r |= matches!(e, Ex::Attr(_, a) | Ex::Name(a) if a == "__annotations__" || a == "WRAPPER_ASSIGNMENTS") || matches!(e, Ex::Str(t) if t == "__annotations__")
        });
        r
    });
    let sig = mods.iter().any(|(_, b)| {
        let mut r = false;
        walk_all(b, &mut |e, _| r |= matches!(e, Ex::Attr(_, a) if a == "from_callable") || matches!(e, Ex::Str(t) if t == "from_callable"));
        r
    });
    let defs = (reads || sig)
        && !OWN_PYDANTIC.load(std::sync::atomic::Ordering::Relaxed)
        && mods.iter().all(|(m, b)| !names[*m].lazy && !odd_def_annotation(b) && !typing_def_annotation(b, &names[*m]));
    ANN_DEFS.store(defs, std::sync::atomic::Ordering::Relaxed);
    SIG_DEFS.store(sig && defs, std::sync::atomic::Ordering::Relaxed);
    for (m, body) in mods {
        let nm = &names[*m];
        stmts(body, nm).map_err(|(l, w)| (m.to_string(), l, w))?;
    }
    if let Some((m, l)) = generic_introspection(mods, &names) {
        return Err((m, l, "typing: reading a class's bases, MRO, __dict__, dir() or vars() in a program with a class on Generic or Protocol (left out of its bases here) needs CPython's typing".into()));
    }
    // inspect.signature and its kin run in CPython's inspect (through the bridge), which cannot read the
    // program's own functions and classes: refused rather than raising there
    for (m, body) in mods {
        if !imports(body, "inspect") {
            continue;
        }
        const SIG: [&str; 3] = ["signature", "getfullargspec", "getcallargs"];
        let mut at = None;
        walk_all(body, &mut |e, line| {
            if matches!(e, Ex::Attr(o, a) if SIG.contains(&a.as_str()) && matches!(o.as_ref(), Ex::Name(n) if n == "inspect")) {
                at.get_or_insert(line);
            }
        });
        let mut from = false;
        walk_from(body, &mut |md, names| from |= md == "inspect" && names.iter().any(|n| SIG.contains(&n.as_str())));
        if from && at.is_none() {
            at = Some(0);
        }
        if let Some(l) = at {
            return Err((m.to_string(), l, "inspect.signature of a dynamic build's functions needs CPython".into()));
        }
    }
    // a dataclass's generated __doc__ is the text of inspect.signature(cls), which Brevi writes only for the
    // __init__ dataclasses itself makes: a program that reads __doc__ and has a def __init__ or an init=False is
    // refused (the class's own __init__, or its base's, would give the text)
    for (m, body) in mods {
        if !imports(body, "dataclasses") {
            continue;
        }
        let mut doc = None;
        walk_all(body, &mut |e, line| {
            if matches!(e, Ex::Attr(_, a) if a == "__doc__") || matches!(e, Ex::Str(t) if t == "__doc__") || matches!(e, Ex::Attr(_, a) | Ex::Name(a) if a == "getdoc") {
                doc.get_or_insert(line);
            }
        });
        let Some(l) = doc else { continue };
        let mut own_init = false;
        let mut no_init = false;
        walk_all(body, &mut |e, _| {
            if let Ex::Call(_, args) = e {
                no_init |= args.iter().any(|a| matches!(a, Arg::Kw(k, v) if k == "init" && !matches!(v, Ex::True)) || matches!(a, Arg::KwStar(_)));
            }
        });
        walk_defs(body, &mut |name| own_init |= name == "__init__");
        if own_init || no_init {
            return Err((m.to_string(), l, "dataclasses: the __doc__ of a dataclass whose __init__ dataclasses does not write needs CPython".into()));
        }
    }
    // The program's own objects cannot pass to CPython's pickle or unittest.mock (the bridge stops at run time):
    // the uses that would hand them over are refused here, before the program runs.
    for (m, body) in mods {
        let classes = &names[*m].classes;
        // make_dataclass sets __module__ from its caller's frame; Brevi gives "__main__"
        if *m != "__main__" {
            let mut at = None;
            walk_all(body, &mut |e, line| {
                if matches!(e, Ex::Name(n) | Ex::Attr(_, n) if n == "make_dataclass") {
                    at.get_or_insert(line);
                }
            });
            if let Some(l) = at {
                return Err((m.to_string(), l, "dataclasses.make_dataclass outside the main module needs CPython".into()));
            }
        }
        // field(metadata=M(...)) with a mapping class of the program (types.MappingProxyType over it)
        let mut at = None;
        walk_all(body, &mut |e, line| {
            if let Ex::Call(f, args) = e {
                if matches!(f.as_ref(), Ex::Name(n) | Ex::Attr(_, n) if n == "field")
                    && args.iter().any(|a| matches!(a, Arg::Kw(k, Ex::Call(g, _)) if k == "metadata" && matches!(g.as_ref(), Ex::Name(n) if classes.contains(n))))
                {
                    at.get_or_insert(line);
                }
            }
        });
        if let Some(l) = at {
            return Err((m.to_string(), l, "dataclasses: field(metadata=...) with a mapping class of the program needs CPython".into()));
        }
        // pickling a dataclass instance or class
        if imports(body, "pickle") {
            let mut dc = false;
            let mut pk = None;
            walk_all(body, &mut |e, line| {
                dc |= matches!(e, Ex::Name(n) | Ex::Attr(_, n) if n == "dataclass" || n == "make_dataclass");
                if matches!(e, Ex::Attr(o, a) if matches!(a.as_str(), "dumps" | "dump" | "Pickler") && matches!(o.as_ref(), Ex::Name(n) if n == "pickle")) {
                    pk.get_or_insert(line);
                }
            });
            walk_from(body, &mut |md, ns| {
                if md == "pickle" && ns.iter().any(|n| matches!(n.as_str(), "dumps" | "dump" | "Pickler")) {
                    pk.get_or_insert(0);
                }
            });
            if let (true, Some(l)) = (dc, pk) {
                return Err((m.to_string(), l, "pickle of the program's dataclasses needs CPython".into()));
            }
        }
        // a unittest.mock object stored on a class of the program (`D.__set__ = Mock()`, or `x = Mock()` in its
        // body): the runtime would call it with the program's objects
        if imports(body, "unittest") {
            fn is_mock(e: &Ex) -> bool {
                matches!(e, Ex::Call(f, _) if matches!(f.as_ref(), Ex::Name(n) | Ex::Attr(_, n) if matches!(n.as_str(), "Mock" | "MagicMock" | "NonCallableMock" | "NonCallableMagicMock" | "AsyncMock" | "PropertyMock" | "create_autospec")))
            }
            fn walk(body: &[Stmt], classes: &HashSet<String>, in_class: bool, at: &mut Option<usize>) {
                for s in body {
                    match &s.st {
                        St::Assign(ts, v) if is_mock(v) => {
                            if in_class || ts.iter().any(|t| matches!(t, Ex::Attr(o, _) if matches!(o.as_ref(), Ex::Name(n) if classes.contains(n)))) {
                                at.get_or_insert(s.line);
                            }
                        }
                        St::AnnAssign(_, Some(v), _, _) if in_class && is_mock(v) => {
                            at.get_or_insert(s.line);
                        }
                        St::Expr(Ex::Call(f, args)) if matches!(f.as_ref(), Ex::Name(n) if n == "setattr") => {
                            if matches!(args.first(), Some(Arg::Pos(Ex::Name(n))) if classes.contains(n)) && matches!(args.get(2), Some(Arg::Pos(v)) if is_mock(v)) {
                                at.get_or_insert(s.line);
                            }
                        }
                        St::Class(c) => walk(&c.body, classes, true, at),
                        st => blocks(st, &mut |b| walk(b, classes, false, at)),
                    }
                }
            }
            let mut at = None;
            walk(body, classes, false, &mut at);
            if let Some(l) = at {
                return Err((m.to_string(), l, "unittest.mock: a Mock stored on a class of the program needs CPython".into()));
            }
        }
    }
    // dataclasses.Field.type and a dataclass's generated __doc__ show the class's annotations, which the build
    // keeps (emit.rs keep_ann) unless the module has `from __future__ import annotations`
    let lazy_dc = mods.iter().any(|(m, b)| names[*m].lazy && imports(b, "dataclasses"));
    if lazy_dc {
        let mut at = None;
        for (m, body) in mods {
            walk_all(body, &mut |e, line| {
                if matches!(e, Ex::Attr(_, a) if a == "__doc__") && at.is_none() {
                    at = Some((m.to_string(), line));
                }
            });
        }
        if let Some((m, l)) = at {
            return Err((m, l, "dataclasses: the __doc__ of a dataclass in a module with `from __future__ import annotations` needs CPython".into()));
        }
    }
    let mut fields_used = false;
    let mut type_at = None;
    for (m, body) in mods {
        walk_all(body, &mut |e, line| match e {
            Ex::Name(n) if n == "fields" || n == "__dataclass_fields__" => fields_used = true,
            Ex::Attr(_, a) if a == "fields" || a == "__dataclass_fields__" => fields_used = true,
            Ex::Attr(_, a) if a == "type" && type_at.is_none() => type_at = Some((m.to_string(), line)),
            _ => {}
        });
    }
    // json.JSONEncoder.iterencode: CPython's chunks (and encode, dump through it), which Brevi does not make
    for (m, body) in mods {
        let mut at = None;
        walk_all(body, &mut |e, line| {
            if matches!(e, Ex::Attr(_, a) if a == "iterencode") {
                at.get_or_insert(line);
            }
        });
        let mut defs = |b: &[Stmt]| {
            for s in b {
                if let St::Class(c) = &s.st {
                    for x in &c.body {
                        if matches!(&x.st, St::Def(d) if d.name == "iterencode") {
                            at.get_or_insert(x.line);
                        }
                    }
                }
            }
        };
        defs(body);
        if let Some(l) = at {
            return Err((m.to_string(), l, "json: JSONEncoder.iterencode needs CPython".into()));
        }
    }
    if let (true, true, Some((m, l))) = (lazy_dc, fields_used, type_at) {
        return Err((m, l, "dataclasses: a field's .type in a module with `from __future__ import annotations` needs CPython".into()));
    }
    Ok(())
}
// Every expression of a body, with its statement's line.
pub fn walk_all(body: &[Stmt], f: &mut dyn FnMut(&Ex, usize)) {
    for s in body {
        let line = s.line;
        crate::typed_guard::walk_st(&s.st, &mut |e| f(e, line));
        if let St::Def(d) = &s.st {
            for x in &d.decorators {
                crate::typed_guard::walk_ex(x, &mut |e| f(e, line));
            }
        }
        if let St::Class(c) = &s.st {
            for x in c.decorators.iter().chain(c.bases.iter()).chain(c.kwargs.iter().map(|(_, x)| x)) {
                crate::typed_guard::walk_ex(x, &mut |e| f(e, line));
            }
        }
        blocks(&s.st, &mut |b| walk_all(b, f));
    }
}

// The typed build's isinstance(x, Iterable) with a typing or collections.abc name: the line (the typed build
// would test against object).
pub fn typed_isinstance(body: &[Stmt]) -> Option<usize> {
    let mut nm = Names::default();
    collect(body, &mut nm, &HashMap::new(), true, true);
    let mut found = None;
    fn walk(body: &[Stmt], nm: &Names, found: &mut Option<usize>) {
        for s in body {
            crate::typed_guard::walk_st(&s.st, &mut |e| {
                if let Ex::Call(f, args) = e {
                    // a class pattern (`case C():`, rewritten to __brevi_pm_cls(subject, C, ...)) is an isinstance
                    if matches!(f.as_ref(), Ex::Name(n) if n == "isinstance" || n == "issubclass" || n == "__brevi_pm_cls") {
                        if let Some(Arg::Pos(t)) = args.get(1) {
                            let mut hit = false;
                            crate::typed_guard::walk_ex(t, &mut |x| hit |= nm.form(x).is_some());
                            if hit && found.is_none() {
                                *found = Some(s.line);
                            }
                        }
                    }
                }
            });
            blocks(&s.st, &mut |b| walk(b, nm, found));
        }
    }
    walk(body, &nm, &mut found);
    found
}

// Does the program use collections.abc only where Brevi's typing forms do (annotations, aliases)?
pub fn uses_abc(mods: &[(&str, &[Stmt])]) -> bool {
    let mut any = false;
    for (_, body) in mods {
        walk_imports(body, &mut |m| any |= m == "collections.abc" || m == "collections");
    }
    any
}
// Whether the body imports module `m` (or a submodule of it, or a name from it) anywhere.
pub fn imports(body: &[Stmt], m: &str) -> bool {
    let mut any = false;
    walk_imports(body, &mut |x| any |= x == m || x.strip_prefix(m).is_some_and(|r| r.starts_with('.')));
    any
}
fn walk_from(body: &[Stmt], f: &mut dyn FnMut(&str, Vec<String>)) {
    for s in body {
        if let St::ImportFrom(m, v) = &s.st {
            f(m, v.iter().map(|(n, _)| n.clone()).collect());
        }
        blocks(&s.st, &mut |b| walk_from(b, f));
    }
}
fn walk_defs(body: &[Stmt], f: &mut dyn FnMut(&str)) {
    for s in body {
        if let St::Def(d) = &s.st {
            f(&d.name);
        }
        blocks(&s.st, &mut |b| walk_defs(b, f));
    }
}
fn walk_imports(body: &[Stmt], f: &mut dyn FnMut(&str)) {
    for s in body {
        match &s.st {
            St::Import(v) => v.iter().for_each(|(m, _)| f(m)),
            St::ImportFrom(m, _) => f(m),
            _ => {}
        }
        blocks(&s.st, &mut |b| walk_imports(b, f));
    }
}

pub(crate) fn blocks(st: &St, f: &mut dyn FnMut(&[Stmt])) {
    match st {
        St::If(arms, els) => {
            arms.iter().for_each(|(_, b)| f(b));
            if let Some(b) = els {
                f(b);
            }
        }
        St::While(_, b, e) | St::For(_, _, b, e) => {
            f(b);
            if let Some(b) = e {
                f(b);
            }
        }
        St::Try(b, hs, e, fin) => {
            f(b);
            hs.iter().for_each(|h| f(&h.body));
            for b in [e, fin].into_iter().flatten() {
                f(b);
            }
        }
        St::With(_, b) => f(b),
        St::Def(d) => f(&d.body),
        St::Class(c) => f(&c.body),
        _ => {}
    }
}

// Every class a module defines, at any depth (a class's __annotations__ is kept by the runtime; nothing
// else's is, so only reads on a class name are allowed below).
fn class_names(body: &[Stmt], out: &mut HashSet<String>) {
    for s in body {
        if let St::Class(c) = &s.st {
            out.insert(c.name.clone());
        }
        blocks(&s.st, &mut |b| class_names(b, out));
    }
}

// Whether a class body (at any depth) has an annotation other than a name, a dotted name or a string: once the
// program reads __annotations__, class annotations are evaluated as the class body runs (dynpy/emit.rs), and
// the runtime has no `int | None` or `list[int]` type objects, so such a program is refused, not run to a
// TypeError that CPython would not raise.
fn odd_class_annotation(body: &[Stmt], in_class: bool) -> bool {
    fn plain(e: &Ex) -> bool {
        match e {
            Ex::Name(_) | Ex::Str(_) | Ex::NoneV => true,
            Ex::Attr(o, _) => plain(o),
            _ => false,
        }
    }
    body.iter().any(|s| match &s.st {
        St::AnnAssign(_, _, ann, _) if in_class => !plain(ann),
        St::Class(c) => odd_class_annotation(&c.body, true),
        // a def's own annotated statements are not evaluated (a function body keeps no annotations)
        St::Def(d) => odd_class_annotation(&d.body, false),
        st => {
            let mut odd = false;
            blocks(st, &mut |b| odd |= odd_class_annotation(b, in_class));
            odd
        }
    })
}

// A def anywhere in the body (methods and nested defs too) with an annotation that is not a plain name,
// attribute, string or None.
fn odd_def_annotation(body: &[Stmt]) -> bool {
    fn plain(e: &Ex) -> bool {
        match e {
            Ex::Name(_) | Ex::Str(_) | Ex::NoneV => true,
            Ex::Attr(o, _) => plain(o),
            _ => false,
        }
    }
    body.iter().any(|s| match &s.st {
        St::Def(d) => d.params.ann.iter().map(|(_, a)| a).chain(d.ret.iter()).any(|a| !plain(a)) || odd_def_annotation(&d.body),
        St::Class(c) => odd_def_annotation(&c.body),
        st => {
            let mut odd = false;
            blocks(st, &mut |b| odd |= odd_def_annotation(b));
            odd
        }
    })
}

// A def anywhere in the body whose annotation names a typing form (`List`, `t.Optional`): as a value, that needs
// CPython's typing.
fn typing_def_annotation(body: &[Stmt], nm: &Names) -> bool {
    body.iter().any(|s| match &s.st {
        St::Def(d) => d.params.ann.iter().map(|(_, a)| a).chain(d.ret.iter()).any(|a| nm.typing_expr(a)) || typing_def_annotation(&d.body, nm),
        St::Class(c) => typing_def_annotation(&c.body, nm),
        st => {
            let mut t = false;
            blocks(st, &mut |b| t |= typing_def_annotation(b, nm));
            t
        }
    })
}

fn future_annotations(body: &[Stmt]) -> bool {
    body.iter().any(|s| matches!(&s.st, St::ImportFrom(m, v) if m == "__future__" && v.iter().any(|(n, _)| n == "annotations")))
}

// `o.__annotations__` is kept when o is a class of the program, or an instance of one (`C().__annotations__`).
fn on_class(x: &Ex, nm: &Names) -> bool {
    match x {
        Ex::Name(n) => nm.classes.contains(n) && !nm.tdicts.contains(n) && !nm.ntuples.contains(n),
        Ex::Call(f, _) => on_class(f, nm),
        _ => false,
    }
}

fn collect(body: &[Stmt], nm: &mut Names, all: &HashMap<String, Names>, top: bool, abc_forms: bool) {
    let typing = |m: &str| typing_mod_for_guard(m, abc_forms);
    for s in body {
        match &s.st {
            St::Import(v) => {
                for (m, a) in v {
                    if typing(m) && a.is_some() {
                        nm.mods.insert(a.clone().unwrap(), m.clone());
                    } else if typing(m) {
                        nm.mods.insert(m.split('.').next().unwrap().to_string(), if m == "collections.abc" { "collections".into() } else { m.clone() });
                    }
                }
            }
            St::ImportFrom(m, v) => {
                for (n, a) in v {
                    let bind = a.clone().unwrap_or_else(|| n.clone());
                    if typing(m) {
                        nm.forms.insert(bind, (m.clone(), n.clone()));
                    } else if m == "collections" && n == "abc" {
                        nm.mods.insert(bind, "collections.abc".into());
                    } else if let Some(o) = all.get(m.as_str()) {
                        if let Some(f) = o.forms.get(n) {
                            nm.forms.insert(bind.clone(), f.clone());
                        }
                        for (from, to) in [(&o.generic, &mut nm.generic), (&o.protocols, &mut nm.protocols), (&o.tdicts, &mut nm.tdicts), (&o.on_alias, &mut nm.on_alias)] {
                            if from.contains(n) {
                                to.insert(bind.clone());
                            }
                        }
                    }
                }
            }
            // `Vector = List[float]`, `T = TypeVar("T")`, `Vector: TypeAlias = list[float]`
            St::Assign(ts, v) if top && ts.len() == 1 && matches!(&ts[0], Ex::Name(_)) && nm.typing_expr(v) => {
                if let Ex::Name(n) = &ts[0] {
                    // `UserId = NewType("UserId", int)` is called like a function
                    let made = if matches!(v, Ex::Call(f, _) if nm.is_form(f, "NewType")) { "@newtype" } else { "" };
                    nm.forms.insert(n.clone(), (String::new(), made.into()));
                }
            }
            St::AnnAssign(Ex::Name(n), Some(v), _, _) if top && nm.typing_expr(v) => {
                nm.forms.insert(n.clone(), (String::new(), String::new()));
            }
            St::Class(c) => {
                let own = c.bases.iter().any(|b| matches!(b, Ex::Index(o, _) if nm.is_form(o, "Generic") || nm.is_form(o, "Protocol")));
                if !own && c.bases.iter().any(|b| matches!(b, Ex::Index(o, _) if nm.generic_class(o))) {
                    nm.on_alias.insert(c.name.clone());
                }
                for b in &c.bases {
                    let head = match b {
                        Ex::Index(o, _) => o.as_ref(),
                        b => b,
                    };
                    if nm.is_form(head, "TypedDict") || matches!(head, Ex::Name(n) if nm.tdicts.contains(n)) {
                        nm.tdicts.insert(c.name.clone());
                    }
                    if nm.is_form(head, "Protocol") {
                        nm.protocols.insert(c.name.clone());
                    }
                    if nm.is_form(head, "NamedTuple") {
                        nm.ntuples.insert(c.name.clone());
                    }
                    if (matches!(b, Ex::Index(..)) && (nm.is_form(head, "Generic") || nm.is_form(head, "Protocol"))) || nm.generic_class(b) {
                        nm.generic.insert(c.name.clone());
                    }
                }
            }
            _ => {}
        }
        // `class C[T]` (PEP 695): the parser's hidden annotation scope holds the class, on __brevi_generic(T)
        if let St::Def(d) = &s.st {
            for x in d.body.iter().filter(|_| d.name.starts_with("__brevi_gp_")) {
                if let St::Class(c) = &x.st {
                    if c.bases.iter().any(pep695_base) {
                        nm.generic.insert(c.disp.clone().unwrap_or_else(|| c.name.clone()));
                    }
                }
            }
        }
        // `if TYPE_CHECKING:` imports, aliases in an `if` or `try`
        match &s.st {
            St::Def(_) | St::Class(_) => {}
            st => blocks(st, &mut |b| collect(b, nm, all, top, abc_forms)),
        }
    }
}

// The base the parser gives a `class C[T]` (PEP 695 Generic[T], made at run time)
fn pep695_base(b: &Ex) -> bool {
    matches!(b, Ex::Call(f, _) if matches!(f.as_ref(), Ex::Name(n) if n == "__brevi_generic"))
}
// What a class on typing's Generic or Protocol shows differently here: the runtime takes Generic and Protocol out
// of its bases (so __bases__, __mro__ and mro() leave them out at any depth), and the class has no __orig_bases__,
// __class_getitem__ or Protocol's own attributes, so its __dict__, vars() and dir() differ too.
const GENERIC_INTROSPECTION: &[&str] = &[
    "__bases__", "__mro__", "mro", "__orig_bases__", "__class_getitem__", "__dict__", "__protocol_attrs__", "_is_protocol",
    "__non_callable_proto_members__", "__abstractmethods__", "_abc_impl", "get_original_bases", "getmro",
];
// A program with such a class that reads one of those (by attribute, by name in getattr or hasattr, or by dir()
// or vars()): the first module and line, refused rather than answered without Generic or Protocol.
fn generic_introspection(mods: &[(&str, &[Stmt])], names: &HashMap<String, Names>) -> Option<(String, usize)> {
    fn has_generic(body: &[Stmt], nm: &Names) -> bool {
        body.iter().any(|s| {
            let own = matches!(&s.st, St::Class(c) if c.bases.iter().any(|b| pep695_base(b) || match b {
                Ex::Index(o, _) => nm.is_form(o, "Generic") || nm.is_form(o, "Protocol"),
                b => nm.is_form(b, "Protocol") || nm.is_form(b, "Generic"),
            }));
            let mut deep = false;
            blocks(&s.st, &mut |b| deep |= has_generic(b, nm));
            own || deep
        })
    }
    if !mods.iter().any(|(m, b)| has_generic(b, &names[*m])) {
        return None;
    }
    for (m, body) in mods {
        let mut at = None;
        walk_all(body, &mut |e, line| {
            let hit = match e {
                Ex::Attr(_, a) => GENERIC_INTROSPECTION.contains(&a.as_str()),
                Ex::Str(t) => t.starts_with('_') && GENERIC_INTROSPECTION.contains(&t.as_str()),
                Ex::Call(f, args) => match f.as_ref() {
                    Ex::Name(n) if n == "dir" || n == "vars" => true,
                    Ex::Name(n) if n == "getattr" || n == "hasattr" => matches!(args.get(1), Some(Arg::Pos(Ex::Str(t))) if GENERIC_INTROSPECTION.contains(&t.as_str())),
                    _ => false,
                },
                Ex::Name(n) => n == "get_original_bases" || n == "getmro",
                _ => false,
            };
            if hit {
                at.get_or_insert(line);
            }
        });
        if let Some(l) = at {
            return Some((m.to_string(), l));
        }
    }
    None
}

fn stmts(body: &[Stmt], nm: &Names) -> Result<(), Er> {
    for s in body {
        stmt(&s.st, s.line, nm)?;
    }
    Ok(())
}

fn stmt(st: &St, line: usize, nm: &Names) -> Result<(), Er> {
    let ex = |e: &Ex| expr(e, false, line, nm);
    match st {
        St::Expr(e) | St::Return(Some(e)) => ex(e)?,
        St::Assign(ts, v) => {
            for t in ts.iter().filter(|t| !matches!(t, Ex::Name(_))) {
                ex(t)?;
            }
            // an alias: the right side may be any typing expression
            let alias = ts.len() == 1 && matches!(&ts[0], Ex::Name(n) if nm.forms.contains_key(n));
            expr(v, alias, line, nm)?;
        }
        St::AnnAssign(t, v, _, _) => {
            if !matches!(t, Ex::Name(_)) {
                ex(t)?;
            }
            if let Some(v) = v {
                let alias = matches!(t, Ex::Name(n) if nm.forms.contains_key(n));
                expr(v, alias, line, nm)?;
            }
        }
        St::AugAssign(t, _, v) => {
            ex(t)?;
            ex(v)?;
        }
        St::If(arms, els) => {
            for (c, b) in arms {
                // `if TYPE_CHECKING:` / `if not TYPE_CHECKING:`
                let c = match c {
                    Ex::Unary("not", x) => x.as_ref(),
                    c => c,
                };
                if !nm.is_form(c, "TYPE_CHECKING") {
                    ex(c)?;
                }
                stmts(b, nm)?;
            }
            if let Some(b) = els {
                stmts(b, nm)?;
            }
        }
        St::While(c, b, e) => {
            ex(c)?;
            stmts(b, nm)?;
            if let Some(e) = e {
                stmts(e, nm)?;
            }
        }
        St::For(t, it, b, e) => {
            ex(t)?;
            ex(it)?;
            stmts(b, nm)?;
            if let Some(e) = e {
                stmts(e, nm)?;
            }
        }
        St::Try(b, hs, e, fin) => {
            stmts(b, nm)?;
            for h in hs {
                if let Some(t) = &h.types {
                    expr(t, false, h.line, nm)?;
                }
                stmts(&h.body, nm)?;
            }
            for b in [e, fin].into_iter().flatten() {
                stmts(b, nm)?;
            }
        }
        St::With(items, b) => {
            for (e, t) in items {
                ex(e)?;
                if let Some(t) = t {
                    ex(t)?;
                }
            }
            stmts(b, nm)?;
        }
        St::Raise(a, b) => {
            for e in [a, b].into_iter().flatten() {
                ex(e)?;
            }
        }
        St::Assert(a, b) => {
            ex(a)?;
            if let Some(b) = b {
                ex(b)?;
            }
        }
        St::Del(xs) => xs.iter().try_for_each(|x| ex(x))?,
        St::ImportFrom(m, v) if typing_mod(m) => {
            for (n, _) in v {
                if !module_has(m, n) {
                    return Err(err(line, &format!("{m}.{n}")));
                }
            }
        }
        St::Def(d) => {
            decorators(&d.decorators, d.line, nm)?;
            params(&d.params, d.line, nm)?;
            // a Protocol class made in the function (`from typing import Protocol` there too): isinstance with
            // it, or a class pattern on it, is refused as at module level. Only the protocol names are taken
            // from the body, so nothing else here is checked differently.
            let mut probe = nm.clone();
            collect(&d.body, &mut probe, &HashMap::new(), false, false);
            if probe.protocols.len() > nm.protocols.len() {
                let mut local = nm.clone();
                local.protocols = probe.protocols;
                stmts(&d.body, &local)?;
            } else {
                stmts(&d.body, nm)?;
            }
        }
        St::Class(c) => class(c, nm)?,
        _ => {}
    }
    Ok(())
}

fn decorators(ds: &[Ex], line: usize, nm: &Names) -> Result<(), Er> {
    for d in ds {
        match nm.form(d) {
            Some((_, n)) if DECORATORS.contains(&n.as_str()) => {}
            _ => expr(d, false, line, nm)?,
        }
    }
    Ok(())
}

fn params(p: &Params, line: usize, nm: &Names) -> Result<(), Er> {
    for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
        if let Some(d) = d {
            expr(d, false, line, nm)?;
        }
    }
    Ok(())
}

fn class(c: &ClassDef, nm: &Names) -> Result<(), Er> {
    decorators(&c.decorators, c.line, nm)?;
    let td = nm.tdicts.contains(&c.name);
    for (k, v) in &c.kwargs {
        if k == "metaclass" {
            expr(v, false, c.line, nm)?;
            // the metaclass sees the namespace, so the class's annotations are made: a typing form there is a value
            if c.body.iter().any(|s| matches!(&s.st, St::AnnAssign(_, _, a, _) if nm.typing_expr(a))) {
                return Err((c.line, "typing: an annotation naming a typing form in a class with a metaclass needs CPython's typing".into()));
            }
            continue;
        }
        if !td || k != "total" || !matches!(v, Ex::True | Ex::False) {
            return Err((c.line, "class keyword arguments are not supported".into()));
        }
    }
    for b in &c.bases {
        let (head, sub) = match b {
            Ex::Index(o, i) => (o.as_ref(), Some(i.as_ref())),
            b => (b, None),
        };
        let ok = match nm.form(head) {
            // Generic needs its parameters; Protocol may have them
            Some((_, n)) => match n.as_str() {
                "Generic" => sub.is_some(),
                "Protocol" => true,
                "NamedTuple" | "TypedDict" => sub.is_none(),
                _ => false,
            },
            None => match head {
                Ex::Name(n) if nm.tdicts.contains(n) => sub.is_none(),
                Ex::Name(n) if nm.generic.contains(n) => true,
                _ => {
                    expr(b, false, c.line, nm)?;
                    continue;
                }
            },
        };
        if !ok {
            return Err(err(c.line, "this base class"));
        }
        if let Some(i) = sub {
            expr(i, true, c.line, nm)?;
        }
    }
    // A NamedTuple or TypedDict class body: fields, defaults, methods and a docstring, as CPython takes
    stmts(&c.body, nm)
}

// `e` where a typing expression may be (`ok`: an alias's right side, a subscript of a typing form, ...).
fn expr(e: &Ex, ok: bool, line: usize, nm: &Names) -> Result<(), Er> {
    let go = |x: &Ex| expr(x, false, line, nm);
    match e {
        Ex::Name(n) => {
            // TYPE_CHECKING and NoDefault are plain values, not forms
            if (nm.forms.contains_key(n) || nm.tdicts.contains(n)) && !ok && !nm.is_form(e, "TYPE_CHECKING") && !nm.is_form(e, "NoDefault") {
                return Err(err(line, &format!("`{n}` used as a value")));
            }
            if nm.generic.contains(n) || nm.protocols.contains(n) {
                // a generic or protocol class used as a plain value: a class, which the runtime has
            }
            Ok(())
        }
        Ex::Attr(o, a) => {
            if let Some((m, n)) = nm.form(e) {
                if !module_has(&m, &n) {
                    return Err(err(line, &format!("{m}.{n}")));
                }
                if !ok && n != "TYPE_CHECKING" && n != "NoDefault" {
                    return Err(err(line, &format!("`{m}.{a}` used as a value")));
                }
                return Ok(());
            }
            // Class annotations are kept (dynpy/emit.rs); anything else's are not, and neither are a
            // NamedTuple's, a TypedDict's, nor any under `from __future__ import annotations`. An instance
            // `C().__annotations__` reads its class's own, so a call of a class is allowed too. Brevi's own
            // pydantic replaces every class's __annotations__ with descriptors, so its programs stay refused.
            if a == "__annotations__" {
                let own_pyd = OWN_PYDANTIC.load(std::sync::atomic::Ordering::Relaxed);
                if nm.lazy || own_pyd {
                    return Err(err(line, ".__annotations__"));
                }
                // anything but a class of the program: a def's (kept under ANN_DEFS), and at run time a
                // builtin's AttributeError; a lambda's, a module's and the rest are refused there
                if !on_class(o, nm) {
                    if !ANN_DEFS.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(err(line, ".__annotations__"));
                    }
                } else if nm.odd_ann {
                    return Err(err(line, ".__annotations__ of a class annotated with a type expression (such as int | None or list[int])"));
                }
            }
            // `Movie.__annotations__`, `T.__name__`
            if nm.typing_expr(o) {
                return Err(err(line, &format!(".{a} of a typing form")));
            }
            go(o)
        }
        Ex::Index(o, _) if matches!(o.as_ref(), Ex::Name(n) if nm.on_alias.contains(n)) => Err(err(line, "subscripting a class built on a subscripted generic class")),
        Ex::Index(o, i) => {
            if nm.typing_expr(o) && !nm.generic_class(o) {
                // typing.List[int], typing.Optional[X], ...: values the runtime makes (rt_typing.rs TAlias, TUnion)
                if !ok && value_form(e, nm) {
                    return expr(i, true, line, nm);
                }
                if !ok {
                    return Err(err(line, "a subscripted typing form used as a value"));
                }
                expr(o, true, line, nm)?;
                return expr(i, true, line, nm);
            }
            if nm.generic_class(o) && !ok {
                // `Stack[int]` as a value is a typing alias, not the class (`Stack[int]()` is checked at the call)
                return Err(err(line, "a subscripted generic class used as a value"));
            }
            if nm.generic_class(o) {
                return expr(i, true, line, nm);
            }
            go(o)?;
            go(i)
        }
        Ex::Bin("|", a, b) if nm.typing_expr(e) => {
            if !ok && union_safe(e, nm) {
                expr(a, true, line, nm)?;
                return expr(b, true, line, nm);
            }
            if !ok {
                return Err(err(line, "a typing union used as a value"));
            }
            expr(a, true, line, nm)?;
            expr(b, true, line, nm)
        }
        Ex::Call(f, args) => {
            let all = |ok: bool| -> Result<(), Er> {
                for a in args {
                    match a {
                        Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => expr(x, ok, line, nm)?,
                    }
                }
                Ok(())
            };
            if let Some((m, n)) = nm.form(f) {
                if !m.is_empty() && !module_has(&m, &n) {
                    return Err(err(line, &format!("{m}.{n}")));
                }
                return match n.as_str() {
                    "TypeVar" | "ParamSpec" | "TypeVarTuple" | "NewType" | "assert_type" => all(true),
                    "NamedTuple" if args.len() == 2 && args.iter().all(|a| matches!(a, Arg::Pos(_))) => all(true),
                    "cast" if args.len() == 2 && args.iter().all(|a| matches!(a, Arg::Pos(_))) => {
                        if let (Arg::Pos(t), Arg::Pos(v)) = (&args[0], &args[1]) {
                            expr(t, true, line, nm)?;
                            go(v)?;
                        }
                        Ok(())
                    }
                    "assert_never" | "reveal_type" if args.len() == 1 => all(false),
                    "@newtype" if args.len() == 1 && matches!(args[0], Arg::Pos(_)) => all(false),
                    _ if m.is_empty() => Err(err(line, "calling a typing alias")),
                    _ => Err(err(line, &format!("{m}.{n}(...)"))),
                };
            }
            if matches!(f.as_ref(), Ex::Name(n) if nm.tdicts.contains(n)) {
                return all(false);
            }
            // Brevi's pydantic: TypeAdapter's type is an annotation (src/dynpy/pyd.rs makes it a descriptor)
            if OWN_PYDANTIC.load(std::sync::atomic::Ordering::Relaxed) && matches!(f.as_ref(), Ex::Name(n) if n == "TypeAdapter") {
                if let Some(Arg::Pos(t)) = args.first() {
                    expr(t, true, line, nm)?;
                    for a in &args[1..] {
                        match a {
                            Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => go(x)?,
                        }
                    }
                    return Ok(());
                }
            }
            // and anthropic's output_format=T (Brevi's anthropic hands T to TypeAdapter)
            if OWN_PYDANTIC.load(std::sync::atomic::Ordering::Relaxed)
                && matches!(f.as_ref(), Ex::Attr(_, m) if matches!(m.as_str(), "parse" | "stream" | "count_tokens"))
                && args.iter().any(|a| matches!(a, Arg::Kw(k, _) if k == "output_format"))
            {
                go(f)?;
                for a in args {
                    match a {
                        Arg::Kw(k, x) if k == "output_format" => expr(x, true, line, nm)?,
                        Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => go(x)?,
                    }
                }
                return Ok(());
            }
            // `Stack[int]()`
            if let Ex::Index(o, i) = f.as_ref() {
                if matches!(o.as_ref(), Ex::Name(n) if nm.on_alias.contains(n)) {
                    return Err(err(line, "subscripting a class built on a subscripted generic class"));
                }
                if nm.generic_class(o) {
                    expr(i, true, line, nm)?;
                    return all(false);
                }
            }
            // isinstance(x, TypeVar) (or TypeVarTuple, ParamSpec): the runtime answers it (rt_lib.rs isinstance)
            if matches!(f.as_ref(), Ex::Name(n) if n == "isinstance") && args.len() == 2 {
                if let (Arg::Pos(x), Arg::Pos(t)) = (&args[0], &args[1]) {
                    if matches!(nm.form(t), Some((m, n)) if !m.is_empty() && matches!(n.as_str(), "TypeVar" | "ParamSpec" | "TypeVarTuple")) {
                        return go(x);
                    }
                }
            }
            // isinstance(x, P) for a Protocol class P: structural in CPython (or a TypeError); a class pattern
            // `case P():` is the same check
            if matches!(f.as_ref(), Ex::Name(n) if n == "isinstance" || n == "issubclass" || n == "__brevi_pm_cls") {
                if let Some(Arg::Pos(t)) = args.get(1) {
                    let protocol = |x: &Ex| matches!(x, Ex::Name(n) if nm.protocols.contains(n));
                    if protocol(t) || matches!(t, Ex::Tuple(v) if v.iter().any(protocol)) {
                        return Err(err(line, "isinstance with a Protocol class"));
                    }
                }
            }
            go(f)?;
            all(false)
        }
        Ex::FStr(parts) => fparts(parts, line, nm),
        Ex::Lambda(p, b) => {
            params(p, line, nm)?;
            go(b)
        }
        Ex::Comp(_, elt, val, fors) => {
            go(elt)?;
            if let Some(v) = val {
                go(v)?;
            }
            for c in fors {
                go(&c.target)?;
                go(&c.iter)?;
                c.ifs.iter().try_for_each(|x| go(x))?;
            }
            Ok(())
        }
        Ex::List(v) | Ex::Tuple(v) => v.iter().try_for_each(|x| expr(x, ok, line, nm)),
        Ex::Set(v) => v.iter().try_for_each(|x| go(x)),
        Ex::Dict(items) => {
            for (k, v) in items {
                if let Some(k) = k {
                    go(k)?;
                }
                go(v)?;
            }
            Ok(())
        }
        Ex::Star(x) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => go(x),
        Ex::Yield(Some(x)) => go(x),
        Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            go(a)?;
            go(b)
        }
        Ex::Slice(a, b, c) => [a, b, c].into_iter().flatten().try_for_each(|x| go(x)),
        Ex::Cmp(a, rest) => {
            go(a)?;
            rest.iter().try_for_each(|(_, x)| go(x))
        }
        Ex::IfExp(a, b, c) => {
            go(a)?;
            go(b)?;
            go(c)
        }
        _ => Ok(()),
    }
}

fn fparts(parts: &[FPart], line: usize, nm: &Names) -> Result<(), Er> {
    for p in parts {
        if let FPart::Expr(e, _, spec, _) = p {
            expr(e, false, line, nm)?;
            fparts(spec, line, nm)?;
        }
    }
    Ok(())
}

// Whether a PEP 695 def (`def f[T](...)`) uses one of its type parameters' names anywhere CPython would look
// them up in the def's annotation scope: its defaults, its body (nested defs, lambdas and classes too) and, when
// `anns` (annotations are evaluated: ANN_DEFS), its annotations. Brevi makes the TypeVar objects for
// __type_params__ but does not bind the names, so such a def is refused.
pub fn tparams_used(d: &super::parse::FuncDef, names: &[String], anns: bool) -> bool {
    let mut hit = false;
    let mut look = |e: &Ex| {
        crate::typed_guard::walk_ex(e, &mut |x| match x {
            Ex::Name(n) => hit |= names.contains(n),
            Ex::Lambda(ps, _) => {
                for (_, dv) in ps.pos.iter().chain(ps.kwonly.iter()) {
                    if let Some(dv) = dv {
                        crate::typed_guard::walk_ex(dv, &mut |y| hit |= matches!(y, Ex::Name(n) if names.contains(n)));
                    }
                }
            }
            _ => {}
        })
    };
    fn params_of(p: &Params, ret: &Option<Ex>, anns: bool, look: &mut dyn FnMut(&Ex)) {
        for (_, dv) in p.pos.iter().chain(p.kwonly.iter()) {
            if let Some(dv) = dv {
                look(dv);
            }
        }
        if anns {
            p.ann.iter().for_each(|(_, a)| look(a));
            if let Some(r) = ret {
                look(r);
            }
        }
    }
    fn nested(body: &[Stmt], anns: bool, look: &mut dyn FnMut(&Ex)) {
        for s in body {
            if let St::Def(d) = &s.st {
                params_of(&d.params, &d.ret, anns, look);
            }
            if let St::AnnAssign(_, _, a, _) = &s.st {
                if anns {
                    look(a);
                }
            }
            blocks(&s.st, &mut |b| nested(b, anns, look));
        }
    }
    params_of(&d.params, &d.ret, anns, &mut look);
    nested(&d.body, anns, &mut look);
    walk_all(&d.body, &mut |e, _| look(e));
    hit
}

// Whether a def or class inside `body` (at any depth) has an annotation CPython evaluates that names one of
// `names` (PEP 695 parameters of an enclosing def, which its body does not bind here). A function body's own
// annotated statements are never evaluated; a class body's are.
pub fn tparams_in_nested_anns(body: &[Stmt], names: &[String]) -> bool {
    fn names_one(e: &Ex, names: &[String]) -> bool {
        let mut hit = false;
        crate::typed_guard::walk_ex(e, &mut |x| hit |= matches!(x, Ex::Name(n) if names.contains(n)));
        hit
    }
    fn go(body: &[Stmt], in_class: bool, names: &[String]) -> bool {
        body.iter().any(|s| match &s.st {
            St::Def(d) => d.params.ann.iter().any(|(_, a)| names_one(a, names)) || d.ret.as_ref().is_some_and(|r| names_one(r, names)) || go(&d.body, false, names),
            St::Class(c) => go(&c.body, true, names),
            St::AnnAssign(_, _, a, _) if in_class => names_one(a, names),
            st => {
                let mut hit = false;
                blocks(st, &mut |b| hit |= go(b, in_class, names));
                hit
            }
        })
    }
    go(body, false, names)
}

// The first use of exec() or compile(), or of eval() on anything but a string literal, in modules that do not bind
// that name: a program that runs a string as code, which the dynamic build refuses on purpose (emit.rs). When the
// typing check refuses such a program too, this is the refusal reported: the program could not be built either way,
// and it is the one that holds by design.
pub fn string_as_code(mods: &[(&str, &[Stmt])]) -> Option<(String, usize, String)> {
    fn binds(body: &[Stmt], n: &str) -> bool {
        body.iter().any(|s| {
            let here = match &s.st {
                St::Def(d) => d.name == n,
                St::Class(c) => c.name == n,
                St::Assign(ts, _) => ts.iter().any(|t| matches!(t, Ex::Name(x) if x == n)),
                St::Import(v) => v.iter().any(|(m, a)| a.as_deref().unwrap_or(m.split('.').next().unwrap_or("")) == n),
                St::ImportFrom(_, v) => v.iter().any(|(m, a)| a.as_deref().unwrap_or(m) == n || m == "*"),
                St::Global(_) => false,
                _ => false,
            };
            let mut inner = false;
            blocks(&s.st, &mut |b| inner |= binds(b, n));
            here || inner
        })
    }
    for (m, body) in mods {
        let mut found: Option<(usize, String)> = None;
        walk_all(body, &mut |e, line| {
            if found.is_some() {
                return;
            }
            match e {
                Ex::Name(n) if (n == "exec" || n == "compile") && !binds(body, n) => found = Some((line, n.clone())),
                Ex::Call(f, args) if matches!(f.as_ref(), Ex::Name(n) if n == "eval") && !matches!(args.as_slice(), [Arg::Pos(Ex::Str(_))]) && !binds(body, "eval") => {
                    found = Some((line, "eval".into()))
                }
                _ => {}
            }
        });
        if let Some((l, n)) = found {
            return Some((m.to_string(), l, format!("{n}() is not supported in dynamic builds")));
        }
    }
    None
}

// A subscripted typing form the runtime makes as a value, with CPython's repr, == and isinstance (rt_typing.rs).
fn value_form(e: &Ex, nm: &Names) -> bool {
    let Ex::Index(o, _) = e else { return false };
    matches!(nm.form(o), Some((m, n)) if typing_mod(&m) && matches!(n.as_str(), "List" | "Dict" | "Set" | "FrozenSet" | "Tuple" | "Type" | "Union" | "Optional"))
}
// `a | b` whose sides are plain values, such typing values, or unions of them (a bare typing name is not)
fn union_safe(e: &Ex, nm: &Names) -> bool {
    match e {
        Ex::Bin("|", a, b) => union_safe(a, nm) && union_safe(b, nm),
        _ => !nm.typing_expr(e) || value_form(e, nm),
    }
}

// A class built on a datetime class (`class D(date)`, `class E(date, Enum)`), or a functional Enum with
// `type=date`: the dynamic build's date classes cannot be subclassed (their instances hold no date), so such a
// program is refused at build time: (line, the class's dotted name).
// A class statement subclassing set or frozenset in a program that imports pickle: the line and the base.
// Pickling such an instance would hand it to CPython's pickle, which cannot take it, so the build refuses.
pub fn pickled_set_subclass(body: &[Stmt]) -> Option<(usize, String)> {
    let mut pickle = false;
    fn scan(body: &[Stmt], pickle: &mut bool) {
        for s in body {
            match &s.st {
                St::ImportFrom(m, _) if m == "pickle" || m.starts_with("pickle.") => *pickle = true,
                St::Import(v) if v.iter().any(|(m, _)| m == "pickle" || m.starts_with("pickle.")) => *pickle = true,
                _ => {}
            }
            blocks(&s.st, &mut |b| scan(b, pickle));
        }
    }
    scan(body, &mut pickle);
    if !pickle {
        return None;
    }
    let mut found: Option<(usize, String)> = None;
    fn classes(body: &[Stmt], f: &mut dyn FnMut(&[Ex], usize)) {
        for s in body {
            if let St::Class(c) = &s.st {
                f(&c.bases, s.line);
            }
            blocks(&s.st, &mut |b| classes(b, f));
        }
    }
    classes(body, &mut |bases, line| {
        if found.is_none() {
            if let Some(n) = bases.iter().find_map(|b| match b {
                Ex::Name(n) if n == "set" || n == "frozenset" => Some(n.clone()),
                _ => None,
            }) {
                found = Some((line, n));
            }
        }
    });
    found
}
pub fn datetime_subclass(body: &[Stmt]) -> Option<(usize, String)> {
    const DT: [&str; 5] = ["date", "datetime", "time", "timedelta", "timezone"];
    let mut names: HashMap<String, String> = HashMap::new();
    let mut mods: Vec<String> = vec![];
    fn scan(body: &[Stmt], names: &mut HashMap<String, String>, mods: &mut Vec<String>) {
        for s in body {
            match &s.st {
                St::ImportFrom(m, v) if m == "datetime" => {
                    for (n, a) in v {
                        if DT.contains(&n.as_str()) {
                            names.insert(a.clone().unwrap_or_else(|| n.clone()), n.clone());
                        }
                    }
                }
                St::Import(v) => {
                    for (m, a) in v {
                        if m == "datetime" {
                            mods.push(a.clone().unwrap_or_else(|| m.clone()));
                        }
                    }
                }
                _ => {}
            }
            blocks(&s.st, &mut |b| scan(b, names, mods));
        }
    }
    scan(body, &mut names, &mut mods);
    if names.is_empty() && mods.is_empty() {
        return None;
    }
    let dt_of = |e: &Ex| -> Option<String> {
        match e {
            Ex::Name(n) => names.get(n).cloned(),
            Ex::Attr(o, a) if DT.contains(&a.as_str()) && matches!(o.as_ref(), Ex::Name(m) if mods.contains(m)) => Some(a.clone()),
            _ => None,
        }
    };
    let mut found: Option<(usize, String)> = None;
    fn classes(body: &[Stmt], f: &mut dyn FnMut(&[Ex], usize)) {
        for s in body {
            if let St::Class(c) = &s.st {
                f(&c.bases, s.line);
            }
            blocks(&s.st, &mut |b| classes(b, f));
        }
    }
    classes(body, &mut |bases, line| {
        if found.is_none() {
            if let Some(n) = bases.iter().find_map(|b| dt_of(b)) {
                found = Some((line, n));
            }
        }
    });
    if found.is_none() {
        walk_all(body, &mut |e, line| {
            if let Ex::Call(_, args) = e {
                for a in args {
                    if let Arg::Kw(k, v) = a {
                        if k == "type" && found.is_none() {
                            if let Some(n) = dt_of(v) {
                                found = Some((line, n));
                            }
                        }
                    }
                }
            }
        });
    }
    found.map(|(l, n)| (l, format!("datetime.{n}")))
}
