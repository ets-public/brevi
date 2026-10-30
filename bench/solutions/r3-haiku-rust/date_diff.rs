use std::io::{self, Read};

// Days since 1970-01-01 in the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.trim().split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    Some(days_from_civil(y, m, d))
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut out = String::new();
    for line in s.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 2 {
            continue;
        }
        if let (Some(a), Some(b)) = (parse_date(p[0]), parse_date(p[1])) {
            out.push_str(&format!("{}\n", (a - b).abs()));
        }
    }
    print!("{}", out);
}
