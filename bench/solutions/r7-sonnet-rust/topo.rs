use std::io::*;use std::collections::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut adj:BTreeMap<String,BTreeSet<String>>=BTreeMap::new();let mut indeg:BTreeMap<String,usize>=BTreeMap::new();
for l in s.lines(){let p:Vec<&str>=l.split("->").collect();if p.len()!=2{continue}let a=p[0].trim().to_string();let b=p[1].trim().to_string();
indeg.entry(a.clone()).or_insert(0);indeg.entry(b.clone()).or_insert(0);
if adj.entry(a).or_default().insert(b.clone()){*indeg.get_mut(&b).unwrap()+=1;}}
let mut h:BTreeSet<String>=indeg.iter().filter(|(_,&d)|d==0).map(|(k,_)|k.clone()).collect();
let mut out=vec![];
while let Some(n)=h.iter().next().cloned(){h.remove(&n);out.push(n.clone());if let Some(v)=adj.get(&n){for m in v{let d=indeg.get_mut(m).unwrap();*d-=1;if *d==0{h.insert(m.clone());}}}}
if out.len()!=indeg.len(){println!("cycle")}else{for o in out{println!("{}",o)}}}
