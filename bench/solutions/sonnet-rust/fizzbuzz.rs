use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let n:u64=s.trim().parse().unwrap_or(0);for i in 1..=n{if i%15==0{println!("FizzBuzz")}else if i%3==0{println!("Fizz")}else if i%5==0{println!("Buzz")}else{println!("{}",i)}}}
