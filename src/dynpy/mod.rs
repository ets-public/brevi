// Native builds of Python files that typed inference cannot handle: the program is compiled to Rust over a
// dynamic value type (runtime in rt_*.rs), so mixed-type values work as they do in CPython.
pub mod emit;
pub mod parse;
pub mod tok;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

// The bridge to CPython for libraries Brevi lacks (pasted into the program only when it imports one).
const GLUE: &str = include_str!("pyglue.rs");

// Set while a build may not bridge: the typed build, which bridges the same calls, goes instead.
pub static NO_BRIDGE: AtomicBool = AtomicBool::new(false);
// What the last to_rust has CPython run (modules, `open`), for the path report.
static BRIDGED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn bridged() -> Vec<String> {
    BRIDGED.lock().unwrap().clone()
}

// Whether this build may hand libraries to CPython: not under the sandbox without allow native, and only with
// a python3 that has libpython (asked once, only when a program imports such a library).
fn bridge_ok() -> bool {
    cfg!(unix) && !crate::pybridge::no_native() && !NO_BRIDGE.load(Ordering::Relaxed) && std::env::var("BREVI_DYN_BRIDGE").map_or(true, |v| v != "0") && crate::pybridge::python_ok().is_ok()
}

pub const RUNTIME: &str = concat!(
    include_str!("rt_core.rs"),
    include_str!("rt_ops.rs"),
    include_str!("rt_fmt.rs"),
    include_str!("rt_call.rs"),
    include_str!("rt_lib.rs"),
    include_str!("rt_mod.rs"),
    include_str!("rt_text.rs"),
    include_str!("rt_ext.rs"),
    include_str!("rt_glue.rs"),
    include_str!("rt_gen.rs"),
    include_str!("rt_async.rs"),
    include_str!("rt_enum.rs"),
    include_str!("rt_rand.rs"),
    include_str!("rt_bytes.rs"),
    include_str!("rt_np.rs"),
    include_str!("rt_npfmt.rs"),
    include_str!("rt_npfn.rs"),
    include_str!("rt_nprand.rs"),
    include_str!("rt_nprdist.rs"),
    include_str!("rt_nplinalg.rs"),
    include_str!("rt_nplsq.rs"),
    include_str!("rt_tokre.rs"),
    include_str!("rt_tokuni.rs"),
    include_str!("rt_tok.rs"),
    include_str!("../alloc_rt.rs"),
);

const HEADER: &str = "#![allow(warnings)] #[macro_use] extern crate brevi_dyn; use brevi_dyn::*; use std::rc::Rc; use std::cell::{Cell, RefCell};\n";

fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for p in parts {
        for b in p.bytes() {
            h = (h ^ b as u64).wrapping_mul(1099511628211);
        }
        h = (h ^ 0xff).wrapping_mul(1099511628211);
    }
    h
}

fn walk_imports(body: &[parse::Stmt], out: &mut Vec<String>) {
    use parse::St;
    for s in body {
        match &s.st {
            St::Import(v) => {
                for (m, a) in v {
                    out.push(if a.is_some() { m.clone() } else { m.split('.').next().unwrap().to_string() });
                }
            }
            St::ImportFrom(m, _) => out.push(m.clone()),
            St::Def(d) => walk_imports(&d.body, out),
            St::Class(c) => walk_imports(&c.body, out),
            St::If(arms, els) => {
                for (_, b) in arms {
                    walk_imports(b, out);
                }
                if let Some(b) = els {
                    walk_imports(b, out);
                }
            }
            St::For(_, _, b, e) | St::While(_, b, e) => {
                walk_imports(b, out);
                if let Some(e) = e {
                    walk_imports(e, out);
                }
            }
            St::Try(b, hs, e, f) => {
                walk_imports(b, out);
                for h in hs {
                    walk_imports(&h.body, out);
                }
                if let Some(e) = e {
                    walk_imports(e, out);
                }
                if let Some(f) = f {
                    walk_imports(f, out);
                }
            }
            St::With(_, b) => walk_imports(b, out),
            _ => {}
        }
    }
}

// Python modules that CPython would run (pybridge), written in plain Python and built with the program instead
// when the sandbox turns CPython off (no `allow native`). Each holds only what needs no file, process, network
// or environment access: os is os.path and its constants, pathlib is PurePath. caps.rs refuses any other use
// of os and pathlib before this runs. Their results match CPython's on POSIX systems.
fn sandbox_module(m: &str) -> Option<&'static str> {
    Some(match m {
        "random" => include_str!("py/random.py"),
        "os" => include_str!("py/os.py"),
        "posixpath" => include_str!("py/posixpath.py"),
        "os.path" => include_str!("py/posixpath.py"),
        "pathlib" => include_str!("py/pathlib.py"),
        "io" => include_str!("py/io.py"),
        "textwrap" => include_str!("py/textwrap.py"),
        "argparse" => include_str!("py/argparse.py"),
        "asyncio" => include_str!("py/asyncio.py"),
        "tiktoken" => include_str!("py/tiktoken.py"),
        _ => return None,
    })
}

// shutil.get_terminal_size().columns, for argparse's help outside the sandbox: $COLUMNS when it is a positive
// int, else the width of the terminal on stdout, else 80.
const TERM_COLS_FN: &str = r#"
fn __term_cols() -> i64 {
    if let Some(n) = std::env::var("COLUMNS").ok().and_then(|v| v.trim().parse::<i64>().ok()) {
        if n > 0 {
            return n;
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        #[repr(C)]
        struct Ws(u16, u16, u16, u16);
        extern "C" {
            fn ioctl(fd: i32, req: std::os::raw::c_ulong, ...) -> i32;
        }
        #[cfg(target_os = "linux")]
        const REQ: std::os::raw::c_ulong = 0x5413;
        #[cfg(target_os = "macos")]
        const REQ: std::os::raw::c_ulong = 0x40087468;
        let mut w = Ws(0, 0, 0, 0);
        if unsafe { ioctl(1, REQ, &mut w as *mut Ws) } == 0 && w.1 > 0 {
            return w.1 as i64;
        }
    }
    80
}
"#;

// Every `seed(...)` and `Random(...)` in the text takes no argument or an int literal: the copy of random
// takes only int seeds (CPython hashes a str or float one), and a seed it cannot see may be either.
fn int_seeds(src: &str) -> bool {
    for pat in ["seed(", "Random("] {
        let mut from = 0;
        while let Some(k) = src[from..].find(pat) {
            let at = from + k + pat.len();
            let arg = src[at..].split(')').next().unwrap_or("").trim();
            let arg = arg.strip_prefix('-').unwrap_or(arg).replace('_', "");
            if !arg.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            from = at;
        }
    }
    true
}

// The Rust program for a Python file and the modules it imports from its own directory.
// Err is (file, line, message).
pub fn to_rust(file: &str) -> Result<String, (String, usize, String)> {
    BRIDGED.lock().unwrap().clear();
    use std::sync::atomic::Ordering::Relaxed;
    let np_off = std::env::var("BREVI_NUMPY").is_ok_and(|v| v == "0");
    emit::NP_NATIVE.store(!np_off, Relaxed);
    let r = to_rust_np(file);
    emit::NP_ON.store(false, Relaxed);
    emit::NP_NARROW.store(false, Relaxed);
    // Something of numpy that Brevi's own numpy lacks: no dynamic build. CPython's numpy through
    // the dynamic bridge is not used: the typed build's bridge (or CPython itself) runs it right.
    if let Err((f, l, m)) = &r {
        if m.contains("numpy") && std::env::var("BREVI_NUMPY_DEBUG").is_ok() {
            eprintln!("native numpy refused: {f}:{l}: {m}");
        }
    }
    r
}

fn to_rust_np(file: &str) -> Result<String, (String, usize, String)> {
    match to_rust_with(file, true) {
        // More of os, pathlib, random, ... than Brevi's copies have: CPython's own modules, through the bridge.
        Err((_, 0, m)) if m == BEYOND_COPIES && bridge_ok() => to_rust_with(file, false),
        r => r,
    }
}

const BEYOND_COPIES: &str = "os, pathlib, random, io, textwrap, argparse, asyncio or tiktoken beyond Brevi's own copies runs in CPython";

fn to_rust_with(file: &str, copies: bool) -> Result<String, (String, usize, String)> {
    let src = std::fs::read_to_string(file).map_err(|e| (file.to_string(), 0, e.to_string()))?;
    let src = crate::modules::fill_file_name(&src, Path::new(file));
    // `allow` lines (checked in main.rs before this runs) are not Python.
    let src = crate::caps::split(&src).0;
    let src = crate::pygen::rewrite(&src);
    let body = parse::parse(&src).map_err(|(l, m)| (file.to_string(), l, m))?;
    let dir = Path::new(file).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut mods = vec![emit::Module { name: "__main__".into(), body }];
    // Brevi's own random and path-text copies stand in for CPython's only when no file of the program uses
    // more of os, pathlib or random than they have.
    let pure = |s: &str| crate::caps::path_uses(s).map_or(true, |v| v.is_empty()) && int_seeds(s);
    let mut own_ok = pure(&src);
    let mut own_used = false;
    let mut own_mods: HashSet<String> = HashSet::new();
    let mut bridge = emit::Bridge::default();
    let mut texts: HashMap<String, (String, String)> = HashMap::new();
    texts.insert("__main__".into(), (file.to_string(), src.clone()));
    let mut k = 0;
    while k < mods.len() {
        let mut imps = vec![];
        walk_imports(&mods[k].body, &mut imps);
        for m in imps {
            if mods.iter().any(|x| x.name == m) || emit::builtin_mod(&m) {
                if m == "numpy" {
                    emit::NP_ON.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                continue;
            }
            if emit::np_native() && (m.starts_with("numpy.") || m == "numpy") {
                return Err((file.to_string(), 0, format!("numpy: module {m} is not in Brevi's native numpy")));
            }
            let p = dir.join(format!("{m}.py"));
            // Outside the sandbox too, when every use of these modules is one Brevi's copies have (checked
            // over all the program's files below). random's generator core is native (rt_rand.rs).
            if !p.exists() && ((copies && own_ok) || !crate::pybridge::py_allowed(&m)) {
                if let Some(src) = sandbox_module(&m) {
                    own_used = true;
                    own_mods.insert(m.clone());
                    let body = parse::parse(src).map_err(|(l, msg)| (format!("<{m}>"), l, msg))?;
                    mods.push(emit::Module { name: m, body });
                    continue;
                }
            }
            if p.exists() {
                let ps = p.to_string_lossy().to_string();
                let src = std::fs::read_to_string(&p).map_err(|e| (ps.clone(), 0, e.to_string()))?;
                let src = crate::modules::fill_file_name(&src, &p);
                let src = crate::caps::split(&src).0;
                let body = parse::parse(&src).map_err(|(l, m)| (ps.clone(), l, m))?;
                own_ok &= pure(&src);
                texts.insert(m.clone(), (ps.clone(), src.to_string()));
                mods.push(emit::Module { name: m, body });
            } else if m == "asyncio" || m.starts_with("asyncio.") {
                // Brevi's coroutines run only on Brevi's own asyncio.
                return Err((file.to_string(), 0, format!("module {m} beyond Brevi's own asyncio runs in CPython")));
            } else if !m.starts_with('.') && crate::pybridge::py_allowed(&m) && (bridge.on || bridge_ok()) {
                // A library Brevi lacks: CPython runs it inside the program.
                bridge.on = true;
                bridge.modules.insert(m.split('.').next().unwrap_or(&m).to_string());
            }
        }
        k += 1;
    }
    if own_used && !own_ok && !crate::pybridge::no_native() {
        return Err((file.to_string(), 0, BEYOND_COPIES.into()));
    }
    if bridge.on {
        // A coroutine handed to a library CPython runs would not be one of CPython's.
        if mods.iter().any(|m| m.name != "asyncio" && emit::uses_async(&m.body)) {
            return Err((file.to_string(), 0, "async code with a library CPython runs".into()));
        }
        lib_classes(&mods, &texts, &mut bridge)?;
    }
    // The file of the module the error is in, not always the main file (#194).
    if bridge.on && emit::NP_ON.load(std::sync::atomic::Ordering::Relaxed) {
        // numpy's arrays would cross into the bridged library: both go to CPython
        return Err((file.to_string(), 0, "numpy: arrays passed to a library CPython runs".into()));
    }
    // Brevi's tiktoken: the vocabularies to build in, and its patterns checked
    let tok_files = if own_mods.contains("tiktoken") {
        let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
        let r = tok::check(&user).and_then(|f| tok::blobs(&crate::cache_dir(), &f));
        match r {
            Ok(b) => Some(b),
            Err(m) => {
                if std::env::var("BREVI_TOK_DEBUG").is_ok() {
                    eprintln!("native tiktoken refused: {m}");
                }
                // CPython's tiktoken, through the bridge, when it may be used
                return Err((file.to_string(), 0, if copies && bridge_ok() && crate::pybridge::py_allowed("tiktoken") { BEYOND_COPIES.into() } else { m }));
            }
        }
    } else {
        None
    };
    emit::NP_NARROW.store(emit::NP_ON.load(std::sync::atomic::Ordering::Relaxed) && texts.values().any(|(_, t)| emit::names_narrow(t)), std::sync::atomic::Ordering::Relaxed);
    let (mut rust, used) = emit::program(&mods, bridge).map_err(|(md, l, m)| {
        let f = match texts.get(&md) {
            Some((p, _)) => p.clone(),
            None if md != "__main__" => format!("<{md}>"),
            None => file.to_string(),
        };
        (f, l, m)
    })?;
    if !used.is_empty() {
        crate::pybridge::set_file(file);
        rust.push_str(&crate::pybridge::dyn_runtime());
        rust.push_str(GLUE);
        *BRIDGED.lock().unwrap() = used;
    }
    if let Some(b) = &tok_files {
        rust = tok::embed(rust, b);
    }
    if mods.iter().any(|m| m.name == "argparse") && crate::pybridge::py_allowed("argparse") {
        rust = rust.replacen("fn main() { run_main(__run); }", "fn main() { TERM_COLS.set(__term_cols).ok(); run_main(__run); }", 1);
        rust.push_str(TERM_COLS_FN);
    }
    Ok(format!("{HEADER}{rust}"))
}

// The runtime as a library, built once per rustc version into the cache.
fn runtime_lib(cache: &Path, rustc_v: &str) -> Result<PathBuf, String> {
    let dir = cache.join("dynrt").join(format!("{:016x}", fnv(&[RUNTIME, rustc_v])));
    let lib = dir.join("libbrevi_dyn.rlib");
    if lib.exists() {
        return Ok(lib);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("build-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    std::fs::write(tmp.join("rt.rs"), RUNTIME).map_err(|e| e.to_string())?;
    let o = crate::rustc_cmd()
        .args(["--edition", "2021", "--crate-type", "rlib", "--crate-name", "brevi_dyn", "-A", "warnings", "-C", "opt-level=3", "-C", "overflow-checks=off"])
        .arg("-o")
        .arg(tmp.join("libbrevi_dyn.rlib"))
        .arg(tmp.join("rt.rs"))
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        std::fs::remove_dir_all(&tmp).ok();
        return Err(format!("dynamic runtime did not build: {}", String::from_utf8_lossy(&o.stderr).lines().take(20).collect::<Vec<_>>().join("\n")));
    }
    let moved = std::fs::rename(tmp.join("libbrevi_dyn.rlib"), &lib).is_ok();
    std::fs::remove_dir_all(&tmp).ok();
    if moved || lib.exists() {
        Ok(lib)
    } else {
        Err("dynamic runtime could not be saved".into())
    }
}

// Builds `rust` (from to_rust) into `out`. Builds are cached by the code's hash.
// `brevi check` of a file on the dynamic path: rustc's checks only (no code generated, nothing linked), about
// a third of a build's time. A success is remembered in the cache like a build.
pub fn check(rust: &str, dir: &Path, cache: &Path, rustc_v: &str) -> Result<(), String> {
    let lib = runtime_lib(cache, rustc_v)?;
    let key = format!("{:016x}", fnv(&[rust, rustc_v, RUNTIME, "dyn-meta"]));
    let mark = cache.join("bin").join(&key);
    let use_cache = crate::runcache::on();
    if use_cache && mark.exists() {
        crate::touch(&mark);
        return Ok(());
    }
    let rs = dir.join("dyn_main.rs");
    std::fs::write(&rs, rust).map_err(|e| e.to_string())?;
    let o = crate::rustc_cmd()
        .args(["--edition", "2021", "--crate-name", "brevi_prog", "-A", "warnings", "--emit=metadata", "-C", "overflow-checks=off"])
        .arg("--extern")
        .arg(format!("brevi_dyn={}", lib.display()))
        .arg("--out-dir")
        .arg(dir)
        .arg(&rs)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).to_string());
    }
    if use_cache && std::fs::create_dir_all(cache.join("bin")).is_ok() {
        std::fs::write(&mark, "").ok();
    }
    Ok(())
}

pub fn compile(rust: &str, dir: &Path, out: &Path, release: bool, cache: &Path, rustc_v: &str) -> Result<(), String> {
    let lib = runtime_lib(cache, rustc_v)?;
    let opt = if release { "opt-level=3" } else { "opt-level=1" };
    let key = format!("{:016x}", fnv(&[rust, opt, rustc_v, RUNTIME, "dyn"]));
    let cdir = cache.join("bin");
    let cached = cdir.join(&key);
    let use_cache = crate::runcache::on();
    if use_cache && cached.exists() && crate::copy_bin(&cached, out) {
        crate::touch(&cached);
        return Ok(());
    }
    let rs = dir.join("dyn_main.rs");
    std::fs::write(&rs, rust).map_err(|e| e.to_string())?;
    let o = crate::rustc_cmd()
        .args(["--edition", "2021", "--crate-name", "brevi_prog", "-A", "warnings", "-C", opt, "-C", "overflow-checks=off", "-C", "strip=debuginfo"])
        .args(if release { None } else { Some(["-C", crate::codegen_units()]) }.into_iter().flatten())
        .arg("--extern")
        .arg(format!("brevi_dyn={}", lib.display()))
        .arg("-o")
        .arg(out)
        .arg(&rs)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).to_string());
    }
    if use_cache && std::fs::create_dir_all(&cdir).is_ok() {
        let tmp = cdir.join(format!("{key}.{}", std::process::id()));
        if std::fs::copy(out, &tmp).is_ok() && std::fs::rename(&tmp, &cached).is_ok() {
            crate::runcache::prune(&cdir);
        } else {
            std::fs::remove_file(&tmp).ok();
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------- library classes

// The name an expression starts from: `pydantic` in `pydantic.BaseModel`, `Generic` in `Generic[T]`.
fn root_name(e: &parse::Ex) -> Option<&str> {
    use parse::Ex;
    match e {
        Ex::Name(n) => Some(n),
        Ex::Attr(o, _) | Ex::Index(o, _) => root_name(o),
        Ex::Call(f, _) => root_name(f),
        _ => None,
    }
}

// Asked of python3 for each class: where its lines (with decorators) start and end, and the names it uses that
// it does not set itself and that are not Python builtins.
const CLASS_QUERY: &str = r#"import ast, builtins, json, sys
d = json.load(sys.stdin)
out = {}
for node in ast.parse(d['src']).body:
    if isinstance(node, ast.ClassDef) and node.lineno in d['lines']:
        stored, loaded = set(), set()
        for n in ast.walk(node):
            if isinstance(n, ast.Name):
                (loaded if isinstance(n.ctx, ast.Load) else stored).add(n.id)
            elif isinstance(n, ast.arg):
                stored.add(n.arg)
            elif isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                stored.add(n.name)
            elif isinstance(n, ast.alias):
                stored.add((n.asname or n.name).split('.')[0])
            elif isinstance(n, ast.ExceptHandler) and n.name:
                stored.add(n.name)
        free = sorted(x for x in loaded - stored if not hasattr(builtins, x) and x != '__class__')
        start = min([x.lineno for x in node.decorator_list] + [node.lineno])
        out[str(node.lineno)] = [start, node.end_lineno, free]
print(json.dumps(out))
"#;

// Classes built on a library's classes (`class Post(BaseModel)`, `@lib.decorate class X`, and classes built on
// those): CPython runs each whole, from its own lines, with the module's import lines run before it. Refused
// (the file then runs in CPython) when such a class is not at the top level of its file, uses a name of the
// program CPython cannot see (anything not imported, not a builtin, not another such class), or is built on
// from another of the program's files.
fn lib_classes(mods: &[emit::Module], texts: &HashMap<String, (String, String)>, bridge: &mut emit::Bridge) -> Result<(), (String, usize, String)> {
    use parse::St;
    let users: HashSet<String> = mods.iter().map(|m| m.name.clone()).collect();
    let mut by_mod: HashMap<String, HashSet<String>> = HashMap::new();
    for m in mods {
        let (path, src) = texts.get(&m.name).cloned().unwrap_or_default();
        // Names bound by the file's import lines: of libraries, and of any module that is not a file of the
        // program (CPython has those too).
        let mut imported: HashSet<String> = HashSet::new();
        let mut libnames: HashSet<String> = HashSet::new();
        let mut lines = vec![];
        let mut future = String::new();
        for s in &m.body {
            match &s.st {
                St::Import(v) => {
                    for (name, a) in v {
                        if users.contains(name) || name.starts_with('.') {
                            continue;
                        }
                        let bind = a.clone().unwrap_or_else(|| name.split('.').next().unwrap().to_string());
                        imported.insert(bind.clone());
                        if bridge.has(name) && !emit::BUILTIN_MODULES.contains(&name.as_str()) {
                            libnames.insert(bind);
                        }
                        lines.push(match a {
                            Some(a) => format!("import {name} as {a}"),
                            None => format!("import {name}"),
                        });
                    }
                }
                St::ImportFrom(name, v) => {
                    if users.contains(name) || name.starts_with('.') {
                        continue;
                    }
                    let lib = bridge.has(name) && !emit::BUILTIN_MODULES.contains(&name.as_str());
                    let mut parts = vec![];
                    for (n, a) in v {
                        let bind = a.clone().unwrap_or_else(|| n.clone());
                        imported.insert(bind.clone());
                        if lib {
                            libnames.insert(bind);
                        }
                        parts.push(match a {
                            Some(a) => format!("{n} as {a}"),
                            None => n.clone(),
                        });
                    }
                    let line = format!("from {name} import {}", parts.join(", "));
                    if name == "__future__" {
                        future = line;
                    } else {
                        lines.push(line);
                    }
                }
                _ => {}
            }
        }
        // Top-level classes on library names, in order (one built on another counts too).
        let mut want: Vec<(usize, String)> = vec![];
        let uses_lib = |c: &parse::ClassDef, names: &HashSet<String>| c.bases.iter().chain(c.decorators.iter()).filter_map(root_name).any(|n| names.contains(n));
        for s in &m.body {
            if let St::Class(c) = &s.st {
                if uses_lib(c, &libnames) {
                    libnames.insert(c.name.clone());
                    want.push((c.line, c.name.clone()));
                }
            }
        }
        // One anywhere else is refused.
        fn nested<'a>(body: &'a [parse::Stmt], top: bool, out: &mut Vec<&'a parse::ClassDef>) {
            for s in body {
                match &s.st {
                    St::Class(c) => {
                        if !top {
                            out.push(c);
                        }
                        nested(&c.body, false, out);
                    }
                    St::Def(d) => nested(&d.body, false, out),
                    St::If(arms, els) => {
                        arms.iter().for_each(|(_, b)| nested(b, false, out));
                        if let Some(b) = els {
                            nested(b, false, out);
                        }
                    }
                    St::For(_, _, b, e) | St::While(_, b, e) => {
                        nested(b, false, out);
                        if let Some(e) = e {
                            nested(e, false, out);
                        }
                    }
                    St::Try(b, hs, e, f) => {
                        nested(b, false, out);
                        hs.iter().for_each(|h| nested(&h.body, false, out));
                        for x in [e, f].into_iter().flatten() {
                            nested(x, false, out);
                        }
                    }
                    St::With(_, b) => nested(b, false, out),
                    _ => {}
                }
            }
        }
        let mut inner = vec![];
        nested(&m.body, true, &mut inner);
        if let Some(c) = inner.into_iter().find(|c| uses_lib(c, &libnames)) {
            return Err((path, c.line, format!("class {} is built on a library class inside a function or block, which dynamic builds leave to CPython", c.name)));
        }
        by_mod.insert(m.name.clone(), want.iter().map(|w| w.1.clone()).collect());
        if want.is_empty() {
            continue;
        }
        let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
        let ask = format!("{{\"src\":{},\"lines\":[{}]}}", jq(&src), want.iter().map(|w| w.0.to_string()).collect::<Vec<_>>().join(","));
        let got = (|| -> Option<String> {
            use std::io::Write;
            let mut c = std::process::Command::new(&exe).args(["-I", "-c", CLASS_QUERY]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
            c.stdin.take()?.write_all(ask.as_bytes()).ok()?;
            let o = c.wait_with_output().ok()?;
            o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
        })()
        .and_then(|t| crate::json::parse(t.trim()))
        .ok_or_else(|| (path.clone(), want[0].0, format!("class {} is built on a library class, and {exe} could not read the file", want[0].1)))?;
        let text: Vec<&str> = src.lines().collect();
        let known: HashSet<String> = imported.iter().cloned().chain(want.iter().map(|w| w.1.clone())).collect();
        for (line, name) in &want {
            let e = got.get(&line.to_string()).map(|v| v.arr().to_vec()).unwrap_or_default();
            let (Some(start), Some(end)) = (e.first().and_then(|x| x.num()), e.get(1).and_then(|x| x.num())) else {
                return Err((path.clone(), *line, format!("class {name} is built on a library class, and its lines could not be read")));
            };
            let free: Vec<String> = e.get(2).map(|x| x.arr().iter().filter_map(|s| s.str().map(String::from)).collect()).unwrap_or_default();
            if let Some(x) = free.iter().find(|x| !known.contains(*x)) {
                return Err((path.clone(), *line, format!("class {name} is built on a library class and uses {x}, a name of the program that CPython, which runs that class, cannot see")));
            }
            let (start, end) = (start as usize, end as usize);
            // Its own line numbers, so CPython's errors name the file's lines; a __future__ line on the first.
            let mut body = String::new();
            let pad = if future.is_empty() { start - 1 } else { body.push_str(&future); body.push('\n'); start.saturating_sub(2) };
            body.push_str(&"\n".repeat(pad));
            for l in text.iter().take(end).skip(start - 1) {
                body.push_str(l);
                body.push('\n');
            }
            bridge.classes.insert((m.name.clone(), *line), body);
        }
        let mut imports = future.clone();
        for l in &lines {
            imports.push('\n');
            imports.push_str(l);
        }
        bridge.imports.insert(m.name.clone(), imports);
        bridge.files.insert(m.name.clone(), std::fs::canonicalize(&path).map(|p| p.to_string_lossy().to_string()).unwrap_or(path.clone()));
    }
    // A class of the program built on such a class of another file: refused.
    for m in mods {
        let path = texts.get(&m.name).map(|t| t.0.clone()).unwrap_or_default();
        let mut from_other: HashSet<String> = HashSet::new();
        for s in &m.body {
            if let St::ImportFrom(name, v) = &s.st {
                if let Some(cls) = by_mod.get(name).filter(|_| users.contains(name) && *name != m.name) {
                    for (n, a) in v {
                        if cls.contains(n) {
                            from_other.insert(a.clone().unwrap_or_else(|| n.clone()));
                        }
                    }
                }
            }
        }
        for s in &m.body {
            if let St::Class(c) = &s.st {
                if !bridge.classes.contains_key(&(m.name.clone(), c.line)) && c.bases.iter().filter_map(root_name).any(|n| from_other.contains(n)) {
                    return Err((path, c.line, format!("class {} is built on a library-based class of another file, which dynamic builds leave to CPython", c.name)));
                }
            }
        }
    }
    Ok(())
}

fn jq(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
