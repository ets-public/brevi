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
    "true", "false", "mut", "test", "assert", "break", "continue", "and", "or",
];

// Longest symbols first so the scanner can take the first match.
const SYMBOLS: &[&str] = &[
    "->", "=>", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "~=", "..", "(", ")", "[", "]",
    "{", "}", ",", ":", ".", "+", "-", "*", "/", "%", "<", ">", "=", "!", "?",
];

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
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            i += 1;
            col += 1;
            loop {
                match chars.get(i) {
                    None | Some('\n') => {
                        return Err(Diag::new("lex", tl, tc, "unterminated string").fix_hint("add a closing \""));
                    }
                    Some('"') => {
                        i += 1;
                        col += 1;
                        break;
                    }
                    Some('\\') => {
                        let e = chars.get(i + 1).copied().unwrap_or(' ');
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '0' => '\0',
                            '"' => '"',
                            '\\' => '\\',
                            other => {
                                return Err(Diag::new("lex", line, col, &format!("unknown escape \\{other}"))
                                    .fix_hint("escapes are \\n \\t \\r \\0 \\\" \\\\"));
                            }
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
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => push(&mut out, Tok::Kw(k)),
                None => push(&mut out, Tok::Ident(word)),
            }
            continue;
        }
        let rest: String = chars[i..(i + 2).min(chars.len())].iter().collect();
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
