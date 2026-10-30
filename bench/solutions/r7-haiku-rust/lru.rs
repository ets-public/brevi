use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let mut lines = stdin.lock().lines().map(|l| l.unwrap_or_default());
    let cap: usize = loop {
        match lines.next() {
            Some(l) => {
                let t = l.trim();
                if t.is_empty() {
                    continue;
                }
                break t.parse().unwrap_or(0);
            }
            None => return,
        }
    };

    // key -> (value, tick)
    let mut map: HashMap<String, (String, u64)> = HashMap::new();
    // tick -> key, smallest tick is least recently used
    let mut order: BTreeMap<u64, String> = BTreeMap::new();
    let mut tick: u64 = 0;

    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "get" if parts.len() >= 2 => {
                let key = parts[1];
                if let Some((val, old)) = map.get(key).cloned() {
                    order.remove(&old);
                    tick += 1;
                    order.insert(tick, key.to_string());
                    map.insert(key.to_string(), (val.clone(), tick));
                    writeln!(out, "{}", val).unwrap();
                } else {
                    writeln!(out, "-1").unwrap();
                }
            }
            "put" if parts.len() >= 3 => {
                let key = parts[1];
                let val = parts[2].to_string();
                tick += 1;
                if let Some((_, old)) = map.get(key).cloned() {
                    order.remove(&old);
                } else if map.len() >= cap {
                    if let Some((&t, _)) = order.iter().next() {
                        let k = order.remove(&t).unwrap();
                        map.remove(&k);
                    }
                }
                order.insert(tick, key.to_string());
                map.insert(key.to_string(), (val, tick));
            }
            _ => {}
        }
    }
}
