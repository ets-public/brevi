// Native builds of Python files that typed inference cannot handle: the program is compiled to Rust over a
// dynamic value type (runtime in rt_*.rs), so mixed-type values work as they do in CPython.
pub mod emit;
pub mod parse;

use std::path::{Path, PathBuf};
use std::process::Command;

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
    include_str!("rt_enum.rs"),
    include_str!("rt_rand.rs"),
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
    let mut k = 0;
    while k < mods.len() {
        let mut imps = vec![];
        walk_imports(&mods[k].body, &mut imps);
        for m in imps {
            if mods.iter().any(|x| x.name == m) || emit::BUILTIN_MODULES.contains(&m.as_str()) {
                continue;
            }
            let p = dir.join(format!("{m}.py"));
            // Outside the sandbox too, when every use of these modules is one Brevi's copies have (checked
            // over all the program's files below). random's generator core is native (rt_rand.rs).
            if !p.exists() && (crate::pybridge::no_native() || own_ok) {
                if let Some(src) = sandbox_module(&m) {
                    own_used = true;
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
                mods.push(emit::Module { name: m, body });
            }
        }
        k += 1;
    }
    if own_used && !own_ok && !crate::pybridge::no_native() {
        return Err((file.to_string(), 0, "os, pathlib, random, io, textwrap or argparse beyond Brevi's own copies runs in CPython".into()));
    }
    let mut rust = emit::program(&mods).map_err(|(l, m)| (file.to_string(), l, m))?;
    if mods.iter().any(|m| m.name == "argparse") && !crate::pybridge::no_native() {
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
