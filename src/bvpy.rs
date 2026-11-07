// Python-style .bv files: what in them is Brevi's and not Python's (main.rs's bv_as_py builds a file with none
// of it as Python), and the logical lines of Python source that tells it.

// A logical line of Python source: from its first character to its end, past brackets and triple-quoted
// strings that run over several physical lines. Comments are left out of `text`.
#[allow(dead_code)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub indent: usize,
    pub text: String,
}

// Where each string literal starts and ends (byte offsets, quotes and prefix included), and the logical lines.
pub fn logical(src: &str) -> Vec<Line> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        // A new physical line: its indent.
        let ls = i;
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        let indent = i - ls;
        if i >= b.len() {
            break;
        }
        if b[i] == b'\n' || b[i] == b'\r' || b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            i += 1;
            continue;
        }
        let start = i;
        let mut text = String::new();
        let mut depth = 0i32;
        let mut end = i;
        while i < b.len() {
            let c = b[i];
            if c == b'#' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if c == b'"' || c == b'\'' {
                let (s, e) = (i, string_end(b, i));
                text.push_str(&src[s..e]);
                i = e;
                end = e;
                continue;
            }
            if c == b'\\' && b.get(i + 1) == Some(&b'\n') {
                text.push(' ');
                i += 2;
                continue;
            }
            if c == b'\n' {
                if depth > 0 {
                    text.push(' ');
                    i += 1;
                    continue;
                }
                break;
            }
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            let ch = src[i..].chars().next().unwrap_or(' ');
            text.push(ch);
            i += ch.len_utf8();
            if !ch.is_whitespace() {
                end = i;
            }
        }
        out.push(Line { start, end, indent, text: text.trim_end().to_string() });
        i += 1;
    }
    out
}

// The end (exclusive) of the string literal whose opening quote is at `q`.
pub fn string_end(b: &[u8], q: usize) -> usize {
    let c = b[q];
    let triple = b.get(q + 1) == Some(&c) && b.get(q + 2) == Some(&c);
    let mut j = if triple { q + 3 } else { q + 1 };
    while j < b.len() {
        if b[j] == b'\\' {
            j += 2;
            continue;
        }
        if b[j] == c && (!triple || (b.get(j + 1) == Some(&c) && b.get(j + 2) == Some(&c))) {
            return if triple { j + 3 } else { j + 1 };
        }
        if !triple && b[j] == b'\n' {
            return j;
        }
        j += 1;
    }
    b.len()
}

const STMT_WORDS: &[&str] = &[
    "return", "if", "elif", "else", "for", "while", "try", "except", "finally", "with", "raise", "pass", "break", "continue", "del", "global", "nonlocal", "assert", "import", "from",
    "class", "def", "fn", "async", "match", "case", "yield", "type", "enum", "test", "let", "var", "print",
];

// The first word of a logical line.
fn first_word(t: &str) -> &str {
    let e = t.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(t.len());
    &t[..e]
}

// Whether a statement is a bare value: no assignment, no statement keyword, not a string alone (a docstring)
// and not `...`. `call` says whether it is a call (`f(x)`, `await g()`), which Python code ends a function with
// too.
pub fn bare_value(t: &str) -> Option<bool> {
    let w = first_word(t);
    if STMT_WORDS.contains(&w) || t == "..." || t.starts_with('"') || t.starts_with('\'') || t.starts_with('@') {
        return None;
    }
    let b = t.as_bytes();
    let mut depth = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            i = string_end(b, i);
            continue;
        }
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth > 0 => {}
            b'=' => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if !matches!(prev, b'=' | b'!' | b'<' | b'>') && !matches!(next, b'=' | b'>') {
                    return None;
                }
            }
            b':' => return None,
            _ => {}
        }
        i += 1;
    }
    Some(is_call(t.strip_prefix("await ").unwrap_or(t)))
}

// `f(x)`, `a.b[0].c(y)`: a chain of names, fields and indexes ending in a call.
fn is_call(t: &str) -> bool {
    let b = t.as_bytes();
    if !t.ends_with(')') {
        return false;
    }
    let mut depth = 0;
    let mut i = 0;
    let mut last_open = None;
    let mut plain = true;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            if depth == 0 {
                plain = false;
            }
            i = string_end(b, i);
            continue;
        }
        match c {
            b'(' | b'[' | b'{' => {
                if depth == 0 && c == b'(' {
                    last_open = Some(i);
                }
                depth += 1;
            }
            b')' | b']' | b'}' => depth -= 1,
            _ if depth > 0 => {}
            c if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c >= 0x80 => {}
            _ => plain = false,
        }
        i += 1;
    }
    plain && last_open.is_some_and(|k| k > 0)
}

// Functions whose last statement is a bare value, each as the logical line of that statement (index into
// `lines`). Brevi returns that value; Python drops it. A function's last `if` or `with` is looked into.
pub fn tails(lines: &[Line]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let w = first_word(&l.text);
        let header = matches!(w, "def" | "fn") || (w == "async" && matches!(first_word(l.text[5..].trim_start()), "def" | "fn"));
        if !header || !l.text.ends_with(':') {
            continue;
        }
        let Some(body) = lines.get(i + 1).filter(|b| b.indent > l.indent).map(|b| b.indent) else { continue };
        let mut j = i + 1;
        while j < lines.len() && lines[j].indent > l.indent {
            j += 1;
        }
        block_tails(lines, i + 1, j, body, &mut out);
    }
    out
}

// The bare-value tails of the block lines[from..to] (statements at `indent`).
fn block_tails(lines: &[Line], from: usize, to: usize, indent: usize, out: &mut Vec<usize>) {
    let Some(last) = (from..to).rev().find(|&k| lines[k].indent == indent) else { return };
    let t = &lines[last].text;
    let w = first_word(t);
    if matches!(w, "if" | "elif" | "else" | "with") && t.ends_with(':') {
        // The branches of the last if: each `if`/`elif`/`else` at this indent from the `if` down.
        let mut k = last;
        while k > from && !(lines[k].indent == indent && first_word(&lines[k].text) == "if") {
            k -= 1;
        }
        let heads: Vec<usize> = (k..to).filter(|&h| lines[h].indent == indent).collect();
        for (n, &h) in heads.iter().enumerate() {
            let stop = heads.get(n + 1).copied().unwrap_or(to);
            if let Some(inner) = lines.get(h + 1).filter(|x| x.indent > indent && h + 1 < stop).map(|x| x.indent) {
                block_tails(lines, h + 1, stop, inner, out);
            }
        }
        return;
    }
    if bare_value(t).is_some() {
        out.push(last);
    }
}

// A function in Brevi's manner: one that ends with a bare value that is not a call (`x * 2`, `total`), which
// Python would drop and Brevi returns.
pub fn returns_tail(src: &str) -> bool {
    let lines = logical(src);
    tails(&lines).iter().any(|&k| bare_value(&lines[k].text) == Some(false))
}

// Whether a plain (not f-) string literal holds `{name`, `{(`, which Brevi interpolates and Python does not.
pub fn interpolates(src: &str) -> bool {
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' | b'\'' => {
                let mut k = i;
                let mut f = false;
                while k > 0 && b[k - 1].is_ascii_alphabetic() {
                    k -= 1;
                    f |= matches!(b[k], b'f' | b'F');
                }
                let e = string_end(b, i);
                if !f && interp_at(&b[i..e]).is_some() {
                    return true;
                }
                i = e;
            }
            _ => i += 1,
        }
    }
    false
}

// The first `{` in a string literal's text that starts Brevi interpolation.
fn interp_at(s: &[u8]) -> Option<usize> {
    let mut j = 0;
    while j < s.len() {
        if s[j] == b'\\' {
            j += 2;
            continue;
        }
        if s[j] == b'{' && s.get(j + 1).is_some_and(|&n| n.is_ascii_alphabetic() || n >= 0x80 || matches!(n, b'_' | b'(')) {
            return Some(j);
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tails_found() {
        assert!(returns_tail("def f(x):\n    x * 2\n"));
        assert!(returns_tail("def f(x):\n    if x:\n        1\n    else:\n        print(x)\n"));
        assert!(!returns_tail("def f(x):\n    print(x)\n"));
        assert!(!returns_tail("def f(x):\n    \"\"\"doc\"\"\"\n"));
        assert!(!returns_tail("def f(x):\n    y = x\n    for i in x:\n        i\n"));
        assert!(!returns_tail("def f(x):\n    return (x +\n        1)\n"));
        assert!(!returns_tail("def f(x):\n    xs.append(x)\n"));
        assert!(!returns_tail("async def f(x):\n    await asyncio.sleep(1)\n"));
        assert!(returns_tail("def f(x):\n    a.b\n"));
    }

    #[test]
    fn interpolation() {
        assert!(interpolates("print(\"n {n}\")"));
        assert!(!interpolates("print(f\"n {n}\")"));
        assert!(!interpolates("print(\"{} {:>3}\".format(1, 2))"));
        assert!(!interpolates("# \"{n}\"\nx = 1"));
    }
}
