// Python's `\N{NAME}` escape, for both .py lexers. The table comes from tools/unicode_names.py, which describes it.
const NAMES: &str = include_str!("pynames/unicode_names.txt");

// Names match in any case, as in CPython. Runs once per escape at build time, so a scan is fast enough.
fn lookup(name: &str) -> Option<char> {
    let want = name.to_ascii_uppercase();
    let (mut cur, mut code) = (String::new(), u32::MAX);
    for line in NAMES.lines() {
        let (text, hex) = line
            .split_once('\t')
            .map_or((line, None), |(t, h)| (t, Some(h)));
        code = hex.map_or(code.wrapping_add(1), |h| {
            u32::from_str_radix(h, 16).unwrap()
        });
        cur.truncate((text.as_bytes()[0] - 33) as usize);
        cur.push_str(&text[1..]);
        if cur == want {
            return char::from_u32(code);
        }
    }
    None
}

// `rest` is what follows `\N`. Gives the character and how many chars of `rest` the `{NAME}` takes, or None when the
// escape is malformed or the name is not in the table: the caller must refuse it, since CPython would either stop with
// a SyntaxError or knows a name the table leaves out.
pub fn escape(rest: &[char]) -> Option<(char, usize)> {
    if rest.first() != Some(&'{') {
        return None;
    }
    let end = rest.iter().position(|c| *c == '}')?;
    lookup(&rest[1..end].iter().collect::<String>()).map(|c| (c, end + 1))
}

#[cfg(test)]
mod tests {
    use super::lookup;

    #[test]
    fn finds_names_and_aliases_in_any_case() {
        assert_eq!(lookup("BULLET"), Some('\u{2022}'));
        assert_eq!(lookup("bullet"), Some('\u{2022}'));
        assert_eq!(lookup("SPACE"), Some(' '));
        assert_eq!(lookup("NBSP"), Some('\u{a0}'));
        assert_eq!(lookup("LF"), Some('\n'));
        assert_eq!(lookup("GRINNING FACE"), Some('\u{1f600}'));
    }

    #[test]
    fn refuses_what_it_does_not_hold() {
        assert_eq!(lookup(""), None);
        assert_eq!(lookup(" BULLET"), None);
        assert_eq!(lookup("NO SUCH NAME"), None);
        // A named sequence: unicodedata.lookup knows it, \N{} does not.
        assert_eq!(lookup("LATIN SMALL LETTER R WITH TILDE"), None);
    }
}
