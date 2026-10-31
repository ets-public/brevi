use std::collections::HashMap;
use std::io::{stdin, Read};

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let mut m: HashMap<String, String> = HashMap::new();
    // each tx: map key -> original Option<value>
    let mut txs: Vec<HashMap<String, Option<String>>> = vec![];
    let mut out = String::new();
    for line in s.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.is_empty() { continue; }
        macro_rules! rec {
            ($k:expr) => {
                if let Some(t) = txs.last_mut() {
                    if !t.contains_key($k) { t.insert($k.to_string(), m.get($k).cloned()); }
                }
            };
        }
        match (p[0], p.len()) {
            ("SET", 3) => { rec!(p[1]); m.insert(p[1].to_string(), p[2].to_string()); }
            ("GET", 2) => { out += m.get(p[1]).map(|s| s.as_str()).unwrap_or("NULL"); out += "\n"; }
            ("DEL", 2) => {
                if m.contains_key(p[1]) { rec!(p[1]); m.remove(p[1]); } else { out += "NOT FOUND\n"; }
            }
            ("INCR", 3) => {
                let n: Result<i64, _> = p[2].parse();
                let cur: Result<i64, _> = match m.get(p[1]) { None => Ok(0), Some(v) => v.parse() };
                match (cur, n) {
                    (Ok(c), Ok(n)) => {
                        rec!(p[1]);
                        let v = c + n;
                        m.insert(p[1].to_string(), v.to_string());
                        out += &format!("{}\n", v);
                    }
                    _ => out += "ERR not an integer\n",
                }
            }
            ("COUNT", 2) => { out += &format!("{}\n", m.values().filter(|v| v.as_str() == p[1]).count()); }
            ("KEYS", 2) => {
                let mut k: Vec<&String> = m.keys().filter(|k| k.starts_with(p[1])).collect();
                k.sort();
                if k.is_empty() { out += "(empty)\n"; } else {
                    out += &k.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "); out += "\n";
                }
            }
            ("BEGIN", 1) => txs.push(HashMap::new()),
            ("ROLLBACK", 1) => match txs.pop() {
                None => out += "NO TRANSACTION\n",
                Some(t) => for (k, v) in t {
                    match v { Some(v) => { m.insert(k, v); } None => { m.remove(&k); } }
                },
            },
            ("COMMIT", 1) => { if txs.is_empty() { out += "NO TRANSACTION\n"; } else { txs.clear(); } }
            _ => out += "ERR unknown command\n",
        }
    }
    print!("{}", out);
}
