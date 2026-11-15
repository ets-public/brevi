#!/usr/bin/env python3.13
"""Regenerate src/unicode.rs from CPython's own str methods.

Run with the CPython the conformance tests target (3.13, unicodedata 15.1.0):
    python3.13 tools/gen_unicode.py > src/unicode.rs
`--check` exits 1 if src/unicode.rs differs from what this script writes.
"""
import _sre
import sys
import unicodedata
from pathlib import Path

HEADER = """\
// Shared Unicode helpers (generated from CPython {py}, unicodedata {ud}): do not edit by hand.
// The typed build's prelude (src/emit.rs) appends this file only to programs that use casefold() or
// isdigit(), and the dynamic runtime compiles it as part of its runtime library.
// str.isdecimal/isnumeric are not compiled by the typed build (it routes them to the dynamic build),
// so their tables matter for the dynamic build only; they are here so both builds share one source.

// str.casefold() for every non-ASCII character whose folded form differs from the character itself:
// the full (C and F) CaseFolding mappings, so it does not depend on rustc's Unicode version.
const AX_CASEFOLD: &[(u32, &str)] = &[
"""

TAIL = """\
pub fn py_casefold(t: &str) -> String {
    if t.is_ascii() {
        return t.to_ascii_lowercase();
    }
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        if c.is_ascii() {
            out.push(c.to_ascii_lowercase());
        } else {
            let u = c as u32;
            match AX_CASEFOLD.binary_search_by_key(&u, |&(k, _)| k) {
                Ok(i) => out.push_str(AX_CASEFOLD[i].1),
                Err(_) => out.push(c),
            }
        }
    }
    out
}

fn uni_tab_has(tab: &[(u32, u32)], u: u32) -> bool {
    tab.binary_search_by(|&(a, b)| if b < u { std::cmp::Ordering::Less } else if a > u { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }).is_ok()
}

// A str CPython's compiler leaves as it is when it NFKC-normalizes an identifier: true when every character is
// ASCII or outside AX_NFKC_MAY_CHANGE (each such character is NFKC-invariant alone, a starter, and never the
// second half of a composition, so no sequence of them changes). False means it may change, not that it does.
pub fn py_nfkc_stable(t: &str) -> bool {
    t.chars().all(|c| c.is_ascii() || !uni_tab_has(AX_NFKC_MAY_CHANGE, c as u32))
}

pub fn py_is_decimal(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_digit();
    }
    uni_tab_has(AX_DECIMAL, c as u32)
}

pub fn py_is_digit(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_digit();
    }
    uni_tab_has(AX_DIGIT, c as u32)
}

// str.isnumeric(): everything str.isdigit() accepts plus Numeric_Type=Numeric characters.
pub fn py_is_numeric(c: char) -> bool {
    py_is_digit(c) || uni_tab_has(AX_NUMERIC, c as u32)
}

pub fn py_str_isdigit(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_is_digit)
}

pub fn py_str_isdecimal(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_is_decimal)
}

pub fn py_str_isnumeric(t: &str) -> bool {
    !t.is_empty() && t.chars().all(py_is_numeric)
}

fn py_sre_case(tab: &[(u32, u32, i32, u32)], c: u32) -> u32 {
    match tab.binary_search_by(|&(a, b, _, _)| if b < c { std::cmp::Ordering::Less } else if a > c { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal }) {
        Ok(i) if (c - tab[i].0) % tab[i].3 == 0 => (c as i64 + tab[i].2 as i64) as u32,
        _ => c,
    }
}

// _sre.unicode_tolower (re.IGNORECASE): CPython's simple lowercase of a code point, whatever rustc's tables say.
// Lone surrogates and every unassigned code point map to themselves.
pub fn py_sre_lower(c: u32) -> u32 {
    if c < 0x80 {
        return if (0x41..=0x5a).contains(&c) { c + 32 } else { c };
    }
    py_sre_case(AX_SRE_LOWER, c)
}

// _sre's sre_upper_unicode (Py_UNICODE_TOUPPER), which re.IGNORECASE uses for ranges and sets.
pub fn py_sre_upper(c: u32) -> u32 {
    if c < 0x80 {
        return if (0x61..=0x7a).contains(&c) { c - 32 } else { c };
    }
    py_sre_case(AX_SRE_UPPER, c)
}

// _sre.unicode_iscased: the code point has a different lowercase or uppercase.
pub fn py_sre_iscased(c: u32) -> bool {
    py_sre_lower(c) != c || py_sre_upper(c) != c
}
"""


def chars():
    for u in range(0x80, 0x110000):
        if 0xD800 <= u <= 0xDFFF:
            continue
        yield u, chr(u)


def ranges(pred):
    out = []
    for u, c in chars():
        if pred(c):
            if out and out[-1][1] == u - 1:
                out[-1][1] = u
            else:
                out.append([u, u])
    return "&[" + ", ".join(f"({a:#x}, {b:#x})" for a, b in out) + "]"


def case_runs(first):
    """(first, last, delta, step) runs over the non-ASCII code points `first` changes."""
    out = []
    for u, c in chars():
        t = ord(first(c))
        if t == u:
            continue
        d = t - u
        if out:
            a, b, dd, st = out[-1]
            # step 2 only merges when the code point between is unchanged: it would be the last run otherwise
            if dd == d and ((a == b and u - b in (1, 2)) or (a != b and u - b == st)):
                out[-1] = [a, u, d, u - b]
                continue
        out.append([u, u, d, 1])
    return "&[" + ", ".join(f"({a:#x}, {b:#x}, {d}, {st})" for a, b, d, st in out) + "]"


def sre_lower(c):
    # _sre.unicode_tolower is _PyUnicode_ToLowercase: the first character of the full lowercase mapping
    t = _sre.unicode_tolower(ord(c))
    assert t == ord(c.lower()[0])
    return chr(t)


SECONDS = None


def nfkc_may_change(c):
    global SECONDS
    if SECONDS is None:
        SECONDS = set()
        for _, x in chars():
            d = unicodedata.decomposition(x)
            if d and not d.startswith("<"):
                SECONDS.update(chr(int(h, 16)) for h in d.split()[1:])
        SECONDS.update(map(chr, range(0x1161, 0x1176)))
        SECONDS.update(map(chr, range(0x11A8, 0x11C3)))
    first = unicodedata.normalize("NFKD", c)[0]
    return (unicodedata.normalize("NFKC", c) != c or unicodedata.combining(c) != 0 or c in SECONDS
            or unicodedata.combining(first) != 0 or first in SECONDS)


def render():
    v = sys.version_info
    s = HEADER.format(py=f"{v.major}.{v.minor}", ud=unicodedata.unidata_version)
    for u, c in chars():
        f = c.casefold()
        if f != c:
            s += f'    ({u:#x}, "' + "".join(x if x.isascii() and x.isalnum() else f"\\u{{{ord(x):x}}}" for x in f) + '"),\n'
    s += "];\n\n"
    s += "// Ranges of non-ASCII characters str.isdecimal() accepts: Nd decimal digits only (Numeric_Type=Decimal).\n"
    s += f"const AX_DECIMAL: &[(u32, u32)] = {ranges(str.isdecimal)};\n\n"
    s += "// Ranges of non-ASCII characters str.isdigit() accepts: Nd decimal digits and Numeric_Type=Digit\n"
    s += "// (superscripts and the like), but not Numeric_Type=Numeric characters such as fractions.\n"
    s += f"const AX_DIGIT: &[(u32, u32)] = {ranges(str.isdigit)};\n\n"
    s += "// Ranges of non-ASCII characters str.isnumeric() accepts beyond str.isdigit(): Numeric_Type=Numeric\n"
    s += "// (fractions, number words, and the Unihan numerals), not Nd or Digit (those are in AX_DIGIT above).\n"
    s += f"const AX_NUMERIC: &[(u32, u32)] = {ranges(lambda c: c.isnumeric() and not c.isdigit())};\n\n"
    s += "// Ranges of non-ASCII characters a string's NFKC form may differ at: those NFKC changes alone, those with\n"
    s += "// a nonzero combining class or whose NFKD starts with one, and those that can be the second character of a\n"
    s += "// canonical composition (including Hangul medial and final jamo). Used by py_nfkc_stable.\n"
    s += f"const AX_NFKC_MAY_CHANGE: &[(u32, u32)] = {ranges(nfkc_may_change)};\n\n"
    s += "// _sre's case mappings (re.IGNORECASE): the first character of each non-ASCII code point's full\n"
    s += "// lowercase / uppercase mapping (_PyUnicode_ToLowercase / _PyUnicode_ToUppercase), as runs\n"
    s += "// (first, last, delta, step): first, first + step, ... last map to themselves + delta, the code points\n"
    s += "// between them to themselves. Every other code point maps to itself.\n"
    s += f"const AX_SRE_LOWER: &[(u32, u32, i32, u32)] = {case_runs(sre_lower)};\n"
    s += f"const AX_SRE_UPPER: &[(u32, u32, i32, u32)] = {case_runs(lambda c: c.upper()[0])};\n\n"
    return s + TAIL


if __name__ == "__main__":
    text = render()
    if "--check" in sys.argv:
        path = Path(__file__).resolve().parent.parent / "src" / "unicode.rs"
        if path.read_text() != text:
            sys.exit("src/unicode.rs is stale: run python3.13 tools/gen_unicode.py > src/unicode.rs")
    else:
        sys.stdout.write(text)
