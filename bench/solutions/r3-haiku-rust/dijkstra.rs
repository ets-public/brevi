use std::collections::BTreeMap;
use std::io::{self, Read};

fn dijkstra(adj: &Vec<Vec<(usize, u64)>>, src: usize) -> Vec<Option<u64>> {
    let n = adj.len();
    let mut dist: Vec<Option<u64>> = vec![None; n];
    let mut done = vec![false; n];
    dist[src] = Some(0);
    loop {
        let mut best: Option<usize> = None;
        for v in 0..n {
            if !done[v] {
                if let Some(d) = dist[v] {
                    match best {
                        None => best = Some(v),
                        Some(b) => {
                            if d < dist[b].unwrap() {
                                best = Some(v);
                            }
                        }
                    }
                }
            }
        }
        let u = match best {
            Some(u) => u,
            None => break,
        };
        done[u] = true;
        let du = dist[u].unwrap();
        for &(v, w) in &adj[u] {
            let nd = du + w;
            if dist[v].map_or(true, |dv| nd < dv) {
                dist[v] = Some(nd);
            }
        }
    }
    dist
}

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();

    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return;
    }
    let query = lines[lines.len() - 1];
    let qp: Vec<&str> = query.split_whitespace().collect();
    let (from, to) = (qp[0].to_string(), qp[1].to_string());

    let mut idx: BTreeMap<String, usize> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut get_id = |name: &str, idx: &mut BTreeMap<String, usize>, names: &mut Vec<String>| -> usize {
        if let Some(&i) = idx.get(name) {
            i
        } else {
            let i = names.len();
            names.push(name.to_string());
            idx.insert(name.to_string(), i);
            i
        }
    };

    let mut edges: Vec<(usize, usize, u64)> = Vec::new();
    for line in &lines[..lines.len() - 1] {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 3 {
            continue;
        }
        let a = get_id(p[0], &mut idx, &mut names);
        let b = get_id(p[1], &mut idx, &mut names);
        let w: u64 = p[2].parse().unwrap_or(0);
        edges.push((a, b, w));
    }
    let fi = get_id(&from, &mut idx, &mut names);
    let ti = get_id(&to, &mut idx, &mut names);

    let n = names.len();
    // Keep the minimum weight among parallel edges via a matrix.
    let mut best_w: Vec<Vec<Option<u64>>> = vec![vec![None; n]; n];
    for &(a, b, w) in &edges {
        if a == b {
            continue;
        }
        for (x, y) in [(a, b), (b, a)] {
            let cur = best_w[x][y];
            if cur.map_or(true, |c| w < c) {
                best_w[x][y] = Some(w);
            }
        }
    }
    let mut adj: Vec<Vec<(usize, u64)>> = vec![Vec::new(); n];
    for x in 0..n {
        for y in 0..n {
            if let Some(w) = best_w[x][y] {
                adj[x].push((y, w));
            }
        }
    }

    let d_from = dijkstra(&adj, fi);
    let d_to = dijkstra(&adj, ti);
    let total = match d_from[ti] {
        Some(d) => d,
        None => {
            println!("none");
            return;
        }
    };

    // Greedy lexicographically smallest walk along shortest-path edges.
    let mut path: Vec<usize> = vec![fi];
    let mut cur = fi;
    while cur != ti {
        let dc = d_from[cur].unwrap();
        let mut next: Option<usize> = None;
        for &(v, w) in &adj[cur] {
            let on_path = d_from[v].map_or(false, |dv| {
                dc + w == dv && dv + d_to[v].unwrap_or(u64::MAX / 4) == total
            });
            if on_path {
                match next {
                    None => next = Some(v),
                    Some(nv) => {
                        if names[v] < names[nv] {
                            next = Some(v);
                        }
                    }
                }
            }
        }
        match next {
            Some(v) => {
                path.push(v);
                cur = v;
            }
            None => break,
        }
    }

    println!("{}", total);
    let strs: Vec<&str> = path.iter().map(|&i| names[i].as_str()).collect();
    println!("{}", strs.join("->"));
}
