// Python-style .bv files: what in them is Brevi's and not Python's (main.rs's bv_as_py builds a file with none
// of it as Python), and the logical lines of Python source that tells it.

// A logical line of Python source: from its first character to its end, past brackets and triple-quoted
// strings that run over several physical lines. Comments are left out of `text`.
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

// Whether a statement is a bare value: no assignment, no statement keyword, not `...`, and (unless `strings`)
// not a string alone, which may be a docstring. `call` says whether it is a call (`f(x)`, `await g()`), which Python code ends a function with
// too.
pub fn bare_value(t: &str, strings: bool) -> Option<bool> {
    let w = first_word(t);
    if STMT_WORDS.contains(&w) || t == "..." || (!strings && (t.starts_with('"') || t.starts_with('\''))) || t.starts_with('@') {
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
pub fn tails(lines: &[Line], strings: bool) -> Vec<usize> {
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
        block_tails(lines, i + 1, j, body, strings, &mut out);
    }
    out
}

// The bare-value tails of the block lines[from..to] (statements at `indent`).
fn block_tails(lines: &[Line], from: usize, to: usize, indent: usize, strings: bool, out: &mut Vec<usize>) {
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
                block_tails(lines, h + 1, stop, inner, strings, out);
            }
        }
        return;
    }
    if bare_value(t, strings).is_some() {
        out.push(last);
    }
}

// A function in Brevi's manner: one that ends with a bare value that is not a call (`x * 2`, `total`), which
// Python would drop and Brevi returns.
pub fn returns_tail(src: &str) -> bool {
    let lines = logical(src);
    tails(&lines, false).iter().any(|&k| bare_value(&lines[k].text, false) == Some(false))
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

// A Python-style .bv file's Brevi spellings in Python's: `fn` as `def` (and `fn f(x) = e` as `def f(x): return e`),
// a function's last bare value returned, `x => e` as `lambda x: e`, interpolating strings as f-strings, `f(n: 1)`
// as `f(n=1)`, `&&` `||` `!` as `and` `or` `not`, `true`/`false`, `.push(` as `.append(` and `for i in a..b:` as
// `range(a, b)`. Each has exactly the Python meaning a Python-style file's typed build gives it. Lines stay where
// they were. None when the file has a spelling this cannot write as Python (`(a, b) => ...`, a raw string that
// interpolates).
pub fn to_python(src: &str) -> Option<String> {
    let b = src.as_bytes();
    let toks = tokens(src);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let defines = |n: &str| toks.windows(2).any(|w| matches!(w[0].1, T::Name) && matches!(&src[w[0].0.0..w[0].0.1], "def" | "fn") && &src[w[1].0.0..w[1].0.1] == n);
    let push_ok = !defines("push");
    // Brackets open at each token: (kind, index of the token before it).
    let mut stack: Vec<(u8, Option<usize>)> = Vec::new();
    let mut lambda_depth: Option<usize> = None;
    let mut line_start = true;
    let text = |k: usize| &src[toks[k].0.0..toks[k].0.1];
    for k in 0..toks.len() {
        let ((s, e), t) = (toks[k].0, toks[k].1);
        let starts = line_start;
        line_start = false;
        let prev = k.checked_sub(1).map(text).unwrap_or("");
        match t {
            T::Nl => {
                line_start = stack.is_empty();
                if !stack.is_empty() {
                    line_start = false;
                }
            }
            T::Str => {
                let q = s + src[s..e].find(['"', '\'']).unwrap_or(0);
                let prefix = &src[s..q];
                if prefix.is_empty() {
                    if let Some(f) = fstring(&src[s..e]) {
                        edits.push((s, e, f));
                    }
                } else if !prefix.contains(['f', 'F']) && interp_at(&b[q..e]).is_some() && !prefix.contains(['b', 'B']) {
                    return None;
                }
            }
            T::Op => {
                let o = &src[s..e];
                match o {
                    "(" | "[" | "{" => stack.push((o.as_bytes()[0], k.checked_sub(1))),
                    ")" | "]" | "}" => {
                        stack.pop();
                        if lambda_depth.is_some_and(|d| d > stack.len()) {
                            lambda_depth = None;
                        }
                    }
                    "=>" => {
                        // `x => e`: the name before it, at the start of an argument or value.
                        let ok = k >= 1 && matches!(toks[k - 1].1, T::Name) && k >= 2 && matches!(text(k - 2), "(" | "," | "=" | "[" | "{" | ":" | "return");
                        if !ok {
                            return None;
                        }
                        let (ns, ne) = toks[k - 1].0;
                        let name = src[ns..ne].to_string();
                        edits.push((ns, e, format!("lambda {name}:")));
                    }
                    "&&" => edits.push((s, e, "and".into())),
                    "||" => edits.push((s, e, "or".into())),
                    "!" => edits.push((s, e, "not ".into())),
                    ":" => {
                        if lambda_depth == Some(stack.len()) {
                            lambda_depth = None;
                        } else if k >= 2 && matches!(toks[k - 1].1, T::Name) && matches!(text(k - 2), "(" | ",") && call_paren(&stack, &toks, src) {
                            edits.push((s, e, "=".into()));
                        }
                    }
                    ".." => {
                        // `for i in a..b`, in a statement or a comprehension: `range(a, b)`.
                        let at_depth = |from: usize, to: usize, fwd: bool| -> Option<usize> {
                            let mut d = 0i32;
                            let mut j = from;
                            loop {
                                let w = text(j);
                                if matches!(toks[j].1, T::Op) && matches!(w, "(" | "[" | "{") {
                                    d += if fwd { 1 } else { -1 };
                                } else if matches!(toks[j].1, T::Op) && matches!(w, ")" | "]" | "}") {
                                    d += if fwd { -1 } else { 1 };
                                }
                                if d < 0 || matches!(toks[j].1, T::Nl) {
                                    return Some(j);
                                }
                                if d == 0 && (if fwd { matches!(w, ":" | "," | "if") } else { w == "in" }) && matches!(toks[j].1, T::Op | T::Name) {
                                    return Some(j);
                                }
                                if j == to {
                                    return None;
                                }
                                j = if fwd { j + 1 } else { j - 1 };
                            }
                        };
                        let inn = at_depth(k - 1, 0, false).filter(|&j| text(j) == "in")?;
                        let stop = at_depth(k + 1, toks.len() - 1, true)?;
                        if inn + 1 >= k || stop <= k + 1 {
                            return None;
                        }
                        edits.push((toks[inn + 1].0.0, toks[inn + 1].0.0, "range(".into()));
                        edits.push((s, e, ", ".into()));
                        let end = toks[stop - 1].0.1;
                        edits.push((end, end, ")".into()));
                    }
                    _ => {}
                }
            }
            T::Name => {
                let w = &src[s..e];
                let after_dot = prev == ".";
                match w {
                    "fn" if (starts || prev == "async") && toks.get(k + 1).is_some_and(|n| matches!(n.1, T::Name)) => {
                        edits.push((s, e, "def".into()));
                        // `fn f(x) = e`: the `=` after the parameters.
                        let mut j = k + 2;
                        if text(j) != "(" {
                            return None;
                        }
                        let mut d = 0;
                        while j < toks.len() {
                            match text(j) {
                                "(" | "[" | "{" => d += 1,
                                ")" | "]" | "}" => {
                                    d -= 1;
                                    if d == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        let mut m = j + 1;
                        while m < toks.len() && !matches!(toks[m].1, T::Nl) && !matches!(text(m), "=" | ":") {
                            m += 1;
                        }
                        if m < toks.len() && text(m) == "=" {
                            edits.push((toks[m].0.0, toks[m].0.1, ": return".into()));
                        }
                    }
                    "lambda" => lambda_depth = Some(stack.len()),
                    "true" if !after_dot => edits.push((s, e, "True".into())),
                    "false" if !after_dot => edits.push((s, e, "False".into())),
                    "push" if after_dot && push_ok && toks.get(k + 1).is_some_and(|n| &src[n.0.0..n.0.1] == "(") => edits.push((s, e, "append".into())),
                    _ => {}
                }
            }
            T::Other => {}
        }
    }
    // A function's last bare value is what it returns (not in `__init__` or a generator).
    let lines = logical(src);
    for k in tails(&lines, true) {
        let owner = (0..k).rev().find(|&h| lines[h].indent < lines[k].indent && matches!(first_word(lines[h].text.trim_start_matches("async ").trim_start()), "def" | "fn"));
        let Some(h) = owner else { continue };
        let body_end = (h + 1..lines.len()).find(|&j| lines[j].indent <= lines[h].indent).unwrap_or(lines.len());
        let yields = (h + 1..body_end).any(|j| lines[j].text.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == "yield"));
        if lines[h].text.contains("__init__") || yields {
            continue;
        }
        edits.push((lines[k].start, lines[k].start, "return ".into()));
    }
    edits.sort_by_key(|x| (x.0, x.1));
    let mut out = String::with_capacity(src.len() + 64);
    let mut at = 0;
    for (s, e, r) in edits {
        if s < at {
            return None;
        }
        out.push_str(&src[at..s]);
        out.push_str(&r);
        at = e;
    }
    out.push_str(&src[at..]);
    Some(out)
}

// Whether the innermost open bracket is a call's `(` (after a name, `)` or `]`), not a def's or class's.
fn call_paren(stack: &[(u8, Option<usize>)], toks: &[((usize, usize), T)], src: &str) -> bool {
    let Some(&(b'(', Some(p))) = stack.last() else { return false };
    let t = &src[toks[p].0.0..toks[p].0.1];
    let before = p.checked_sub(1).map(|q| &src[toks[q].0.0..toks[q].0.1]).unwrap_or("");
    (matches!(toks[p].1, T::Name) && !matches!(before, "def" | "fn" | "class") && !matches!(t, "if" | "in" | "and" | "or" | "not" | "return" | "elif" | "while" | "print")) || matches!(t, ")" | "]")
}

// A Brevi string literal (no prefix) as a Python f-string, when it interpolates; None when it does not.
fn fstring(lit: &str) -> Option<String> {
    let b = lit.as_bytes();
    interp_at(b)?;
    let q = b[0];
    let triple = b.len() >= 6 && b[1] == q && b[2] == q;
    let (open, close) = if triple { (3, b.len() - 3) } else { (1, b.len().saturating_sub(1)) };
    let body = &lit[open..close.max(open)];
    let bb = body.as_bytes();
    let mut out = String::from("f");
    out.push_str(&lit[..open]);
    let mut i = 0;
    while i < bb.len() {
        let c = bb[i];
        if c == b'\\' && matches!(bb.get(i + 1), Some(b'{' | b'}')) {
            out.push_str(if bb[i + 1] == b'{' { "{{" } else { "}}" });
            i += 2;
            continue;
        }
        if c == b'\\' {
            let n = body[i + 1..].chars().next().map_or(0, char::len_utf8);
            out.push_str(&body[i..i + 1 + n]);
            i += 1 + n;
            continue;
        }
        if (c == b'{' || c == b'}') && bb.get(i + 1) == Some(&c) {
            // In double quotes `{{` and `}}` are one brace; in single quotes two.
            let one = if c == b'{' { "{{" } else { "}}" };
            out.push_str(&if q == b'"' { one.to_string() } else { one.repeat(2) });
            i += 2;
            continue;
        }
        if c == b'{' && bb.get(i + 1).is_some_and(|&n| n.is_ascii_alphabetic() || n >= 0x80 || matches!(n, b'_' | b'(' | b'-')) {
            // The expression, to its `}`.
            let mut d = 0;
            let mut j = i + 1;
            while j < bb.len() {
                match bb[j] {
                    b'(' | b'[' | b'{' => d += 1,
                    b')' | b']' => d -= 1,
                    b'}' if d == 0 => break,
                    b'}' => d -= 1,
                    b'"' | b'\'' if bb[j] == q => return None,
                    _ => {}
                }
                j += 1;
            }
            if j >= bb.len() {
                return None;
            }
            out.push_str(&body[i..=j]);
            i = j + 1;
            continue;
        }
        match c {
            b'{' => out.push_str("{{"),
            b'}' => out.push_str("}}"),
            _ => {
                let n = body[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&body[i..i + n]);
                i += n;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&lit[close.max(open)..]);
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum T {
    Name,
    Str,
    Op,
    Nl,
    Other,
}

// Python-ish tokens with their byte spans; comments and line continuations are left out.
fn tokens(src: &str) -> Vec<((usize, usize), T)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'\\' && b.get(i + 1) == Some(&b'\n') {
            i += 2;
            continue;
        }
        if c == b'\n' {
            out.push(((i, i + 1), T::Nl));
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 0x80) {
                i += 1;
            }
            // A string prefix (`f"..."`, `rb'...'`).
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') && i - s <= 2 && src[s..i].chars().all(|c| "rRbBfFuU".contains(c)) {
                let e = string_end(b, i);
                out.push(((s, e), T::Str));
                i = e;
                continue;
            }
            out.push(((s, i), T::Name));
            continue;
        }
        if c == b'"' || c == b'\'' {
            let e = string_end(b, i);
            out.push(((i, e), T::Str));
            i = e;
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && !(i > 0 && b[i - 1] == b'.')) {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || (b[i] == b'.' && b.get(i + 1) != Some(&b'.') && !(i > s && b[i - 1] == b'.'))) {
                i += 1;
            }
            out.push(((s, i), T::Other));
            continue;
        }
        const OPS: [&str; 22] = ["**=", "//=", "...", "=>", "&&", "||", "..", "==", "!=", "<=", ">=", "->", "**", "//", "+=", "-=", "*=", "/=", "%=", ":=", "<<", ">>"];
        if let Some(o) = OPS.iter().find(|o| src[i..].starts_with(**o)) {
            out.push(((i, i + o.len()), T::Op));
            i += o.len();
            continue;
        }
        let n = src[i..].chars().next().map_or(1, char::len_utf8);
        out.push(((i, i + n), T::Op));
        i += n;
    }
    out
}

#[cfg(test)]
mod rewrite_tests {
    use super::to_python;

    #[test]
    fn spellings() {
        let p = |s: &str| to_python(s).unwrap();
        assert_eq!(p("fn f(x):\n  x * 2\n"), "def f(x):\n  return x * 2\n");
        assert_eq!(p("fn f(x) = x * 2\n"), "def f(x) : return x * 2\n");
        assert_eq!(p("async fn f(x):\n  await g(x)\n"), "async def f(x):\n  return await g(x)\n");
        assert_eq!(p("ys = map(x => x + 1, xs)\n"), "ys = map(lambda x: x + 1, xs)\n");
        assert_eq!(p("print(\"n={n} {{ok}}\")\n"), "print(f\"n={n} {{ok}}\")\n");
        assert_eq!(p("print('{\"id\":{i}}')\n"), "print(f'{{\"id\":{i}}}')\n");
        assert_eq!(p("p = Point(x: 1, y: 2)\n"), "p = Point(x= 1, y= 2)\n");
        assert_eq!(p("def f(x: int) -> int:\n  return x\n"), "def f(x: int) -> int:\n  return x\n");
        assert_eq!(p("if a && !b || true:\n  xs.push(1)\n"), "if a and not b or True:\n  xs.append(1)\n");
        assert_eq!(p("for i in 0..n:\n  print(i)\n"), "for i in range(0, n):\n  print(i)\n");
        assert_eq!(p("f(lambda a, b: a)\nd = {k: 1}\n"), "f(lambda a, b: a)\nd = {k: 1}\n");
        assert_eq!(p("class A:\n  fn __init__(self):\n    self.reset()\n"), "class A:\n  def __init__(self):\n    self.reset()\n");
        assert_eq!(p("xs = [i * 2 for i in 0..n if i]\n"), "xs = [i * 2 for i in range(0, n) if i]\n");
        assert_eq!(p("ys = [f(i) for i in (a + 1)..len(xs)]\n"), "ys = [f(i) for i in range((a + 1), len(xs))]\n");
        assert!(to_python("f((a, b) => a)\n").is_none());
    }
}
