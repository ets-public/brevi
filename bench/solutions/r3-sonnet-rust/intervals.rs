use std::io::*;
fn main(){let mut t=String::new();stdin().read_to_string(&mut t).unwrap();
let mut v:Vec<(i64,i64)>=vec![];
for l in t.lines(){let p:Vec<i64>=l.split_whitespace().filter_map(|x|x.parse().ok()).collect();if p.len()>=2{v.push((p[0],p[1]))}}
v.sort();let mut m:Vec<(i64,i64)>=vec![];
for (s,e) in v{if let Some(l)=m.last_mut(){if s<=l.1{if e>l.1{l.1=e}continue}}m.push((s,e))}
let mut tot=0;for (s,e) in &m{println!("{} {}",s,e);tot+=e-s}
println!("total {}",tot)}
