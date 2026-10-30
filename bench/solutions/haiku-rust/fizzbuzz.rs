use std::io::{self, Read};

fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let n: i64 = s.trim().parse().unwrap_or(0);
    let mut out = String::new();
    for i in 1..=n {
        if i % 15 == 0 {
            out.push_str("FizzBuzz\n");
        } else if i % 3 == 0 {
            out.push_str("Fizz\n");
        } else if i % 5 == 0 {
            out.push_str("Buzz\n");
        } else {
            out.push_str(&format!("{}\n", i));
        }
    }
    print!("{}", out);
}
