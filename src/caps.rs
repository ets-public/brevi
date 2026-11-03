// Declared capabilities: `allow read, write` at the top of a program says what it may touch.
// `brevi run --sandbox` (and the MCP tools, by default) refuse a program that uses anything it did not declare.
// A program with an `allow` line is held to it on every command, sandbox or not.
//
//   read    read(path)
//   write   write(path, text)
//   native  rust fn, py fn, rust: and py: blocks, crates (use NAME "VER"), and Python libraries CPython runs
//           (import os, import numpy: pybridge.rs): these can do anything
//
// A capability refusal is final: main.rs checks before any build, so a refused program is never built over
// dynamic values, by pybridge or run by python3 instead. A held program (--sandbox or an allow line) never falls
// back to python3 at all, since CPython cannot hold it to its allow lines. Dynamic builds (src/dynpy) have no
// file, network, environment or library access, so a program that passes this check may be built that way.
//
// stdin, stdout, args() and `use "file.bv"` are always allowed. Brevi's own library has no other way to reach
// files, the network or the environment, so checking these calls covers everything outside native code.
use crate::ast::{Expr, Item, Stmt, StrPart};
use crate::diag::Diag;

pub const CAPS: &[&str] = &["read", "write", "native"];

// Removes `allow ...` lines (keeping line numbers) and returns the capabilities they name.
// None: no allow line. A misspelt capability is an error.
pub fn split(src: &str) -> (String, Option<Vec<String>>, Vec<Diag>) {
    let mut allowed: Option<Vec<String>> = None;
    let mut errs = Vec::new();
    let mut out = Vec::new();
    for (i, l) in src.split('\n').enumerate() {
        let rest = l.strip_prefix("allow ").filter(|r| !r.trim_start().starts_with(['=', '.', '(', '[', '+', '-']));
        // `# allow read` keeps a .py file valid Python. Only a comment of nothing but capabilities counts
        // (`# allow me to explain` stays a comment), and the line is left as it is.
        if rest.is_none() {
            if let Some(list) = comment_caps(l) {
                allowed.get_or_insert_with(Vec::new).extend(list);
            }
            out.push(l);
            continue;
        }
        let Some(rest) = rest else { unreachable!() };
        let list = allowed.get_or_insert_with(Vec::new);
        let rest = rest.split('#').next().unwrap_or("");
        for c in rest.split([',', ' ']).map(str::trim).filter(|c| !c.is_empty()) {
            if CAPS.contains(&c) || c == "none" {
                list.push(c.to_string());
            } else {
                errs.push(Diag::new("caps", i + 1, 1, &format!("unknown capability '{c}'")).fix_hint("allow takes read, write, native or none"));
            }
        }
        out.push("");
    }
    (out.join("\n"), allowed, errs)
}

// The capabilities of a `# allow read, write` comment line, or None when it is any other line or comment.
fn comment_caps(l: &str) -> Option<Vec<String>> {
    let rest = l.trim_end().strip_prefix('#')?.trim_start().strip_prefix("allow ")?;
    let words: Vec<&str> = rest.split([',', ' ']).map(str::trim).filter(|c| !c.is_empty()).collect();
    if words.is_empty() || !words.iter().all(|c| CAPS.contains(c) || *c == "none") {
        return None;
    }
    Some(words.iter().map(|c| c.to_string()).collect())
}

// How to write the allow line in the file being built: `# allow X` in a .py file (still valid Python), the bare
// `allow X` in a .bv file.
pub fn allow_line(cap: &str) -> String {
    if crate::PY_FILE.load(std::sync::atomic::Ordering::Relaxed) {
        format!("# allow {cap}")
    } else {
        format!("allow {cap}")
    }
}

// Python's os, os.path and pathlib reach files, processes and the environment; their path functions do not.
// Without `allow native` a program may use only these, and random only as far as Brevi's own copy goes (all
// built from src/dynpy/py, see dynpy::sandbox_module).
const PURE_OS: &[&str] = &["path", "sep", "curdir", "pardir", "extsep", "altsep", "linesep", "name"];
const PURE_OS_PATH: &[&str] = &["join", "basename", "dirname", "split", "splitext", "normpath", "normcase", "isabs", "commonprefix", "sep", "curdir", "pardir", "extsep", "altsep"];
const PURE_PATHLIB: &[&str] = &["PurePath", "PurePosixPath"];
// What src/dynpy/py/random.py has (CPython's generator, so seeded draws match).
const OWN_RANDOM: &[&str] = &["Random", "seed", "random", "getrandbits", "randrange", "randint", "choice", "choices", "shuffle", "sample", "uniform", "gauss"];

// The module os, os.path (posixpath) or pathlib a name or dotted module stands for.
fn path_module(m: &str) -> Option<&'static str> {
    match m {
        "os" => Some("os"),
        "os.path" | "posixpath" => Some("os.path"),
        "pathlib" => Some("pathlib"),
        "random" => Some("random"),
        "io" => Some("io"),
        _ => None,
    }
}

fn pure_attr(module: &str, attr: &str) -> bool {
    match module {
        "os" => PURE_OS.contains(&attr),
        "os.path" => PURE_OS_PATH.contains(&attr),
        "random" => OWN_RANDOM.contains(&attr),
        "io" => attr == "StringIO",
        _ => PURE_PATHLIB.contains(&attr),
    }
}

// The program's lines with comments and the text of plain strings blanked (f-string text is kept: it may hold
// code), joined into logical lines: (first line number, text).
fn code_lines(src: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = vec![];
    let mut cur = String::new();
    let mut start = 1;
    let mut depth = 0i32;
    // Open string: its quote and whether its text is kept.
    let mut open: Option<(String, bool)> = None;
    for (i, l) in src.split('\n').enumerate() {
        if cur.is_empty() && open.is_none() {
            start = i + 1;
        }
        let cs: Vec<char> = l.chars().collect();
        let mut k = 0;
        let mut line = String::new();
        while k < cs.len() {
            let c = cs[k];
            if let Some((q, keep)) = &open {
                let ql: Vec<char> = q.chars().collect();
                if c == '\\' {
                    line.push_str(if *keep { "\\" } else { " " });
                    if k + 1 < cs.len() {
                        line.push(if *keep { cs[k + 1] } else { ' ' });
                    }
                    k += 2;
                    continue;
                }
                if cs[k..].starts_with(&ql) {
                    line.push_str(q);
                    k += ql.len();
                    open = None;
                    continue;
                }
                line.push(if *keep { c } else { ' ' });
                k += 1;
                continue;
            }
            match c {
                '#' => break,
                '\'' | '"' => {
                    let triple = cs[k..].starts_with(&[c, c, c]);
                    let q: String = if triple { [c, c, c].iter().collect() } else { c.to_string() };
                    // A prefix just before: f / rf / fr keep the text.
                    let mut j = k;
                    while j > 0 && cs[j - 1].is_ascii_alphabetic() {
                        j -= 1;
                    }
                    let prefix: String = cs[j..k].iter().collect::<String>().to_ascii_lowercase();
                    let keep = prefix.contains('f') && prefix.len() <= 2;
                    line.push_str(&q);
                    k += q.len();
                    open = Some((q, keep));
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    line.push(c);
                    k += 1;
                }
                ')' | ']' | '}' => {
                    depth -= 1;
                    line.push(c);
                    k += 1;
                }
                _ => {
                    line.push(c);
                    k += 1;
                }
            }
        }
        // A one-quote string does not run on to the next line.
        if open.as_ref().is_some_and(|(q, _)| q.len() == 1) {
            open = None;
        }
        cur.push_str(&line);
        let cont = cur.trim_end().ends_with('\\') || depth > 0 || open.is_some();
        if cont {
            cur.push(' ');
            continue;
        }
        out.push((start, std::mem::take(&mut cur)));
        depth = 0;
    }
    if !cur.is_empty() {
        out.push((start, cur));
    }
    out
}

// Every use of os, os.path or pathlib that reaches more than path text, and of random beyond Brevi's own:
// (line, what), such as (3, "os.listdir"). None when the program imports none of them.
pub fn path_uses(src: &str) -> Option<Vec<(usize, String)>> {
    let lines = code_lines(src);
    let mut names: std::collections::HashMap<String, &'static str> = Default::default();
    let mut bad = vec![];
    let mut any = false;
    let mut imports = vec![];
    for (n, l) in &lines {
        for st in l.split(';') {
            let t = st.trim();
            if let Some(rest) = t.strip_prefix("import ") {
                imports.push(*n);
                for part in rest.split(',') {
                    let mut w = part.split_whitespace();
                    let m = w.next().unwrap_or("");
                    let alias = if w.next() == Some("as") { w.next() } else { None };
                    let Some(pm) = path_module(m).or_else(|| path_module(m.split('.').next().unwrap_or(""))) else { continue };
                    any = true;
                    match alias {
                        // `import os.path as p`: p is os.path.
                        Some(a) => drop(names.insert(a.to_string(), path_module(m).unwrap_or(pm))),
                        // `import os.path`: os is os.
                        None => drop(names.insert(m.split('.').next().unwrap_or(m).to_string(), path_module(m.split('.').next().unwrap_or(m)).unwrap_or(pm))),
                    }
                }
            } else if let Some(rest) = t.strip_prefix("from ") {
                let Some((m, list)) = rest.split_once(" import ") else { continue };
                let Some(pm) = path_module(m.trim()) else { continue };
                imports.push(*n);
                any = true;
                for part in list.trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace()).split(',') {
                    let mut w = part.split_whitespace();
                    let a = w.next().unwrap_or("");
                    let bind = if w.next() == Some("as") { w.next().unwrap_or(a) } else { a };
                    if a.is_empty() {
                        continue;
                    }
                    if pm == "os" && a == "path" {
                        names.insert(bind.to_string(), "os.path");
                    } else if !pure_attr(pm, a) {
                        bad.push((*n, if a == "*" { format!("from {pm} import *") } else { format!("{pm}.{a}") }));
                    }
                }
            }
        }
    }
    if !any {
        return None;
    }
    let word = |c: char| c.is_alphanumeric() || c == '_';
    for (n, l) in &lines {
        if imports.contains(n) {
            continue;
        }
        let cs: Vec<char> = l.chars().collect();
        let mut k = 0;
        while k < cs.len() {
            if !word(cs[k]) || (k > 0 && word(cs[k - 1])) {
                k += 1;
                continue;
            }
            let mut e = k;
            while e < cs.len() && word(cs[e]) {
                e += 1;
            }
            let id: String = cs[k..e].iter().collect();
            let before = cs[..k].iter().rev().find(|c| !c.is_whitespace());
            let Some(&m) = names.get(&id).filter(|_| before != Some(&'.')) else {
                k = e;
                continue;
            };
            // The attribute after `name.` (skipping spaces), and where it ends.
            let attr = |from: usize| -> Option<(String, usize)> {
                let mut j = from;
                while j < cs.len() && cs[j] == ' ' {
                    j += 1;
                }
                if cs.get(j) != Some(&'.') {
                    return None;
                }
                j += 1;
                while j < cs.len() && cs[j] == ' ' {
                    j += 1;
                }
                let s = j;
                while j < cs.len() && word(cs[j]) {
                    j += 1;
                }
                (j > s).then(|| (cs[s..j].iter().collect(), j))
            };
            match attr(e) {
                Some((a, j)) if m == "os" && a == "path" => match attr(j) {
                    Some((b, j2)) if pure_attr("os.path", &b) => k = j2,
                    Some((b, j2)) => {
                        bad.push((*n, format!("os.path.{b}")));
                        k = j2
                    }
                    None => {
                        bad.push((*n, "os.path used as a value".into()));
                        k = j
                    }
                },
                Some((a, j)) => {
                    if a == "path" || !pure_attr(m, &a) {
                        bad.push((*n, format!("{m}.{a}")));
                    }
                    k = j
                }
                None => {
                    bad.push((*n, format!("{m} used as a value")));
                    k = e
                }
            }
        }
    }
    Some(bad)
}

// Every use of a capability not in `allowed`, one error per line.
pub fn check(src: &str, items: &[Item], allowed: &[String]) -> Vec<Diag> {
    let mut uses: Vec<(usize, &'static str, String)> = Vec::new();
    for it in items {
        match it {
            Item::Fn { body, .. } | Item::Test { body, .. } => expr(body, &mut uses),
            Item::Type { fields, .. } => fields.iter().filter_map(|f| f.default.as_ref()).for_each(|e| expr(e, &mut uses)),
            Item::Enum { .. } => {}
        }
    }
    for (i, l) in src.split('\n').enumerate() {
        let t = l.trim_end();
        let what = if t.starts_with("rust fn ") || t.starts_with("py fn ") {
            Some("a rust fn or py fn")
        } else if t == "rust:" || t == "py:" {
            Some("a rust: or py: block")
        } else if t.starts_with("use ") && !t.starts_with("use \"") {
            Some("a crate")
        } else {
            None
        };
        if let Some(w) = what {
            uses.push((i + 1, "native", w.to_string()));
        }
    }
    // os, os.path and pathlib are allowed for their path functions only (see path_uses).
    let paths = path_uses(src);
    for (line, m) in crate::pybridge::bridged_imports(src) {
        if paths.is_some() && path_module(&m).or_else(|| path_module(m.split('.').next().unwrap_or(""))).is_some() {
            continue;
        }
        uses.push((line, "native", format!("import {m} (a Python library, run by CPython)")));
    }
    for (line, what) in paths.unwrap_or_default() {
        uses.push((line, "native", what));
    }
    uses.sort();
    let mut seen = Vec::new();
    let mut errs = Vec::new();
    for (line, cap, what) in uses {
        if allowed.iter().any(|a| a == cap) || seen.contains(&(line, cap)) {
            continue;
        }
        seen.push((line, cap));
        let add = format!("add the line `{}` at the top, or do without it", allow_line(cap));
        let fix = if what.starts_with("random.") {
            format!("Brevi's own random has {}; for the rest, {add}", OWN_RANDOM.join(", "))
        } else if what.starts_with("os") || what.starts_with("pathlib") || what.starts_with("from os") || what.starts_with("from pathlib") {
            format!("without it only path functions run (os.path.join, basename, dirname, splitext, normpath, os.sep, pathlib.PurePath); {add}")
        } else {
            add
        };
        errs.push(Diag::new("caps", line, 1, &format!("{what} needs allow {cap}")).fix_hint(&fix));
    }
    errs
}

fn expr(e: &Expr, out: &mut Vec<(usize, &'static str, String)>) {
    match e {
        Expr::Call(n, args, line) => {
            match n.as_str() {
                "read" | "__try_read" | "__open_r" => out.push((*line, "read", "read()".into())),
                "write" | "__open_w" | "__open_a" => out.push((*line, "write", "write()".into())),
                _ => {}
            }
            args.iter().for_each(|(_, a)| expr(a, out));
        }
        Expr::Str(parts) => parts.iter().for_each(|p| {
            if let StrPart::Interp(x) = p {
                expr(x, out)
            }
        }),
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| expr(x, out)),
        Expr::Map(kv) => kv.iter().for_each(|(k, v)| {
            expr(k, out);
            expr(v, out)
        }),
        Expr::Unary(_, x) | Expr::Field(x, _) | Expr::TupleField(x, _) | Expr::Lambda(_, x) | Expr::Try(x) => expr(x, out),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::While(a, b) | Expr::Range(a, b) | Expr::For(_, a, b) => {
            expr(a, out);
            expr(b, out)
        }
        Expr::Method(r, _, args, _) => {
            expr(r, out);
            args.iter().for_each(|a| expr(a, out));
        }
        Expr::If(c, t, f) => {
            expr(c, out);
            expr(t, out);
            if let Some(f) = f {
                expr(f, out)
            }
        }
        Expr::Match(s, arms) => {
            expr(s, out);
            arms.iter().for_each(|(_, a)| expr(a, out));
        }
        Expr::Block(stmts, tail) => {
            stmts.iter().for_each(|s| stmt(s, out));
            if let Some(t) = tail {
                expr(t, out)
            }
        }
        _ => {}
    }
}

fn stmt(s: &Stmt, out: &mut Vec<(usize, &'static str, String)>) {
    match s {
        Stmt::Let { value, .. } => expr(value, out),
        Stmt::Assign { target, value, .. } => {
            expr(target, out);
            expr(value, out)
        }
        Stmt::Expr { expr: e, .. } | Stmt::Assert { cond: e, .. } => expr(e, out),
        Stmt::Return { value: Some(e), .. } => expr(e, out),
        _ => {}
    }
}

// Puts the `allow` lines that `split` blanked back into an edited copy (for `brevi fix`, which writes the file).
pub fn restore(orig: &str, edited: &str) -> String {
    let allow: Vec<&str> = orig.split('\n').collect();
    edited
        .split('\n')
        .enumerate()
        .map(|(i, l)| match allow.get(i) {
            Some(a) if l.is_empty() && a.starts_with("allow ") => a,
            _ => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// The check on a whole program, for main.rs: parses it as compile_to_rust does. A program that does not
// parse still gets the line checks (native code, Python library imports); the compiler reports why it does
// not parse.
pub fn check_src(src: &str, allowed: &[String]) -> Vec<Diag> {
    let (code, _) = crate::ffi::split(src);
    match crate::parser::parse_program(&code) {
        Ok(items) => check(src, &items, allowed),
        Err(_) => check(src, &[], allowed),
    }
}

#[cfg(test)]
mod tests {
    // The modules built in place of CPython's under the sandbox (dynpy::sandbox_module) import only each other
    // and Brevi's own math and time, and open nothing.
    #[test]
    fn sandbox_modules_reach_nothing() {
        for (name, src) in [("random", include_str!("dynpy/py/random.py")), ("os", include_str!("dynpy/py/os.py")), ("posixpath", include_str!("dynpy/py/posixpath.py")), ("pathlib", include_str!("dynpy/py/pathlib.py")), ("io", include_str!("dynpy/py/io.py"))] {
            for l in src.lines().map(str::trim) {
                if let Some(m) = l.strip_prefix("import ").or_else(|| l.strip_prefix("from ")) {
                    let m = m.split_whitespace().next().unwrap();
                    assert!(["math", "time", "posixpath"].contains(&m), "{name}.py imports {m}");
                }
                for bad in ["open(", "__import__", "exec(", "eval(", "getattr(", "sys."] {
                    assert!(!l.contains(bad), "{name}.py uses {bad}");
                }
            }
        }
    }

    // `# allow read` counts only when the comment is nothing but capability names.
    #[test]
    fn comment_allow_lines() {
        assert_eq!(super::split("# allow read, write\nprint(1)").1, Some(vec!["read".to_string(), "write".to_string()]));
        assert_eq!(super::split("#allow native").1, Some(vec!["native".to_string()]));
        assert_eq!(super::split("# allow me to explain\nprint(1)").1, None);
        assert_eq!(super::split("allow read\nprint(1)").1, Some(vec!["read".to_string()]));
        // The comment stays in the code (it is valid Python); the bare line is blanked.
        assert_eq!(super::split("# allow read\nx").0, "# allow read\nx");
    }

    // Path functions pass; anything that reaches files, processes or the environment is named, whatever the
    // alias, import form, or text in strings and comments.
    #[test]
    fn path_uses_names_io() {
        let uses = |s: &str| super::path_uses(s).unwrap().into_iter().map(|(_, w)| w).collect::<Vec<_>>();
        assert_eq!(super::path_uses("print(1)"), None);
        assert!(uses("import os\nprint(os.path.join('a', 'b'), os.sep)  # os.system\ns = 'os.listdir'\n").is_empty());
        assert!(uses("import os.path as p\nfrom pathlib import PurePath\nprint(p.basename('x'), PurePath('a'))\n").is_empty());
        assert_eq!(uses("import os as o\no.system('x')\n"), ["os.system"]);
        assert_eq!(uses("from os import path, remove\npath.exists('x')\n"), ["os.remove", "os.path.exists"]);
        assert_eq!(uses("import os\nf = getattr(os, 'system')\n"), ["os used as a value"]);
        assert_eq!(uses("import os\nprint(f\"{os.getcwd()}\")\n"), ["os.getcwd"]);
        assert_eq!(uses("from pathlib import Path\n"), ["pathlib.Path"]);
        assert_eq!(uses("import pathlib\npathlib.Path('x').unlink()\n"), ["pathlib.Path"]);
        assert_eq!(uses("from os import *\n"), ["from os import *"]);
        assert_eq!(uses("import os\nx = (os.environ\n     ['HOME'])\n"), ["os.environ"]);
        assert_eq!(uses("import random\nrandom.seed(1)\nrandom.betavariate(1, 2)\n"), ["random.betavariate"]);
    }

    // A program that passes the check may be built over dynamic values (main.rs relies on it under --sandbox),
    // so the dynamic runtime must have no way out: no files, processes, network, environment or CPython.
    #[test]
    fn dynamic_runtime_reaches_nothing() {
        let rt = crate::dynpy::RUNTIME;
        for bad in ["std::fs", "std::process::Command", "std::net", "std::env::var", "std::env::set", "File::", "libpython", "Py_Initialize", "dlopen", "#[link"] {
            assert!(!rt.contains(bad), "the dynamic runtime uses {bad}: check it in caps.rs before --sandbox builds over it");
        }
        // C functions it declares: only the generator stacks' memory calls.
        for block in rt.split("extern \"C\" {").skip(1) {
            let decls = &block[..block.find('}').unwrap()];
            for d in decls.lines().map(str::trim).filter(|l| !l.is_empty()) {
                assert!(["fn mmap(", "fn munmap(", "fn mprotect("].iter().any(|ok| d.starts_with(ok)), "the dynamic runtime declares {d}");
            }
        }
    }
}
