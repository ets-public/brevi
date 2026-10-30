use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut v:Vec<(i64,i64)>=vec![];
for l in s.lines(){let p:Vec<i64>=l.split_whitespace().filter_map(|x|x.parse().ok()).collect();if p.len()>=2{v.push((p[0],p[1]))}}
v.sort();let mut m:Vec<(i64,i64)>=vec![];
for (a,b) in v{if let Some(l)=m.last_mut(){if a<=l.1{if b>l.1{l.1=b}continue}}m.push((a,b))}
let mut t=0;for (a,b) in &m{println!("{} {}",a,b);t+=b-a}println!("total {}",t)}
