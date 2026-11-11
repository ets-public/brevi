// `brevi-python`: a drop-in for `python3` that builds the program natively when it can and is CPython
// otherwise, so a harness switches with one PATH change (docs/SDK_AND_IMAGES.md, section 2).
//
// The brevi binary acts as the shim when it is started as `brevi-python`, `python3` or `python` (a link or a
// copy under that name), or as `brevi python ARGS`.
//
//   brevi-python script.py [args]    brevi run script.py -- args (native when it builds, CPython otherwise)
//   brevi-python -c CODE [args]      the same, on CODE saved under the cache
//   brevi-python - [args] < script   the same, on the script read from stdin (also with no args, stdin not a terminal)
//   any other flag, -m, no args at a terminal (the REPL), a file not ending in .py, BREVI_SHIM=0
//                                    exec CPython with the same arguments
//
// Brevi says nothing of its own: the {"path":...} line and CPython's whole traceback are as CPython would
// print them, unless BREVI_SHIM_VERBOSE=1. A program that looks at what differs between a native run and
// CPython (sys.executable, sys.argv[0], sys.orig_argv, argparse's program name, __file__ for -c) goes to
// CPython. CPython is BREVI_PYTHON, else the first python3 on PATH that is not this binary.
use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

const NAMES: [&str; 3] = ["brevi-python", "python3", "python"];

// The shim's arguments when this process is the shim: started under one of NAMES, or as `brevi python`.
pub fn args() -> Option<Vec<OsString>> {
    let mut all = std::env::args_os();
    let zero = all.next()?;
    let stem = Path::new(&zero).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let rest: Vec<OsString> = all.collect();
    if NAMES.contains(&stem.as_str()) {
        return Some(rest);
    }
    if rest.first().is_some_and(|a| a == "python") {
        return Some(rest[1..].to_vec());
    }
    None
}

pub fn main(args: Vec<OsString>) -> ! {
    let python = cpython();
    // BREVI_STRICT=1: no fallback, so nothing is handed to CPython (src/main.rs strict). A script still goes to
    // `brevi run`, which holds it to the same rule.
    let exec = |c: &mut Command| -> ! {
        if crate::strict() && c.get_program() == python.as_os_str() {
            eprintln!("brevi-python: BREVI_STRICT=1 allows no fallback, and this would run CPython ({}); unset BREVI_STRICT to let it", c.get_args().map(|a| a.to_string_lossy().to_string()).collect::<Vec<_>>().join(" "));
            std::process::exit(1);
        }
        exec(c)
    };
    if std::env::var("BREVI_SHIM").is_ok_and(|v| v == "0") {
        exec(Command::new(&python).args(&args));
    }
    let first = args.first().map(|a| a.to_string_lossy().to_string());
    let (file, rest, inline) = match first.as_deref() {
        Some("-c") if args.len() >= 2 => {
            let code = args[1].to_string_lossy().to_string();
            (saved(&code), args[2..].to_vec(), Some(code))
        }
        Some("-") => {
            let code = read_stdin();
            (saved(&code), args[1..].to_vec(), Some(code))
        }
        None if !std::io::stdin().is_terminal() => {
            let code = read_stdin();
            (saved(&code), vec![], Some(code))
        }
        Some(a) if !a.starts_with('-') && a.ends_with(".py") => (Some(PathBuf::from(&args[0])), args[1..].to_vec(), None),
        _ => exec(Command::new(&python).args(&args)),
    };
    // A script read from stdin is gone once read, so CPython is handed the saved copy.
    let to_cpython = |file: &Option<PathBuf>| -> ! {
        match (&inline, file) {
            (Some(code), _) if first.as_deref() == Some("-c") => exec(Command::new(&python).arg("-c").arg(code).args(&rest)),
            (Some(code), _) => {
                let mut c = Command::new(&python);
                c.arg("-c").arg(code).args(&rest);
                exec(&mut c)
            }
            _ => exec(Command::new(&python).args(&args)),
        }
    };
    let Some(file) = file else { to_cpython(&None) };
    let src = match &inline {
        Some(c) => c.clone(),
        None => match crate::read_src(&file) {
            Ok(s) => s,
            // Let CPython say what is wrong with the file, in its own words.
            Err(_) => to_cpython(&Some(file)),
        },
    };
    if needs_cpython(&src, inline.is_some()) {
        to_cpython(&Some(file));
    }
    let me = std::env::current_exe().unwrap_or_else(|_| "brevi".into());
    let mut c = Command::new(me);
    c.arg("run").arg(&file).arg("--").args(&rest).env("BREVI_PYTHON", &python);
    if !std::env::var("BREVI_SHIM_VERBOSE").is_ok_and(|v| v == "1") {
        c.env("BREVI_PATH", "0").env("BREVI_TRACEBACK", "1");
    }
    if inline.is_some() && first.as_deref() != Some("-c") {
        // The program's own stdin was its source; CPython would give it an empty stdin.
        c.stdin(std::process::Stdio::null());
    }
    exec(&mut c)
}

// What only CPython answers the same way: the interpreter's own path, argv[0] (a native program's is its
// binary), argparse and optparse (they print argv[0] as the program's name), and __file__ for code that has
// no file in CPython.
fn needs_cpython(src: &str, inline: bool) -> bool {
    const ALWAYS: [&str; 6] = ["sys.executable", "orig_argv", "argparse", "optparse", "__loader__", "__spec__"];
    if ALWAYS.iter().any(|w| src.contains(w)) || (inline && src.contains("__file__")) {
        return true;
    }
    // sys.argv is fine when only its arguments are read: argv[1], argv[2:], len(sys.argv).
    let mut at = 0;
    while let Some(i) = src[at..].find("argv") {
        let start = at + i;
        let after = &src[start + 4..];
        let indexed = after.starts_with('[') && after[1..].starts_with(|c: char| ('1'..='9').contains(&c));
        let counted = src[..start].trim_end_matches("sys.").ends_with("len(") && after.starts_with(')');
        if !indexed && !counted {
            return true;
        }
        at = start + 4;
    }
    false
}

// CPython: BREVI_PYTHON, else the first python3 (then python) on PATH that is not this binary.
fn cpython() -> OsString {
    if let Some(p) = std::env::var_os("BREVI_PYTHON").filter(|p| !p.is_empty()) {
        return p;
    }
    let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exe = |n: &str| if cfg!(windows) { format!("{n}.exe") } else { n.to_string() };
    let names: [&str; 3] = if cfg!(windows) { ["python", "python3", "python3.real"] } else { ["python3", "python3.real", "python"] };
    for name in names {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(exe(name));
            if p.is_file() && p.canonicalize().ok() != me {
                return p.into_os_string();
            }
        }
    }
    eprintln!("brevi-python: no CPython found: set BREVI_PYTHON to the python3 to run when Brevi cannot build a program");
    std::process::exit(127)
}

fn read_stdin() -> String {
    let mut s = String::new();
    let _ = std::io::stdin().read_to_string(&mut s);
    s
}

// Code from -c or stdin, saved as main.py in a folder named by its hash under the build cache, so a repeat of
// the same code is a cache hit. None when it cannot be written (CPython runs it then).
fn saved(code: &str) -> Option<PathBuf> {
    // FNV-1a: stable across runs and Rust versions.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in code.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let dir = crate::cache_dir().join("shim").join(format!("{h:016x}"));
    let file = dir.join("main.py");
    if std::fs::read_to_string(&file).is_ok_and(|s| s == code) {
        return Some(file);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let tmp = dir.join(format!("main.py.{}", std::process::id()));
    std::fs::write(&tmp, code).ok()?;
    std::fs::rename(&tmp, &file).ok()?;
    Some(file)
}

#[cfg(unix)]
fn exec(c: &mut Command) -> ! {
    use std::os::unix::process::CommandExt;
    let e = c.exec();
    eprintln!("brevi-python: cannot start {}: {e}", c.get_program().to_string_lossy());
    std::process::exit(127)
}

#[cfg(not(unix))]
fn exec(c: &mut Command) -> ! {
    match c.status() {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("brevi-python: cannot start {}: {e}", c.get_program().to_string_lossy());
            std::process::exit(127)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::needs_cpython;

    #[test]
    fn sends_what_differs_to_cpython() {
        assert!(!needs_cpython("print(1)\n", false));
        assert!(!needs_cpython("import sys\nprint(sys.argv[1], sys.argv[2:], len(sys.argv))\n", false));
        assert!(needs_cpython("import sys\nprint(sys.argv)\n", false));
        assert!(needs_cpython("import sys\nprint(sys.argv[0])\n", false));
        assert!(needs_cpython("import sys\nprint(sys.executable)\n", false));
        assert!(needs_cpython("import argparse\n", false));
        assert!(!needs_cpython("print(__file__)\n", false));
        assert!(needs_cpython("print(__file__)\n", true));
    }
}
