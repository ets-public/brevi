// The one CPython Brevi asks and runs: CPython 3.13. Brevi answers as CPython 3.13 does (the conformance
// scores compare with 3.13.13), so every CPython it uses must be a 3.13: the interpreter a library program
// bridges to (its libpython, standard library and packages, found when the program is built and checked again
// by the program when it loads libpython, pyrt_load.rs), the one asked about modules, classes and installed
// packages at build time, the fallback for a program Brevi cannot build, `brevi verify`'s reference and the
// brevi-python shim's CPython. Another version is refused with a clear error, never used: a 3.14 or 3.15
// answers some calls differently (pickle's default protocol, for one), which would be a silent wrong answer.
//
// Which one: BREVI_PYTHON when set (it must be a CPython 3.13; nothing else is tried), else the first CPython
// 3.13 among python3, python3.13 and python on PATH (not this binary, which may be the shim), then (Windows)
// the py launcher's 3.13, then `uv python find 3.13`. Asked once per process.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const WANT: &str = "3.13";

// Prints the implementation, the version and the interpreter's path.
const PROBE: &str = "import sys\nprint(sys.implementation.name, '%d.%d.%d' % sys.version_info[:3], sys.executable)";

// What a candidate answers: (implementation, version, sys.executable), or None if it does not start.
fn probe(mut c: Command) -> Option<(String, String, String)> {
    // BREVI_SHIM=0: a brevi-python shim answers as the CPython behind it, without building anything.
    // BREVI_CPYTHON_PROBE: a brevi copy reached this way does not search in turn (two copies would ask each other).
    let o = c.arg("-c").arg(PROBE).env("BREVI_SHIM", "0").env("BREVI_CPYTHON_PROBE", "1").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if !o.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let mut it = s.splitn(3, ' ');
    Some((it.next()?.to_string(), it.next()?.to_string(), it.next().unwrap_or("").to_string()))
}

// Whether a probe's answer is a CPython 3.13 (any 3.13.x: patch releases fix bugs, they do not change answers).
pub fn is_wanted(imp: &str, ver: &str) -> bool {
    imp == "cpython" && ver.strip_prefix(WANT).is_some_and(|r| r.starts_with('.'))
}

// The files named n on PATH, in order, leaving out this binary (it may be the shim, linked as python3).
fn on_path(n: &str) -> Vec<PathBuf> {
    let me = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let file = if cfg!(windows) { format!("{n}.exe") } else { n.to_string() };
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).map(|d| d.join(&file)).filter(|p| p.is_file() && p.canonicalize().ok() != me).collect()
}

// python3 first, so the usual name is the one shown (`"by":"python3"`) where it is a 3.13.
// The first file named n on PATH, this binary included.
fn first_on_path(n: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) { format!("{n}.exe") } else { n.to_string() };
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|d| d.join(&file)).find(|p| p.is_file())
}

const NAMES: [&str; 4] = if cfg!(windows) { ["python", "python3.13", "python3", "python3.real"] } else { ["python3", "python3.13", "python3.real", "python"] };

// The files a search would probe, without starting any (the run cache keys on them, runcache.rs).
pub fn candidates() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("BREVI_PYTHON").filter(|p| !p.is_empty()) {
        return vec![PathBuf::from(p)];
    }
    NAMES.iter().flat_map(|n| on_path(n)).collect()
}

fn find() -> Result<String, String> {
    if std::env::var_os("BREVI_CPYTHON_PROBE").is_some() {
        return Err("a copy of brevi was asked which CPython it is, and it is none".into());
    }
    let fix = format!("install CPython {WANT} (for example `uv python install {WANT}`, then put it on PATH or set BREVI_PYTHON to it)");
    if let Some(p) = std::env::var_os("BREVI_PYTHON").filter(|p| !p.is_empty()) {
        let shown = p.to_string_lossy().to_string();
        return match probe(Command::new(&p)) {
            Some((i, v, _)) if is_wanted(&i, &v) => Ok(shown),
            Some((i, v, _)) => Err(format!("BREVI_PYTHON ({shown}) is {i} {v}, and Brevi answers as CPython {WANT} does, so it runs only CPython {WANT}: set BREVI_PYTHON to a CPython {WANT}")),
            None => Err(format!("BREVI_PYTHON ({shown}) does not start: set it to a CPython {WANT}")),
        };
    }
    let mut seen: Vec<String> = vec![];
    let mut try_one = |c: Command, name: String| -> Option<String> {
        match probe(c) {
            Some((i, v, exe)) if is_wanted(&i, &v) => Some(if exe.is_empty() { name } else { exe }),
            Some((i, v, _)) => {
                if !seen.iter().any(|s| s.starts_with(&format!("{name} "))) {
                    seen.push(format!("{name} is {i} {v}"));
                }
                None
            }
            None => None,
        }
    };
    for n in NAMES {
        for (k, p) in on_path(n).into_iter().enumerate() {
            let shown = p.to_string_lossy().to_string();
            if let Some(e) = try_one(Command::new(&p), shown) {
                // The bare name when it is the first on PATH (what `python3 x.py` runs), else its path.
                return Ok(if k == 0 && first_on_path(n).as_ref() == Some(&p) { n.to_string() } else { e });
            }
        }
    }
    if cfg!(windows) {
        let mut c = Command::new("py");
        c.arg(format!("-{WANT}"));
        if let Some(e) = try_one(c, format!("py -{WANT}")) {
            return Ok(e);
        }
    }
    // uv's own CPython builds (`uv python install 3.13`), found without downloading anything.
    if let Ok(o) = Command::new("uv").args(["python", "find", "--no-project", WANT]).env("UV_PYTHON_DOWNLOADS", "never").stdin(Stdio::null()).stderr(Stdio::null()).output() {
        let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if o.status.success() && !p.is_empty() {
            if let Some(e) = try_one(Command::new(&p), p.clone()) {
                return Ok(e);
            }
        }
    }
    let found = if seen.is_empty() { "found no CPython".to_string() } else { format!("found only {}", seen.join(", ")) };
    Err(format!("Brevi answers as CPython {WANT} does, so it runs only CPython {WANT}, and {found}: {fix}"))
}

// The CPython 3.13 to ask or run, or why there is none (asked once).
pub fn exe() -> Result<String, String> {
    static R: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    R.get_or_init(find).clone()
}

// The same, as an OsString for Command::new.
pub fn exe_os() -> Result<OsString, String> {
    exe().map(OsString::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanted() {
        assert!(is_wanted("cpython", "3.13.0"));
        assert!(is_wanted("cpython", "3.13.13"));
        assert!(!is_wanted("cpython", "3.130.1"));
        assert!(!is_wanted("cpython", "3.14.0"));
        assert!(!is_wanted("cpython", "3.15.2"));
        assert!(!is_wanted("cpython", "3.12.9"));
        assert!(!is_wanted("pypy", "3.13.1"));
    }
}
