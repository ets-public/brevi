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
        let Some(rest) = rest else {
            out.push(l);
            continue;
        };
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
    for (line, m) in crate::pybridge::bridged_imports(src) {
        uses.push((line, "native", format!("import {m} (a Python library, run by CPython)")));
    }
    uses.sort();
    let mut seen = Vec::new();
    let mut errs = Vec::new();
    for (line, cap, what) in uses {
        if allowed.iter().any(|a| a == cap) || seen.contains(&(line, cap)) {
            continue;
        }
        seen.push((line, cap));
        errs.push(
            Diag::new("caps", line, 1, &format!("{what} needs allow {cap}"))
                .fix_hint(&format!("add the line `allow {cap}` at the top, or do without it")),
        );
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
