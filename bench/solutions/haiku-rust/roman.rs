use std::io::{self, BufRead};

fn to_roman(mut n: u32) -> String {
    let table = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
        (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut s = String::new();
    for &(v, sym) in table.iter() {
        while n >= v {
            s.push_str(sym);
            n -= v;
        }
    }
    s
}

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(n) = t.parse::<u32>() {
            println!("{}", to_roman(n));
        }
    }
}
