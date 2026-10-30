use std::io::{self, BufRead, Write};

fn add(a: &str, b: &str) -> String {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (mut i, mut j) = (a.len(), b.len());
    let mut carry = 0u8;
    let mut res: Vec<u8> = Vec::new();
    while i > 0 || j > 0 || carry > 0 {
        let da = if i > 0 { i -= 1; a[i] - b'0' } else { 0 };
        let db = if j > 0 { j -= 1; b[j] - b'0' } else { 0 };
        let s = da + db + carry;
        res.push(b'0' + s % 10);
        carry = s / 10;
    }
    while res.len() > 1 && *res.last().unwrap() == b'0' {
        res.pop();
    }
    if res.is_empty() {
        res.push(b'0');
    }
    res.reverse();
    String::from_utf8(res).unwrap()
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let a = parts[0].trim_start_matches('0');
        let b = parts[1].trim_start_matches('0');
        let a = if a.is_empty() { "0" } else { a };
        let b = if b.is_empty() { "0" } else { b };
        writeln!(out, "{}", add(a, b)).unwrap();
    }
}
