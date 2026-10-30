use std::collections::HashMap;
use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut lines = s.lines();

    let cap: usize = loop {
        match lines.next() {
            Some(l) if !l.trim().is_empty() => break l.trim().parse().unwrap_or(1),
            Some(_) => continue,
            None => return,
        }
    };

    // key -> (value, last-use tick)
    let mut map: HashMap<String, (String, u64)> = HashMap::new();
    let mut tick: u64 = 0;
    let mut out = String::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, ' ');
        let cmd = parts.next().unwrap_or("");
        match cmd {
            "put" => {
                let key = parts.next().unwrap_or("").to_string();
                let val = parts.next().unwrap_or("").trim().to_string();
                tick += 1;
                if let Some(entry) = map.get_mut(&key) {
                    entry.0 = val;
                    entry.1 = tick;
                } else {
                    if map.len() >= cap {
                        let victim = map
                            .iter()
                            .min_by_key(|(_, (_, t))| *t)
                            .map(|(k, _)| k.clone());
                        if let Some(v) = victim {
                            map.remove(&v);
                        }
                    }
                    map.insert(key, (val, tick));
                }
            }
            "get" => {
                let key = parts.next().unwrap_or("").trim().to_string();
                tick += 1;
                if let Some(entry) = map.get_mut(&key) {
                    entry.1 = tick;
                    out.push_str(&format!("{}\n", entry.0));
                } else {
                    out.push_str("-1\n");
                }
            }
            _ => {}
        }
    }
    print!("{}", out);
}
