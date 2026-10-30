use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();
    let mut sum: i64 = 0;
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(v) = t.parse::<i64>() {
            sum += v;
        }
    }
    println!("{}", sum);
}
