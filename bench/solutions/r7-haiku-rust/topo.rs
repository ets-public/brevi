use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::cmp::Reverse;
use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for line in input.lines() {
        if let Some(idx) = line.find("->") {
            let a = line[..idx].trim().to_string();
            let b = line[idx + 2..].trim().to_string();
            if a.is_empty() || b.is_empty() {
                continue;
            }
            nodes.insert(a.clone());
            nodes.insert(b.clone());
            edges.insert((a, b));
        }
    }

    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let names: Vec<String> = nodes.iter().cloned().collect();
    for (i, n) in names.iter().enumerate() {
        ids.insert(n.clone(), i);
    }

    let n = names.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for (a, b) in &edges {
        let ia = ids[a];
        let ib = ids[b];
        adj[ia].push(ib);
        indeg[ib] += 1;
    }

    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();
    for i in 0..n {
        if indeg[i] == 0 {
            heap.push(Reverse((names[i].clone(), i)));
        }
    }

    let mut order: Vec<usize> = Vec::new();
    while let Some(Reverse((_, u))) = heap.pop() {
        order.push(u);
        for &v in &adj[u] {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                heap.push(Reverse((names[v].clone(), v)));
            }
        }
    }

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    if order.len() != n {
        writeln!(out, "cycle").unwrap();
    } else {
        for u in order {
            writeln!(out, "{}", names[u]).unwrap();
        }
    }
}
