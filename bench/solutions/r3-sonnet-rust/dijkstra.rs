use std::io::*;use std::collections::*;
fn main(){let mut t=String::new();stdin().read_to_string(&mut t).unwrap();
let ls:Vec<&str>=t.lines().filter(|l|!l.trim().is_empty()).collect();
let last:Vec<&str>=ls[ls.len()-1].split_whitespace().collect();let (from,to)=(last[0],last[1]);
let mut adj:HashMap<&str,Vec<(&str,u64)>>=HashMap::new();
for l in &ls[..ls.len()-1]{let p:Vec<&str>=l.split_whitespace().collect();if p.len()<3{continue}let w:u64=p[2].parse().unwrap();
adj.entry(p[0]).or_default().push((p[1],w));adj.entry(p[1]).or_default().push((p[0],w));}
let mut d:HashMap<&str,u64>=HashMap::new();let mut h=BinaryHeap::new();
d.insert(to,0);h.push(std::cmp::Reverse((0u64,to)));
while let Some(std::cmp::Reverse((du,u)))=h.pop(){if du>d[u]{continue}
if let Some(v)=adj.get(u){for &(n,w) in v{let nd=du+w;if d.get(n).map_or(true,|&x|nd<x){d.insert(n,nd);h.push(std::cmp::Reverse((nd,n)))}}}}
match d.get(from){None=>println!("none"),Some(&dist)=>{println!("{}",dist);let mut path=vec![from];let mut cur=from;
while cur!=to{let mut best:Option<&str>=None;for &(n,w) in &adj[cur]{if let Some(&dn)=d.get(n){if dn+w==d[cur]&&best.map_or(true,|b|n<b){best=Some(n)}}}
cur=best.unwrap();path.push(cur);}
println!("{}",path.join("->"));}}}
