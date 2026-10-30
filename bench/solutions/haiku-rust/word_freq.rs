use std::collections::HashMap;
use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for w in s.split_whitespace() {
        *counts.entry(w.to_lowercase()).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (w, c) in v.iter().take(3) {
        println!("{} {}", w, c);
    }
}
