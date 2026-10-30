// Turns Axon items into one Rust source file. v0 ownership strategy: every value read is
// cloned and `mut` parameters become &mut, so generated code never hits borrow errors.
use crate::ast::*;
use crate::diag::Diag;
use std::collections::{HashMap, HashSet};

pub const PRELUDE: &str = include_str!("prelude.rs");

const BUILTINS: &[&str] = &["print", "read", "write", "args", "input", "stdin", "lines", "str", "fail"];
const FALLIBLE_BUILTINS: &[&str] = &["read", "write"];
const METHODS: &[&str] = &[
    "abs", "all", "any", "ceil", "chars", "contains", "count", "ends_with", "enumerate", "filter", "first",
    "float", "floor", "fmt", "fold", "ints", "get_or", "has", "index_of", "insert", "int", "is_empty", "join", "keys", "last",
    "len", "lines", "lower", "map", "max", "min", "pairs", "pop", "push", "remove", "repeat", "replace", "rev",
    "round", "skip", "sort", "sort_by", "split", "sqrt", "starts_with", "sum", "take", "trim", "upper", "values",
    "words", "zip",
];

// Methods that return a new value; calling one as a statement does nothing.
const PURE_METHODS: &[&str] = &[
    "sort", "sort_by", "rev", "map", "filter", "take", "skip", "trim", "lower", "upper", "replace", "enumerate", "zip",
];
const MUTATING_METHODS: &[&str] = &["push", "pop", "insert", "remove"];

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
            other => other.into(),
        },
        Ty::List(t) => format!("Vec<{}>", rust_ty(t)),
        Ty::Map(k, v) => format!("BTreeMap<{}, {}>", rust_ty(k), rust_ty(v)),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(rust_ty).collect::<Vec<_>>().join(", ")),
        Ty::Unit => "()".into(),
    }
}

fn rust_str_lit(s: &str) -> String {
    format!("{s:?}")
}

fn assigns_to(e: &Expr, name: &str) -> bool {
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
            fallible: false,
            line: 0,
            diags: Vec::new(),
        };
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
                Item::Fn { name, params, fallible, line, .. } => {
                    if e.fns.contains_key(name) {
                        e.diag(*line, &format!("function '{name}' is defined twice"), None);
                    }
                    e.fns.insert(name.clone(), FnSig { params: params.clone(), fallible: *fallible });
                }
                Item::Test { .. } => {}
            }
        }
        e
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
        for it in items {
            match it {
                Item::Type { name, fields, line } => {
                    out.push_str(&format!("/*@L{line}*/#[derive(Debug, Clone, PartialEq, PartialOrd)]\npub struct {name} {{\n"));
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
                }
                Item::Fn { name, params, ret, fallible, body, line } => {
                    out.push_str(&self.function(name, params, ret, *fallible, body, *line));
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
            Stmt::Let { name, ty, value, mutable, .. } => {
                let v = self.expr(value);
                self.bind_pat(name);
                let p = self.pat(name);
                let t = ty.as_ref().map(|t| format!(": {}", rust_ty(t))).unwrap_or_default();
                format!("let {}{p}{t} = {v};", if *mutable { "mut " } else { "" })
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
                self.check_ident(n);
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) => format!("{}.{f}", self.place_mut(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place_mut(b)),
            Expr::Index(b, i) => {
                let b = self.place_mut(b);
                let i = self.expr(i);
                format!("(*{b}.ax_at_mut({i}))")
            }
            other => {
                self.diag(self.line, "this expression cannot be assigned to", Some("assign to a variable, field or index".into()));
                self.expr(other)
            }
        }
    }

    // A place to read through without cloning (method receivers, field bases).
    fn place(&mut self, e: &Expr) -> String {
        match e {
            Expr::Ident(n) if self.locals.contains(n) => {
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) => format!("{}.{f}", self.place(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place(b)),
            other => format!("({})", self.expr(other)),
        }
    }

    fn check_ident(&mut self, n: &str) {
        if self.locals.contains(n) || self.fns.contains_key(n) {
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
            Expr::Ident(n) => {
                if let Some((en, fields)) = self.variants.get(n).cloned() {
                    if fields.is_empty() {
                        return format!("{en}::{n}");
                    }
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
                    "BTreeMap::new()".into()
                } else {
                    let ps: Vec<String> = pairs.iter().map(|(k, v)| format!("({}, {})", self.expr(k), self.expr(v))).collect();
                    format!("BTreeMap::from([{}])", ps.join(", "))
                }
            }
            Expr::Tuple(items) => format!("({})", items.iter().map(|i| self.expr(i)).collect::<Vec<_>>().join(", ")),
            Expr::Unary(op, a) => format!("({op}{})", self.expr(a)),
            Expr::Binary(op, a, b) => {
                let (a, b) = (self.expr(a), self.expr(b));
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
                let r = if MUTATING_METHODS.contains(&name.as_str()) && matches!(recv.as_ref(), Expr::Index(..)) {
                    self.place_mut(recv)
                } else {
                    self.place(recv)
                };
                let a: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{{ ax_l({line}); {r}.ax_{name}({}) }}", a.join(", "))
            }
            Expr::Field(..) | Expr::TupleField(..) => format!("{}.clone()", self.place(e)),
            Expr::Index(b, i) => {
                let b = self.place(b);
                format!("{b}.ax_at({})", self.expr(i))
            }
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
                    other => format!("({})?", self.expr(other)),
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
                let it = self.expr(iter);
                self.bind_pat(p);
                let p = self.pat(p);
                let b = self.block_d(body, true);
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
        let s = self.expr(subj);
        let s = if has_str { format!("({s}).as_str()") } else { s };
        let mut out = format!("match {s} {{\n");
        for (p, body) in arms {
            self.bind_pat(p);
            let p = self.pat(p);
            let b = self.block_d(body, discard);
            out.push_str(&format!("        {p} => {b},\n"));
        }
        out.push_str("    }");
        out
    }

    fn call(&mut self, name: &str, args: &[(Option<String>, Expr)], tried: bool) -> String {
        if !tried && self.call_is_fallible(&Expr::Call(name.into(), vec![], 0)) {
            self.diag(self.line, &format!("{name}() can fail but its error is not handled"), Some(format!("write {name}(...)? and mark this function with !")));
        }
        let plain: Vec<String> = args.iter().map(|(_, a)| self.expr(a)).collect();
        match name {
            "print" | "read" | "write" | "args" | "input" | "stdin" | "lines" => return format!("{name}({})", plain.join(", ")),
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

    fn construct(&mut self, path: &str, name: &str, fields: &[Field], args: &[(Option<String>, Expr)], plain: Vec<String>) -> String {
        if fields.is_empty() && args.is_empty() {
            return path.to_string();
        }
        if fields.len() != args.len() {
            let want = fields.iter().map(|f| format!("{}: ...", f.name)).collect::<Vec<_>>().join(", ");
            self.diag(self.line, &format!("{name} has {} fields, got {}", fields.len(), args.len()), Some(format!("write {name}({want})")));
        }
        let mut parts = Vec::new();
        for (i, f) in fields.iter().enumerate() {
            let j = args.iter().position(|(n, _)| n.as_deref() == Some(f.name.as_str())).unwrap_or(i);
            if let Some(v) = plain.get(j) {
                parts.push(format!("{}: {v}", f.name));
            }
        }
        format!("{path} {{ {} }}", parts.join(", "))
    }
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}
