use std::collections::HashMap;
use std::io::{self, BufRead, Write};

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

// parse a decimal string into hundredths
fn to_cents(s: &str) -> i64 {
    let s = s.trim();
    let (neg, s) = if let Some(r) = s.strip_prefix('-') { (true, r) } else { (false, s) };
    let (ip, fp) = match s.find('.') {
        Some(p) => (&s[..p], &s[p + 1..]),
        None => (s, ""),
    };
    let ipv: i64 = if ip.is_empty() { 0 } else { ip.parse().unwrap_or(0) };
    let mut fpd: String = fp.chars().take(2).collect();
    while fpd.len() < 2 {
        fpd.push('0');
    }
    let fpv: i64 = fpd.parse().unwrap_or(0);
    let v = ipv * 100 + fpv;
    if neg { -v } else { v }
}

fn main() {
    let stdin = io::stdin();
    let mut totals: HashMap<String, i64> = HashMap::new();
    let mut first = true;
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        if first {
            first = false;
            if line.trim().to_lowercase().starts_with("item") {
                continue;
            }
        }
        let f = split_csv(line);
        if f.len() < 3 {
            continue;
        }
        let item = f[0].trim().to_string();
        let qty: i64 = f[1].trim().parse().unwrap_or(0);
        let price = to_cents(&f[2]);
        *totals.entry(item).or_insert(0) += qty * price;
    }
    let mut v: Vec<(String, i64)> = totals.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for (name, c) in v {
        let sign = if c < 0 { "-" } else { "" };
        let a = c.abs();
        writeln!(out, "{}: {}{}.{:02}", name, sign, a / 100, a % 100).unwrap();
    }
}
