use std::io::*;use std::collections::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let ls:Vec<Vec<&str>>=s.lines().map(|l|l.split_whitespace().collect()).filter(|v:&Vec<&str>|!v.is_empty()).collect();
let last=ls.last().unwrap();let (from,to)=(last[0].to_string(),last[1].to_string());
let mut g:HashMap<String,Vec<(String,u64)>>=HashMap::new();
for e in &ls[..ls.len()-1]{if e.len()<3{continue}let w:u64=e[2].parse().unwrap();g.entry(e[0].to_string()).or_default().push((e[1].to_string(),w));g.entry(e[1].to_string()).or_default().push((e[0].to_string(),w));}
let mut d:HashMap<String,u64>=HashMap::new();let mut h=BinaryHeap::new();d.insert(to.clone(),0);h.push(std::cmp::Reverse((0u64,to.clone())));
while let Some(std::cmp::Reverse((du,u)))=h.pop(){if du>d[&u]{continue}if let Some(ns)=g.get(&u){for (v,w) in ns{let nd=du+w;if d.get(v).map_or(true,|&x|nd<x){d.insert(v.clone(),nd);h.push(std::cmp::Reverse((nd,v.clone())));}}}}
if !d.contains_key(&from){println!("none");return}
println!("{}",d[&from]);let mut p=vec![from.clone()];let mut cur=from;
while cur!=to{let mut best:Option<String>=None;for (v,w) in &g[&cur]{if let Some(&dv)=d.get(v){if dv+w==d[&cur]{if best.as_ref().map_or(true,|b|v<b){best=Some(v.clone())}}}}cur=best.unwrap();p.push(cur.clone());}
println!("{}",p.join("->"))}
