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

// The Rust program for a Python file and the modules it imports from its own directory.
// Err is (file, line, message).
pub fn to_rust(file: &str) -> Result<String, (String, usize, String)> {
    let src = std::fs::read_to_string(file).map_err(|e| (file.to_string(), 0, e.to_string()))?;
    let body = parse::parse(&src).map_err(|(l, m)| (file.to_string(), l, m))?;
    let dir = Path::new(file).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut mods = vec![emit::Module { name: "__main__".into(), body }];
    let mut k = 0;
    while k < mods.len() {
        let mut imps = vec![];
        walk_imports(&mods[k].body, &mut imps);
        for m in imps {
            if mods.iter().any(|x| x.name == m) || emit::BUILTIN_MODULES.contains(&m.as_str()) {
                continue;
            }
            let p = dir.join(format!("{m}.py"));
            if p.exists() {
                let ps = p.to_string_lossy().to_string();
                let src = std::fs::read_to_string(&p).map_err(|e| (ps.clone(), 0, e.to_string()))?;
                let body = parse::parse(&src).map_err(|(l, m)| (ps.clone(), l, m))?;
                mods.push(emit::Module { name: m, body });
            }
        }
        k += 1;
    }
    let rust = emit::program(&mods).map_err(|(l, m)| (file.to_string(), l, m))?;
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
    let o = Command::new("rustc")
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
pub fn compile(rust: &str, dir: &Path, out: &Path, release: bool, cache: &Path, rustc_v: &str) -> Result<(), String> {
    let lib = runtime_lib(cache, rustc_v)?;
    let opt = if release { "opt-level=3" } else { "opt-level=1" };
    let key = format!("{:016x}", fnv(&[rust, opt, rustc_v, RUNTIME, "dyn"]));
    let cdir = cache.join("bin");
    let cached = cdir.join(&key);
    if cached.exists() && std::fs::copy(&cached, out).is_ok() {
        return Ok(());
    }
    let rs = dir.join("dyn_main.rs");
    std::fs::write(&rs, rust).map_err(|e| e.to_string())?;
    let o = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name", "brevi_prog", "-A", "warnings", "-C", opt, "-C", "overflow-checks=off", "-C", "strip=debuginfo"])
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
    if std::fs::create_dir_all(&cdir).is_ok() {
        let tmp = cdir.join(format!("{key}.{}", std::process::id()));
        if std::fs::copy(out, &tmp).is_ok() {
            std::fs::rename(&tmp, &cached).ok();
        }
    }
    Ok(())
}
