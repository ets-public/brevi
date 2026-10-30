use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut v: Vec<(i64, i64)> = Vec::new();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let nums: Vec<i64> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() >= 2 {
            let (a, b) = (nums[0], nums[1]);
            v.push(if a <= b { (a, b) } else { (b, a) });
        }
    }
    v.sort();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in v {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        merged.push((s, e));
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut total: i64 = 0;
    for (s, e) in &merged {
        writeln!(out, "{} {}", s, e).unwrap();
        total += e - s;
    }
    writeln!(out, "total {}", total).unwrap();
}
