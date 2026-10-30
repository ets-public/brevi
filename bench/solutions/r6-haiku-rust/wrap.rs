use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let w: usize = loop {
        match lines.next() {
            Some(Ok(l)) if !l.trim().is_empty() => break l.trim().parse().unwrap_or(80),
            Some(Ok(_)) => continue,
            _ => return,
        }
    };
    // collect paragraphs as word lists
    let mut paras: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for line in lines {
        let line = line.unwrap();
        if line.trim().is_empty() {
            if !cur.is_empty() {
                paras.push(std::mem::take(&mut cur));
            }
        } else {
            for word in line.split_whitespace() {
                cur.push(word.to_string());
            }
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for (pi, words) in paras.iter().enumerate() {
        if pi > 0 {
            writeln!(out).unwrap();
        }
        let mut line = String::new();
        for word in words {
            let wl = word.chars().count();
            if line.is_empty() {
                line.push_str(word);
            } else if line.chars().count() + 1 + wl <= w {
                line.push(' ');
                line.push_str(word);
            } else {
                writeln!(out, "{}", line).unwrap();
                line.clear();
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            writeln!(out, "{}", line).unwrap();
        }
    }
}
