use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();

    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for line in s.lines() {
        if let Some((a, b)) = line.split_once("->") {
            let a = a.trim().to_string();
            let b = b.trim().to_string();
            if a.is_empty() || b.is_empty() {
                continue;
            }
            nodes.insert(a.clone());
            nodes.insert(b.clone());
            edges.insert((a, b));
        }
    }

    let mut indeg: BTreeMap<String, usize> = nodes.iter().map(|n| (n.clone(), 0)).collect();
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (a, b) in &edges {
        *indeg.get_mut(b).unwrap() += 1;
        adj.entry(a.clone()).or_default().push(b.clone());
    }

    let mut ready: BTreeSet<String> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();

    let mut order: Vec<String> = Vec::new();
    while let Some(n) = ready.iter().next().cloned() {
        ready.remove(&n);
        if let Some(vs) = adj.get(&n) {
            for v in vs {
                let d = indeg.get_mut(v).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.insert(v.clone());
                }
            }
        }
        order.push(n);
    }

    if order.len() < nodes.len() {
        println!("cycle");
    } else {
        for n in order {
            println!("{}", n);
        }
    }
}
