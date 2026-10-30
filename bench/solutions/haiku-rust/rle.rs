use std::io::{self, BufRead};

fn encode(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut j = i;
        while j < chars.len() && chars[j] == c {
            j += 1;
        }
        out.push(c);
        out.push_str(&(j - i).to_string());
        i = j;
    }
    out
}

fn decode(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        let mut n: usize = 0;
        while i < chars.len() && chars[i].is_ascii_digit() {
            n = n * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        for _ in 0..n {
            out.push(c);
        }
    }
    out
}

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if line.len() < 2 {
            continue;
        }
        let (op, rest) = line.split_at(1);
        let rest = &rest[1..];
        match op {
            "e" => println!("{}", encode(rest)),
            "d" => println!("{}", decode(rest)),
            _ => {}
        }
    }
}
