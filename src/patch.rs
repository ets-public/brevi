// `brevi patch FILE PATCH`: edit a program by name. Each top-level item in PATCH (`fn name`, `type Name`,
// `enum Name`, `test "name"`) replaces the item of the same name in FILE, or is added at the end.
// `del fn name` removes one. Top-level script lines in PATCH are refused, since they have no name.

// Top-level chunks: a line at column 0 starts one; indented, blank, comment and `}` lines continue it.
fn chunks(src: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in src.lines() {
        let cont = l.is_empty() || l.starts_with(' ') || l.starts_with('\t') || l.starts_with('}') || l.starts_with('#');
        if cont && !out.is_empty() {
            let last = out.last_mut().unwrap();
            last.push_str(l);
            last.push('\n');
        } else {
            out.push(format!("{l}\n"));
        }
    }
    out
}

// The item's name, like "fn parse" or "test \"totals\"", or None for a script line.
fn key(chunk: &str) -> Option<String> {
    let first = chunk.lines().next()?;
    // `rust fn f` and `py fn f` are functions too: a patch may swap one kind for another.
    let first = first.strip_prefix("rust ").or_else(|| first.strip_prefix("py ")).filter(|r| r.starts_with("fn ")).unwrap_or(first);
    let mut w = first.split(|c: char| c.is_whitespace() || c == '(' || c == ':' || c == '{' || c == '[' || c == '=');
    let kw = w.next()?;
    match kw {
        "fn" | "type" | "enum" | "struct" => w.find(|s| !s.is_empty()).map(|n| format!("{kw} {n}")),
        "test" => first.split('"').nth(1).map(|n| format!("test \"{n}\"")),
        _ => None,
    }
}

pub fn patch(src: &str, p: &str) -> Result<(String, Vec<String>), String> {
    let mut items = chunks(src);
    let mut done = vec![];
    for c in chunks(p) {
        if c.trim().is_empty() || c.trim_start().starts_with('#') {
            continue;
        }
        if let Some(rest) = c.trim().strip_prefix("del ") {
            let k = key(&format!("{rest}\n")).ok_or_else(|| format!("cannot delete {rest}: write del fn NAME"))?;
            let n = items.len();
            items.retain(|i| key(i).as_deref() != Some(k.as_str()));
            if items.len() == n {
                return Err(format!("no {k} to delete"));
            }
            done.push(format!("deleted {k}"));
            continue;
        }
        let k = key(&c).ok_or_else(|| format!("patch line has no name: {}", c.lines().next().unwrap_or("").trim()))?;
        let mut body = c.trim_end().to_string();
        body.push('\n');
        match items.iter().position(|i| key(i).as_deref() == Some(k.as_str())) {
            Some(i) => {
                // Keep the blank lines that separated the old item from the next one.
                let gap = items[i].len() - items[i].trim_end().len();
                body = format!("{}{}", body.trim_end(), &items[i][items[i].trim_end().len()..]);
                if gap == 0 {
                    body.push('\n');
                }
                items[i] = body;
                done.push(format!("replaced {k}"));
            }
            None => {
                if let Some(last) = items.last_mut() {
                    if !last.ends_with("\n\n") {
                        last.push('\n');
                    }
                }
                items.push(body);
                done.push(format!("added {k}"));
            }
        }
    }
    Ok((items.concat(), done))
}

#[cfg(test)]
mod tests {
    #[test]
    fn replaces_adds_and_deletes() {
        let src = "fn a() -> int:\n  1\n\nfn b() -> int:\n  2\n\nprint(a())\n";
        let (out, done) = super::patch(src, "fn b() -> int:\n  3\nfn c() -> int:\n  4\n").unwrap();
        assert_eq!(out, "fn a() -> int:\n  1\n\nfn b() -> int:\n  3\n\nprint(a())\n\nfn c() -> int:\n  4\n");
        assert_eq!(done, vec!["replaced fn b", "added fn c"]);
        let (out, _) = super::patch(&out, "del fn c\n").unwrap();
        assert!(!out.contains("fn c"));
        assert!(super::patch(src, "x = 1\n").is_err());
        assert!(super::patch(src, "del fn zz\n").is_err());
    }
}
