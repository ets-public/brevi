// Appended to a dynamic build whose source holds a `\N{` and that uses re: unicodedata.lookup for re's \N{NAME}
// (re turns named sequences down, as the table does). UNAMES is src/pynames/unicode_names.txt, which
// tools/unicode_names.py describes; it leaves out the names CPython computes, which are computed here.
// The table is read into a map on the first lookup (some 40,000 names); later patterns look up in it.
fn __uname_table(want: &str) -> Option<char> {
    thread_local! {
        static MAP: std::collections::HashMap<String, u32> = {
            let mut m = std::collections::HashMap::with_capacity(41000);
            let (mut cur, mut code) = (String::new(), u32::MAX);
            for line in UNAMES.lines() {
                let (text, hex) = line.split_once('\t').map_or((line, None), |(t, h)| (t, Some(h)));
                code = hex.map_or(code.wrapping_add(1), |h| u32::from_str_radix(h, 16).unwrap());
                cur.truncate((text.as_bytes()[0] - 33) as usize);
                cur.push_str(&text[1..]);
                m.insert(cur.clone(), code);
            }
            m
        };
    }
    MAP.with(|m| m.get(want).copied()).and_then(char::from_u32)
}
// Unicode 15.1 (CPython 3.13's unicodedata)
const __CJK: &[(u32, u32)] = &[
    (0x3400, 0x4dbf), (0x4e00, 0x9fff), (0x20000, 0x2a6df), (0x2a700, 0x2b739), (0x2b740, 0x2b81d), (0x2b820, 0x2cea1),
    (0x2ceb0, 0x2ebe0), (0x2ebf0, 0x2ee5d), (0x30000, 0x3134a), (0x31350, 0x323af),
];
const __JL: [&str; 19] = ["G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P", "H"];
const __JV: [&str; 21] = ["A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO", "WE", "WI", "YU", "EU", "YI", "I"];
const __JT: [&str; 28] = ["", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH", "M", "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H"];
fn __uname(name: &str) -> Option<char> {
    let want = name.to_ascii_uppercase();
    if let Some(h) = want.strip_prefix("CJK UNIFIED IDEOGRAPH-") {
        // only the name unicodedata.name gives (4 or 5 upper hex digits, no leading zero)
        let v = u32::from_str_radix(h, 16).ok().filter(|_| h.bytes().all(|b| b.is_ascii_hexdigit()))?;
        if format!("{v:04X}") != h || !__CJK.iter().any(|&(a, b)| (a..=b).contains(&v)) {
            return None;
        }
        return char::from_u32(v);
    }
    if let Some(j) = want.strip_prefix("HANGUL SYLLABLE ") {
        for (l, lj) in __JL.iter().enumerate() {
            let Some(r) = j.strip_prefix(lj) else { continue };
            for (v, vj) in __JV.iter().enumerate() {
                let Some(r) = r.strip_prefix(vj) else { continue };
                if let Some(t) = __JT.iter().position(|t| *t == r) {
                    return char::from_u32(0xAC00 + ((l * 21 + v) * 28 + t) as u32);
                }
            }
        }
        return None;
    }
    __uname_table(&want)
}
