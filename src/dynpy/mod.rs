// Native builds of Python files that typed inference cannot handle: the program is compiled to Rust over a
// dynamic value type (runtime in rt_*.rs), so mixed-type values work as they do in CPython.
pub mod emit;
pub mod parse;
pub mod pyd;
pub mod tok;
pub mod hf;
pub mod typing;

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
    include_str!("rt_zone.rs"),
    include_str!("rt_glue.rs"),
    include_str!("rt_gen.rs"),
    include_str!("rt_async.rs"),
    include_str!("rt_enum.rs"),
    include_str!("rt_typing.rs"),
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
    include_str!("rt_hfuni.rs"),
    include_str!("rt_hfcls.rs"),
    include_str!("rt_hfnorm.rs"),
    include_str!("rt_hfcore.rs"),
    include_str!("rt_hf.rs"),
    include_str!("rt_net.rs"),
    include_str!("rt_db.rs"),
    include_str!("rt_st.rs"),
    include_str!("rt_ort.rs"),
    include_str!("../alloc_rt.rs"),
);

pub const HEADER: &str = "#![allow(warnings)] #[macro_use] extern crate brevi_dyn; use brevi_dyn::*; use std::rc::Rc; use std::cell::{Cell, RefCell};\n";

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

// `from .x import y` in module `name` (a package's __init__ when `pkg`) as the absolute `from name.x import y`
fn absolutize(body: &mut [parse::Stmt], name: &str, pkg: bool) {
    use parse::St;
    for s in body {
        match &mut s.st {
            St::ImportFrom(m, _) if m.starts_with('.') => {
                let dots = m.chars().take_while(|c| *c == '.').count();
                let mut base: Vec<&str> = name.split('.').collect();
                if !pkg {
                    base.pop();
                }
                for _ in 1..dots {
                    base.pop();
                }
                let rest = &m[dots..];
                let mut a = base.join(".");
                if !rest.is_empty() {
                    if !a.is_empty() {
                        a.push('.');
                    }
                    a.push_str(rest);
                }
                *m = a;
            }
            St::Def(d) => absolutize(&mut d.body, name, pkg),
            St::Class(c) => absolutize(&mut c.body, name, pkg),
            St::If(arms, els) => {
                for (_, b) in arms {
                    absolutize(b, name, pkg);
                }
                if let Some(b) = els {
                    absolutize(b, name, pkg);
                }
            }
            St::For(_, _, b, e) | St::While(_, b, e) => {
                absolutize(b, name, pkg);
                if let Some(e) = e {
                    absolutize(e, name, pkg);
                }
            }
            St::Try(b, hs, e, f) => {
                absolutize(b, name, pkg);
                for h in hs {
                    absolutize(&mut h.body, name, pkg);
                }
                if let Some(e) = e {
                    absolutize(e, name, pkg);
                }
                if let Some(f) = f {
                    absolutize(f, name, pkg);
                }
            }
            St::With(_, b) => absolutize(b, name, pkg),
            _ => {}
        }
    }
}
// `from pkg import sub` where sub is a module of the program's package pkg: (pkg, sub) pairs
fn from_names(body: &[parse::Stmt], out: &mut Vec<(String, String)>) {
    use parse::St;
    for s in body {
        match &s.st {
            St::ImportFrom(m, v) => out.extend(v.iter().map(|(n, _)| (m.clone(), n.clone()))),
            St::Def(d) => from_names(&d.body, out),
            St::Class(c) => from_names(&c.body, out),
            St::If(arms, els) => {
                arms.iter().for_each(|(_, b)| from_names(b, out));
                if let Some(b) = els {
                    from_names(b, out);
                }
            }
            St::For(_, _, b, e) | St::While(_, b, e) => {
                from_names(b, out);
                if let Some(e) = e {
                    from_names(e, out);
                }
            }
            St::Try(b, hs, e, f) => {
                from_names(b, out);
                hs.iter().for_each(|h| from_names(&h.body, out));
                for x in [e, f].into_iter().flatten() {
                    from_names(x, out);
                }
            }
            St::With(_, b) => from_names(b, out),
            _ => {}
        }
    }
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
                for (c, b) in arms {
                    // `if TYPE_CHECKING:` imports are for type checkers (the block never runs)
                    if !typing::type_checking(c) {
                        walk_imports(b, out);
                    }
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
// every import in the package's .py files names the package itself, a module Brevi builds in, or a file of the
// program next to the package
fn package_native(pkg: &Path, top: &str) -> bool {
    let Ok(rd) = std::fs::read_dir(pkg) else { return false };
    let parent = pkg.parent().unwrap_or(Path::new("."));
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.join("__init__.py").is_file() && !package_native(&p, top) {
                return false;
            }
            continue;
        }
        if p.extension().and_then(|x| x.to_str()) != Some("py") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&p) else { return false };
        for l in src.lines() {
            let t = l.trim_start();
            let names: Vec<&str> = if let Some(r) = t.strip_prefix("import ") {
                r.split(',').filter_map(|x| x.split_whitespace().next()).collect()
            } else if let Some(r) = t.strip_prefix("from ") {
                r.split_whitespace().next().into_iter().collect()
            } else {
                continue;
            };
            for n in names {
                let first = n.split('.').next().unwrap_or(n);
                if n.starts_with('.') || first == top || emit::builtin_mod(n) || emit::builtin_mod(first) || sandbox_module(n).is_some() || parent.join(format!("{first}.py")).is_file() {
                    continue;
                }
                return false;
            }
        }
    }
    true
}
fn sandbox_module(m: &str) -> Option<&'static str> {
    Some(match m {
        "random" => include_str!("py/random.py"),
        "os" => include_str!("py/os.py"),
        "posixpath" => include_str!("py/posixpath.py"),
        "os.path" => include_str!("py/posixpath.py"),
        "pathlib" => include_str!("py/pathlib.py"),
        "io" => include_str!("py/io.py"),
        "textwrap" => include_str!("py/textwrap.py"),
        "logging" => include_str!("py/logging.py"),
        "argparse" => include_str!("py/argparse.py"),
        "asyncio" => include_str!("py/asyncio.py"),
        "asyncio.subprocess" => include_str!("py/asyncio_subprocess.py"),
        "tiktoken" => include_str!("py/tiktoken.py"),
        "tokenizers" => include_str!("py/tokenizers.py"),
        "requests" => include_str!("py/requests.py"),
        "requests.exceptions" => include_str!("py/requests_exceptions.py"),
        "requests.auth" => include_str!("py/requests_auth.py"),
        "httpx" => include_str!("py/httpx.py"),
        "pydantic" => include_str!("py/pydantic.py"),
        "anthropic" => include_str!("py/anthropic.py"),
        "anthropic.types" => include_str!("py/anthropic_types.py"),
        "_brevi_anthropic_types" => include_str!("py/_brevi_anthropic_types.py"),
        "_brevi_sdk" => include_str!("py/_brevi_sdk.py"),
        "openai" => include_str!("py/openai.py"),
        "openai.types" => include_str!("py/openai_types.py"),
        "openai.types.chat" => include_str!("py/openai_types_chat.py"),
        "openai.types.responses" => include_str!("py/openai_types_responses.py"),
        "_brevi_openai_types" => include_str!("py/_brevi_openai_types.py"),
        "_brevi_http" => include_str!("py/_brevi_http.py"),
        "mcp" => include_str!("py/mcp.py"),
        "mcp.types" | "mcp_types" => include_str!("py/mcp_types.py"),
        "mcp.server" => include_str!("py/mcp_server.py"),
        "mcp.server.mcpserver" => include_str!("py/mcp_server_mcpserver.py"),
        "mcp.server.lowlevel" => include_str!("py/mcp_server_lowlevel.py"),
        "mcp.server.context" => include_str!("py/mcp_server_context.py"),
        "mcp.server.stdio" => include_str!("py/mcp_server_stdio.py"),
        "mcp.server.transport_security" => include_str!("py/mcp_server_transport_security.py"),
        "mcp.shared.exceptions" => include_str!("py/mcp_shared_exceptions.py"),
        "mcp.server.mcpserver.exceptions" => include_str!("py/mcp_server_mcpserver_exceptions.py"),
        "mcp.server.mcpserver.prompts" => include_str!("py/mcp_server_mcpserver_prompts.py"),
        "mcp.server.mcpserver.prompts.base" => include_str!("py/mcp_server_mcpserver_prompts_base.py"),
        "mcp.server.mcpserver.resources" => include_str!("py/mcp_server_mcpserver_resources.py"),
        "mcp.server.mcpserver.tools" => include_str!("py/mcp_server_mcpserver_tools.py"),
        "mcp.server.mcpserver.utilities.types" => include_str!("py/mcp_server_mcpserver_utilities_types.py"),
        "_brevi_mcp" => include_str!("py/_brevi_mcp.py"),
        "_brevi_mcp_models" => include_str!("py/_brevi_mcp_models.py"),
        "_brevi_mcp_types" => include_str!("py/_brevi_mcp_types.py"),
        "contextlib" => include_str!("py/contextlib.py"),
        "_brevi_open" => include_str!("py/_brevi_open.py"),
        "jinja2" => include_str!("py/jinja2.py"),
        "jinja2.ext" => include_str!("py/jinja2_ext.py"),
        "jinja2.exceptions" => include_str!("py/jinja2_exceptions.py"),
        "jinja2.runtime" => include_str!("py/jinja2_runtime.py"),
        "jinja2.utils" => include_str!("py/jinja2_utils.py"),
        "jinja2.loaders" => include_str!("py/jinja2_loaders.py"),
        "markupsafe" => include_str!("py/markupsafe.py"),
        "polars" => include_str!("py/polars.py"),
        "polars.exceptions" => include_str!("py/polars_exceptions.py"),
        "sqlite3" | "sqlite3.dbapi2" => include_str!("py/sqlite3.py"),
        "duckdb" => include_str!("py/duckdb.py"),
        "safetensors" => include_str!("py/safetensors.py"),
        "safetensors.numpy" => include_str!("py/safetensors_numpy.py"),
        "onnxruntime" => include_str!("py/onnxruntime.py"),
        "onnxruntime.capi.onnxruntime_pybind11_state" => include_str!("py/onnxruntime_state.py"),
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
    emit::NP_ATTR_ST.store(false, Relaxed);
    emit::PYD_ON.store(false, Relaxed);
    // Something of numpy that Brevi's own numpy lacks: no dynamic build. CPython's numpy through
    // the dynamic bridge is not used: the typed build's bridge (or CPython itself) runs it right.
    if let Err((f, l, m)) = &r {
        if m.contains("numpy") && std::env::var("BREVI_NUMPY_DEBUG").is_ok() {
            eprintln!("native numpy refused: {f}:{l}: {m}");
        }
    }
    r
}

const ABC_BRIDGE: &str = "collections.abc used as a value runs CPython's ABCs";

fn to_rust_np(file: &str) -> Result<String, (String, usize, String)> {
    emit::ABC_NATIVE.store(true, Ordering::Relaxed);
    let r = to_rust_np2(file);
    if matches!(&r, Err((_, 0, m)) if m == ABC_BRIDGE) {
        emit::ABC_NATIVE.store(false, Ordering::Relaxed);
        let r = to_rust_np2(file);
        emit::ABC_NATIVE.store(true, Ordering::Relaxed);
        return r;
    }
    r
}

fn to_rust_np2(file: &str) -> Result<String, (String, usize, String)> {
    match to_rust_with(file, true) {
        // More of os, pathlib, random, ... than Brevi's copies have: CPython's own modules, through the bridge.
        Err((_, 0, m)) if m == BEYOND_COPIES && bridge_ok() => to_rust_with(file, false),
        r => r,
    }
}

// Where the duckdb Python package's library is (it holds DuckDB's C API) and CPython's library, which it needs
// loaded first: asked of python3 when the program is built, without importing duckdb.
fn duck_libs() -> (Vec<String>, String) {
    const Q: &str = "import importlib.util, glob, os, sysconfig\ns = importlib.util.find_spec('duckdb')\nd = os.path.dirname(list(s.submodule_search_locations)[0]) if s and s.submodule_search_locations else ''\nlibs = sorted(glob.glob(os.path.join(d, '_duckdb*.so'))) if d else []\nlib = os.path.join(sysconfig.get_config_var('LIBDIR') or '', sysconfig.get_config_var('INSTSONAME') or sysconfig.get_config_var('LDLIBRARY') or '')\nprint('\\x01'.join([lib] + libs))";
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let out = std::process::Command::new(&exe).args(["-c", Q]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string()).unwrap_or_default();
    let mut parts = out.split('\u{1}').map(String::from);
    let pre = parts.next().unwrap_or_default();
    (parts.filter(|x| !x.is_empty()).collect(), pre)
}

const BEYOND_COPIES: &str = "os, pathlib, random, io, textwrap, argparse, logging, asyncio, tiktoken, tokenizers, requests, httpx, pydantic, anthropic, openai, jinja2, polars, sqlite3, duckdb, safetensors or onnxruntime beyond Brevi's own copies runs in CPython";

// The ONNX Runtime library inside the onnxruntime Python package (it has the C API), asked of python3 when the
// program is built, without importing onnxruntime.
fn ort_libs() -> Vec<String> {
    const Q: &str = "import importlib.util, glob, os\ns = importlib.util.find_spec('onnxruntime')\nd = list(s.submodule_search_locations)[0] if s and s.submodule_search_locations else ''\nlibs = sorted(glob.glob(os.path.join(d, 'capi', 'libonnxruntime.so*')) + glob.glob(os.path.join(d, 'capi', 'libonnxruntime*.dylib'))) if d else []\nprint('\\x01'.join(libs))";
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let Ok(o) = std::process::Command::new(&exe).args(["-I", "-c", Q]).output() else { return vec![] };
    String::from_utf8_lossy(&o.stdout).trim().split('\x01').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
}

// A program's text in Python's spellings, for a Python-style .bv file written with Brevi's (main.rs's bv_as_py).
static SOURCE: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

pub fn set_source(file: &str, py: String) {
    *SOURCE.lock().unwrap() = Some((file.to_string(), py));
}

pub fn has_source(file: &str) -> bool {
    SOURCE.lock().unwrap().as_ref().is_some_and(|(f, _)| f == file)
}

fn to_rust_with(file: &str, copies: bool) -> Result<String, (String, usize, String)> {
    let given = SOURCE.lock().unwrap().as_ref().filter(|(f, _)| f == file).map(|(_, t)| t.clone());
    let src = match given {
        Some(t) => t,
        None => std::fs::read_to_string(file).map_err(|e| (file.to_string(), 0, e.to_string()))?,
    };
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
    let mut net_used = false;
    let mut stdio_used = false;
    let mut fs_used = false;
    let mut sql_used = false;
    let mut duck_used = false;
    let mut ort_used = false;
    let mut zone_used = false;
    let mut meta_used = false;
    let mut bridge = emit::Bridge::default();
    let mut texts: HashMap<String, (String, String)> = HashMap::new();
    texts.insert("__main__".into(), (file.to_string(), src.clone()));
    let mut k = 0;
    while k < mods.len() {
        let mut imps = vec![];
        walk_imports(&mods[k].body, &mut imps);
        // a module of a package of the program's, named in `from pkg import sub`
        let mut fr = vec![];
        from_names(&mods[k].body, &mut fr);
        for (m, n) in fr {
            let d = dir.join(m.replace('.', "/"));
            if d.join("__init__.py").is_file() && (d.join(format!("{n}.py")).is_file() || d.join(&n).join("__init__.py").is_file()) {
                imps.push(format!("{m}.{n}"));
            }
        }
        // the builtin open(): Brevi's, in _brevi_open (emit.rs loads it when the program uses no CPython library)
        if texts.get(&mods[k].name).is_some_and(|t| uses_open(&t.1)) {
            imps.push("_brevi_open".into());
        }
        for m in imps {
            if mods.iter().any(|x| x.name == m) || emit::builtin_mod(&m) {
                if m == "_brevi_net" {
                    net_used = true;
                }
                if m == "_brevi_stdio" {
                    stdio_used = true;
                }
                if m == "_brevi_fs" {
                    fs_used = true;
                }
                if m == "_brevi_sqlite" {
                    sql_used = true;
                }
                if m == "_brevi_duckdb" {
                    duck_used = true;
                }
                if m == "_brevi_ort" {
                    ort_used = true;
                }
                if m == "importlib.metadata" {
                    meta_used = true;
                }
                if m == "zoneinfo" || m == "tzlocal" {
                    zone_used = true;
                }
                if m == "numpy" {
                    emit::NP_ON.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                continue;
            }
            if emit::np_native() && (m.starts_with("numpy.") || m == "numpy") {
                return Err((file.to_string(), 0, format!("numpy: module {m} is not in Brevi's native numpy")));
            }
            // A program's own module: `a.b` is a/b.py, else the package a/b/__init__.py.
            let base = m.replace('.', "/");
            let p = dir.join(format!("{base}.py"));
            let p = if !m.starts_with('.') && !p.exists() && dir.join(&base).join("__init__.py").is_file() { dir.join(&base).join("__init__.py") } else { p };
            // A package of the program's builds with it only when its own imports all build natively too; one
            // that leans on a library Brevi would hand to CPython goes to CPython whole, like a library.
            let top = m.split('.').next().unwrap_or(&m);
            let p = if p.exists() && (m.contains('.') || p.ends_with("__init__.py")) && !package_native(&dir.join(top), top) { dir.join("\0") } else { p };
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
                let mut body = parse::parse(&src).map_err(|(l, m)| (ps.clone(), l, m))?;
                absolutize(&mut body, &m, p.ends_with("__init__.py"));
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
    // typing names used only where Brevi's typing forms behave as CPython's
    {
        let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
        typing::OWN_PYDANTIC.store(own_mods.contains("pydantic"), Ordering::Relaxed);
        if let Err((m, l, why)) = typing::check(&user) {
            // collections.abc as a value (isinstance(x, Mapping)): CPython's, through the bridge
            if emit::ABC_NATIVE.load(Ordering::Relaxed) && typing::uses_abc(&user) && bridge_ok() && crate::pybridge::py_allowed("collections") {
                return Err((file.to_string(), 0, ABC_BRIDGE.into()));
            }
            let f = texts.get(&m).map_or(file.to_string(), |t| t.0.clone());
            return Err((f, l, why));
        }
    }
    if bridge.on {
        // A coroutine handed to a library CPython runs would not be one of CPython's, nor CPython's awaitables
        // Brevi's: async code builds next to the library only when none can cross (emit::async_crosses_bridge).
        if mods.iter().any(|m| m.name != "asyncio" && emit::uses_async(&m.body)) {
            let bodies: Vec<&[parse::Stmt]> = mods.iter().map(|m| m.body.as_slice()).collect();
            if let Some((l, what)) = emit::async_crosses_bridge(&bodies, &|m: &str| bridge.has(m)) {
                let mut libs: Vec<&str> = bridge.modules.iter().map(|m| m.as_str()).collect();
                libs.sort();
                return Err((file.to_string(), l, format!("async code with a library CPython runs ({}): {what}", libs.join(", "))));
            }
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
    // Brevi's tokenizers: the tokenizer.json files to build in, each read by Brevi's port
    let hf_files = if own_mods.contains("tokenizers") {
        let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
        match hf::check(&user).and_then(|f| hf::files(&crate::cache_dir(), &f)) {
            Ok(b) => Some(b),
            Err(m) => {
                if std::env::var("BREVI_TOK_DEBUG").is_ok() {
                    eprintln!("native tokenizers refused: {m}");
                }
                return Err((file.to_string(), 0, if copies && bridge_ok() && crate::pybridge::py_allowed("tokenizers") { BEYOND_COPIES.into() } else { m }));
            }
        }
    } else {
        None
    };
    // Brevi's pydantic: models' annotations as descriptors, each model checked
    emit::PYD_ON.store(own_mods.contains("pydantic"), Ordering::Relaxed);
    if own_mods.contains("pydantic") {
        let (classes, models) = {
            let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
            (pyd::class_names(&user), pyd::models(&user))
        };
        for m in mods.iter_mut().filter(|m| !own_mods.contains(&m.name)) {
            pyd::rewrite(&mut m.body, &classes, &models);
        }
        let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
        if let Err((m, l, why)) = pyd::check(&user) {
            if std::env::var("BREVI_PYD_DEBUG").is_ok() {
                eprintln!("native pydantic refused: {m}:{l}: {why}");
            }
            if copies && bridge_ok() && crate::pybridge::py_allowed("pydantic") {
                return Err((file.to_string(), 0, BEYOND_COPIES.into()));
            }
            let f = texts.get(&m).map_or(file.to_string(), |t| t.0.clone());
            return Err((f, l, why));
        }
    }
    // Brevi's mcp: each function's signature for the SDK's tool / resource / prompt machinery (pyd::mcp_sigs)
    if own_mods.contains("_brevi_mcp") {
        let user: Vec<(&str, &[parse::Stmt])> = mods.iter().filter(|m| !own_mods.contains(&m.name)).map(|m| (m.name.as_str(), m.body.as_slice())).collect();
        let (classes, models) = (pyd::class_names(&user), pyd::models(&user));
        let mut extra: HashSet<String> = include_str!("py/_brevi_mcp_models.py")
            .lines()
            .filter_map(|l| l.strip_prefix("class ").and_then(|r| r.split('(').next()).map(|n| n.to_string()))
            .collect();
        for n in ["Context", "Image", "Audio", "Message", "UserMessage", "AssistantMessage", "ServerSession", "ServerRequestContext", "MCPServer"] {
            extra.insert(n.to_string());
        }
        for m in mods.iter_mut().filter(|m| !own_mods.contains(&m.name)) {
            if let Err((l, why)) = pyd::mcp_sigs(&mut m.body, &classes, &models, &extra) {
                if std::env::var("BREVI_MCP_DEBUG").is_ok() {
                    eprintln!("native mcp refused: {}:{l}: {why}", m.name);
                }
                if copies && bridge_ok() && crate::pybridge::py_allowed("mcp") {
                    return Err((file.to_string(), 0, BEYOND_COPIES.into()));
                }
                let f = texts.get(&m.name).map_or(file.to_string(), |t| t.0.clone());
                return Err((f, l, why));
            }
        }
    }
    // arrays from safetensors files and ONNX models are often float32 / int32, whatever the program names
    let narrow_libs = ["safetensors", "safetensors.numpy", "onnxruntime"].iter().any(|m| own_mods.contains(*m));
    emit::NP_ATTR_ST.store(own_mods.contains("safetensors"), std::sync::atomic::Ordering::Relaxed);
    emit::NP_NARROW.store(emit::NP_ON.load(std::sync::atomic::Ordering::Relaxed) && (narrow_libs || texts.values().any(|(_, t)| emit::names_narrow(t))), std::sync::atomic::Ordering::Relaxed);
    *emit::OWN_COPIES.lock().unwrap() = own_mods.iter().cloned().collect();
    emit::ic_reset();
    let (mut rust, used) = emit::program(&mods, bridge).map_err(|(md, l, m)| {
        let f = match texts.get(&md) {
            Some((p, _)) => p.clone(),
            None if md != "__main__" => format!("<{md}>"),
            None => file.to_string(),
        };
        (f, l, m)
    })?;
    let walrus = mods.iter().any(|m| {
        let mut w = false;
        emit::visit_any(&m.body, &mut |x| w |= matches!(x, parse::Ex::Walrus(..)));
        w
    });
    if !walrus {
        rust = borrow_loads(&rust);
    }
    rust.push_str(&emit::ic_statics());
    if !used.is_empty() {
        crate::pybridge::set_file(file);
        rust.push_str(&crate::pybridge::dyn_runtime());
        rust.push_str(GLUE);
        *BRIDGED.lock().unwrap() = used;
    }
    if let Some(b) = &tok_files {
        rust = tok::embed(rust, b);
    }
    if let Some(b) = &hf_files {
        rust = hf::embed(rust, b);
    }
    if net_used {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own requests and httpx run on Linux and macOS".into()));
        }
        rust = rust.replacen("fn main() { ", "fn main() { NET.set(brevi_netglue::ops()).ok(); ", 1);
        rust.push_str(&net_glue());
    }
    // child processes (Brevi's asyncio create_subprocess_exec / _shell): the glue only where the program starts one
    if own_mods.contains("asyncio") || mods.iter().any(|m| m.name == "asyncio") {
        if texts.iter().any(|(n, (_, t))| n != "asyncio" && t.contains("create_subprocess_")) {
            if !cfg!(unix) {
                return Err((file.to_string(), 0, "Brevi's own asyncio subprocesses run on Linux and macOS".into()));
            }
            rust = rust.replacen("fn main() { ", "fn main() { PROC.set(brevi_procglue::ops()).ok(); ", 1);
            rust.push('\n');
            rust.push_str(include_str!("procglue.rs"));
        }
    }
    // an HTTP server's sockets (Brevi's mcp, transport="streamable-http" or "sse"): only where the program may serve one
    if stdio_used && texts.iter().any(|(n, (_, t))| !own_mods.contains(n) && (t.contains("streamable") || t.contains("\"sse\"") || t.contains("'sse'") || t.contains("run_sse"))) {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own mcp runs on Linux and macOS".into()));
        }
        rust = rust.replacen("fn main() { ", "fn main() { SRV.set(brevi_srvglue::ops()).ok(); ", 1);
        // under --sandbox a server listens only on a host the caller granted (--allow-net HOST, or --allow net)
        let (any, hosts) = crate::caps::net_hosts();
        let list: Vec<String> = hosts.iter().map(|h| format!("{h:?}")).collect();
        rust.push_str(&format!("\nconst SRV_ANY: bool = {any};\nconst SRV_HOSTS: &[&str] = &[{}];\n", list.join(", ")));
        rust.push_str(include_str!("srvglue.rs"));
    }
    if stdio_used {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own mcp runs on Linux and macOS".into()));
        }
        rust = rust.replacen("fn main() { ", "fn main() { STDIO.set(brevi_stdioglue::ops()).ok(); ", 1);
        rust.push('\n');
        rust.push_str(include_str!("stdioglue.rs"));
    }
    if meta_used {
        let all: Vec<String> = texts.values().map(|(_, t)| t.clone()).collect();
        rust = rust.replacen("fn main() { ", "fn main() { DISTS.set(BREVI_DISTS).ok(); ", 1);
        rust.push_str(&dist_table(&all));
    }
    if zone_used {
        rust = rust.replacen("fn main() { ", "fn main() { ZONE_FS.set(brevi_zoneglue::ops()).ok(); ", 1);
        rust.push_str(include_str!("zoneglue.rs"));
    }
    if fs_used || sql_used || duck_used || ort_used || net_used {
        rust.push_str(&fs_consts());
    }
    if fs_used {
        rust = rust.replacen("fn main() { ", "fn main() { FS.set(brevi_fsglue::ops()).ok(); ", 1);
        rust.push_str(include_str!("fsglue.rs"));
    }
    if sql_used {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own sqlite3 runs on Linux and macOS".into()));
        }
        rust = rust.replacen("fn main() { ", "fn main() { SQL.set(brevi_sqlglue::ops()).ok(); ", 1);
        rust.push_str(include_str!("sqlglue.rs"));
    }
    if duck_used {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own duckdb runs on Linux and macOS".into()));
        }
        let (libs, pre) = duck_libs();
        rust = rust.replacen("fn main() { ", "fn main() { DUCK.set(brevi_duckglue::ops()).ok(); ", 1);
        rust.push_str(&format!("\nconst DUCK_LIBS: &[&str] = &{libs:?};\nconst DUCK_PRELOAD: &str = {pre:?};\n"));
        rust.push_str(include_str!("duckglue.rs"));
    }
    if ort_used {
        if !cfg!(unix) {
            return Err((file.to_string(), 0, "Brevi's own onnxruntime runs on Linux and macOS".into()));
        }
        rust = rust.replacen("fn main() { ", "fn main() { ORT.set(brevi_ortglue::ops()).ok(); ", 1);
        rust.push_str(&format!("\nconst ORT_LIBS: &[&str] = &{:?};\n", ort_libs()));
        rust.push_str(include_str!("ortglue.rs"));
    }
    if mods.iter().any(|m| m.name == "argparse") && crate::pybridge::py_allowed("argparse") {
        rust = rust.replacen("run_main(__run); }", "TERM_COLS.set(__term_cols).ok(); run_main(__run); }", 1);
        rust.push_str(TERM_COLS_FN);
    }
    Ok(format!("{HEADER}{rust}"))
}

// What the caller granted the program's files under --sandbox, for the glue that opens them (fsglue.rs,
// sqlglue.rs, duckglue.rs, ortglue.rs, netglue.rs' CA files): read and write, and the folders each is held to
// (--allow-read DIR, --allow-write DIR; None: every file). brevi_fs_path checks a path against them.
fn fs_consts() -> String {
    let scope = |cap: &str| match crate::caps::fs_scope(cap) {
        None => "None".to_string(),
        Some(r) => format!("Some(&[{}])", r.iter().map(|x| format!("{x:?}")).collect::<Vec<_>>().join(", ")),
    };
    format!(
        "\nconst FS_SANDBOX: bool = {};\nconst FS_READ: bool = {};\nconst FS_WRITE: bool = {};\nconst FS_READ_IN: Option<&[&str]> = {};\nconst FS_WRITE_IN: Option<&[&str]> = {};\n{}{}",
        crate::caps::sandboxed(),
        crate::caps::granted("read"),
        crate::caps::granted("write"),
        scope("read"),
        scope("write"),
        include_str!("../fsscope.rs"),
        FS_PATH_FN,
    )
}

const FS_PATH_FN: &str = r#"
// The path to open for a program's file: itself, or under folder-scoped grants the checked real path.
fn brevi_fs_path(p: &str, write: bool) -> Result<String, String> {
    let (scope, cap) = if write { (FS_WRITE_IN, "write") } else { (FS_READ_IN, "read") };
    match scope {
        None => Ok(p.to_string()),
        Some(r) => brevi_scoped(p, r, cap),
    }
}
"#;

// The network for Brevi's requests and httpx (netglue.rs), with the hosts the caller granted under --sandbox.
fn net_glue() -> String {
    let (any, hosts) = crate::caps::net_hosts();
    let list: Vec<String> = hosts.iter().map(|h| format!("{h:?}")).collect();
    format!(
        "\nconst NET_ANY: bool = {any};\nconst NET_HOSTS: &[&str] = &[{}];\nconst NET_SANDBOX: bool = {};\nconst NET_READ: bool = {};\n{}",
        list.join(", "),
        crate::caps::sandboxed(),
        crate::caps::granted("read"),
        include_str!("netglue.rs")
    )
}

// The runtime as a library for a typed program's engine (engine.rs).
pub fn runtime_lib_path() -> Result<PathBuf, String> {
    runtime_lib(&crate::cache_dir(), crate::rustc_version())
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
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        // as in compile: without borrow_loads's borrows
        if rust.contains("ldr(&") && ["E0506", "E0505", "E0502", "E0499"].iter().any(|c| err.contains(c)) {
            return check(&rust.replace("ldr(&", "&ld(&"), dir, cache, rustc_v);
        }
        return Err(err);
    }
    if use_cache && std::fs::create_dir_all(cache.join("bin")).is_ok() {
        std::fs::write(&mark, "").ok();
    }
    Ok(())
}

// `&ld(&v_x, "x")?` clones a local only to borrow it: `ldr` borrows it in place. A local is only assigned
// inside an expression by `:=`, which could then clash with the borrow, so programs with one keep `ld`.
fn borrow_loads(src: &str) -> String {
    let pat = "&ld(&";
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find(pat) {
        out.push_str(&rest[..i]);
        let tail = &rest[i + pat.len()..];
        let id_len = tail.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
        let after = &tail[id_len..];
        let lit_end = after.strip_prefix(", \"").and_then(|a| a.find('"').map(|k| k + 4));
        match lit_end {
            Some(k) if id_len > 0 && !out.ends_with('&') && after[k..].starts_with(")?") && !after[3..k - 1].contains('\\') => {
                out.push_str("ldr(&");
                out.push_str(&tail[..id_len]);
                out.push_str(&after[..k + 2]);
                rest = &after[k + 2..];
            }
            _ => {
                out.push_str(pat);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}
pub fn compile(rust: &str, dir: &Path, out: &Path, release: bool, cache: &Path, rustc_v: &str) -> Result<(), String> {
    let opt = if release { "opt-level=3" } else { "opt-level=1" };
    // The edit loop (src/tier.rs): an opt-level 1 binary not built yet is built at opt-level 0 now, and at
    // opt-level 1 in the background.
    if !release && crate::tier::on() {
        let dest = cache.join("bin").join(bin_key(rust, opt, rustc_v));
        if !dest.exists() {
            crate::tm("tiered: building at opt-level 0, opt-level 1 in the background");
            let built = build(rust, dir, out, "opt-level=0", cache, rustc_v)?;
            let mut args = rustc_args(opt, &runtime_lib(cache, rustc_v)?);
            args.extend(["-C".into(), crate::codegen_units().into()]);
            crate::tier::background(&dest, &built, &args);
            return Ok(());
        }
    }
    build(rust, dir, out, opt, cache, rustc_v).map(|_| ())
}

fn bin_key(rust: &str, opt: &str, rustc_v: &str) -> String {
    format!("{:016x}", fnv(&[rust, opt, rustc_v, RUNTIME, "dyn"]))
}

fn rustc_args(opt: &str, lib: &Path) -> Vec<String> {
    let mut a: Vec<String> = ["--edition", "2021", "--crate-name", "brevi_prog", "-A", "warnings", "-C", opt, "-C", "overflow-checks=off", "-C", "strip=debuginfo"].map(String::from).to_vec();
    a.extend(["--extern".into(), format!("brevi_dyn={}", lib.display())]);
    a
}

// Builds at `opt`, through the build cache: the Rust that compiled (the program, or its plainer form).
fn build(rust: &str, dir: &Path, out: &Path, opt: &str, cache: &Path, rustc_v: &str) -> Result<String, String> {
    let lib = runtime_lib(cache, rustc_v)?;
    let release = opt == "opt-level=3";
    let key = bin_key(rust, opt, rustc_v);
    // BREVI_DYN_SRC=file: save the generated Rust there (for profiling a dynamic build)
    if let Ok(p) = std::env::var("BREVI_DYN_SRC") {
        let _ = std::fs::write(p, rust);
    }
    let cdir = cache.join("bin");
    let cached = cdir.join(&key);
    let use_cache = crate::runcache::on();
    // borrow_loads's `ldr` borrows can clash with an assignment inside the same expression: such a program
    // builds once more with every local cloned, as before that pass
    let plain = || rust.replace("ldr(&", "&ld(&");
    if use_cache && cached.exists() && crate::copy_bin(&cached, out) {
        crate::touch(&cached);
        let used = if cached.with_extension("plain").exists() { plain() } else { rust.to_string() };
        return Ok(used);
    }
    let rs = dir.join("dyn_main.rs");
    let run = |src: &str| -> Result<std::process::Output, String> {
        std::fs::write(&rs, src).map_err(|e| e.to_string())?;
        crate::rustc_cmd()
            .args(rustc_args(opt, &lib))
            .args(if release { None } else { Some(["-C", crate::codegen_units()]) }.into_iter().flatten())
            .arg("-o")
            .arg(out)
            .arg(&rs)
            .output()
            .map_err(|e| e.to_string())
    };
    let o = run(rust)?;
    let mut used = rust.to_string();
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        if rust.contains("ldr(&") && (err.contains("E0506") || err.contains("E0505") || err.contains("E0502") || err.contains("E0499")) {
            used = plain();
            let o2 = run(&used)?;
            if !o2.status.success() {
                return Err(String::from_utf8_lossy(&o2.stderr).to_string());
            }
        } else {
            return Err(err);
        }
    }
    if use_cache && std::fs::create_dir_all(&cdir).is_ok() {
        let tmp = cdir.join(format!("{key}.{}", std::process::id()));
        if std::fs::copy(out, &tmp).is_ok() && std::fs::rename(&tmp, &cached).is_ok() {
            if used != rust {
                std::fs::write(cached.with_extension("plain"), "").ok();
            }
            crate::runcache::prune(&cdir);
        } else {
            std::fs::remove_file(&tmp).ok();
        }
    }
    Ok(used)
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

// importlib.metadata.version's table: each distribution installed for the build machine's python3 whose name
// the program spells as a string, with its version. A name not in it raises PackageNotFoundError.
fn dist_table(texts: &[String]) -> String {
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    const Q: &str = "import importlib.metadata as m\nfor d in m.distributions():\n    n = d.metadata['Name']\n    if n: print(n + '\\t' + d.version)";
    let out = std::process::Command::new(&exe).args(["-I", "-c", Q]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let mut named: HashSet<String> = HashSet::new();
    for t in texts {
        for q in ['"', '\''] {
            for line in t.lines() {
                for (i, part) in line.split(q).enumerate() {
                    if i % 2 == 1 && part.len() < 120 {
                        named.insert(crate::dynpy::rt_zone_norm(part));
                    }
                }
            }
        }
    }
    let mut rows: Vec<(String, String)> = vec![];
    for l in out.lines() {
        if let Some((n, v)) = l.split_once('\t') {
            let k = crate::dynpy::rt_zone_norm(n);
            if named.contains(&k) && !rows.iter().any(|(a, _)| *a == k) {
                rows.push((k, v.to_string()));
            }
        }
    }
    rows.sort();
    let body: Vec<String> = rows.iter().map(|(n, v)| format!("({n:?}, {v:?})")).collect();
    format!("\nconst BREVI_DISTS: &[(&str, &str)] = &[{}];\n", body.join(", "))
}
// The same normalising as the runtime's dist_norm (rt_zone.rs), at build time.
pub fn rt_zone_norm(n: &str) -> String {
    let mut o = String::new();
    let mut sep = false;
    for c in n.chars() {
        if matches!(c, '-' | '_' | '.') {
            sep = true;
            continue;
        }
        if sep {
            o.push('_');
        }
        sep = false;
        o.extend(c.to_lowercase());
    }
    if sep {
        o.push('_');
    }
    o
}

// Whether the source names the builtin open (a call, or open passed as a value).
fn uses_open(src: &str) -> bool {
    src.match_indices("open").any(|(i, _)| {
        let before = src[..i].chars().next_back();
        let after = src[i + 4..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}
