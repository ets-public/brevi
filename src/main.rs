mod alias;
mod bvpy;
mod records;
mod ast;
mod bigint;
mod caps;
mod cgroup;
mod diag;
mod dynpy;
mod emit;
mod engine;
mod ffi;
mod gui;
mod json;
mod lexer;
mod moves;
mod infer;
mod mcp;
mod mcp_http;
mod proc;
mod modules;
mod parser;
mod pybridge;
mod pydiag;
mod patch;
mod typed_guard;
mod unicode_names;
mod pysentinel;
mod pystd;
mod pygen;
mod scoped;
mod runcache;
mod tier;
mod shim;
mod verify;

use diag::{esc, Diag};
use std::path::{Path, PathBuf};
use std::io::Write;
use std::process::{exit, Command};

const USAGE: &str = "usage: brevi <run|build|test|check|fix|emit> FILE.py|FILE.bv [--release] [--fast] [--native] [--sandbox] [--allow read,write,net,native] [--allow-py MODULES] [--allow-net HOSTS] [--allow-read DIRS] [--allow-write DIRS] [--limit cpu=S,mem=MB,time=S,output=KB,file=MB,written=MB,net=MB,threads=N,fds=N,files=N] [--traceback] [--window] [--explain] [--standalone] [-o OUT] [-- ARGS...] | brevi run --verify FILE.py [--strict] [-- ARGS...] | brevi verify FILE.py [--input PATH]... [--once] [--timeout SECS] [--limit SPEC] [-- ARGS...] | brevi report [--clear] | brevi guide [SECTION|WORD] | brevi patch FILE PATCH | brevi mcp [--http [HOST:]PORT --token TOKEN]";

// The language guide, so `brevi guide` and `--explain` can serve parts of it.
pub const GUIDE: &str = include_str!("../BREVI.md");
// Kept out of the main guide, which every session reads: shown by `brevi guide Libraries` and word lookups.
pub const GUIDE_LIBS: &str = include_str!("../LIBRARIES.md");
// Details kept out of the main guide for the same reason: `brevi guide Json`, `brevi guide re_list`.
pub const GUIDE_REF: &str = include_str!("../REFERENCE.md");
static EXPLAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// `## Name` sections of the guide, with the intro under the name "Intro".
fn guide_sections() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![("Intro".into(), String::new())];
    let extra = |g: &'static str| g.lines().skip_while(|l| !l.starts_with("## "));
    for l in GUIDE.lines().chain(extra(GUIDE_REF)).chain(extra(GUIDE_LIBS)) {
        if let Some(h) = l.strip_prefix("## ") {
            out.push((h.trim().to_string(), String::new()));
        }
        let last = out.last_mut().unwrap();
        last.1.push_str(l);
        last.1.push('\n');
    }
    out
}

// A section by name (any case), else every guide line that mentions the word, under its section name.
pub fn guide_lookup(q: &str) -> String {
    let secs = guide_sections();
    if let Some((_, body)) = secs.iter().find(|(n, _)| n.eq_ignore_ascii_case(q)) {
        return body.clone();
    }
    let word = q.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
    if word.is_empty() {
        return String::new();
    }
    let hit = |l: &str| {
        let b = l.as_bytes();
        l.match_indices(word).any(|(i, _)| {
            let before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
            let j = i + word.len();
            let after = j >= b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'_');
            before && after
        })
    };
    // A reader of `brevi guide WORD` has read the core guide already, so when the reference or
    // libraries mention the word, only those lines are shown.
    let core = GUIDE.lines().filter(|l| l.starts_with("## ")).count() + 1;
    let in_extra = secs[core..].iter().any(|(_, body)| body.lines().any(|l| !l.starts_with('#') && hit(l)));
    let mut out = String::new();
    for (i, (name, body)) in secs.iter().enumerate() {
        if in_extra && i < core {
            continue;
        }
        let lines: Vec<&str> = body.lines().filter(|l| !l.starts_with('#') && hit(l)).collect();
        if !lines.is_empty() {
            out.push_str(&format!("[{name}]\n"));
            for l in lines {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}

// With --explain, the guide lines about the names an error mentions (`.char()`, `dec`, `json`).
fn explain(msg: &str, fix: &str) {
    if !EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let text = format!("{msg} {fix}");
    let mut words: Vec<String> = vec![];
    for w in text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')) {
        let w = w.trim_matches('.');
        if w.len() > 1 && !words.iter().any(|x| x == w) {
            words.push(w.to_string());
        }
    }
    const SKIP: &[&str] = &["is", "the", "a", "an", "of", "for", "to", "in", "on", "it", "or", "and", "with", "write", "did", "you", "mean", "here", "this", "use", "not", "be", "can", "found", "expected", "type", "value", "line", "so", "needs", "inside", "function", "used", "valid", "methods", "unknown", "method", "set", "x", "xs", "real", "has", "no", "named"];
    let mut shown = 0;
    for w in words.iter().filter(|w| !SKIP.contains(&w.as_str()) && !w.chars().all(|c| c.is_ascii_digit())) {
        let g = guide_lookup(w);
        if g.is_empty() || g.lines().count() > 8 {
            continue;
        }
        println!("{{\"guide\":\"{}\"}}", esc(g.trim_end()));
        shown += 1;
        if shown == 2 {
            break;
        }
    }
}

// The files of the program (see modules.rs), so error lines can name the file they are in.
static SEGS: std::sync::OnceLock<Vec<modules::Seg>> = std::sync::OnceLock::new();
fn at(json: &str) -> String {
    match SEGS.get() {
        Some(s) if s.len() > 1 => modules::relocate(s, json),
        _ => json.to_string(),
    }
}

fn print_diag(d: &Diag) {
    println!("{}", at(&d.to_json()));
    explain(&d.msg, d.fix.as_deref().unwrap_or(""));
}

/// The CPython to run when a library program falls back, or when verify compares: `python` on Windows
/// (there `python3` is often a Store alias), `python3` elsewhere.
pub fn python_exe() -> String {
    std::env::var("BREVI_PYTHON").unwrap_or_else(|_| if cfg!(windows) { "python".into() } else { "python3".into() })
}

fn main() {
    tm("start");
    if let Some(args) = shim::args() {
        shim::main(args);
    }
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(|a| a.as_str()) == Some("mcp") {
        mcp::serve(&argv[1..]);
        return;
    }
    if argv.first().map(|a| a.as_str()) == Some("__tier") {
        tier::job(&argv[1..]);
    }
    if matches!(argv.first().map(|a| a.as_str()), Some("--help" | "-h" | "help")) {
        println!("{USAGE}");
        return;
    }
    if matches!(argv.first().map(|a| a.as_str()), Some("--version" | "version")) {
        println!("brevi {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if matches!(argv.first().map(|a| a.as_str()), Some("build" | "run" | "test" | "check" | "fix" | "verify")) {
        // Find rustc and its version while the front end works.
        std::thread::spawn(|| {
            rustc_version();
        });
    }
    if argv.first().map(|a| a.as_str()) == Some("verify") {
        verify::main(&argv[1..]);
    }
    // `brevi run --verify`: the run checked against CPython (verify.rs).
    if argv.first().map(|a| a.as_str()) == Some("run") {
        let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
        if let Some(i) = argv[..dash].iter().position(|a| a == "--verify") {
            let mut rest = argv[1..].to_vec();
            rest.remove(i - 1);
            verify::run_main(&rest);
        }
    }
    if argv.first().map(|a| a.as_str()) == Some("report") {
        verify::report(&argv[1..]);
    }
    if argv.first().map(|a| a.as_str()) == Some("guide") {
        let q = argv[1..].join(" ");
        let g = if q.is_empty() { GUIDE.to_string() } else { guide_lookup(&q) };
        if g.is_empty() {
            let names: Vec<String> = guide_sections().into_iter().map(|(n, _)| n).collect();
            println!("{{\"msg\":\"nothing in the guide for {}\",\"fix\":\"sections: {}\"}}", esc(&q), esc(&names.join(", ")));
            exit(1);
        }
        print!("{g}");
        return;
    }
    if argv.first().map(|a| a.as_str()) == Some("patch") && argv.len() == 3 {
        let read = |f: &str| std::fs::read_to_string(f).unwrap_or_else(|e| {
            println!("{{\"e\":\"io\",\"at\":\"{}\",\"msg\":\"{}\"}}", esc(f), esc(&e.to_string()));
            exit(1);
        });
        match patch::patch(&read(&argv[1]), &read(&argv[2])) {
            Ok((out, done)) => {
                std::fs::write(&argv[1], out).unwrap_or_else(|e| {
                    println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                    exit(1);
                });
                println!("{{\"patched\":\"{}\"}}", esc(&done.join(", ")));
            }
            Err(m) => {
                println!("{{\"msg\":\"{}\",\"fix\":\"a patch holds whole fn, type, enum or test items\"}}", esc(&m));
                exit(1);
            }
        }
        return;
    }
    if let Some(i) = argv.iter().position(|a| a == "--explain") {
        argv.remove(i);
        EXPLAIN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let fast = argv.iter().position(|a| a == "--fast").map(|i| argv.remove(i)).is_some();
    let dash0 = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    if argv[..dash0].iter().position(|a| a == "--no-cache").map(|i| argv.remove(i)).is_some() {
        runcache::turn_off();
    }
    let mut sandbox = argv.iter().position(|a| a == "--sandbox").map(|i| argv.remove(i)).is_some();
    // --allow read,write,net,native, --allow-net HOSTS and --allow-py MODULES: what the caller grants a sandboxed program (caps.rs).
    // Either implies --sandbox.
    let mut grant: Vec<String> = vec![];
    loop {
        let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
        let Some(i) = argv[..dash].iter().position(|a| matches!(a.split('=').next().unwrap_or(""), "--allow" | "--allow-py" | "--allow-net" | "--allow-read" | "--allow-write")) else { break };
        let flag = argv.remove(i);
        let (name, value) = match flag.split_once('=') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None if i < dash - 1 => (flag.clone(), argv.remove(i)),
            None => {
                eprintln!("{flag} needs a value: --allow read,write, --allow-py requests or --allow-net api.example.com");
                exit(2);
            }
        };
        let parsed = match name.as_str() {
            "--allow-net" => caps::parse_net(&value),
            "--allow-read" => caps::parse_paths("read", &value),
            "--allow-write" => caps::parse_paths("write", &value),
            _ => caps::parse_grant(name == "--allow-py", &value),
        };
        match parsed {
            Ok(g) => grant.extend(g),
            Err(m) => {
                eprintln!("{m}");
                exit(2);
            }
        }
        sandbox = true;
    }
    // --limit cpu=2,mem=256,time=5,output=64,file=10: what the program may use while it runs (proc.rs).
    let limits = proc::take_limit_flags(&mut argv).unwrap_or_else(|m| {
        eprintln!("{m}");
        exit(2);
    });
    if !limits.is_empty() {
        if !matches!(argv.first().map(String::as_str), Some("run" | "test")) {
            eprintln!("--limit holds a program while it runs: use it with brevi run or brevi test (brevi verify takes it too)");
            exit(2);
        }
        // files, net and written are counted by Brevi's own file and network layers, which hold only a
        // sandboxed program that runs no CPython or native code.
        if (limits.files.is_some() || limits.net.is_some() || limits.written.is_some()) && (!sandbox || grant.iter().any(|g| g == "native" || g.starts_with("py:"))) {
            eprintln!("--limit files, net and written are counted by Brevi's own file and network code, so they need --sandbox without --allow native or --allow-py (CPython and native code are not counted); leave them out, or drop those grants");
            exit(2);
        }
        proc::LIMITS.set(limits).ok();
    }
    grant.sort();
    grant.dedup();
    caps::GRANT.set(grant).ok();
    if sandbox {
        caps::SANDBOX.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let traceback = argv.iter().position(|a| a == "--").map_or(argv.len(), |d| d);
    if argv[..traceback].iter().position(|a| a == "--traceback").map(|i| argv.remove(i)).is_some() || std::env::var("BREVI_TRACEBACK").is_ok_and(|v| !v.is_empty() && v != "0") {
        TRACEBACK.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let standalone = argv.iter().position(|a| a == "--standalone").map(|i| argv.remove(i)).is_some();
    if argv.iter().position(|a| a == "--window").map(|i| argv.remove(i)).is_some() {
        gui::WINDOW.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let native = argv[..dash].iter().position(|a| a == "--native").map(|i| argv.remove(i)).is_some()
        || std::env::var("BREVI_NATIVE").map_or(false, |v| !v.is_empty() && v != "0");
    let (cmd, file) = match (argv.first(), argv.get(1)) {
        (Some(c), Some(f)) => (c.as_str(), f.clone()),
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    };
    // run and test are the edit loop: a new build may be the quick tier (src/tier.rs).
    if matches!(cmd, "run" | "test") && !argv.iter().take_while(|a| a.as_str() != "--").any(|a| a == "--release") {
        tier::edit_loop();
    }
    // A .bv file written in plain Python (no Brevi spellings) takes the .py path: the guard, the typed build and the
    // dynamic build, but never python3 (it is a Brevi file).
    let bv_py = matches!(cmd, "build" | "run" | "check" | "emit") && bv_as_py(&file);
    let native = native || bv_py;
    // `brevi run x.py` falls back to CPython when Brevi cannot build the file; --native (or BREVI_NATIVE=1) keeps it strict.
    if is_py(&file) {
        PY_FILE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // An unchanged program is not built again: check, build and strict runs replay the build from the cache.
    if matches!(cmd, "check" | "build" | "run") && !(cmd == "run" && is_py(&file) && !native) && !standalone && runcache::on() && !runcache::in_child() {
        let mut flags: Vec<String> = vec![];
        for (on, f) in [(fast, "--fast"), (sandbox, "--sandbox"), (gui::window(), "--window"), (EXPLAIN.load(std::sync::atomic::Ordering::Relaxed), "--explain"), (native, "--native")] {
            if on {
                flags.push(f.to_string());
            }
        }
        flags.extend(caps::grant_flags());
        cached(cmd, &file, &argv, &flags);
    }
    // Capabilities first: a refusal here is final, before any dynamic, pybridge or python3 path can run.
    let held = matches!(cmd, "build" | "run" | "check" | "emit" | "test" | "fix") && hold_to_caps(&file, sandbox);
    if is_py(&file) && matches!(cmd, "build" | "run" | "emit") && std::env::var("BREVI_DYN").map_or(false, |v| v == "1") {
        dyn_cmd(cmd, &file, &argv);
    }
    if cmd == "run" && is_py(&file) && !native {
        run_or_fall_back(&file, &argv, sandbox, held);
    }
    // Brevi's own syntax in a .py file: CPython stops on it, so Brevi does too.
    if is_py(&file) && matches!(cmd, "build" | "run" | "check") {
        refuse_not_python(&file);
    }
    // Python the typed build would answer differently (a set printed or walked in CPython's hash order):
    // built over dynamic values, or refused, never built typed.
    let mut unread = None;
    if is_py(&file) && matches!(cmd, "build" | "run" | "check" | "emit") && std::env::var("BREVI_DYN").map_or(true, |v| v != "0") {
        let guard;
        (guard, unread) = guard_files(&file);
        if std::env::var("BREVI_GUARD").is_ok_and(|v| v == "print") {
            let typed = unread.as_ref().map_or("typed".to_string(), |(l, w, _)| format!("unread {l}: {w}"));
            println!("{}", guard.as_ref().map_or(typed, |(l, w, _, k)| if k.is_empty() { format!("dyn {l}: {w}") } else { format!("typed if {}: {l}: {w}", k.join(" ")) }));
            exit(0);
        }
        // Set walks on names that may hold strings: the typed build tries, with rustc checking each name.
        let guard = match guard {
            Some((line, why, gfile, names)) if !names.is_empty() => {
                emit::RAND_ORDER.set(names).ok();
                *SOFT_GUARD.lock().unwrap() = Some((line, why, gfile));
                None
            }
            g => g.map(|(l, w, f, _)| (l, w, f)),
        };
        if let Some((line, why, gfile)) = guard {
            guard_refusal(cmd, &file, &argv, line, &why, &gfile);
        }
    }
    let release = argv.iter().any(|a| a == "--release");
    let out_path = argv.iter().position(|a| a == "-o").and_then(|i| argv.get(i + 1)).map(PathBuf::from);
    let prog_args: Vec<String> = argv.iter().position(|a| a == "--").map(|i| argv[i + 1..].to_vec()).unwrap_or_default();

    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            println!("{{\"e\":\"io\",\"at\":\"{}\",\"msg\":\"{}\"}}", esc(&file), esc(&e.to_string()));
            exit(1);
        }
    };
    // Errors with a sure one-word fix (a typo of a known name) are fixed before building; `brevi fix` writes
    // every suggested fix back to the file.
    let src = modules::fill_file_name(&src, Path::new(&file));
    let src = if is_py(&file) { pystd::rewrite(&pysentinel::rewrite(&pygen::rewrite(&src))) } else { src };
    let (src, segs) = match modules::expand(&file, &src) {
        Ok(x) => x,
        Err(e) => {
            println!("{e}");
            exit(1);
        }
    };
    SEGS.set(segs).ok();
    pybridge::set_file(&file);
    // `allow read, write` lines, and with --sandbox (or any allow line) a refusal of anything undeclared.
    let with_allow = src.clone();
    let (src, allowed, cap_errs) = caps::split(&src);
    let cap_errs = if sandbox || allowed.is_some() {
        let (allowed, held_errs) = caps::held(&with_allow, allowed, sandbox);
        [cap_errs, held_errs, caps::check_src(&src, &allowed)].concat()
    } else {
        cap_errs
    };
    if !cap_errs.is_empty() {
        cap_errs.iter().for_each(print_diag);
        exit(1);
    }
    let foreign = ffi::split(&src).1;
    let mut deps = ffi::all_deps(&foreign);
    // A tkinter program built with --window draws through eframe.
    if gui::window() && src.lines().any(|l| (l.starts_with("import tkinter") || l.starts_with("from tkinter")) && !deps.iter().any(|d| d.name == "eframe")) {
        deps.push(gui::window_dep());
    }
    DEPS.set(deps).ok();
    RUST_LINES.set(foreign.rust_lines.clone()).ok();
    // A Python file is never "fixed": CPython would not rename `s.rpartition` to `s.partition`, so a name
    // the typed build lacks sends the file to the dynamic build instead.
    let (src, fixed) = if PY_FILE.load(std::sync::atomic::Ordering::Relaxed) && cmd != "fix" { (src, vec![]) } else { autofix(&src, cmd == "fix") };
    for (line, old, new) in &fixed {
        eprintln!("{}", at(&format!("{{\"line\":{line},\"fixed\":\"{} -> {}\"}}", esc(old), esc(new))));
    }
    if cmd == "fix" {
        if !fixed.is_empty() {
            let segs = SEGS.get().cloned().unwrap_or_default();
            let src = caps::restore(&with_allow, &src);
            let parts = if segs.len() > 1 { modules::split_back(&segs, &src) } else { vec![(PathBuf::from(&file), src.clone())] };
            for (path, text) in parts {
                if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
                    continue;
                }
                if let Err(e) = std::fs::write(&path, &text) {
                    println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                    exit(1);
                }
            }
        }
    }
    // A Python file the typed path cannot build is built over dynamic values instead (still native, no CPython).
    let dyn_ok = matches!(cmd, "build" | "run" | "check") && (is_py(&file) || dynpy::has_source(&file));
    // The set-order guard let the file through on names rustc must prove hold strings: any failure from here
    // on is reported as the guard's reason, and goes where the guard would have sent it.
    let soft_fail = |cmd: &str| {
        if let Some((line, why, gfile)) = SOFT_GUARD.lock().unwrap().clone() {
            guard_refusal(cmd, &file, &argv, line, &why, &gfile);
        }
    };
    let rust = match compile_to_rust(&src, true, true) {
        Ok(r) => {
            // The typed build takes Brevi's names (`fail("x")`, `s.trim()`), which are mistakes in a .py file.
            if is_py(&file) && matches!(cmd, "build" | "run" | "check") {
                refuse_sure(&file, pydiag::scan_brevi);
            }
            // The typed build took a file the guard could not read, so nothing says it matches CPython. (A file
            // it refuses goes on below, keeping its own errors.)
            if let Some((line, why, gfile)) = &unread {
                // By now errors are moved from the joined source to their file: give the joined line.
                let seg = SEGS.get().and_then(|s| s.iter().skip(1).find(|g| g.path.file_name() == gfile.file_name()));
                guard_refusal(cmd, &file, &argv, seg.map_or(*line, |g| g.start + line - 1), why, gfile);
            }
            if emit::RAND_ORDER.get().is_some_and(|ns| ns.iter().any(|k| !r.contains(&format!("/*axr:{k}*/")))) {
                soft_fail(cmd);
            }
            r
        }
        Err(diags) => {
            soft_fail(cmd);
            // A mistake CPython would also stop on (a name defined nowhere, `x.uppr()`, `"n=" + len(xs)`) is
            // reported, not handed to the dynamic build, where it would only show when the line ran.
            let dir = Path::new(&file).parent().unwrap_or(Path::new("."));
            let sure = if is_py(&file) {
                Some(pydiag::definite(&diags, &src, dir)).filter(|s| !s.is_empty()).unwrap_or_else(|| pydiag::scan(&src, dir))
            } else {
                vec![]
            };
            if !sure.is_empty() {
                sure.iter().for_each(print_diag);
                exit_sure();
            }
            let (line, func, why, extra) = typed_refusal(&diags, &src);
            if dyn_ok && dyn_fallback(cmd, &file, &argv, (String::new(), line, func, why, extra, true)) {
                exit(0);
            }
            dyn_blocked_note();
            // --sandbox without CPython: Brevi's own library (numpy, polars, ...) lacks what the program uses, so
            // say what, in place of the typed build's "a call CPython runs needs allow native".
            let diags = own_lib_refusal(diags);
            for d in diags {
                print_diag(&d);
            }
            exit(1);
        }
    };
    // Typed only with CPython running some of Brevi's own modules' functions (math.gcd, itertools.product):
    // the dynamic build has them natively, so it goes first (no libpython at run time, fine under --sandbox).
    if dyn_ok && !forced_dyn() && pybridge::active() && (pybridge::stdlib_only() || pybridge::numpy_only() || pybridge::own_package_only(Path::new(&file).parent().unwrap_or(Path::new(".")))) {
        let why = format!("{} not in the typed build", pybridge::uses().join(", "));
        // Without CPython: one that needs it anyway is better built typed.
        dynpy::NO_BRIDGE.store(true, std::sync::atomic::Ordering::Relaxed);
        if dyn_fallback(cmd, &file, &argv, (String::new(), 0, None, why, String::new(), false)) {
            exit(0);
        }
        dynpy::NO_BRIDGE.store(false, std::sync::atomic::Ordering::Relaxed);
        *DYN_BLOCKED.lock().unwrap() = None;
    }
    if dyn_ok {
        DEFER_REPORT.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let mut versions = Versions { src: &src, first: rust.clone() };
    let fail = |cmd: &str| -> ! {
        soft_fail(cmd);
        let why = DEFERRED.lock().unwrap().as_ref().map_or_else(|| "the typed build's Rust did not compile".to_string(), |(_, e)| rustc_first_error(e));
        if dyn_ok && dyn_fallback(cmd, &file, &argv, (String::new(), 0, None, why, String::new(), false)) {
            exit(0);
        }
        dyn_blocked_note();
        if let Some((rust, stderr)) = DEFERRED.lock().unwrap().take() {
            report(&rust, &stderr);
        }
        exit(1);
    };

    match cmd {
        "emit" => print!("{rust}"),
        "check" | "fix" if fast => {
            if cmd == "check" {
                say_typed();
            }
        }
        "check" | "fix" => {
            // Front-end checks passed; also run rustc's checks without producing a binary.
            let dir = build_dir(&file);
            if !rustc(&mut versions, &dir, &dir.join("check"), &["--emit=metadata"], false) {
                fail(cmd);
            }
            if cmd == "check" {
                say_typed();
            }
        }
        "build" | "run" | "test" => {
            let dir = build_dir(&file);
            let stem = Path::new(&file).file_stem().unwrap().to_string_lossy().to_string();
            let bin = if cmd == "build" { out_path.unwrap_or_else(|| PathBuf::from(&stem)) } else { dir.join(&stem) };
            let mut extra: Vec<&str> = Vec::new();
            if cmd == "test" {
                extra.push("--test");
            }
            // Without --release: opt-level 1 against the runtime library built at opt-level 3. On the
            // comparison programs this runs 3x to 20x faster than an unoptimised build, within about 2x of
            // --release, for about 0.3 s more build time (--release takes about 1.5 s more).
            extra.extend(["-C", if release { "opt-level=3" } else { "opt-level=1" }]);
            if !rustc(&mut versions, &dir, &bin, &extra, true) {
                fail(cmd);
            }
            if cmd == "build" {
                // --standalone: a program that calls Python libraries carries its own CPython.
                if standalone && pybridge::active() {
                    match pybridge::bundle(&file, &bin) {
                        Ok(note) => eprintln!("note: {note}"),
                        Err(m) => {
                            print_diag(&Diag::new("build", 1, 1, &m).fix_hint("build without --standalone; the program then needs python3 and its libraries installed"));
                            exit(1);
                        }
                    }
                }
                say_typed();
                return;
            }
            if cmd == "run" {
                say_typed();
            }
            let mut c = proc::program(&bin);
            if cmd == "test" {
                c.args(["--quiet", "--test-threads=1"]);
            } else {
                c.args(&prog_args);
            }
            let code = proc::run(c).unwrap_or_else(|e| {
                println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                exit(1);
            });
            exit(code);
        }
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    }
}

// --sandbox, or an allow line in the program: refuses (prints the errors and exits) any use of a capability it
// did not declare. Without `allow native`, also turns pybridge off for this run. Runs before every other path,
// so a refusal is final. True when the program is held to its allow lines.
fn hold_to_caps(file: &str, sandbox: bool) -> bool {
    let Ok(src) = std::fs::read_to_string(file) else { return sandbox };
    let src = modules::fill_file_name(&src, Path::new(file));
    // A program whose `use` lines do not resolve is reported by the build; check what is there.
    let src = modules::expand(file, &src).map(|x| x.0).unwrap_or(src);
    let (code, allowed, mut errs) = caps::split(&src);
    let held = sandbox || allowed.is_some();
    if held {
        // Under --sandbox, only what the caller granted (--allow, --allow-py): the file cannot grant itself more.
        let (allowed, held_errs) = caps::held(&src, allowed, sandbox);
        errs.extend(held_errs);
        if !allowed.iter().any(|a| a == "native" || a.starts_with("py:")) {
            pybridge::NO_NATIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        errs.extend(caps::check_src(&code, &allowed));
    }
    if !errs.is_empty() {
        errs.iter().for_each(print_diag);
        exit(1);
    }
    held
}

// The dynamic build for a Python file the typed path could not build: false (nothing printed) if it cannot
// be built that way either. For run, runs the program and exits with its status.
// `why` is the path report's (file when not the main one, line, fn, why, more JSON fields, whether line is in
// the joined source).
type Why = (String, usize, Option<String>, String, String, bool);

fn dyn_fallback(cmd: &str, file: &str, argv: &[String], why: Why) -> bool {
    if std::env::var("BREVI_DYN").map_or(false, |v| v == "0") {
        return false;
    }
    let rust = match dynpy::to_rust(file) {
        Ok(r) => r,
        Err((f, line, msg)) => {
            *DYN_BLOCKED.lock().unwrap() = Some((f, line, msg));
            return false;
        }
    };
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let release = argv[..dash].iter().any(|a| a == "--release");
    let dir = build_dir(file);
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy().to_string();
    let out_path = argv[..dash].iter().position(|a| a == "-o").and_then(|i| argv.get(i + 1)).map(PathBuf::from);
    let bin = if cmd == "build" { out_path.unwrap_or_else(|| PathBuf::from(&stem)) } else { dir.join(format!("{stem}-dyn")) };
    let built = if cmd == "check" { dynpy::check(&rust, &dir, &cache_dir(), rustc_version()) } else { dynpy::compile(&rust, &dir, &bin, release, &cache_dir(), rustc_version()) };
    if let Err(e) = built {
        if std::env::var("BREVI_DYN_DEBUG").is_ok() {
            eprintln!("{e}");
        }
        // Not a fact about the program: the runtime library may not have built (a cold cache shared by many
        // builds at once, a full disk) or rustc may have been stopped. The refusal printed next names the typed
        // build's reason, which a later build (after the machine is fine) would not stop at.
        runcache::not_kept();
        *DYN_BLOCKED.lock().unwrap() = Some((file.to_string(), 0, "internal: the dynamic build's code did not compile".into()));
        return false;
    }
    let (wfile, line, func, why, extra, relocate) = why;
    say_path("dynamic", &wfile, line, func, &why, &format!("{extra}{}", dyn_py_field()), relocate);
    if cmd == "run" {
        let mut c = proc::script(&bin, file);
        c.args(argv.get(dash + 1..).unwrap_or(&[]));
        let code = proc::run(c).unwrap_or_else(|e| {
            println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
            exit(1);
        });
        exit(code);
    }
    true
}

// A file typed_guard sends off the typed path (why, at line of gfile): built over dynamic values, or refused.
fn guard_refusal(cmd: &str, file: &str, argv: &[String], line: usize, why: &str, gfile: &Path) -> ! {
    let file = file.to_string();
    let why = why.to_string();
        // The same mistakes the typed build would report (see below), before the dynamic build takes the file.
        if cmd != "emit" {
            refuse_sure(&file, pydiag::scan);
        }
        if cmd == "emit" {
            dyn_cmd(cmd, &file, &argv);
        }
        let gsrc = std::fs::read_to_string(&gfile).unwrap_or_default();
        let gname = gfile.file_name().unwrap_or_default().to_string_lossy().to_string();
        let main_file = Path::new(&file).file_name().map(|n| n.to_string_lossy().to_string()) == Some(gname.clone());
        let gshown = if main_file { String::new() } else { gname.clone() };
        let gwhy = format!("{}, so the typed build would differ from CPython", why.split_once(": ").filter(|_| !main_file).map_or(why.as_str(), |(_, w)| w));
        if dyn_fallback(cmd, &file, argv, (gshown, line, enclosing_def(&gsrc, line), gwhy, String::new(), false)) {
            exit(0);
        }
        dyn_blocked_note();
        // Name what stopped the dynamic build too: fixing only the typed reason would still leave the file unbuilt.
        let blocked = match DYN_BLOCKED.lock().unwrap().as_ref() {
            Some((f, l, m)) if *l > 0 => format!(" ({} line {l}: {m})", Path::new(f).file_name().unwrap_or_default().to_string_lossy()),
            Some((_, _, m)) => format!(" ({m})"),
            None => String::new(),
        };
        let mut d = Diag::new("check", line, 1, &format!("{why}, and this file cannot be built over dynamic values{blocked}"));
        if why.contains(" set") {
            d.fix = Some("sort it first (sorted(s)) where the order shows".into());
        } else if why.contains("generator") {
            d.fix = Some(if blocked.is_empty() || blocked.contains("yield inside try") { "move the yield out of the try/finally or with block, or build a list and return it" } else { "build a list and return it" }.into());
        }
        print_diag(&d);
        exit(1);
}

// Stops on the mistakes in a Python file that `scan` (from pydiag) is sure of.
fn refuse_sure(file: &str, scan: fn(&str, &Path) -> Vec<Diag>) {
    let sure = std::fs::read_to_string(file).map(|t| scan(&t, Path::new(file).parent().unwrap_or(Path::new(".")))).unwrap_or_default();
    if !sure.is_empty() {
        sure.iter().for_each(print_diag);
        exit_sure();
    }
}

// Stops on the first file of a Python program, the main one or one it imports, that CPython could not compile.
fn refuse_not_python(file: &str) {
    let Ok(src) = std::fs::read_to_string(file) else { return };
    let segs = modules::expand(file, &src).map_or(vec![], |x| x.1);
    for s in segs.iter().skip(1).filter(|s| s.path.extension().is_some_and(|x| x == "py")) {
        if let Some(mut d) = std::fs::read_to_string(&s.path).ok().and_then(|t| pydiag::not_python(&t)) {
            d.line += s.start - 1;
            println!("{}", modules::relocate(&segs, &d.to_json()));
            exit_sure();
        }
    }
    if let Some(d) = pydiag::not_python(&src) {
        print_diag(&d);
        exit_sure();
    }
}

fn exit_sure() -> ! {
    // A test hook: tools that scan many files tell these refusals apart by exit code 3.
    exit(if std::env::var("BREVI_SURE_EXIT").is_ok_and(|v| v == "3") { 3 } else { 1 })
}

// The set-order guard reason the typed build was let past (see guard_files), for when it fails after all.
static SOFT_GUARD: std::sync::Mutex<Option<(usize, String, PathBuf)>> = std::sync::Mutex::new(None);

// Under --sandbox without a CPython grant: when the typed build refused only because CPython would run the
// program's libraries and the dynamic build (Brevi's own copies of them) refused too, one error naming what the
// copy lacks and the grant that runs CPython's instead.
fn own_lib_refusal(diags: Vec<Diag>) -> Vec<Diag> {
    let generic = |d: &Diag| d.msg == "a call CPython runs needs allow native" || (d.line == 1 && d.msg.ends_with(" not in the typed build"));
    if !caps::sandboxed() || !pybridge::no_native() || !diags.iter().any(generic) {
        return diags;
    }
    let Some((f, line, msg)) = DYN_BLOCKED.lock().unwrap().clone() else { return diags };
    let mut mods: Vec<String> = pybridge::uses().iter().map(|u| u.split('.').next().unwrap_or(u).to_string()).collect();
    mods.sort();
    mods.dedup();
    if mods.is_empty() {
        return diags;
    }
    let shown = Path::new(&f).file_name().unwrap_or_default().to_string_lossy().to_string();
    let main = SEGS.get().and_then(|s| s.first()).and_then(|g| g.path.file_name()).map_or(true, |n| n.to_string_lossy() == shown);
    let at = if main { String::new() } else { format!(" ({shown})") };
    let text = if msg.contains("--sandbox") { format!("{msg}{at}") } else { format!("{msg}{at}; under --sandbox CPython's {} needs a grant", mods.join(", ")) };
    let d = Diag::new("caps", line.max(1), 1, &text).fix_hint(&caps::grant_hint("native", Some(&mods.join(", "))));
    let mut out = vec![d];
    out.extend(diags.into_iter().filter(|d| !generic(d)));
    out
}

// Why the dynamic build refused the file (file, line, msg), for the path report of a CPython run.
static DYN_BLOCKED: std::sync::Mutex<Option<(String, usize, String)>> = std::sync::Mutex::new(None);

// For `brevi run`'s strict child build (BREVI_DYN_WHY=1): why the dynamic build refused the file, as
// {"dyn_blocked":...} on stderr; the parent puts it in its {"path":"cpython"} line.
fn dyn_blocked_note() {
    if !std::env::var("BREVI_DYN_WHY").is_ok_and(|v| v == "1") {
        return;
    }
    if let Some((f, line, msg)) = DYN_BLOCKED.lock().unwrap().as_ref() {
        let name = Path::new(f).file_name().unwrap_or_default().to_string_lossy();
        eprintln!("{{\"dyn_blocked\":{{\"file\":\"{}\",\"line\":{line},\"msg\":\"{}\"}}}}", esc(&name), esc(msg));
    }
}

// Builds (and runs) a Python file through the dynamic-value path. Never returns.
fn dyn_cmd(cmd: &str, file: &str, argv: &[String]) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let release = argv[..dash].iter().any(|a| a == "--release");
    let rust = match dynpy::to_rust(file) {
        Ok(r) => r,
        Err((f, line, msg)) => {
            println!("{{\"e\":\"dyn\",\"at\":\"{}\",\"line\":{line},\"msg\":\"{}\"}}", esc(&f), esc(&msg));
            exit(1);
        }
    };
    if cmd == "emit" {
        print!("{rust}");
        exit(0);
    }
    let dir = build_dir(file);
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy().to_string();
    let out_path = argv[..dash].iter().position(|a| a == "-o").and_then(|i| argv.get(i + 1)).map(PathBuf::from);
    let bin = if cmd == "build" { out_path.unwrap_or_else(|| PathBuf::from(&stem)) } else { dir.join(format!("{stem}-dyn")) };
    if let Err(e) = dynpy::compile(&rust, &dir, &bin, release, &cache_dir(), rustc_version()) {
        println!("{{\"e\":\"dyn\",\"msg\":\"internal: generated code did not compile\"}}");
        eprintln!("{e}");
        exit(1);
    }
    say_path("dynamic", "", 0, None, "BREVI_DYN=1", &dyn_py_field(), false);
    if cmd == "build" {
        exit(0);
    }
    let mut c = proc::script(&bin, file);
    c.args(argv.get(dash + 1..).unwrap_or(&[]));
    let code = proc::run(c).unwrap_or_else(|e| {
        println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
        exit(1);
    });
    exit(code);
}

// check, build and run through the result cache (src/runcache.rs): the build runs as a child `brevi check` or
// `brevi build`, and what it printed (and the binary it made) is replayed while the files stay the same.
fn cached(cmd: &str, file: &str, argv: &[String], flags: &[String]) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let mut opts: Vec<String> = argv[2..dash].to_vec();
    let out = opts.iter().position(|a| a == "-o").and_then(|i| {
        let o = opts.get(i + 1).cloned();
        opts.drain(i..(i + 2).min(opts.len()));
        o
    });
    let stem = Path::new(file).file_stem().unwrap_or_default().to_string_lossy().to_string();
    let bin = match cmd {
        "check" => None,
        "build" => Some(PathBuf::from(out.unwrap_or_else(|| stem.clone()))),
        _ => Some(build_dir(file).join(&stem)),
    };
    let mut args: Vec<String> = vec![if cmd == "check" { "check" } else { "build" }.into(), file.into()];
    if let Some(b) = &bin {
        args.extend(["-o".to_string(), b.to_string_lossy().to_string()]);
    }
    args.extend(opts);
    args.extend(flags.iter().cloned());
    // A refusal of `build -o OUT` may be about OUT (a missing folder), which the key leaves out: not kept.
    // A run is the edit loop: its build may be the quick tier (src/tier.rs). A build is never.
    let envs: &[(&str, &str)] = if cmd == "run" && !args.iter().any(|a| a == "--release") { &[tier::ENV] } else { &[] };
    let done = runcache::phase(&args, file, bin.as_deref(), envs, cmd != "build");
    if done.hit {
        tm("replayed from the result cache");
    }
    std::io::stdout().write_all(&done.stdout).ok();
    std::io::stdout().flush().ok();
    std::io::stderr().write_all(&done.stderr).ok();
    if cmd != "run" || !done.ok() {
        exit(done.code.unwrap_or(1));
    }
    let mut c = proc::script(bin.as_ref().unwrap(), file);
    c.args(argv.get(dash + 1..).unwrap_or(&[]));
    let code = proc::run(c).unwrap_or_else(|e| {
        println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
        exit(1);
    });
    exit(code);
}

// `brevi run x.py`: builds with Brevi and runs the program; when the build fails, runs the file with python3
// instead and says so in one line on stderr. Never returns.
// A held program (--sandbox or an allow line) never runs with python3, which cannot hold it to its allow lines.
fn run_or_fall_back(file: &str, argv: &[String], sandbox: bool, held: bool) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let prog_args = argv.get(dash + 1..).unwrap_or(&[]).to_vec();
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy().to_string();
    let bin = build_dir(file).join(&stem);
    // main removed --sandbox, --window and --explain from argv when it read them: pass them on again (the child
    // build once ran without --sandbox, unchecked).
    let mut flags: Vec<String> = argv[2..dash].iter().filter(|a| a.as_str() == "--release").cloned().collect();
    if sandbox {
        flags.push("--sandbox".into());
        flags.extend(caps::grant_flags());
    }
    if gui::window() {
        flags.push("--window".into());
    }
    if EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
        flags.push("--explain".into());
    }
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    // Exit code 3 from the build: a sure mistake (pydiag), which CPython stops on too.
    let mut args: Vec<String> = vec!["build".into(), file.into(), "-o".into(), bin.to_string_lossy().to_string()];
    args.extend(flags);
    let mut envs = vec![("BREVI_NATIVE", "1"), ("BREVI_SURE_EXIT", "3"), ("BREVI_DYN_WHY", "1")];
    if !argv[2..dash].iter().any(|a| a == "--release") {
        envs.push(tier::ENV);
    }
    // Through the result cache: an unchanged file is not built again, and one Brevi could not build is not
    // tried again before it runs with python3.
    let built: std::io::Result<runcache::Done> = if runcache::on() {
        Ok(runcache::phase(&args, file, Some(&bin), &envs, true))
    } else {
        let mut c = Command::new(&me);
        c.args(&args).envs(envs).env("BREVI_NO_CACHE", "1");
        c.output().map(|o| runcache::Done { stdout: o.stdout, stderr: o.stderr, code: o.status.code(), hit: false })
    };
    if built.as_ref().is_ok_and(|d| d.hit) {
        tm("replayed from the result cache");
    }
    let code = match built {
        Ok(o) if o.ok() => {
            std::io::stderr().write_all(&o.stderr).ok();
            let mut c = proc::script(&bin, file);
            c.args(&prog_args);
            proc::run(c).unwrap_or_else(|e| {
                println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                1
            })
        }
        Ok(o) if held => {
            print!("{}", String::from_utf8_lossy(&o.stdout));
            let err = String::from_utf8_lossy(&o.stderr);
            err.lines().filter(|l| !l.starts_with("{\"dyn_blocked\":")).for_each(|l| eprintln!("{l}"));
            eprintln!("note: not run with python3: with --sandbox or an allow line, Brevi never falls back to CPython, which cannot enforce allow lines");
            1
        }
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let python = python_exe();
            // On a sure mistake the note already names the error, so CPython's traceback is cut to its last
            // line (`--traceback` or BREVI_TRACEBACK=1 shows all of it).
            let short = o.code == Some(3) && !TRACEBACK.load(std::sync::atomic::Ordering::Relaxed);
            let mut cmd = proc::program(&python);
            cmd.arg(file).args(&prog_args);
            // Under --limit the program runs through proc::run, which shows the whole traceback.
            if !proc::limits().is_empty() {
                say_cpython(file, &python, &out, &String::from_utf8_lossy(&o.stderr));
                exit(proc::run(cmd).unwrap_or(1));
            }
            if short {
                cmd.stderr(std::process::Stdio::piped());
            }
            match cmd.spawn() {
                Ok(mut child) => {
                    say_cpython(file, &python, &out, &String::from_utf8_lossy(&o.stderr));
                    if let Some(err) = child.stderr.take() {
                        last_traceback_line(err);
                    }
                    child.wait().map(proc::code).unwrap_or(1)
                }
                Err(_) => {
                    // No Python here: show Brevi's errors as a strict run would.
                    print!("{out}");
                    std::io::stderr().write_all(&o.stderr).ok();
                    1
                }
            }
        }
        Err(e) => {
            println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
            1
        }
    };
    exit(code)
}

// Passes CPython's stderr on as it comes, except a traceback: of that only the error line is shown, with the
// file and line it was raised at, such as `NameError: name 'x' is not defined (main.py line 3)`.
fn last_traceback_line(err: std::process::ChildStderr) {
    use std::io::BufRead;
    let mut r = std::io::BufReader::new(err);
    let mut held: Option<Vec<String>> = None;
    let mut buf = Vec::new();
    let mut stderr = std::io::stderr();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&buf).to_string();
        if let Some(h) = held.as_mut() {
            h.push(line);
        } else if line.trim_end() == "Traceback (most recent call last):" {
            held = Some(vec![]);
        } else {
            stderr.write_all(&buf).ok();
            stderr.flush().ok();
        }
    }
    let Some(h) = held else { return };
    let last = h.iter().rev().map(|l| l.trim_end()).find(|l| !l.is_empty() && !l.starts_with(' ')).unwrap_or("");
    let at = h.iter().rev().find_map(|l| {
        let rest = l.trim_start().strip_prefix("File \"")?;
        let (f, rest) = rest.split_once("\", line ")?;
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let f = Path::new(f).file_name().map_or(f.to_string(), |x| x.to_string_lossy().to_string());
        Some(format!(" ({f} line {n})"))
    });
    eprintln!("{last}{}", at.unwrap_or_default());
}

// Which path a build took (docs/ARCHITECTURE.md, "Which path ran"), as one JSON line on stderr:
//   typed    typed native code (the fast path)
//   pybridge typed native code; the Python libraries it imports run in CPython through libpython
//   dynamic  native code over dynamic values: the typed build was refused, `line`/`fn`/`why` say where and why
//   cpython  run by python3: Brevi could not build the file either way
// Printed for Python files on paths other than typed; --explain or BREVI_PATH=1 print it always (also for .bv
// files and BREVI_DYN=1 builds), BREVI_PATH=0 never.
// `extra` is more JSON fields (",\"py\":[...]").
fn say_path(path: &str, file: &str, line: usize, func: Option<String>, why: &str, extra: &str, relocate: bool) {
    let shown = match std::env::var("BREVI_PATH").ok().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) || (path != "typed" && PY_FILE.load(std::sync::atomic::Ordering::Relaxed) && !forced_dyn()),
    };
    if !shown {
        return;
    }
    let mut s = format!("{{\"path\":\"{path}\"");
    if !file.is_empty() {
        s.push_str(&format!(",\"file\":\"{}\"", esc(file)));
    }
    if line > 0 {
        s.push_str(&format!(",\"line\":{line}"));
    }
    if let Some(f) = func {
        s.push_str(&format!(",\"fn\":\"{}\"", esc(&f)));
    }
    if !why.is_empty() {
        s.push_str(&format!(",\"why\":\"{}\"", esc(why)));
    }
    s.push_str(extra);
    s.push('}');
    eprintln!("{}", if relocate { at(&s) } else { s });
}

// What CPython runs for a dynamic build (",\"py\":[...]"), or nothing.
fn dyn_py_field() -> String {
    let b = dynpy::bridged();
    if b.is_empty() {
        return String::new();
    }
    format!(",\"py\":[{}]", b.iter().map(|u| format!("\"{}\"", esc(u))).collect::<Vec<_>>().join(","))
}

fn forced_dyn() -> bool {
    std::env::var("BREVI_DYN").is_ok_and(|v| v == "1")
}

// The typed path's own report: typed, or pybridge naming what CPython runs.
fn say_typed() {
    if pybridge::active() && !engine::on() {
        let uses = pybridge::uses();
        let list = uses.iter().map(|u| format!("\"{}\"", esc(u))).collect::<Vec<_>>().join(",");
        let why = if uses.is_empty() { "library calls run in CPython".to_string() } else { format!("{} run in CPython", uses.join(", ")) };
        say_path("pybridge", "", 0, None, &why, &format!(",\"py\":[{list}]"), false);
    } else {
        say_path("typed", "", 0, None, "", "", false);
    }
}

// Why the typed build was refused, from its first error; with --explain, the other errors too (",\"also\":[...]").
fn typed_refusal(diags: &[Diag], src: &str) -> (usize, Option<String>, String, String) {
    let Some(d) = diags.first() else { return (0, None, "typed build refused".into(), String::new()) };
    let mut extra = String::new();
    if EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(f) = json::parse(&d.to_json()).and_then(|j| j.get("fix").and_then(|v| v.str()).map(String::from)) {
            extra.push_str(&format!(",\"fix\":\"{}\"", esc(&f)));
        }
        let more: Vec<String> = diags[1..].iter().take(5).map(|d| at(&d.to_json())).collect();
        if !more.is_empty() {
            extra.push_str(&format!(",\"also\":[{}]", more.join(",")));
        }
    }
    // The message as the error line gives it (no internal names: `type`, not `ax_type_of`).
    let msg = json::parse(&d.to_json()).and_then(|j| j.get("msg").and_then(|v| v.str()).map(String::from)).unwrap_or_else(|| d.msg.clone());
    (d.line, def_at_joined(src, d.line), msg, extra)
}

// enclosing_def for a line of the joined source (main file and its modules), looked up in the file it is in.
fn def_at_joined(src: &str, line: usize) -> Option<String> {
    let segs = SEGS.get().filter(|s| s.len() > 1);
    let Some(segs) = segs else { return enclosing_def(src, line) };
    let (f, l) = modules::locate(segs, line);
    if f.is_empty() {
        return enclosing_def(src, line);
    }
    let seg = segs.iter().find(|s| s.file == f)?;
    enclosing_def(&std::fs::read_to_string(&seg.path).ok()?, l)
}

// The def (or class method, `Board.move`) a line is in; None at the top level.
fn enclosing_def(src: &str, line: usize) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let first = *lines.get(line.checked_sub(1)?)?;
    let indent = |l: &str| l.len() - l.trim_start().len();
    let name = |t: &str| -> Option<(bool, String)> {
        let t = t.strip_prefix("async ").unwrap_or(t);
        let (def, r) = if let Some(r) = t.strip_prefix("def ").or_else(|| t.strip_prefix("fn ")) {
            (true, r)
        } else {
            (false, t.strip_prefix("class ")?)
        };
        let n: String = r.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        (!n.is_empty()).then_some((def, n))
    };
    let mut names: Vec<(bool, String)> = vec![];
    let mut level = indent(first);
    if let Some(n) = name(first.trim_start()) {
        names.push(n);
    }
    for l in lines[..line - 1].iter().rev() {
        let t = l.trim_start();
        if t.is_empty() || t.starts_with('#') || indent(l) >= level {
            continue;
        }
        level = indent(l);
        if let Some(n) = name(t) {
            names.push(n);
        }
        if level == 0 {
            break;
        }
    }
    // Innermost def and the classes and defs around it.
    let k = names.iter().position(|(def, _)| *def)?;
    let mut parts: Vec<String> = names[k..].iter().map(|(_, n)| n.clone()).collect();
    parts.reverse();
    Some(parts.join("."))
}

// The first error of a failed rustc run over the typed build's code, in one line.
fn rustc_first_error(stderr: &str) -> String {
    for l in stderr.lines() {
        if let Some(j) = json::parse(l) {
            if j.get("level").and_then(|v| v.str()) == Some("error") {
                if let Some(m) = j.get("message").and_then(|v| v.str()) {
                    return format!("the typed build's Rust did not compile ({m})");
                }
            }
        } else if let Some(m) = l.strip_prefix("error") {
            return format!("the typed build's Rust did not compile ({})", m.trim_start_matches(|c| c != ':').trim_start_matches(':').trim());
        }
    }
    "the typed build's Rust did not compile".into()
}

// The path report of a run handed to CPython: where and why Brevi's build stopped (its first error, from the
// strict child build's stdout) and, with --explain, why the dynamic build refused the file too.
fn say_cpython(file: &str, python: &str, out: &str, err: &str) {
    let first = out.lines().filter(|l| l.starts_with('{')).filter_map(json::parse).find(|j| j.get("msg").is_some());
    let get = |k: &str| first.as_ref().and_then(|j| j.get(k)).and_then(|v| v.str()).unwrap_or("").to_string();
    let line = first.as_ref().and_then(|j| j.get("line")).and_then(|v| v.num()).unwrap_or(0.0) as usize;
    let dir = Path::new(file).parent().unwrap_or(Path::new("."));
    let in_file = get("file");
    let src = std::fs::read_to_string(if in_file.is_empty() { PathBuf::from(file) } else { dir.join(&in_file) }).unwrap_or_default();
    let mut extra = format!(",\"by\":\"{}\"", esc(python));
    if EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(d) = err.lines().find_map(|l| l.strip_prefix("{\"dyn_blocked\":").and_then(|r| r.strip_suffix('}'))) {
            extra.push_str(&format!(",\"dynamic\":{d}"));
        }
    }
    let why = if first.is_some() { format!("Brevi could not build it: {}", get("msg")) } else { "Brevi could not build it".into() };
    say_path("cpython", &in_file, line, if line > 0 { enclosing_def(&src, line) } else { None }, &why, &extra, false);
}

// Applies the edits that front-end errors suggest, a round at a time, until the program passes the front end.
// Without `all`, only sure edits are used, and only if they make the whole front end pass.
fn autofix(src: &str, all: bool) -> (String, Vec<(usize, String, String)>) {
    let mut cur = src.to_string();
    let mut done = Vec::new();
    for _ in 0..8 {
        let Err(diags) = compile_to_rust(&cur, true, true) else { return (cur, done) };
        let mut changed = false;
        for d in diags.iter().filter(|d| all || d.sure) {
            if let Some(next) = d.apply(&cur) {
                let (old, new) = d.edit.clone().unwrap();
                done.push((d.line, old.trim_matches(|c| c == '.' || c == '(').to_string(), new.trim_matches(|c| c == '.' || c == '(').to_string()));
                cur = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if all || compile_to_rust(&cur, true, true).is_ok() {
        (cur, done)
    } else {
        (src.to_string(), vec![])
    }
}

// Whether a function builds a dict literal with string keys whose values are all names or fields.
fn has_untyped_map(items: &[ast::Item]) -> bool {
    let mut hit = false;
    for it in items {
        if let ast::Item::Fn { body, .. } = it {
            infer::walk(body, &mut |e| {
                if let ast::Expr::Map(ps) = e {
                    let open = |v: &ast::Expr| matches!(v, ast::Expr::Ident(_) | ast::Expr::Field(..) | ast::Expr::Call(..) | ast::Expr::Method(..));
                    let lit = |v: &ast::Expr| matches!(v, ast::Expr::Bool(_) | ast::Expr::Int(_) | ast::Expr::Str(_) | ast::Expr::Float(_));
                    if ps.len() > 1 && ps.iter().all(|(k, v)| matches!(k, ast::Expr::Str(_)) && (open(v) || lit(v))) && ps.iter().any(|(_, v)| open(v)) {
                        hit = true;
                    }
                }
            });
        }
    }
    hit
}

// Records, inference and checks.
fn typed(mut items: Vec<ast::Item>, py: bool, eager: bool) -> Result<Vec<ast::Item>, Vec<Diag>> {
    if py {
        // Dicts used as records (values of different types under fixed keys) become record types.
        let (_, mixed) = infer::empty_types(&items);
        records::json_lists(&mut items);
        records::records_with(&mut items, &mixed, eager);
        // A dict literal built from untyped parameters shows its mixed types only once the parameters
        // are inferred: infer a copy, then make those dicts records too. Even when the copy fails to
        // type (often because those dicts are not records yet), what it worked out shows which mix types.
        if !eager && has_untyped_map(&items) {
            let mut copy = items.clone();
            infer::auto_mut(&mut copy, py);
            let _ = infer::returns(&mut copy);
            let (_, mixed) = infer::empty_types(&copy);
            if !mixed.is_empty() {
                records::records(&mut items, &mixed);
            }
        }
    }
    tm("typed: records");
    infer::auto_mut(&mut items, py);
    tm("typed: auto_mut");
    infer::returns(&mut items)?;
    tm("typed: returns");
    // Again: returns can turn `obj.m(x)` into a call of the class method `C_m(obj, x)`, which may change obj.
    infer::auto_mut(&mut items, py);
    infer::rows_reset();
    let errs = infer::check(&mut items, py);
    tm("typed: check");
    if !errs.is_empty() {
        return Err(errs);
    }
    // A tuple type used both for mixed-list rows and for real tuples: rows would print as tuples (and be
    // copied where Python shares them), so a Python file goes to the dynamic build.
    if infer::rows_clash() && py {
        return Err(vec![diag::Diag::new("check", 1, 1, "a mixed list such as [1, 'a'] and a tuple such as (1, 'a') have the same item types here, which the typed build can't keep apart")]);
    }
    if infer::rows_clash() {
        for it in items.iter_mut() {
            if let ast::Item::Fn { body, .. } | ast::Item::Test { body, .. } = it {
                parser::walk_mut(body, &mut |e| {
                    if let ast::Expr::Call(f, a, _) = e {
                        if f == "ax_row" && a.len() == 1 {
                            *e = a.remove(0).1;
                        }
                    }
                });
            }
        }
    }
    Ok(items)
}

// BREVI_TIME=1: how long each stage took, on stderr (ms since start).
pub(crate) fn tm(what: &str) {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let t0 = *T0.get_or_init(std::time::Instant::now);
    if *ON.get_or_init(|| std::env::var("BREVI_TIME").is_ok_and(|v| v == "1")) {
        eprintln!("time {:>7.1} ms  {what}", t0.elapsed().as_secs_f64() * 1000.0);
    }
}

pub fn compile_to_rust(src: &str, moves: bool, hints: bool) -> Result<String, Vec<Diag>> {
    // pybridge: a function of a Python module that Brevi lacks (math.gcd, itertools.combinations) is run by
    // CPython; build again with it.
    let mut k = 0;
    loop {
        let r = compile_once(src, moves, hints);
        k += 1;
        match &r {
            Err(ds) if k < 8 && python_style(src) && pybridge::learn(ds, src) => continue,
            // --sandbox without allow native: say that CPython would have run it, not just that Brevi lacks it.
            // One error that names the calls, in place of the "unknown method" and "unknown name" errors each
            // call gave; errors about anything else stay.
            Err(ds) if !pybridge::blocked().is_empty() => {
                let mut b = pybridge::blocked();
                b.sort();
                b.dedup();
                let names: Vec<String> = b.iter().map(|(m, n)| format!("{m}.{n}")).collect();
                let line = ds.iter().filter(|d| pybridge::blocked_noise(d)).map(|d| d.line).min().unwrap_or(1);
                let one = if names.len() == 1 { "runs" } else { "run" };
                let mut mods: Vec<String> = b.iter().map(|(m, _)| m.split('.').next().unwrap_or(m).to_string()).collect();
                mods.dedup();
                let d = Diag::new("caps", line, 1, &format!("{} {one} in CPython, which needs allow native", names.join(", "))).fix_hint(&caps::grant_hint("native", Some(&mods.join(", "))));
                let mut out = vec![d];
                out.extend(ds.iter().filter(|d| !pybridge::blocked_noise(d)).cloned());
                return Err(out);
            }
            _ => return r,
        }
    }
}

fn compile_once(src: &str, moves: bool, hints: bool) -> Result<String, Vec<Diag>> {
    let (src, mut foreign) = ffi::split(src);
    tm("compile: start");
    let mut items = parser::parse_program(&src).map_err(|d| vec![lib_hint(d, &src)])?;
    tm("compile: parsed");
    if gui::active() {
        foreign.rust_top.push_str(&gui::runtime());
    }
    // pybridge: CPython for the Python libraries the program uses.
    // --sandbox --allow-py: only the modules the caller named.
    let ungranted: Vec<String> = if pybridge::active() { pybridge::uses().into_iter().filter(|u| !pybridge::py_allowed(u)).collect() } else { vec![] };
    if pybridge::active() && !pybridge::no_native() && !ungranted.is_empty() {
        let mut mods: Vec<String> = ungranted.iter().map(|u| u.split('.').next().unwrap_or(u).to_string()).collect();
        mods.dedup();
        return Err(vec![Diag::new("caps", 1, 1, &format!("{} runs in CPython, which needs allow native", ungranted.join(", "))).fix_hint(&caps::grant_hint("native", Some(&mods.join(", "))))]);
    }
    if pybridge::active() && pybridge::no_native() {
        // Only modules the dynamic build has: it builds them, so say why the typed build did not.
        if pybridge::stdlib_only() || pybridge::numpy_only() {
            return Err(vec![Diag::new("check", 1, 1, &format!("{} not in the typed build", pybridge::uses().join(", "))).fix_hint("the dynamic build has them: build it without BREVI_DYN=0")]);
        }
        return Err(vec![Diag::new("caps", 1, 1, "a call CPython runs needs allow native").fix_hint(&caps::grant_hint("native", None))]);
    }
    if pybridge::active() {
        // Libraries Brevi has natively run on its own engine (engine.rs); others in CPython.
        let (mods, froms) = pybridge::engine_imports();
        match engine::glue(&mods, &froms, &items) {
            Some(g) => foreign.rust_top.push_str(&g),
            None => {
                pybridge::python_ok().map_err(|m| vec![Diag::new("check", 1, 1, &m).fix_hint("run the program with python3")])?;
                foreign.rust_top.push_str(&pybridge::runtime());
            }
        }
    }
    ffi::attach(&mut items, &foreign);
    used_consts_first(&mut items);
    if python_style(&src) {
        bigint::promote(&mut items);
    }
    // Set before alias.rs runs: its Python-only checks read it (once set after, the first build of every
    // Python file skipped them, so `zs = xs; xs.sort(); print(zs)` printed the unsorted copy).
    let py = python_style(&src);
    infer::PY.store(py, std::sync::atomic::Ordering::Relaxed);
    // Python names share objects (`r = rows[k]; r[0] -= 1` changes rows); Brevi programs keep copies.
    if py {
        let errs = alias::writeback(&mut items);
        if !errs.is_empty() {
            return Err(errs);
        }
    }
    // Python dicts built from names (`{"sku": sku, "qty": qty}` in a method) are tried as records first;
    // if that does not type, the program is typed as before.
    let mut items = if py && has_untyped_map(&items) {
        match typed(items.clone(), py, true) {
            Ok(t) => t,
            Err(_) => {
                typed(items, py, false)?
            }
        }
    } else {
        typed(items, py, false)?
    };
    tm("compile: typed");
    emit::auto_try(&mut items);
    let mut em = emit::Emitter::new(&items, hints);
    em.foreign = foreign;
    em.py_mode = python_style(&src);
    em.files = SEGS.get().map(|s| s.iter().skip(1).map(|g| (g.file.clone(), g.start, g.len)).collect()).unwrap_or_default();
    if moves {
        moves::mark_moves(&mut items);
    }
    em.borrow_loops = moves;
    let rust = em.program(&items);
    tm("compile: emitted");
    if em.diags.is_empty() {
        Ok(pystd::drop_shim_lines(rust))
    } else {
        Err(em.diags)
    }
}

// Names a used file sets at its top (`PAT = "..."`) are set before the main file's own statements run, as a
// Python module's are on import.
fn used_consts_first(items: &mut [ast::Item]) {
    let Some(first) = SEGS.get().and_then(|s| s.get(1)).map(|s| s.start) else { return };
    for it in items.iter_mut() {
        if let ast::Item::Fn { script: true, body: ast::Expr::Block(stmts, _), .. } = it {
            let line = |st: &ast::Stmt| match st {
                ast::Stmt::Let { line, .. } | ast::Stmt::Assign { line, .. } => *line,
                _ => 0,
            };
            let (mut used, own): (Vec<_>, Vec<_>) = std::mem::take(stmts).into_iter().partition(|st| line(st) >= first);
            used.extend(own);
            *stmts = used;
        }
    }
}

// Parse errors on lines that try to reach a library the Python or Rust way point at how Brevi does it.
fn lib_hint(mut d: Diag, src: &str) -> Diag {
    let l = src.lines().nth(d.line.wrapping_sub(1)).unwrap_or("").trim();
    let code = l.split('#').next().unwrap_or("");
    if l.starts_with("import ") || (l.starts_with("from ") && l.contains(" import ")) {
        d.fix = Some("Python modules are used inside a py fn (py fn f(x: int) -> int:, then Python lines); see brevi guide Libraries".into());
    } else if code.split('"').step_by(2).any(|part| part.contains("::")) {
        d.fix = Some("call crates from a rust fn (rust fn f(x: str) -> str: then Rust lines) after use NAME \"VERSION\"; see brevi guide Libraries".into());
    }
    d
}

// This process's own build directory for a file: build_dir_path(file)/pPID. Two brevi processes that build the
// same file at once (MCP sessions, parallel verify runs) once shared one directory, so one could overwrite the
// generated Rust or the binary the other was about to run (and that run then fell back to CPython). Built
// binaries are still shared through the cache in cache_dir()/bin, so a second process reuses the first's build.
// Directories of processes that have ended are removed here, the next time the file is built.
pub(crate) fn build_dir(file: &str) -> PathBuf {
    let base = build_dir_path(file);
    let me = std::process::id();
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            if !is_dir {
                // Left by brevi before 0.27, which built straight into the shared directory.
                if name != "build.lock" {
                    std::fs::remove_file(e.path()).ok();
                }
                continue;
            }
            let Some(pid) = name.strip_prefix('p').and_then(|p| p.parse::<u32>().ok()) else { continue };
            // (proc::alive is always true off unix: there a directory goes once it is a day old)
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a > std::time::Duration::from_secs(86400));
            if pid != me && (!proc::alive(pid) || (old && !cfg!(unix))) {
                std::fs::remove_dir_all(e.path()).ok();
            }
        }
    }
    let dir = base.join(format!("p{me}"));
    std::fs::create_dir_all(&dir).ok();
    let mut own = OWN_BUILD_DIRS.lock().unwrap();
    if !own.contains(&dir) {
        if own.is_empty() {
            unsafe extern "C" {
                fn atexit(f: extern "C" fn()) -> i32;
            }
            // SAFETY: registers a plain function with the C runtime; std::process::exit runs it.
            unsafe { atexit(remove_own_build_dirs) };
        }
        own.push(dir.clone());
    }
    dir
}
// This process's build folders, removed when it exits (a process that is killed leaves its folder to the next
// build_dir call of that file).
static OWN_BUILD_DIRS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
extern "C" fn remove_own_build_dirs() {
    if let Ok(own) = OWN_BUILD_DIRS.try_lock() {
        for d in own.iter() {
            std::fs::remove_dir_all(d).ok();
            // The file's folder too, once it is empty (no other process building in it, no cargo project).
            if let Some(base) = d.parent() {
                std::fs::remove_dir(base).ok();
            }
        }
    }
}

// One build at a time of something built once into the cache (a runtime, a library of copies): true when this
// process took `lock` and should build, false when `ready()` came true while another process built it. The lock
// names its owner (pid and boot id); a lock whose owner has died (killed, or the machine restarted) is taken over
// at once, and any lock over 20 minutes old is too. The owner removes `lock` when done.
pub fn build_lock(lock: &Path, ready: &dyn Fn() -> bool) -> bool {
    use std::io::Write;
    let boot = || std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default().trim().to_string();
    let me = format!("{} {}", std::process::id(), boot());
    let stale = || {
        let old = std::fs::metadata(lock).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() >= 1200);
        let owner = std::fs::read_to_string(lock).unwrap_or_default();
        let mut it = owner.split_whitespace();
        let dead = match (it.next().and_then(|p| p.parse::<u32>().ok()), it.next().unwrap_or("")) {
            (Some(pid), b) => b != boot() || !proc::alive(pid),
            _ => false, // being written, or a lock from an older brevi: only its age counts
        };
        old || dead
    };
    // The lock's folder may not exist yet (a fresh cache): without it every open fails and nothing ever takes the lock.
    if let Some(dir) = lock.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    while !ready() {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(lock) {
            Ok(mut f) => {
                f.write_all(me.as_bytes()).ok();
                return true;
            }
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                // The lock can't be made at all (no folder, no write access): build without it rather than wait forever.
                return true;
            }
            Err(_) => {}
        }
        if stale() {
            // Remove it and race for a new one, so only one waiter takes over.
            std::fs::remove_file(lock).ok();
            continue;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    false
}

// The directory that holds a file's builds (one per source path, a folder per process inside it, see
// build_dir); the MCP server removes it when done.
pub fn build_dir_path(file: &str) -> PathBuf {
    let abs = std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file));
    let mut h: u64 = 1469598103934665603;
    for b in abs.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    std::env::temp_dir().join(format!("brevi-{h:x}"))
}

// The builds to try in order: with inferred types for empty values, then without (the errors shown come
// from the first). Then the same two with Python's searched lists kept as plain Vecs (emit::FAST_SETS),
// should a hash index not fit (an item type without Hash). Each is (program, the same program without moves).
// Only the first is made up front: every other one is a whole front-end pass (about as long as inference),
// made only when the builds before it failed. Making all eight first cost a check or a cached run up to 5 s.
struct Versions<'a> {
    src: &'a str,
    first: String,
}
const VERSIONS: [(bool, bool); 4] = [(true, true), (true, false), (false, true), (false, false)];
impl Versions<'_> {
    fn make(&self, i: usize, moves: bool) -> Option<String> {
        if i == 0 && moves {
            return Some(self.first.clone());
        }
        let (f, h) = VERSIONS[i];
        emit::FAST_SETS.store(f, std::sync::atomic::Ordering::Relaxed);
        let r = compile_to_rust(self.src, moves, h).ok();
        emit::FAST_SETS.store(true, std::sync::atomic::Ordering::Relaxed);
        r
    }
}

// Compiles generated Rust. rustc errors are mapped back to Brevi lines and printed as JSON.
// The program without moves is used if moving a value broke the borrow rules. Errors are reported from the
// first version tried.
fn rustc(versions: &mut Versions, dir: &Path, out: &Path, extra: &[&str], link: bool) -> bool {
    const MOVE_CODES: [&str; 8] = ["E0382", "E0505", "E0507", "E0502", "E0499", "E0506", "E0716", "E0597"];
    let release = extra.contains(&"opt-level=3");
    // A program that implements a prelude trait for a prelude type cannot use the library: paste the prelude.
    let orphan = |e: &str| ["E0117", "E0116", "E0210"].iter().any(|c| e.contains(&format!("\"code\":\"{c}\"")));
    // The first build's result is kept, so it is not compiled a second time below.
    let mut lib = true;
    let mut probe = None;
    let first = use_runtime_lib(&versions.first, release);
    if first.starts_with(RT_HEADER) {
        let res = compile(&first, dir, out, extra, link);
        if matches!(&res, Some(Err(e)) if orphan(e)) {
            lib = false;
        } else {
            probe = Some(res);
        }
    }
    let shape = |r: String| if lib { use_runtime_lib(&r, release) } else { r };
    let mut seen: Vec<String> = vec![];
    let mut last = None;
    for i in 0..VERSIONS.len() {
        let Some(rust) = versions.make(i, true) else { continue };
        if seen.contains(&rust) {
            continue;
        }
        seen.push(rust.clone());
        let rust = shape(rust);
        let mut r = (rust.clone(), match probe.take().unwrap_or_else(|| compile(&rust, dir, out, extra, link)) {
            Some(Ok(())) => return true,
            Some(Err(e)) => e,
            None => return false,
        });
        if MOVE_CODES.iter().any(|c| r.1.contains(&format!("\"code\":\"{c}\""))) {
            if let Some(p) = versions.make(i, false).map(shape) {
                r = (p.clone(), match compile(&p, dir, out, extra, link) {
                    Some(Ok(())) => return true,
                    Some(Err(e)) => e,
                    None => return false,
                });
            }
        }
        // Report the first build's errors: it has the inferred types, so its errors name the real gap
        // (without them, a `best = None` that inference typed would be blamed instead).
        if last.is_none() {
            last = Some(r);
        }
    }
    match last {
        Some((rust, stderr)) if DEFER_REPORT.load(std::sync::atomic::Ordering::Relaxed) => *DEFERRED.lock().unwrap() = Some((rust, stderr)),
        Some((rust, stderr)) => report(&rust, &stderr),
        None => {}
    }
    false
}
// Python files keep rustc's errors until the dynamic build has been tried.
static DEFER_REPORT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DEFERRED: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

// Crates the program uses (`use` lines, and crate paths in rust fns). With any, builds go through cargo.
static DEPS: std::sync::OnceLock<Vec<ffi::Dep>> = std::sync::OnceLock::new();
// Source lines written in Rust (rust fn and rust: blocks): their errors keep rustc's own help.
static RUST_LINES: std::sync::OnceLock<Vec<(usize, usize)>> = std::sync::OnceLock::new();

// Where crates are built once and shared by every program: $BREVI_CACHE, else ~/.cache/brevi.
pub(crate) fn cache_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BREVI_CACHE") {
        // Absolute, so a background build (src/tier.rs), which runs in its own folder, finds the same cache.
        return std::path::absolute(&d).unwrap_or_else(|_| PathBuf::from(d));
    }
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    home.join(".cache").join("brevi")
}

// Builds with cargo, for programs that use crates. Same results as compile().
fn cargo_compile(deps: &[ffi::Dep], rust: &str, dir: &Path, out: &Path, extra: &[&str], link: bool) -> Option<Result<(), String>> {
    // The cargo project stays at one path per source file (build_dir_path/cargo, not this process's folder), so
    // cargo's fingerprints in the shared target directory still match and an unchanged program is not rebuilt.
    // A lock file keeps two processes building the same file from writing it at the same time.
    let base = dir.parent().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("brevi-"))).unwrap_or(dir);
    let name = base.file_name().map(|n| n.to_string_lossy().replace('-', "_")).unwrap_or_else(|| "brevi_prog".into());
    let dir = &base.join("cargo");
    std::fs::create_dir_all(dir).ok();
    let lock = std::fs::File::create(base.join("build.lock")).ok();
    if let Some(l) = &lock {
        l.lock().ok();
    }
    let toml = ffi::cargo_toml(&name, deps);
    let tp = dir.join("Cargo.toml");
    if std::fs::read_to_string(&tp).ok().as_deref() != Some(toml.as_str()) {
        std::fs::write(&tp, &toml).expect("write Cargo.toml");
    }
    // On the first line, so rustc's line numbers still match the generated code.
    std::fs::write(dir.join("main.rs"), format!("#![allow(warnings)] {rust}")).expect("write generated rust");
    let test = extra.contains(&"--test");
    let mut c = Command::new("cargo");
    if extra.contains(&"--emit=metadata") {
        c.arg("check");
    } else if test {
        c.args(["test", "--no-run"]);
    } else {
        c.arg("build");
    }
    if extra.contains(&"opt-level=3") {
        c.arg("--release");
    }
    c.args(["-q", "--message-format=json", "--manifest-path"]).arg(&tp);
    c.env("CARGO_TARGET_DIR", cache_dir().join("target"));
    let o = match c.output() {
        Ok(o) => o,
        Err(e) => {
            println!("{{\"e\":\"io\",\"msg\":\"cargo not found (needed for crates): {}\"}}", esc(&e.to_string()));
            return None;
        }
    };
    let stdout = String::from_utf8_lossy(&o.stdout);
    let mut msgs = String::new();
    let mut exe = None;
    for l in stdout.lines() {
        if l.starts_with("{\"reason\":\"compiler-message\"") {
            if let Some(p) = l.find(",\"message\":") {
                msgs.push_str(&l[p + 11..l.len() - 1]);
                msgs.push('\n');
            }
        } else if l.starts_with("{\"reason\":\"compiler-artifact\"") && l.contains(&format!("\"name\":\"{name}\"")) {
            if let Some(e) = json::parse(l).and_then(|j| j.get("executable").and_then(|e| e.str()).map(String::from)) {
                exe = Some(e);
            }
        }
    }
    if o.status.success() {
        if link {
            let Some(e) = exe else {
                println!("{{\"e\":\"io\",\"msg\":\"cargo built no program\"}}");
                return None;
            };
            if std::fs::copy(&e, out).is_err() {
                println!("{{\"e\":\"io\",\"msg\":\"cannot write {}\"}}", esc(&out.to_string_lossy()));
                return None;
            }
        }
        return Some(Ok(()));
    }
    if msgs.contains("\"level\":\"error\"") {
        return Some(Err(msgs));
    }
    // A crate that cannot be found or fetched: point at its use line.
    let stderr = String::from_utf8_lossy(&o.stderr);
    let first: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).take(4).collect();
    let text = first.join(" ").replace("error: ", "");
    let dep = deps.iter().find(|d| text.contains(&format!("`{}`", d.name)) || text.contains(&format!("named {}", d.name)));
    let fix = if text.contains("no matching package") || text.contains("failed to select a version") {
        "check the crate name and version on crates.io: use NAME \"VERSION\""
    } else if text.contains("feature") {
        "check the feature names: use NAME \"VERSION\" \"feat1 feat2\""
    } else {
        "crates download from crates.io on first use; check the network"
    };
    println!("{}", at(&format!("{{\"line\":{},\"msg\":\"cargo: {}\",\"fix\":\"{}\"}}", dep.map_or(0, |d| d.line), esc(text.trim()), esc(fix))));
    None
}

// None when rustc could not be started.
// The typed-build guard over the file and the local modules it imports (directly or through each other).
// With the names (see typed_guard::check_soft) when every reason found is a set walked in hash order on them.
// Second: a file the guard could not read (line, why, file), when no reason found was final.
fn guard_files(file: &str) -> (Option<(usize, String, PathBuf, Vec<String>)>, Option<(usize, String, PathBuf)>) {
    let dir = Path::new(file).parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut todo = vec![PathBuf::from(file)];
    let mut seen: Vec<PathBuf> = vec![];
    let mut first: Option<(usize, String, PathBuf)> = None;
    let mut soft: Vec<String> = vec![];
    let mut unread: Option<(usize, String, PathBuf)> = None;
    while let Some(f) = todo.pop() {
        if seen.contains(&f) {
            continue;
        }
        seen.push(f.clone());
        let Ok(src) = std::fs::read_to_string(&f) else { continue };
        let src = if seen.len() == 1 { pysentinel::rewrite(&pygen::rewrite(&src)) } else { src };
        let shown = |why: String| if seen.len() == 1 { why } else { format!("{}: {why}", f.file_name().unwrap_or_default().to_string_lossy()) };
        // `allow` lines are not Python, and the guard reads what the typed build reads once they are gone.
        match typed_guard::check_soft(&caps::split(&src).0) {
            Ok(Some((line, why, names))) => {
                let why = shown(why);
                if names.is_empty() {
                    return (Some((line, why, f, vec![])), None);
                }
                first.get_or_insert((line, why, f.clone()));
                soft.extend(names);
            }
            Ok(None) => {}
            Err((line, msg)) => {
                unread.get_or_insert((line, shown(format!("the guard cannot read this file: {msg}")), f.clone()));
            }
        }
        for l in src.lines() {
            let t = l.trim_start();
            let names: Vec<&str> = if let Some(r) = t.strip_prefix("from ") {
                r.split_whitespace().next().into_iter().collect()
            } else if let Some(r) = t.strip_prefix("import ") {
                r.split(',').filter_map(|x| x.split_whitespace().next()).collect()
            } else {
                vec![]
            };
            for n in names {
                let m = dir.join(format!("{}.py", n.trim_start_matches('.')));
                if m.exists() {
                    todo.push(m);
                }
            }
        }
    }
    (first.map(|(l, w, f)| (l, w, f, soft)), unread)
}

// Builds are cached by a hash of the generated code, the flags and the rustc version, so running the same
// program again (an agent re-running its tests, or an MCP call) skips rustc. BREVI_NO_CACHE=1 turns this off.
fn compile(rust: &str, dir: &Path, out: &Path, extra: &[&str], link: bool) -> Option<Result<(), String>> {
    if let Some(deps) = DEPS.get().filter(|d| !d.is_empty()) {
        return cargo_compile(deps, rust, dir, out, extra, link);
    }
    let release = extra.contains(&"opt-level=3");
    let rt = if rust.starts_with(RT_HEADER) { runtime_lib(release) } else { None };
    let use_cache = runcache::on();
    // The prelude is linked from a prebuilt library rather than pasted in, so its own hash is part of the key:
    // a changed runtime (a brevi upgrade) must not reuse a binary or error built against the old one.
    static PRELUDE_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let ph = PRELUDE_HASH.get_or_init(|| format!("{:016x}", fnv(&[emit::PRELUDE, env!("CARGO_PKG_VERSION")], 3)));
    let eng = engine::lib().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let key_for = |extra: &[&str]| format!("{:016x}{:016x}", fnv(&[rust, &extra.join(" "), if link { "link" } else { "meta" }, rustc_version(), ph, &eng], 0), fnv(&[rust, &extra.join(" "), rustc_version(), ph, &eng], 7));
    let key = key_for(extra);
    let cdir = cache_dir().join("bin");
    let cached = cdir.join(&key);
    // The edit loop (src/tier.rs): an opt-level 1 binary not built yet is built at opt-level 0 now, and at
    // opt-level 1 in the background.
    if link && tier::on() && !cached.exists() {
        if let Some(quick) = tier::quick_flags(extra) {
            tm("tiered: building at opt-level 0, opt-level 1 in the background");
            let r = compile(rust, dir, out, &quick, link);
            if matches!(r, Some(Ok(()))) {
                let rust = engine_rust(rust);
                let mut args = rustc_args(extra, rt.as_deref(), release);
                args.extend(["-C".into(), "strip=debuginfo".into()]);
                tier::background(&cached, &rust, &args);
            }
            return r;
        }
    }
    if use_cache {
        if let Ok(e) = std::fs::read_to_string(cached.with_extension("err")) {
            return Some(Err(e));
        }
        if cached.exists() && (!link || copy_bin(&cached, out)) {
            touch(&cached);
            return Some(Ok(()));
        }
    }
    let rust: &str = &engine_rust(rust);
    let rs = dir.join("main.rs");
    std::fs::write(&rs, rust).expect("write generated rust");
    let mut c = rustc_cmd();
    c.args(rustc_args(extra, rt.as_deref(), release));
    if link {
        // Debug info is never used (runtime errors carry Brevi line numbers), and it is most of the file.
        c.args(["-C", "strip=debuginfo"]);
        c.arg("-o").arg(out);
    } else {
        c.arg("--out-dir").arg(dir);
    }
    c.arg(&rs);
    let mut stopped = false;
    let r = match c.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            // Killed (out of memory, a time limit): whatever it printed is not about the program.
            stopped = o.status.code().is_none();
            Err(String::from_utf8_lossy(&o.stderr).to_string())
        }
        Err(e) => {
            println!("{{\"e\":\"io\",\"msg\":\"rustc not found: {}\"}}", esc(&e.to_string()));
            return None;
        }
    };
    let about_code = matches!(&r, Err(e) if !stopped && e.contains("\"level\":\"error\"") && about_the_code(e));
    if r.is_err() && !about_code {
        runcache::not_kept();
    }
    if use_cache && std::fs::create_dir_all(&cdir).is_ok() {
        let tmp = cdir.join(format!("{key}.{}", std::process::id()));
        let saved = match &r {
            Ok(()) if link => std::fs::copy(out, &tmp).is_ok(),
            Ok(()) => std::fs::write(&tmp, "").is_ok(),
            Err(e) if about_code => std::fs::write(&tmp, e).is_ok(),
            Err(_) => false,
        };
        let dest = if r.is_ok() { cached.clone() } else { cached.with_extension("err") };
        if saved && std::fs::rename(&tmp, &dest).is_ok() {
            runcache::prune(&cdir);
        } else {
            std::fs::remove_file(&tmp).ok();
        }
    }
    Some(r)
}

// Libraries on Brevi's engine: its runtime is linked too, and the program uses that runtime's allocator.
fn engine_rust(rust: &str) -> String {
    match engine::lib() {
        // (as many lines as it replaces, so rustc's line numbers still match the program's)
        Some(_) => rust.replacen(include_str!("alloc_rt.rs"), &format!("pub use brevi_dyn::AX_ASCII;{}", "\n".repeat(include_str!("alloc_rt.rs").matches('\n').count())), 1),
        None => rust.to_string(),
    }
}

// rustc's arguments for a program build, without the output and the source file.
fn rustc_args(extra: &[&str], rt: Option<&Path>, release: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![];
    if let Some(lib) = engine::lib() {
        a.extend(["--extern".into(), format!("brevi_dyn={}", lib.display())]);
    }
    a.extend(["--edition", "2021", "--error-format=json", "--crate-name", "brevi_prog", "-A", "warnings", "-C", "overflow-checks=on"].map(String::from));
    a.extend(extra.iter().map(|e| e.to_string()));
    if !release {
        a.extend(["-C".into(), codegen_units().into()]);
    }
    if let Some(lib) = rt {
        a.extend(["--extern".into(), format!("brevi_rt={}", lib.display())]);
    }
    a
}

// True when every rustc error points into the program. An error with no place in it (a folder that could not
// be written, a full disk) is about the machine: kept out of the cache, so it goes once the machine is fixed.
// A crate that could not be loaded (the runtime library missing, unreadable or built by another rustc) points
// at the program's `extern crate` line, yet is about the cache: not about the code either.
fn about_the_code(stderr: &str) -> bool {
    const CRATE_LOAD: [&str; 6] = ["E0460", "E0461", "E0462", "E0463", "E0514", "E0786"];
    stderr.lines().filter_map(json::parse).filter(|j| j.get("level").and_then(|v| v.str()) == Some("error")).all(|j| {
        let msg = j.get("message").and_then(|v| v.str()).unwrap_or("");
        let code = j.get("code").and_then(|c| c.get("code")).and_then(|c| c.str()).unwrap_or("");
        msg.starts_with("aborting due to") || (j.get("spans").is_some_and(|v| !v.arr().is_empty()) && !CRATE_LOAD.contains(&code))
    })
}

// The first line of a program built against the runtime library instead of a pasted prelude.
const RT_HEADER: &str = "#![allow(warnings)] extern crate brevi_rt; use brevi_rt::*;\n";

// Swaps the pasted prelude for a use of the runtime library, when the library can be built.
// rustc then skips type-checking and compiling about 2,600 lines of prelude on every build.
// Release builds keep the pasted prelude: without it, prelude functions cannot be inlined into the program
// (word counts ran 20% slower), and a release build is made once, not on every edit.
fn use_runtime_lib(rust: &str, release: bool) -> String {
    if release {
        return rust.to_string();
    }
    let short = &emit::PRELUDE[..emit::PRELUDE.find("// @regex").unwrap_or(emit::PRELUDE.len())];
    let rest = rust.strip_prefix(emit::PRELUDE).or_else(|| rust.strip_prefix(short));
    match rest {
        // Python interop implements prelude traits for prelude types, which only works inside one crate.
        Some(rest) if DEPS.get().map_or(true, |d| d.is_empty()) && !rest.contains("trait AxFromJson") && engine::lib().is_none() && runtime_lib(release).is_some() => format!("{RT_HEADER}{rest}"),
        _ => rust.to_string(),
    }
}

// The prelude as a library: every top-level item public, built once per rustc version and build mode
// into the cache. None (fall back to pasting the prelude) if it cannot be built.
fn runtime_lib(release: bool) -> Option<PathBuf> {
    static LIBS: [std::sync::OnceLock<Option<PathBuf>>; 2] = [std::sync::OnceLock::new(), std::sync::OnceLock::new()];
    LIBS[release as usize]
        .get_or_init(|| {
            if std::env::var("BREVI_NO_RTLIB").map_or(false, |v| !v.is_empty() && v != "0") {
                return None;
            }
            let src = runtime_lib_source();
            let mode = if release { "release" } else { "dev-o3" };
            let dir = cache_dir().join("rt").join(format!("{:016x}-{mode}", fnv(&[&src, rustc_version()], 1)));
            let lib = dir.join("libbrevi_rt.rlib");
            if lib.exists() {
                return Some(lib);
            }
            std::fs::create_dir_all(&dir).ok()?;
            // programs built side by side wait for one build of it
            let lock = dir.join(".lock");
            let owned = build_lock(&lock, &|| lib.exists());
            let unlock = || {
                if owned {
                    std::fs::remove_file(&lock).ok();
                }
            };
            if lib.exists() {
                unlock();
                return Some(lib);
            }
            let tmp = dir.join(format!("build-{}", std::process::id()));
            if std::fs::create_dir_all(&tmp).is_err() || std::fs::write(tmp.join("rt.rs"), &src).is_err() {
                unlock();
                return None;
            }
            let mut c = rustc_cmd();
            c.args(["--edition", "2021", "--crate-type", "rlib", "--crate-name", "brevi_rt", "-A", "warnings", "-C", "overflow-checks=on"]);
            // Built once, so always optimised: programs built without --release call into it too.
            c.args(["-C", "opt-level=3"]);
            c.arg("-o").arg(tmp.join("libbrevi_rt.rlib")).arg(tmp.join("rt.rs"));
            let ok = c.output().map_or(false, |o| o.status.success());
            let moved = ok && std::fs::rename(tmp.join("libbrevi_rt.rlib"), &lib).is_ok();
            std::fs::remove_dir_all(&tmp).ok();
            unlock();
            if moved || lib.exists() { Some(lib) } else { None }
        })
        .clone()
}

fn runtime_lib_source() -> String {
    let mut out = String::with_capacity(emit::PRELUDE.len() + 4096);
    for l in emit::PRELUDE.lines() {
        let item = ["fn ", "struct ", "enum ", "trait ", "type ", "const ", "static ", "use ", "mod "].iter().any(|k| l.starts_with(k));
        if item {
            out.push_str("pub ");
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

// `rustc -vV`, asked once per run. Part of every cache key, so a new toolchain never reuses old builds.
pub(crate) fn rustc_version() -> &'static str {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| rustc_cmd().arg("-vV").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default())
}

// rustc itself rather than the rustup proxy on PATH: each call through the proxy costs 20 to 50 ms, and a
// build makes two or three. The toolchain is still the one the proxy picks. Its answer is remembered in the
// cache under a key of everything rustup decides by (PATH, RUSTUP_* settings, rustup's settings file and any
// rust-toolchain file above the working directory), so a toolchain switch asks the proxy again.
pub(crate) fn rustc_cmd() -> Command {
    static P: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    Command::new(P.get_or_init(|| {
        let saved = toolchain_key().map(|k| cache_dir().join("toolchain").join(k));
        if let Some(p) = saved.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).map(PathBuf::from) {
            if p.is_file() {
                return p;
            }
        }
        let real = Command::new("rustc").args(["--print", "sysroot"]).output().ok().filter(|o| o.status.success()).map(|o| {
            PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()).join("bin").join(format!("rustc{}", std::env::consts::EXE_SUFFIX))
        });
        match real.filter(|p| p.is_file()) {
            Some(p) => {
                if let Some(f) = saved {
                    let tmp = f.with_extension(format!("tmp{}", std::process::id()));
                    let ok = std::fs::create_dir_all(f.parent().unwrap()).is_ok() && std::fs::write(&tmp, p.to_string_lossy().as_bytes()).is_ok();
                    if !(ok && std::fs::rename(&tmp, &f).is_ok()) {
                        std::fs::remove_file(&tmp).ok();
                    }
                }
                p
            }
            None => PathBuf::from("rustc"),
        }
    }))
}

// What rustup's choice of toolchain depends on, hashed. None (always ask the proxy) without a working directory.
pub(crate) fn toolchain_key() -> Option<String> {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).map_or(String::from("-"), |t| format!("{t:?}"));
    let mut parts: Vec<String> = ["PATH", "RUSTUP_TOOLCHAIN", "RUSTUP_HOME", "CARGO_HOME", "HOME"].iter().map(|v| std::env::var(v).unwrap_or_default()).collect();
    let home = std::env::var("RUSTUP_HOME").map(PathBuf::from).ok().or_else(|| std::env::var("HOME").ok().map(|h| Path::new(&h).join(".rustup")));
    if let Some(h) = home {
        parts.push(mtime(&h.join("settings.toml")));
    }
    let cwd = std::env::current_dir().ok()?;
    parts.push(cwd.to_string_lossy().to_string());
    for d in cwd.ancestors() {
        for f in ["rust-toolchain", "rust-toolchain.toml"] {
            parts.push(mtime(&d.join(f)));
        }
    }
    let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    Some(format!("{:016x}", fnv(&refs, 11)))
}

// Codegen units for builds without --release: one per core (4 to 16). rustc's default of 16 spends more time
// splitting and re-joining (ThinLTO) a small program than it saves on a machine with few cores: on 4 cores
// the 12 comparison programs build about 20% faster, and run as fast.
pub(crate) fn codegen_units() -> &'static str {
    static C: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let n = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(4, 16);
        format!("codegen-units={n}")
    })
}

pub(crate) fn fnv(parts: &[&str], seed: u64) -> u64 {
    let mut h: u64 = 1469598103934665603 ^ seed.wrapping_mul(0x9e3779b97f4a7c15);
    for p in parts {
        for b in p.bytes().chain([0xff]) {
            h = (h ^ b as u64).wrapping_mul(1099511628211);
        }
    }
    h
}

// Copies a cached program into place: a hard link when possible, else a copy (written aside, then renamed).
pub(crate) fn copy_bin(from: &Path, to: &Path) -> bool {
    std::fs::remove_file(to).ok();
    if std::fs::hard_link(from, to).is_ok() {
        return true;
    }
    let tmp = to.with_extension(format!("tmp{}", std::process::id()));
    std::fs::copy(from, &tmp).is_ok() && std::fs::rename(&tmp, to).is_ok()
}

pub(crate) fn touch(p: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(p) {
        f.set_modified(std::time::SystemTime::now()).ok();
    }
}

fn report(rust: &str, stderr: &str) {
    let line_map = brevi_lines(rust);
    let mut seen = std::collections::HashSet::new();
    let errs: Vec<json::J> = stderr
        .lines()
        .filter_map(json::parse)
        .filter(|j| j.get("level").and_then(|v| v.str()) == Some("error"))
        .filter(|j| !j.get("message").and_then(|v| v.str()).unwrap_or("").starts_with("aborting due to"))
        .collect();
    let code_of = |j: &json::J| j.get("code").and_then(|c| c.get("code")).and_then(|c| c.str()).unwrap_or("").to_string();
    // "type annotations needed" (E0282/E0283) is usually a knock-on of a real error elsewhere,
    // e.g. a missing trait on a value; show it only when nothing else went wrong.
    let vague = |c: &str| c == "E0282" || c == "E0283";
    let has_real = errs.iter().any(|j| !vague(&code_of(j)));
    for j in &errs {
        let code = code_of(j);
        if has_real && vague(&code) {
            continue;
        }
        let code = code.as_str();
        let msg = j.get("message").and_then(|v| v.str()).unwrap_or("");
        let spans = j.get("spans").map(|s| s.arr()).unwrap_or(&[]);
        // Spans inside the runtime library (rt.rs) say nothing about the program's lines.
        let ours = |s: &&json::J| s.get("file_name").and_then(|f| f.str()).map_or(true, |f| !f.ends_with("rt.rs"));
        let primary = spans.iter().filter(ours).find(|s| s.get("is_primary").map_or(false, |p| p.truthy())).or(spans.iter().find(ours)).or(spans.first());
        let (rline, label) = match primary {
            Some(s) => (
                s.get("line_start").and_then(|v| v.num()).unwrap_or(0.0) as usize,
                s.get("label").and_then(|v| v.str()).unwrap_or("").to_string(),
            ),
            None => (0, String::new()),
        };
        let aline = line_map.get(rline.saturating_sub(1)).copied().unwrap_or(0);
        let mut text = to_brevi_words(msg);
        if code == "E0596" || code == "E0384" {
            let name = msg.split_whitespace().skip_while(|w| *w != "borrow" && *w != "variable").nth(1).unwrap_or("it").replace('`', "");
            text = format!("cannot change {name}: it was declared with let");
        } else if code == "E0381" {
            let name = msg.split('`').nth(1).unwrap_or("it");
            text = format!("{name} is not set on every path before this use");
        } else if !label.is_empty() && !text.contains(&label) && !label.starts_with("method not found in") {
            text = format!("{text}: {}", to_brevi_words(&label));
        }
        // A copy Brevi refuses because the value is not shared (a `__shared` marker from alias.rs).
        if msg.contains("AxShared") || msg.contains("AxWhole") || msg.contains("AxAug") {
            let src = primary.and_then(|s| s.get("text")).and_then(|t| t.arr().first()).and_then(|t| t.get("text")).and_then(|t| t.str()).unwrap_or("");
            if let Some(h) = src.split("axsh:").nth(1).map(|h| h.split("*/").next().unwrap_or("")) {
                let bytes: Vec<u8> = (0..h.len() / 2).filter_map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect();
                let t = String::from_utf8_lossy(&bytes).to_string();
                let (m, f) = t.split_once('\u{1}').unwrap_or((&t, ""));
                if !seen.insert(aline) {
                    continue;
                }
                let mut d = Diag::new("check", aline, 1, m);
                d.fix = Some(f.to_string());
                print_diag(&d);
                continue;
            }
        }
        let mut fix = fix_for(code, &format!("{msg} {label}"));
        if vague(code) {
            if let Some((t, f)) = vague_type_words(primary.and_then(|s| s.get("text")).and_then(|t| t.arr().first()).and_then(|t| t.get("text")).and_then(|t| t.str()).unwrap_or("")) {
                text = t;
                fix = Some(f);
            }
        }
        if let Some(opts) = valid_options(code, msg, rust) {
            fix = Some(match fix {
                Some(_) if code == "E0599" => opts,
                Some(f) => format!("{f}; {opts}"),
                None => opts,
            });
        }
        if RUST_LINES.get().map_or(false, |rs| rs.iter().any(|&(a, b)| aline >= a && aline <= b)) {
            fix = rust_help(j, code).or(fix);
        }
        // One error per line: later ones are usually knock-on effects.
        if !seen.insert(aline) {
            continue;
        }
        let mut d = Diag::new("type", aline, 1, &text);
        d.fix = fix;
        print_diag(&d);
    }
}

// "type annotations needed" on `let mut NAME = ...`: name the variable and say why its type is unknown.
fn vague_type_words(src: &str) -> Option<(String, String)> {
    let p = src.find("let ")?;
    let rest = src[p + 4..].trim_start().trim_start_matches("mut ").trim_start();
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    let after = rest[name.len()..].trim_start();
    if name.is_empty() || !after.starts_with('=') {
        return None;
    }
    let shown = name.trim_start_matches("v_").to_string();
    let rhs = &after[1..];
    Some(if rhs.contains("ax_none()") {
        (format!("can't work out the type of {shown}, which starts as None"), format!("give it a type where it starts, e.g. {shown}: (int, str) = None, or set it to a real value first"))
    } else if rhs.contains("AxM::new()") || rhs.contains("Vec::new()") || rhs.contains("vec![]") || rhs.contains("BTreeSet::new()") {
        (format!("can't work out the type of the empty {shown}"), format!("give it a type, e.g. {shown}: {{str: int}} = {{}} or {shown}: [str] = []"))
    } else {
        (format!("can't work out the type of {shown}"), format!("give {shown} a type where it is first set, e.g. {shown}: int = ..."))
    })
}

// For each generated Rust line, the Brevi line it came from (from /*@L..*/ markers).
fn brevi_lines(rust: &str) -> Vec<usize> {
    let mut cur = 0;
    rust.lines()
        .map(|l| {
            let mut rest = l;
            while let Some(p) = rest.find("/*@L") {
                let tail = &rest[p + 4..];
                let end = tail.find("*/").unwrap_or(0);
                cur = tail[..end].parse().unwrap_or(cur);
                rest = &tail[end..];
            }
            cur
        })
        .collect()
}

// Rewrites Rust type names in rustc messages into Brevi's.
fn to_brevi_words(s: &str) -> String {
    // Programs built with cargo have the crate name brevi_<hash>.
    let mut s = s.to_string();
    let mut from = 0;
    while let Some(p) = s[from..].find("brevi_").map(|p| p + from) {
        let hex = s[p + 6..].chars().take_while(|c| c.is_ascii_hexdigit()).count();
        if hex > 0 && s[p + 6 + hex..].starts_with("::") {
            s.replace_range(p..p + 8 + hex, "");
        } else {
            from = p + 6;
        }
    }
    let s = s.as_str();
    let mut t = s
        .replace("alloc::string::String", "str")
        .replace("std::string::String", "str")
        .replace("alloc::vec::Vec", "Vec")
        .replace("std::collections::BTreeMap", "BTreeMap")
        .replace("brevi_prog::AxM", "AxM")
        .replace("brevi_prog::", "")
        .replace("brevi_rt::", "")
        .replace("found for struct ", "found for ")
        .replace(" in the current scope", "")
        .replace("&mut ", "mut ")
        .replace("`", "");
    for (r, a) in [("String", "str"), ("i64", "int"), ("f64", "f64")] {
        t = replace_word(&t, r, a);
    }
    t = rewrite_generic(&t, "Vec", |args| format!("[{}]", args));
    t = rewrite_generic(&t, "AxM", |args| format!("{{{}}}", args.replacen(", ", ": ", 1)));
    t = rewrite_generic(&t, "R", |args| format!("{args} !"));
    t = rewrite_generic(&t, "Result", |args| format!("{} !", args.trim_end_matches(", AxErr")));
    t.replace("ax_", "")
}

fn replace_word(s: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = s.chars().collect();
    let f: Vec<char> = from.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        if b[i..].starts_with(&f) && (i == 0 || !is_word(b[i - 1])) && b.get(i + f.len()).map_or(true, |c| !is_word(*c)) {
            out.push_str(to);
            i += f.len();
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn rewrite_generic(s: &str, name: &str, f: impl Fn(&str) -> String) -> String {
    let pat = format!("{name}<");
    let mut out = s.to_string();
    while let Some(p) = out.find(&pat) {
        if p > 0 && out[..p].chars().last().map_or(false, |c| c.is_alphanumeric()) {
            break;
        }
        let start = p + pat.len();
        let mut depth = 1;
        let mut end = start;
        for (i, c) in out[start..].char_indices() {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                end = start + i;
                break;
            }
        }
        if depth != 0 {
            break;
        }
        let inner = out[start..end].to_string();
        out = format!("{}{}{}", &out[..p], f(&inner), &out[end + 1..]);
    }
    out
}

// Lists what would have been valid, since errors that name the alternatives get fixed far more often.
fn valid_options(code: &str, msg: &str, rust: &str) -> Option<String> {
    const STR: &str = "len words lines chars split slice find lower upper trim contains starts_with ends_with replace repeat is_empty is_digit is_alpha is_space digit int float ints big";
    const LIST: &str = "len is_empty push pop insert remove first last slice map filter any all count fold sort sort_by rev take skip enumerate zip sum min max contains index_of join";
    const MAP: &str = "len is_empty has get_or remove keys values pairs";
    const NUM: &str = "abs float int fmt sqrt round floor ceil";
    match code {
        "E0599" => {
            let ty = msg.split("found for ").nth(1).unwrap_or("");
            let list = if ty.contains("Vec") {
                LIST
            } else if ty.contains("AxM") {
                MAP
            } else if ty.contains("String") {
                STR
            } else if ty.contains("i64") || ty.contains("f64") {
                NUM
            } else {
                return None;
            };
            Some(format!("valid methods here: {list}"))
        }
        "E0609" | "E0560" | "E0026" => {
            // "no field `z` on type `Point`": list the struct's fields from the generated code.
            let ty = msg.rsplit('`').nth(1)?;
            let src = if rust.contains(&format!("pub struct {ty} {{")) { rust } else { emit::PRELUDE };
            let start = src.find(&format!("pub struct {ty} {{"))?;
            let body = &src[start..];
            let body = &body[..body.find('}')?];
            let fields: Vec<&str> = body.lines().filter_map(|l| l.trim().strip_prefix("pub ")?.split(':').next()).filter(|f| !f.starts_with("struct")).collect();
            Some(format!("fields of {ty}: {}", fields.join(", ")))
        }
        _ => None,
    }
}

// For errors in Rust code: rustc's help lines, and a hint to pin crate versions when an API is not found.
fn rust_help(j: &json::J, code: &str) -> Option<String> {
    let mut hs: Vec<String> = vec![];
    for c in j.get("children").map(|c| c.arr()).unwrap_or(&[]) {
        let lvl = c.get("level").and_then(|v| v.str()).unwrap_or("");
        if lvl != "help" && lvl != "note" {
            continue;
        }
        let mut m = c.get("message").and_then(|v| v.str()).unwrap_or("").replace('`', "");
        // "#[deny(lint)] on by default" says nothing about how to fix the code.
        if m.starts_with("#[") && m.ends_with("on by default") {
            continue;
        }
        let reps: Vec<String> = c.get("spans").map(|s| s.arr()).unwrap_or(&[]).iter().filter_map(|s| s.get("suggested_replacement").and_then(|v| v.str()).map(|r| r.trim().to_string())).filter(|r| !r.is_empty()).collect();
        if !reps.is_empty() {
            m = format!("{m}: {}", reps.join(" | "));
        }
        if !m.is_empty() && m.len() < 300 {
            hs.push(m);
        }
        if hs.len() == 2 {
            break;
        }
    }
    let unpinned: Vec<&str> = DEPS.get().map(|d| d.iter().filter(|d| d.version == "*").map(|d| d.name.as_str()).collect()).unwrap_or_default();
    if !unpinned.is_empty() && matches!(code, "E0599" | "E0432" | "E0433" | "E0425" | "E0412" | "E0061" | "E0277" | "E0308" | "E0603") {
        hs.push(format!("{} has no version, so cargo used the newest: pin the one you know with use {} \"VERSION\"", unpinned.join(", "), unpinned[0]));
    }
    if hs.is_empty() {
        None
    } else {
        Some(hs.join("; "))
    }
}

fn fix_for(code: &str, msg: &str) -> Option<String> {
    if msg.contains("Result<") || msg.contains("AxErr") {
        return Some("this value can fail: add ? after the call that produced it".into());
    }
    if msg.contains("will panic at runtime") {
        return Some("this always panics when run: keep indexes in range, and window/chunk sizes above 0".into());
    }
    match code {
        "E0384" | "E0596" => Some("write x = 1 instead of let x = 1, or mark the parameter mut (fn f(mut xs: [int]))".into()),
        "E0609" if msg.contains("on type [") || msg.contains("on type str") => Some("read list items with xs[0], not xs.0 (that is for tuples)".into()),
        "E0381" => Some("set it before the if or match, so every path gives it a value".into()),
        "E0599" if msg.contains("ref") && msg.contains("tuple") => Some("read a tuple's items with t.0, t.1, not t[0]".into()),
        "E0308" if msg.contains("Result") || msg.contains("R<") => Some("a call that can fail needs ? after it".into()),
        "E0308" if msg.contains("mismatched") => Some("convert with .int(), .float() or str(x)".into()),
        "E0599" => Some("check the method name and the value's type".into()),
        "E0282" => Some("give the empty value a type, e.g. m: {str: int} = {} or xs: [str] = []".into()),
        "E0412" => Some("types are int f64 bool str dec big Date Json [T] {K: V} (A, B) heap[T], or a type you defined".into()),
        "E0277" if msg.contains("Plus") => Some("both sides of + must have the same type".into()),
        "E0277" if msg.contains("i64") && msg.contains("f64") => Some("convert one side with .float() or .int()".into()),
        _ => None,
    }
}

// A program written as Python (`def`, `class`, `global`) prints values and orders dicts as Python does.
// Also a script with spellings only Python code uses: `None`, `xs.append(x)` (Brevi code writes `True` too).
// Set for a .py file: Python rules apply whatever the source looks like.
// `brevi run --traceback` (or BREVI_TRACEBACK=1): a fallback to CPython shows its whole traceback.
static TRACEBACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static PY_FILE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Set when the .bv file being built is plain Python (bv_as_py).
static BV_PY: std::sync::OnceLock<String> = std::sync::OnceLock::new();

// A Python file: a .py file, or a .bv file written in plain Python.
pub fn is_py(file: &str) -> bool {
    file.ends_with(".py") || BV_PY.get().is_some_and(|f| f == file)
}

// A .bv file written as Python with no Brevi spellings: CPython could compile it (no `fn`, `=>`, `use "x.bv"`,
// foreign code), it uses no Brevi-only names (`fail(...)`, `.push(...)`), it is Python-style (so the typed
// build already gives it Python's meaning), and no plain string in it would interpolate (`"{n}"` is Brevi's
// interpolation, Python's literal text), and no function ends with a bare value (`x * 2`: Brevi returns it, Python
// drops it). Such a file gets what a .py file gets.
fn bv_as_py(file: &str) -> bool {
    if !file.ends_with(".bv") || std::env::var("BREVI_BV_PY").is_ok_and(|v| v == "0") {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(file) else { return false };
    let src = caps::split(&text).0;
    let dir = Path::new(file).parent().unwrap_or(Path::new("."));
    let foreign = src.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("use ") || t.starts_with("rust ") || t.starts_with("py fn") || t == "rust:" || t == "py:" || t.starts_with("test \"")
    });
    let ok = !foreign && python_style(&src) && pydiag::not_python(&text).is_none() && pydiag::scan_brevi(&src, dir).is_empty() && !bvpy::interpolates(&src) && !bvpy::returns_tail(&src);
    if ok {
        BV_PY.set(file.to_string()).ok();
    } else if !foreign && python_style(&src) && std::env::var("BREVI_BV_DYN").map_or(true, |v| v != "0") {
        // Python-style but with Brevi's spellings (`fn`, `x => e`, `"{n}"`): built typed as it is, and if the
        // typed build refuses it, over dynamic values from the same program in Python's spellings.
        if let Some(py) = bvpy::to_python(&src).filter(|py| python_style(py) && pydiag::not_python(py).is_none() && pydiag::scan_brevi(py, dir).is_empty() && !bvpy::interpolates(py)) {
            if std::env::var("BREVI_BV_DYN").is_ok_and(|v| v == "print") {
                eprint!("{py}");
            }
            dynpy::set_source(file, py);
        }
    }
    ok
}

pub fn python_style(src: &str) -> bool {
    if PY_FILE.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    let word = |l: &str, w: &str| {
        l.match_indices(w).any(|(i, _)| {
            let before = l[..i].chars().next_back();
            let after = l[i + w.len()..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '"' || c == '\'') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '"' || c == '\'')
        })
    };
    src.lines().map(str::trim_start).filter(|l| !l.starts_with('#')).any(|l| {
        l.starts_with("def ")
            || l.starts_with("class ")
            || l.starts_with("global ")
            || l.starts_with("nonlocal ")
            || l.starts_with("import ")
            || (l.starts_with("from ") && l.contains(" import "))
            || l.contains(".append(")
            || word(l, "None")
            || (l.starts_with("with ") && l.contains(" as "))
            || (word(l, "open") && l.contains("open("))
    })
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn build_lock_makes_its_folder() {
        // a fresh cache: the lock's folder does not exist yet, and the lock must still be taken (it used to wait forever)
        let dir = std::env::temp_dir().join(format!("brevi-lock-test-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let lock = dir.join("bin").join(".tmp-lib-x.lock");
        assert!(build_lock(&lock, &|| false));
        assert!(lock.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enclosing_def_names_the_function() {
        let src = "import sys\n\nclass Board:\n    def move(self, n):\n        xs = [1]\n        # note\n\n        return xs\n\ndef main():\n    def inner():\n        return 1\n    return inner()\nprint(main())\n";
        assert_eq!(enclosing_def(src, 1), None);
        assert_eq!(enclosing_def(src, 3), None);
        assert_eq!(enclosing_def(src, 4).as_deref(), Some("Board.move"));
        assert_eq!(enclosing_def(src, 5).as_deref(), Some("Board.move"));
        assert_eq!(enclosing_def(src, 8).as_deref(), Some("Board.move"));
        assert_eq!(enclosing_def(src, 10).as_deref(), Some("main"));
        assert_eq!(enclosing_def(src, 12).as_deref(), Some("main.inner"));
        assert_eq!(enclosing_def(src, 13).as_deref(), Some("main"));
        assert_eq!(enclosing_def(src, 14), None);
        assert_eq!(enclosing_def(src, 0), None);
        assert_eq!(enclosing_def(src, 99), None);
        assert_eq!(enclosing_def("fn total(xs: [int]) -> int:\n    return 0\n", 2).as_deref(), Some("total"));
    }

    #[test]
    fn rustc_first_error_reads_json_and_text() {
        let j = r#"{"$message_type":"diagnostic","message":"mismatched types","level":"error"}"#;
        assert_eq!(rustc_first_error(&format!("{{\"level\":\"warning\",\"message\":\"unused\"}}\n{j}")), "the typed build's Rust did not compile (mismatched types)");
        assert_eq!(rustc_first_error("error[E0308]: mismatched types\n"), "the typed build's Rust did not compile (mismatched types)");
        assert_eq!(rustc_first_error(""), "the typed build's Rust did not compile");
    }
}
