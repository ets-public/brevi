use std::io::{self, BufRead};

fn main() {
    let mut entries: Vec<(String, i64)> = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        if let Ok(score) = parts[1].parse::<i64>() {
            entries.push((parts[0].to_string(), score));
        }
    }
    if entries.is_empty() {
        return;
    }
    let mut scores: Vec<i64> = entries.iter().map(|e| e.1).collect();
    scores.sort();
    let n = scores.len();
    let median = if n % 2 == 1 {
        scores[n / 2] as f64
    } else {
        (scores[n / 2 - 1] + scores[n / 2]) as f64 / 2.0
    };
    println!("{:.1}", median);

    let max = *scores.last().unwrap();
    let mut names: Vec<&str> = entries
        .iter()
        .filter(|e| e.1 == max)
        .map(|e| e.0.as_str())
        .collect();
    names.sort();
    names.dedup();
    for name in names {
        println!("{}", name);
    }
}
