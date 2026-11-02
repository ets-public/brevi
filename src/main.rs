mod alias;
mod records;
mod ast;
mod bigint;
mod caps;
mod diag;
mod emit;
mod ffi;
mod gui;
mod json;
mod lexer;
mod moves;
mod infer;
mod mcp;
mod modules;
mod parser;
mod pybridge;
mod patch;

use diag::{esc, Diag};
use std::path::{Path, PathBuf};
use std::io::Write;
use std::process::{exit, Command};

const USAGE: &str = "usage: brevi <run|build|test|check|fix|emit> FILE.bv [--release] [--fast] [--native] [--sandbox] [--window] [--explain] [-o OUT] [-- ARGS...] | brevi guide [SECTION|WORD] | brevi patch FILE.bv PATCH.bv | brevi mcp";

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

fn main() {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(|a| a.as_str()) == Some("mcp") {
        mcp::serve();
        return;
    }
    if matches!(argv.first().map(|a| a.as_str()), Some("--version" | "version")) {
        println!("brevi {}", env!("CARGO_PKG_VERSION"));
        return;
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
    let sandbox = argv.iter().position(|a| a == "--sandbox").map(|i| argv.remove(i)).is_some();
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
    // `brevi run x.py` falls back to CPython when Brevi cannot build the file; --native (or BREVI_NATIVE=1) keeps it strict.
    if file.ends_with(".py") {
        PY_FILE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if cmd == "run" && file.ends_with(".py") && !native {
        run_or_fall_back(&file, &argv);
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
    let cap_errs = if sandbox || allowed.is_some() { [cap_errs, caps::check_src(&src, &allowed.unwrap_or_default())].concat() } else { cap_errs };
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
    let (src, fixed) = autofix(&src, cmd == "fix");
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
    // Builds to try in order: with inferred types for empty values, then without (the errors shown come from the first).
    let versions: Vec<(String, Option<String>)> = [true, false]
        .iter()
        .filter_map(|&h| compile_to_rust(&src, true, h).ok().map(|r| (r, compile_to_rust(&src, false, h).ok())))
        .fold(vec![], |mut vs, v| {
            if !vs.iter().any(|(r, _)| *r == v.0) {
                vs.push(v);
            }
            vs
        });
    let rust = match compile_to_rust(&src, true, true) {
        Ok(r) => r,
        Err(diags) => {
            for d in diags {
                print_diag(&d);
            }
            exit(1);
        }
    };

    match cmd {
        "emit" => print!("{rust}"),
        "check" | "fix" if fast => {}
        "check" | "fix" => {
            // Front-end checks passed; also run rustc's checks without producing a binary.
            let dir = build_dir(&file);
            if !rustc(&versions, &dir, &dir.join("check"), &["--emit=metadata"], false) {
                exit(1);
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
            if release {
                extra.extend(["-C", "opt-level=3"]);
            }
            if !rustc(&versions, &dir, &bin, &extra, true) {
                exit(1);
            }
            if cmd == "build" {
                return;
            }
            let mut c = Command::new(&bin);
            if cmd == "test" {
                c.args(["--quiet", "--test-threads=1"]);
            } else {
                c.args(&prog_args);
            }
            let status = c.status().unwrap_or_else(|e| {
                println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                exit(1);
            });
            exit(status.code().unwrap_or(1));
        }
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    }
}

// `brevi run x.py`: builds with Brevi and runs the program; when the build fails, runs the file with python3
// instead and says so in one line on stderr. Never returns.
fn run_or_fall_back(file: &str, argv: &[String]) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let prog_args = argv.get(dash + 1..).unwrap_or(&[]).to_vec();
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy().to_string();
    let bin = build_dir(file).join(&stem);
    let flags: Vec<&String> = argv[2..dash].iter().filter(|a| matches!(a.as_str(), "--release" | "--sandbox" | "--window" | "--explain")).collect();
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    let built = Command::new(&me).arg("build").arg(file).arg("-o").arg(&bin).args(&flags).env("BREVI_NATIVE", "1").output();
    let code = match built {
        Ok(o) if o.status.success() => {
            std::io::stderr().write_all(&o.stderr).ok();
            Command::new(&bin).args(&prog_args).status().map(|s| s.code().unwrap_or(1)).unwrap_or_else(|e| {
                println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                1
            })
        }
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let python = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
            match Command::new(&python).arg(file).args(&prog_args).spawn() {
                Ok(mut child) => {
                    eprintln!("note: ran with {python}; Brevi could not build {}", first_error(&out));
                    child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1)
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

// "line N: msg" from the first JSON error line of a failed build (or "it" when there is none).
fn first_error(out: &str) -> String {
    let field = |l: &str, k: &str| -> Option<String> {
        let i = l.find(&format!("\"{k}\":"))? + k.len() + 3;
        let rest = &l[i..];
        if let Some(r) = rest.strip_prefix('"') {
            let mut v = String::new();
            let mut esc = false;
            for c in r.chars() {
                match (esc, c) {
                    (true, 'n') => v.push(' '),
                    (true, c) => v.push(c),
                    (false, '\\') => {
                        esc = true;
                        continue;
                    }
                    (false, '"') => break,
                    (false, c) => v.push(c),
                }
                esc = false;
            }
            Some(v)
        } else {
            Some(rest.chars().take_while(|c| c.is_ascii_digit()).collect())
        }
    };
    for l in out.lines().filter(|l| l.starts_with('{')) {
        if let Some(msg) = field(l, "msg") {
            return match field(l, "line") {
                Some(n) if !n.is_empty() => format!("line {n}: {msg}"),
                _ => msg,
            };
        }
    }
    "it".into()
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
            infer::auto_mut(&mut copy);
            let _ = infer::returns(&mut copy);
            let (_, mixed) = infer::empty_types(&copy);
            if !mixed.is_empty() {
                records::records(&mut items, &mixed);
            }
        }
    }
    infer::auto_mut(&mut items);
    infer::returns(&mut items)?;
    // Again: returns can turn `obj.m(x)` into a call of the class method `C_m(obj, x)`, which may change obj.
    infer::auto_mut(&mut items);
    infer::rows_reset();
    let errs = infer::check(&mut items, py);
    if !errs.is_empty() {
        return Err(errs);
    }
    // A tuple type used both for mixed-list rows and for real tuples: rows print as tuples then.
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

pub fn compile_to_rust(src: &str, moves: bool, hints: bool) -> Result<String, Vec<Diag>> {
    // pybridge: a function of a Python module that Brevi lacks (math.gcd, itertools.combinations) is run by
    // CPython; build again with it.
    let mut k = 0;
    loop {
        let r = compile_once(src, moves, hints);
        k += 1;
        match &r {
            Err(ds) if k < 8 && python_style(src) && pybridge::learn(ds, src) => continue,
            _ => return r,
        }
    }
}

fn compile_once(src: &str, moves: bool, hints: bool) -> Result<String, Vec<Diag>> {
    let (src, mut foreign) = ffi::split(src);
    let mut items = parser::parse_program(&src).map_err(|d| vec![lib_hint(d, &src)])?;
    if gui::active() {
        foreign.rust_top.push_str(&gui::runtime());
    }
    // pybridge: CPython for the Python libraries the program uses.
    if pybridge::active() {
        pybridge::python_ok().map_err(|m| vec![Diag::new("check", 1, 1, &m).fix_hint("run the program with python3")])?;
        foreign.rust_top.push_str(&pybridge::runtime());
    }
    ffi::attach(&mut items, &foreign);
    used_consts_first(&mut items);
    if python_style(&src) {
        bigint::promote(&mut items);
    }
    // Python names share objects (`r = rows[k]; r[0] -= 1` changes rows); Brevi programs keep copies.
    if python_style(&src) {
        let errs = alias::writeback(&mut items);
        if !errs.is_empty() {
            return Err(errs);
        }
    }
    let py = python_style(&src);
    infer::PY.store(py, std::sync::atomic::Ordering::Relaxed);
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
    if em.diags.is_empty() {
        Ok(rust)
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

fn build_dir(file: &str) -> PathBuf {
    let abs = std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file));
    let mut h: u64 = 1469598103934665603;
    for b in abs.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    let dir = std::env::temp_dir().join(format!("brevi-{h:x}"));
    std::fs::create_dir_all(&dir).ok();
    dir
}

// Compiles generated Rust. rustc errors are mapped back to Brevi lines and printed as JSON.
// Each version is (program, the same program without moves). The plain one is used if moving a value broke
// the borrow rules. Errors are reported from the last version tried.
fn rustc(versions: &[(String, Option<String>)], dir: &Path, out: &Path, extra: &[&str], link: bool) -> bool {
    const MOVE_CODES: [&str; 8] = ["E0382", "E0505", "E0507", "E0502", "E0499", "E0506", "E0716", "E0597"];
    let release = extra.contains(&"opt-level=3");
    let lib: Vec<(String, Option<String>)> = versions.iter().map(|(r, p)| (use_runtime_lib(r, release), p.as_ref().map(|p| use_runtime_lib(p, release)))).collect();
    // A program that implements a prelude trait for a prelude type cannot use the library: paste the prelude.
    let orphan = |e: &str| ["E0117", "E0116", "E0210"].iter().any(|c| e.contains(&format!("\"code\":\"{c}\"")));
    let versions = match lib.first() {
        Some((r, _)) if r.starts_with(RT_HEADER) && matches!(compile(r, dir, out, extra, link), Some(Err(e)) if orphan(&e)) => versions.to_vec(),
        _ => lib,
    };
    let mut last = None;
    for (rust, plain) in &versions {
        let mut r = (rust.as_str(), match compile(rust, dir, out, extra, link) {
            Some(Ok(())) => return true,
            Some(Err(e)) => e,
            None => return false,
        });
        if let Some(p) = plain {
            if MOVE_CODES.iter().any(|c| r.1.contains(&format!("\"code\":\"{c}\""))) {
                r = (p.as_str(), match compile(p, dir, out, extra, link) {
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
        Some((rust, stderr)) => report(rust, &stderr),
        None => {}
    }
    false
}

// Crates the program uses (`use` lines, and crate paths in rust fns). With any, builds go through cargo.
static DEPS: std::sync::OnceLock<Vec<ffi::Dep>> = std::sync::OnceLock::new();
// Source lines written in Rust (rust fn and rust: blocks): their errors keep rustc's own help.
static RUST_LINES: std::sync::OnceLock<Vec<(usize, usize)>> = std::sync::OnceLock::new();

// Where crates are built once and shared by every program: $BREVI_CACHE, else ~/.cache/brevi.
fn cache_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BREVI_CACHE") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    home.join(".cache").join("brevi")
}

// Builds with cargo, for programs that use crates. Same results as compile().
fn cargo_compile(deps: &[ffi::Dep], rust: &str, dir: &Path, out: &Path, extra: &[&str], link: bool) -> Option<Result<(), String>> {
    let name = dir.file_name().map(|n| n.to_string_lossy().replace('-', "_")).unwrap_or_else(|| "brevi_prog".into());
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
// Builds are cached by a hash of the generated code, the flags and the rustc version, so running the same
// program again (an agent re-running its tests, or an MCP call) skips rustc. BREVI_NO_CACHE=1 turns this off.
fn compile(rust: &str, dir: &Path, out: &Path, extra: &[&str], link: bool) -> Option<Result<(), String>> {
    if let Some(deps) = DEPS.get().filter(|d| !d.is_empty()) {
        return cargo_compile(deps, rust, dir, out, extra, link);
    }
    let release = extra.contains(&"opt-level=3");
    let rt = if rust.starts_with(RT_HEADER) { runtime_lib(release) } else { None };
    let use_cache = std::env::var("BREVI_NO_CACHE").map_or(true, |v| v.is_empty() || v == "0");
    // The prelude is linked from a prebuilt library rather than pasted in, so its own hash is part of the key:
    // a changed runtime (a brevi upgrade) must not reuse a binary or error built against the old one.
    static PRELUDE_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let ph = PRELUDE_HASH.get_or_init(|| format!("{:016x}", fnv(&[emit::PRELUDE, env!("CARGO_PKG_VERSION")], 3)));
    let key = format!("{:016x}{:016x}", fnv(&[rust, &extra.join(" "), if link { "link" } else { "meta" }, rustc_version(), ph], 0), fnv(&[rust, &extra.join(" "), rustc_version(), ph], 7));
    let cdir = cache_dir().join("bin");
    let cached = cdir.join(&key);
    if use_cache {
        if let Ok(e) = std::fs::read_to_string(cached.with_extension("err")) {
            return Some(Err(e));
        }
        if cached.exists() && (!link || copy_bin(&cached, out)) {
            touch(&cached);
            return Some(Ok(()));
        }
    }
    let rs = dir.join("main.rs");
    std::fs::write(&rs, rust).expect("write generated rust");
    let mut c = Command::new("rustc");
    c.args(["--edition", "2021", "--error-format=json", "--crate-name", "brevi_prog", "-A", "warnings", "-C", "overflow-checks=on"]);
    c.args(extra);
    if let Some(lib) = &rt {
        c.arg("--extern").arg(format!("brevi_rt={}", lib.display()));
    }
    if link {
        // Debug info is never used (runtime errors carry Brevi line numbers), and it is most of the file.
        c.args(["-C", "strip=debuginfo"]);
        c.arg("-o").arg(out);
    } else {
        c.arg("--out-dir").arg(dir);
    }
    c.arg(&rs);
    let r = match c.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => {
            println!("{{\"e\":\"io\",\"msg\":\"rustc not found: {}\"}}", esc(&e.to_string()));
            return None;
        }
    };
    if use_cache && std::fs::create_dir_all(&cdir).is_ok() {
        let tmp = cdir.join(format!("{key}.{}", std::process::id()));
        let saved = match &r {
            Ok(()) if link => std::fs::copy(out, &tmp).is_ok(),
            Ok(()) => std::fs::write(&tmp, "").is_ok(),
            Err(e) if e.contains("\"level\":\"error\"") => std::fs::write(&tmp, e).is_ok(),
            Err(_) => false,
        };
        let dest = if r.is_ok() { cached.clone() } else { cached.with_extension("err") };
        if saved && std::fs::rename(&tmp, &dest).is_ok() {
            prune_cache(&cdir);
        } else {
            std::fs::remove_file(&tmp).ok();
        }
    }
    Some(r)
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
        Some(rest) if DEPS.get().map_or(true, |d| d.is_empty()) && !rest.contains("trait AxFromJson") && runtime_lib(release).is_some() => format!("{RT_HEADER}{rest}"),
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
            let mode = if release { "release" } else { "debug" };
            let dir = cache_dir().join("rt").join(format!("{:016x}-{mode}", fnv(&[&src, rustc_version()], 1)));
            let lib = dir.join("libbrevi_rt.rlib");
            if lib.exists() {
                return Some(lib);
            }
            std::fs::create_dir_all(&dir).ok()?;
            let tmp = dir.join(format!("build-{}", std::process::id()));
            std::fs::create_dir_all(&tmp).ok()?;
            std::fs::write(tmp.join("rt.rs"), &src).ok()?;
            let mut c = Command::new("rustc");
            c.args(["--edition", "2021", "--crate-type", "rlib", "--crate-name", "brevi_rt", "-A", "warnings", "-C", "overflow-checks=on"]);
            if release {
                c.args(["-C", "opt-level=3"]);
            }
            c.arg("-o").arg(tmp.join("libbrevi_rt.rlib")).arg(tmp.join("rt.rs"));
            let ok = c.output().map_or(false, |o| o.status.success());
            let moved = ok && std::fs::rename(tmp.join("libbrevi_rt.rlib"), &lib).is_ok();
            std::fs::remove_dir_all(&tmp).ok();
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
fn rustc_version() -> &'static str {
    static V: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        Command::new("rustc").arg("-vV").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
    })
}

fn fnv(parts: &[&str], seed: u64) -> u64 {
    let mut h: u64 = 1469598103934665603 ^ seed.wrapping_mul(0x9e3779b97f4a7c15);
    for p in parts {
        for b in p.bytes().chain([0xff]) {
            h = (h ^ b as u64).wrapping_mul(1099511628211);
        }
    }
    h
}

// Copies a cached program into place: a hard link when possible, else a copy (written aside, then renamed).
fn copy_bin(from: &Path, to: &Path) -> bool {
    std::fs::remove_file(to).ok();
    if std::fs::hard_link(from, to).is_ok() {
        return true;
    }
    let tmp = to.with_extension(format!("tmp{}", std::process::id()));
    std::fs::copy(from, &tmp).is_ok() && std::fs::rename(&tmp, to).is_ok()
}

fn touch(p: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(p) {
        f.set_modified(std::time::SystemTime::now()).ok();
    }
}

// Keeps the build cache small: past 400 entries, the least recently used half goes.
fn prune_cache(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if es.len() <= 400 {
        return;
    }
    es.sort();
    for (_, p) in es.iter().take(es.len() - 200) {
        std::fs::remove_file(p).ok();
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
        if msg.contains("AxShared") || msg.contains("AxWhole") {
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
pub static PY_FILE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
