use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let lines: Vec<Vec<&str>> = input
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<&str>>())
        .filter(|v| !v.is_empty())
        .collect();
    if lines.is_empty() {
        return;
    }
    let query = lines.last().unwrap().clone();
    let from = query.get(0).copied().unwrap_or("").to_string();
    let to = query.get(1).copied().unwrap_or("").to_string();

    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize, u64)> = Vec::new();
    let mut get_id = |s: &str, ids: &mut BTreeMap<String, usize>, names: &mut Vec<String>| -> usize {
        if let Some(&i) = ids.get(s) {
            i
        } else {
            let i = names.len();
            names.push(s.to_string());
            ids.insert(s.to_string(), i);
            i
        }
    };
    for v in &lines[..lines.len() - 1] {
        if v.len() < 3 {
            continue;
        }
        let w: u64 = match v[2].parse() {
            Ok(w) => w,
            Err(_) => continue,
        };
        let a = get_id(v[0], &mut ids, &mut names);
        let b = get_id(v[1], &mut ids, &mut names);
        edges.push((a, b, w));
    }

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    if from == to {
        writeln!(out, "0").unwrap();
        writeln!(out, "{}", from).unwrap();
        return;
    }

    let n = names.len();
    let (fi, ti) = match (ids.get(&from), ids.get(&to)) {
        (Some(&f), Some(&t)) => (f, t),
        _ => {
            writeln!(out, "none").unwrap();
            return;
        }
    };

    let mut adj: Vec<Vec<(usize, u64)>> = vec![Vec::new(); n];
    for &(a, b, w) in &edges {
        adj[a].push((b, w));
        adj[b].push((a, w));
    }

    // Shortest distances from the target (graph is undirected).
    const INF: u64 = u64::MAX;
    let mut dist = vec![INF; n];
    dist[ti] = 0;
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    heap.push(Reverse((0, ti)));
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

    // Walk from the source, always taking the smallest-named neighbour that stays on a shortest path.
    let mut path: Vec<usize> = vec![fi];
    let mut cur = fi;
    while cur != ti {
        let mut best: Option<usize> = None;
        for &(v, w) in &adj[cur] {
            if dist[v] != INF && dist[v] + w == dist[cur] {
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

    writeln!(out, "{}", dist[fi]).unwrap();
    let s: Vec<&str> = path.iter().map(|&i| names[i].as_str()).collect();
    writeln!(out, "{}", s.join("->")).unwrap();
}
