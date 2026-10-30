use std::io::Read; use std::collections::BinaryHeap; use std::cmp::Reverse;
fn main(){let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();let mut it=s.lines();
let n:usize=it.next().unwrap().split_whitespace().next().unwrap().parse().unwrap();
let mut adj=vec![Vec::new();n];
for l in it{let v:Vec<usize>=l.split_whitespace().map(|x|x.parse().unwrap()).collect();if v.len()<3{continue}adj[v[0]].push((v[1],v[2] as i64));adj[v[1]].push((v[0],v[2] as i64));}
let inf=1_000_000_000i64;let mut d=vec![inf;n];d[0]=0;let mut h=BinaryHeap::new();h.push(Reverse((0i64,0usize)));
while let Some(Reverse((du,u)))=h.pop(){if du>d[u]{continue}for &(v,w) in &adj[u]{let nd=du+w;if nd<d[v]{d[v]=nd;h.push(Reverse((nd,v)));}}}
let r:Vec<i64>=d.into_iter().filter(|&x|x<inf).collect();println!("{} {}",r.len(),r.iter().sum::<i64>());}
