use std::collections::HashMap; use std::io::Read;
fn main(){let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();
let mut c:HashMap<&str,i64>=HashMap::new();for w in s.split_whitespace(){*c.entry(w).or_insert(0)+=1;}
let mut v:Vec<_>=c.into_iter().collect();v.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(b.0)));
for (w,n) in v.iter().take(10){println!("{} {}",w,n);}}
