use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();

    let mut lines = input.lines();
    let w: usize = loop {
        match lines.next() {
            Some(l) if !l.trim().is_empty() => break l.trim().parse().unwrap_or(80),
            Some(_) => continue,
            None => return,
        }
    };

    // Collect paragraphs: maximal runs of non-blank lines.
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    let mut cur_par: Vec<String> = Vec::new();
    for l in lines {
        if l.trim().is_empty() {
            if !cur_par.is_empty() {
                paragraphs.push(std::mem::take(&mut cur_par));
            }
        } else {
            cur_par.push(l.to_string());
        }
    }
    if !cur_par.is_empty() {
        paragraphs.push(cur_par);
    }

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut first = true;
    for par in &paragraphs {
        let words: Vec<&str> = par.iter().flat_map(|l| l.split_whitespace()).collect();
        if words.is_empty() {
            continue;
        }
        let mut wrapped: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut cur_len = 0usize;
        for word in words {
            let wl = word.chars().count();
            if cur.is_empty() {
                cur.push_str(word);
                cur_len = wl;
            } else if cur_len + 1 + wl <= w {
                cur.push(' ');
                cur.push_str(word);
                cur_len += 1 + wl;
            } else {
                wrapped.push(std::mem::take(&mut cur));
                cur.push_str(word);
                cur_len = wl;
            }
        }
        if !cur.is_empty() {
            wrapped.push(cur);
        }

        if !first {
            writeln!(out).unwrap();
        }
        first = false;
        for l in wrapped {
            writeln!(out, "{}", l).unwrap();
        }
    }
}
