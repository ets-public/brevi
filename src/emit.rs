// Turns Axon items into one Rust source file. v0 ownership strategy: every value read is
// cloned and `mut` parameters become &mut, so generated code never hits borrow errors.
use crate::ast::*;
use crate::diag::Diag;
use std::collections::{HashMap, HashSet};

pub const PRELUDE: &str = include_str!("prelude.rs");

const BUILTINS: &[&str] = &["print", "read", "write", "args", "input", "stdin", "lines", "str", "fail", "json", "csv", "date", "abs", "min", "max", "heap"];
const FALLIBLE_BUILTINS: &[&str] = &["read", "write", "json", "date"];
const METHODS: &[&str] = &[
    "abs", "add_days", "all", "any", "big", "day", "days", "dec", "digit", "or", "ceil", "chars", "contains", "count", "ends_with", "enumerate", "filter", "first",
    "find", "float", "floor", "fmt", "fold", "ints", "is_alpha", "is_digit", "is_space", "slice", "get_or", "has", "index_of", "insert", "int", "is_empty", "join", "keys", "last",
    "len", "lines", "lower", "map", "max", "min", "pairs", "pop", "push", "remove", "repeat", "replace", "rev",
    "round", "skip", "sort", "sort_by", "split", "sqrt", "starts_with", "sum", "take", "trim", "upper", "values",
    "weekday", "words", "year", "zip", "month",
];

// Methods that return a new value; calling one as a statement does nothing.
const PURE_METHODS: &[&str] = &[
    "sort", "sort_by", "rev", "map", "filter", "take", "skip", "trim", "lower", "upper", "replace", "enumerate", "zip",
];
const MUTATING_METHODS: &[&str] = &["push", "pop", "insert", "remove"];
// Methods whose lambda may use `?`; the whole call then fails with the first error.
const TRY_METHODS: &[&str] = &["map", "filter", "any", "all", "count", "fold", "sort_by"];
// Methods whose lambda takes one item: `(a, b) => ...` there unpacks a tuple item.
const ONE_ARG_METHODS: &[&str] = &["map", "filter", "any", "all", "count", "sort_by"];

struct FnSig {
    params: Vec<Param>,
    fallible: bool,
}

pub struct Emitter {
    structs: HashMap<String, Vec<Field>>,
    variants: HashMap<String, (String, Vec<Field>)>,
    fns: HashMap<String, FnSig>,
    // Per-function state.
    locals: HashSet<String>,
    refs: HashSet<String>, // names bound to &mut (mut params, iter_mut loop vars)
    // Script-level variables that functions use: name -> (type, line of its let).
    globals: HashMap<String, (Ty, usize)>,
    // Names first set inside a block that has ended, for a clearer "unknown name" error.
    block_only: HashSet<String>,
    fallible: bool,
    line: usize,
    pub diags: Vec<Diag>,
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == *cb { 0 } else { 1 };
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn closest<'a>(name: &str, options: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    options
        .map(|o| (edit_distance(name, o), o))
        .filter(|(d, _)| *d <= 2.max(name.len() / 2))
        .min()
        .map(|(_, o)| o)
}

pub fn rust_ty(t: &Ty) -> String {
    match t {
        Ty::Named(n) => match n.as_str() {
            "int" => "i64".into(),
            "f64" | "float" => "f64".into(),
            "bool" => "bool".into(),
            "str" => "String".into(),
            "dec" => "Dec".into(),
            "big" => "Big".into(),
            "date" => "Date".into(),
            "json" => "Json".into(),
            other => other.into(),
        },
        Ty::List(t) => format!("Vec<{}>", rust_ty(t)),
        Ty::Map(k, v) => format!("AxM<{}, {}>", rust_ty(k), rust_ty(v)),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(rust_ty).collect::<Vec<_>>().join(", ")),
        Ty::Unit => "()".into(),
    }
}

fn rust_str_lit(s: &str) -> String {
    format!("{s:?}")
}

// Does this expression use `?` outside any nested lambda?
fn contains_try(e: &Expr) -> bool {
    walk_try(e, false)
}

// Can this function body fail: any `?` (lambdas included) or a call to fail()?
fn can_fail(e: &Expr) -> bool {
    walk_try(e, true)
}

fn walk_try(e: &Expr, deep: bool) -> bool {
    let contains_try = |x: &Expr| walk_try(x, deep);
    let any = |xs: &[Expr]| xs.iter().any(|x| walk_try(x, deep));
    match e {
        Expr::Try(_) => true,
        Expr::Call(n, _, _) if deep && n == "fail" => true,
        Expr::Lambda(_, b) => deep && contains_try(b),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Move(_) => false,
        Expr::Str(parts) => parts.iter().any(|p| matches!(p, StrPart::Interp(x) if contains_try(x))),
        Expr::List(xs) | Expr::Tuple(xs) => any(xs),
        Expr::Map(ps) => ps.iter().any(|(k, v)| contains_try(k) || contains_try(v)),
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) => contains_try(a),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => contains_try(a) || contains_try(b),
        Expr::Call(_, args, _) => args.iter().any(|(_, a)| contains_try(a)),
        Expr::Method(r, _, args, _) => contains_try(r) || any(args),
        Expr::If(c, t, f) => contains_try(c) || contains_try(t) || f.as_ref().map_or(false, |f| contains_try(f)),
        Expr::Match(s, arms) => contains_try(s) || arms.iter().any(|(_, b)| contains_try(b)),
        Expr::For(_, i, b) => contains_try(i) || contains_try(b),
        Expr::Block(stmts, tail) => {
            tail.as_ref().map_or(false, |t| contains_try(t))
                || stmts.iter().any(|s| match s {
                    Stmt::Let { value, .. } => contains_try(value),
                    Stmt::Assign { target, value, .. } => contains_try(target) || contains_try(value),
                    Stmt::Expr { expr, .. } | Stmt::Assert { cond: expr, .. } => contains_try(expr),
                    Stmt::Return { value, .. } => value.as_ref().map_or(false, |v| contains_try(v)),
                    _ => false,
                })
        }
    }
}

// `?` is optional on calls: a call to a function that can fail passes its error up unless it is
// already handled (`?`, `.or(...)`, or a match on Ok/Err). Repeats until no more functions become fallible.
pub fn auto_try(items: &mut [Item]) {
    loop {
        let mut fallible: HashSet<String> = FALLIBLE_BUILTINS.iter().map(|s| s.to_string()).collect();
        for it in items.iter() {
            if let Item::Fn { name, fallible: f, body, .. } = it {
                if *f || can_fail(body) {
                    fallible.insert(name.clone());
                }
            }
        }
        let before = fallible.len();
        for it in items.iter_mut() {
            match it {
                Item::Fn { body, .. } | Item::Test { body, .. } => wrap_calls(body, &fallible),
                _ => {}
            }
        }
        let after = items.iter().filter(|it| matches!(it, Item::Fn { fallible: f, body, .. } if *f || can_fail(body))).count()
            + FALLIBLE_BUILTINS.len();
        if after <= before {
            return;
        }
    }
}

// Conversions that fail on bad text (`"x".int()`); on numbers they cannot fail.
const CONVERT_METHODS: &[&str] = &["int", "float", "ints", "big", "dec"];

fn wrap_calls(e: &mut Expr, f: &HashSet<String>) {
    // Walks into a call or conversion whose error is handled here, without wrapping it.
    let args_of = |e: &mut Expr, f: &HashSet<String>| match e {
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| wrap_calls(a, f)),
        Expr::Method(r, m, args, _) if CONVERT_METHODS.contains(&m.as_str()) => {
            wrap_calls(r, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
        }
        other => wrap_calls(other, f),
    };
    match e {
        Expr::Try(inner) => args_of(inner, f),
        Expr::Method(recv, m, args, _) if m == "or" => {
            args_of(recv, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
        }
        Expr::Match(subj, arms) => {
            if arms.iter().any(|(p, _)| matches!(p, Pat::Variant(n, _) if n == "Ok" || n == "Err")) {
                args_of(subj, f);
            } else {
                wrap_calls(subj, f);
            }
            arms.iter_mut().for_each(|(_, b)| wrap_calls(b, f));
        }
        Expr::Call(n, args, _) => {
            args.iter_mut().for_each(|(_, a)| wrap_calls(a, f));
            if f.contains(n.as_str()) {
                let call = std::mem::replace(e, Expr::Bool(false));
                *e = Expr::Try(Box::new(call));
            }
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Move(_) => {}
        Expr::Str(parts) => parts.iter_mut().for_each(|p| {
            if let StrPart::Interp(x) = p {
                wrap_calls(x, f)
            }
        }),
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| wrap_calls(x, f)),
        Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
            wrap_calls(k, f);
            wrap_calls(v, f)
        }),
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) => wrap_calls(a, f),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) | Expr::For(_, a, b) => {
            wrap_calls(a, f);
            wrap_calls(b, f)
        }
        Expr::Method(r, m, args, _) => {
            wrap_calls(r, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
            if CONVERT_METHODS.contains(&m.as_str()) {
                let call = std::mem::replace(e, Expr::Bool(false));
                *e = Expr::Try(Box::new(call));
            }
        }
        Expr::If(c, t, e2) => {
            wrap_calls(c, f);
            wrap_calls(t, f);
            if let Some(x) = e2 {
                wrap_calls(x, f)
            }
        }
        Expr::Block(stmts, tail) => {
            for s in stmts {
                match s {
                    Stmt::Let { value, .. } => wrap_calls(value, f),
                    Stmt::Assign { target, value, .. } => {
                        wrap_calls(target, f);
                        wrap_calls(value, f)
                    }
                    Stmt::Expr { expr, .. } | Stmt::Assert { cond: expr, .. } => wrap_calls(expr, f),
                    Stmt::Return { value: Some(v), .. } => wrap_calls(v, f),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                wrap_calls(t, f)
            }
        }
    }
}

// The variable at the bottom of a place like a.b[i].c.
fn root_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident(n) | Expr::Move(n) => Some(n),
        Expr::Field(b, _) | Expr::TupleField(b, _) | Expr::Index(b, _) => root_name(b),
        _ => None,
    }
}

pub fn assigns_to(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Block(stmts, tail) => {
            stmts.iter().any(|s| match s {
                Stmt::Assign { target: Expr::Ident(n), .. } => n == name,
                Stmt::Expr { expr, .. } => assigns_to(expr, name),
                _ => false,
            }) || tail.as_ref().map_or(false, |t| assigns_to(t, name))
        }
        Expr::If(_, a, b) => assigns_to(a, name) || b.as_ref().map_or(false, |b| assigns_to(b, name)),
        Expr::For(_, _, b) | Expr::While(_, b) => assigns_to(b, name),
        Expr::Match(_, arms) => arms.iter().any(|(_, b)| assigns_to(b, name)),
        _ => false,
    }
}

impl Emitter {
    pub fn new(items: &[Item]) -> Emitter {
        let mut e = Emitter {
            structs: HashMap::new(),
            variants: HashMap::new(),
            fns: HashMap::new(),
            locals: HashSet::new(),
            refs: HashSet::new(),
            globals: HashMap::new(),
            block_only: HashSet::new(),
            fallible: false,
            line: 0,
            diags: Vec::new(),
        };
        let one = |n: &str, t: &str| vec![Field { name: "v".into(), ty: Ty::Named(t.into()), default: None }].into_iter().filter(|_| !n.is_empty()).collect::<Vec<_>>();
        for (v, t) in [("Null", ""), ("Bool", "bool"), ("Num", "f64"), ("Str", "str"), ("Arr", "[Json]"), ("Obj", "{str: Json}")] {
            e.variants.insert(v.into(), ("Json".into(), one(t, t)));
        }
        for it in items {
            match it {
                Item::Type { name, fields, .. } => {
                    e.structs.insert(name.clone(), fields.clone());
                }
                Item::Enum { name, variants, .. } => {
                    for v in variants {
                        e.variants.insert(v.name.clone(), (name.clone(), v.fields.clone()));
                    }
                }
                Item::Fn { name, params, fallible, line, body, .. } => {
                    if e.fns.contains_key(name) {
                        e.diag(*line, &format!("function '{name}' is defined twice"), None);
                    }
                    // `!` is optional: a function that uses ? or fail() can fail.
                    let fallible = *fallible || can_fail(body);
                    e.fns.insert(name.clone(), FnSig { params: params.clone(), fallible });
                }
                Item::Test { .. } => {}
            }
        }
        e.find_globals(items);
        e
    }

    // A script-level `let`/`var` that a function or test mentions becomes a global.
    fn find_globals(&mut self, items: &[Item]) {
        let Some(Expr::Block(stmts, _)) = items.iter().find_map(|i| match i {
            Item::Fn { script: true, body, .. } => Some(body),
            _ => None,
        }) else {
            return;
        };
        // (parameter names, debug text of the body) for every function and test.
        let others: Vec<(Vec<String>, String)> = items
            .iter()
            .filter_map(|i| match i {
                Item::Fn { script: false, body, params, .. } => Some((params.iter().map(|p| p.name.clone()).collect(), format!("{body:?}"))),
                Item::Test { body, .. } => Some((vec![], format!("{body:?}"))),
                _ => None,
            })
            .collect();
        let decls: Vec<(String, Option<Ty>, Expr, usize)> = stmts
            .iter()
            .filter_map(|st| match st {
                Stmt::Let { name: Pat::Bind(n), ty, value, line, .. } => Some((n.clone(), ty.clone(), value.clone(), *line)),
                Stmt::Assign { target: Expr::Ident(n), op: "=", value, line } => Some((n.clone(), None, value.clone(), *line)),
                _ => None,
            })
            .collect();
        for (n, ty, value, line) in &decls {
            let (n, ty, value, line) = (n, ty, value, line);
            {
                // Used by name, and not a parameter or a local of the same name there.
                let used = others.iter().any(|(ps, body)| {
                    body.contains(&format!("Ident({n:?})")) && !ps.contains(n) && !body.contains(&format!("Bind({n:?})")) && !body.contains(&format!("{n:?}]"))
                });
                if !used || self.globals.contains_key(n) {
                    continue;
                }
                let found = ty.clone().or_else(|| self.infer_ty(value));
                // With no known type, a function that sets the name before reading it has its own local.
                let own_local = |body: &String| {
                    let id = format!("Ident({n:?})");
                    body.find(&id).map_or(true, |i| body[..i].ends_with("Assign { target: ") && body[i..].starts_with(&format!("{id}, op: \"=\"")))
                };
                if found.is_none() && others.iter().all(|(ps, body)| ps.contains(n) || own_local(body)) {
                    continue;
                }
                match found {
                    Some(t) => {
                        self.globals.insert(n.clone(), (t, *line));
                    }
                    None => {
                        let example = match value {
                            Expr::List(_) => "[int] = []",
                            Expr::Map(_) => "{str: int} = {}",
                            _ => "TYPE = ...",
                        };
                        self.diag(*line, &format!("{n} is used inside a function, so it needs a type"), Some(format!("write {n}: {example} with the real type")));
                        // Known by name from here on, so later uses are not reported too.
                        self.globals.insert(n.clone(), (Ty::Named("_".into()), *line));
                    }
                }
            }
        }
    }

    // The type of a simple initial value, for globals.
    fn infer_ty(&self, e: &Expr) -> Option<Ty> {
        let named = |n: &str| Some(Ty::Named(n.into()));
        match e {
            Expr::Int(_) => named("int"),
            Expr::Float(_) => named("f64"),
            Expr::Bool(_) => named("bool"),
            Expr::Str(_) => named("str"),
            Expr::Unary(_, a) => self.infer_ty(a),
            Expr::List(xs) if !xs.is_empty() => Some(Ty::List(Box::new(self.infer_ty(&xs[0])?))),
            Expr::Tuple(xs) => Some(Ty::Tuple(xs.iter().map(|x| self.infer_ty(x)).collect::<Option<Vec<_>>>()?)),
            Expr::Call(n, _, _) => match n.as_str() {
                "lines" | "args" => Some(Ty::List(Box::new(Ty::Named("str".into())))),
                "stdin" | "input" | "str" => named("str"),
                "csv" => Some(Ty::List(Box::new(Ty::List(Box::new(Ty::Named("str".into())))))),
                n if self.structs.contains_key(n) => named(n),
                n => self.variants.get(n).map(|(en, _)| Ty::Named(en.clone())),
            },
            Expr::Try(inner) => match inner.as_ref() {
                Expr::Method(_, m, _, _) if m == "int" => named("int"),
                Expr::Method(_, m, _, _) if m == "float" => named("f64"),
                Expr::Method(_, m, _, _) if m == "ints" => Some(Ty::List(Box::new(Ty::Named("int".into())))),
                Expr::Call(n, _, _) if n == "read" => named("str"),
                Expr::Call(n, _, _) if n == "json" => named("Json"),
                Expr::Call(n, _, _) if n == "date" => named("Date"),
                _ => None,
            },
            Expr::Method(_, m, _, _) => match m.as_str() {
                "len" | "count" | "find" | "index_of" | "digit" => named("int"),
                "words" | "lines" | "chars" | "split" => Some(Ty::List(Box::new(Ty::Named("str".into())))),
                "trim" | "lower" | "upper" | "join" | "replace" => named("str"),
                _ => None,
            },
            _ => None,
        }
    }

    fn global(&self, n: &str) -> Option<String> {
        if self.locals.contains(n) || !self.globals.contains_key(n) {
            return None;
        }
        Some(format!("(*ax_gp(unsafe {{ std::ptr::addr_of_mut!(AXG_{n}) }}, {n:?}))"))
    }

    fn diag(&mut self, line: usize, msg: &str, fix: Option<String>) {
        let mut d = Diag::new("check", line, 1, msg);
        d.fix = fix;
        self.diags.push(d);
    }

    pub fn program(&mut self, items: &[Item]) -> String {
        let mut out = String::from(PRELUDE);
        out.push('\n');
        let mut tests = String::new();
        let mut test_n = 0;
        let mut gs: Vec<_> = self.globals.iter().map(|(n, (t, _))| (n.clone(), rust_ty(t))).collect();
        gs.sort();
        for (n, t) in gs {
            out.push_str(&format!("static mut AXG_{n}: Option<{t}> = None;\n"));
        }
        for it in items {
            match it {
                Item::Type { name, fields, line } => {
                    out.push_str(&format!("/*@L{line}*/#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]\npub struct {name} {{\n"));
                    for f in fields {
                        out.push_str(&format!("    pub {}: {},\n", f.name, rust_ty(&f.ty)));
                    }
                    out.push_str(&format!("}}\nimpl Show for {name} {{ fn show(&self) -> String {{ format!(\"{{:?}}\", self) }} }}\n"));
                }
                Item::Enum { name, variants, line } => {
                    out.push_str(&format!("/*@L{line}*/#[derive(Debug, Clone, PartialEq, PartialOrd)]\npub enum {name} {{\n"));
                    for v in variants {
                        if v.fields.is_empty() {
                            out.push_str(&format!("    {},\n", v.name));
                        } else {
                            let fs: Vec<String> = v.fields.iter().map(|f| format!("{}: {}", f.name, rust_ty(&f.ty))).collect();
                            out.push_str(&format!("    {} {{ {} }},\n", v.name, fs.join(", ")));
                        }
                    }
                    out.push_str(&format!("}}\nimpl Show for {name} {{ fn show(&self) -> String {{ format!(\"{{:?}}\", self) }} }}\n"));
                    // Default is the first variant, so `m[k]` can create entries of this type.
                    if let Some(v) = variants.first() {
                        let fs: Vec<String> = v.fields.iter().map(|f| format!("{}: Default::default()", f.name)).collect();
                        let val = if fs.is_empty() { format!("{name}::{}", v.name) } else { format!("{name}::{} {{ {} }}", v.name, fs.join(", ")) };
                        out.push_str(&format!("impl Default for {name} {{ fn default() -> Self {{ {val} }} }}\n"));
                    }
                }
                Item::Fn { name, params, ret, body, line, .. } => {
                    let fallible = self.fns.get(name).map_or(false, |f| f.fallible);
                    out.push_str(&self.function(name, params, ret, fallible, body, *line));
                }
                Item::Test { body, line, name } => {
                    test_n += 1;
                    self.begin_fn(&[], true);
                    let b = self.body(body, &Ty::Unit, true);
                    tests.push_str(&format!("/*@L{line}*/#[test] fn t{test_n}_{}() -> R<()> {b}\n", slug(name)));
                }
            }
        }
        match self.fns.get("main") {
            Some(sig) if sig.fallible => out.push_str(
                "fn main() {\n    ax_init();\n    if let Err(e) = ax_main() {\n        eprintln!(\"{{\\\"e\\\":\\\"runtime\\\",\\\"line\\\":{},\\\"msg\\\":{:?}}}\", AX_LINE.load(std::sync::atomic::Ordering::Relaxed), e.0);\n        std::process::exit(1);\n    }\n}\n",
            ),
            Some(_) => {}
            None => out.push_str("fn main() {}\n"),
        }
        if !tests.is_empty() {
            out.push_str("#[cfg(test)]\nmod ax_tests {\n    use super::*;\n");
            out.push_str(&tests);
            out.push_str("}\n");
        }
        out
    }

    fn begin_fn(&mut self, params: &[Param], fallible: bool) {
        self.locals = params.iter().map(|p| p.name.clone()).collect();
        self.refs = params.iter().filter(|p| p.is_mut).map(|p| p.name.clone()).collect();
        self.fallible = fallible;
    }

    fn function(&mut self, name: &str, params: &[Param], ret: &Ty, fallible: bool, body: &Expr, line: usize) -> String {
        self.begin_fn(params, fallible);
        self.line = line;
        let ps: Vec<String> = params
            .iter()
            .map(|p| if p.is_mut { format!("{}: &mut {}", p.name, rust_ty(&p.ty)) } else { format!("{}: {}", p.name, rust_ty(&p.ty)) })
            .collect();
        let rname = if name == "main" && fallible { "ax_main" } else { name };
        let init = if name == "main" && !fallible { "ax_init();\n" } else { "" };
        if name == "main" && !params.is_empty() {
            self.diag(line, "main takes no parameters", Some("use args() to read command line arguments".into()));
        }
        let rret = if fallible { format!(" -> R<{}>", rust_ty(ret)) } else if matches!(ret, Ty::Unit) { String::new() } else { format!(" -> {}", rust_ty(ret)) };
        let b = self.body(body, ret, fallible);
        let b = if init.is_empty() { b } else { b.replacen("{\n", &format!("{{\n    {init}"), 1) };
        format!("/*@L{line}*/fn {rname}({}){rret} {b}\n", ps.join(", "))
    }

    // A function body: the tail value becomes the return value, wrapped in Ok for fallible functions.
    fn body(&mut self, body: &Expr, ret: &Ty, fallible: bool) -> String {
        let unit = matches!(ret, Ty::Unit);
        let (stmts, tail): (&[Stmt], Option<&Expr>) = match body {
            Expr::Block(s, t) => (s, t.as_deref()),
            e => (&[], Some(e)),
        };
        let mut out = String::from("{\n");
        for s in stmts {
            out.push_str(&self.stmt(s));
        }
        match tail {
            Some(t) if unit => {
                out.push_str(&format!("    {};\n", self.stmt_expr(t)));
                if fallible {
                    out.push_str("    Ok(())\n");
                }
            }
            Some(t) => {
                let v = self.expr(t);
                out.push_str(&if fallible { format!("    Ok({v})\n") } else { format!("    {v}\n") });
            }
            None if fallible && unit => out.push_str("    Ok(())\n"),
            None if !unit => {
                // A non-unit function must end in a value unless every path returns.
                let ends_in_return = matches!(stmts.last(), Some(Stmt::Return { .. }));
                if !ends_in_return {
                    self.diag(self.line, "function body ends without a value", Some("make the last line the value to return".into()));
                }
            }
            None => {}
        }
        out.push('}');
        out
    }

    fn block(&mut self, e: &Expr) -> String {
        self.block_d(e, false)
    }

    // discard: the block is used as a statement, so its last value is thrown away.
    fn block_d(&mut self, e: &Expr, discard: bool) -> String {
        match e {
            Expr::Block(stmts, tail) => {
                // Names declared in a block end with it.
                let outer = self.locals.clone();
                let mut out = String::from("{\n");
                for s in stmts {
                    out.push_str(&self.stmt(s));
                }
                if let Some(t) = tail {
                    if discard {
                        out.push_str(&format!("    {};\n", self.stmt_expr(t)));
                    } else {
                        out.push_str(&format!("    {}\n", self.expr(t)));
                    }
                }
                out.push('}');
                let inner = std::mem::replace(&mut self.locals, outer);
                self.block_only.extend(inner.into_iter().filter(|n| !self.locals.contains(n)));
                out
            }
            other if discard => format!("{{ {}; }}", self.stmt_expr(other)),
            other => format!("{{ {} }}", self.expr(other)),
        }
    }

    // An expression whose value is not used: if/match branches need not agree on a type.
    fn stmt_expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::If(c, t, f) => {
                let c = self.expr(c);
                let t = self.block_d(t, true);
                match f {
                    Some(f) if matches!(f.as_ref(), Expr::If(..)) => format!("if {c} {t} else {}", self.stmt_expr(f)),
                    Some(f) => format!("if {c} {t} else {}", self.block_d(f, true)),
                    None => format!("if {c} {t}"),
                }
            }
            Expr::Match(subj, arms) => self.match_expr(subj, arms, true),
            Expr::Method(recv, name, _, line) if PURE_METHODS.contains(&name.as_str()) => {
                let target = match recv.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    _ => "x".into(),
                };
                self.diag(*line, &format!(".{name}() returns a new value and does not change {target}"), Some(format!("write {target} = {target}.{name}(...)")));
                self.expr(e)
            }
            other => self.expr(other),
        }
    }

    fn bind_pat(&mut self, p: &Pat) {
        match p {
            Pat::Bind(n) => {
                self.locals.insert(n.clone());
            }
            Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| self.bind_pat(p)),
            _ => {}
        }
    }

    fn pat(&mut self, p: &Pat) -> String {
        match p {
            Pat::Wild => "_".into(),
            Pat::Bind(n) => n.clone(),
            Pat::Int(n) => format!("{n}i64"),
            Pat::Str(s) => rust_str_lit(s),
            Pat::Bool(b) => b.to_string(),
            Pat::Tuple(ps) => format!("({})", ps.iter().map(|p| self.pat(p)).collect::<Vec<_>>().join(", ")),
            Pat::Variant(name, args) if (name == "Ok" || name == "Err") && args.len() == 1 => {
                let p = self.pat(&args[0]);
                if name == "Ok" { format!("Ok({p})") } else { format!("Err(AxErr({p}))") }
            }
            Pat::Variant(name, args) => match self.variants.get(name).cloned() {
                Some((en, fields)) => {
                    if fields.is_empty() {
                        format!("{en}::{name}")
                    } else {
                        if args.len() != fields.len() {
                            self.diag(self.line, &format!("pattern {name} has {} fields, not {}", fields.len(), args.len()), Some(format!("write {name}({})", fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>().join(", "))));
                        }
                        let parts: Vec<String> = fields.iter().zip(args).map(|(f, a)| format!("{}: {}", f.name, self.pat(a))).collect();
                        format!("{en}::{name} {{ {} }}", parts.join(", "))
                    }
                }
                None => {
                    let hint = closest(name, self.variants.keys().map(|s| s.as_str())).map(|c| format!("did you mean {c}?"));
                    self.diag(self.line, &format!("unknown variant {name}"), hint);
                    "_".into()
                }
            },
        }
    }

    fn stmt(&mut self, s: &Stmt) -> String {
        let line = match s {
            Stmt::Let { line, .. } | Stmt::Assign { line, .. } | Stmt::Expr { line, .. } | Stmt::Return { line, .. } | Stmt::Break { line } | Stmt::Continue { line } | Stmt::Assert { line, .. } => *line,
        };
        self.line = line;
        let body = match s {
            Stmt::Let { name: Pat::Bind(n), value, .. } if self.globals.get(n).map_or(false, |g| g.1 == line) => {
                let v = self.expr(value);
                format!("unsafe {{ AXG_{n} = Some({v}); }}")
            }
            Stmt::Let { name, ty, value, mutable, .. } => {
                let v = self.expr(value);
                self.bind_pat(name);
                let p = self.pat(name);
                let t = ty.as_ref().map(|t| format!(": {}", rust_ty(t))).unwrap_or_default();
                format!("let {}{p}{t} = {v};", if *mutable { "mut " } else { "" })
            }
            Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if self.globals.get(n).map_or(false, |g| g.1 == line) => {
                let v = self.expr(value);
                format!("unsafe {{ AXG_{n} = Some({v}); }}")
            }
            // `x = v` for a new name declares it, like `var x = v`.
            Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. }
                if !self.locals.contains(n) && !self.globals.contains_key(n) && !self.fns.contains_key(n) && !self.variants.contains_key(n) =>
            {
                let v = self.expr(value);
                self.locals.insert(n.clone());
                format!("let mut {n} = {v};")
            }
            // `(a, b) = v` unpacks; new names are declared.
            Stmt::Assign { target: Expr::Tuple(xs), op: "=", value, .. } if xs.iter().all(|x| matches!(x, Expr::Ident(_))) => {
                let v = self.expr(value);
                let names: Vec<String> = xs.iter().map(|x| if let Expr::Ident(n) = x { n.clone() } else { unreachable!() }).collect();
                if names.iter().all(|n| self.locals.contains(n) && !self.globals.contains_key(n)) {
                    let ps: Vec<String> = names.iter().map(|n| if self.refs.contains(n) { format!("*{n}") } else { n.clone() }).collect();
                    format!("({}) = {v};", ps.join(", "))
                } else {
                    names.iter().for_each(|n| {
                        self.locals.insert(n.clone());
                    });
                    format!("let ({}) = {v};", names.iter().map(|n| format!("mut {n}")).collect::<Vec<_>>().join(", "))
                }
            }
            Stmt::Assign { target, op, value, .. } => {
                let v = self.expr(value);
                match (target, *op) {
                    (Expr::Index(base, idx), "=") => {
                        let b = self.place_mut(base);
                        let i = self.expr(idx);
                        format!("{{ let __v = {v}; {b}.ax_set({i}, __v); }}")
                    }
                    (_, "+=") => {
                        let p = self.place_mut(target);
                        format!("{{ let __v = {v}; let __p = &mut {p}; *__p = Plus::plus(std::mem::take(__p), __v); }}")
                    }
                    (_, op) => {
                        let p = self.place_mut(target);
                        format!("{{ let __v = {v}; {p} {op} __v; }}")
                    }
                }
            }
            Stmt::Expr { expr, .. } => format!("{};", self.stmt_expr(expr)),
            Stmt::Return { value, .. } => {
                let v = value.as_ref().map(|v| self.expr(v)).unwrap_or_else(|| "()".into());
                if self.fallible { format!("return Ok({v});") } else { format!("return {v};") }
            }
            Stmt::Break { .. } => "break;".into(),
            Stmt::Continue { .. } => "continue;".into(),
            Stmt::Assert { cond, .. } => format!("assert!({}, \"assert failed at line {line}\");", self.expr(cond)),
        };
        format!("    /*@L{line}*/ax_l({line}); {body}\n")
    }

    // A place that can be written to.
    fn place_mut(&mut self, e: &Expr) -> String {
        match e {
            Expr::Ident(n) => {
                if let Some(g) = self.global(n) {
                    return g;
                }
                self.check_ident(n);
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) => format!("{}.{f}", self.place_mut(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place_mut(b)),
            Expr::Index(b, i) => {
                // The key is borrowed unless it reads the collection being changed; then it is computed first.
                let reads_base = root_name(b).map_or(true, |r| format!("{i:?}").contains(&format!("({r:?})")));
                let b = self.place_mut(b);
                let i = if reads_base { format!("&({})", self.expr(i)) } else { self.arg_ref(i) };
                format!("(*{b}.ax_mut({i}))")
            }
            other => {
                self.diag(self.line, "this expression cannot be assigned to", Some("assign to a variable, field or index".into()));
                self.expr(other)
            }
        }
    }

    // A place to read through without cloning (method receivers, field bases).
    fn place(&mut self, e: &Expr) -> String {
        if let Expr::Ident(n) = e {
            if let Some(g) = self.global(n) {
                return g;
            }
        }
        match e {
            Expr::Ident(n) if self.locals.contains(n) => {
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) => format!("{}.{f}", self.place(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place(b)),
            Expr::Index(b, i) => {
                let b = self.place(b);
                format!("(*{b}.ax_ref({}))", self.arg_ref(i))
            }
            other => format!("({})", self.expr(other)),
        }
    }

    fn operand(&mut self, e: &Expr) -> String {
        match e {
            Expr::Ident(_) | Expr::Field(..) | Expr::TupleField(..) | Expr::Index(..) => self.place(e),
            other => self.expr(other),
        }
    }

    // A reference to a value, for arguments that are only read (keys, searched-for items).
    fn arg_ref(&mut self, e: &Expr) -> String {
        format!("&{}", self.place(e))
    }

    fn check_ident(&mut self, n: &str) {
        if self.locals.contains(n) || self.fns.contains_key(n) || self.globals.contains_key(n) {
            return;
        }
        if self.block_only.contains(n) {
            self.diag(self.line, &format!("{n} was first set inside a block that has ended"), Some(format!("set {n} before that block, e.g. {n} = 0")));
            return;
        }
        let names: Vec<String> = self.locals.iter().cloned().chain(self.fns.keys().cloned()).collect();
        let hint = closest(n, names.iter().map(|s| s.as_str())).map(|c| format!("did you mean {c}?"));
        self.diag(self.line, &format!("unknown name '{n}'"), hint.or(Some(format!("declare it first with let {n} = ..."))));
    }

    fn call_is_fallible(&self, e: &Expr) -> bool {
        match e {
            Expr::Call(name, _, _) => FALLIBLE_BUILTINS.contains(&name.as_str()) || self.fns.get(name).map_or(false, |f| f.fallible),
            _ => false,
        }
    }

    pub fn expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::Int(n) => format!("{n}i64"),
            Expr::Float(f) => format!("{f:?}f64"),
            Expr::Bool(b) => b.to_string(),
            Expr::Str(parts) => {
                if let [StrPart::Lit(s)] = parts.as_slice() {
                    return format!("String::from({})", rust_str_lit(s));
                }
                let mut fmt = String::new();
                let mut args = Vec::new();
                for p in parts {
                    match p {
                        StrPart::Lit(s) => {
                            let lit = rust_str_lit(s);
                            fmt.push_str(&lit[1..lit.len() - 1].replace('{', "{{").replace('}', "}}"));
                        }
                        StrPart::Interp(e) => {
                            fmt.push_str("{}");
                            args.push(format!("({}).show()", self.expr(e)));
                        }
                    }
                }
                format!("format!(\"{fmt}\", {})", args.join(", "))
            }
            Expr::Move(n) => {
                if self.global(n).is_none() && self.locals.contains(n) && !self.refs.contains(n) {
                    n.clone()
                } else {
                    self.expr(&Expr::Ident(n.clone()))
                }
            }
            Expr::Ident(n) => {
                if let Some((en, fields)) = self.variants.get(n).cloned() {
                    if fields.is_empty() {
                        return format!("{en}::{n}");
                    }
                }
                if let Some(g) = self.global(n) {
                    return format!("{g}.clone()");
                }
                self.check_ident(n);
                if self.locals.contains(n) {
                    format!("{}.clone()", self.place(e))
                } else {
                    n.clone()
                }
            }
            Expr::List(items) => format!("vec![{}]", items.iter().map(|i| self.expr(i)).collect::<Vec<_>>().join(", ")),
            Expr::Map(pairs) => {
                if pairs.is_empty() {
                    "AxM::new()".into()
                } else {
                    let ps: Vec<String> = pairs.iter().map(|(k, v)| format!("({}, {})", self.expr(k), self.expr(v))).collect();
                    format!("AxM::from([{}])", ps.join(", "))
                }
            }
            Expr::Tuple(items) => format!("({})", items.iter().map(|i| self.expr(i)).collect::<Vec<_>>().join(", ")),
            Expr::Unary(op, a) => format!("({op}{})", self.expr(a)),
            Expr::Binary(op, a, b) => {
                // Comparisons read their operands in place, without copying them.
                let (a, b) = if ["==", "!=", "<", ">", "<=", ">="].contains(op) { (self.operand(a), self.operand(b)) } else { (self.expr(a), self.expr(b)) };
                match *op {
                    "+" => format!("Plus::plus({a}, {b})"),
                    "and" => format!("({a} && {b})"),
                    "or" => format!("({a} || {b})"),
                    "~=" => format!("ax_approx({a}, {b})"),
                    op => format!("({a} {op} {b})"),
                }
            }
            Expr::Call(name, args, line) => {
                self.line = *line;
                let c = self.call(name, args, false);
                if c.starts_with("(return") { c } else { format!("{{ ax_l({line}); {c} }}") }
            }
            Expr::Method(recv, name, args, line) => {
                self.line = *line;
                if !METHODS.contains(&name.as_str()) {
                    let hint = closest(name, METHODS.iter().copied()).map(|c| format!("did you mean .{c}()?"));
                    self.diag(*line, &format!("unknown method .{name}()"), hint);
                }
                // Changing a list stored in a map or list creates the entry if needed.
                let r = if name == "or" {
                    // The receiver's error is handled here, so no `?` is needed on it.
                    match recv.as_ref() {
                        Expr::Call(n, cargs, l) => {
                            self.line = *l;
                            format!("({})", self.call(n, cargs, true))
                        }
                        other => format!("({})", self.expr(other)),
                    }
                } else if MUTATING_METHODS.contains(&name.as_str()) && matches!(recv.as_ref(), Expr::Index(..)) {
                    self.place_mut(recv)
                } else {
                    self.place(recv)
                };
                let tries = TRY_METHODS.contains(&name.as_str()) && args.iter().any(|a| matches!(a, Expr::Lambda(_, b) if contains_try(b)));
                if tries && !self.fallible {
                    self.diag(*line, "'?' used in a function that cannot fail", Some("add ! after the function's return type".into()));
                }
                let unpack = ONE_ARG_METHODS.contains(&name.as_str());
                let ref_arg = ["has", "get_or", "contains", "index_of"].contains(&name.as_str());
                let a: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(k, a)| match a {
                        a if k == 0 && ref_arg => self.arg_ref(a),
                        Expr::Lambda(ps, body) => {
                            for p in ps {
                                self.locals.insert(p.clone());
                            }
                            let params = if unpack && ps.len() > 1 { format!("({})", ps.join(", ")) } else { ps.join(", ") };
                            if !tries {
                                return format!("(|{params}| {})", self.expr(body));
                            }
                            let saved = std::mem::replace(&mut self.fallible, true);
                            let b = self.expr(body);
                            self.fallible = saved;
                            format!("(|{params}| -> R<_> {{ Ok({b}) }})")
                        }
                        other => self.expr(other),
                    })
                    .collect();
                if tries {
                    format!("{{ ax_l({line}); {r}.ax_try_{name}({})? }}", a.join(", "))
                } else {
                    format!("{{ ax_l({line}); {r}.ax_{name}({}) }}", a.join(", "))
                }
            }
            Expr::Field(..) | Expr::TupleField(..) => format!("{}.clone()", self.place(e)),
            Expr::Index(..) => format!("{}.clone()", self.place(e)),
            Expr::Lambda(params, body) => {
                for p in params {
                    self.locals.insert(p.clone());
                }
                format!("(|{}| {})", params.join(", "), self.expr(body))
            }
            Expr::Try(inner) => {
                if !self.fallible {
                    self.diag(self.line, "'?' used in a function that cannot fail", Some("add ! after the function's return type".into()));
                }
                match inner.as_ref() {
                    Expr::Call(name, args, line) => {
                        self.line = *line;
                        format!("{}?", self.call(name, args, true))
                    }
                    // ax_q lets `?` follow conversions that cannot fail on numbers (`2.5.int()`).
                    other => format!("ax_q({})?", self.expr(other)),
                }
            }
            Expr::If(c, t, f) => {
                let c = self.expr(c);
                let t = self.block(t);
                match f {
                    Some(f) => {
                        let f = if matches!(f.as_ref(), Expr::If(..)) { self.expr(f) } else { self.block(f) };
                        format!("if {c} {t} else {f}")
                    }
                    None => format!("if {c} {t}"),
                }
            }
            Expr::Match(subj, arms) => self.match_expr(subj, arms, false),
            Expr::For(p, iter, body) => {
                // Assigning to the loop variable edits the list in place.
                if let (Pat::Bind(n), Expr::Ident(_)) = (p, iter.as_ref()) {
                    if assigns_to(body, n) {
                        let it = self.place_mut(iter);
                        self.locals.insert(n.clone());
                        self.refs.insert(n.clone());
                        let b = self.block_d(body, true);
                        self.refs.remove(n);
                        return format!("for {n} in {it}.iter_mut() {b}");
                    }
                }
                let it = match iter.as_ref() {
                    // A range is counted through directly instead of being built as a list.
                    Expr::Range(a, b) => format!("({}..{})", self.expr(a), self.expr(b)),
                    other => self.expr(other),
                };
                let outer = self.locals.clone();
                self.bind_pat(p);
                let p = self.pat(p);
                let b = self.block_d(body, true);
                self.locals = outer;
                format!("for {p} in {it} {b}")
            }
            Expr::While(c, body) => {
                let c = self.expr(c);
                format!("while {c} {}", self.block_d(body, true))
            }
            Expr::Range(a, b) => format!("({}..{}).collect::<Vec<i64>>()", self.expr(a), self.expr(b)),
            Expr::Block(..) => self.block(e),
        }
    }

    fn match_expr(&mut self, subj: &Expr, arms: &[(Pat, Expr)], discard: bool) -> String {
        let has_str = arms.iter().any(|(p, _)| matches!(p, Pat::Str(_)));
        let handles_err = arms.iter().any(|(p, _)| matches!(p, Pat::Variant(n, _) if n == "Ok" || n == "Err"));
        let s = match subj {
            // Matching Ok/Err handles the call's error, so it needs no `?`.
            Expr::Call(n, args, line) if handles_err => {
                self.line = *line;
                format!("{{ ax_l({line}); {} }}", self.call(n, args, true))
            }
            other => self.expr(other),
        };
        let s = if has_str { format!("({s}).as_str()") } else { s };
        let mut out = format!("match {s} {{\n");
        for (p, body) in arms {
            let outer = self.locals.clone();
            self.bind_pat(p);
            let p = self.pat(p);
            let b = self.block_d(body, discard);
            self.locals = outer;
            out.push_str(&format!("        {p} => {b},\n"));
        }
        out.push_str("    }");
        out
    }

    fn call(&mut self, name: &str, args: &[(Option<String>, Expr)], tried: bool) -> String {
        if !tried && self.call_is_fallible(&Expr::Call(name.into(), vec![], 0)) {
            self.diag(self.line, &format!("{name}() can fail but its error is not handled"), Some(format!("write {name}(...)? to pass the error up")));
        }
        let plain: Vec<String> = args.iter().map(|(_, a)| self.expr(a)).collect();
        match name {
            "print" | "read" | "write" | "args" | "input" | "stdin" | "lines" | "json" | "csv" | "date" | "abs" | "min" | "max" | "heap" => return format!("{name}({})", plain.join(", ")),
            "str" => return format!("({}).show()", plain.join(", ")),
            "fail" => {
                if !self.fallible {
                    self.diag(self.line, "fail() used in a function that cannot fail", Some("add ! after the function's return type".into()));
                }
                return format!("(return Err(AxErr({})))", plain.join(", "));
            }
            _ => {}
        }
        if let Some(fields) = self.structs.get(name).cloned() {
            return self.construct(name, name, &fields, args, plain);
        }
        if let Some((en, fields)) = self.variants.get(name).cloned() {
            return self.construct(&format!("{en}::{name}"), name, &fields, args, plain);
        }
        if let Some(sig) = self.fns.get(name) {
            let params = sig.params.clone();
            if params.len() != args.len() {
                let sigtext = params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
                self.diag(self.line, &format!("{name}() takes {} arguments, got {}", params.len(), args.len()), Some(format!("call {name}({sigtext})")));
            }
            // Named arguments may come in any order.
            let mut ordered = Vec::new();
            for (i, p) in params.iter().enumerate() {
                let found = args.iter().position(|(n, _)| n.as_deref() == Some(p.name.as_str())).or(if args.get(i).map_or(false, |(n, _)| n.is_none()) { Some(i) } else { None });
                match found {
                    Some(j) if p.is_mut => ordered.push(format!("&mut {}", self.place_mut(&args[j].1))),
                    Some(j) => ordered.push(plain[j].clone()),
                    None => {}
                }
            }
            return format!("{name}({})", ordered.join(", "));
        }
        if self.locals.contains(name) {
            return format!("{name}({})", plain.join(", "));
        }
        let known: Vec<String> = self.fns.keys().cloned().chain(BUILTINS.iter().map(|s| s.to_string())).chain(self.structs.keys().cloned()).chain(self.variants.keys().cloned()).collect();
        let hint = closest(name, known.iter().map(|s| s.as_str())).map(|c| format!("did you mean {c}()?"));
        self.diag(self.line, &format!("unknown function {name}()"), hint);
        format!("{name}({})", plain.join(", "))
    }

    // Each field takes, in order of preference: an argument named after it (`total: t`), a plain
    // variable of the same name (`total`), the next plain argument, or the field's default.
    fn construct(&mut self, path: &str, name: &str, fields: &[Field], args: &[(Option<String>, Expr)], plain: Vec<String>) -> String {
        if fields.is_empty() && args.is_empty() {
            return path.to_string();
        }
        let mut used = vec![false; args.len()];
        let mut slot: Vec<Option<usize>> = vec![None; fields.len()];
        for (fi, f) in fields.iter().enumerate() {
            let by_name = args.iter().enumerate().position(|(j, (n, a))| {
                !used[j] && (n.as_deref() == Some(f.name.as_str()) || (n.is_none() && matches!(a, Expr::Ident(v) | Expr::Move(v) if *v == f.name)))
            });
            if let Some(j) = by_name {
                used[j] = true;
                slot[fi] = Some(j);
            }
        }
        for fi in 0..fields.len() {
            if slot[fi].is_some() {
                continue;
            }
            if let Some(j) = (0..args.len()).find(|&j| !used[j] && args[j].0.is_none()) {
                used[j] = true;
                slot[fi] = Some(j);
            }
        }
        let missing: Vec<&str> = fields.iter().zip(&slot).filter(|(f, s)| s.is_none() && f.default.is_none()).map(|(f, _)| f.name.as_str()).collect();
        let extra: Vec<String> = args.iter().zip(&used).filter(|(_, u)| !**u).map(|((n, _), _)| n.clone().unwrap_or_else(|| "a value".into())).collect();
        if !missing.is_empty() || !extra.is_empty() {
            let want = fields.iter().filter(|f| f.default.is_none()).map(|f| format!("{}: ...", f.name)).collect::<Vec<_>>().join(", ");
            let msg = if !missing.is_empty() { format!("{name} is missing {}", missing.join(", ")) } else { format!("{name} has no field for {}", extra.join(", ")) };
            self.diag(self.line, &msg, Some(format!("write {name}({want})")));
        }
        let mut parts = Vec::new();
        for (f, s) in fields.iter().zip(&slot) {
            match (s, &f.default) {
                (Some(j), _) => parts.push(format!("{}: {}", f.name, plain[*j])),
                (None, Some(d)) => {
                    let v = self.expr(d);
                    parts.push(format!("{}: {v}", f.name));
                }
                _ => {}
            }
        }
        format!("{path} {{ {} }}", parts.join(", "))
    }
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}
