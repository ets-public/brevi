use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let mut stack: Vec<char> = Vec::new();
        let mut ok = true;
        for c in line.chars() {
            match c {
                '(' | '[' | '{' => stack.push(c),
                ')' | ']' | '}' => {
                    let want = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    if stack.pop() != Some(want) {
                        ok = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if ok && stack.is_empty() {
            println!("yes");
        } else {
            println!("no");
        }
    }
}
