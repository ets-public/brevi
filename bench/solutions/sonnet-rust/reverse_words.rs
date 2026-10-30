use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();for l in s.lines(){let v:Vec<&str>=l.split_whitespace().rev().collect();println!("{}",v.join(" "));}}
