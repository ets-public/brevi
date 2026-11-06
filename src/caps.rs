// Declared capabilities: `allow read, write` at the top of a program says what it may touch.
// `brevi run --sandbox` (and the MCP tools, by default) refuse a program that uses anything it did not declare.
// A program with an `allow` line is held to it on every command, sandbox or not.
//
//   read    read(path)
//   write   write(path, text)
//   native  rust fn, py fn, rust: and py: blocks, crates (use NAME "VER"), and Python libraries CPython runs
//           (import os, import numpy: pybridge.rs): these can do anything
//
// Under --sandbox only the caller grants capabilities: `--allow read,write,native` and `--allow-py requests,...`
// (the MCP tools' allow and allow_py arguments). The code being held cannot free itself: an allow line in it that
// asks for more than the caller granted is refused, and one that asks for less narrows nothing. `--allow-py`
// grants CPython for the named Python modules only (and whatever CPython reaches through them), not rust fn,
// rust: blocks or crates. Without --sandbox, a program with an allow line is held to it as before.
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

// What the caller granted with --allow and --allow-py: "read", "write", "native", "py:requests".
pub static GRANT: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
// Whether this run is under --sandbox (or --allow / --allow-py, which imply it).
pub static SANDBOX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn sandboxed() -> bool {
    SANDBOX.load(std::sync::atomic::Ordering::Relaxed)
}

fn grant() -> Vec<String> {
    GRANT.get().cloned().unwrap_or_default()
}

// The value of --allow (read, write, native, comma or space separated) or --allow-py (Python module names).
pub fn parse_grant(py: bool, value: &str) -> Result<Vec<String>, String> {
    let mut out = vec![];
    for w in value.split([',', ' ']).map(str::trim).filter(|w| !w.is_empty()) {
        if py {
            if !w.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_')) {
                return Err(format!("--allow-py takes Python module names, not '{w}'"));
            }
            out.push(format!("py:{}", w.split('.').next().unwrap_or(w)));
        } else if CAPS.contains(&w) {
            out.push(w.to_string());
        } else if w != "none" {
            return Err(format!("--allow takes read, write or native, not '{w}'"));
        }
    }
    Ok(out)
}

// The grant as flags, for the brevi processes this one starts (and the result cache's key).
pub fn grant_flags() -> Vec<String> {
    let g = grant();
    let caps: Vec<&str> = g.iter().filter(|c| !c.starts_with("py:")).map(String::as_str).collect();
    let py: Vec<&str> = g.iter().filter_map(|c| c.strip_prefix("py:")).collect();
    let mut out = vec![];
    if !caps.is_empty() {
        out.extend(["--allow".to_string(), caps.join(",")]);
    }
    if !py.is_empty() {
        out.extend(["--allow-py".to_string(), py.join(",")]);
    }
    out
}

// The Python modules --allow-py granted, when the caller granted those and not all of native.
pub fn py_only() -> Option<Vec<String>> {
    let g = grant();
    if g.iter().any(|c| c == "native") {
        return None;
    }
    let py: Vec<String> = g.iter().filter_map(|c| c.strip_prefix("py:")).map(String::from).collect();
    if py.is_empty() { None } else { Some(py) }
}

// The capabilities a held program may use: under --sandbox, what the caller granted, with an error for each allow
// line that asks for more; otherwise its own allow lines. `src` still has its allow lines.
pub fn held(src: &str, allowed: Option<Vec<String>>, sandbox: bool) -> (Vec<String>, Vec<Diag>) {
    if !sandbox {
        return (allowed.unwrap_or_default(), vec![]);
    }
    let g = grant();
    let covered = |c: &str| c == "none" || g.iter().any(|x| x == c) || (c == "native" && g.iter().any(|x| x.starts_with("py:")));
    let mut errs = vec![];
    for (i, l) in src.split('\n').enumerate() {
        let list = match l.strip_prefix("allow ").filter(|r| !r.trim_start().starts_with(['=', '.', '(', '[', '+', '-'])) {
            Some(rest) => rest.split('#').next().unwrap_or("").split([',', ' ']).map(str::trim).filter(|c| CAPS.contains(c)).map(String::from).collect(),
            None => comment_caps(l).unwrap_or_default(),
        };
        for c in list.iter().filter(|c| !covered(c)) {
            errs.push(
                Diag::new("caps", i + 1, 1, &format!("`{}` grants nothing under --sandbox: only the caller grants {c}", allow_line(c)))
                    .fix_hint(&format!("remove the line; if you trust the program with it, run with --allow {c} (MCP tools: \"allow\": [\"{c}\"])")),
            );
        }
    }
    (g, errs)
}

// How to get a capability a program lacks: from the caller under --sandbox, else an allow line. `module`: the
// Python module(s) the use is about, which --allow-py can grant alone.
pub fn grant_hint(cap: &str, module: Option<&str>) -> String {
    if !sandboxed() {
        return format!("add the line `{}` at the top, or do without it", allow_line(cap));
    }
    match module {
        Some(m) if cap == "native" => {
            let list = m.split(", ").map(|x| format!("\"{x}\"")).collect::<Vec<_>>().join(", ");
            format!("do without it, or the caller grants it: --allow-py {} (MCP tools: \"allow_py\": [{list}])", m.replace(", ", ","))
        }
        _ => format!("do without it, or the caller grants it: --allow {cap} (MCP tools: \"allow\": [\"{cap}\"])"),
    }
}

// The module a native use names: `import requests`, `os.system`, `from os import *`, `argparse.FileType`.
fn use_module(what: &str) -> Option<String> {
    let w = what.strip_prefix("import ").or_else(|| what.strip_prefix("from ")).unwrap_or(what);
    let m = w.split(['.', ' ']).next()?;
    (!m.is_empty() && m.chars().all(|c| c.is_alphanumeric() || c == '_') && !what.starts_with("a ")).then(|| m.to_string())
}

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

// What src/dynpy/py/asyncio.py has: the event loop without I/O, threads, processes or signals.
const OWN_ASYNCIO: &[&str] = &[
    "run", "Runner", "sleep", "gather", "create_task", "ensure_future", "wait", "wait_for", "as_completed", "shield", "current_task", "all_tasks",
    "Future", "Task", "Queue", "PriorityQueue", "LifoQueue", "QueueEmpty", "QueueFull", "QueueShutDown", "Lock", "Event", "Condition", "Semaphore",
    "BoundedSemaphore", "timeout", "timeout_at", "Timeout", "TaskGroup", "CancelledError", "InvalidStateError", "TimeoutError", "BrokenBarrierError",
    "FIRST_COMPLETED", "FIRST_EXCEPTION", "ALL_COMPLETED", "get_running_loop", "get_event_loop", "new_event_loop", "set_event_loop", "iscoroutine",
    "isfuture", "AbstractEventLoop", "BaseEventLoop", "Handle", "TimerHandle",
];
// Event loop methods Brevi's asyncio lacks (they reach sockets, files, threads, processes or signals).
const ASYNCIO_LOOP_IO: &[&str] = &[
    "run_in_executor", "set_default_executor", "getaddrinfo", "getnameinfo", "create_connection", "create_server", "create_unix_connection",
    "create_unix_server", "create_datagram_endpoint", "connect_accepted_socket", "sendfile", "start_tls", "add_reader", "remove_reader",
    "add_writer", "remove_writer", "sock_recv", "sock_recv_into", "sock_recvfrom", "sock_sendall", "sock_sendto", "sock_connect", "sock_accept",
    "sock_sendfile", "connect_read_pipe", "connect_write_pipe", "subprocess_exec", "subprocess_shell", "add_signal_handler", "remove_signal_handler",
    "shutdown_default_executor",
];

// The module os, os.path (posixpath) or pathlib a name or dotted module stands for.
fn path_module(m: &str) -> Option<&'static str> {
    match m {
        "os" => Some("os"),
        "os.path" | "posixpath" => Some("os.path"),
        "pathlib" => Some("pathlib"),
        "random" => Some("random"),
        "io" => Some("io"),
        "textwrap" => Some("textwrap"),
        "argparse" => Some("argparse"),
        "asyncio" => Some("asyncio"),
        _ => None,
    }
}

fn pure_attr(module: &str, attr: &str) -> bool {
    match module {
        "os" => PURE_OS.contains(&attr),
        "os.path" => PURE_OS_PATH.contains(&attr),
        "random" => OWN_RANDOM.contains(&attr),
        "io" => attr == "StringIO",
        "textwrap" => matches!(attr, "wrap" | "fill" | "shorten" | "dedent" | "indent" | "TextWrapper"),
        // all of argparse but FileType, which opens files
        "argparse" => attr != "FileType",
        "asyncio" => OWN_ASYNCIO.contains(&attr),
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
    if names.values().any(|m| *m == "asyncio") {
        for (n, l) in &lines {
            if let Some(m) = ASYNCIO_LOOP_IO.iter().find(|m| l.contains(&format!(".{m}("))) {
                bad.push((*n, format!("asyncio loop.{m}")));
            }
        }
    }
    // argparse's `@file` arguments read files; Brevi's copy has no way to.
    if names.values().any(|m| *m == "argparse") {
        if let Some((n, _)) = lines.iter().find(|(_, l)| l.contains("fromfile_prefix_chars")) {
            bad.push((*n, "argparse fromfile_prefix_chars".into()));
        }
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
        let module = if cap == "native" { use_module(&what) } else { None };
        let py_granted = module.as_ref().is_some_and(|m| allowed.iter().any(|a| a.strip_prefix("py:") == Some(m.as_str())));
        if allowed.iter().any(|a| a == cap) || py_granted || seen.contains(&(line, cap)) {
            continue;
        }
        seen.push((line, cap));
        let add = grant_hint(cap, module.as_deref());
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
        for (name, src) in [("random", include_str!("dynpy/py/random.py")), ("os", include_str!("dynpy/py/os.py")), ("posixpath", include_str!("dynpy/py/posixpath.py")), ("pathlib", include_str!("dynpy/py/pathlib.py")), ("io", include_str!("dynpy/py/io.py")), ("textwrap", include_str!("dynpy/py/textwrap.py"))] {
            for l in src.lines().map(str::trim) {
                if let Some(m) = l.strip_prefix("import ").or_else(|| l.strip_prefix("from ")) {
                    let m = m.split_whitespace().next().unwrap();
                    assert!(["math", "time", "posixpath", "_brevi_mt"].contains(&m), "{name}.py imports {m}");
                }
                for bad in ["open(", "__import__", "exec(", "eval(", "getattr(", "sys."] {
                    assert!(!l.contains(bad), "{name}.py uses {bad}");
                }
            }
        }
    }

    // Brevi's asyncio (CPython's, cut down: see its header) runs callbacks and timers only: it imports Brevi's own
    // modules (sys only to write the event loop's error reports to stderr) and opens nothing.
    #[test]
    fn asyncio_reaches_nothing() {
        let src = include_str!("dynpy/py/asyncio.py");
        for l in src.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            if let Some(m) = l.strip_prefix("import ").or_else(|| l.strip_prefix("from ")) {
                let m = m.split_whitespace().next().unwrap();
                assert!(["_brevi_async", "collections", "heapq", "math", "time", "sys"].contains(&m), "asyncio.py imports {m}");
            }
            for bad in ["open(", "__import__", "exec(", "eval(", "socket", "subprocess"] {
                assert!(!l.contains(bad), "asyncio.py uses {bad}");
            }
        }
    }

    // Brevi's argparse (CPython's, edited: see its header) reads sys.argv and writes to stdout and stderr, so it
    // uses sys and getattr, but imports only Brevi's own modules and opens nothing.
    #[test]
    fn argparse_copy_reaches_nothing() {
        for l in include_str!("dynpy/py/argparse.py").lines().map(str::trim) {
            if let Some(m) = l.strip_prefix("import ").or_else(|| l.strip_prefix("from ")) {
                let m = m.split_whitespace().next().unwrap();
                assert!(["os", "re", "sys", "textwrap", "_brevi_term"].contains(&m), "argparse.py imports {m}");
            }
            for bad in ["open(", "__import__", "exec(", "eval(", "_sys.stdin", "environ"] {
                assert!(!l.contains(bad), "argparse.py uses {bad}");
            }
        }
    }

    // Grants come from the caller: --allow takes capability names, --allow-py module names (kept by package).
    #[test]
    fn caller_grants() {
        assert_eq!(super::parse_grant(false, "read, write").unwrap(), ["read", "write"]);
        assert!(super::parse_grant(false, "net").is_err());
        assert_eq!(super::parse_grant(true, "requests,google.genai").unwrap(), ["py:requests", "py:google"]);
        assert!(super::parse_grant(true, "os;rm").is_err());
        // Outside the sandbox the file's own allow lines hold it; under it they add nothing (no grant here).
        let (g, errs) = super::held("# allow read
x", Some(vec!["read".into()]), false);
        assert_eq!((g, errs.len()), (vec!["read".to_string()], 0));
        let (g, errs) = super::held("# allow read
allow native
x", Some(vec!["read".into(), "native".into()]), true);
        assert!(g.is_empty());
        assert_eq!(errs.iter().map(|d| d.line).collect::<Vec<_>>(), [1, 2]);
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
        assert!(uses("import argparse\np = argparse.ArgumentParser(formatter_class=argparse.RawTextHelpFormatter)\n").is_empty());
        assert_eq!(uses("import argparse\np = argparse.ArgumentParser(fromfile_prefix_chars='@')\np.add_argument('f', type=argparse.FileType('r'))\n"), ["argparse fromfile_prefix_chars", "argparse.FileType"]);
    }

    // A program that passes the check may be built over dynamic values (main.rs relies on it under --sandbox),
    // so the dynamic runtime must have no way out: no files, processes, network, environment or CPython.
    #[test]
    fn dynamic_runtime_reaches_nothing() {
        let rt = crate::dynpy::RUNTIME;
        for bad in ["std::fs", "std::process::Command", "std::net", "std::env::var", "std::env::set", "File::", "libpython", "Py_Initialize", "dlopen", "#[link"] {
            assert!(!rt.contains(bad), "the dynamic runtime uses {bad}: check it in caps.rs before --sandbox builds over it");
        }
        // C functions it declares: only the generator stacks' memory calls and the clock.
        for block in rt.split("extern \"C\" {").skip(1) {
            let decls = &block[..block.find('}').unwrap()];
            for d in decls.lines().map(str::trim).filter(|l| !l.is_empty()) {
                assert!(["fn mmap(", "fn munmap(", "fn mprotect(", "fn clock_gettime("].iter().any(|ok| d.starts_with(ok)), "the dynamic runtime declares {d}");
            }
        }
    }
}
