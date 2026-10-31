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

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "const", "crate", "dyn", "extern", "impl", "loop", "mod", "move", "priv", "pub", "ref",
    "self", "static", "struct", "super", "trait", "try", "typeof", "unsafe", "use", "where", "yield", "macro", "final",
    "override", "virtual", "become", "abstract", "do", "unsized",
];

// "{{#each items}}", "{{/if}}", "{{else}}", or "{{#each " at a string's end: where the tag's text ends,
// and whether `}}` follows it.
fn template_tag(chars: &[char], from: usize) -> Option<(usize, bool)> {
    let first = *chars.get(from)?;
    let tag_start = matches!(first, '#' | '/');
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

// Does the `{` just before `from` close on this line (quoted strings inside skipped)?
fn interp_closes(chars: &[char], from: usize) -> bool {
    // `{3}` or `{2,4}` is a regex count, not a value.
    let count: String = chars[from..].iter().take_while(|c| **c != '}' && **c != '"' && **c != '\n').collect();
    if !count.is_empty() && count.chars().all(|c| c.is_ascii_digit() || c == ',') && chars.get(from + count.len()) == Some(&'}') {
        return false;
    }
    let (mut depth, mut in_str, mut i) = (1, false, from);
    while let Some(&c) = chars.get(i) {
        match c {
            '\n' => return false,
            '\\' if in_str => i += 1,
            // A quote right after a value ends the outer string, so this `{` was not an interpolation.
            '"' if !in_str && chars[..i].iter().rev().find(|c| **c != ' ').map_or(false, |p| p.is_alphanumeric() || matches!(p, '_' | ')' | ']')) => return false,
            '"' if in_str => {
                in_str = false;
                // After a string inside `{}` comes an operator or the closing brace, not more text.
                if chars[i + 1..].iter().find(|c| **c != ' ').map_or(true, |c| c.is_alphanumeric() || matches!(c, ':' | '"' | '\n')) {
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
    "->", "=>", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "~=", "..", "(", ")", "[", "]",
    "{", "}", ",", ":", ".", "+", "-", "*", "/", "%", "<", ">", "=", "!", "?",
];

// After a value, `a // b` is floor division unless what follows reads like words (`x = 5 // the count`).
fn looks_like_prose(rest: &str) -> bool {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let is_word = |w: &str| w.chars().all(|c| c.is_alphanumeric() || c == '_') && !["and", "or", "not", "if", "else", "in"].contains(&w);
    rest.trim().is_empty() || words.windows(2).any(|p| is_word(p[0]) && is_word(p[1]))
}

pub fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1usize, 1usize);
    // Newlines inside () or [] never end a statement.
    let mut depth: Vec<char> = Vec::new();

    while i < chars.len() {
        let c = chars[i];
        let (tl, tc) = (line, col);
        let push = |out: &mut Vec<Token>, tok: Tok| out.push(Token { tok, line: tl, col: tc });

        if c == '\n' {
            if !matches!(depth.last(), Some('(') | Some('[')) {
                push(&mut out, Tok::Newline);
            }
            i += 1;
            line += 1;
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
            let rest = rest.split("//").next().unwrap_or("").split('#').next().unwrap_or("").to_string();
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
            let mut s = String::new();
            i += 1;
            col += 1;
            loop {
                match chars.get(i) {
                    None | Some('\n') => return Err(Diag::new("lex", tl, tc, "unterminated string").fix_hint("add a closing '")),
                    Some('\'') => {
                        i += 1;
                        col += 1;
                        break;
                    }
                    Some('{') if chars.get(i + 1).map_or(false, |c| c.is_alphabetic() || matches!(c, '_' | '(')) && interp_closes(&chars, i + 1) => {
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
                    Some('\\') => {
                        let e = chars.get(i + 1).copied().unwrap_or(' ');
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '0' => s.push('\0'),
                            '\'' | '\\' => s.push(e),
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
            let mut s = String::new();
            i += 1;
            col += 1;
            // Inside `{...}` the text is kept as written (quotes included), to be lexed again as an expression.
            let (mut depth, mut inner_str, mut esc_open, mut lone_open) = (0usize, false, 0usize, 0usize);
            // Inside a value, a 'single-quoted' string is skipped whole ("{'#'.repeat(n)}").
            let mut inner_sq = false;
            // Just after `\\`: a `{` here is a regex's escaped brace (`"\\{(\\d+)\\}"`), never a value.
            let mut after_bs = false;
            loop {
                let bs = std::mem::take(&mut after_bs);
                if depth > 0 {
                    match chars.get(i) {
                        None | Some('\n') => return Err(Diag::new("lex", tl, tc, "unterminated string").fix_hint("add a closing \"")),
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
                        return Err(Diag::new("lex", tl, tc, "unterminated string").fix_hint("add a closing \""));
                    }
                    // A template tag such as "{{#each items}}", "{{/if}}" or "{{else}}" keeps both braces on each side.
                    Some('{') if chars.get(i + 1) == Some(&'{') && template_tag(&chars, i + 2).is_some() => {
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
                    Some('{') if !bs && chars.get(i + 1).map_or(false, |c| c.is_alphabetic() || matches!(c, '_' | '(' | '-' | '!' | '"' | '\'')) && interp_closes(&chars, i + 1) => {
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
                    Some('\\') => {
                        let e = chars.get(i + 1).copied().unwrap_or(' ');
                        after_bs = e == '\\';
                        if e == '{' || e == '}' {
                            s.push(e);
                        }
                        // Any other escape keeps its backslash, as in Python (handy for regexes: "\\d+").
                        if !"ntr0\"\\{}".contains(e) {
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
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                i += 1;
            }
            let is_float = chars.get(i) == Some(&'.') && chars.get(i + 1).map_or(false, |d| d.is_ascii_digit());
            if is_float {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // `1e15`, `2.5e-3`
            let mut is_float = is_float;
            if matches!(chars.get(i), Some('e' | 'E')) {
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
                    Err(_) => return Err(Diag::new("lex", tl, tc, "integer literal too large for int")),
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
            // Python's `elif` is accepted as `else if`.
            if word == "elif" {
                push(&mut out, Tok::Kw("else"));
                push(&mut out, Tok::Kw("if"));
                continue;
            }
            // Python's True and False are true and false.
            let word = match word.as_str() { "True" => "true".to_string(), "False" => "false".to_string(), _ => word };
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => push(&mut out, Tok::Kw(k)),
                // Rust's own keywords are fine names in Brevi; they get a suffix in the generated Rust.
                None if RUST_KEYWORDS.contains(&word.as_str()) => push(&mut out, Tok::Ident(format!("{word}_"))),
                None => push(&mut out, Tok::Ident(word)),
            }
            continue;
        }
        let rest: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        // `&&` and `||` are accepted as spellings of `and` and `or`.
        if rest == "&&" || rest == "||" {
            push(&mut out, Tok::Kw(if rest == "&&" { "and" } else { "or" }));
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
            None => return Err(Diag::new("lex", tl, tc, &format!("unexpected character '{c}'"))),
        }
    }
    out.push(Token { tok: Tok::Newline, line, col });
    out.push(Token { tok: Tok::Eof, line, col });
    Ok(out)
}

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
    let mk = |tok: Tok, at: &Token| Token { tok, line: at.line, col: at.col };
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
        } else if l.last().map_or(false, |t| t.tok == Tok::Sym("=>")) && open.last().map_or(false, |&h| ind > h) {
            // A match arm whose body is the indented lines below it.
            let at = l.last().unwrap().clone();
            l.push(mk(Tok::Sym("{"), &at));
            open.push(ind);
        }
        let last = l.last().unwrap().clone();
        out.extend(l);
        out.push(mk(Tok::Newline, &last));
    }
    let end = eof.unwrap_or(Token { tok: Tok::Eof, line: 0, col: 0 });
    for _ in open {
        out.push(mk(Tok::Sym("}"), &end));
        out.push(mk(Tok::Newline, &end));
    }
    out.push(end);
    out
}
