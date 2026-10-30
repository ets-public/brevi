use std::collections::BTreeMap;
use std::io::{self, BufRead};

fn main() {
    // dept -> (sum, count); BTreeMap keeps dept names sorted
    let mut depts: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut header_seen = false;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !header_seen {
            header_seen = true;
            if t.starts_with("name") {
                continue;
            }
        }
        let parts: Vec<&str> = t.split(',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            continue;
        }
        let salary: f64 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let e = depts.entry(parts[1].to_string()).or_insert((0.0, 0));
        e.0 += salary;
        e.1 += 1;
    }
    for (d, (sum, n)) in &depts {
        println!("{} {:.2}", d, sum / *n as f64);
    }
}
