use std::cmp::Reverse;
use std::collections::{BinaryHeap, BTreeMap, BTreeSet};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut indeg: BTreeMap<String, usize> = BTreeMap::new();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split("->").collect();
        if parts.len() != 2 {
            continue;
        }
        let a = parts[0].trim().to_string();
        let b = parts[1].trim().to_string();
        if a.is_empty() || b.is_empty() {
            continue;
        }
        nodes.insert(a.clone());
        nodes.insert(b.clone());
        adj.entry(a.clone()).or_default().push(b.clone());
        *indeg.entry(b.clone()).or_insert(0) += 1;
        indeg.entry(a).or_insert(0);
    }
    let names: Vec<String> = nodes.iter().cloned().collect();
    let mut idx: BTreeMap<String, usize> = BTreeMap::new();
    for (i, n) in names.iter().enumerate() {
        idx.insert(n.clone(), i);
    }
    let mut deg = vec![0usize; names.len()];
    let mut g: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
    for (a, outs) in &adj {
        let ai = idx[a];
        for b in outs {
            let bi = idx[b];
            g[ai].push(bi);
            deg[bi] += 1;
        }
    }
    let mut heap: BinaryHeap<Reverse<usize>> = BinaryHeap::new();
    for i in 0..names.len() {
        if deg[i] == 0 {
            heap.push(Reverse(i));
        }
    }
    let mut order: Vec<usize> = Vec::new();
    while let Some(Reverse(u)) = heap.pop() {
        order.push(u);
        for &v in &g[u] {
            deg[v] -= 1;
            if deg[v] == 0 {
                heap.push(Reverse(v));
            }
        }
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    if order.len() != names.len() {
        writeln!(out, "cycle").unwrap();
    } else {
        for u in order {
            writeln!(out, "{}", names[u]).unwrap();
        }
    }
}
