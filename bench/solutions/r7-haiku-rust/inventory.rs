use std::collections::BTreeMap;
use std::io::{self, Read, Write};

// Split one CSV line into fields, honoring double quotes and "" escapes.
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_q {
            if c == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    cur.push('"');
                    i += 1;
                } else {
                    in_q = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_q = true;
        } else if c == ',' {
            fields.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
        i += 1;
    }
    fields.push(cur);
    fields
}

// Parse a decimal price into integer cents (at most 2 decimals).
fn parse_cents(s: &str) -> i64 {
    let s = s.trim();
    let (neg, body) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix('+') {
        (false, rest)
    } else {
        (false, s)
    };
    let mut parts = body.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");
    let whole: i64 = if int_part.is_empty() { 0 } else { int_part.parse().unwrap_or(0) };
    let mut frac: String = frac_part.chars().take(2).collect();
    while frac.len() < 2 {
        frac.push('0');
    }
    let f: i64 = frac.parse().unwrap_or(0);
    let v = whole * 100 + f;
    if neg { -v } else { v }
}

fn fmt_cents(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.abs();
    format!("{}{}.{:02}", sign, a / 100, a % 100)
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut totals: BTreeMap<String, i64> = BTreeMap::new();
    for (idx, line) in input.lines().enumerate() {
        if idx == 0 {
            continue; // header
        }
        if line.trim().is_empty() {
            continue;
        }
        let f = split_csv(line);
        if f.len() < 3 {
            continue;
        }
        let item = f[0].trim().to_string();
        let qty: i64 = f[1].trim().parse().unwrap_or(0);
        let cents = parse_cents(&f[2]);
        *totals.entry(item).or_insert(0) += qty * cents;
    }

    let mut rows: Vec<(String, i64)> = totals.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for (name, v) in rows {
        writeln!(out, "{}: {}", name, fmt_cents(v)).unwrap();
    }
}
