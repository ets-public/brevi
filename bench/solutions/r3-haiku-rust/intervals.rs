use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();

    let mut v: Vec<(i64, i64)> = Vec::new();
    for line in s.lines() {
        let p: Vec<i64> = line
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if p.len() >= 2 {
            v.push((p[0], p[1]));
        }
    }
    v.sort();

    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (st, en) in v {
        if let Some(last) = merged.last_mut() {
            if st <= last.1 {
                if en > last.1 {
                    last.1 = en;
                }
                continue;
            }
        }
        merged.push((st, en));
    }

    let mut out = String::new();
    let mut total: i64 = 0;
    for (st, en) in &merged {
        out.push_str(&format!("{} {}\n", st, en));
        total += en - st;
    }
    out.push_str(&format!("total {}\n", total));
    print!("{}", out);
}
