use std::io::*;use std::collections::HashMap;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let mut m:HashMap<String,usize>=HashMap::new();for w in s.split_whitespace(){*m.entry(w.to_lowercase()).or_insert(0)+=1;}let mut v:Vec<_>=m.into_iter().collect();v.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(&b.0)));for (w,c) in v.into_iter().take(3){println!("{} {}",w,c);}}
