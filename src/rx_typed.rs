// Shared by the compiler (src/parser.rs, include!) and the typed runtime (src/emit.rs PRELUDE, after its @regex
// marker): literal patterns are checked at build time, patterns built at run time when ax_rx compiles them.
// The regex syntax the typed build's matcher (the prelude's ax_rx) handles. A pattern outside it would match
// wrongly there, so a literal one sends the program to the dynamic build, whose matcher has the rest of Python's
// syntax: back-references, look-arounds, named and atomic groups, possessive quantifiers, inline flags in the
// middle, `\B`, and so on. Errors CPython raises at compile time (nothing to repeat, unbalanced parentheses) go
// there too, so CPython's error is what the program sees.
pub fn rx_typed_unsupported(p: &str) -> Option<&'static str> {
    let c: Vec<char> = p.chars().collect();
    let mut i = 0;
    // (?i) first, then one group of s, m and a: the forms ax_rx_search_at and ax_rx_flags read.
    if c.starts_with(&['(', '?', 'i', ')']) {
        i = 4;
    }
    if c[i..].starts_with(&['(', '?']) {
        let mut j = i + 2;
        while j < c.len() && matches!(c[j], 's' | 'm' | 'a') {
            j += 1;
        }
        if j > i + 2 && c.get(j) == Some(&')') {
            i = j + 1;
        }
    }
    let hex = |i: usize, e: char| -> Option<usize> {
        let n = match e { 'x' => 2, 'u' => 4, 'U' => 8, _ => return Some(0) };
        let h: String = c.iter().skip(i).take(n).collect();
        (h.len() == n && u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).is_some()).then_some(n)
    };
    // whether the escape `\e` (e already read, at i - 1) is one the typed matcher reads as CPython does
    let esc_ok = |e: char, in_class: bool| -> bool {
        if e.is_ascii_digit() {
            return false;
        }
        if !e.is_ascii_alphabetic() {
            return true;
        }
        matches!(e, 'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'n' | 't' | 'r' | 'f' | 'v' | 'a' | 'x' | 'u' | 'U') || (!in_class && matches!(e, 'b' | 'A' | 'Z'))
    };
    let mut depth = 0usize;
    // whether the last item can take a quantifier, and whether it already has one
    let (mut atom, mut quantified) = (false, false);
    while i < c.len() {
        let ch = c[i];
        i += 1;
        match ch {
            '\\' => {
                let Some(&e) = c.get(i) else { return Some("a trailing backslash") };
                i += 1;
                if e.is_ascii_digit() {
                    return Some("a back-reference or octal escape");
                }
                if !esc_ok(e, false) {
                    return Some("this escape");
                }
                match hex(i, e) { Some(n) => i += n, None => return Some("a bad \\x or \\u escape") }
                (atom, quantified) = (!matches!(e, 'b' | 'A' | 'Z'), false);
            }
            '(' => {
                if c.get(i) == Some(&'?') {
                    if c.get(i + 1) != Some(&':') {
                        return Some("this group syntax");
                    }
                    i += 2;
                }
                depth += 1;
                (atom, quantified) = (false, false);
            }
            ')' => {
                if depth == 0 {
                    return Some("an unbalanced parenthesis");
                }
                depth -= 1;
                (atom, quantified) = (true, false);
            }
            '|' => (atom, quantified) = (false, false),
            '*' | '+' | '?' | '{' => {
                let mut lo_hi = None;
                if ch == '{' {
                    let mut j = i;
                    let num = |j: &mut usize| -> Option<u64> {
                        let s = *j;
                        while *j < c.len() && c[*j].is_ascii_digit() {
                            *j += 1;
                        }
                        (*j > s).then(|| c[s..*j].iter().collect::<String>().parse().unwrap_or(u64::MAX))
                    };
                    let lo = num(&mut j);
                    let hi = if c.get(j) == Some(&',') { j += 1; num(&mut j) } else { lo };
                    if c.get(j) != Some(&'}') || (lo.is_none() && (hi.is_some() || c.get(i) == Some(&','))) {
                        if lo.is_none() && c.get(i) == Some(&',') && c.get(j) == Some(&'}') {
                            return Some("{,n}");
                        }
                        // not a repeat: a literal `{`, as in CPython
                        (atom, quantified) = (true, false);
                        continue;
                    }
                    if lo.is_none() {
                        (atom, quantified) = (true, false);
                        continue;
                    }
                    lo_hi = Some((lo, hi));
                    i = j + 1;
                }
                if let Some((lo, hi)) = lo_hi {
                    // CPython's limit is MAXREPEAT; past it, or min above max, CPython raises
                    if lo.unwrap_or(0) >= u32::MAX as u64 || hi.map_or(false, |h| h >= u32::MAX as u64 || h < lo.unwrap_or(0)) {
                        return Some("this repeat count");
                    }
                }
                if !atom || quantified {
                    return Some("a repeat with nothing to repeat");
                }
                if c.get(i) == Some(&'?') {
                    i += 1;
                } else if c.get(i) == Some(&'+') {
                    return Some("a possessive quantifier");
                }
                quantified = true;
            }
            '[' => {
                if c.get(i) == Some(&'^') {
                    i += 1;
                }
                let mut first = true;
                loop {
                    let Some(&k) = c.get(i) else { return Some("an unterminated character class") };
                    i += 1;
                    if k == ']' && !first {
                        break;
                    }
                    first = false;
                    if k == '\\' {
                        let Some(&e) = c.get(i) else { return Some("an unterminated character class") };
                        i += 1;
                        if !esc_ok(e, true) {
                            return Some("this escape in a character class");
                        }
                        match hex(i, e) { Some(n) => i += n, None => return Some("a bad \\x or \\u escape") }
                    } else if k == '[' && matches!(c.get(i), Some(':' | '=' | '.')) {
                        return Some("a nested set");
                    }
                }
                (atom, quantified) = (true, false);
            }
            _ => (atom, quantified) = (!matches!(ch, '^' | '$'), false),
        }
    }
    if depth > 0 {
        return Some("an unbalanced parenthesis");
    }
    None
}
