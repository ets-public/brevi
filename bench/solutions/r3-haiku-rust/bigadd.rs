use std::io::{self, Read};

fn add(a: &str, b: &str) -> String {
    let a: Vec<u8> = a.bytes().rev().map(|c| c - b'0').collect();
    let b: Vec<u8> = b.bytes().rev().map(|c| c - b'0').collect();
    let n = a.len().max(b.len());
    let mut res: Vec<u8> = Vec::with_capacity(n + 1);
    let mut carry = 0u8;
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        let s = x + y + carry;
        res.push(s % 10);
        carry = s / 10;
    }
    if carry > 0 {
        res.push(carry);
    }
    while res.len() > 1 && *res.last().unwrap() == 0 {
        res.pop();
    }
    res.iter().rev().map(|d| (d + b'0') as char).collect()
}

fn strip(s: &str) -> &str {
    let t = s.trim_start_matches('0');
    if t.is_empty() { "0" } else { t }
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut out = String::new();
    for line in s.lines() {
        let mut it = line.split_whitespace();
        let (a, b) = match (it.next(), it.next()) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };
        let r = add(strip(a), strip(b));
        out.push_str(&r);
        out.push('\n');
    }
    print!("{}", out);
}
