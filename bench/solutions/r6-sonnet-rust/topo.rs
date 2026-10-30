use std::io::*;use std::collections::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut nodes=BTreeSet::new();let mut edges=BTreeSet::new();
for l in s.lines(){if let Some((a,b))=l.split_once("->"){let a=a.trim().to_string();let b=b.trim().to_string();nodes.insert(a.clone());nodes.insert(b.clone());edges.insert((a,b));}}
let mut indeg:BTreeMap<String,usize>=nodes.iter().map(|n|(n.clone(),0)).collect();
let mut adj:BTreeMap<String,Vec<String>>=BTreeMap::new();
for (a,b) in &edges{*indeg.get_mut(b).unwrap()+=1;adj.entry(a.clone()).or_default().push(b.clone());}
let mut ready:BTreeSet<String>=indeg.iter().filter(|(_,&d)|d==0).map(|(n,_)|n.clone()).collect();
let mut out=vec![];
while let Some(n)=ready.iter().next().cloned(){ready.remove(&n);out.push(n.clone());if let Some(v)=adj.get(&n){for m in v{let d=indeg.get_mut(m).unwrap();*d-=1;if *d==0{ready.insert(m.clone());}}}}
if out.len()<nodes.len(){println!("cycle")}else{for n in out{println!("{}",n)}}}
