// The result cache: a repeated `brevi check`, `build` or `run` of files that did not change skips the whole
// build (parse, inference, rustc) and replays what the build printed, with the binary it made.
//
// The build runs as a child (`brevi build` or `brevi check` with BREVI_PHASE=1), so everything it prints is
// caught exactly, path report line included. The key is a hash of:
//   - the brevi binary (version, size and modification time), the toolchain (PATH, rustup settings) and
//     the python3 on PATH,
//   - the command and its flags, and every BREVI_* and Python setting in the environment,
//   - the file's path and the text of every .py and .bv file next to it (the modules it may import; also
//     those in package folders it imports).
// A success is kept with its binary; a refusal is kept only when nothing outside the program's folder and
// Python's standard library can change it (a `pip install` later must not meet an old "cannot build").
// Entries live in $BREVI_CACHE/runs (default ~/.cache/brevi/runs), least recently used first out once the
// cache passes BREVI_CACHE_MB (default 1024). `--no-cache` or BREVI_NO_CACHE=1 skips it.
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Done {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    // None when the build was killed by a signal.
    pub code: Option<i32>,
    pub hit: bool,
}

impl Done {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn turn_off() {
    OFF.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn on() -> bool {
    !OFF.load(std::sync::atomic::Ordering::Relaxed) && std::env::var("BREVI_NO_CACHE").map_or(true, |v| v.is_empty() || v == "0")
}

// Inside the child build: the cache is the parent's business.
pub fn in_child() -> bool {
    std::env::var("BREVI_PHASE").is_ok_and(|v| v == "1")
}

// The line a child build prints on stderr when its result is about the machine rather than the program (see
// `not_kept`). The parent takes it out of what it prints and does not keep the result.
const NOT_KEPT: &str = "{\"brevi_cache\":\"not kept\"}";

// Called where a build failed for a reason outside the program's files: rustc killed or unable to write, the
// dynamic runtime library not built (many builds starting it at once on a cold cache, a full disk). The
// refusal printed afterwards may still name a program line (the typed build's reason, when the dynamic build
// was the way out), so the parent could not tell it from a real one: it must not be replayed later.
pub fn not_kept() {
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if in_child() && !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("{NOT_KEPT}");
    }
}

// The line (then a tab and a path) a child build prints on stderr when the binary it made is a quick
// opt-level 0 one (src/tier.rs): the optimised binary will land at that path. The parent keeps the entry with
// the quick binary and the path, and a replay swaps the optimised binary in once it is there.
const UPGRADE: &str = "{\"brevi_cache\":\"upgrade\"}";

pub fn upgrade_at(path: &Path) {
    if in_child() {
        eprintln!("{UPGRADE}\t{}", path.display());
    }
}

// Takes the child's not-kept line out of its stderr: true when it was there.
fn take_not_kept(stderr: &mut Vec<u8>) -> bool {
    let text = String::from_utf8_lossy(stderr);
    if !text.lines().any(|l| l == NOT_KEPT) {
        return false;
    }
    let kept: String = text.split_inclusive('\n').filter(|l| l.trim_end_matches(['\n', '\r']) != NOT_KEPT).collect();
    *stderr = kept.into_bytes();
    true
}

// Takes the child's upgrade lines out of its stderr: the path of the last one, if any.
fn take_upgrade(stderr: &mut Vec<u8>) -> Option<String> {
    let text = String::from_utf8_lossy(stderr).to_string();
    let at = |l: &str| l.trim_end_matches(['\n', '\r']).strip_prefix(UPGRADE).and_then(|r| r.strip_prefix('\t')).map(String::from);
    let path = text.lines().filter_map(at).last()?;
    let kept: String = text.split_inclusive('\n').filter(|l| at(l).is_none()).collect();
    *stderr = kept.into_bytes();
    Some(path)
}

// Runs `brevi ARGS` (a check, or a build to `bin`), or replays it from the cache. `file` is the program.
pub fn phase(args: &[String], file: &str, bin: Option<&Path>, envs: &[(&str, &str)], refusals: bool) -> Done {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    let key = if on() { key(args, file, envs, &me) } else { None };
    let dir = key.as_ref().map(|k| crate::cache_dir().join("runs").join(k));
    if let Some(d) = &dir {
        if let Some(done) = replay(d, bin) {
            return done;
        }
    }
    let mut c = Command::new(&me);
    c.args(args).env("BREVI_PHASE", "1");
    for (k, v) in envs {
        c.env(k, v);
    }
    crate::proc::die_with_parent(&mut c);
    let mut done = match c.output() {
        Ok(o) => Done { stdout: o.stdout, stderr: o.stderr, code: o.status.code(), hit: false },
        Err(e) => Done { stdout: format!("{{\"e\":\"io\",\"msg\":\"{}\"}}\n", crate::diag::esc(&e.to_string())).into_bytes(), stderr: vec![], code: Some(1), hit: false },
    };
    let machine = take_not_kept(&mut done.stderr);
    let upgrade = take_upgrade(&mut done.stderr);
    if let Some(d) = &dir {
        if !machine && (done.ok() || (refusals && refusal_kept(&done, file))) {
            save(d, &done, bin, upgrade.as_deref().filter(|_| done.ok()));
        }
    }
    done
}

fn replay(d: &Path, bin: Option<&Path>) -> Option<Done> {
    let code: i32 = std::fs::read_to_string(d.join("code")).ok()?.trim().parse().ok()?;
    let stdout = std::fs::read(d.join("out")).ok()?;
    let stderr = std::fs::read(d.join("err")).ok()?;
    // A quick (opt-level 0) entry whose optimised binary has landed: it takes the quick one's place.
    if let Ok(at) = std::fs::read_to_string(d.join("upgrade")) {
        let at = Path::new(at.trim());
        let tmp = d.join(format!("bin.tmp{}", std::process::id()));
        if at.is_file() && crate::copy_bin(at, &tmp) && std::fs::rename(&tmp, d.join("bin")).is_ok() {
            std::fs::remove_file(d.join("upgrade")).ok();
            crate::tm("tiered: the opt-level 1 binary took the quick one's place");
        } else {
            std::fs::remove_file(&tmp).ok();
        }
    }
    if code == 0 {
        if let Some(b) = bin {
            if !crate::copy_bin(&d.join("bin"), b) {
                return None;
            }
        }
    }
    crate::touch(&d.join("code"));
    Some(Done { stdout, stderr, code: Some(code), hit: true })
}

// Successes are always kept. A refusal (exit 1, or 3 for a sure mistake) is kept only for a program that
// imports nothing an install could change, and never one that may be about the machine rather than the
// program: no rustc, a missing folder, a full disk, a crate download (errors with no line of the program).
fn refusal_kept(done: &Done, file: &str) -> bool {
    let text = String::from_utf8_lossy(&done.stdout);
    matches!(done.code, Some(1 | 3)) && !text.contains("\"e\":") && !text.contains("cargo") && !text.contains("\"line\":0") && only_local_and_stdlib(file)
}

fn save(d: &Path, done: &Done, bin: Option<&Path>, upgrade: Option<&str>) {
    let runs = d.parent().unwrap();
    if std::fs::create_dir_all(runs).is_err() {
        return;
    }
    let tmp = runs.join(format!(".tmp-{}", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    if std::fs::create_dir_all(&tmp).is_err() {
        return;
    }
    let mut ok = std::fs::write(tmp.join("out"), &done.stdout).is_ok() && std::fs::write(tmp.join("err"), &done.stderr).is_ok();
    if done.ok() {
        if let Some(b) = bin {
            ok = ok && crate::copy_bin(b, &tmp.join("bin"));
        }
    }
    if let Some(u) = upgrade {
        ok = ok && std::fs::write(tmp.join("upgrade"), u).is_ok();
    }
    // `code` last: an entry without it is never read.
    ok = ok && std::fs::write(tmp.join("code"), done.code.unwrap_or(1).to_string()).is_ok();
    if ok {
        std::fs::remove_dir_all(d).ok();
        if std::fs::rename(&tmp, d).is_ok() {
            prune(runs);
            return;
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
}

fn key(args: &[String], file: &str, envs: &[(&str, &str)], me: &Path) -> Option<String> {
    let mut parts: Vec<String> = vec![env!("CARGO_PKG_VERSION").into()];
    let m = std::fs::metadata(me).ok()?;
    parts.push(format!("{} {:?}", m.len(), m.modified().ok()?));
    parts.push(crate::toolchain_key().unwrap_or_default());
    // The output path (-o) changes nothing the build prints, and the binary is copied there afterwards.
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            parts.push("-o".into());
            continue;
        }
        skip = a == "-o";
        if !skip {
            parts.push(a.clone());
        }
    }
    for (k, v) in envs {
        parts.push(format!("{k}={v}"));
    }
    let mut vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| (k.starts_with("BREVI_") && !matches!(k.as_str(), "BREVI_TIME" | "BREVI_NO_CACHE" | "BREVI_CACHE_MB" | "BREVI_PHASE" | "BREVI_MEM_MB")) || matches!(k.as_str(), "PYTHONPATH" | "PYTHONHOME" | "VIRTUAL_ENV" | "CONDA_PREFIX"))
        .collect();
    vars.sort();
    parts.extend(vars.into_iter().map(|(k, v)| format!("{k}={v}")));
    // The Python that library calls link against (an upgrade in place leaves PATH as it was).
    let py = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let found = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).map(|d| d.join(&py)).find(|f| f.is_file())).flatten();
    if let Some(m) = found.and_then(|f| std::fs::canonicalize(f).ok()).and_then(|f| std::fs::metadata(&f).ok().map(|m| (f, m))) {
        parts.push(format!("{} {} {:?}", m.0.display(), m.1.len(), m.1.modified().ok()));
    }
    let path = std::fs::canonicalize(file).ok()?;
    parts.push(path.to_string_lossy().into());
    for f in sources(&path)? {
        parts.push(f.to_string_lossy().into());
        parts.push(std::fs::read_to_string(&f).ok()?);
    }
    let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    Some(format!("{:016x}{:016x}", crate::fnv(&refs, 21), crate::fnv(&refs, 22)))
}

// The program's files: every .py and .bv file in its folder (sorted), and those of the folders its files
// reach (package folders they import, `use "lib/x.bv"` lines), followed from each file found. None when
// there are too many to read each time (then nothing is cached).
fn sources(path: &Path) -> Option<Vec<PathBuf>> {
    let root = path.parent()?.to_path_buf();
    let mut dirs: Vec<PathBuf> = vec![root.clone()];
    let mut out: Vec<PathBuf> = vec![];
    let mut k = 0;
    while k < dirs.len() {
        let d = dirs[k].clone();
        k += 1;
        let files = src_files(&d)?;
        for f in &files {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            let here = f.parent().unwrap_or(&root).to_path_buf();
            let mut reach: Vec<PathBuf> = vec![];
            for m in imports(&text) {
                for base in [&here, &root] {
                    let mut p = base.clone();
                    for part in m.split('.').filter(|s| !s.is_empty()) {
                        p.push(part);
                        if p.is_dir() {
                            reach.push(p.clone());
                        }
                    }
                }
            }
            for l in text.lines() {
                if let Some(u) = l.trim().strip_prefix("use \"").and_then(|r| r.split('"').next()) {
                    if let Some(d) = here.join(u).parent() {
                        reach.push(d.to_path_buf());
                    }
                }
            }
            for r in reach {
                let r = std::fs::canonicalize(&r).unwrap_or(r);
                if !dirs.contains(&r) {
                    dirs.push(r);
                }
            }
        }
        out.extend(files);
        if out.len() > 200 || dirs.len() > 20 {
            return None;
        }
    }
    Some(out)
}

fn src_files(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && matches!(p.extension().and_then(|e| e.to_str()), Some("py" | "bv")))
        .take(201)
        .collect();
    v.sort();
    Some(v)
}

// Module names in import lines (`import a.b, c`, `from a.b import x`), relative dots kept.
fn imports(text: &str) -> Vec<String> {
    let mut out = vec![];
    for l in text.lines() {
        let t = l.trim_start();
        if let Some(r) = t.strip_prefix("from ") {
            if let Some(m) = r.split_whitespace().next() {
                out.push(m.to_string());
            }
        } else if let Some(r) = t.strip_prefix("import ") {
            for x in r.split(',') {
                if let Some(m) = x.split_whitespace().next() {
                    out.push(m.to_string());
                }
            }
        }
    }
    out
}

// True when every module the program's files import is one of them or part of Python's standard library.
fn only_local_and_stdlib(file: &str) -> bool {
    let Ok(path) = std::fs::canonicalize(file) else { return false };
    let Some(files) = sources(&path) else { return false };
    let dir = path.parent().unwrap_or(Path::new("."));
    files.iter().all(|f| {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        imports(&text).iter().all(|m| {
            let top = m.trim_start_matches('.').split('.').next().unwrap_or("");
            m.starts_with('.') || top.is_empty() || STDLIB.split(' ').any(|s| s == top) || dir.join(format!("{top}.py")).exists() || dir.join(top).is_dir()
        })
    })
}

// sys.stdlib_module_names (CPython 3.13), without the private ones.
const STDLIB: &str = "abc antigravity argparse array ast asyncio atexit base64 bdb binascii bisect builtins bz2 cProfile calendar cmath cmd code codecs codeop collections colorsys compileall concurrent configparser contextlib contextvars copy copyreg csv ctypes curses dataclasses datetime dbm decimal difflib dis doctest email encodings ensurepip enum errno faulthandler fcntl filecmp fileinput fnmatch fractions ftplib functools gc genericpath getopt getpass gettext glob graphlib grp gzip hashlib heapq hmac html http idlelib imaplib importlib inspect io ipaddress itertools json keyword linecache locale logging lzma mailbox marshal math mimetypes mmap modulefinder msvcrt multiprocessing netrc nt ntpath nturl2path numbers opcode operator optparse os pathlib pdb pickle pickletools pkgutil platform plistlib poplib posix posixpath pprint profile pstats pty pwd py_compile pyclbr pydoc pydoc_data pyexpat queue quopri random re readline reprlib resource rlcompleter runpy sched secrets select selectors shelve shlex shutil signal site smtplib socket socketserver sqlite3 sre_compile sre_constants sre_parse ssl stat statistics string stringprep struct subprocess symtable sys sysconfig syslog tabnanny tarfile tempfile termios textwrap this threading time timeit tkinter token tokenize tomllib trace traceback tracemalloc tty turtle turtledemo types typing unicodedata unittest urllib uuid venv warnings wave weakref webbrowser winreg winsound wsgiref xml xmlrpc zipapp zipfile zipimport zlib zoneinfo";

// Keeps the result cache under BREVI_CACHE_MB megabytes (default 1024, shared with the binary cache):
// past it, the least recently used entries go until it is down to three quarters.
pub fn prune(dir: &Path) {
    let limit: u64 = std::env::var("BREVI_CACHE_MB").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(1024) * (1 << 20) / 2;
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let size = |p: &Path| -> u64 {
        if p.is_dir() {
            std::fs::read_dir(p).map(|r| r.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0)
        } else {
            std::fs::metadata(p).map_or(0, |m| m.len())
        }
    };
    let used = |p: &Path| -> std::time::SystemTime {
        let at = if p.is_dir() { p.join("code") } else { p.to_path_buf() };
        std::fs::metadata(at).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
    };
    let mut es: Vec<(std::time::SystemTime, u64, PathBuf)> = rd.flatten().map(|e| e.path()).filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".tmp-"))).map(|p| (used(&p), size(&p), p)).collect();
    let mut total: u64 = es.iter().map(|e| e.1).sum();
    if total <= limit {
        return;
    }
    es.sort();
    for (_, n, p) in es {
        if total <= limit / 4 * 3 {
            break;
        }
        if p.is_dir() {
            std::fs::remove_dir_all(&p).ok();
        } else {
            std::fs::remove_file(&p).ok();
        }
        total = total.saturating_sub(n);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn not_kept_line_is_taken_out() {
        let mut e = b"note: a\n{\"brevi_cache\":\"not kept\"}\nnote: b\n".to_vec();
        assert!(super::take_not_kept(&mut e));
        assert_eq!(e, b"note: a\nnote: b\n");
        let mut u = b"note: a\n{\"brevi_cache\":\"upgrade\"}\t/c/bin/k\n".to_vec();
        assert_eq!(super::take_upgrade(&mut u).as_deref(), Some("/c/bin/k"));
        assert_eq!(u, b"note: a\n");
        let mut plain = b"note: a\n".to_vec();
        assert!(!super::take_not_kept(&mut plain));
        assert_eq!(plain, b"note: a\n");
    }
}
