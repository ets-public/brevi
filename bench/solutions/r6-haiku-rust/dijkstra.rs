use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize, u64)> = Vec::new();
    let mut query: Option<(String, String)> = None;
    let mut id_of = |s: &str, ids: &mut BTreeMap<String, usize>, names: &mut Vec<String>| -> usize {
        if let Some(&i) = ids.get(s) {
            i
        } else {
            let i = names.len();
            ids.insert(s.to_string(), i);
            names.push(s.to_string());
            i
        }
    };
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() == 3 {
            if let Ok(w) = t[2].parse::<u64>() {
                let a = id_of(t[0], &mut ids, &mut names);
                let b = id_of(t[1], &mut ids, &mut names);
                edges.push((a, b, w));
                continue;
            }
        }
        if t.len() == 2 {
            query = Some((t[0].to_string(), t[1].to_string()));
        }
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let (from, to) = match query {
        Some(q) => q,
        None => return,
    };
    let fi = id_of(&from, &mut ids, &mut names);
    let ti = id_of(&to, &mut ids, &mut names);
    let n = names.len();
    let mut adj: Vec<Vec<(usize, u64)>> = vec![Vec::new(); n];
    for &(a, b, w) in &edges {
        adj[a].push((b, w));
        adj[b].push((a, w));
    }
    // distances to target (graph is undirected)
    const INF: u64 = u64::MAX;
    let mut dist = vec![INF; n];
    dist[ti] = 0;
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((0u64, ti)));
    while let Some(Reverse((d, u))) = heap.pop() {
        if d > dist[u] {
            continue;
        }
        for &(v, w) in &adj[u] {
            let nd = d + w;
            if nd < dist[v] {
                dist[v] = nd;
                heap.push(Reverse((nd, v)));
            }
        }
    }
    if dist[fi] == INF {
        writeln!(out, "none").unwrap();
        return;
    }
    writeln!(out, "{}", dist[fi]).unwrap();
    // greedy lexicographically smallest walk along shortest-path DAG
    let mut path = vec![fi];
    let mut cur = fi;
    while cur != ti {
        let mut best: Option<usize> = None;
        for &(v, w) in &adj[cur] {
            if dist[v] != INF && w + dist[v] == dist[cur] {
                match best {
                    None => best = Some(v),
                    Some(b) => {
                        if names[v] < names[b] {
                            best = Some(v);
                        }
                    }
                }
            }
        }
        match best {
            Some(v) => {
                path.push(v);
                cur = v;
            }
            None => break,
        }
    }
    let s: Vec<&str> = path.iter().map(|&i| names[i].as_str()).collect();
    writeln!(out, "{}", s.join("->")).unwrap();
}
