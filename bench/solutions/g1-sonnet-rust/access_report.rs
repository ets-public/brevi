use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{stdin, Read};

struct Rec { ip: String, hour: String, path: String, status: u32, size: u64, ua: String }

fn digits(s: &str, n: usize) -> bool { s.len() == n && s.bytes().all(|c| c.is_ascii_digit()) }

fn parse(l: &str) -> Option<Rec> {
    let (ip, rest) = l.split_once(' ')?;
    if ip.is_empty() { return None; }
    let rest = rest.strip_prefix("- - [")?;
    let (ts, rest) = rest.split_once("] \"")?;
    let tb: Vec<char> = ts.chars().collect();
    // dd/Mon/yyyy:HH:MM:SS +zzzz
    if tb.len() != 26 { return None; }
    let t: String = ts.to_string();
    if !(digits(&t[0..2], 2) && &t[2..3] == "/" && t[3..6].bytes().all(|c| c.is_ascii_alphabetic()) && &t[6..7] == "/"
        && digits(&t[7..11], 4) && &t[11..12] == ":" && digits(&t[12..14], 2) && &t[14..15] == ":" && digits(&t[15..17], 2)
        && &t[17..18] == ":" && digits(&t[18..20], 2) && &t[20..21] == " " && (&t[21..22] == "+" || &t[21..22] == "-") && digits(&t[22..26], 4)) {
        return None;
    }
    let (req, rest) = rest.split_once("\" ")?;
    let rp: Vec<&str> = req.split(' ').collect();
    if rp.len() != 3 || rp.iter().any(|s| s.is_empty()) { return None; }
    let path = rp[1].split('?').next().unwrap().to_string();
    let mut it = rest.splitn(3, ' ');
    let st = it.next()?;
    let sz = it.next()?;
    let tail = it.next()?;
    if !digits(st, 3) { return None; }
    let size: u64 = if sz == "-" { 0 } else if !sz.is_empty() && sz.bytes().all(|c| c.is_ascii_digit()) { sz.parse().ok()? } else { return None };
    // tail: "ref" "ua"
    let tail = tail.strip_prefix('"')?;
    let (_r, tail) = tail.split_once("\" \"")?;
    let ua = tail.strip_suffix('"')?;
    Some(Rec { ip: ip.to_string(), hour: t[12..14].to_string(), path, status: st.parse().ok()?, size, ua: ua.to_string() })
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let (mut n, mut mal, mut bytes, mut bots) = (0u64, 0u64, 0u64, 0u64);
    let mut ips = HashSet::new();
    let mut classes: BTreeMap<u32, u64> = BTreeMap::new();
    let mut paths: HashMap<String, u64> = HashMap::new();
    let mut hours: BTreeMap<String, u64> = BTreeMap::new();
    let mut errs: HashMap<String, u64> = HashMap::new();
    for l in s.lines() {
        if l.trim().is_empty() { continue; }
        match parse(l.trim_end_matches('\r')) {
            None => mal += 1,
            Some(r) => {
                n += 1; bytes += r.size; ips.insert(r.ip.clone());
                *classes.entry(r.status / 100).or_insert(0) += 1;
                *paths.entry(r.path).or_insert(0) += 1;
                *hours.entry(r.hour).or_insert(0) += 1;
                if r.ua.to_lowercase().contains("bot") { bots += 1; }
                if r.status >= 400 { *errs.entry(r.ip).or_insert(0) += 1; }
            }
        }
    }
    println!("requests {}\nmalformed {}\nbytes {}\nunique_ips {}", n, mal, bytes, ips.len());
    println!("status");
    for (c, k) in &classes { println!("  {}xx {}", c, k); }
    println!("top_paths");
    let mut pv: Vec<(&String, &u64)> = paths.iter().collect();
    pv.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (p, c) in pv.iter().take(3) { println!("  {} {}", c, p); }
    println!("hours");
    let max = hours.values().copied().max().unwrap_or(1);
    for (h, c) in &hours {
        let bar = ((c * 20 * 2 + max) / (2 * max)).max(1) as usize;
        println!("  {} {} {}", h, c, "#".repeat(bar));
    }
    println!("bots {}", bots);
    println!("errors_by_ip");
    let mut ev: Vec<(&String, &u64)> = errs.iter().filter(|(_, c)| **c >= 2).collect();
    ev.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (ip, c) in ev { println!("  {} {}", ip, c); }
}
