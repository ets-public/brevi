use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let mut lines = s.lines();

    let w: usize = loop {
        match lines.next() {
            Some(l) if !l.trim().is_empty() => break l.trim().parse().unwrap_or(80),
            Some(_) => continue,
            None => return,
        }
    };

    // Group the rest into paragraphs separated by blank lines.
    let mut paras: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                paras.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(line.to_string());
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }

    let mut out_paras: Vec<String> = Vec::new();
    for p in &paras {
        let words: Vec<&str> = p.iter().flat_map(|l| l.split_whitespace()).collect();
        if words.is_empty() {
            continue;
        }
        let mut out_lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut cur_len = 0usize;
        for word in words {
            let wl = word.chars().count();
            if cur_len == 0 {
                cur.push_str(word);
                cur_len = wl;
            } else if cur_len + 1 + wl <= w {
                cur.push(' ');
                cur.push_str(word);
                cur_len += 1 + wl;
            } else {
                out_lines.push(std::mem::take(&mut cur));
                cur.push_str(word);
                cur_len = wl;
            }
        }
        if cur_len > 0 {
            out_lines.push(cur);
        }
        out_paras.push(out_lines.join("\n"));
    }

    let out = out_paras.join("\n\n");
    if out.is_empty() {
        return;
    }
    println!("{}", out);
}
