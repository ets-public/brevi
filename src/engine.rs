// The typed build's libraries on Brevi's own engine (the dynamic build's runtime) instead of CPython.
//
// A typed program (every .bv file, and .py files the typed build takes) that imports a library goes through
// pybridge.rs, whose values are `AxPy` and whose operations are `ax_pyb_*` calls. When every library it uses
// is one Brevi has natively (numpy, httpx, jinja2, polars, ...), those calls run on the dynamic build's
// runtime instead of CPython: engine_rt.rs implements the same calls over `brevi_dyn::V`, and the libraries
// themselves are compiled by the dynamic build into a `__dyn` module of the same program.
//
// The dynamic build refuses, at build time, any use of a library that its own copy lacks (`np.fft`, a
// polars method it has not got, an unknown tiktoken encoding). So that it sees the typed program's uses, the
// module it compiles is a stub that imports each library and repeats each library call the typed program
// makes, with the same names, keyword names and string literals, in a function that never runs. If the
// dynamic build refuses the stub, the program keeps CPython's libraries, as before.
use crate::ast::*;
use std::sync::Mutex;

// The library modules (top-level names) Brevi has its own copies of, plus its built-in modules.
const OWN: &[&str] = &[
    "numpy", "tiktoken", "tokenizers", "requests", "httpx", "anthropic", "openai", "jinja2", "markupsafe", "polars", "sqlite3", "duckdb", "safetensors", "onnxruntime", "pydantic", "textwrap", "io", "argparse",
];
const NATIVE: &[&str] = &[
    "sys", "math", "collections", "typing", "dataclasses", "re", "functools", "itertools", "datetime", "json", "calendar", "decimal", "heapq", "bisect", "string", "statistics", "time", "copy",
    "operator", "csv", "fractions", "enum", "abc", "hashlib", "base64", "binascii",
];

// (modules imported, (module, name) imported from them) -> the engine module, or None (CPython stays).
type Key = (Vec<String>, Vec<(String, String)>, Vec<String>);
static LAST: Mutex<Option<(Key, Option<String>)>> = Mutex::new(None);
static LIB: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Whether the last typed build put its libraries on the engine.
pub fn on() -> bool {
    ON.load(std::sync::atomic::Ordering::Relaxed)
}

// The engine's runtime library, for rustc's --extern (set once a build uses the engine).
pub fn lib() -> Option<&'static std::path::PathBuf> {
    if on() {
        LIB.get()
    } else {
        None
    }
}

fn off() -> bool {
    std::env::var("BREVI_ENGINE").is_ok_and(|v| v == "0")
}

// The Rust for the program's libraries on the engine (runtime calls + the dynamic build's module), or None to
// keep CPython. `mods` and `froms` are what pybridge bridges; `items` the parsed program.
pub fn glue(mods: &[String], froms: &[(String, String)], items: &[Item]) -> Option<String> {
    ON.store(false, std::sync::atomic::Ordering::Relaxed);
    // .py files have the whole dynamic build for these libraries (main.rs tries it first); this is for .bv.
    if off() || crate::PY_FILE.load(std::sync::atomic::Ordering::Relaxed) || !cfg!(unix) || crate::pybridge::no_native() || crate::caps::sandboxed() {
        return None;
    }
    let top = |m: &str| m.split('.').next().unwrap_or(m).to_string();
    let all: Vec<String> = mods.iter().cloned().chain(froms.iter().map(|(m, _)| m.clone())).collect();
    if all.is_empty() || !all.iter().all(|m| OWN.contains(&top(m).as_str()) || NATIVE.contains(&top(m).as_str())) {
        return None;
    }
    let calls = shapes(items);
    let key: Key = (mods.to_vec(), froms.to_vec(), calls.clone());
    if let Some((k, r)) = LAST.lock().unwrap().as_ref() {
        if *k == key {
            ON.store(r.is_some(), std::sync::atomic::Ordering::Relaxed);
            return r.clone();
        }
    }
    let r = build(mods, froms, &calls);
    let r = match r {
        Ok(x) => Some(x),
        Err(m) => {
            if std::env::var("BREVI_ENGINE_DEBUG").is_ok() {
                eprintln!("engine refused: {m}");
            }
            None
        }
    };
    ON.store(r.is_some(), std::sync::atomic::Ordering::Relaxed);
    *LAST.lock().unwrap() = Some((key, r.clone()));
    r
}

// The stub: each module as __bvN, each from-import as __bvfN, and every library call in a function that never runs.
fn stub(mods: &[String], froms: &[(String, String)], calls: &[String]) -> (String, Vec<(String, String)>, Vec<(String, String, String)>) {
    let mut src = String::new();
    let mut mtab = vec![];
    let mut ftab = vec![];
    let mut all: Vec<String> = mods.to_vec();
    for (m, _) in froms {
        if !all.contains(m) {
            all.push(m.clone());
        }
    }
    for c in calls {
        for m in c.split('\u{1}').skip(1).step_by(2) {
            if !all.iter().any(|x| x == m) {
                all.push(m.to_string());
            }
        }
    }
    for (i, m) in all.iter().enumerate() {
        let a = format!("__bv{i}");
        src.push_str(&format!("import {m} as {a}\n"));
        mtab.push((m.clone(), a));
    }
    for (i, (m, n)) in froms.iter().enumerate() {
        let a = format!("__bvf{i}");
        src.push_str(&format!("from {m} import {n} as {a}\n"));
        ftab.push((m.clone(), n.clone(), a));
    }
    if !calls.is_empty() {
        src.push_str("def __bv_shapes(_v):\n");
        for c in calls {
            // a module reference is written \x01module\x01
            let mut line = String::new();
            let mut rest = c.as_str();
            while let Some(k) = rest.find('\u{1}') {
                line.push_str(&rest[..k]);
                let after = &rest[k + 1..];
                let end = after.find('\u{1}').unwrap_or(after.len());
                let m = &after[..end];
                line.push_str(&mtab.iter().find(|(x, _)| x == m).map_or("_v".to_string(), |(_, a)| a.clone()));
                rest = &after[(end + 1).min(after.len())..];
            }
            line.push_str(rest);
            src.push_str(&format!("    {line}\n"));
        }
    }
    (src, mtab, ftab)
}

fn build(mods: &[String], froms: &[(String, String)], calls: &[String]) -> Result<String, String> {
    let (src, mtab, ftab) = stub(mods, froms, calls);
    let dir = std::env::temp_dir().join(format!("brevi-engine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("__bv_libs.py");
    std::fs::write(&file, &src).map_err(|e| e.to_string())?;
    let r = crate::dynpy::to_rust(&file.to_string_lossy());
    std::fs::remove_dir_all(&dir).ok();
    let rust = r.map_err(|(_, l, m)| if std::env::var("BREVI_ENGINE_DEBUG").is_ok() { format!("{m} (stub line {l}):\n{src}") } else { m })?;
    if !crate::dynpy::bridged().is_empty() {
        return Err(format!("{} would run in CPython", crate::dynpy::bridged().join(", ")));
    }
    let lib = crate::dynpy::runtime_lib_path().map_err(|e| e.to_string())?;
    LIB.set(lib).ok();
    let body = rust.strip_prefix(crate::dynpy::HEADER).ok_or("unexpected dynamic build header")?;
    // The dynamic build's main runs the module bodies; here they run before the first library call.
    let (head, tail) = body.rsplit_once("fn main() { ").ok_or("no main in the dynamic build")?;
    let tail = tail.replacen("run_main(__run); }", "ARGV.with(|a| *a.borrow_mut() = std::env::args().collect()); init_builtin_classes(); __run() }", 1);
    let mut out = String::from("\nmod __dyn {\n#![allow(warnings)]\nuse brevi_dyn::*; use std::rc::Rc; use std::cell::{Cell, RefCell};\n");
    out.push_str(head);
    out.push_str("pub fn __bv_main() -> Result<(), E> { ");
    out.push_str(&tail);
    out.push_str("\npub fn __bv_mod(m: &str) -> Option<R> {\n    let k = match m {\n");
    for (m, a) in &mtab {
        out.push_str(&format!("        {m:?} => {a:?},\n"));
    }
    out.push_str("        _ => return None,\n    };\n    Some(gl(gtab___main__(k)?, k))\n}\n");
    out.push_str("pub fn __bv_from(m: &str, n: &str) -> Option<R> {\n    let k = match (m, n) {\n");
    for (m, n, a) in &ftab {
        out.push_str(&format!("        ({m:?}, {n:?}) => {a:?},\n"));
    }
    out.push_str("        _ => return None,\n    };\n    Some(gl(gtab___main__(k)?, k))\n}\n}\n");
    out.push_str(include_str!("engine_rt.rs"));
    Ok(out)
}

// ------------------------------------------------------------------------------------------- the stub's calls
// Every library call and attribute the typed program makes, as one line of Python each: modules as
// \x01module\x01, other values `_v`, string literals kept (tiktoken.get_encoding("cl100k_base")).
fn shapes(items: &[Item]) -> Vec<String> {
    let mut out = vec![];
    for it in items {
        match it {
            Item::Fn { body, .. } | Item::Test { body, .. } => walk(body, &mut out),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

fn strip(e: &Expr) -> &Expr {
    match e {
        Expr::Try(x) => strip(x),
        x => x,
    }
}

fn lit(e: &Expr) -> Option<&str> {
    match strip(e) {
        Expr::Str(p) if p.len() == 1 => match &p[0] {
            StrPart::Lit(s) => Some(s),
            _ => None,
        },
        Expr::Str(p) if p.is_empty() => Some(""),
        _ => None,
    }
}

// The receiver as the stub writes it: a module path, or `_v`.
fn recv(e: &Expr) -> String {
    match strip(e) {
        Expr::Call(f, a, _) if f == "ax_pyb_mod" && a.len() == 1 => lit(&a[0].1).map_or("_v".into(), |m| format!("\u{1}{m}\u{1}")),
        Expr::Call(f, a, _) if f == "ax_pyb_from" && a.len() == 2 => match (lit(&a[0].1), lit(&a[1].1)) {
            (Some(m), Some(n)) => format!("\u{1}{m}\u{1}.{n}"),
            _ => "_v".into(),
        },
        Expr::Call(f, a, _) if f == "ax_pyb_get" && a.len() == 2 => {
            let r = recv(&a[0].1);
            match lit(&a[1].1) {
                Some(n) if r != "_v" => format!("{r}.{n}"),
                _ => "_v".into(),
            }
        }
        _ => "_v".into(),
    }
}

fn arg(e: &Expr) -> String {
    match lit(e) {
        Some(s) => format!("{s:?}").replace("\\u{", "\\U000").replace('}', ""),
        None => match strip(e) {
            Expr::Int(n) => n.to_string(),
            _ => "_v".into(),
        },
    }
}

fn ident_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// A call's receiver chain (`np.random.default_rng`): its gets are part of the call's line, not lines of their own.
fn walk_recv(e: &Expr, out: &mut Vec<String>) {
    match strip(e) {
        Expr::Call(f, a, _) if f == "ax_pyb_get" && a.len() == 2 => walk_recv(&a[0].1, out),
        Expr::Call(f, _, _) if f == "ax_pyb_mod" || f == "ax_pyb_from" => {}
        x => walk(x, out),
    }
}

fn walk(e: &Expr, out: &mut Vec<String>) {
    if let Expr::Call(f, a, _) = e {
        if f == "ax_pyb_call" && a.len() == 4 {
            if let (Some(name), Expr::List(pos), Expr::List(kw)) = (lit(&a[1].1), strip(&a[2].1), strip(&a[3].1)) {
                let r = recv(&a[0].1);
                let mut args: Vec<String> = pos.iter().map(arg).collect();
                for k in kw {
                    if let Expr::Tuple(kv) = k {
                        if let (Some(k), Some(v)) = (kv.first().and_then(lit), kv.get(1)) {
                            if ident_ok(k) {
                                args.push(format!("{k}={}", arg(v)));
                            }
                        }
                    }
                }
                let callee = if name.is_empty() { r.clone() } else if ident_ok(name) && !name.starts_with("__") { format!("{r}.{name}") } else { String::new() };
                if !callee.is_empty() && callee != "_v" {
                    out.push(format!("{callee}({})", args.join(", ")));
                }
                walk_recv(&a[0].1, out);
                a[2..].iter().for_each(|(_, x)| walk(x, out));
                return;
            }
        }
        if f == "ax_pyb_get" && a.len() == 2 {
            let r = recv(e);
            if r != "_v" {
                out.push(r);
            } else if let Some(n) = lit(&a[1].1).filter(|n| ident_ok(n) && !n.starts_with("__")) {
                out.push(format!("_v.{n}"));
            }
            walk_recv(&a[0].1, out);
            return;
        }
    }
    each(e, &mut |x| walk(x, out));
}

// Calls f on each direct sub-expression of e.
fn each(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    match e {
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Move(_) => {}
        Expr::Str(ps) => {
            for p in ps {
                if let StrPart::Interp(x) = p {
                    f(x);
                }
            }
        }
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| f(x)),
        Expr::Map(ps) => ps.iter().for_each(|(k, v)| {
            f(k);
            f(v)
        }),
        Expr::Unary(_, x) | Expr::Field(x, _) | Expr::TupleField(x, _) | Expr::Try(x) | Expr::Lambda(_, x) => f(x),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::While(a, b) | Expr::Range(a, b) => {
            f(a);
            f(b)
        }
        Expr::Call(_, a, _) => a.iter().for_each(|(_, x)| f(x)),
        Expr::Method(r, _, a, _) => {
            f(r);
            a.iter().for_each(|x| f(x))
        }
        Expr::If(c, a, b) => {
            f(c);
            f(a);
            if let Some(b) = b {
                f(b)
            }
        }
        Expr::Match(x, arms) => {
            f(x);
            arms.iter().for_each(|(_, a)| f(a))
        }
        Expr::For(_, a, b) => {
            f(a);
            f(b)
        }
        Expr::Block(ss, last) => {
            for s in ss {
                match s {
                    Stmt::Let { value, .. } => f(value),
                    Stmt::Assign { target, value, .. } => {
                        f(target);
                        f(value)
                    }
                    Stmt::Expr { expr, .. } => f(expr),
                    Stmt::Return { value: Some(v), .. } => f(v),
                    Stmt::Assert { cond, .. } => f(cond),
                    _ => {}
                }
            }
            if let Some(l) = last {
                f(l)
            }
        }
    }
}
