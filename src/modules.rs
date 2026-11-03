// `use "geom.bv"` brings in the fn, type, enum, test and foreign items of another Brevi file (path relative to
// the file that names it). The files are joined after the main file into one program; errors in them name
// the file: {"file":"geom.bv","line":3,...}.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Seg {
    pub file: String, // as written in the use line (the main file's segment has "")
    pub path: PathBuf,
    pub start: usize, // first line in the joined source, from 1
    pub len: usize,
}

fn use_path(l: &str) -> Option<&str> {
    let r = l.strip_prefix("use ")?.trim();
    let r = match r.find(" #") {
        Some(i) => r[..i].trim_end(),
        None => r,
    };
    let p = r.strip_prefix('"')?.strip_suffix('"')?;
    if p.ends_with(".bv") && !p.contains('"') {
        Some(p)
    } else {
        None
    }
}

// Python's `import stock, key as k` or `from stock import Stock`: the modules named, with the name each
// is called by in `stock.f()` (None for `from`). Only modules with a `.bv` or `.py` file next to this one count
// (`lib.stock` is lib/stock.bv, else lib/stock.py; a folder `lib/` with `__init__.py` is a package).
// Like Python, a module is looked for next to the importing file, then next to the main file.
fn py_imports(l: &str, dirs: &[&Path]) -> Vec<(String, Option<String>)> {
    let l = l.split(" #").next().unwrap_or("").trim();
    let mods: Vec<(String, Option<String>)> = if let Some(r) = l.strip_prefix("import ") {
        r.split(',')
            .filter_map(|m| {
                let mut w = m.split_whitespace();
                let name = w.next()?.to_string();
                let alias = match (w.next(), w.next()) {
                    (Some("as"), Some(a)) => a.to_string(),
                    _ => name.clone(),
                };
                Some((name, Some(alias)))
            })
            .collect()
    } else if let Some(r) = l.strip_prefix("from ") {
        match r.split_whitespace().next() {
            Some(m) if r[m.len()..].trim_start().starts_with("import ") => vec![(m.trim_start_matches('.').to_string(), None)],
            _ => vec![],
        }
    } else {
        vec![]
    };
    mods.into_iter()
        .filter(|(m, _)| m.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.'))
        .filter_map(|(m, a)| {
            let base = m.replace('.', "/");
            let base = &base;
            let mut found = dirs.iter().flat_map(|d| ["bv", "py"].iter().map(move |e| d.join(format!("{base}.{e}"))));
            let f = found.find(|p| p.is_file());
            if f.is_some() {
                crate::pybridge::note_local(&m); // pybridge: a file of the program, not a Python library
            }
            f.map(|p| (p.to_string_lossy().to_string(), a))
        })
        .collect()
}

// A GUI library a top-level Python import names (`import tkinter as tk`, `from PyQt5.QtWidgets import *`).
fn gui_import(l: &str) -> Option<&'static str> {
    let r = l.strip_prefix("import ").or_else(|| l.strip_prefix("from "))?;
    let m = r.split([' ', ',', '.']).next()?;
    ["tkinter", "turtle", "pygame", "PyQt5", "PyQt6", "PySide2", "PySide6", "wx", "kivy", "customtkinter", "pyglet", "arcade", "dearpygui"]
        .into_iter()
        .find(|g| *g == m)
}

// `stock.find(x)` becomes `find(x)` for a module brought in with `import stock`: the files are one program.
fn unqualify(l: &str, names: &[String]) -> String {
    if names.is_empty() {
        return l.to_string();
    }
    let cs: Vec<char> = l.chars().collect();
    let mut out = String::with_capacity(l.len());
    let mut i = 0;
    while i < cs.len() {
        let boundary = i == 0 || !(cs[i - 1].is_alphanumeric() || cs[i - 1] == '_' || cs[i - 1] == '.');
        if boundary {
            if let Some(n) = names.iter().find(|n| {
                let k = n.chars().count();
                cs[i..].starts_with(&n.chars().collect::<Vec<_>>()) && cs.get(i + k) == Some(&'.') && cs.get(i + k + 1).map_or(false, |c| c.is_alphabetic() || *c == '_')
            }) {
                i += n.chars().count() + 1;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

// The joined source and where each file's lines are. Err is a JSON error line.
pub fn expand(main: &str, src: &str) -> Result<(String, Vec<Seg>), String> {
    let mut out = String::new();
    let mut segs: Vec<Seg> = vec![];
    let mut queue: Vec<(String, PathBuf, String, Option<(String, usize)>)> = vec![("".into(), PathBuf::from(main), src.to_string(), None)];
    let mut seen: Vec<PathBuf> = vec![std::fs::canonicalize(main).unwrap_or_else(|_| PathBuf::from(main))];
    // Names Python code calls modules by (`import stock` then `stock.load()`), across all files.
    let mut quals: Vec<String> = vec![];
    let mut changed = false;
    let root = Path::new(main).parent().map(Path::to_path_buf).unwrap_or_default();
    while !queue.is_empty() {
        let (name, path, text, _) = queue.remove(0);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let start = out.lines().count() + 1;
        let mut n = 0;
        // Inside a `"""docstring"""` that spans lines.
        let mut in_doc = false;
        // Inside a used file's `if __name__ == "__main__":` block, which Python only runs for the main file.
        let mut in_main_guard = false;
        for (i, l) in text.lines().enumerate() {
            n += 1;
            if !name.is_empty() {
                let guard = l.starts_with("if __name__") && l.contains("__main__");
                if guard || (in_main_guard && (l.trim().is_empty() || l.starts_with([' ', '\t']))) {
                    in_main_guard = true;
                    // Kept as a blank line so line numbers in errors still match the file.
                    out.push('\n');
                    continue;
                }
                in_main_guard = false;
            }
            let t = l.trim_start();
            let doc_line = in_doc || t.starts_with("\"\"\"") || t.starts_with("'''") || (t.starts_with('"') && t.trim_end().ends_with('"'));
            if in_doc {
                if t.contains("\"\"\"") || t.contains("'''") {
                    in_doc = false;
                }
            } else if let Some(q) = ["\"\"\"", "'''"].iter().find(|q| t.starts_with(**q)) {
                // A docstring that does not close on its own line.
                in_doc = !t[3..].contains(q);
            }
            if let Some(g) = gui_import(l).filter(|g| *g != "tkinter") {
                let at = if name.is_empty() { String::new() } else { format!("\"file\":\"{}\",", crate::diag::esc(&name)) };
                return Err(format!("{{{at}\"line\":{},\"msg\":\"{g} is not supported yet: the GUI library Brevi compiles is tkinter\",\"fix\":\"write the window with tkinter, or run this program with python3\"}}", i + 1));
            }
            // Each file to bring in, as written (or found) and its path on disk.
            let mut paths: Vec<(String, PathBuf)> = use_path(l.trim_end()).map(|p| vec![(p.to_string(), dir.join(p))]).unwrap_or_default();
            if paths.is_empty() && !l.starts_with([' ', '\t']) {
                for (p, q) in py_imports(l, &[dir.as_path(), root.as_path()]) {
                    quals.extend(q);
                    let full = PathBuf::from(&p);
                    // Shown in errors relative to the main file, as Python finds it.
                    let shown = full.strip_prefix(&root).map_or(p.clone(), |r| r.to_string_lossy().to_string());
                    paths.push((shown, full));
                }
            }
            for (k, (p, full)) in paths.iter().enumerate() {
                let p = p.as_str();
                let full = full.clone();
                let py = use_path(l.trim_end()).is_none();
                let canon = std::fs::canonicalize(&full).unwrap_or_else(|_| full.clone());
                if k == 0 {
                    out.push_str(l);
                    out.push('\n');
                }
                if seen.contains(&canon) {
                    continue;
                }
                let body = std::fs::read_to_string(&full).map_err(|e| {
                    let at = if name.is_empty() { String::new() } else { format!("\"file\":\"{}\",", crate::diag::esc(&name)) };
                    format!("{{{at}\"line\":{},\"msg\":\"cannot read {}: {}\"}}", i + 1, crate::diag::esc(p), crate::diag::esc(&e.to_string()))
                })?;
                let body = fill_file_name(&body, &full);
                seen.push(canon);
                let shown = if name.is_empty() || py { p.to_string() } else { Path::new(&name).parent().map_or(p.into(), |d| d.join(p).to_string_lossy().to_string()) };
                queue.push((shown, full, body, Some((name.clone(), i + 1))));
            }
            if !paths.is_empty() {
                changed = true;
                continue;
            }
            // A used file holds only items: its top-level statements would run as part of the main program.
            if !name.is_empty() {
                let first = l.split(|c: char| c.is_whitespace() || c == '(' || c == ':').next().unwrap_or("");
                // `NAME = value` sets a name every file can read.
                let set = first.chars().all(|c| c.is_alphanumeric() || c == '_') && !first.is_empty() && {
                    let rest = l[first.len()..].trim_start();
                    (rest.starts_with('=') && !rest.starts_with("==")) || rest.starts_with(':')
                };
                // Python modules also hold `def`, `class`, decorators, imports and the closing line of a long list.
                let item = l.is_empty()
                    || doc_line
                    || set
                    || l.starts_with([' ', '\t', '#', '}', ']', ')', '@'])
                    || matches!(first, "fn" | "type" | "enum" | "test" | "rust" | "py" | "use" | "def" | "class" | "import" | "from");
                if !item {
                    return Err(format!(
                        "{{\"file\":\"{}\",\"line\":{},\"msg\":\"a used file holds only fn, type, enum and test items and NAME = value lines\",\"fix\":\"move this statement into a function, or into the main file\"}}",
                        crate::diag::esc(&name),
                        i + 1
                    ));
                }
            }
            out.push_str(l);
            out.push('\n');
        }
        segs.push(Seg { file: name, path, start, len: n });
    }
    if segs.len() == 1 && !changed {
        return Ok((src.to_string(), segs));
    }
    if !quals.is_empty() {
        // Not inside a function with a parameter of the module's name (`def fill(self, sku, stock)`, then
        // `stock.take(...)` is the parameter's method).
        let mut scope: Vec<(usize, Vec<String>)> = vec![];
        let mut joined = String::with_capacity(out.len());
        for l in out.lines() {
            let t = l.trim_start();
            let ind = l.len() - t.len();
            if !t.is_empty() && !t.starts_with('#') {
                scope.retain(|(d, _)| ind > *d);
            }
            let shadow: Vec<String> = scope.iter().flat_map(|(_, ps)| ps.iter().cloned()).collect();
            let names: Vec<String> = quals.iter().filter(|q| !shadow.contains(q)).cloned().collect();
            joined.push_str(&unqualify(l, &names));
            joined.push('\n');
            if let Some(rest) = t.strip_prefix("def ") {
                let ps = rest.split_once('(').and_then(|(_, r)| r.split_once(')')).map_or(vec![], |(ps, _)| {
                    ps.split(',').map(|p| p.split(&[':', '='][..]).next().unwrap_or("").trim().trim_start_matches('*').to_string()).filter(|p| !p.is_empty()).collect()
                });
                scope.push((ind, ps));
            }
        }
        out = joined;
    }
    Ok((out, segs))
}

// (file, line in that file) for a line of the joined source; the main file is "".
pub fn locate(segs: &[Seg], line: usize) -> (&str, usize) {
    for s in segs.iter().skip(1) {
        if line >= s.start && line < s.start + s.len {
            return (&s.file, line - s.start + 1);
        }
    }
    ("", line)
}

// A JSON error line with its line number moved into the file it is in.
pub fn relocate(segs: &[Seg], json: &str) -> String {
    let Some(p) = json.find("\"line\":") else { return json.to_string() };
    let digits: String = json[p + 7..].chars().take_while(|c| c.is_ascii_digit()).collect();
    let Ok(line) = digits.parse::<usize>() else { return json.to_string() };
    let (f, l) = locate(segs, line);
    if f.is_empty() {
        return json.to_string();
    }
    format!("{}\"file\":\"{}\",\"line\":{l}{}", &json[..p], crate::diag::esc(f), &json[p + 7 + digits.len()..])
}

// Splits the joined source back into each file's text (for `brevi fix`).
pub fn split_back(segs: &[Seg], joined: &str) -> Vec<(PathBuf, String)> {
    let lines: Vec<&str> = joined.lines().collect();
    segs.iter()
        .map(|s| {
            let part: Vec<&str> = lines.iter().skip(s.start - 1).take(s.len).copied().collect();
            (s.path.clone(), format!("{}\n", part.join("\n")))
        })
        .collect()
}

// Python's `__file__`: the file's absolute path, written into the source as a string (outside strings and
// comments), as CPython 3.9+ gives it for a script run by path.
pub fn fill_file_name(src: &str, path: &Path) -> String {
    if !src.contains("__file__") {
        return src.to_string();
    }
    // `sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))` lets CPython find the program's
    // other files; a build finds them already, so the line does nothing.
    let src: String = src
        .split_inclusive('\n')
        .map(|l| {
            let t = l.trim();
            if (t.starts_with("sys.path.insert(") || t.starts_with("sys.path.append(")) && t.ends_with(')') && t.contains("__file__") {
                let ind = &l[..l.len() - l.trim_start().len()];
                format!("{ind}pass{}", if l.ends_with('\n') { "\n" } else { "" })
            } else {
                l.to_string()
            }
        })
        .collect();
    // Then an `import os` that only served that line goes too (os itself would be run by CPython).
    let os_used = src.lines().filter(|l| l.trim() != "import os").any(|l| {
        let b = l.as_bytes();
        l.match_indices("os").any(|(i, _)| (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && b.get(i + 2).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_')))
    });
    let src: String = if os_used { src } else { src.split_inclusive('\n').map(|l| if l.trim() == "import os" { l.replacen("import os", "pass", 1) } else { l.to_string() }).collect() };
    let src = src.as_str();
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let lit = format!("\"{}\"", abs.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""));
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'"' || c == b'\'' {
            let triple = i + 2 < b.len() && b[i + 1] == c && b[i + 2] == c;
            let q = if triple { 3 } else { 1 };
            i += q;
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 2;
                } else if b[i] == c && (!triple || i + 2 < b.len() && b[i + 1] == c && b[i + 2] == c) {
                    i += q;
                    break;
                } else if !triple && b[i] == b'\n' {
                    break;
                } else {
                    i += 1;
                }
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            // A string prefix (r"", f'', b"") is read with its string on the next turn.
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') && i - st <= 2 {
                continue;
            }
            if &src[st..i] == "__file__" && (st == 0 || b[st - 1] != b'.') {
                out.push_str(&src[last..st]);
                out.push_str(&lit);
                last = i;
            }
        } else {
            i += 1;
        }
    }
    out.push_str(&src[last..]);
    out
}
