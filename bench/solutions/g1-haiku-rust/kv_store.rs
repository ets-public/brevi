use std::collections::HashMap;
use std::io::{self, BufRead, Write};

type UndoLog = Vec<(String, Option<String>)>;

struct Store {
    data: HashMap<String, String>,
    // One undo log per open transaction; the last entry is the innermost.
    txs: Vec<UndoLog>,
}

impl Store {
    fn new() -> Self {
        Store {
            data: HashMap::new(),
            txs: Vec::new(),
        }
    }

    // Writes (Some) or removes (None) a key, recording the previous value
    // in the innermost open transaction so it can be undone.
    fn write(&mut self, key: &str, value: Option<String>) {
        let old = match value {
            Some(v) => self.data.insert(key.to_string(), v),
            None => self.data.remove(key),
        };
        if let Some(log) = self.txs.last_mut() {
            log.push((key.to_string(), old));
        }
    }

    fn rollback(&mut self) -> bool {
        match self.txs.pop() {
            None => false,
            Some(log) => {
                for (key, old) in log.into_iter().rev() {
                    match old {
                        Some(v) => {
                            self.data.insert(key, v);
                        }
                        None => {
                            self.data.remove(&key);
                        }
                    }
                }
                true
            }
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut store = Store::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let cmd = parts[0];
        let args = &parts[1..];

        match (cmd, args.len()) {
            ("SET", 2) => {
                store.write(args[0], Some(args[1].to_string()));
            }
            ("GET", 1) => match store.data.get(args[0]) {
                Some(v) => writeln!(out, "{}", v).unwrap(),
                None => writeln!(out, "NULL").unwrap(),
            },
            ("DEL", 1) => {
                if store.data.contains_key(args[0]) {
                    store.write(args[0], None);
                } else {
                    writeln!(out, "NOT FOUND").unwrap();
                }
            }
            ("INCR", 2) => {
                let delta: i64 = match args[1].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        writeln!(out, "ERR not an integer").unwrap();
                        continue;
                    }
                };
                let current: i64 = match store.data.get(args[0]) {
                    None => 0,
                    Some(v) => match v.parse() {
                        Ok(n) => n,
                        Err(_) => {
                            writeln!(out, "ERR not an integer").unwrap();
                            continue;
                        }
                    },
                };
                match current.checked_add(delta) {
                    Some(new) => {
                        store.write(args[0], Some(new.to_string()));
                        writeln!(out, "{}", new).unwrap();
                    }
                    None => {
                        writeln!(out, "ERR not an integer").unwrap();
                    }
                }
            }
            ("COUNT", 1) => {
                let n = store.data.values().filter(|v| v.as_str() == args[0]).count();
                writeln!(out, "{}", n).unwrap();
            }
            ("KEYS", 1) => {
                let mut keys: Vec<&String> = store
                    .data
                    .keys()
                    .filter(|k| k.starts_with(args[0]))
                    .collect();
                keys.sort();
                if keys.is_empty() {
                    writeln!(out, "(empty)").unwrap();
                } else {
                    let joined: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                    writeln!(out, "{}", joined.join(" ")).unwrap();
                }
            }
            ("BEGIN", 0) => {
                store.txs.push(Vec::new());
            }
            ("ROLLBACK", 0) => {
                if !store.rollback() {
                    writeln!(out, "NO TRANSACTION").unwrap();
                }
            }
            ("COMMIT", 0) => {
                if store.txs.is_empty() {
                    writeln!(out, "NO TRANSACTION").unwrap();
                } else {
                    store.txs.clear();
                }
            }
            _ => {
                writeln!(out, "ERR unknown command").unwrap();
            }
        }
    }
    out.flush().unwrap();
}
