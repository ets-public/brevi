use std::io::Read; use std::collections::HashMap;
fn main(){let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();
let mut t:HashMap<&str,(i64,i64)>=HashMap::new();
for l in s.lines(){let f:Vec<&str>=l.split(',').collect();if f.len()<3{continue}let e=t.entry(f[1]).or_insert((0,0));e.0+=f[2].parse::<i64>().unwrap();e.1+=1;}
let mut v:Vec<_>=t.into_iter().collect();v.sort_by(|a,b|b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
for (d,(x,c)) in v.iter().take(5){println!("{} {} {}",d,x,c);}}
