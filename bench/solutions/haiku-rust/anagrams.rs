use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    // key: sorted letters -> set of distinct words (BTreeSet keeps them sorted, dedups)
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for w in s.split_whitespace() {
        let mut chars: Vec<char> = w.chars().collect();
        chars.sort();
        let key: String = chars.into_iter().collect();
        groups.entry(key).or_default().insert(w.to_string());
    }
    let mut lines: Vec<String> = groups
        .values()
        .map(|set| set.iter().cloned().collect::<Vec<_>>().join(" "))
        .collect();
    lines.sort();
    for l in lines {
        println!("{}", l);
    }
}
