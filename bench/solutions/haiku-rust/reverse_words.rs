use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let words: Vec<&str> = line.split_whitespace().rev().collect();
        println!("{}", words.join(" "));
    }
}
