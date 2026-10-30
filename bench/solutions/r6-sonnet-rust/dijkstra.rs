use std::io::*;use std::collections::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let ls:Vec<&str>=s.lines().filter(|l|!l.trim().is_empty()).collect();
let q:Vec<&str>=ls.last().unwrap().split_whitespace().collect();let (from,to)=(q[0].to_string(),q[1].to_string());
let mut adj:HashMap<String,Vec<(String,u64)>>=HashMap::new();
for l in &ls[..ls.len()-1]{let p:Vec<&str>=l.split_whitespace().collect();if p.len()<3{continue}let w:u64=p[2].parse().unwrap();
adj.entry(p[0].to_string()).or_default().push((p[1].to_string(),w));adj.entry(p[1].to_string()).or_default().push((p[0].to_string(),w));}
let mut d:HashMap<String,u64>=HashMap::new();let mut h=BinaryHeap::new();
d.insert(to.clone(),0);h.push((std::cmp::Reverse(0u64),to.clone()));
while let Some((std::cmp::Reverse(du),u))=h.pop(){if du>d[&u]{continue}if let Some(v)=adj.get(&u){for (n,w) in v{let nd=du+w;if d.get(n).map_or(true,|&x|nd<x){d.insert(n.clone(),nd);h.push((std::cmp::Reverse(nd),n.clone()));}}}}
match d.get(&from){None=>println!("none"),Some(&t)=>{println!("{}",t);let mut path=vec![from.clone()];let mut cur=from.clone();
while cur!=to{let dc=d[&cur];let mut best:Option<String>=None;for (n,w) in &adj[&cur]{if let Some(&dn)=d.get(n){if dn+w==dc&&best.as_ref().map_or(true,|b|n<b){best=Some(n.clone())}}}cur=best.unwrap();path.push(cur.clone());}
println!("{}",path.join("->"))}}}
