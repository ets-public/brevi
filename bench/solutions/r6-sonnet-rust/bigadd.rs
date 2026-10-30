use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
for l in s.lines(){let p:Vec<&str>=l.split_whitespace().collect();if p.len()<2{continue}
let a:Vec<u8>=p[0].bytes().rev().map(|c|c-b'0').collect();let b:Vec<u8>=p[1].bytes().rev().map(|c|c-b'0').collect();
let mut r=vec![];let mut c=0;for i in 0..a.len().max(b.len()){let x=a.get(i).copied().unwrap_or(0)+b.get(i).copied().unwrap_or(0)+c;r.push(x%10);c=x/10}
if c>0{r.push(c)}while r.len()>1&&*r.last().unwrap()==0{r.pop();}
println!("{}",r.iter().rev().map(|d|(d+b'0') as char).collect::<String>())}}
