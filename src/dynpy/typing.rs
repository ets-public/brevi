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

// The typing modules: their names are typing forms.
pub fn typing_mod(m: &str) -> bool {
    matches!(m, "typing" | "typing_extensions") || (m == "collections.abc" && super::emit::ABC_NATIVE.load(std::sync::atomic::Ordering::Relaxed))
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
    "TypeVar", "ParamSpec", "TypeVarTuple", "NewType", "Generic", "Protocol", "NamedTuple", "TypedDict", "TYPE_CHECKING", "cast",
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

pub fn check(mods: &[(&str, &[Stmt])]) -> Result<(), (String, usize, String)> {
    let mut names: HashMap<String, Names> = HashMap::new();
    // twice, for names a module imports from another module of the program
    for _ in 0..2 {
        for (m, body) in mods {
            let mut nm = names.get(*m).cloned().unwrap_or_default();
            collect(body, &mut nm, &names, true);
            names.insert(m.to_string(), nm);
        }
    }
    for (m, body) in mods {
        let nm = names.get_mut(*m).unwrap();
        class_names(body, &mut nm.classes);
        nm.lazy = future_annotations(body);
        nm.odd_ann = odd_class_annotation(body, false);
    }
    let reads = mods.iter().any(|(_, b)| {
        let mut r = false;
        walk_all(b, &mut |e, _| r |= matches!(e, Ex::Attr(_, a) if a == "__annotations__"));
        r
    });
    let defs = reads
        && !OWN_PYDANTIC.load(std::sync::atomic::Ordering::Relaxed)
        && mods.iter().all(|(m, b)| !names[*m].lazy && !odd_def_annotation(b) && !typing_def_annotation(b, &names[*m]));
    ANN_DEFS.store(defs, std::sync::atomic::Ordering::Relaxed);
    for (m, body) in mods {
        let nm = &names[*m];
        stmts(body, nm).map_err(|(l, w)| (m.to_string(), l, w))?;
    }
    // dataclasses.Field.type: the annotation, which Brevi does not keep
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
    if let (true, Some((m, l))) = (fields_used, type_at) {
        return Err((m, l, "dataclasses: a field's .type (Brevi does not keep annotations) needs CPython".into()));
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
            for x in c.decorators.iter().chain(c.bases.iter()) {
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
    collect(body, &mut nm, &HashMap::new(), true);
    let mut found = None;
    fn walk(body: &[Stmt], nm: &Names, found: &mut Option<usize>) {
        for s in body {
            crate::typed_guard::walk_st(&s.st, &mut |e| {
                if let Ex::Call(f, args) = e {
                    if matches!(f.as_ref(), Ex::Name(n) if n == "isinstance" || n == "issubclass") {
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

fn blocks(st: &St, f: &mut dyn FnMut(&[Stmt])) {
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
        St::AnnAssign(_, _, ann) if in_class => !plain(ann),
        St::Class(c) => odd_class_annotation(&c.body, true),
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

fn collect(body: &[Stmt], nm: &mut Names, all: &HashMap<String, Names>, top: bool) {
    for s in body {
        match &s.st {
            St::Import(v) => {
                for (m, a) in v {
                    if typing_mod(m) && a.is_some() {
                        nm.mods.insert(a.clone().unwrap(), m.clone());
                    } else if typing_mod(m) {
                        nm.mods.insert(m.split('.').next().unwrap().to_string(), if m == "collections.abc" { "collections".into() } else { m.clone() });
                    }
                }
            }
            St::ImportFrom(m, v) => {
                for (n, a) in v {
                    let bind = a.clone().unwrap_or_else(|| n.clone());
                    if typing_mod(m) {
                        nm.forms.insert(bind, (m.clone(), n.clone()));
                    } else if m == "collections" && n == "abc" {
                        nm.mods.insert(bind, "collections.abc".into());
                    } else if let Some(o) = all.get(m.as_str()) {
                        if let Some(f) = o.forms.get(n) {
                            nm.forms.insert(bind.clone(), f.clone());
                        }
                        for (from, to) in [(&o.generic, &mut nm.generic), (&o.protocols, &mut nm.protocols), (&o.tdicts, &mut nm.tdicts)] {
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
            St::AnnAssign(Ex::Name(n), Some(v), _) if top && nm.typing_expr(v) => {
                nm.forms.insert(n.clone(), (String::new(), String::new()));
            }
            St::Class(c) => {
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
        // `if TYPE_CHECKING:` imports, aliases in an `if` or `try`
        match &s.st {
            St::Def(_) | St::Class(_) => {}
            st => blocks(st, &mut |b| collect(b, nm, all, top)),
        }
    }
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
        St::AnnAssign(t, v, _) => {
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
            stmts(&d.body, nm)?;
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
            if (nm.forms.contains_key(n) || nm.tdicts.contains(n)) && !ok && !nm.is_form(e, "TYPE_CHECKING") {
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
                if !ok && n != "TYPE_CHECKING" {
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
        Ex::Index(o, i) => {
            if nm.typing_expr(o) && !nm.generic_class(o) {
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
                if nm.generic_class(o) {
                    expr(i, true, line, nm)?;
                    return all(false);
                }
            }
            // isinstance(x, P) for a Protocol class P: structural in CPython (or a TypeError)
            if matches!(f.as_ref(), Ex::Name(n) if n == "isinstance" || n == "issubclass") {
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
