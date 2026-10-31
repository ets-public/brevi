use std::io::{self, Read, Write};

struct Train {
    name: String,
    stops: Vec<(String, i64)>,
}

fn parse_time(s: &str) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    let h: i64 = h.parse().ok()?;
    let m: i64 = m.parse().ok()?;
    if (0..24).contains(&h) && (0..60).contains(&m) {
        Some(h * 60 + m)
    } else {
        None
    }
}

fn fmt_time(t: i64) -> String {
    format!("{:02}:{:02}", t / 60, t % 60)
}

// Best journey so far: (arrival, number of trains, joined train list).
type Best = Option<(i64, usize, String)>;

struct Search<'a> {
    trains: &'a [Train],
    to: &'a str,
    best: Best,
}

impl<'a> Search<'a> {
    // `ready` is the earliest departure allowed at `station`.
    // `last` is the train we arrived on (boarding it again is not a change).
    fn dfs(
        &mut self,
        station: &str,
        ready: i64,
        last: Option<usize>,
        path: &mut Vec<usize>,
        visited: &mut Vec<String>,
    ) {
        for ti in 0..self.trains.len() {
            if Some(ti) == last {
                continue;
            }
            let stops = &self.trains[ti].stops;
            for k in 0..stops.len() {
                if stops[k].0 != station || stops[k].1 < ready {
                    continue;
                }
                path.push(ti);
                for j in k + 1..stops.len() {
                    let (st, arr) = (&stops[j].0, stops[j].1);
                    if visited.iter().any(|v| v == st) {
                        continue;
                    }
                    if st == self.to {
                        let names: Vec<&str> = path
                            .iter()
                            .map(|&t| self.trains[t].name.as_str())
                            .collect();
                        let cand = (arr, path.len(), names.join(">"));
                        let better = match &self.best {
                            None => true,
                            Some(b) => cand < *b,
                        };
                        if better {
                            self.best = Some(cand);
                        }
                        break; // continuing past TO never helps
                    }
                    if let Some(b) = &self.best {
                        // Any further journey through here arrives after arr + 5.
                        if arr + 5 > b.0 {
                            continue;
                        }
                    }
                    let st = st.clone();
                    visited.push(st.clone());
                    self.dfs(&st, arr + 5, Some(ti), path, visited);
                    visited.pop();
                }
                path.pop();
            }
        }
    }
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut trains: Vec<Train> = Vec::new();
    let mut queries: Vec<&str> = Vec::new();
    let mut in_queries = false;

    for line in input.lines() {
        let line = line.trim();
        if line == "---" {
            in_queries = true;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if in_queries {
            queries.push(line);
        } else {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 3 || toks.len() % 2 == 0 {
                continue;
            }
            let mut stops = Vec::new();
            let mut ok = true;
            for pair in toks[1..].chunks(2) {
                match parse_time(pair[1]) {
                    Some(t) => stops.push((pair[0].to_string(), t)),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                trains.push(Train {
                    name: toks[0].to_string(),
                    stops,
                });
            }
        }
    }

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for q in queries {
        let toks: Vec<&str> = q.split_whitespace().collect();
        if toks.len() != 3 {
            continue;
        }
        let (from, to) = (toks[0], toks[1]);
        let t0 = match parse_time(toks[2]) {
            Some(t) => t,
            None => continue,
        };

        if from == to {
            writeln!(out, "{} {} {} via -", from, to, fmt_time(t0)).unwrap();
            continue;
        }

        let mut search = Search {
            trains: &trains,
            to,
            best: None,
        };
        let mut path = Vec::new();
        let mut visited = vec![from.to_string()];
        search.dfs(from, t0, None, &mut path, &mut visited);

        match search.best {
            Some((arr, _, joined)) => {
                writeln!(out, "{} {} {} via {}", from, to, fmt_time(arr), joined).unwrap();
            }
            None => {
                writeln!(out, "{} {} unreachable", from, to).unwrap();
            }
        }
    }
    out.flush().unwrap();
}
