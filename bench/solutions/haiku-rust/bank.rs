use std::collections::HashMap;
use std::io::{self, BufRead};

fn main() {
    let mut accounts: HashMap<String, i64> = HashMap::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "deposit" if parts.len() >= 3 => {
                let amt: i64 = parts[2].parse().unwrap_or(0);
                *accounts.entry(parts[1].to_string()).or_insert(0) += amt;
            }
            "withdraw" if parts.len() >= 3 => {
                let amt: i64 = parts[2].parse().unwrap_or(0);
                let bal = accounts.entry(parts[1].to_string()).or_insert(0);
                if amt > *bal {
                    println!("error: insufficient funds for {}", parts[1]);
                } else {
                    *bal -= amt;
                }
            }
            "balance" if parts.len() >= 2 => {
                let bal = accounts.get(parts[1]).copied().unwrap_or(0);
                println!("{} {}", parts[1], bal);
            }
            _ => {}
        }
    }
}
