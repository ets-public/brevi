// Brevi's pydantic (src/dynpy/py/pydantic.py), the compiler's side.
//
// pydantic reads a model's annotations as typing objects, which Brevi does not build. In a program that imports
// Brevi's pydantic, each class body gets one more statement instead: `__annotations__ = ((name, descriptor),
// ...)`, where a descriptor is a tuple the copy reads, such as ('list', ('int',), 'list', False) for list[int]
// or ('union', (('int',), ('none',))) for Optional[int]. Class names stay names (evaluated where CPython
// evaluates the annotation); in a string annotation, or with `from __future__ import annotations`, they become
// `lambda: Name`, looked up when the model is first used. Module-level aliases (`Tags = list[str]`) become
// descriptors too.
//
// check() refuses, when the program is built, a model with an annotation the copy has no validator for, so
// CPython's pydantic runs that program instead.
use super::parse::{self, Arg, ClassDef, Ex, FuncDef, Params, St, Stmt};
use std::collections::HashSet;

fn tag(s: &str) -> Ex {
    Ex::Str(s.to_string())
}
fn tup(v: Vec<Ex>) -> Ex {
    Ex::Tuple(v)
}
fn boolean(b: bool) -> Ex {
    if b { Ex::True } else { Ex::False }
}

// typing names, with or without `typing.` / `t.` in front
fn typing_name(e: &Ex) -> Option<&str> {
    match e {
        Ex::Name(n) => Some(n.as_str()),
        Ex::Attr(o, n) if matches!(o.as_ref(), Ex::Name(m) if matches!(m.as_str(), "typing" | "typing_extensions" | "t" | "tp" | "pydantic" | "pd")) => {
            Some(n.as_str())
        }
        _ => None,
    }
}

fn any() -> Ex {
    tup(vec![tag("any"), tag("Any")])
}

// The items of a subscript: `X[a, b]` -> [a, b]
fn items(i: &Ex) -> Vec<&Ex> {
    match i {
        Ex::Tuple(v) => v.iter().collect(),
        e => vec![e],
    }
}

pub struct Conv<'a> {
    // names bound by `from typing import X` and friends are typing names; user names shadow nothing here
    pub lazy: bool,
    pub classes: &'a HashSet<String>,
    pub models: &'a HashSet<String>,
    // module-level aliases already rewritten to descriptors (`Even = Annotated[int, ...]`)
    pub aliases: HashSet<String>,
}

impl Conv<'_> {
    // The descriptor of annotation `e`, or why there is none.
    pub fn desc(&self, e: &Ex) -> Result<Ex, String> {
        self.desc_d(e, 0)
    }
    fn desc_d(&self, e: &Ex, depth: usize) -> Result<Ex, String> {
        if depth > 40 {
            return Err("annotation nested too deep".into());
        }
        let d = |x: &Ex| self.desc_d(x, depth + 1);
        match e {
            Ex::NoneV => return Ok(tup(vec![tag("none")])),
            Ex::Str(s) => {
                let parsed = parse::parse(&format!("{}\n", s.trim()))
                    .map_err(|_| format!("annotation {s:?} is not an expression"))?;
                let ex = match parsed.as_slice() {
                    [
                        Stmt {
                            st: St::Expr(x), ..
                        },
                    ] => x.clone(),
                    _ => return Err(format!("annotation {s:?} is not an expression")),
                };
                let c = Conv {
                    lazy: true,
                    classes: self.classes,
                    models: self.models,
                    aliases: self.aliases.clone(),
                };
                return c.desc_d(&ex, depth + 1);
            }
            Ex::Bin("|", a, b) => {
                let mut out = vec![];
                for x in [a.as_ref(), b.as_ref()] {
                    let dx = d(x)?;
                    flatten(dx, &mut out);
                }
                return Ok(tup(vec![tag("union"), tup(out)]));
            }
            Ex::Index(head, i) => {
                let Some(h) = typing_name(head) else {
                    return Err("a subscripted annotation that is not a typing form".into());
                };
                let args = items(i);
                let one = |what: &str| -> Result<Ex, String> {
                    if args.len() != 1 {
                        return Err(format!("{what}[...] takes one type"));
                    }
                    d(args[0])
                };
                return Ok(match h {
                    "Optional" => {
                        let mut out = vec![];
                        flatten(one("Optional")?, &mut out);
                        out.push(tup(vec![tag("none")]));
                        tup(vec![tag("union"), tup(out)])
                    }
                    "Union" => {
                        let mut out = vec![];
                        for a in &args {
                            flatten(d(a)?, &mut out);
                        }
                        tup(vec![tag("union"), tup(out)])
                    }
                    "list" | "List" => tup(vec![tag("list"), one(h)?, tag(h), boolean(false)]),
                    "set" | "Set" => tup(vec![tag("set"), one(h)?, tag(h), boolean(false)]),
                    "frozenset" | "FrozenSet" => {
                        tup(vec![tag("frozenset"), one(h)?, tag(h), boolean(false)])
                    }
                    "dict" | "Dict" => {
                        if args.len() != 2 {
                            return Err(format!("{h}[...] takes two types"));
                        }
                        tup(vec![
                            tag("dict"),
                            d(args[0])?,
                            d(args[1])?,
                            tag(h),
                            boolean(false),
                        ])
                    }
                    "tuple" | "Tuple" => {
                        if args.len() == 2 && matches!(args[1], Ex::Ellipsis) {
                            tup(vec![tag("tuplevar"), d(args[0])?, tag(h), boolean(false)])
                        } else if args.len() == 1 && matches!(args[0], Ex::Tuple(v) if v.is_empty())
                        {
                            tup(vec![tag("tuple"), tup(vec![]), tag(h)])
                        } else {
                            let mut v = vec![];
                            for a in &args {
                                if matches!(a, Ex::Ellipsis) {
                                    return Err("`...` in a tuple type".into());
                                }
                                v.push(d(a)?);
                            }
                            tup(vec![tag("tuple"), tup(v), tag(h)])
                        }
                    }
                    "Literal" => {
                        let mut v = vec![];
                        for a in &args {
                            if !literal_ok(a) {
                                return Err(
                                    "a Literal value Brevi's pydantic does not compare".into()
                                );
                            }
                            v.push((*a).clone());
                        }
                        tup(vec![tag("literal"), tup(v)])
                    }
                    "Annotated" => {
                        if args.len() < 2 {
                            return Err("Annotated[...] needs metadata".into());
                        }
                        let mut meta = vec![];
                        for a in &args[1..] {
                            if !meta_ok(a) {
                                return Err("Annotated metadata other than Field(...), Strict() and the functional validators".into());
                            }
                            meta.push((*a).clone());
                        }
                        tup(vec![tag("ann"), d(args[0])?, tup(meta)])
                    }
                    "ClassVar" => tup(vec![tag("classvar")]),
                    _ => return Err(format!("{h}[...] in a model field")),
                });
            }
            _ => {}
        }
        let Some(n) = typing_name(e) else {
            // `module.Class`
            if let Ex::Attr(o, n) = e {
                if matches!(o.as_ref(), Ex::Name(_)) && self.classes.contains(n) {
                    return Ok(self.cls(e, n));
                }
            }
            return Err("an annotation Brevi's pydantic does not read".into());
        };
        // an alias is its descriptor at run time
        if self.aliases.contains(n) && matches!(e, Ex::Name(_)) {
            return Ok(e.clone());
        }
        // a user class shadows a builtin name
        if self.classes.contains(n) {
            return Ok(self.cls(e, n));
        }
        Ok(match n {
            "int" | "float" | "str" | "bool" | "bytes" => tup(vec![tag(n)]),
            "None" | "NoneType" => tup(vec![tag("none")]),
            "Any" => any(),
            "object" => tup(vec![tag("any"), tag("object")]),
            "list" | "List" => tup(vec![tag("list"), any(), tag(n), boolean(true)]),
            "set" | "Set" => tup(vec![tag("set"), any(), tag(n), boolean(true)]),
            "frozenset" | "FrozenSet" => tup(vec![tag("frozenset"), any(), tag(n), boolean(true)]),
            "dict" | "Dict" => tup(vec![tag("dict"), any(), any(), tag(n), boolean(true)]),
            "tuple" | "Tuple" => tup(vec![tag("tuplevar"), any(), tag(n), boolean(true)]),
            "ClassVar" => tup(vec![tag("classvar")]),
            // pydantic's constrained types: descriptors in the copy
            "PositiveInt" | "NegativeInt" | "NonNegativeInt" | "NonPositiveInt"
            | "PositiveFloat" | "NegativeFloat" | "NonNegativeFloat" | "NonPositiveFloat"
            | "FiniteFloat" | "StrictInt" | "StrictFloat" | "StrictStr" | "StrictBool"
            | "StrictBytes" => tup(vec![tag("cls"), e.clone()]),
            _ => return Err(format!("field type {n}")),
        })
    }
    fn cls(&self, e: &Ex, n: &str) -> Ex {
        if self.lazy {
            tup(vec![
                tag("lazy"),
                Ex::Lambda(Box::new(Params::default()), Box::new(e.clone())),
                tag(n),
            ])
        } else {
            tup(vec![tag("cls"), e.clone()])
        }
    }
}

fn flatten(d: Ex, out: &mut Vec<Ex>) {
    if let Ex::Tuple(v) = &d {
        if matches!(v.first(), Some(Ex::Str(t)) if t == "union") {
            if let Some(Ex::Tuple(m)) = v.get(1) {
                out.extend(m.iter().cloned());
                return;
            }
        }
    }
    out.push(d);
}

fn literal_ok(a: &Ex) -> bool {
    match a {
        Ex::Str(_) | Ex::Int(_) | Ex::True | Ex::False | Ex::NoneV => true,
        Ex::Unary("-", x) => matches!(x.as_ref(), Ex::Int(_)),
        // an enum member
        Ex::Attr(o, _) => matches!(o.as_ref(), Ex::Name(_)),
        _ => false,
    }
}

const META_CALLS: &[&str] = &[
    "Field",
    "Strict",
    "AfterValidator",
    "BeforeValidator",
    "PlainValidator",
    "WrapValidator",
];

fn meta_ok(a: &Ex) -> bool {
    match a {
        Ex::Call(f, _) => typing_name(f).is_some_and(|n| META_CALLS.contains(&n)),
        _ => false,
    }
}

// Every class name of the program's own modules.
pub fn class_names(mods: &[(&str, &[Stmt])]) -> HashSet<String> {
    fn walk(b: &[Stmt], out: &mut HashSet<String>) {
        for s in b {
            match &s.st {
                St::Class(c) => {
                    out.insert(c.name.clone());
                    walk(&c.body, out);
                }
                St::Def(d) => walk(&d.body, out),
                St::If(arms, els) => {
                    for (_, b) in arms {
                        walk(b, out);
                    }
                    if let Some(b) = els {
                        walk(b, out);
                    }
                }
                St::Try(b, hs, e, f) => {
                    walk(b, out);
                    for h in hs {
                        walk(&h.body, out);
                    }
                    for x in [e, f].into_iter().flatten() {
                        walk(x, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = HashSet::new();
    for (_, b) in mods {
        walk(b, &mut out);
    }
    out
}

fn future_annotations(body: &[Stmt]) -> bool {
    body.iter().any(|s| matches!(&s.st, St::ImportFrom(m, v) if m == "__future__" && v.iter().any(|(n, _)| n == "annotations")))
}

// Whether a module-level value is a type alias the copy reads (`Tags = list[str]`, `Id = int | str`).
fn alias_like(v: &Ex) -> bool {
    match v {
        Ex::Index(h, _) => typing_name(h).is_some_and(|n| {
            matches!(
                n,
                "Optional"
                    | "Union"
                    | "list"
                    | "List"
                    | "set"
                    | "Set"
                    | "frozenset"
                    | "FrozenSet"
                    | "dict"
                    | "Dict"
                    | "tuple"
                    | "Tuple"
                    | "Literal"
                    | "Annotated"
            )
        }),
        Ex::Bin("|", a, b) => {
            alias_like(a)
                || alias_like(b)
                || matches!(a.as_ref(), Ex::Name(n) if matches!(n.as_str(), "int" | "str" | "float" | "bool" | "bytes"))
        }
        _ => false,
    }
}

// The rewrite: `__annotations__` descriptors for every class, aliases as descriptors. Models whose annotations
// have no descriptor are refused by check(); other classes just get a ('?', why) the copy never reads.
pub fn rewrite(body: &mut [Stmt], classes: &HashSet<String>, models: &HashSet<String>) {
    let lazy = future_annotations(body);
    let mut cv = Conv {
        lazy,
        classes,
        models,
        aliases: HashSet::new(),
    };
    for s in body.iter_mut() {
        if let St::Assign(ts, v) = &mut s.st {
            if let [Ex::Name(n)] = ts.as_slice() {
                if alias_like(v) {
                    if let Ok(d) = cv.desc(v) {
                        *v = d;
                        cv.aliases.insert(n.clone());
                    }
                }
            }
        }
    }
    walk(body, &cv);
    // TypeAdapter(T): T as a descriptor too (a type Brevi's pydantic lacks becomes ('?', why), refused by check)
    stmts_mut(body, &mut |e| {
        if let Ex::Call(f, args) = e {
            if typing_name(f) == Some("TypeAdapter") {
                if let Some(Arg::Pos(t)) = args.first_mut() {
                    let d = cv.desc(t).unwrap_or_else(|why| tup(vec![tag("?"), Ex::Str(why)]));
                    *t = d;
                }
            }
        }
    });
}

// Every expression of the statements (nested blocks, defs and classes too), children first.
fn stmts_mut(body: &mut [Stmt], f: &mut dyn FnMut(&mut Ex)) {
    for s in body.iter_mut() {
        match &mut s.st {
            St::Expr(e) => ex_mut(e, f),
            St::Assign(ts, v) => {
                ts.iter_mut().for_each(|t| ex_mut(t, f));
                ex_mut(v, f);
            }
            St::AugAssign(t, _, v) => {
                ex_mut(t, f);
                ex_mut(v, f);
            }
            St::AnnAssign(t, v, _) => {
                ex_mut(t, f);
                if let Some(v) = v {
                    ex_mut(v, f);
                }
            }
            St::Return(Some(e)) => ex_mut(e, f),
            St::Raise(a, b) => {
                for x in [a, b].into_iter().flatten() {
                    ex_mut(x, f);
                }
            }
            St::Del(v) => v.iter_mut().for_each(|x| ex_mut(x, f)),
            St::Assert(a, b) => {
                ex_mut(a, f);
                if let Some(b) = b {
                    ex_mut(b, f);
                }
            }
            St::If(arms, els) => {
                for (c, b) in arms.iter_mut() {
                    ex_mut(c, f);
                    stmts_mut(b, f);
                }
                if let Some(b) = els {
                    stmts_mut(b, f);
                }
            }
            St::While(c, b, e) => {
                ex_mut(c, f);
                stmts_mut(b, f);
                if let Some(e) = e {
                    stmts_mut(e, f);
                }
            }
            St::For(t, it, b, e) => {
                ex_mut(t, f);
                ex_mut(it, f);
                stmts_mut(b, f);
                if let Some(e) = e {
                    stmts_mut(e, f);
                }
            }
            St::Try(b, hs, e, fin) => {
                stmts_mut(b, f);
                for h in hs.iter_mut() {
                    stmts_mut(&mut h.body, f);
                }
                for x in [e, fin].into_iter().flatten() {
                    stmts_mut(x, f);
                }
            }
            St::With(items, b) => {
                for (c, t) in items.iter_mut() {
                    ex_mut(c, f);
                    if let Some(t) = t {
                        ex_mut(t, f);
                    }
                }
                stmts_mut(b, f);
            }
            St::Def(d) => {
                d.decorators.iter_mut().for_each(|x| ex_mut(x, f));
                for (_, dv) in d.params.pos.iter_mut().chain(d.params.kwonly.iter_mut()) {
                    if let Some(dv) = dv {
                        ex_mut(dv, f);
                    }
                }
                stmts_mut(&mut d.body, f);
            }
            St::Class(c) => {
                c.decorators.iter_mut().for_each(|x| ex_mut(x, f));
                stmts_mut(&mut c.body, f);
            }
            _ => {}
        }
    }
}

fn ex_mut(e: &mut Ex, f: &mut dyn FnMut(&mut Ex)) {
    match e {
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter_mut().for_each(|x| ex_mut(x, f)),
        Ex::Dict(v) => v.iter_mut().for_each(|(k, x)| {
            if let Some(k) = k {
                ex_mut(k, f);
            }
            ex_mut(x, f)
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => ex_mut(x, f),
        Ex::Yield(Some(x)) => ex_mut(x, f),
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            ex_mut(a, f);
            ex_mut(b, f)
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                ex_mut(x, f);
            }
        }
        Ex::Call(c, args) => {
            ex_mut(c, f);
            for a in args.iter_mut() {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => ex_mut(x, f),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            ex_mut(a, f);
            rest.iter_mut().for_each(|(_, x)| ex_mut(x, f))
        }
        Ex::IfExp(a, b, c) => {
            ex_mut(a, f);
            ex_mut(b, f);
            ex_mut(c, f)
        }
        Ex::Comp(_, a, b, gens) => {
            ex_mut(a, f);
            if let Some(b) = b {
                ex_mut(b, f);
            }
            for g in gens.iter_mut() {
                ex_mut(&mut g.target, f);
                ex_mut(&mut g.iter, f);
                g.ifs.iter_mut().for_each(|x| ex_mut(x, f));
            }
        }
        Ex::Lambda(p, b) => {
            for (_, d) in p.pos.iter_mut().chain(p.kwonly.iter_mut()) {
                if let Some(d) = d {
                    ex_mut(d, f);
                }
            }
            ex_mut(b, f);
        }
        _ => {}
    }
    f(e);
}

fn walk(body: &mut [Stmt], cv: &Conv) {
    for s in body.iter_mut() {
        match &mut s.st {
            St::Class(c) => class(c, cv),
            St::Def(d) => walk(&mut d.body, cv),
            St::If(arms, els) => {
                for (_, b) in arms.iter_mut() {
                    walk(b, cv);
                }
                if let Some(b) = els {
                    walk(b, cv);
                }
            }
            St::Try(b, hs, e, f) => {
                walk(b, cv);
                for h in hs.iter_mut() {
                    walk(&mut h.body, cv);
                }
                if let Some(x) = e {
                    walk(x, cv);
                }
                if let Some(x) = f {
                    walk(x, cv);
                }
            }
            _ => {}
        }
    }
}

fn class(c: &mut ClassDef, cv: &Conv) {
    walk(&mut c.body, cv);
    if !cv.models.contains(&c.name) {
        return;
    }
    let mut pairs = vec![];
    for s in &c.body {
        if let St::AnnAssign(Ex::Name(n), _, ann) = &s.st {
            let d = cv
                .desc(ann)
                .unwrap_or_else(|why| tup(vec![tag("?"), Ex::Str(why)]));
            pairs.push(tup(vec![Ex::Str(n.clone()), d]));
        }
    }
    if pairs.is_empty() {
        return;
    }
    c.body.push(Stmt {
        line: c.line,
        st: St::Assign(vec![Ex::Name("__annotations__".into())], tup(pairs)),
    });
}

// The models of the program (classes with BaseModel, or another model, among their bases), each annotation
// checked. Err(module, line, why).
fn all_classes<'a>(mods: &[(&'a str, &'a [Stmt])]) -> Vec<(&'a str, &'a ClassDef)> {
    let mut all: Vec<(&str, &ClassDef)> = vec![];
    fn collect<'a>(m: &'a str, b: &'a [Stmt], out: &mut Vec<(&'a str, &'a ClassDef)>) {
        for s in b {
            match &s.st {
                St::Class(c) => {
                    out.push((m, c));
                    collect(m, &c.body, out);
                }
                St::Def(d) => collect(m, &d.body, out),
                St::If(arms, els) => {
                    for (_, b) in arms {
                        collect(m, b, out);
                    }
                    if let Some(b) = els {
                        collect(m, b, out);
                    }
                }
                St::Try(b, hs, e, f) => {
                    collect(m, b, out);
                    for h in hs {
                        collect(m, &h.body, out);
                    }
                    for x in [e, f].into_iter().flatten() {
                        collect(m, x, out);
                    }
                }
                _ => {}
            }
        }
    }
    for (m, b) in mods {
        collect(m, b, &mut all);
    }
    all
}

fn base_name(b: &Ex) -> Option<String> {
    match b {
        Ex::Name(n) => Some(n.clone()),
        Ex::Attr(_, n) => Some(n.clone()),
        _ => None,
    }
}

// The names of the program's model classes: BaseModel, or another model, among their bases.
pub fn models(mods: &[(&str, &[Stmt])]) -> HashSet<String> {
    let all = all_classes(mods);
    let mut models: HashSet<String> = HashSet::new();
    loop {
        let before = models.len();
        for (_, c) in &all {
            if c.bases
                .iter()
                .any(|b| base_name(b).is_some_and(|n| n == "BaseModel" || models.contains(&n)))
            {
                models.insert(c.name.clone());
            }
        }
        if models.len() == before {
            break;
        }
    }
    models
}

// Every model's annotations and decorators checked (after rewrite). Err(module, line, why).
pub fn check(mods: &[(&str, &[Stmt])]) -> Result<(), (String, usize, String)> {
    for (m, body) in mods {
        for s in body.iter() {
            let mut why = None;
            super::emit::visit_any(std::slice::from_ref(s), &mut |e| {
                if let Ex::Call(f, args) = e {
                    if typing_name(f) == Some("TypeAdapter") {
                        if let Some(Arg::Pos(Ex::Tuple(d))) = args.first() {
                            if let (Some(Ex::Str(t)), Some(Ex::Str(w))) = (d.first(), d.get(1)) {
                                if t == "?" && why.is_none() {
                                    why = Some(w.clone());
                                }
                            }
                        }
                    }
                }
            });
            if let Some(w) = why {
                return Err((m.to_string(), s.line, format!("pydantic: {w} (a TypeAdapter type Brevi's pydantic does not have)")));
            }
        }
    }
    let models = models(mods);
    let all = all_classes(mods);
    for (m, c) in &all {
        if !models.contains(&c.name) {
            continue;
        }
        let refuse = |l: usize, why: String| {
            Err((
                m.to_string(),
                l,
                format!("pydantic: {why} (Brevi's pydantic does not have it)"),
            ))
        };
        for b in &c.bases {
            if matches!(b, Ex::Index(..)) {
                return refuse(c.line, format!("generic model {}", c.name));
            }
            if base_name(b).is_some_and(|n| n == "RootModel" || n == "Generic") {
                return refuse(
                    c.line,
                    format!("{} base of {}", base_name(b).unwrap(), c.name),
                );
            }
        }
        if !c.kwargs.is_empty() {
            return refuse(c.line, "class keyword arguments on a model".into());
        }
        for s in &c.body {
            match &s.st {
                St::Class(k) if k.name == "Config" => {
                    return refuse(s.line, "a class Config in a model".into());
                }
                St::Assign(ts, Ex::Tuple(pairs)) if matches!(ts.as_slice(), [Ex::Name(n)] if n == "__annotations__") => {
                    for p in pairs {
                        if let Ex::Tuple(kv) = p {
                            if let (Some(Ex::Str(name)), Some(Ex::Tuple(d))) =
                                (kv.first(), kv.get(1))
                            {
                                if let (Some(Ex::Str(t)), Some(Ex::Str(why))) =
                                    (d.first(), d.get(1))
                                {
                                    if t == "?" {
                                        let line = c.body.iter().find(|x| matches!(&x.st, St::AnnAssign(Ex::Name(n), _, _) if n == name)).map_or(c.line, |x| x.line);
                                        return refuse(
                                            line,
                                            format!("{why} (field {name} of {})", c.name),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                St::Def(d) => {
                    for dec in &d.decorators {
                        let f = match dec {
                            Ex::Call(f, _) => f.as_ref(),
                            f => f,
                        };
                        if let Some(n) = typing_name(f) {
                            if matches!(
                                n,
                                "validator"
                                    | "root_validator"
                                    | "field_serializer"
                                    | "model_serializer"
                            ) {
                                return refuse(s.line, format!("@{n}"));
                            }
                            if n == "model_validator" {
                                if let Ex::Call(_, args) = dec {
                                    let wrap = args.iter().any(|a| matches!(a, Arg::Kw(k, Ex::Str(v)) if k == "mode" && v == "wrap"));
                                    let _ = wrap;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- MCP tool signatures
//
// Brevi's mcp copy (py/_brevi_mcp.py) builds a tool's input and output schemas, and validates its arguments,
// from the function's signature, which the SDK reads with inspect and typing. Brevi keeps no annotations at
// run time, so every function of a module that imports mcp gets `f.__brevi_sig__ = (params, ret, has_ret,
// is_async, doc)` set right after its def: each parameter (name, descriptor or None, kind, has default,
// default), kind 0 positional, 1 *args, 2 keyword-only, 3 **kwargs; descriptors as for models. A function
// the mcp decorators (`@x.tool()`, `@x.resource(..)`, `@x.prompt()`) are on gets it before they run: the
// decorators become assignments after the def. A signature Brevi's copy cannot read on such a function (or
// on one handed to add_tool / from_function) refuses the build. Err(line, why).

const MCP_DECORATORS: &[&str] = &["tool", "resource", "prompt"];
const MCP_GENERIC: &[&str] = &["Context", "RequestContext", "ServerRequestContext"];

pub fn mcp_sigs(body: &mut Vec<Stmt>, classes: &HashSet<String>, models: &HashSet<String>, extra: &HashSet<String>) -> Result<(), (usize, String)> {
    let mut all: HashSet<String> = classes.clone();
    all.extend(extra.iter().cloned());
    let mut cv = Conv {
        lazy: future_annotations(body),
        classes: &all,
        models,
        aliases: HashSet::new(),
    };
    for s in body.iter() {
        if let St::Assign(ts, v) = &s.st {
            if let [Ex::Name(n)] = ts.as_slice() {
                if matches!(v, Ex::Tuple(t) if matches!(t.first(), Some(Ex::Str(_)))) {
                    cv.aliases.insert(n.clone());
                }
            }
        }
    }
    let mut handed = HashSet::new();
    handed_names(body, &mut handed);
    sig_walk(body, &cv, &handed)
}

// names of functions passed to add_tool(...) / X.from_function(...)
fn handed_names(body: &[Stmt], out: &mut HashSet<String>) {
    fn ex(e: &Ex, out: &mut HashSet<String>) {
        if let Ex::Call(f, args) = e {
            if let Ex::Attr(_, m) = f.as_ref() {
                if m == "add_tool" || m == "from_function" {
                    for a in args {
                        if let Some(Ex::Name(n)) = arg_value(a) {
                            out.insert(n.clone());
                            break;
                        }
                    }
                }
            }
            ex(f, out);
            for a in args {
                if let Some(v) = arg_value(a) {
                    ex(v, out);
                }
            }
        }
    }
    for s in body {
        match &s.st {
            St::Expr(e) | St::Assign(_, e) => ex(e, out),
            St::Def(d) => handed_names(&d.body, out),
            St::Class(c) => handed_names(&c.body, out),
            St::If(arms, els) => {
                for (c, b) in arms {
                    ex(c, out);
                    handed_names(b, out);
                }
                if let Some(b) = els {
                    handed_names(b, out);
                }
            }
            St::For(_, it, b, e) => {
                ex(it, out);
                handed_names(b, out);
                if let Some(b) = e {
                    handed_names(b, out);
                }
            }
            St::While(_, b, _) | St::With(_, b) => handed_names(b, out),
            St::Try(b, hs, e, f) => {
                handed_names(b, out);
                for h in hs {
                    handed_names(&h.body, out);
                }
                for x in [e, f].into_iter().flatten() {
                    handed_names(x, out);
                }
            }
            _ => {}
        }
    }
}

fn arg_value(a: &Arg) -> Option<&Ex> {
    match a {
        Arg::Pos(e) => Some(e),
        _ => None,
    }
}

fn is_mcp_decorator(d: &Ex) -> bool {
    if let Ex::Call(f, _) = d {
        if let Ex::Attr(_, m) = f.as_ref() {
            return MCP_DECORATORS.contains(&m.as_str());
        }
    }
    false
}

fn sig_walk(body: &mut Vec<Stmt>, cv: &Conv, handed: &HashSet<String>) -> Result<(), (usize, String)> {
    let mut i = 0;
    while i < body.len() {
        let line = body[i].line;
        let mut extra: Vec<Stmt> = vec![];
        match &mut body[i].st {
            St::Def(d) => {
                sig_walk(&mut d.body, cv, handed)?;
                let mcp = d.decorators.iter().any(is_mcp_decorator);
                let needed = mcp || handed.contains(&d.name);
                let sig = signature(d, cv);
                match sig {
                    Ok(s) => {
                        if d.decorators.is_empty() || mcp {
                            let target = Ex::Attr(Box::new(Ex::Name(d.name.clone())), "__brevi_sig__".into());
                            extra.push(Stmt { line, st: St::Assign(vec![target], s) });
                            if mcp {
                                let decs = std::mem::take(&mut d.decorators);
                                let mut v = Ex::Name(d.name.clone());
                                for dec in decs.into_iter().rev() {
                                    v = Ex::Call(Box::new(dec), vec![Arg::Pos(v)]);
                                }
                                extra.push(Stmt { line, st: St::Assign(vec![Ex::Name(d.name.clone())], v) });
                            }
                        } else if needed {
                            return Err((line, format!("mcp: {} is handed to the SDK but has other decorators", d.name)));
                        }
                    }
                    Err(why) => {
                        if needed {
                            return Err((line, format!("mcp: the signature of {}: {why}", d.name)));
                        }
                    }
                }
            }
            St::Class(c) => sig_walk(&mut c.body, cv, handed)?,
            St::If(arms, els) => {
                for (_, b) in arms.iter_mut() {
                    sig_walk(b, cv, handed)?;
                }
                if let Some(b) = els {
                    sig_walk(b, cv, handed)?;
                }
            }
            St::Try(b, hs, e, f) => {
                sig_walk(b, cv, handed)?;
                for h in hs.iter_mut() {
                    sig_walk(&mut h.body, cv, handed)?;
                }
                if let Some(x) = e {
                    sig_walk(x, cv, handed)?;
                }
                if let Some(x) = f {
                    sig_walk(x, cv, handed)?;
                }
            }
            St::With(_, b) | St::While(_, b, _) => sig_walk(b, cv, handed)?,
            St::For(_, _, b, _) => sig_walk(b, cv, handed)?,
            _ => {}
        }
        let k = extra.len();
        for (j, s) in extra.into_iter().enumerate() {
            body.insert(i + 1 + j, s);
        }
        i += 1 + k;
    }
    Ok(())
}

fn strip_generic(e: &Ex) -> Ex {
    match e {
        Ex::Index(h, i) => {
            let n = match h.as_ref() {
                Ex::Name(n) => Some(n.as_str()),
                Ex::Attr(_, n) => Some(n.as_str()),
                _ => None,
            };
            if n.is_some_and(|n| MCP_GENERIC.contains(&n)) {
                return (**h).clone();
            }
            Ex::Index(Box::new(strip_generic(h)), Box::new(strip_generic(i)))
        }
        Ex::Tuple(v) => Ex::Tuple(v.iter().map(strip_generic).collect()),
        Ex::Bin(op, a, b) => Ex::Bin(op, Box::new(strip_generic(a)), Box::new(strip_generic(b))),
        _ => e.clone(),
    }
}

fn signature(d: &FuncDef, cv: &Conv) -> Result<Ex, String> {
    let ann = |n: &str| -> Result<Ex, String> {
        match d.params.ann.iter().find(|(m, _)| m == n) {
            Some((_, a)) => cv.desc(&strip_generic(a)),
            None => Ok(Ex::NoneV),
        }
    };
    let mut ps = vec![];
    let param = |n: &str, kind: i64, dflt: &Option<Ex>| -> Result<Ex, String> {
        Ok(tup(vec![
            tag(n),
            ann(n)?,
            Ex::Int(kind.to_string()),
            boolean(dflt.is_some()),
            dflt.clone().unwrap_or(Ex::NoneV),
        ]))
    };
    for (n, dflt) in &d.params.pos {
        ps.push(param(n, 0, dflt)?);
    }
    if let Some(n) = &d.params.star {
        ps.push(param(n, 1, &None)?);
    }
    for (n, dflt) in &d.params.kwonly {
        ps.push(param(n, 2, dflt)?);
    }
    if let Some(n) = &d.params.kwstar {
        ps.push(param(n, 3, &None)?);
    }
    let ret = match &d.ret {
        Some(r) => cv.desc(&strip_generic(r))?,
        None => Ex::NoneV,
    };
    let doc = match d.body.first().map(|s| &s.st) {
        Some(St::Expr(Ex::Str(s))) => Ex::Str(clean_doc(s)),
        _ => Ex::NoneV,
    };
    Ok(tup(vec![tup(ps), ret, boolean(d.ret.is_some()), boolean(d.is_async), doc]))
}

// a docstring as CPython 3.13's compiler keeps it: tabs expanded, the first line's leading whitespace gone,
// the common indentation of the other lines removed, blank lines emptied
fn clean_doc(s: &str) -> String {
    let lines: Vec<String> = s.split('\n').map(expand_tabs).collect();
    let margin = lines
        .iter()
        .skip(1)
        .filter(|l| !l.trim_start_matches(' ').is_empty())
        .map(|l| l.len() - l.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    let mut out = vec![];
    for (i, l) in lines.iter().enumerate() {
        if i == 0 {
            out.push(l.trim_start_matches(' ').to_string());
        } else if l.trim_start_matches(' ').is_empty() {
            out.push(String::new());
        } else {
            out.push(l[margin..].to_string());
        }
    }
    out.join("\n")
}

fn expand_tabs(l: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for c in l.chars() {
        if c == '\t' {
            let n = 8 - col % 8;
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}
