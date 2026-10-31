use std::collections::HashSet;
use std::io::{stdin, Read};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn find(c: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if c.len() < pat.len() { return None; }
    let mut i = from;
    while i + pat.len() <= c.len() {
        if &c[i..i + pat.len()] == pat { return Some(i); }
        i += 1;
    }
    None
}

fn inline(s: &str) -> String {
    let c: Vec<char> = esc(s).chars().collect();
    inl(&c)
}

fn inl(c: &[char]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '`' {
            if let Some(j) = find(c, i + 1, &['`']) {
                out += "<code>";
                out.extend(&c[i + 1..j]);
                out += "</code>";
                i = j + 1;
                continue;
            }
        } else if ch == '*' && c.get(i + 1) == Some(&'*') {
            if let Some(j) = find(c, i + 2, &['*', '*']) {
                if j > i + 2 {
                    out += "<strong>";
                    out += &inl(&c[i + 2..j]);
                    out += "</strong>";
                    i = j + 2;
                    continue;
                }
            }
        } else if ch == '*' {
            if let Some(j) = find(c, i + 1, &['*']) {
                if j > i + 1 {
                    out += "<em>";
                    out += &inl(&c[i + 1..j]);
                    out += "</em>";
                    i = j + 1;
                    continue;
                }
            }
        } else if ch == '[' {
            if let Some(j) = find(c, i + 1, &[']']) {
                if c.get(j + 1) == Some(&'(') {
                    if let Some(k) = find(c, j + 2, &[')']) {
                        out += "<a href=\"";
                        out.extend(&c[j + 2..k]);
                        out += "\">";
                        out += &inl(&c[i + 1..j]);
                        out += "</a>";
                        i = k + 1;
                        continue;
                    }
                }
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn heading(l: &str) -> Option<(usize, &str)> {
    let n = l.chars().take_while(|&c| c == '#').count();
    if n >= 1 && n <= 6 && l[n..].starts_with(' ') { Some((n, l[n + 1..].trim())) } else { None }
}
fn ordered(l: &str) -> Option<&str> {
    let n = l.chars().take_while(|c| c.is_ascii_digit()).count();
    if n > 0 && l[n..].starts_with(". ") { Some(&l[n + 2..]) } else { None }
}

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let lines: Vec<&str> = s.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut body: Vec<String> = vec![];
    let mut heads: Vec<(String, String)> = vec![];
    let mut used: HashSet<String> = HashSet::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() { i += 1; continue; }
        if l == "```" {
            let mut j = i + 1;
            let mut v = vec![];
            while j < lines.len() && lines[j] != "```" { v.push(esc(lines[j])); j += 1; }
            body.push(format!("<pre><code>{}</code></pre>", v.join("\n")));
            i = j + 1;
            continue;
        }
        if let Some((n, t)) = heading(l) {
            let mut id = String::new();
            let mut dash = false;
            for ch in t.to_lowercase().chars() {
                if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
                    if dash && !id.is_empty() { id.push('-'); }
                    dash = false;
                    id.push(ch);
                } else { dash = true; }
            }
            let mut fin = id.clone();
            let mut k = 2;
            while used.contains(&fin) { fin = format!("{}-{}", id, k); k += 1; }
            used.insert(fin.clone());
            body.push(format!("<h{} id=\"{}\">{}</h{}>", n, fin, inline(t), n));
            heads.push((fin, esc(&t.replace('`', "").replace('*', ""))));
            i += 1;
            continue;
        }
        if l.starts_with("- ") {
            body.push("<ul>".into());
            while i < lines.len() && lines[i].starts_with("- ") {
                body.push(format!("<li>{}</li>", inline(&lines[i][2..])));
                i += 1;
            }
            body.push("</ul>".into());
            continue;
        }
        if ordered(l).is_some() {
            body.push("<ol>".into());
            while i < lines.len() {
                match ordered(lines[i]) { Some(t) => body.push(format!("<li>{}</li>", inline(t))), None => break }
                i += 1;
            }
            body.push("</ol>".into());
            continue;
        }
        let mut v = vec![];
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() || l == "```" || heading(l).is_some() || l.starts_with("- ") || ordered(l).is_some() { break; }
            v.push(inline(l));
            i += 1;
        }
        body.push(format!("<p>{}</p>", v.join(" ")));
    }
    let mut out = String::new();
    if heads.len() >= 2 {
        out += "<nav>\n";
        for (id, t) in &heads { out += &format!("<a href=\"#{}\">{}</a>\n", id, t); }
        out += "</nav>\n";
    }
    for b in body { out += &b; out.push('\n'); }
    print!("{}", out);
}
