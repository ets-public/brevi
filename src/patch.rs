// `brevi patch FILE PATCH`: edit a program by name. Each top-level item in PATCH (`fn name`, `type Name`,
// `enum Name`, `test "name"`) replaces the item of the same name in FILE, or is added at the end.
// `del fn name` removes one. Top-level script lines in PATCH are refused, since they have no name.
// Python files work the same way with `def name` and `class Name` (decorators go with their def); a new
// def or class goes after the file's last one, so script lines below it can call it.

// Top-level chunks: a line at column 0 starts one; indented, blank, comment and `}` lines continue it.
fn chunks(src: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in src.lines() {
        let cont = l.is_empty()
            || l.starts_with(' ')
            || l.starts_with('\t')
            || l.starts_with('}')
            || l.starts_with('#');
        // `@decorator` lines belong to the def or class below them.
        let decorated = out
            .last()
            .is_some_and(|c: &String| c.lines().all(|x| x.starts_with('@')));
        if (cont || decorated) && !out.is_empty() {
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
    let first = chunk.lines().find(|l| !l.starts_with('@'))?;
    let first = first
        .strip_prefix("async ")
        .filter(|r| r.starts_with("def "))
        .unwrap_or(first);
    // `rust fn f` and `py fn f` are functions too: a patch may swap one kind for another.
    let first = first
        .strip_prefix("rust ")
        .or_else(|| first.strip_prefix("py "))
        .filter(|r| r.starts_with("fn "))
        .unwrap_or(first);
    let mut w = first.split(|c: char| {
        c.is_whitespace() || c == '(' || c == ':' || c == '{' || c == '[' || c == '='
    });
    let kw = w.next()?;
    match kw {
        "fn" | "type" | "enum" | "struct" | "def" | "class" => {
            w.find(|s| !s.is_empty()).map(|n| format!("{kw} {n}"))
        }
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
            let k = key(&format!("{rest}\n")).ok_or_else(|| {
                format!("cannot delete {rest}: write del fn NAME (or del def NAME)")
            })?;
            let n = items.len();
            items.retain(|i| key(i).as_deref() != Some(k.as_str()));
            if items.len() == n {
                return Err(format!("no {k} to delete"));
            }
            done.push(format!("deleted {k}"));
            continue;
        }
        let k = key(&c).ok_or_else(|| {
            format!(
                "patch line has no name: {}",
                c.lines().next().unwrap_or("").trim()
            )
        })?;
        let mut body = c.trim_end().to_string();
        body.push('\n');
        match items
            .iter()
            .position(|i| key(i).as_deref() == Some(k.as_str()))
        {
            Some(i) => {
                // Keep the blank lines that separated the old item from the next one.
                let gap = items[i].len() - items[i].trim_end().len();
                body = format!(
                    "{}{}",
                    body.trim_end(),
                    &items[i][items[i].trim_end().len()..]
                );
                if gap == 0 {
                    body.push('\n');
                }
                items[i] = body;
                done.push(format!("replaced {k}"));
            }
            // Python: after the last def or class (or at the top), not below the script that calls it.
            None if k.starts_with("def ") || k.starts_with("class ") => {
                let at = items
                    .iter()
                    .rposition(|i| {
                        key(i).is_some_and(|k| k.starts_with("def ") || k.starts_with("class "))
                    })
                    .map(|i| i + 1)
                    .unwrap_or_else(|| {
                        items
                            .iter()
                            .take_while(|i| {
                                i.starts_with("import ")
                                    || i.starts_with("from ")
                                    || i.trim().is_empty()
                                    || i.starts_with('#')
                            })
                            .count()
                    });
                if at > 0 && !items[at - 1].ends_with("\n\n") {
                    items[at - 1].push('\n');
                }
                if at < items.len() {
                    body.push('\n');
                }
                items.insert(at, body);
                done.push(format!("added {k}"));
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
        assert_eq!(
            out,
            "fn a() -> int:\n  1\n\nfn b() -> int:\n  3\n\nprint(a())\n\nfn c() -> int:\n  4\n"
        );
        assert_eq!(done, vec!["replaced fn b", "added fn c"]);
        let (out, _) = super::patch(&out, "del fn c\n").unwrap();
        assert!(!out.contains("fn c"));
        assert!(super::patch(src, "x = 1\n").is_err());
        assert!(super::patch(src, "del fn zz\n").is_err());
    }

    #[test]
    fn python_defs() {
        let src =
            "import sys\n\n@cache\ndef a(x):\n    return 1\n\nclass B:\n    pass\n\nprint(a(1))\n";
        let (out, done) = super::patch(
            src,
            "@cache\ndef a(x):\n    return 2\ndef c():\n    return 3\n",
        )
        .unwrap();
        assert_eq!(
            out,
            "import sys\n\n@cache\ndef a(x):\n    return 2\n\nclass B:\n    pass\n\ndef c():\n    return 3\n\nprint(a(1))\n"
        );
        assert_eq!(done, vec!["replaced def a", "added def c"]);
        let (out, _) = super::patch(&out, "del def c\n").unwrap();
        assert!(!out.contains("def c"));
        let (out, _) = super::patch("import os\nprint(f())\n", "def f():\n    return 1\n").unwrap();
        assert_eq!(out, "import os\n\ndef f():\n    return 1\n\nprint(f())\n");
    }
}
