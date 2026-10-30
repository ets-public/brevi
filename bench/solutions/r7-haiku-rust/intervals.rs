use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut iv: Vec<(i64, i64)> = Vec::new();
    for line in input.lines() {
        let parts: Vec<i64> = line
            .split_whitespace()
            .filter_map(|t| t.parse::<i64>().ok())
            .collect();
        if parts.len() >= 2 {
            let (s, e) = (parts[0], parts[1]);
            let (s, e) = if s <= e { (s, e) } else { (e, s) };
            iv.push((s, e));
        }
    }
    iv.sort();

    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in iv {
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
    for &(s, e) in &merged {
        writeln!(out, "{} {}", s, e).unwrap();
        total += e - s;
    }
    writeln!(out, "total {}", total).unwrap();
}
