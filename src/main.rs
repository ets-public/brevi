mod ast;
mod diag;
mod emit;
mod json;
mod lexer;
mod moves;
mod infer;
mod parser;

use diag::{esc, Diag};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "usage: brevi <run|build|test|check|fix|emit> FILE.bv [--release] [-o OUT] [-- ARGS...]";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, file) = match (argv.first(), argv.get(1)) {
        (Some(c), Some(f)) => (c.as_str(), f.clone()),
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    };
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
    let (src, fixed) = autofix(&src, cmd == "fix");
    for (line, old, new) in &fixed {
        eprintln!("{{\"line\":{line},\"fixed\":\"{} -> {}\"}}", esc(old), esc(new));
    }
    if cmd == "fix" {
        if !fixed.is_empty() {
            if let Err(e) = std::fs::write(&file, &src) {
                println!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&e.to_string()));
                exit(1);
            }
        }
    }
    // Builds to try in order: with inferred types for empty values, then without (the errors shown come from the last).
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
                println!("{}", d.to_json());
            }
            exit(1);
        }
    };

    match cmd {
        "emit" => print!("{rust}"),
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

pub fn compile_to_rust(src: &str, moves: bool, hints: bool) -> Result<String, Vec<Diag>> {
    let mut items = parser::parse_program(src).map_err(|d| vec![d])?;
    infer::returns(&mut items)?;
    let errs = infer::check(&mut items);
    if !errs.is_empty() {
        return Err(errs);
    }
    emit::auto_try(&mut items);
    let mut em = emit::Emitter::new(&items, hints);
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
    let mut last = None;
    for (rust, plain) in versions {
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
        last = Some(r);
    }
    match last {
        Some((rust, stderr)) => report(rust, &stderr),
        None => {}
    }
    false
}

// None when rustc could not be started.
fn compile(rust: &str, dir: &Path, out: &Path, extra: &[&str], link: bool) -> Option<Result<(), String>> {
    let rs = dir.join("main.rs");
    std::fs::write(&rs, rust).expect("write generated rust");
    let mut c = Command::new("rustc");
    c.args(["--edition", "2021", "--error-format=json", "--crate-name", "brevi_prog", "-A", "warnings", "-C", "overflow-checks=on"]);
    c.args(extra);
    if link {
        c.arg("-o").arg(out);
    } else {
        c.arg("--out-dir").arg(dir);
    }
    c.arg(&rs);
    match c.output() {
        Ok(o) if o.status.success() => Some(Ok(())),
        Ok(o) => Some(Err(String::from_utf8_lossy(&o.stderr).to_string())),
        Err(e) => {
            println!("{{\"e\":\"io\",\"msg\":\"rustc not found: {}\"}}", esc(&e.to_string()));
            None
        }
    }
}

fn report(rust: &str, stderr: &str) {
    let line_map = brevi_lines(rust);
    let mut seen = std::collections::HashSet::new();
    for l in stderr.lines() {
        let Some(j) = json::parse(l) else { continue };
        if j.get("level").and_then(|v| v.str()) != Some("error") {
            continue;
        }
        let msg = j.get("message").and_then(|v| v.str()).unwrap_or("");
        if msg.starts_with("aborting due to") {
            continue;
        }
        let code = j.get("code").and_then(|c| c.get("code")).and_then(|c| c.str()).unwrap_or("");
        let spans = j.get("spans").map(|s| s.arr()).unwrap_or(&[]);
        let primary = spans.iter().find(|s| s.get("is_primary").map_or(false, |p| p.truthy())).or(spans.first());
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
        } else if !label.is_empty() && !text.contains(&label) {
            text = format!("{text}: {}", to_brevi_words(&label));
        }
        let mut fix = fix_for(code, &format!("{msg} {label}"));
        if let Some(opts) = valid_options(code, msg, rust) {
            fix = Some(match fix {
                Some(_) if code == "E0599" => opts,
                Some(f) => format!("{f}; {opts}"),
                None => opts,
            });
        }
        // One error per line: later ones are usually knock-on effects.
        if !seen.insert(aline) {
            continue;
        }
        let mut d = Diag::new("type", aline, 1, &text);
        d.fix = fix;
        println!("{}", d.to_json());
    }
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
    let mut t = s
        .replace("alloc::string::String", "str")
        .replace("std::string::String", "str")
        .replace("alloc::vec::Vec", "Vec")
        .replace("std::collections::BTreeMap", "BTreeMap")
        .replace("brevi_prog::AxM", "AxM")
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
            let start = rust.find(&format!("pub struct {ty} {{"))?;
            let body = &rust[start..];
            let body = &body[..body.find('}')?];
            let fields: Vec<&str> = body.lines().filter_map(|l| l.trim().strip_prefix("pub ")?.split(':').next()).filter(|f| !f.starts_with("struct")).collect();
            Some(format!("fields of {ty}: {}", fields.join(", ")))
        }
        _ => None,
    }
}

fn fix_for(code: &str, msg: &str) -> Option<String> {
    if msg.contains("Result<") || msg.contains("AxErr") {
        return Some("this value can fail: add ? after the call that produced it".into());
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
