#!/usr/bin/env python3.13
"""Regenerate src/unicode_case.rs from CPython's own str methods.

Run with the CPython the conformance tests target (3.13, unicodedata 15.1.0):
    python3.13 tools/gen_unicode_case.py > src/unicode_case.rs
`--check` exits 1 if src/unicode_case.rs differs from what this script writes.

The tables are read from CPython's str methods, never from the Unicode data files, so they are what CPython 3.13
answers. Properties CPython does not expose directly are read from behaviour it defines with them:
- Cased: "ca".title() lowers the "a" only after a cased character.
- Case_Ignorable: lower() of a final capital sigma looks back past case-ignorable characters for a cased one.
"""
import sys
import unicodedata
from pathlib import Path

HEADER = """\
// str case mappings and predicates (generated from CPython {py}, unicodedata {ud}): do not edit by hand.
// Regenerate with `python3.13 tools/gen_unicode_case.py > src/unicode_case.rs`.
// rustc's char::to_uppercase/is_alphabetic/... follow whatever Unicode version that rustc ships, which is newer
// than CPython 3.13's 15.1, so str.upper/lower/title/... use these tables instead, in both builds. The typed
// build appends this file (with src/unicode.rs) only to programs that call one of these methods.

"""

TAIL = r"""
fn uc_find(tab: &[(u32, &'static str)], u: u32) -> Option<&'static str> {
    tab.binary_search_by_key(&u, |&(k, _)| k).ok().map(|i| tab[i].1)
}

fn uc_has(tab: &[(u32, u32)], u: u32) -> bool {
    tab.binary_search_by(|&(a, b)| if b < u { std::cmp::Ordering::Less } else if a > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}

// The per-character properties CPython's unicodectype.c answers (Py_UNICODE_ISUPPER and friends).
pub fn py_ch_isupper(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_uppercase() } else { uc_has(UC_UPPER, c as u32) }
}
pub fn py_ch_islower(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_lowercase() } else { uc_has(UC_LOWER, c as u32) }
}
pub fn py_ch_istitle(c: char) -> bool {
    !c.is_ascii() && uc_has(UC_TITLE, c as u32)
}
pub fn py_ch_iscased(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphabetic() } else { uc_has(UC_CASED, c as u32) }
}
pub fn py_ch_iscaseignorable(c: char) -> bool {
    if c.is_ascii() { matches!(c, '\'' | '.' | ':' | '^' | '`') } else { uc_has(UC_CASE_IGNORABLE, c as u32) }
}
pub fn py_ch_isalpha(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphabetic() } else { uc_has(UC_ALPHA, c as u32) }
}
pub fn py_ch_isspace(c: char) -> bool {
    if c.is_ascii() { matches!(c, ' ' | '\t'..='\r' | '\x1c'..='\x1f') } else { uc_has(UC_SPACE, c as u32) }
}
// Py_UNICODE_ISALNUM: alpha, decimal, digit or numeric (src/unicode.rs holds the numeric tables).
pub fn py_ch_isalnum(c: char) -> bool {
    py_ch_isalpha(c) || py_is_numeric(c)
}

// The full mappings (_PyUnicode_ToUpperFull and friends): one to three characters.
fn uc_push_upper(out: &mut String, c: char) {
    if c.is_ascii() {
        out.push(c.to_ascii_uppercase());
    } else {
        match uc_find(UC_TO_UPPER, c as u32) { Some(m) => out.push_str(m), None => out.push(c) }
    }
}
fn uc_push_lower(out: &mut String, c: char) {
    if c.is_ascii() {
        out.push(c.to_ascii_lowercase());
    } else {
        match uc_find(UC_TO_LOWER, c as u32) { Some(m) => out.push_str(m), None => out.push(c) }
    }
}
fn uc_push_title(out: &mut String, c: char) {
    match uc_find(UC_TO_TITLE, c as u32) { Some(m) => out.push_str(m), None => uc_push_upper(out, c) }
}

// CPython's handle_capital_sigma: U+03A3 lowers to final sigma U+03C2 after a cased character (skipping
// case-ignorable ones) when no cased character follows (again skipping case-ignorable ones).
fn uc_sigma(cs: &[char], i: usize) -> char {
    let mut j = i;
    let mut prev = None;
    while j > 0 {
        j -= 1;
        if !py_ch_iscaseignorable(cs[j]) {
            prev = Some(cs[j]);
            break;
        }
    }
    let mut fin = prev.is_some_and(py_ch_iscased);
    if fin {
        fin = cs[i + 1..].iter().find(|&&c| !py_ch_iscaseignorable(c)).map_or(true, |&c| !py_ch_iscased(c));
    }
    if fin { '\u{3c2}' } else { '\u{3c3}' }
}
// lower_ucs4: the lower mapping of cs[i], with the final sigma rule.
fn uc_push_lower_at(out: &mut String, cs: &[char], i: usize) {
    if cs[i] == '\u{3a3}' { out.push(uc_sigma(cs, i)) } else { uc_push_lower(out, cs[i]) }
}

pub fn py_upper(t: &str) -> String {
    if t.is_ascii() {
        return t.to_ascii_uppercase();
    }
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        uc_push_upper(&mut out, c);
    }
    out
}

pub fn py_lower(t: &str) -> String {
    if t.is_ascii() {
        return t.to_ascii_lowercase();
    }
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    for i in 0..cs.len() {
        uc_push_lower_at(&mut out, &cs, i);
    }
    out
}

pub fn py_swapcase(t: &str) -> String {
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    for i in 0..cs.len() {
        let c = cs[i];
        if py_ch_isupper(c) {
            uc_push_lower_at(&mut out, &cs, i);
        } else if py_ch_islower(c) {
            uc_push_upper(&mut out, c);
        } else {
            out.push(c);
        }
    }
    out
}

pub fn py_capitalize(t: &str) -> String {
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    for i in 0..cs.len() {
        if i == 0 { uc_push_title(&mut out, cs[0]) } else { uc_push_lower_at(&mut out, &cs, i) }
    }
    out
}

pub fn py_title(t: &str) -> String {
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    let mut prev_cased = false;
    for i in 0..cs.len() {
        if prev_cased { uc_push_lower_at(&mut out, &cs, i) } else { uc_push_title(&mut out, cs[i]) }
        prev_cased = py_ch_iscased(cs[i]);
    }
    out
}

pub fn py_str_isupper(t: &str) -> bool {
    let mut cs = t.chars();
    if let (Some(c), None) = (cs.next(), cs.next()) {
        return py_ch_isupper(c);
    }
    let mut cased = false;
    for c in t.chars() {
        if py_ch_islower(c) || py_ch_istitle(c) {
            return false;
        }
        cased = cased || py_ch_isupper(c);
    }
    cased
}

pub fn py_str_islower(t: &str) -> bool {
    let mut cs = t.chars();
    if let (Some(c), None) = (cs.next(), cs.next()) {
        return py_ch_islower(c);
    }
    let mut cased = false;
    for c in t.chars() {
        if py_ch_isupper(c) || py_ch_istitle(c) {
            return false;
        }
        cased = cased || py_ch_islower(c);
    }
    cased
}

pub fn py_str_istitle(t: &str) -> bool {
    let mut cs = t.chars();
    if let (Some(c), None) = (cs.next(), cs.next()) {
        return py_ch_istitle(c) || py_ch_isupper(c);
    }
    let (mut cased, mut prev_cased) = (false, false);
    for c in t.chars() {
        if py_ch_isupper(c) || py_ch_istitle(c) {
            if prev_cased {
                return false;
            }
            prev_cased = true;
            cased = true;
        } else if py_ch_islower(c) {
            if !prev_cased {
                return false;
            }
            prev_cased = true;
            cased = true;
        } else {
            prev_cased = false;
        }
    }
    cased
}

pub fn py_str_isalpha(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_ch_isalpha)
}
pub fn py_str_isalnum(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_ch_isalnum)
}
pub fn py_str_isspace(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_ch_isspace)
}
"""


def ranges(points):
    out = []
    for u in points:
        if out and out[-1][1] == u - 1:
            out[-1][1] = u
        else:
            out.append([u, u])
    return "&[" + ", ".join(f"({a:#x}, {b:#x})" for a, b in out) + "]"


def rs_str(s):
    return '"' + "".join(f"\\u{{{ord(c):x}}}" for c in s) + '"'


def generate():
    to_upper, to_lower, to_title = [], [], []
    upper, lower, title, cased, ignorable, alpha, space = [], [], [], [], [], [], []
    for u in range(0x80, 0x110000):
        if 0xD800 <= u <= 0xDFFF:
            continue
        c = chr(u)
        up, lo, ti = c.upper(), c.lower(), c.title()
        if up != c:
            to_upper.append((u, up))
        if lo != c:
            to_lower.append((u, lo))
        if ti != up:
            to_title.append((u, ti))
        is_cased = (c + "a").title() == ti + "a"
        s1 = (c + "Σ").lower().endswith("ς")
        s2 = ("A" + c + "Σ").lower().endswith("ς")
        is_ign = s2 and not s1
        is_title = c.istitle() and not c.isupper()
        assert is_title == (unicodedata.category(c) == "Lt"), hex(u)
        assert not (c.isupper() or c.islower() or is_title) or is_cased, hex(u)
        for flag, tab in ((c.isupper(), upper), (c.islower(), lower), (is_title, title), (is_cased, cased),
                          (is_ign, ignorable), (c.isalpha(), alpha), (c.isspace(), space)):
            if flag:
                tab.append(u)
    # The ASCII fast paths in TAIL must be what CPython answers.
    for u in range(0x80):
        c = chr(u)
        assert ((c + "a").title() == c.title() + "a") == c.isalpha(), u
        ign = ("A" + c + "Σ").lower().endswith("ς") and not (c + "Σ").lower().endswith("ς")
        assert ign == (c in "'.:^`"), u
        assert c.isspace() == (c in " \t\n\x0b\x0c\r\x1c\x1d\x1e\x1f"), u
        assert c.title() == c.upper() and not (c.istitle() and not c.isupper()), u
    py = ".".join(map(str, sys.version_info[:2]))
    out = [HEADER.format(py=py, ud=unicodedata.unidata_version)]
    for name, doc, tab in (
        ("UC_TO_UPPER", "str.upper() of every non-ASCII character it changes (full mappings, SpecialCasing included).", to_upper),
        ("UC_TO_LOWER", "str.lower() of every non-ASCII character it changes (final sigma is handled in code).", to_lower),
        ("UC_TO_TITLE", "The title mapping of every character whose title form differs from its upper form.", to_title),
    ):
        out.append(f"// {doc}\nconst {name}: &[(u32, &str)] = &[\n")
        out.extend(f"    ({u:#x}, {rs_str(m)}),\n" for u, m in tab)
        out.append("];\n\n")
    for name, doc, pts in (
        ("UC_UPPER", "Non-ASCII characters str.isupper() counts as upper case (the Uppercase property).", upper),
        ("UC_LOWER", "Non-ASCII characters str.islower() counts as lower case (the Lowercase property).", lower),
        ("UC_TITLE", "Title case characters (category Lt).", title),
        ("UC_CASED", "Non-ASCII Cased characters (what str.title() looks at).", cased),
        ("UC_CASE_IGNORABLE", "Non-ASCII Case_Ignorable characters (the final sigma rule skips them).", ignorable),
        ("UC_ALPHA", "Non-ASCII characters str.isalpha() accepts (categories Lm, Lt, Lu, Ll, Lo).", alpha),
        ("UC_SPACE", "Non-ASCII characters str.isspace() accepts.", space),
    ):
        out.append(f"// {doc}\nconst {name}: &[(u32, u32)] = {ranges(pts)};\n\n")
    out.append(TAIL)
    return "".join(out)


def main():
    if sys.version_info[:2] != (3, 13):
        sys.exit("run this with python3.13: the tables must be CPython 3.13's")
    text = generate()
    if "--check" in sys.argv[1:]:
        path = Path(__file__).resolve().parent.parent / "src" / "unicode_case.rs"
        if path.read_text() != text:
            sys.exit("src/unicode_case.rs is stale: run python3.13 tools/gen_unicode_case.py > src/unicode_case.rs")
        return
    sys.stdout.write(text)


if __name__ == "__main__":
    main()
