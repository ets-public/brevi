use std::collections::HashMap;
use std::io::{self, Read};

// Split one CSV line into raw fields, honoring double quotes.
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                    cur.push('"');
                } else {
                    in_q = false;
                    cur.push('"');
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_q = true;
            cur.push('"');
        } else if c == ',' {
            fields.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    fields.push(cur);
    fields
}

fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].replace("\"\"", "\"")
    } else {
        t.to_string()
    }
}

// Parse a decimal string into integer cents (up to 2 decimals).
fn to_cents(s: &str) -> i64 {
    let t = s.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let (ip, fp) = match t.split_once('.') {
        Some((a, b)) => (a, b),
        None => (t, ""),
    };
    let ipv: i64 = if ip.is_empty() { 0 } else { ip.parse().unwrap_or(0) };
    let mut frac = String::from(fp);
    while frac.len() < 2 {
        frac.push('0');
    }
    let fpv: i64 = frac[..2].parse().unwrap_or(0);
    let v = ipv * 100 + fpv;
    if neg { -v } else { v }
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut totals: HashMap<String, i64> = HashMap::new();
    let mut header_seen = false;
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !header_seen {
            header_seen = true;
            if line.trim_start().to_ascii_lowercase().starts_with("item") {
                continue;
            }
        }
        let f = split_csv(line);
        if f.len() < 3 {
            continue;
        }
        let item = unquote(&f[0]);
        let qty: i64 = unquote(&f[1]).trim().parse().unwrap_or(0);
        let price = to_cents(&unquote(&f[2]));
        *totals.entry(item).or_insert(0) += qty * price;
    }

    let mut rows: Vec<(String, i64)> = totals.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut out = String::new();
    for (name, v) in rows {
        let sign = if v < 0 { "-" } else { "" };
        let av = v.abs();
        out.push_str(&format!("{}: {}{}.{:02}\n", name, sign, av / 100, av % 100));
    }
    print!("{}", out);
}
