use std::io::{self, BufRead, Write};

// days since 1970-01-01 (proleptic Gregorian)
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_date(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.trim().split('-').collect();
    if p.len() != 3 {
        return None;
    }
    let y: i64 = p[0].parse().ok()?;
    let m: i64 = p[1].parse().ok()?;
    let d: i64 = p[2].parse().ok()?;
    Some(days_from_civil(y, m, d))
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 2 {
            continue;
        }
        if let (Some(a), Some(b)) = (parse_date(t[0]), parse_date(t[1])) {
            writeln!(out, "{}", (a - b).abs()).unwrap();
        }
    }
}
