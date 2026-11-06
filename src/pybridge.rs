// Python libraries Brevi does not build itself (numpy, pandas, requests, sqlite3, argparse, ...), run by CPython
// inside the built program.
//
// `scan` reads the program's import lines. A module Brevi has built in (math, re, json, collections, ...) stays
// Brevi's; any other module is "bridged". The parser asks this file about each name and value it reads:
//   - a bridged module (`np`), a name imported from one (`from itertools import combinations`), and every value
//     made from them (`a = np.array(xs)`, `a.sum()`, `a[1:]`, `a * 2`) is a Python value, type `Py` in the
//     built program (see pyrt.rs);
//   - names assigned such values are "held" (per function), so their later uses are Python values too. The
//     parser runs again while new names are found (`parse_program`), so uses before the assignment count.
// Each operation on a Python value becomes a call into the runtime (`ax_pyb_call(o, "sum", [args], [kwargs])`,
// `ax_pyb_op("+", a, b)`, ...), which runs it in CPython, so the result is CPython's. Brevi values passed in are
// converted (lists to lists, dicts to dicts, ...); Python values come back as Brevi values only through
// int(), float(), str(), bool() and len().
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

// Modules the parser maps onto Brevi's own library (see the lexer's PY_BUILTIN_MODULES and PY_ALIAS_MODULES).
const NATIVE: &[&str] = &[
    "sys", "math", "collections", "typing", "dataclasses", "re", "functools", "itertools", "datetime", "json", "calendar", "decimal", "heapq", "bisect", "string",
    "statistics", "random", "time", "copy", "operator", "csv", "threading", "__future__", "fractions", "enum", "abc", "hashlib", "base64", "binascii",
];
// GUI libraries (gui.rs handles tkinter; the others are refused in modules.rs).
const NOT_BRIDGED: &[&str] = &["tkinter", "Tkinter", "turtle", "pygame", "PyQt5", "PyQt6", "PySide2", "PySide6", "wx", "kivy", "customtkinter", "pyglet", "arcade", "dearpygui"];

#[derive(Default, Clone)]
struct Reg {
    // alias -> module: `import numpy as np` gives np -> numpy, `import os.path` gives os -> os.
    mods: HashMap<String, String>,
    // name -> (module, attribute): `from itertools import combinations as comb`.
    names: HashMap<String, (String, String)>,
    // (function, name) of names holding Python values; "" is the top level.
    held: HashSet<(String, String)>,
    // (function, name) of names a function sets (so a top-level held name is not its local).
    set: HashSet<(String, String)>,
    // Functions that return Python values.
    rets: HashSet<String>,
    // (function, name) of comprehension names over Brevi values (`[i * 3 for i in range(n)]`): never held, so a
    // name reused for both stays a Brevi value (a Python value still works there through its Rust operators).
    comp: HashSet<(String, String)>,
    // Parameters (function, position) that are passed Python values.
    params: HashSet<(String, usize)>,
    // The function being parsed and its parameters; the class being parsed.
    scope: Vec<String>,
    class: Option<String>,
    grew: bool,
}

thread_local! {
    static REG: RefCell<Option<Reg>> = const { RefCell::new(None) };
    // Modules that are files of the program (`import stock` with stock.py beside it), from modules.rs.
    static LOCAL: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    // (module, name) of Brevi's built-in modules that Brevi lacks (`math.gcd`, `from itertools import
    // combinations`), learned from the build's errors (see learn): CPython runs those.
    static EXTRA: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
    // Built-in modules the program imports: name in the code -> module.
    static NATIVE_MODS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    // `import urllib.parse`: submodules imported along with their package.
    static SUBS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

// --sandbox (or an allow line) without `allow native`: CPython must not run any of the program. A Python value
// reaches everything CPython can (`x.__class__.__mro__[1].__subclasses__()`, `sys.modules["os"]`), so there is
// no safe subset: bridged imports are refused by caps.rs, nothing is learned, and compile_once refuses a build
// that still needs the bridge.
pub static NO_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn no_native() -> bool {
    NO_NATIVE.load(std::sync::atomic::Ordering::Relaxed)
}

// Whether CPython may run Python module m (or its package's) for this program: not under the sandbox without
// allow native, and under `--sandbox --allow-py` only the modules the caller named (caps.rs).
pub fn py_allowed(m: &str) -> bool {
    if no_native() {
        return false;
    }
    let root = m.split('.').next().unwrap_or(m);
    crate::caps::py_only().map_or(true, |l| l.iter().any(|x| x == root))
}

thread_local! {
    // What learn would have handed to CPython while NO_NATIVE is set: (module, name).
    static BLOCKED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn blocked() -> Vec<(String, String)> {
    BLOCKED.with(|b| b.borrow().clone())
}

// The build errors that only say a blocked call is missing (`unknown method .seed()`, `unknown name 'random'`
// on `random.seed(1)`), which the one "needs allow native" error replaces.
pub fn blocked_noise(d: &crate::diag::Diag) -> bool {
    let b = blocked();
    let named = |pre: &str, post: &str| d.msg.strip_prefix(pre).and_then(|r| r.strip_suffix(post)).map(String::from);
    if let Some(f) = named("unknown method .", "()").or_else(|| named("unknown function ", "()")) {
        return b.iter().any(|(_, n)| *n == f);
    }
    if let Some(n) = named("unknown name '", "'") {
        let m = NATIVE_MODS.with(|x| x.borrow().get(&n).cloned());
        return b.iter().any(|(bm, bn)| Some(bm) == m.as_ref() || *bn == n);
    }
    false
}

// The import lines of Python libraries CPython would run (not Brevi's built-in modules, not files of the
// program): (line, module), for caps.rs.
pub fn bridged_imports(src: &str) -> Vec<(usize, String)> {
    let mut out = vec![];
    for (i, l) in src.lines().enumerate() {
        let t = l.split(" #").next().unwrap_or("").trim();
        let mods: Vec<&str> = if let Some(rest) = t.strip_prefix("import ") {
            rest.split(',').filter_map(|p| p.split_whitespace().next()).collect()
        } else if let Some(rest) = t.strip_prefix("from ") {
            rest.split_once(" import ").map(|(m, _)| vec![m.trim()]).unwrap_or_default()
        } else {
            vec![]
        };
        for m in mods {
            if ok_name(m) && bridged(m) {
                out.push((i + 1, m.to_string()));
            }
        }
    }
    out
}

fn extra(m: &str, a: &str) -> bool {
    EXTRA.with(|x| x.borrow().contains(&(m.to_string(), a.to_string())))
}

// `math.gcd` with gcd learned as missing: `math` as a Python module (the parser's hook before `.`).
pub fn native_attr(e: &Expr, attr: &str, line: usize) -> Option<Expr> {
    let Expr::Ident(n) = e else { return None };
    let m = NATIVE_MODS.with(|x| x.borrow().get(n).cloned())?;
    if extra(&m, attr) {
        Some(tcall("ax_pyb_mod", vec![s(&m)], line))
    } else {
        None
    }
}

// Errors of a build that Python's module would not have: a function of a built-in module Brevi lacks
// (`unknown method .gcd()` on `math.gcd(a, b)`, `unknown function reduce()` after `from functools import
// reduce`). Each is noted to be run by CPython; true when something new was noted (build again).
pub fn learn(diags: &[crate::diag::Diag], src: &str) -> bool {
    let lines: Vec<&str> = src.lines().collect();
    let mods = NATIVE_MODS.with(|x| x.borrow().clone());
    // from-imports of built-in modules: name -> (module, real name)
    let mut froms: HashMap<String, (String, String)> = HashMap::new();
    for l in &lines {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("from ") {
            if let Some((m, names)) = rest.split_once(" import ") {
                let m = m.trim();
                if NATIVE.contains(&m.split('.').next().unwrap_or(m)) {
                    for part in names.trim_matches(|c| c == '(' || c == ')' || c == ' ').split(',') {
                        let mut w = part.split_whitespace();
                        let n = w.next().unwrap_or("");
                        let a = if w.next() == Some("as") { w.next().unwrap_or(n) } else { n };
                        if !n.is_empty() {
                            froms.insert(a.to_string(), (m.to_string(), n.to_string()));
                        }
                    }
                }
            }
        }
    }
    if mods.is_empty() && froms.is_empty() {
        return false;
    }
    let mut new = vec![];
    let uses = |l: &str, m: &str| -> Vec<String> {
        // `m.attr` on the line (not after another dot).
        let mut out = vec![];
        let b = l.as_bytes();
        let mut i = 0;
        while let Some(k) = l[i..].find(&format!("{m}.")) {
            let at = i + k;
            let before_ok = at == 0 || !(b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_' || b[at - 1] == b'.');
            let rest = &l[at + m.len() + 1..];
            let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
            if before_ok && end > 0 {
                out.push(rest[..end].to_string());
            }
            i = at + m.len() + 1;
        }
        out
    };
    let mut methods = diags.iter().any(|d| d.msg.starts_with("unknown method .") || d.msg.starts_with("unknown function "));
    // A second time through, only when the methods taught nothing: the unknown names.
    for (i, d) in diags.iter().chain(diags.iter()).enumerate() {
        if i == diags.len() {
            if !new.is_empty() || !methods {
                break;
            }
            methods = false;
        }
        let Some(l) = lines.get(d.line.wrapping_sub(1)) else { continue };
        let l = l.split(" #").next().unwrap_or("");
        let named = |pre: &str, post: &str| d.msg.strip_prefix(pre).and_then(|r| r.strip_suffix(post)).map(String::from);
        if let Some(f) = named("unknown method .", "()") {
            for (alias, m) in &mods {
                if uses(l, alias).contains(&f) {
                    new.push((m.clone(), f.clone()));
                }
            }
            // `datetime.strptime(...)` after `from datetime import datetime`: the class is CPython's.
            for (alias, (m, real)) in &froms {
                if uses(l, alias).contains(&f) {
                    new.push((m.clone(), real.clone()));
                }
            }
            // `d.strftime(...)` on a value a built-in module made: when that module's CPython classes have the
            // method, the program uses all of the module from CPython.
            if new.is_empty() {
                let mut ms: Vec<&String> = mods.values().chain(froms.values().map(|(m, _)| m)).collect();
                ms.sort();
                ms.dedup();
                for m in ms {
                    if class_has(m, &f) {
                        for (alias, (fm, real)) in &froms {
                            if fm == m {
                                let _ = alias;
                                new.push((m.clone(), real.clone()));
                            }
                        }
                        for (alias, am) in &mods {
                            if am == m {
                                for l in &lines {
                                    for a in uses(l.split(" #").next().unwrap_or(""), alias) {
                                        new.push((m.clone(), a));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if let Some(f) = named("unknown function ", "()") {
            if let Some((m, real)) = froms.get(&f) {
                new.push((m.clone(), real.clone()));
            }
        } else if let Some(n) = named("unknown name '", "'").filter(|_| !methods) {
            // `unknown name 'math'` also follows each `unknown method .gcd()` on the line: those are learned
            // first, so that `math.sqrt` next to `math.gcd` stays native; a name still unknown after they are
            // bridged (`math.pi`) is learned on the next round.
            if let Some(m) = mods.get(&n) {
                for a in uses(l, &n) {
                    new.push((m.clone(), a));
                }
            } else if let Some((m, real)) = froms.get(&n) {
                new.push((m.clone(), real.clone()));
            }
        }
    }
    if std::env::var("BREVI_PYB_DEBUG").is_ok() {
        for d in diags {
            eprintln!("pybridge diag: line {}: {}", d.line, d.msg);
        }
        eprintln!("pybridge learn: {new:?}");
    }
    let (new, blocked): (Vec<_>, Vec<_>) = new.into_iter().partition(|(m, _)| py_allowed(m));
    if !blocked.is_empty() {
        BLOCKED.with(|b| b.borrow_mut().extend(blocked));
        return false;
    }
    EXTRA.with(|x| {
        let mut x = x.borrow_mut();
        let before = x.len();
        x.extend(new);
        x.len() > before
    })
}

// Whether a public class of CPython's module m has attribute f (asked of python3).
fn class_has(m: &str, f: &str) -> bool {
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let q = "import sys, importlib\nm = importlib.import_module(sys.argv[1])\nprint(any(isinstance(getattr(m, n, None), type) and hasattr(getattr(m, n), sys.argv[2]) for n in dir(m) if not n.startswith('_')))";
    std::process::Command::new(exe).args(["-c", q, m, f]).output().is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "True")
}

// What CPython runs for the program, for the path report: bridged modules (`numpy`) and the functions of
// Brevi's own modules that it lacks (`math.gcd`), sorted.
pub fn uses() -> Vec<String> {
    let mut v: Vec<String> = reg(|r| r.mods.values().cloned().chain(r.names.values().filter(|(m, _)| bridged(m)).map(|(m, _)| m.clone())).collect()).unwrap_or_default();
    v.extend(EXTRA.with(|x| x.borrow().iter().map(|(m, n)| format!("{m}.{n}")).collect::<Vec<_>>()));
    v.sort();
    v.dedup();
    v
}

// The program's CPython calls are all functions of Brevi's own modules (`math.gcd`, `itertools.product`), not
// libraries Brevi lacks: the dynamic build, which has them natively, is tried first.
pub fn stdlib_only() -> bool {
    let u = uses();
    // os and pathlib too: the dynamic build has their path text (and refuses a program that needs more).
    !u.is_empty() && u.iter().all(|m| NATIVE.contains(&m.split('.').next().unwrap_or(m)) || matches!(m.split('.').next().unwrap_or(m), "os" | "posixpath" | "pathlib" | "io" | "textwrap" | "argparse"))
}

// The program's CPython calls are numpy (and Brevi's own modules): the dynamic build, with Brevi's own numpy,
// is tried first (it refuses what that numpy lacks, and the typed build with CPython's numpy goes instead).
pub fn numpy_only() -> bool {
    let u = uses();
    let own = |top: &str| matches!(top, "numpy" | "tiktoken" | "tokenizers" | "requests" | "httpx" | "anthropic" | "openai");
    u.iter().any(|m| own(m.split('.').next().unwrap_or(m)))
        && u.iter().all(|m| {
            let top = m.split('.').next().unwrap_or(m);
            own(top) || NATIVE.contains(&top) || matches!(top, "os" | "posixpath" | "pathlib" | "io" | "textwrap" | "argparse")
        })
        && std::env::var("BREVI_NUMPY").map_or(true, |v| v != "0")
}

pub fn active() -> bool {
    REG.with(|r| r.borrow().is_some())
}

pub fn note_local(module: &str) {
    LOCAL.with(|l| l.borrow_mut().insert(module.split('.').next().unwrap_or(module).to_string()));
}

fn bridged(m: &str) -> bool {
    let top = m.split('.').next().unwrap_or(m);
    !NATIVE.contains(&top) && !NOT_BRIDGED.contains(&top) && !LOCAL.with(|l| l.borrow().contains(top)) && !m.starts_with('.') && !m.is_empty()
}

fn ok_name(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

// Reads the import lines. Clears the registry for a program that uses no bridged module.
pub fn scan(src: &str) {
    let mut r = Reg::default();
    let mut any = false;
    let mut natives: HashMap<String, String> = HashMap::new();
    let mut open: Option<String> = None;
    for l in src.lines() {
        let t = l.split(" #").next().unwrap_or("").trim();
        // `from m import (a,` ... `b)`
        if let Some(m) = open.clone() {
            for part in t.trim_matches(|c| c == '(' || c == ')').split(',') {
                add_from(&mut r, &m, part);
            }
            if t.contains(')') {
                open = None;
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("import ") {
            for part in rest.split(',') {
                let mut w = part.split_whitespace();
                let m = w.next().unwrap_or("");
                if ok_name(m) && NATIVE.contains(&m) {
                    // The lexer reads `import datetime as dt` as datetime throughout.
                    natives.insert(m.to_string(), m.to_string());
                    any |= EXTRA.with(|x| x.borrow().iter().any(|(xm, _)| xm == m));
                }
                if !ok_name(m) || !bridged(m) {
                    continue;
                }
                any = true;
                if m.contains('.') {
                    SUBS.with(|x| {
                        let mut x = x.borrow_mut();
                        if !x.iter().any(|y| y == m) {
                            x.push(m.to_string());
                        }
                    });
                }
                match (w.next(), w.next()) {
                    (Some("as"), Some(a)) => r.mods.insert(a.to_string(), m.to_string()),
                    _ => {
                        let top = m.split('.').next().unwrap_or(m);
                        r.mods.insert(top.to_string(), top.to_string())
                    }
                };
            }
        } else if let Some(rest) = t.strip_prefix("from ") {
            let Some((m, names)) = rest.split_once(" import ") else { continue };
            let m = m.trim();
            if ok_name(m) && NATIVE.contains(&m) {
                natives.insert(m.to_string(), m.to_string());
                for part in names.trim_matches(|c| c == '(' || c == ')' || c == ' ').split(',') {
                    let n = part.split_whitespace().next().unwrap_or("");
                    if extra(m, n) {
                        any = true;
                        add_from(&mut r, m, part);
                    }
                }
                any |= EXTRA.with(|x| x.borrow().iter().any(|(xm, _)| xm == m));
                continue;
            }
            if !ok_name(m) || !bridged(m) {
                continue;
            }
            any = true;
            let names = names.trim();
            if names.starts_with('(') && !names.contains(')') {
                open = Some(m.to_string());
            }
            for part in names.trim_matches(|c| c == '(' || c == ')').split(',') {
                add_from(&mut r, m, part);
            }
        }
    }
    NATIVE_MODS.with(|x| *x.borrow_mut() = natives);
    REG.with(|x| *x.borrow_mut() = if any { Some(r) } else { None });
}

fn add_from(r: &mut Reg, m: &str, part: &str) {
    let mut w = part.split_whitespace();
    let n = w.next().unwrap_or("");
    if n.is_empty() || n == "*" {
        return;
    }
    let alias = if w.next() == Some("as") { w.next().unwrap_or(n) } else { n };
    r.names.insert(alias.to_string(), (m.to_string(), n.to_string()));
}

fn reg<T>(f: impl FnOnce(&Reg) -> T) -> Option<T> {
    REG.with(|r| r.borrow().as_ref().map(f))
}

fn reg_mut(f: impl FnOnce(&mut Reg)) {
    REG.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            f(r)
        }
    })
}

// Before each parse: start at the top level, keep what earlier parses found.
pub fn start_parse() {
    reg_mut(|r| {
        r.scope.clear();
        r.class = None;
        r.grew = false;
    });
}

// Whether the last parse found new names holding Python values.
pub fn grew() -> bool {
    reg(|r| r.grew).unwrap_or(false)
}

fn scope(r: &Reg) -> String {
    r.scope.last().cloned().unwrap_or_default()
}

// The parser is reading function `name` (a method when inside a class).
pub fn enter_fn(name: &str, params: &[Param]) {
    reg_mut(|r| {
        let s = match &r.class {
            Some(c) if r.scope.is_empty() => format!("{c}_{name}"),
            _ => name.to_string(),
        };
        // `self` is not counted: callers write `obj.m(x)`, whose first argument is x.
        let skip = if r.class.is_some() && r.scope.is_empty() && params.first().is_some_and(|p| p.name == "self_" || p.name == "self") { 1 } else { 0 };
        for (i, p) in params.iter().enumerate() {
            r.set.insert((s.clone(), p.name.clone()));
            if i >= skip && r.params.contains(&(s.clone(), i - skip)) && r.held.insert((s.clone(), p.name.clone())) {
                r.grew = true;
            }
        }
        r.scope.push(s);
    })
}

pub fn leave_fn() {
    reg_mut(|r| {
        r.scope.pop();
    })
}

pub fn enter_class(name: &str) {
    reg_mut(|r| r.class = Some(name.to_string()))
}

pub fn leave_class() {
    reg_mut(|r| r.class = None)
}

fn held_in(r: &Reg, s: &str, n: &str) -> bool {
    if r.comp.contains(&(s.to_string(), n.to_string())) {
        return false;
    }
    r.held.contains(&(s.to_string(), n.to_string())) || (!s.is_empty() && !r.set.contains(&(s.to_string(), n.to_string())) && r.held.contains(&(String::new(), n.to_string())))
}

fn hold(n: &str) {
    reg_mut(|r| {
        let s = scope(r);
        r.set.insert((s.clone(), n.to_string()));
        if r.held.insert((s, n.to_string())) {
            r.grew = true;
        }
    })
}

fn pat_names(p: &Pat, out: &mut Vec<String>) {
    match p {
        Pat::Bind(n) => out.push(n.clone()),
        Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| pat_names(p, out)),
        _ => {}
    }
}

fn target_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident(n) | Expr::Move(n) => out.push(n.clone()),
        Expr::Tuple(xs) | Expr::List(xs) => xs.iter().for_each(|x| target_names(x, out)),
        _ => {}
    }
}

// `x = value`: x holds a Python value if the value is one. (Every name set is noted, for scope.)
pub fn note_assign(target: &Expr, value: &Expr) {
    if !active() {
        return;
    }
    let mut ns = vec![];
    target_names(target, &mut ns);
    let py = is_py(value) || matches!(value, Expr::Tuple(xs) if xs.iter().any(is_py));
    // `self.model = ...` in a class: the field holds Python values.
    if let Expr::Field(b, f) = target {
        if py && matches!(b.as_ref(), Expr::Ident(x) if x == "self_" || x == "self") {
            reg_mut(|r| {
                if let Some(c) = r.class.clone() {
                    if r.held.insert((format!("{c}."), f.clone())) {
                        r.grew = true;
                    }
                }
            });
        }
    }
    for n in ns {
        if py {
            hold(&n);
        } else {
            reg_mut(|r| {
                let s = scope(r);
                r.set.insert((s, n.clone()));
            });
        }
    }
}

// `for p in iter`: the loop names hold Python values when iter is one.
pub fn note_for(p: &Pat, iter: &Expr) {
    if !active() {
        return;
    }
    let mut ns = vec![];
    pat_names(p, &mut ns);
    for n in ns {
        if is_py(iter) {
            hold(&n);
        } else {
            reg_mut(|r| {
                let s = scope(r);
                r.set.insert((s, n.clone()));
            });
        }
    }
}

// A comprehension's names over a Brevi value.
pub fn note_comp_native(names: &[String]) {
    reg_mut(|r| {
        let s = scope(r);
        for n in names {
            if r.comp.insert((s.clone(), n.clone())) && r.held.contains(&(s.clone(), n.clone())) {
                r.grew = true;
            }
        }
    })
}

pub fn note_return(value: &Expr) {
    if active() && is_py(value) {
        reg_mut(|r| {
            let s = scope(r);
            if r.rets.insert(s) {
                r.grew = true;
            }
        })
    }
}

// Whether an expression is a Python value.
pub fn is_py(e: &Expr) -> bool {
    match e {
        Expr::Try(x) => is_py(x),
        Expr::Call(f, _, _) => {
            matches!(f.as_str(), "ax_pyb_mod" | "ax_pyb_from" | "ax_pyb_get" | "ax_pyb_call" | "ax_pyb_idx" | "ax_pyb_op" | "ax_pyb_unary" | "ax_pyb_builtin" | "ax_pyb" | "ax_pyb_none" | "ax_pyb_slice") || reg(|r| r.rets.contains(f)).unwrap_or(false)
        }
        Expr::Ident(n) | Expr::Move(n) => reg(|r| held_in(r, &scope(r), n)).unwrap_or(false),
        Expr::Field(b, f) => matches!(b.as_ref(), Expr::Ident(s) if s == "self_" || s == "self") && reg(|r| r.class.as_ref().is_some_and(|c| r.held.contains(&(format!("{c}."), f.clone())))).unwrap_or(false),
        _ => false,
    }
}

// The lexer renames Python names that are Brevi words (`type` is `type_`); Python gets them back.
pub fn unkw(n: &str) -> String {
    match n.strip_suffix('_') {
        Some(b) if matches!(b, "as" | "yield" | "try" | "self" | "type" | "match" | "test" | "fn" | "let" | "var" | "enum" | "mut") => b.to_string(),
        // Rust words too: `np.where` is lexed as `where_`.
        _ => crate::lexer::py_ident(n).to_string(),
    }
}

// `a OP b` with a Python value on either side (the parser's hook): Python's operator.
pub fn binop(op: &str, negate: bool, a: &Expr, b: &Expr, line: usize) -> Option<Expr> {
    if !active() || matches!(op, ".." | "and" | "or") || !(is_py(a) || is_py(b)) {
        return None;
    }
    let none = |e: &Expr| matches!(e, Expr::Call(f, x, _) if f == "none" && x.is_empty());
    let op = match (op, negate, none(a) || none(b)) {
        ("in", true, _) => "not in",
        ("==", _, true) => "is",
        ("!=", _, true) => "is not",
        (o, _, _) => o,
    };
    Some(tcall("ax_pyb_op", vec![s(op), to_py(a.clone(), line), to_py(b.clone(), line)], line))
}

fn s(x: &str) -> Expr {
    Expr::Str(vec![StrPart::Lit(x.to_string())])
}

fn call(f: &str, args: Vec<Expr>, line: usize) -> Expr {
    Expr::Call(f.into(), args.into_iter().map(|a| (None, a)).collect(), line)
}

fn tcall(f: &str, args: Vec<Expr>, line: usize) -> Expr {
    Expr::Try(Box::new(call(f, args, line)))
}

// A value given to Python: a Python value as it is, a Brevi value converted.
fn to_py(e: Expr, line: usize) -> Expr {
    if is_py(&e) {
        return e;
    }
    match e {
        Expr::Call(f, a, _) if f == "none" && a.is_empty() => call("ax_pyb_none", vec![], line),
        // `"abc".encode()` given to a library: Python's bytes (Brevi has none).
        Expr::Method(x, m, a, l) if m == "encode" && a.len() <= 1 => {
            let a: Vec<(Option<String>, Expr)> = a.into_iter().map(|x| (None, x)).collect();
            member(to_py(*x, l), "encode", Some(a), l)
        }
        Expr::List(xs) if xs.is_empty() => tcall("ax_pyb_builtin", vec![s("list"), Expr::List(vec![]), Expr::List(vec![])], line),
        Expr::Map(xs) if xs.is_empty() => tcall("ax_pyb_builtin", vec![s("dict"), Expr::List(vec![]), Expr::List(vec![])], line),
        // List, tuple and dict literals are converted item by item (their items may differ in kind).
        Expr::List(xs) => call("ax_pyb", vec![Expr::List(xs.into_iter().map(|x| to_py(x, line)).collect())], line),
        Expr::Tuple(xs) if xs.len() <= 6 => call("ax_pyb", vec![Expr::Tuple(xs.into_iter().map(|x| to_py(x, line)).collect())], line),
        Expr::Tuple(xs) => tcall("ax_pyb_builtin", vec![s("tuple"), Expr::List(vec![to_py(Expr::List(xs), line)]), Expr::List(vec![])], line),
        Expr::Map(ps) => {
            let pairs = ps.into_iter().map(|(k, v)| Expr::Tuple(vec![to_py(k, line), to_py(v, line)])).collect();
            tcall("ax_pyb_builtin", vec![s("dict"), Expr::List(vec![call("ax_pyb", vec![Expr::List(pairs)], line)]), Expr::List(vec![])], line)
        }
        // `type=int`, `key=len`: the parser made the built-in a lambda; Python gets its own built-in.
        Expr::Lambda(ps, body) if ps.len() == 1 && matches!(body.as_ref(), Expr::Method(r, m, a, _) if a.is_empty() && matches!(r.as_ref(), Expr::Ident(v) if *v == ps[0]) && matches!(m.as_str(), "int" | "float" | "str" | "len")) => {
            let Expr::Method(_, m, _, _) = *body else { unreachable!() };
            from_name("builtins", &m, line)
        }
        Expr::Ident(n) if matches!(n.as_str(), "list" | "dict" | "set" | "tuple" | "bool" | "abs" | "sorted" | "max" | "min" | "sum" | "repr" | "print") => from_name("builtins", &n, line),
        Expr::Call(f, a, _) if f == "__truediv" && a.len() == 2 && (is_py(&a[0].1) || is_py(&a[1].1)) => {
            let mut a = a.into_iter();
            let (x, y) = (a.next().unwrap().1, a.next().unwrap().1);
            tcall("ax_pyb_op", vec![s("/"), to_py(x, line), to_py(y, line)], line)
        }
        // A name of the program's: given by place, so what Python changes in a list or dict (random.shuffle(xs))
        // is written back after the call.
        Expr::Ident(n) | Expr::Move(n) if !n.starts_with("ax_") => call("ax_pyb_argm", vec![Expr::Ident(n)], line),
        // A lambda given to Python (`key=lambda r: r[1]`, `reduce(lambda a, b: a * b, xs)`): a Python function
        // that runs it; its parameters are Python values.
        Expr::Lambda(ps, body) if ps.len() <= 3 => call(&format!("ax_pyb_fn{}", ps.len()), vec![Expr::Lambda(ps, body)], line),
        other => call("ax_pyb", vec![other], line),
    }
}

// Call arguments: positional ones (a `*xs` too, spread by Python) and keyword ones.
fn py_args(raw: Vec<(Option<String>, Expr)>, line: usize) -> (Expr, Expr) {
    // A lambda's parameters hold Python values (the parse runs again to read its body that way).
    for (_, a) in raw.iter() {
        if let Expr::Lambda(ps, _) = a {
            for p in ps {
                hold(p);
            }
        }
    }
    let mut pos = vec![];
    let mut kw = vec![];
    for (k, v) in raw {
        match k.as_deref() {
            None => pos.push(to_py(v, line)),
            Some("__star") => pos.push(tcall("ax_pyb_call", vec![to_py(v, line), s("__brevi_star__"), Expr::List(vec![]), Expr::List(vec![])], line)),
            Some(k) => kw.push(Expr::Tuple(vec![s(&unkw(k)), to_py(v, line)])),
        }
    }
    (Expr::List(pos), Expr::List(kw))
}

// `np`, or `pi` after `from math import pi` (a bridged module's name used as a value).
pub fn ident(name: &str, line: usize) -> Option<Expr> {
    reg(|r| {
        if held_in(r, &scope(r), name) {
            return None;
        }
        if let Some(m) = r.mods.get(name) {
            return Some(tcall("ax_pyb_mod", vec![s(m)], line));
        }
        if let Some((m, a)) = r.names.get(name) {
            return Some(from_name(m, a, line));
        }
        None
    })
    .flatten()
}

// `from m import a`: a submodule (`from os import path`) or an attribute of m.
fn from_name(m: &str, a: &str, line: usize) -> Expr {
    tcall("ax_pyb_from", vec![s(m), s(a)], line)
}

// The parser's hook for `name(args)`: a name imported from a bridged module, a name holding a Python value
// (called), or a Python built-in given a Python value. Calls of the program's own functions are noted (a
// parameter passed a Python value holds one).
pub fn call_hook(name: &str, args: &[(Option<String>, Expr)], user_fns: &[String], line: usize) -> Option<Expr> {
    if !active() {
        return None;
    }
    if user_fns.iter().any(|f| f == name) {
        let pys: Vec<usize> = args.iter().enumerate().filter(|(_, (k, a))| k.is_none() && is_py(a)).map(|(i, _)| i).collect();
        reg_mut(|r| {
            let f = match &r.class {
                // A call inside a class body may be another method (`self.m(x)` is lowered later); plain names are functions.
                _ => name.to_string(),
            };
            for i in pys {
                r.params.insert((f.clone(), i));
            }
        });
        return None;
    }
    let held = reg(|r| held_in(r, &scope(r), name)).unwrap_or(false);
    if held {
        let (pos, kw) = py_args(args.to_vec(), line);
        return Some(tcall("ax_pyb_call", vec![Expr::Ident(name.to_string()), s(""), pos, kw], line));
    }
    if let Some(f) = ident(name, line) {
        let (pos, kw) = py_args(args.to_vec(), line);
        return Some(tcall("ax_pyb_call", vec![f, s(""), pos, kw], line));
    }
    if !args.iter().any(|(_, a)| is_py(a)) {
        return None;
    }
    let one = || args.len() == 1 && args[0].0.is_none();
    match name {
        "int" | "float" | "str" | "bool" | "len" | "repr" if one() => {
            let f = format!("ax_pyb_{name}");
            Some(tcall(&f, vec![args[0].1.clone()], line))
        }
        "print" | "isinstance_" => None,
        "abs" | "all" | "any" | "ascii" | "bin" | "callable" | "chr" | "dict" | "dir" | "divmod" | "enumerate" | "filter" | "format" | "frozenset" | "getattr" | "hasattr" | "hash"
        | "hex" | "id" | "isinstance" | "issubclass" | "iter" | "list" | "map" | "max" | "min" | "next" | "oct" | "ord" | "pow" | "range" | "reversed" | "round" | "set" | "setattr"
        | "slice" | "sorted" | "sum" | "tuple" | "type" | "vars" | "zip" | "int" | "float" | "str" | "bool" | "len" | "repr" => {
            let (pos, kw) = py_args(args.to_vec(), line);
            Some(tcall("ax_pyb_builtin", vec![s(name), pos, kw], line))
        }
        _ => None,
    }
}

// The parser's hook for `recv.name` (args None) and `recv.name(args)` on a Python value.
pub fn member(recv: Expr, name: &str, args: Option<Vec<(Option<String>, Expr)>>, line: usize) -> Expr {
    match args {
        None => tcall("ax_pyb_get", vec![recv, s(name)], line),
        Some(a) => {
            let (pos, kw) = py_args(a, line);
            tcall("ax_pyb_call", vec![recv, s(name), pos, kw], line)
        }
    }
}

// Methods the parser itself makes (comprehensions, str(x), f-strings) are not the value's own.
pub fn py_method(m: &str) -> bool {
    // Brevi's own method names are left to Brevi (a wrong one fails the build, loudly).
    active() && !crate::emit::METHODS.contains(&m) && !matches!(m, "map" | "filter" | "flatten" | "str" | "len" | "int" | "float" | "fmt" | "spec" | "py_repr" | "__call" | "or" | "lax")
}

// `v.name(args)` with v a Python value by its type: CPython's method.
pub fn typed_call(recv: &Expr, name: &str, args: &[Expr], line: usize) -> Expr {
    let a: Vec<(Option<String>, Expr)> = args.iter().map(|x| (None, x.clone())).collect();
    member(recv.clone(), &unkw(name), Some(a), line)
}

// `recv(args)` on a Python value.
pub fn invoke(recv: Expr, args: Vec<(Option<String>, Expr)>, line: usize) -> Expr {
    let (pos, kw) = py_args(args, line);
    tcall("ax_pyb_call", vec![recv, s(""), pos, kw], line)
}

// One part of `recv[...]`: a value, or a slice `a:b:c` (None for a missing part).
pub fn slice(a: Option<Expr>, b: Option<Expr>, c: Option<Expr>, line: usize) -> Expr {
    let p = |x: Option<Expr>| x.map_or_else(|| call("ax_pyb_none", vec![], line), |x| to_py(x, line));
    tcall("ax_pyb_slice", vec![p(a), p(b), p(c)], line)
}

// `recv[k]`, `recv[a, b]` on a Python value.
pub fn index(recv: Expr, mut keys: Vec<Expr>, line: usize) -> Expr {
    let k = if keys.len() == 1 { to_py(keys.pop().unwrap(), line) } else { call("ax_pyb", vec![Expr::Tuple(keys.into_iter().map(|k| to_py(k, line)).collect())], line) };
    tcall("ax_pyb_idx", vec![recv, k], line)
}

// `del recv[k]` on a Python value.
pub fn del(e: &Expr, line: usize) -> Option<Stmt> {
    match e {
        Expr::Try(x) => match x.as_ref() {
            Expr::Call(f, a, _) if f == "ax_pyb_idx" && a.len() == 2 => Some(Stmt::Expr { expr: tcall("ax_pyb_delidx", vec![a[0].1.clone(), a[1].1.clone()], line), line }),
            _ => None,
        },
        _ => None,
    }
}

// `with x as name:` on a Python value: name = x.__enter__(), the body, then x.__exit__(None, None, None).
pub fn with(e: Expr, name: Option<String>, body: Expr, n: usize, line: usize) -> Vec<Stmt> {
    let tmp = format!("ax_pyw{n}");
    let none = || call("ax_pyb_none", vec![], line);
    let mut out = vec![Stmt::Let { name: Pat::Bind(tmp.clone()), ty: None, value: e, mutable: false, line }];
    let enter = tcall("ax_pyb_call", vec![Expr::Ident(tmp.clone()), s("__enter__"), Expr::List(vec![]), Expr::List(vec![])], line);
    match name {
        Some(n) => {
            hold(&n);
            out.push(Stmt::Let { name: Pat::Bind(n), ty: None, value: enter, mutable: true, line });
        }
        None => out.push(Stmt::Expr { expr: enter, line }),
    }
    let (mut stmts, tail) = match body {
        Expr::Block(st, t) => (st, t),
        other => (vec![], Some(Box::new(other))),
    };
    if let Some(t) = tail {
        stmts.push(Stmt::Expr { expr: *t, line });
    }
    let exit = Stmt::Expr { expr: tcall("ax_pyb_call", vec![Expr::Ident(tmp), s("__exit__"), Expr::List(vec![none(), none(), none()]), Expr::List(vec![])], line), line };
    let at = if matches!(stmts.last(), Some(Stmt::Return { .. })) { stmts.len() - 1 } else { stmts.len() };
    stmts.insert(at, exit);
    out.extend(stmts);
    out
}

// ------------------------------------------------------------------------------------------- after parsing

// Operators, assignments and loops on Python values, once every held name is known.
pub fn lower(items: &mut [Item]) {
    if !active() {
        return;
    }
    let mut n = 0;
    for it in items.iter_mut() {
        if let Item::Fn { name, body, params, script, .. } = it {
            // The script's own statements are scope ""; a `def main()` beside them was renamed main_.
            let fname = if *script { String::new() } else if name == "main_" { "main".into() } else { name.clone() };
            reg_mut(|r| {
                r.scope = vec![fname.clone()];
            });
            let _ = params;
            lower_expr(body, &mut n);
        }
    }
    reg_mut(|r| r.scope.clear());
}

fn none_call(e: &Expr) -> bool {
    matches!(e, Expr::Call(f, a, _) if f == "none" && a.is_empty())
}

fn lower_expr(e: &mut Expr, n: &mut usize) {
    // Children first.
    match e {
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| lower_expr(x, n)),
        Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
            lower_expr(k, n);
            lower_expr(v, n);
        }),
        Expr::Str(parts) => {
            for p in parts.iter_mut() {
                if let StrPart::Interp(x) = p {
                    lower_expr(x, n);
                }
            }
        }
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) | Expr::Try(a) => lower_expr(a, n),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => {
            lower_expr(a, n);
            lower_expr(b, n);
        }
        Expr::For(p, it, body) => {
            lower_expr(it, n);
            lower_expr(body, n);
            if let (Pat::Tuple(_), true) = (&*p, is_py(it)) {
                *n += 1;
                let tmp = format!("ax_pyf{n}");
                let pat = std::mem::replace(p, Pat::Bind(tmp.clone()));
                let mut stmts = unpack(&pat, Expr::Ident(tmp), 0, n);
                let b = std::mem::replace(body.as_mut(), Expr::Int(0));
                match b {
                    Expr::Block(st, t) => {
                        stmts.extend(st);
                        **body = Expr::Block(stmts, t);
                    }
                    other => **body = Expr::Block(stmts, Some(Box::new(other))),
                }
            }
        }
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| lower_expr(a, n)),
        Expr::Method(r, _, args, _) => {
            lower_expr(r, n);
            args.iter_mut().for_each(|a| lower_expr(a, n));
        }
        Expr::If(c, a, b) => {
            lower_expr(c, n);
            lower_expr(a, n);
            if let Some(b) = b {
                lower_expr(b, n);
            }
        }
        Expr::Match(sub, arms) => {
            lower_expr(sub, n);
            arms.iter_mut().for_each(|(_, a)| lower_expr(a, n));
        }
        Expr::Block(stmts, tail) => {
            let old = std::mem::take(stmts);
            for mut st in old {
                lower_stmt(&mut st, n, stmts);
            }
            if let Some(t) = tail {
                lower_expr(t, n);
            }
        }
        _ => {}
    }
    // Then this node.
    let line = 0;
    match e {
        Expr::Binary(op, a, b) if is_py(a) || is_py(b) => {
            if *op == "and" || *op == "or" {
                return;
            }
            let op: &str = match (*op, none_call(a) || none_call(b)) {
                ("==", true) => "is",
                ("!=", true) => "is not",
                (o, _) => o,
            };
            let (a, b) = (std::mem::replace(a.as_mut(), Expr::Int(0)), std::mem::replace(b.as_mut(), Expr::Int(0)));
            *e = tcall("ax_pyb_op", vec![s(op), to_py(a, line), to_py(b, line)], line);
        }
        Expr::Unary(op, a) if is_py(a) && *op != "!" => {
            let op = *op;
            let a = std::mem::replace(a.as_mut(), Expr::Int(0));
            *e = tcall("ax_pyb_unary", vec![s(op), a], line);
        }
        Expr::Call(f, a, l) if f == "__truediv" && a.len() == 2 && (is_py(&a[0].1) || is_py(&a[1].1)) => {
            let l = *l;
            let (x, y) = (a[0].1.clone(), a[1].1.clone());
            *e = tcall("ax_pyb_op", vec![s("/"), to_py(x, l), to_py(y, l)], l);
        }
        Expr::Field(r, f) if is_py(r) => {
            let r2 = std::mem::replace(r.as_mut(), Expr::Int(0));
            *e = tcall("ax_pyb_get", vec![r2, s(&unkw(f))], line);
        }
        Expr::Index(r, k) if is_py(r) => {
            let r2 = std::mem::replace(r.as_mut(), Expr::Int(0));
            let k2 = std::mem::replace(k.as_mut(), Expr::Int(0));
            *e = tcall("ax_pyb_idx", vec![r2, to_py(k2, line)], line);
        }
        // A Brevi list indexed by a Python int.
        Expr::Index(r, k) if is_py(k) && !is_py(r) => {
            let k2 = std::mem::replace(k.as_mut(), Expr::Int(0));
            **k = tcall("ax_pyb_int", vec![k2], line);
        }
        // `[f(x) for x in v]`, `sum(a * b for a, b in v)` over a Python value: its items as a Brevi list.
        Expr::Method(r, m, args, l) if is_py(r) && matches!(m.as_str(), "map" | "filter") && matches!(args.as_slice(), [Expr::Lambda(..)]) => {
            let l = *l;
            let r2 = std::mem::replace(r.as_mut(), Expr::Int(0));
            **r = tcall("ax_pyb_list", vec![r2], l);
            if let Expr::Lambda(ps, body) = &mut args[0] {
                if ps.len() > 1 {
                    *n += 1;
                    let tmp = format!("ax_pyf{n}");
                    let pat = Pat::Tuple(ps.iter().map(|p| Pat::Bind(p.clone())).collect());
                    let stmts = unpack(&pat, Expr::Ident(tmp.clone()), l, n);
                    let b = std::mem::replace(body.as_mut(), Expr::Int(0));
                    **body = Expr::Block(stmts, Some(Box::new(b)));
                    *ps = vec![tmp];
                }
            }
        }
        // `xs[:n]` on a Brevi list with n a Python int.
        Expr::Method(r, m, args, l) if !is_py(r) && m == "slice" && args.iter().any(is_py) => {
            let l = *l;
            for a in args.iter_mut().filter(|a| is_py(a)) {
                let x = std::mem::replace(a, Expr::Int(0));
                *a = tcall("ax_pyb_int", vec![x], l);
            }
        }
        // f-string `{x:.2f}` and `{x!r}` on a Python value.
        Expr::Method(r, m, args, l) if is_py(r) => {
            let r2 = std::mem::replace(r.as_mut(), Expr::Int(0));
            let l = *l;
            *e = match (m.as_str(), args.len()) {
                ("fmt" | "spec", 1) => tcall("ax_pyb_format", vec![r2, args[0].clone()], l),
                ("py_repr", 0) => tcall("ax_pyb_repr", vec![r2], l),
                ("str", 0) => tcall("ax_pyb_str", vec![r2], l),
                ("len", 0) => tcall("ax_pyb_len", vec![r2], l),
                ("int", 0) => tcall("ax_pyb_int", vec![r2], l),
                ("float", 0) => tcall("ax_pyb_float", vec![r2], l),
                _ => {
                    let a: Vec<(Option<String>, Expr)> = args.drain(..).map(|a| (None, a)).collect();
                    member(r2, m, Some(a), l)
                }
            };
        }
        _ => {}
    }
}

// `a, (b, c) = v` with v a Python value: its items, n of them, one level at a time.
fn unpack(p: &Pat, v: Expr, line: usize, n: &mut usize) -> Vec<Stmt> {
    let Pat::Tuple(ps) = p else {
        return match p {
            Pat::Bind(x) => vec![Stmt::Let { name: Pat::Bind(x.clone()), ty: None, value: v, mutable: true, line }],
            _ => vec![],
        };
    };
    *n += 1;
    let tmp = format!("ax_pyu{n}");
    let mut out = vec![Stmt::Let { name: Pat::Bind(tmp.clone()), ty: None, value: tcall("ax_pyb_items", vec![v, Expr::Int(ps.len() as i64)], line), mutable: false, line }];
    for (i, q) in ps.iter().enumerate() {
        let item = Expr::Index(Box::new(Expr::Ident(tmp.clone())), Box::new(Expr::Int(i as i64)));
        out.extend(unpack(q, item, line, n));
    }
    out
}

fn assign_target(t: &Expr) -> Option<(&'static str, Vec<Expr>)> {
    let Expr::Try(x) = t else { return None };
    match x.as_ref() {
        Expr::Call(f, a, _) if f == "ax_pyb_idx" && a.len() == 2 => Some(("ax_pyb_setidx", vec![a[0].1.clone(), a[1].1.clone()])),
        Expr::Call(f, a, _) if f == "ax_pyb_get" && a.len() == 2 => Some(("ax_pyb_set", vec![a[0].1.clone(), a[1].1.clone()])),
        _ => None,
    }
}

fn lower_stmt(st: &mut Stmt, n: &mut usize, out: &mut Vec<Stmt>) {
    match st {
        Stmt::Let { value, .. } => lower_expr(value, n),
        Stmt::Assign { target, value, .. } => {
            lower_expr(value, n);
            if assign_target(target).is_none() {
                lower_expr(target, n);
            }
        }
        Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } | Stmt::Assert { cond: expr, .. } => lower_expr(expr, n),
        _ => {}
    }
    match st {
        // `o[k] = v`, `o.a = v`, `o[k] += v` on a Python value.
        Stmt::Assign { target, op, value, line } if assign_target(target).is_some() => {
            let (f, mut a) = assign_target(target).unwrap();
            let line = *line;
            let v = std::mem::replace(value, Expr::Int(0));
            let v = if *op == "=" { to_py(v, line) } else { tcall("ax_pyb_op", vec![s(op), target.clone(), to_py(v, line)], line) };
            a.push(v);
            out.push(Stmt::Expr { expr: tcall(f, a, line), line });
        }
        // `x = v`, `x += v` with x holding Python values.
        Stmt::Assign { target: Expr::Ident(x), op, value, line } if is_py(&Expr::Ident(x.clone())) => {
            let line = *line;
            let v = std::mem::replace(value, Expr::Int(0));
            let v = if *op == "=" { to_py(v, line) } else { tcall("ax_pyb_op", vec![s(op), Expr::Ident(x.clone()), to_py(v, line)], line) };
            out.push(Stmt::Assign { target: Expr::Ident(x.clone()), op: "=", value: v, line });
        }
        Stmt::Let { name: Pat::Bind(x), value, .. } if is_py(&Expr::Ident(x.clone())) && !is_py(value) => {
            let v = std::mem::replace(value, Expr::Int(0));
            *value = to_py(v, 0);
            out.push(st.clone());
        }
        // `a, b = v` with v a Python value.
        Stmt::Assign { target: t @ Expr::Tuple(_), op: "=", value, line } if is_py(value) && expr_pat(t).is_some() => {
            let p = expr_pat(t).unwrap();
            let line = *line;
            let v = std::mem::replace(value, Expr::Int(0));
            let mut stmts = unpack(&p, v, line, n);
            // Assignments, not new names (the names may be set before).
            for s in stmts.iter_mut() {
                if let Stmt::Let { name: Pat::Bind(x), value, line, .. } = s {
                    if !x.starts_with("ax_pyu") {
                        *s = Stmt::Assign { target: Expr::Ident(x.clone()), op: "=", value: value.clone(), line: *line };
                    }
                }
            }
            out.extend(stmts);
        }
        Stmt::Let { name: p @ Pat::Tuple(_), value, line, .. } if is_py(value) => {
            let line = *line;
            let v = std::mem::replace(value, Expr::Int(0));
            out.extend(unpack(&p.clone(), v, line, n));
        }
        _ => out.push(st.clone()),
    }
}

fn expr_pat(e: &Expr) -> Option<Pat> {
    match e {
        Expr::Ident(n) | Expr::Move(n) => Some(Pat::Bind(n.clone())),
        Expr::Tuple(xs) => xs.iter().map(expr_pat).collect::<Option<Vec<_>>>().map(Pat::Tuple),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------- the runtime

const RUNTIME: &str = concat!(include_str!("pyrt_load.rs"), include_str!("pyrt.rs"));
// The loader alone, for the dynamic builds' bridge (dynpy/pyglue.rs).
pub const LOADER: &str = include_str!("pyrt_load.rs");

// Asked of python3 when the program is built: libpython's path, PYTHONHOME and sys.path (minus the script's
// folder), joined by \x01.
pub const QUERY: &str = r#"import sys, os, sysconfig
d = sysconfig.get_config_var
c = []
for n in (d('INSTSONAME'), d('LDLIBRARY')):
    if n:
        c.append(os.path.join(d('LIBDIR') or '', n))
        c.append(os.path.join(d('LIBPL') or '', n))
        c.append(os.path.join(sys.base_prefix, 'lib', n))
fw = d('PYTHONFRAMEWORKPREFIX')
if fw:
    c.append(os.path.join(fw, d('PYTHONFRAMEWORK') + '.framework', 'Versions', d('py_version_short'), d('PYTHONFRAMEWORK')))
lib = next((x for x in c if os.path.isfile(x) and not x.endswith('.a')), '')
print('\x01'.join([lib, sys.base_prefix] + sys.path[1:]))
"#;

static FILE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
pub fn set_file(f: &str) {
    FILE.set(f.to_string()).ok();
}

// What python3 answers to QUERY (asked once).
fn conf() -> &'static Result<String, String> {
    static C: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
        let out = std::process::Command::new(&exe).args(["-c", QUERY]).output().map_err(|e| format!("cannot start {exe}: {e}"))?;
        let s = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        match s.split('\u{1}').next() {
            Some(lib) if !lib.trim().is_empty() => Ok(s),
            _ => Err(format!("{exe} has no shared library (libpython), which Python libraries need in a Brevi program")),
        }
    })
}

// Whether python3 here has a shared library to load (a build fails loudly without one).
pub fn python_ok() -> Result<(), String> {
    conf().as_ref().map(|_| ()).map_err(|e| e.clone())
}

// The runtime for this program: CPython found now (and again at run time if it moved).
pub fn runtime() -> String {
    format!("{}{RUNTIME}", consts())
}

// The loader and its constants, for a dynamic build that bridges libraries (dynpy/pyglue.rs follows it).
pub fn dyn_runtime() -> String {
    format!("{}{LOADER}", consts())
}

fn consts() -> String {
    let file = FILE.get().cloned().unwrap_or_default();
    let conf = conf().clone().unwrap_or_default();
    let abs = std::fs::canonicalize(&file).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| file.clone());
    let dir = std::path::Path::new(&abs).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    // sys.path[0] is the script's folder, as python3 sets it.
    let conf = match conf.split_once('\u{1}') {
        Some((lib, rest)) => match rest.split_once('\u{1}') {
            Some((home, path)) => format!("{lib}\u{1}{home}\u{1}{dir}\u{1}{path}"),
            None => format!("{lib}\u{1}{rest}\u{1}{dir}"),
        },
        None => String::new(),
    };
    let subs = SUBS.with(|x| x.borrow().clone());
    format!("const AX_PYB_SUBS: &[&str] = &{subs:?};\nconst AX_PYB_CONF: &str = {conf:?};\nconst AX_PYB_QUERY: &str = {QUERY:?};\nconst AX_PYB_FILE: &str = {file:?};\n")
}

// ------------------------------------------------------------------------------------------- standalone builds

const BUNDLER: &str = include_str!("pybundle.py");

// `brevi build --standalone`: appends CPython (libpython, its shared libraries, the standard library and the
// packages the program imports) to the binary, which then runs where no Python is installed. pyrt.rs unpacks
// it into a cache folder on first run. Linux only for now.
pub fn bundle(file: &str, bin: &std::path::Path) -> Result<String, String> {
    if !cfg!(target_os = "linux") {
        return Err("--standalone builds of programs that use Python libraries need Linux for now".into());
    }
    let conf = conf().clone()?;
    let lib = conf.split('\u{1}').next().unwrap_or_default().to_string();
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let out = std::process::Command::new(&exe)
        .args(["-c", BUNDLER, file, &bin.to_string_lossy(), &lib])
        .output()
        .map_err(|e| format!("cannot start {exe}: {e}"))?;
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(format!("bundling Python failed: {}", err.trim().lines().last().unwrap_or("")));
    }
    let w: Vec<u64> = String::from_utf8_lossy(&out.stdout).split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let mut note = format!("bundled Python: {} files, {:.1} MB", w.first().unwrap_or(&0), *w.get(1).unwrap_or(&0) as f64 / 1e6);
    for l in err.lines().filter(|l| l.starts_with("brevi:")) {
        note.push_str("\n");
        note.push_str(l);
    }
    Ok(note)
}
