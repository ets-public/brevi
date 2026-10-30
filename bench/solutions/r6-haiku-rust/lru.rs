use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let cap: usize = loop {
        match lines.next() {
            Some(Ok(l)) if !l.trim().is_empty() => break l.trim().parse().unwrap_or(1),
            Some(Ok(_)) => continue,
            _ => return,
        }
    };
    // key -> (value, stamp); stamp -> key
    let mut map: HashMap<String, (String, u64)> = HashMap::new();
    let mut order: BTreeMap<u64, String> = BTreeMap::new();
    let mut clock: u64 = 0;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in lines {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        clock += 1;
        match parts[0] {
            "put" if parts.len() >= 3 => {
                let key = parts[1].to_string();
                let val = parts[2].to_string();
                if let Some((_, old_stamp)) = map.get(&key).cloned() {
                    order.remove(&old_stamp);
                } else if map.len() >= cap {
                    if let Some((&old, _)) = order.iter().next() {
                        let k = order.remove(&old).unwrap();
                        map.remove(&k);
                    }
                }
                order.insert(clock, key.clone());
                map.insert(key, (val, clock));
            }
            "get" if parts.len() >= 2 => {
                let key = parts[1];
                match map.get(key).cloned() {
                    Some((val, old_stamp)) => {
                        writeln!(out, "{}", val).unwrap();
                        order.remove(&old_stamp);
                        order.insert(clock, key.to_string());
                        map.get_mut(key).unwrap().1 = clock;
                    }
                    None => writeln!(out, "-1").unwrap(),
                }
            }
            _ => {}
        }
    }
}
