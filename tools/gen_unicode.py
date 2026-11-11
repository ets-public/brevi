#!/usr/bin/env python3.13
"""Regenerate src/unicode.rs from CPython's own str methods.

Run with the CPython the conformance tests target (3.13, unicodedata 15.1.0):
    python3.13 tools/gen_unicode.py > src/unicode.rs
`--check` exits 1 if src/unicode.rs differs from what this script writes.
"""
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
    return s + TAIL


if __name__ == "__main__":
    text = render()
    if "--check" in sys.argv:
        path = Path(__file__).resolve().parent.parent / "src" / "unicode.rs"
        if path.read_text() != text:
            sys.exit("src/unicode.rs is stale: run python3.13 tools/gen_unicode.py > src/unicode.rs")
    else:
        sys.stdout.write(text)
