use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let t:i64=s.lines().filter_map(|l|l.trim().parse::<i64>().ok()).sum();println!("{}",t);}
