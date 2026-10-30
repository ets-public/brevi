use std::io::*;use std::collections::HashMap;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let mut it=s.lines();let n:usize=it.next().unwrap().trim().parse().unwrap();
let mut m:HashMap<String,(String,u64)>=HashMap::new();let mut t=0u64;
for l in it{let p:Vec<&str>=l.split_whitespace().collect();if p.is_empty(){continue}t+=1;
if p[0]=="get"{match m.get_mut(p[1]){Some(e)=>{e.1=t;println!("{}",e.0)}None=>println!("-1")}}
else if p.len()>=3{if let Some(e)=m.get_mut(p[1]){e.0=p[2].to_string();e.1=t}else{if m.len()>=n{let k=m.iter().min_by_key(|(_,v)|v.1).map(|(k,_)|k.clone()).unwrap();m.remove(&k);}m.insert(p[1].to_string(),(p[2].to_string(),t));}}}}
