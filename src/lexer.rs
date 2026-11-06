use crate::diag::Diag;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Kw(&'static str),
    Sym(&'static str),
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub col: usize,
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "type", "enum", "if", "else", "for", "in", "while", "match", "return",
    "true", "false", "mut", "test", "assert", "break", "continue", "and", "or", "not",
];

// Names Rust's prelude gives to traits and types: a Brevi `type Ord` must not clash with them.
const RUST_PRELUDE: &[&str] = &[
    "Ord",
    "PartialOrd",
    "Eq",
    "PartialEq",
    "Clone",
    "Copy",
    "Default",
    "Debug",
    "Hash",
    "Iterator",
    "Drop",
    "Send",
    "Sync",
    "Sized",
    "From",
    "Into",
    "Vec",
    "String",
    "Box",
    "Option",
    "ToString",
    "Fn",
    "FnMut",
    "FnOnce",
    "Display",
    "Result",
];

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "const", "crate", "dyn", "extern", "impl", "loop", "mod",
    "move", "priv", "pub", "ref", "self", "static", "struct", "super", "trait", "try", "typeof",
    "unsafe", "use", "where", "yield", "macro", "final", "override", "virtual", "become",
    "abstract", "do", "unsized",
];

// "{{#each items}}", "{{/if}}", "{{else}}", or "{{#each " at a string's end: where the tag's text ends,
// and whether `}}` follows it.
fn template_tag(chars: &[char], from: usize) -> Option<(usize, bool)> {
    let first = *chars.get(from)?;
    let tag_start = matches!(first, '#' | '/' | '@' | '>' | '!');
    let mut j = from;
    while let Some(&c) = chars.get(j) {
        if c == '}' {
            break;
        }
        if c == '"' && tag_start && j > from {
            return Some((j, false));
        }
        if c == '"' || c == '\\' || c == '{' || c == '\n' {
            return None;
        }
        j += 1;
    }
    if chars.get(j) != Some(&'}') || chars.get(j + 1) != Some(&'}') || j == from {
        return None;
    }
    let text: String = chars[from..j].iter().collect();
    (tag_start || text == "else").then_some((j, true))
}

// Is the quote at `q` the start of a method's first argument: `.name(` just before it?
fn method_arg_start(chars: &[char], q: usize) -> bool {
    let mut j = q;
    while j > 0 && chars[j - 1] == ' ' {
        j -= 1;
    }
    if j == 0 || chars[j - 1] != '(' {
        return false;
    }
    j -= 1;
    let end = j;
    while j > 0 && (chars[j - 1].is_alphanumeric() || chars[j - 1] == '_') {
        j -= 1;
    }
    j < end && j > 0 && chars[j - 1] == '.'
}

// Does the `{` just before `from` close on this line (quoted strings inside skipped)?
// `7`, `3.14` or `1_000` right after an f-string's `{`, up to a `:` or `}`: where the number ends.
fn num_interp(chars: &[char], from: usize) -> Option<usize> {
    let mut j = from;
    while chars
        .get(j)
        .map_or(false, |c| c.is_ascii_digit() || matches!(c, '.' | '_'))
    {
        j += 1;
    }
    // An exponent: `{1e16:g}`, `{2.5E-3}`.
    if j > from && matches!(chars.get(j), Some('e' | 'E')) {
        let mut k = j + 1;
        if matches!(chars.get(k), Some('+' | '-')) {
            k += 1;
        }
        if chars.get(k).is_some_and(|c| c.is_ascii_digit()) {
            while chars.get(k).is_some_and(|c| c.is_ascii_digit()) {
                k += 1;
            }
            j = k;
        }
    }
    let mut k = j;
    while chars.get(k) == Some(&' ') {
        k += 1;
    }
    (j > from && chars[from].is_ascii_digit() && matches!(chars.get(k), Some(':' | '}')))
        .then_some(j)
}

// Python escapes (in .py files only: Brevi code keeps "\\1" for regex backreferences) that stand for one character: \x41, \u00e9, \U0001F600, \N{BULLET}, octal \101, \a \b \f \v.
// Returns the character and how many chars the escape takes, backslash included.
fn char_escape(chars: &[char], i: usize) -> Option<(char, usize)> {
    let hex = |n: usize| {
        let h: String = chars.get(i + 2..i + 2 + n)?.iter().collect();
        if h.chars().all(|c| c.is_ascii_hexdigit()) {
            u32::from_str_radix(&h, 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| (c, n + 2))
        } else {
            None
        }
    };
    match *chars.get(i + 1)? {
        'x' => hex(2),
        'u' => hex(4),
        'U' => hex(8),
        'a' => Some(('\x07', 2)),
        'b' => Some(('\x08', 2)),
        'f' => Some(('\x0c', 2)),
        'v' => Some(('\x0b', 2)),
        'N' => crate::unicode_names::escape(&chars[i + 2..]).map(|(c, n)| (c, n + 2)),
        '0'..='7' => {
            let n = chars[i + 1..]
                .iter()
                .take(3)
                .take_while(|c| ('0'..='7').contains(c))
                .count();
            let v =
                u32::from_str_radix(&chars[i + 1..i + 1 + n].iter().collect::<String>(), 8).ok()?;
            char::from_u32(v).map(|c| (c, n + 1))
        }
        _ => None,
    }
}

// A `\N` in a .py string that char_escape could not read: left as text it would print the wrong thing.
fn bad_named_escape(line: usize, col: usize) -> Diag {
    Diag::new("lex", line, col, "cannot read this \\N{...} escape")
        .fix_hint("write the character itself, or check the name")
}

fn interp_closes(chars: &[char], from: usize) -> bool {
    // `{3}` or `{2,4}` is a regex count, not a value.
    let count: String = chars[from..]
        .iter()
        .take_while(|c| **c != '}' && **c != '"' && **c != '\n')
        .collect();
    if !count.is_empty()
        && count.chars().all(|c| c.is_ascii_digit() || c == ',')
        && chars.get(from + count.len()) == Some(&'}')
    {
        return false;
    }
    let (mut depth, mut in_str, mut i) = (1, false, from);
    while let Some(&c) = chars.get(i) {
        match c {
            '\n' => return false,
            '\\' if in_str => i += 1,
            // A quote right after a value ends the outer string, so this `{` was not an interpolation.
            '"' if !in_str
                && chars[..i]
                    .iter()
                    .rev()
                    .find(|c| **c != ' ')
                    .map_or(false, |p| {
                        p.is_alphanumeric() || matches!(p, '_' | ')' | ']')
                    }) =>
            {
                return false;
            }
            '"' if in_str => {
                in_str = false;
                // After a string inside `{}` comes an operator or the closing brace, not more text.
                if chars[i + 1..]
                    .iter()
                    .find(|c| **c != ' ')
                    .map_or(true, |c| {
                        c.is_alphanumeric() || matches!(c, ':' | '"' | '\n')
                    })
                {
                    return false;
                }
            }
            '"' => in_str = true,
            '{' if !in_str => depth += 1,
            '}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

// Longest symbols first so the scanner can take the first match.
const SYMBOLS: &[&str] = &[
    "**", "<<=", ">>=", "->", "=>", "==", "!=", "<=", ">=", "<<", ">>", "%=", "|=", "&=", "^=",
    "+=", "-=", "*=", "/=", "~=", "..", "(", ")", "[", "]", "{", "}", ",", ":", ".", "+", "-", "*",
    "/", "%", "<", ">", "=", "!", "?", ";", "|", "&", "^", "@", "~",
];

// After a value, `a // b` is floor division unless what follows reads like words (`x = 5 // the count`).
pub(crate) fn looks_like_prose(rest: &str) -> bool {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let is_word = |w: &str| {
        w.chars().all(|c| c.is_alphanumeric() || c == '_')
            && !["and", "or", "not", "if", "else", "in"].contains(&w)
    };
    rest.trim().is_empty() || words.windows(2).any(|p| is_word(p[0]) && is_word(p[1]))
}

// Python imports of modules Brevi has built in (`import sys`, `from collections import defaultdict`)
// are dropped; the line stays so line numbers hold.
const PY_BUILTIN_MODULES: &[&str] = &[
    "sys",
    "math",
    "collections",
    "typing",
    "dataclasses",
    "re",
    "functools",
    "itertools",
];

// `json`, `os.path`, `xml.etree.ElementTree as ET`: a dotted module name, maybe with `as NAME`.
fn is_module_path(m: &str) -> bool {
    let name = m.split(" as ").next().unwrap_or("").trim();
    !name.is_empty()
        && name
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

thread_local! {
    // Python classes under `@dataclass`: the `class` line (1-based) -> the class's Python name. The parser reads it
    // (dataclass_at) so the class prints as CPython's generated __repr__ does: `P(x=1, y=0)`.
    static DATACLASS: std::cell::RefCell<std::collections::HashMap<usize, String>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn dataclass_at(line: usize) -> Option<String> {
    DATACLASS.with(|d| d.borrow().get(&line).cloned())
}

// The Python name a lexed name stands for: `Box_` (renamed for Rust) is `Box`.
pub fn py_ident(n: &str) -> &str {
    match n.strip_suffix('_') {
        Some(k)
            if RUST_KEYWORDS.contains(&k) || RUST_PRELUDE.contains(&k) || KEYWORDS.contains(&k) =>
        {
            k
        }
        _ => n,
    }
}

// `@dataclass`, `@dataclasses.dataclass`, `@dataclass(order=True)`, but not `@dataclass(repr=False)`.
fn dataclass_decorator(t: &str) -> bool {
    let d = t.trim_start_matches('@').trim_start_matches("dataclasses.");
    let Some(rest) = d.strip_prefix("dataclass") else {
        return false;
    };
    let rest = rest.split('#').next().unwrap_or("").trim();
    (rest.is_empty() || rest.starts_with('(')) && !rest.replace(' ', "").contains("repr=False")
}

fn drop_py_imports(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    // Inside `from m import (a,` ... `b)`: the names run on to the closing bracket.
    let mut open = false;
    let all: Vec<&str> = src.split_inclusive('\n').collect();
    // The def a memo decorator wraps: (its line, its name).
    let mut memo: Option<(usize, String)> = None;
    for (at, l) in all.iter().enumerate() {
        let l = *l;
        let t = l.trim();
        if open {
            open = !t.contains(')');
            out.push_str(if l.ends_with('\n') { "\n" } else { "" });
            continue;
        }
        // `@lru_cache` / `@cache` on a def: the def becomes NAME__raw, and NAME looks results up in a
        // dict first, so a memoised recursion stays fast (dropping the decorator would make it exponential).
        if let Some(w) = memo_wrapper(t, &all[at + 1..], at) {
            let ind = &l[..l.len() - l.trim_start().len()];
            out.push_str(&format!(
                "{ind}{}{}",
                w.1.replace('\u{1}', &format!("\u{1}{ind}")),
                if l.ends_with('\n') { "\n" } else { "" }
            ));
            memo = Some((w.0, w.2));
            continue;
        }
        if let Some((n, name)) = &memo {
            if *n == at {
                let i = l.find("def ").unwrap_or(0) + 4;
                out.push_str(&format!("{}{name}__raw{}", &l[..i], &l[i + name.len()..]));
                memo = None;
                continue;
            }
        }
        // Decorators (`@dataclass`, `@staticmethod`, `@lru_cache(None)`): the code works the same without them,
        // except that a dataclass prints its fields (noted for the parser).
        if t.starts_with('@') && t[1..].chars().next().map_or(false, |c| c.is_alphabetic()) {
            if dataclass_decorator(t) {
                let class = all[at + 1..]
                    .iter()
                    .enumerate()
                    .map(|(k, l)| (k, l.trim()))
                    .find(|(_, l)| !l.starts_with('@') && !l.is_empty() && !l.starts_with('#'));
                if let Some((k, c)) = class {
                    if let Some(rest) = c.strip_prefix("class ") {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        DATACLASS.with(|d| d.borrow_mut().insert(at + k + 2, name));
                    }
                }
            }
            out.push_str(if l.ends_with('\n') { "\n" } else { "" });
            continue;
        }
        let module = t
            .strip_prefix("import ")
            .map(|m| m.split(',').map(str::trim).collect::<Vec<_>>())
            .or_else(|| {
                let rest = t.strip_prefix("from ")?;
                let (m, _) = rest.split_once(" import ")?;
                Some(vec![m.trim()])
            });
        // Any Python import line: the modules Brevi maps are built in, and `import` means nothing else here.
        if module.is_some_and(|ms| {
            ms.iter()
                .all(|m| PY_BUILTIN_MODULES.contains(m) || is_module_path(m))
        }) {
            open = t.starts_with("from ") && t.contains('(') && !t.contains(')');
            out.push_str(if l.ends_with('\n') { "\n" } else { "" });
        } else {
            out.push_str(l);
        }
    }
    out
}

// `@lru_cache(...)`, `@cache`, `@functools.cache` (maybe under other decorators) on `def f(a, b):`:
// gives (the def's line, `ax_memo_f = {}` and the wrapper f joined by a soft newline, f).
// Methods, `*args` and parameter lists split over lines keep the old behaviour (the decorator is dropped).
fn memo_wrapper(t: &str, rest: &[&str], at: usize) -> Option<(usize, String, String)> {
    let d = t.strip_prefix('@')?.trim_start_matches("functools.");
    let d = d.split('(').next()?.trim();
    if d != "lru_cache" && d != "cache" {
        return None;
    }
    let (k, def) = rest
        .iter()
        .enumerate()
        .map(|(k, l)| (k, l.trim()))
        .find(|(_, l)| !l.starts_with('@'))?;
    let sig = def.strip_prefix("def ")?;
    let (name, after) = sig.split_once('(')?;
    let name = name.trim();
    let (params, _) = after.split_once(')')?;
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || !def.ends_with(':') {
        return None;
    }
    let names: Vec<String> = params
        .split(',')
        .map(|p| {
            p.split(&[':', '='][..])
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect();
    if names
        .iter()
        .any(|p| p.starts_with('*') || p == "self" || p == "cls" || p == "/")
    {
        return None;
    }
    let m = format!("ax_memo_{name}");
    let key = match names.len() {
        0 => "0".to_string(),
        1 => names[0].clone(),
        _ => format!("({})", names.join(", ")),
    };
    let args = names.join(", ");
    let w = [
        format!("{m} = {{}}"),
        format!("def {name}({args}):"),
        format!("    if {key} in {m}:"),
        format!("        return {m}[{key}]"),
        format!("    ax_memo_r = {name}__raw({args})"),
        format!("    {m}[{key}] = ax_memo_r"),
        "    return ax_memo_r".to_string(),
    ]
    .join("\u{1}");
    Some((at + 1 + k, w, name.to_string()))
}

// Python's triple-quoted strings (`"""doc"""`, multi-line text) become one-line strings with `\n` in them;
// the newlines they held move to the end of their last line, so later line numbers hold.
fn triple_strings(src: &str) -> String {
    if !src.contains("\"\"\"") && !src.contains("'''") && !src.contains("\\\n") {
        return src.to_string();
    }
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut owed = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '#' {
            while i < c.len() && c[i] != '\n' {
                out.push(c[i]);
                i += 1;
            }
            continue;
        }
        if ch == '\n' {
            for _ in 0..owed {
                out.push('\n');
            }
            owed = 0;
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let triple = c.get(i + 1) == Some(&ch) && c.get(i + 2) == Some(&ch);
            let raw = i > 0
                && matches!(c[i - 1], 'r' | 'R')
                && !(i > 1 && (c[i - 2].is_alphanumeric() || c[i - 2] == '_'));
            if triple && raw {
                // Kept whole for the raw-string reader.
                let mut j = i + 3;
                while j < c.len()
                    && !(c[j] == ch && c.get(j + 1) == Some(&ch) && c.get(j + 2) == Some(&ch))
                {
                    j += 1;
                }
                let end = (j + 3).min(c.len());
                out.extend(&c[i..end]);
                i = end;
                continue;
            }
            if triple {
                let mut j = i + 3;
                let mut body = String::new();
                while j < c.len()
                    && !(c[j] == ch && c.get(j + 1) == Some(&ch) && c.get(j + 2) == Some(&ch))
                {
                    match c[j] {
                        '\\' if j + 1 < c.len() => {
                            if c[j + 1] == '\n' {
                                owed += 1;
                            } else {
                                body.push('\\');
                                body.push(c[j + 1]);
                            }
                            j += 2;
                            continue;
                        }
                        '\n' => {
                            body.push_str("\\n");
                            owed += 1;
                        }
                        '"' => body.push_str("\\\""),
                        x => body.push(x),
                    }
                    j += 1;
                }
                if j >= c.len() {
                    out.extend(&c[i..]);
                    break;
                }
                out.push('"');
                out.push_str(&body);
                out.push('"');
                i = j + 3;
                continue;
            }
            // An ordinary string: copied as it is.
            out.push(ch);
            i += 1;
            while i < c.len() && c[i] != ch && c[i] != '\n' {
                if c[i] == '\\' && i + 1 < c.len() {
                    out.push(c[i]);
                    i += 1;
                }
                out.push(c[i]);
                i += 1;
            }
            if i < c.len() && c[i] == ch {
                out.push(ch);
                i += 1;
            }
            continue;
        }
        // Python's `\` at the end of a line joins the next line to it.
        if ch == '\\' && c.get(i + 1) == Some(&'\n') {
            out.push(' ');
            owed += 1;
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

pub fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    let src_in = src;
    let src = &triple_strings(&drop_py_imports(src));
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1usize, 1usize);
    // Newlines inside () or [] never end a statement.
    let mut depth: Vec<char> = Vec::new();
    // The string just ahead is an f-string, where `{3.14:.2f}` formats a number written in place.
    let mut fstr = false;
    // A .py file: only f-strings fill in `{...}`; braces in any other string are kept as written.
    let py = crate::PY_FILE.load(std::sync::atomic::Ordering::Relaxed);
    // Functions defined with Python's `def var(...)`: the name is a name everywhere, even where Brevi has a keyword.
    let py_defs: Vec<String> = src
        .split(['\n', '\u{1}'])
        .filter_map(|l| l.trim_start().strip_prefix("def "))
        .map(|r| {
            r.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect();

    while i < chars.len() {
        let c = chars[i];
        let (tl, tc) = (line, col);
        let push = |out: &mut Vec<Token>, tok: Tok| {
            out.push(Token {
                tok,
                line: tl,
                col: tc,
            })
        };

        if c == '\n' {
            if !matches!(depth.last(), Some('(') | Some('[')) {
                push(&mut out, Tok::Newline);
            }
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        // A soft newline (from memo_wrapper): a new statement on the same source line.
        if c == '\u{1}' {
            push(&mut out, Tok::Newline);
            i += 1;
            col = 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            col += 1;
            continue;
        }
        // `#` comments are accepted too, since models often write them.
        // `a // 2` after a value, followed by a number or `(`, is floor division (as in Python); otherwise `//` starts a comment.
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            let after_value = matches!(out.last(), Some(Token { tok: Tok::Int(_) | Tok::Float(_) | Tok::Ident(_) | Tok::Sym(")") | Tok::Sym("]"), line: l, .. }) if *l == line);
            let rest: String = chars[i + 2..].iter().take_while(|c| **c != '\n').collect();
            // Only up to a later comment on the same line.
            let rest = rest
                .split("//")
                .next()
                .unwrap_or("")
                .split('#')
                .next()
                .unwrap_or("")
                .to_string();
            if after_value && chars.get(i + 2) == Some(&'=') {
                push(&mut out, Tok::Sym("//="));
                i += 3;
                col += 3;
                continue;
            }
            if after_value && !looks_like_prose(&rest) {
                push(&mut out, Tok::Sym("//"));
                i += 2;
                col += 2;
                continue;
            }
        }
        if c == '#' || (c == '/' && chars.get(i + 1) == Some(&'/')) {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // 'text': quotes inside need no escape, and `{` is a value only before a name or `(` (`'{"id":{i}}'`).
        if c == '\'' {
            // An f'...' string's values may start with a quote or bracket too (`f'{"#" * n}'`).
            let is_f = std::mem::take(&mut fstr);
            let mut s = String::new();
            i += 1;
            col += 1;
            loop {
                match chars.get(i) {
                    None | Some('\n') => {
                        return Err(Diag::new("lex", tl, tc, "unterminated string")
                            .fix_hint("add a closing '"));
                    }
                    Some('\'') => {
                        i += 1;
                        col += 1;
                        break;
                    }
                    Some(&b @ ('{' | '}')) if py && !is_f => {
                        s.push(b);
                        s.push(b);
                        i += 1;
                        col += 1;
                    }
                    Some(&b @ ('{' | '}')) if py && chars.get(i + 1) == Some(&b) => {
                        s.push(b);
                        s.push(b);
                        i += 2;
                        col += 2;
                    }
                    // f'{7:03d}', f'{ x }': a number value is wrapped as `(7)`; spaces after the brace are dropped.
                    Some('{')
                        if py
                            && is_f
                            && chars
                                .get(i + 1)
                                .is_some_and(|c| *c == ' ' || c.is_ascii_digit()) =>
                    {
                        let j = (i + 1..chars.len())
                            .find(|&j| chars[j] != ' ')
                            .unwrap_or(chars.len());
                        s.push('{');
                        if let Some(end) = num_interp(&chars, j) {
                            s.push('(');
                            s.extend(&chars[j..end]);
                            s.push(')');
                            col += end - i;
                            i = end;
                        } else {
                            col += j - i;
                            i = j;
                        }
                        let (mut depth, mut inner) = (1usize, false);
                        while let Some(&ch) = chars.get(i) {
                            s.push(ch);
                            i += 1;
                            col += 1;
                            if inner {
                                if ch == '\\' {
                                    if let Some(&n) = chars.get(i) {
                                        s.push(n);
                                        i += 1;
                                        col += 1;
                                    }
                                } else if ch == '"' {
                                    inner = false;
                                }
                            } else if ch == '"' {
                                inner = true;
                            } else if ch == '{' {
                                depth += 1;
                            } else if ch == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                        }
                    }
                    Some('{')
                        if chars.get(i + 1).map_or(false, |c| {
                            c.is_alphabetic()
                                || matches!(c, '_' | '(')
                                || (is_f && matches!(c, '"' | '[' | '-' | '!'))
                        }) && interp_closes(&chars, i + 1) =>
                    {
                        // The value runs to its matching `}`; strings inside it are skipped.
                        let (mut depth, mut inner) = (0usize, false);
                        while let Some(&ch) = chars.get(i) {
                            s.push(ch);
                            i += 1;
                            col += 1;
                            if inner {
                                if ch == '\\' {
                                    if let Some(&n) = chars.get(i) {
                                        s.push(n);
                                        i += 1;
                                        col += 1;
                                    }
                                } else if ch == '"' {
                                    inner = false;
                                }
                            } else if ch == '"' {
                                inner = true;
                            } else if ch == '{' {
                                depth += 1;
                            } else if ch == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                        }
                    }
                    // `{{` is two braces here, never a value: '{{#each '.
                    Some('{') if chars.get(i + 1) == Some(&'{') => {
                        s.push_str("{{{{");
                        i += 2;
                        col += 2;
                    }
                    Some(&b @ ('{' | '}')) => {
                        s.push(b);
                        s.push(b);
                        i += 1;
                        col += 1;
                    }
                    Some('\\')
                        if py
                            && chars.get(i + 1) == Some(&'N')
                            && char_escape(&chars, i).is_none() =>
                    {
                        return Err(bad_named_escape(tl, tc));
                    }
                    Some('\\') if py && char_escape(&chars, i).is_some() => {
                        let (ch, n) = char_escape(&chars, i).unwrap();
                        s.push(ch);
                        if ch == '{' || ch == '}' {
                            s.push(ch);
                        }
                        i += n;
                        col += n;
                    }
                    // Python: a backslash at the end of a line inside a string joins the lines.
                    Some('\\') if py && chars.get(i + 1) == Some(&'\n') => {
                        i += 2;
                        line += 1;
                        col = 1;
                    }
                    Some('\\') => {
                        let e = chars.get(i + 1).copied().unwrap_or(' ');
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '0' => s.push('\0'),
                            '\'' | '\\' => s.push(e),
                            '"' if py => s.push(e),
                            '{' | '}' => {
                                s.push(e);
                                s.push(e);
                            }
                            other => {
                                s.push('\\');
                                s.push(other);
                            }
                        }
                        i += 2;
                        col += 2;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                        col += 1;
                    }
                }
            }
            push(&mut out, Tok::Str(s));
            continue;
        }
        if c == '"' {
            let is_f = std::mem::take(&mut fstr);
            let mut s = String::new();
            i += 1;
            col += 1;
            // Inside `{...}` the text is kept as written (quotes included), to be lexed again as an expression.
            let (mut depth, mut inner_str, mut esc_open, mut lone_open) =
                (0usize, false, 0usize, 0usize);
            // Inside a value, a 'single-quoted' string is skipped whole ("{'#'.repeat(n)}").
            let mut inner_sq = false;
            // Just after `\\`: a `{` here is a regex's escaped brace (`"\\{(\\d+)\\}"`), never a value.
            let mut after_bs = false;
            // `.split("{{")`, `.find("}}")`, `.push("{{" + p)`: a bare "{{" or "}}" handed straight to a method
            // is two braces, the way template code means it.
            let pair = matches!(
                (chars.get(i), chars.get(i + 1), chars.get(i + 2)),
                (Some('{'), Some('{'), Some('"')) | (Some('}'), Some('}'), Some('"'))
            );
            if pair && !py && method_arg_start(&chars, i - 1) {
                let b = chars[i];
                s.extend([b, b, b, b]);
                i += 3;
                col += 3;
                push(&mut out, Tok::Str(s));
                continue;
            }
            loop {
                let bs = std::mem::take(&mut after_bs);
                if depth > 0 {
                    match chars.get(i) {
                        None | Some('\n') => {
                            return Err(Diag::new("lex", tl, tc, "unterminated string")
                                .fix_hint("add a closing \""));
                        }
                        Some(&ch) => {
                            if inner_sq || (!inner_str && ch == '\'') {
                                inner_sq = !(inner_sq && ch == '\'');
                                s.push(ch);
                                i += 1;
                                col += 1;
                                continue;
                            }
                            if inner_str {
                                if ch == '\\' {
                                    s.push(ch);
                                    if let Some(&n) = chars.get(i + 1) {
                                        s.push(n);
                                    }
                                    i += 2;
                                    col += 2;
                                    continue;
                                }
                                if ch == '"' {
                                    inner_str = false;
                                }
                            } else if ch == '"' {
                                inner_str = true;
                            } else if ch == '{' {
                                depth += 1;
                            } else if ch == '}' {
                                depth -= 1;
                            }
                            s.push(ch);
                            i += 1;
                            col += 1;
                            continue;
                        }
                    }
                }
                match chars.get(i) {
                    None | Some('\n') => {
                        return Err(Diag::new("lex", tl, tc, "unterminated string")
                            .fix_hint("add a closing \""));
                    }
                    Some(&b @ ('{' | '}')) if py && !is_f => {
                        s.push(b);
                        s.push(b);
                        i += 1;
                        col += 1;
                    }
                    Some('\\') if py && !is_f && matches!(chars.get(i + 1), Some('{' | '}')) => {
                        let b = chars[i + 1];
                        s.push('\\');
                        s.push(b);
                        s.push(b);
                        i += 2;
                        col += 2;
                    }
                    Some(&b @ ('{' | '}')) if py && chars.get(i + 1) == Some(&b) => {
                        s.push(b);
                        s.push(b);
                        i += 2;
                        col += 2;
                    }
                    // A template tag such as "{{#each items}}", "{{/if}}" or "{{else}}" keeps both braces on each side.
                    Some('{')
                        if chars.get(i + 1) == Some(&'{')
                            && template_tag(&chars, i + 2).is_some() =>
                    {
                        let (end, closed) = template_tag(&chars, i + 2).unwrap();
                        s.push_str("{{{{");
                        s.extend(&chars[i + 2..end]);
                        let stop = if closed { end + 2 } else { end };
                        if closed {
                            s.push_str("}}}}");
                        }
                        col += stop - i;
                        i = stop;
                    }
                    Some('{') if chars.get(i + 1) == Some(&'{') => {
                        s.push_str("{{");
                        esc_open += 1;
                        i += 2;
                        col += 2;
                    }
                    // `}}` is one brace, unless it closes plain `{` braces opened earlier, as in JSON text.
                    Some('}') if chars.get(i + 1) == Some(&'}') => {
                        if esc_open > 0 || lone_open == 0 {
                            esc_open = esc_open.saturating_sub(1);
                            s.push_str("}}");
                        } else {
                            lone_open = lone_open.saturating_sub(2);
                            s.push_str("}}}}");
                        }
                        i += 2;
                        col += 2;
                    }
                    // f"{ x }": spaces after the brace are dropped, as Python reads them.
                    Some('{')
                        if is_f && !bs && chars.get(i + 1) == Some(&' ') && {
                            let j = (i + 1..chars.len())
                                .find(|&j| chars[j] != ' ')
                                .unwrap_or(chars.len());
                            num_interp(&chars, j).is_some()
                                || chars.get(j).map_or(false, |c| {
                                    c.is_alphabetic()
                                        || matches!(c, '_' | '(' | '[' | '-' | '"' | '\'')
                                }) && interp_closes(&chars, j)
                        } =>
                    {
                        let j = (i + 1..chars.len())
                            .find(|&j| chars[j] != ' ')
                            .unwrap_or(chars.len());
                        if let Some(end) = num_interp(&chars, j) {
                            s.push_str("{(");
                            s.extend(&chars[j..end]);
                            s.push(')');
                            col += end - i;
                            i = end;
                        } else {
                            s.push('{');
                            col += j - i;
                            i = j;
                        }
                        depth = 1;
                    }
                    // f"{7:03d}": the number is wrapped as `(7)` so it reads as a value.
                    Some('{') if is_f && !bs && num_interp(&chars, i + 1).is_some() => {
                        let end = num_interp(&chars, i + 1).unwrap();
                        s.push_str("{(");
                        s.extend(&chars[i + 1..end]);
                        s.push(')');
                        depth = 1;
                        col += end - i;
                        i = end;
                    }
                    // f"{1/3:.3f}", f"{2*n}": a value that starts with a number.
                    Some('{')
                        if py
                            && is_f
                            && !bs
                            && chars
                                .get(i + 1)
                                .is_some_and(|c| c.is_ascii_digit() || *c == '.')
                            && interp_closes(&chars, i + 1) =>
                    {
                        depth = 1;
                        s.push('{');
                        i += 1;
                        col += 1;
                    }
                    Some('{')
                        if !bs
                            && chars.get(i + 1).map_or(false, |c| {
                                c.is_alphabetic()
                                    || matches!(c, '_' | '(' | '[' | '-' | '!' | '"' | '\'')
                            })
                            && interp_closes(&chars, i + 1) =>
                    {
                        depth = 1;
                        s.push('{');
                        i += 1;
                        col += 1;
                    }
                    // Any other brace is literal; it is doubled so the string parser reads it that way.
                    Some(&b @ ('{' | '}')) => {
                        if b == '{' {
                            lone_open += 1;
                        } else {
                            lone_open = lone_open.saturating_sub(1);
                        }
                        s.push(b);
                        s.push(b);
                        i += 1;
                        col += 1;
                    }
                    Some('"') => {
                        i += 1;
                        col += 1;
                        break;
                    }
                    Some('\\')
                        if py
                            && chars.get(i + 1) == Some(&'N')
                            && char_escape(&chars, i).is_none() =>
                    {
                        return Err(bad_named_escape(tl, tc));
                    }
                    Some('\\') if py && char_escape(&chars, i).is_some() => {
                        let (ch, n) = char_escape(&chars, i).unwrap();
                        after_bs = false;
                        s.push(ch);
                        if ch == '{' || ch == '}' {
                            s.push(ch);
                        }
                        i += n;
                        col += n;
                    }
                    Some('\\') if py && chars.get(i + 1) == Some(&'\n') => {
                        i += 2;
                        line += 1;
                        col = 1;
                    }
                    Some('\\') => {
                        let e = chars.get(i + 1).copied().unwrap_or(' ');
                        after_bs = e == '\\';
                        if e == '{' || e == '}' {
                            s.push(e);
                        }
                        // Any other escape keeps its backslash, as in Python (handy for regexes: "\\d+").
                        if !"ntr0\"\\{}".contains(e) && !(py && e == '\'') {
                            s.push('\\');
                        }
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '0' => '\0',
                            '"' => '"',
                            '\\' => '\\',
                            // `\{` and `\}` are literal braces.
                            '{' | '}' => e,
                            other => other,
                        });
                        i += 2;
                        col += 2;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                        col += 1;
                    }
                }
            }
            push(&mut out, Tok::Str(s));
            continue;
        }
        // Python's `0xFF`, `0o17`, `0b101`.
        if c == '0'
            && matches!(chars.get(i + 1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B'))
            && chars.get(i + 2).map_or(false, |d| d.is_ascii_hexdigit())
        {
            let radix = match chars[i + 1] {
                'x' | 'X' => 16,
                'o' | 'O' => 8,
                _ => 2,
            };
            let mut j = i + 2;
            while j < chars.len() && (chars[j].is_digit(radix) || chars[j] == '_') {
                j += 1;
            }
            let text: String = chars[i + 2..j].iter().filter(|&&d| d != '_').collect();
            match u64::from_str_radix(&text, radix) {
                // Up to 64 bits, as Rust reads them (0xFFFFFFFFFFFFFFFF is -1).
                Ok(n) => push(&mut out, Tok::Int(n as i64)),
                Err(_) => {
                    return Err(Diag::new(
                        "lex",
                        tl,
                        tc,
                        "integer literal too large for int",
                    ));
                }
            }
            col += j - i;
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                i += 1;
            }
            // After a `.` this is a tuple field (`p.0.1`), never a float.
            let after_dot = start > 0 && chars[start - 1] == '.';
            let is_float = !after_dot
                && chars.get(i) == Some(&'.')
                && chars.get(i + 1).map_or(false, |d| d.is_ascii_digit());
            if is_float {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // `1e15`, `2.5e-3`
            let mut is_float = is_float;
            if !after_dot && matches!(chars.get(i), Some('e' | 'E')) {
                let j = i + 1 + matches!(chars.get(i + 1), Some('-' | '+')) as usize;
                if chars.get(j).map_or(false, |d| d.is_ascii_digit()) {
                    i = j;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    is_float = true;
                }
            }
            let text: String = chars[start..i].iter().filter(|&&d| d != '_').collect();
            col += i - start;
            if is_float {
                push(&mut out, Tok::Float(text.parse().unwrap()));
            } else {
                match text.parse() {
                    Ok(n) => push(&mut out, Tok::Int(n)),
                    Err(_) => {
                        return Err(Diag::new(
                            "lex",
                            tl,
                            tc,
                            "integer literal too large for int",
                        ));
                    }
                }
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            col += i - start;
            // Python's f"..." is a plain string: Brevi strings already fill in `{name}`.
            if word == "f" && matches!(chars.get(i), Some('"') | Some('\'')) {
                fstr = true;
                continue;
            }
            // Python's `def f(...)` is `fn f(...)`.
            if word == "def"
                && chars.get(i) == Some(&' ')
                && chars
                    .get(i + 1)
                    .map_or(false, |c| c.is_alphabetic() || *c == '_')
            {
                push(&mut out, Tok::Kw("fn"));
                continue;
            }
            // Python's raw strings `r"\d+"`: backslashes stay as written and braces are plain text.
            if matches!(word.as_str(), "r" | "R" | "rb" | "br")
                && matches!(chars.get(i), Some('"') | Some('\''))
            {
                let q = chars[i];
                let triple = chars.get(i + 1) == Some(&q) && chars.get(i + 2) == Some(&q);
                let mut j = i + if triple { 3 } else { 1 };
                let mut s = String::new();
                loop {
                    match chars.get(j) {
                        None => {
                            return Err(Diag::new("lex", tl, tc, "unterminated string")
                                .fix_hint("add a closing quote"));
                        }
                        Some('\n') if !triple => {
                            return Err(Diag::new("lex", tl, tc, "unterminated string")
                                .fix_hint("add a closing quote"));
                        }
                        Some('\\') if chars.get(j + 1).is_some() => {
                            s.push('\\');
                            if matches!(chars[j + 1], '{' | '}') {
                                s.push(chars[j + 1]);
                            }
                            s.push(chars[j + 1]);
                            j += 2;
                        }
                        Some(&c)
                            if c == q
                                && (!triple
                                    || (chars.get(j + 1) == Some(&q)
                                        && chars.get(j + 2) == Some(&q))) =>
                        {
                            j += if triple { 3 } else { 1 };
                            break;
                        }
                        Some(&c) => {
                            if c == '{' || c == '}' {
                                s.push(c);
                            }
                            s.push(c);
                            j += 1;
                        }
                    }
                }
                line += chars[i..j].iter().filter(|c| **c == '\n').count();
                col += j - i;
                i = j;
                push(&mut out, Tok::Str(s));
                continue;
            }
            // Python's `elif` is accepted as `else if`.
            if word == "elif" {
                push(&mut out, Tok::Kw("else"));
                push(&mut out, Tok::Kw("if"));
                continue;
            }
            // Python's `x is None` and `x is not None` compare with None.
            if word == "is" {
                let rest: String = chars[i..chars.len().min(i + 10)].iter().collect();
                if rest.starts_with(" None") {
                    push(&mut out, Tok::Sym("=="));
                    continue;
                }
                // `v is True` / `v is not False`: identity with a bool, which a JSON 1 (or 1.0) is not, though
                // it == True. A method that is true only for that bool.
                if let Some((m, neg, skip)) = [
                    (" True", "is_true_obj", false, 5usize),
                    (" False", "is_false_obj", false, 6),
                    (" not True", "not_true_obj", true, 9),
                    (" not False", "not_false_obj", true, 10),
                ]
                .iter()
                .find(|(w, _, _, k)| {
                    rest.starts_with(w)
                        && !chars
                            .get(i + k)
                            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
                })
                .map(|(_, m, n, k)| (*m, *n, *k))
                {
                    push(&mut out, Tok::Sym("."));
                    push(&mut out, Tok::Ident(m.into()));
                    push(&mut out, Tok::Sym("("));
                    push(&mut out, Tok::Sym(")"));
                    let _ = neg;
                    i += skip;
                    col += skip;
                    continue;
                }
                if rest.starts_with(" not ") {
                    push(&mut out, Tok::Sym("!="));
                    i += 4;
                    col += 4;
                    continue;
                }
                // Python's identity test on other values: Brevi values have no identity, so it compares them.
                push(&mut out, Tok::Sym("=="));
                continue;
            }
            // Python code often names a value `fn`; only `fn NAME` starts a function.
            if word == "fn" {
                let mut j = i;
                while chars.get(j) == Some(&' ') {
                    j += 1;
                }
                if !chars
                    .get(j)
                    .map_or(false, |c| c.is_alphabetic() || *c == '_')
                {
                    push(&mut out, Tok::Ident("fn_".into()));
                    continue;
                }
            }
            // Python code names values `var`, `match`, `type`, `test`... (`def var(expr):`, `match = re.match(...)`): a
            // Brevi-only keyword is a name after `def` or when a value's use (`=`, `.`, `,`, `)`, `[`) follows it.
            if ["let", "var", "type", "enum", "match", "test", "mut"].contains(&word.as_str()) {
                let mut j = i;
                while chars.get(j) == Some(&' ') {
                    j += 1;
                }
                let next = chars.get(j).copied().unwrap_or('\n');
                let after_def = matches!(
                    out.last(),
                    Some(Token {
                        tok: Tok::Kw("fn"),
                        ..
                    })
                );
                let used = matches!(next, '.' | ',' | ')' | ']' | '[')
                    || (next == '=' && chars.get(j + 1) != Some(&'='));
                if after_def || used || py_defs.contains(&word) {
                    push(&mut out, Tok::Ident(format!("{word}_")));
                    continue;
                }
                // Python's `type(v)`, only ever compared with a type (`type(v) is int`).
                if word == "type" && next == '(' {
                    push(&mut out, Tok::Ident("ax_type_of".into()));
                    continue;
                }
            }
            // Python's True and False are true and false.
            let word = match word.as_str() {
                "True" => "true".to_string(),
                "False" => "false".to_string(),
                _ => word,
            };
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => push(&mut out, Tok::Kw(k)),
                // Rust's own keywords are fine names in Brevi; they get a suffix in the generated Rust.
                None if RUST_KEYWORDS.contains(&word.as_str())
                    || RUST_PRELUDE.contains(&word.as_str()) =>
                {
                    push(&mut out, Tok::Ident(format!("{word}_")))
                }
                None => push(&mut out, Tok::Ident(word)),
            }
            continue;
        }
        let rest: String = chars[i..(i + 3).min(chars.len())].iter().collect();
        // `&&` and `||` are accepted as spellings of `and` and `or`.
        if rest.starts_with("&&") || rest.starts_with("||") {
            push(
                &mut out,
                Tok::Kw(if rest.starts_with("&&") { "and" } else { "or" }),
            );
            i += 2;
            col += 2;
            continue;
        }
        match SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
            Some(s) => {
                match *s {
                    "(" | "[" | "{" => depth.push(s.chars().next().unwrap()),
                    ")" | "]" | "}" => {
                        depth.pop();
                    }
                    _ => {}
                }
                push(&mut out, Tok::Sym(s));
                i += s.len();
                col += s.len();
            }
            None => {
                return Err(Diag::new(
                    "lex",
                    tl,
                    tc,
                    &format!("unexpected character '{c}'"),
                ));
            }
        }
    }
    out.push(Token {
        tok: Tok::Newline,
        line,
        col,
    });
    out.push(Token {
        tok: Tok::Eof,
        line,
        col,
    });
    let mut out = inline_bodies(out);
    // Python's `a = 1; b = 2`: each statement after a `;` is a line of its own at the same indentation.
    let mut first_col = 1;
    let mut at_start = true;
    let mut semi = false;
    for t in out.iter_mut() {
        match t.tok {
            Tok::Newline => {
                at_start = true;
                semi = false;
            }
            Tok::Sym(";") => {
                t.tok = Tok::Newline;
                semi = true;
                continue;
            }
            _ => {
                if semi {
                    t.col = first_col;
                    semi = false;
                } else if at_start {
                    first_col = t.col;
                }
                at_start = false;
            }
        }
    }
    // `from datetime import date as Date`, `import datetime as dt`: the short name means the real one.
    let aliases = py_import_aliases(src_in);
    if !aliases.is_empty() {
        for k in 0..out.len() {
            let after_dot = k > 0 && out[k - 1].tok == Tok::Sym(".");
            if let Tok::Ident(w) = &out[k].tok {
                if let Some((_, real)) = aliases.iter().find(|(a, _)| a == w).filter(|_| !after_dot)
                {
                    out[k].tok = Tok::Ident(real.clone());
                }
            }
        }
    }
    // Python's `string` constants: `string.digits`, or `digits` after `from string import digits`.
    let consts = [
        ("ascii_lowercase", "abcdefghijklmnopqrstuvwxyz"),
        ("ascii_uppercase", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        (
            "ascii_letters",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        ),
        ("digits", "0123456789"),
        ("hexdigits", "0123456789abcdefABCDEF"),
        ("octdigits", "01234567"),
    ];
    let from_string: Vec<&str> = src_in
        .lines()
        .filter_map(|l| l.trim().strip_prefix("from string import "))
        .flat_map(|r| r.split(',').map(|n| n.trim()))
        .collect();
    let mut k = 0;
    while k < out.len() {
        let after_dot = k > 0 && out[k - 1].tok == Tok::Sym(".");
        if let Tok::Ident(w) = &out[k].tok {
            if let Some((_, v)) = consts.iter().find(|(n, _)| n == w) {
                let v = Tok::Str(v.to_string());
                if after_dot
                    && k >= 2
                    && out[k - 2].tok == Tok::Ident("string".into())
                    && (k < 3 || out[k - 3].tok != Tok::Sym("."))
                {
                    out[k].tok = v;
                    out.drain(k - 2..k);
                    k -= 2;
                } else if !after_dot && from_string.contains(&w.as_str()) {
                    out[k].tok = v;
                }
            }
        }
        k += 1;
    }
    // `datetime.timedelta(days=1)` is `timedelta(days=1)`.
    let mut k = 0;
    while k + 2 < out.len() {
        if out[k].tok == Tok::Ident("datetime".into())
            && out[k + 1].tok == Tok::Sym(".")
            && out[k + 2].tok == Tok::Ident("timedelta".into())
            && (k == 0 || out[k - 1].tok != Tok::Sym("."))
        {
            out.drain(k..k + 2);
        }
        k += 1;
    }
    // Python joins string literals written next to each other (`("abc "\n "def")`, `f"{n} " f"items"`).
    let mut joined: Vec<Token> = Vec::with_capacity(out.len());
    for t in out {
        if let (
            Tok::Str(b),
            Some(Token {
                tok: Tok::Str(a), ..
            }),
        ) = (&t.tok, joined.last_mut())
        {
            a.push_str(b);
            continue;
        }
        joined.push(t);
    }
    Ok(joined)
}

// (alias, real name) pairs from Python import lines whose module Brevi has built in.
fn py_import_aliases(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let ok = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || c == '_');
    for l in src.lines() {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("from ") {
            let Some((m, names)) = rest.split_once(" import ") else {
                continue;
            };
            if !(PY_BUILTIN_MODULES.contains(&m.trim()) || PY_ALIAS_MODULES.contains(&m.trim())) {
                continue;
            }
            for n in names
                .trim_matches(|c| c == '(' || c == ')' || c == ' ')
                .split(',')
            {
                if let Some((real, alias)) = n.split_once(" as ") {
                    let (real, alias) = (real.trim(), alias.trim());
                    if ok(real) && ok(alias) && real != alias {
                        out.push((alias.to_string(), real.to_string()));
                    }
                }
            }
        } else if let Some(rest) = t.strip_prefix("import ") {
            for n in rest.split(',') {
                if let Some((real, alias)) = n.split_once(" as ") {
                    let (real, alias) = (real.trim(), alias.trim());
                    if (PY_BUILTIN_MODULES.contains(&real) || PY_ALIAS_MODULES.contains(&real))
                        && ok(alias)
                        && real != alias
                    {
                        out.push((alias.to_string(), real.to_string()));
                    }
                }
            }
        }
    }
    out
}

// Modules whose names the parser knows (`datetime.date`, `json.loads`), so an alias can stand for them.
const PY_ALIAS_MODULES: &[&str] = &[
    "datetime",
    "json",
    "calendar",
    "decimal",
    "fractions",
    "heapq",
    "bisect",
    "string",
    "statistics",
    "random",
    "time",
    "copy",
    "operator",
];

// Python-style blocks: a line ending in `:` opens a block that holds the more-indented lines after it.
// The block is turned into `{ ... }` here, so the parser only sees braces. Brace blocks still work.
pub fn indent_blocks(toks: Vec<Token>) -> Vec<Token> {
    let mut lines: Vec<Vec<Token>> = vec![vec![]];
    let mut eof = None;
    for t in toks {
        match t.tok {
            Tok::Newline => lines.push(vec![]),
            Tok::Eof => eof = Some(t),
            _ => lines.last_mut().unwrap().push(t),
        }
    }
    let mut out = Vec::new();
    let mut open: Vec<usize> = Vec::new(); // indentation of each open block's header line
    let mk = |tok: Tok, at: &Token| Token {
        tok,
        line: at.line,
        col: at.col,
    };
    for mut l in lines.into_iter().filter(|l| !l.is_empty()) {
        let ind = l[0].col;
        while open.last().map_or(false, |&h| ind <= h) {
            open.pop();
            out.push(mk(Tok::Sym("}"), &l[0]));
            // `else` continues the `if` whose block just closed.
            let continues = l[0].tok == Tok::Kw("else") && open.last().map_or(true, |&h| ind > h);
            if !continues {
                out.push(mk(Tok::Newline, &l[0]));
            }
        }
        if l.last().map_or(false, |t| t.tok == Tok::Sym(":")) && l.len() > 1 {
            l.last_mut().unwrap().tok = Tok::Sym("{");
            open.push(ind);
        } else if l.last().map_or(false, |t| t.tok == Tok::Sym("=>"))
            && open.last().map_or(false, |&h| ind > h)
        {
            // A match arm whose body is the indented lines below it.
            let at = l.last().unwrap().clone();
            l.push(mk(Tok::Sym("{"), &at));
            open.push(ind);
        }
        let last = l.last().unwrap().clone();
        out.extend(l);
        out.push(mk(Tok::Newline, &last));
    }
    let end = eof.unwrap_or(Token {
        tok: Tok::Eof,
        line: 0,
        col: 0,
    });
    for _ in open {
        out.push(mk(Tok::Sym("}"), &end));
        out.push(mk(Tok::Newline, &end));
    }
    out.push(end);
    out
}

// Python's `if k in c: a; b` runs both statements under the if: the body after the `:` becomes an indented block.
fn inline_bodies(toks: Vec<Token>) -> Vec<Token> {
    const OPENERS: &[&str] = &["if", "else", "for", "while", "fn"];
    let mut out = Vec::with_capacity(toks.len());
    let mut line: Vec<Token> = Vec::new();
    for t in toks {
        let end = matches!(t.tok, Tok::Newline | Tok::Eof);
        line.push(t);
        if !end {
            continue;
        }
        let opens = match line.first().map(|t| &t.tok) {
            Some(Tok::Kw(k)) => OPENERS.contains(k),
            Some(Tok::Ident(w)) => matches!(w.as_str(), "try_" | "except" | "finally" | "with"),
            _ => false,
        };
        let mut colon = None;
        let mut has_semi = false;
        if opens {
            let mut depth = 0i32;
            for (i, t) in line.iter().enumerate() {
                match &t.tok {
                    Tok::Sym("(" | "[" | "{") => depth += 1,
                    Tok::Sym(")" | "]" | "}") => depth -= 1,
                    // A lambda's `x: ...` cannot appear at depth 0 in a statement header, so the first `:` there ends it.
                    Tok::Sym(":") if depth == 0 && colon.is_none() => colon = Some(i),
                    Tok::Sym(";") if depth == 0 && colon.is_some() => has_semi = true,
                    _ => {}
                }
            }
        }
        match colon {
            Some(c)
                if has_semi && c + 1 < line.len() && !matches!(line[c + 1].tok, Tok::Newline) =>
            {
                let indent = line[0].col + 4;
                let (ln, cl) = (line[c].line, line[c].col);
                let rest = line.split_off(c + 1);
                out.extend(line.drain(..));
                out.push(Token {
                    tok: Tok::Newline,
                    line: ln,
                    col: cl + 1,
                });
                let mut first = true;
                for mut t in rest {
                    if first && !matches!(t.tok, Tok::Newline | Tok::Eof) {
                        t.col = indent;
                    }
                    first = matches!(t.tok, Tok::Sym(";"));
                    if first {
                        t.tok = Tok::Newline;
                    }
                    out.push(t);
                }
            }
            _ => out.append(&mut line),
        }
        line.clear();
    }
    out
}
