use std::collections::HashMap;
use std::io::{stdin, Read};

fn tm(s: &str) -> i32 {
    let (h, m) = s.split_once(':').unwrap_or((s, "0"));
    h.parse::<i32>().unwrap_or(0) * 60 + m.parse::<i32>().unwrap_or(0)
}

struct Node { train: usize, k: usize, st: usize, t: i32 }
type L = Option<(usize, Vec<String>)>;

fn key(l: &(usize, Vec<String>)) -> (usize, String) { (l.0, l.1.join(">")) }
fn better(a: &L, b: &L) -> bool {
    match (a, b) { (Some(x), Some(y)) => key(x) < key(y), (Some(_), None) => true, _ => false }
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let mut names: Vec<String> = vec![];
    let mut sid: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<Node> = vec![];
    let mut tnames: Vec<String> = vec![];
    let mut queries = vec![];
    let mut second = false;
    for line in s.lines() {
        let line = line.trim();
        if line == "---" { second = true; continue; }
        if line.is_empty() { continue; }
        let p: Vec<&str> = line.split_whitespace().collect();
        if !second {
            let ti = tnames.len();
            tnames.push(p[0].to_string());
            let mut k = 0;
            let mut i = 1;
            while i + 1 < p.len() {
                let id = *sid.entry(p[i].to_string()).or_insert_with(|| { names.push(p[i].to_string()); names.len() - 1 });
                nodes.push(Node { train: ti, k, st: id, t: tm(p[i + 1]) });
                k += 1; i += 2;
            }
        } else if p.len() >= 3 {
            queries.push((p[0].to_string(), p[1].to_string(), tm(p[2])));
        }
    }
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| (nodes[i].t, nodes[i].k));
    let mut by_st: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() { by_st.entry(n.st).or_default().push(i); }
    // previous node in same train
    let mut prev: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut last: HashMap<usize, usize> = HashMap::new();
    for i in 0..nodes.len() {
        let tr = nodes[i].train;
        prev[i] = last.get(&tr).copied();
        last.insert(tr, i);
    }
    for (from, to, q) in queries {
        let hhmm = |m: i32| format!("{:02}:{:02}", m / 60, m % 60);
        if from == to { println!("{} {} {} via -", from, to, hhmm(q)); continue; }
        let (f, d) = match (sid.get(&from), sid.get(&to)) { (Some(f), Some(d)) => (*f, *d), _ => { println!("{} {} unreachable", from, to); continue; } };
        let mut on: Vec<L> = vec![None; nodes.len()];
        let mut ride: Vec<L> = vec![None; nodes.len()];
        let mut best: Option<(i32, usize, String)> = None;
        for &i in &order {
            let n = &nodes[i];
            let mut cur: L = None;
            if let Some(p) = prev[i] {
                if let Some(l) = &on[p] { cur = Some(l.clone()); }
            }
            ride[i] = cur.clone();
            // boarding
            let mut cand: L = None;
            if n.st == f && n.t >= q {
                cand = Some((1, vec![tnames[n.train].clone()]));
            }
            for &j in &by_st[&n.st] {
                let u = &nodes[j];
                if u.train != n.train && u.t + 5 <= n.t && ride[j].is_some() && j != i {
                    // arrival via ride[j]
                    let l = ride[j].as_ref().unwrap();
                    let mut v = l.1.clone();
                    v.push(tnames[n.train].clone());
                    let c: L = Some((l.0 + 1, v));
                    if better(&c, &cand) { cand = c; }
                }
            }
            if better(&cand, &cur) { cur = cand; }
            on[i] = cur;
            if n.st == d {
                if let Some(l) = &ride[i] {
                    let k = key(l);
                    let c = (n.t, k.0, k.1);
                    if best.as_ref().map_or(true, |b| c < *b) { best = Some(c); }
                }
            }
        }
        match best {
            Some((t, _, l)) => println!("{} {} {} via {}", from, to, hhmm(t), l),
            None => println!("{} {} unreachable", from, to),
        }
    }
}
