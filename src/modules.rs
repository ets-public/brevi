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
// is called by in `stock.f()` (None for `from`). Only modules with a `.bv` file next to this one count
// (`lib.stock` is lib/stock.bv).
fn py_imports(l: &str, dir: &Path) -> Vec<(String, Option<String>)> {
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
        .map(|(m, a)| (format!("{}.bv", m.replace('.', "/")), m, a))
        .filter(|(p, m, _)| m.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') && dir.join(p).is_file())
        .map(|(p, _, a)| (p, a))
        .collect()
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
    while !queue.is_empty() {
        let (name, path, text, _) = queue.remove(0);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let start = out.lines().count() + 1;
        let mut n = 0;
        for (i, l) in text.lines().enumerate() {
            n += 1;
            let mut paths: Vec<String> = use_path(l.trim_end()).map(|p| vec![p.to_string()]).unwrap_or_default();
            if paths.is_empty() && !l.starts_with([' ', '\t']) {
                for (p, q) in py_imports(l, &dir) {
                    quals.extend(q);
                    paths.push(p);
                }
            }
            for (k, p) in paths.iter().enumerate() {
                let p = p.as_str();
                let full = dir.join(p);
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
                seen.push(canon);
                let shown = if name.is_empty() { p.to_string() } else { Path::new(&name).parent().map_or(p.into(), |d| d.join(p).to_string_lossy().to_string()) };
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
        out = out.lines().map(|l| unqualify(l, &quals) + "\n").collect();
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
