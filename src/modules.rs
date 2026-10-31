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

// The joined source and where each file's lines are. Err is a JSON error line.
pub fn expand(main: &str, src: &str) -> Result<(String, Vec<Seg>), String> {
    let mut out = String::new();
    let mut segs: Vec<Seg> = vec![];
    let mut queue: Vec<(String, PathBuf, String, Option<(String, usize)>)> = vec![("".into(), PathBuf::from(main), src.to_string(), None)];
    let mut seen: Vec<PathBuf> = vec![std::fs::canonicalize(main).unwrap_or_else(|_| PathBuf::from(main))];
    while !queue.is_empty() {
        let (name, path, text, _) = queue.remove(0);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let start = out.lines().count() + 1;
        let mut n = 0;
        for (i, l) in text.lines().enumerate() {
            n += 1;
            if let Some(p) = use_path(l.trim_end()) {
                let full = dir.join(p);
                let canon = std::fs::canonicalize(&full).unwrap_or_else(|_| full.clone());
                out.push_str(l);
                out.push('\n');
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
                let item = l.is_empty() || set || l.starts_with([' ', '\t', '#', '}']) || matches!(first, "fn" | "type" | "enum" | "test" | "rust" | "py" | "use");
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
    if segs.len() == 1 {
        return Ok((src.to_string(), segs));
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
