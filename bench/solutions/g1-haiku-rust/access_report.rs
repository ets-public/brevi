use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};

struct Entry<'a> {
    ip: &'a str,
    hour: u32,
    path: &'a str,
    status: u32,
    size: u64,
    ua: &'a str,
}

// Parses a double-quoted field at the start of s. Returns (content, rest after the closing quote).
fn quoted(s: &str) -> Option<(&str, &str)> {
    let inner = s.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some((&inner[..end], &inner[end + 1..]))
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn parse_line(line: &str) -> Option<Entry<'_>> {
    let (ip, rest) = line.split_once(' ')?;
    if ip.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix("- - [")?;
    let (ts, rest) = rest.split_once(']')?;
    let rest = rest.strip_prefix(' ')?;

    // Timestamp: dd/Mon/yyyy:HH:MM:SS zone
    let hour_txt = ts.split(':').nth(1)?;
    if hour_txt.len() != 2 || !all_digits(hour_txt) {
        return None;
    }
    let hour: u32 = hour_txt.parse().ok()?;
    if hour > 23 {
        return None;
    }

    let (req, rest) = quoted(rest)?;
    let parts: Vec<&str> = req.split(' ').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let raw_path = parts[1];
    let path = match raw_path.find('?') {
        Some(q) => &raw_path[..q],
        None => raw_path,
    };

    let rest = rest.strip_prefix(' ')?;
    let (status_txt, rest) = rest.split_once(' ')?;
    if status_txt.len() != 3 || !all_digits(status_txt) {
        return None;
    }
    let status: u32 = status_txt.parse().ok()?;

    let (size_txt, rest) = rest.split_once(' ')?;
    let size: u64 = if size_txt == "-" {
        0
    } else if all_digits(size_txt) {
        size_txt.parse().ok()?
    } else {
        return None;
    };

    let (_referrer, rest) = quoted(rest)?;
    let rest = rest.strip_prefix(' ')?;
    let (ua, rest) = quoted(rest)?;
    if !rest.trim().is_empty() {
        return None;
    }

    Some(Entry {
        ip,
        hour,
        path,
        status,
        size,
        ua,
    })
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let mut malformed: u64 = 0;
    let mut entries: Vec<Entry> = Vec::new();
    // Keep lines alive for borrowing.
    let lines: Vec<String> = stdin
        .lock()
        .lines()
        .map(|l| l.unwrap_or_default())
        .collect();
    for line in &lines {
        let line = line.trim_end_matches('\r');
        match parse_line(line) {
            Some(e) => entries.push(e),
            None => malformed += 1,
        }
    }

    let requests = entries.len();
    let bytes: u64 = entries.iter().map(|e| e.size).sum();
    let unique_ips: HashSet<&str> = entries.iter().map(|e| e.ip).collect();

    writeln!(out, "requests {}", requests).unwrap();
    writeln!(out, "malformed {}", malformed).unwrap();
    writeln!(out, "bytes {}", bytes).unwrap();
    writeln!(out, "unique_ips {}", unique_ips.len()).unwrap();

    // Status classes
    let mut classes = [0u64; 6];
    for e in &entries {
        let c = (e.status / 100) as usize;
        if c < classes.len() {
            classes[c] += 1;
        }
    }
    writeln!(out, "status").unwrap();
    for c in 2..=5 {
        if classes[c] > 0 {
            writeln!(out, "  {}xx {}", c, classes[c]).unwrap();
        }
    }

    // Top paths
    let mut path_counts: HashMap<&str, u64> = HashMap::new();
    for e in &entries {
        *path_counts.entry(e.path).or_insert(0) += 1;
    }
    let mut paths: Vec<(&str, u64)> = path_counts.into_iter().collect();
    paths.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    writeln!(out, "top_paths").unwrap();
    for (p, n) in paths.iter().take(3) {
        writeln!(out, "  {} {}", n, p).unwrap();
    }

    // Hours
    let mut hour_counts = [0u64; 24];
    for e in &entries {
        hour_counts[e.hour as usize] += 1;
    }
    let max_hour = hour_counts.iter().copied().max().unwrap_or(0);
    writeln!(out, "hours").unwrap();
    for (h, &n) in hour_counts.iter().enumerate() {
        if n == 0 {
            continue;
        }
        // round(n * 20 / max), half up, at least 1
        let bar_len = ((2 * n * 20 + max_hour) / (2 * max_hour)).max(1);
        let bar = "#".repeat(bar_len as usize);
        writeln!(out, "  {:02} {} {}", h, n, bar).unwrap();
    }

    // Bots
    let bots = entries
        .iter()
        .filter(|e| e.ua.to_lowercase().contains("bot"))
        .count();
    writeln!(out, "bots {}", bots).unwrap();

    // Errors by IP
    let mut err_counts: HashMap<&str, u64> = HashMap::new();
    for e in &entries {
        if e.status >= 400 {
            *err_counts.entry(e.ip).or_insert(0) += 1;
        }
    }
    let mut errs: Vec<(&str, u64)> = err_counts.into_iter().filter(|(_, n)| *n >= 2).collect();
    errs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    writeln!(out, "errors_by_ip").unwrap();
    for (ip, n) in &errs {
        writeln!(out, "  {} {}", ip, n).unwrap();
    }

    out.flush().unwrap();
}
