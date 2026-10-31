use std::collections::HashSet;
use std::io::{self, Read, Write};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// Heading: 1 to 6 '#' followed by a space. Returns (level, text).
fn heading(line: &str) -> Option<(usize, &str)> {
    let h = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&h) && line.as_bytes().get(h) == Some(&b' ') {
        Some((h, line[h + 1..].trim()))
    } else {
        None
    }
}

fn ul_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
}

fn ol_item(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0 {
        line[digits..].strip_prefix(". ")
    } else {
        None
    }
}

fn is_block_start(line: &str) -> bool {
    line == "```" || heading(line).is_some() || ul_item(line).is_some() || ol_item(line).is_some()
}

fn slug(text: &str) -> String {
    let mut s = String::new();
    let mut last_dash = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
            last_dash = false;
        } else if !last_dash {
            s.push('-');
            last_dash = true;
        }
    }
    s.trim_matches('-').to_string()
}

// Replaces `d ... d` pairs (non-empty content) with open/close.
fn pair_pass(v: &[char], d: &[char], open: &str, close: &str) -> Vec<char> {
    let k = d.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        if i + k <= v.len() && v[i..i + k] == *d {
            let start = i + k;
            let mut found = None;
            let mut j = start + 1;
            while j + k <= v.len() {
                if v[j..j + k] == *d {
                    found = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(j) = found {
                out.extend(open.chars());
                out.extend(&v[start..j]);
                out.extend(close.chars());
                i = j + k;
                continue;
            }
        }
        out.push(v[i]);
        i += 1;
    }
    out
}

// [text](url) -> <a href="url">text</a>
fn link_pass(v: &[char]) -> Vec<char> {
    let mut out = Vec::new();
    let mut i = 0;
    'outer: while i < v.len() {
        if v[i] == '[' {
            for j in i + 2..v.len() {
                if v[j] == ']' && j + 1 < v.len() && v[j + 1] == '(' {
                    for k in j + 3..v.len() {
                        if v[k] == ')' {
                            let text: String = v[i + 1..j].iter().collect();
                            let url: String = v[j + 2..k].iter().collect();
                            out.extend(format!("<a href=\"{}\">{}</a>", url, text).chars());
                            i = k + 1;
                            continue 'outer;
                        }
                    }
                    break;
                }
            }
        }
        out.push(v[i]);
        i += 1;
    }
    out
}

fn inline(s: &str) -> String {
    let e: Vec<char> = esc(s).chars().collect();
    // Code spans first; their content is kept out of later passes.
    let mut codes: Vec<String> = Vec::new();
    let mut v: Vec<char> = Vec::new();
    let mut i = 0;
    while i < e.len() {
        if e[i] == '`' {
            if let Some(j) = (i + 1..e.len()).find(|&j| e[j] == '`') {
                codes.push(e[i + 1..j].iter().collect());
                v.push('\u{1}');
                v.extend((codes.len() - 1).to_string().chars());
                v.push('\u{1}');
                i = j + 1;
                continue;
            }
        }
        v.push(e[i]);
        i += 1;
    }
    let v = pair_pass(&v, &['*', '*'], "<strong>", "</strong>");
    let v = pair_pass(&v, &['*'], "<em>", "</em>");
    let v = link_pass(&v);

    let mut out = String::new();
    let mut i = 0;
    while i < v.len() {
        if v[i] == '\u{1}' {
            let mut j = i + 1;
            let mut num = String::new();
            while j < v.len() && v[j] != '\u{1}' {
                num.push(v[j]);
                j += 1;
            }
            let idx: usize = num.parse().unwrap_or(0);
            out.push_str("<code>");
            out.push_str(&codes[idx]);
            out.push_str("</code>");
            i = j + 1;
        } else {
            out.push(v[i]);
            i += 1;
        }
    }
    out
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let lines: Vec<&str> = input.lines().collect();
    let n = lines.len();

    let mut blocks: Vec<String> = Vec::new();
    let mut heads: Vec<(String, String)> = Vec::new(); // (id, raw text)
    let mut used: HashSet<String> = HashSet::new();

    let mut i = 0;
    while i < n {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        if line == "```" {
            i += 1;
            let mut code = Vec::new();
            while i < n && lines[i] != "```" {
                code.push(esc(lines[i]));
                i += 1;
            }
            i += 1; // closing fence
            blocks.push(format!("<pre><code>{}</code></pre>", code.join("\n")));
            continue;
        }

        if let Some((level, text)) = heading(line) {
            let base = slug(text);
            let mut cand = base.clone();
            let mut k = 2;
            while used.contains(&cand) {
                cand = format!("{}-{}", base, k);
                k += 1;
            }
            used.insert(cand.clone());
            blocks.push(format!(
                "<h{l} id=\"{id}\">{body}</h{l}>",
                l = level,
                id = cand,
                body = inline(text)
            ));
            heads.push((cand, text.to_string()));
            i += 1;
            continue;
        }

        if ul_item(line).is_some() {
            let mut items = Vec::new();
            while i < n {
                match ul_item(lines[i]) {
                    Some(t) => items.push(format!("<li>{}</li>", inline(t.trim()))),
                    None => break,
                }
                i += 1;
            }
            blocks.push(format!("<ul>\n{}\n</ul>", items.join("\n")));
            continue;
        }

        if ol_item(line).is_some() {
            let mut items = Vec::new();
            while i < n {
                match ol_item(lines[i]) {
                    Some(t) => items.push(format!("<li>{}</li>", inline(t.trim()))),
                    None => break,
                }
                i += 1;
            }
            blocks.push(format!("<ol>\n{}\n</ol>", items.join("\n")));
            continue;
        }

        // Paragraph
        let mut parts = Vec::new();
        while i < n && !lines[i].trim().is_empty() && !is_block_start(lines[i]) {
            parts.push(lines[i].trim());
            i += 1;
        }
        blocks.push(format!("<p>{}</p>", inline(&parts.join(" "))));
    }

    let mut output: Vec<String> = Vec::new();
    if heads.len() >= 2 {
        output.push("<nav>".to_string());
        for (id, raw) in &heads {
            let plain: String = raw.chars().filter(|&c| c != '`' && c != '*').collect();
            output.push(format!("<a href=\"#{}\">{}</a>", id, esc(&plain)));
        }
        output.push("</nav>".to_string());
    }
    output.extend(blocks);

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for l in output {
        writeln!(out, "{}", l).unwrap();
    }
    out.flush().unwrap();
}
