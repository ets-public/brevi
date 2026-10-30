use std::io::{self, BufRead, Write};

fn add(a: &str, b: &str) -> String {
    let a: Vec<u8> = a.bytes().rev().map(|c| c - b'0').collect();
    let b: Vec<u8> = b.bytes().rev().map(|c| c - b'0').collect();
    let n = a.len().max(b.len());
    let mut res: Vec<u8> = Vec::with_capacity(n + 1);
    let mut carry = 0u8;
    for i in 0..n {
        let s = carry
            + a.get(i).copied().unwrap_or(0)
            + b.get(i).copied().unwrap_or(0);
        res.push(s % 10);
        carry = s / 10;
    }
    if carry > 0 {
        res.push(carry);
    }
    while res.len() > 1 && *res.last().unwrap() == 0 {
        res.pop();
    }
    if res.is_empty() {
        return "0".to_string();
    }
    res.iter().rev().map(|d| (b'0' + d) as char).collect()
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let a = parts[0].trim_start_matches('0');
        let b = parts[1].trim_start_matches('0');
        writeln!(out, "{}", add(a, b)).unwrap();
    }
}
