use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut ls=s.lines();let n:usize=ls.next().unwrap().trim().parse().unwrap();
let mut c:Vec<(String,String)>=vec![];
for l in ls{let p:Vec<&str>=l.split_whitespace().collect();if p.is_empty(){continue}
if p[0]=="put"{if let Some(i)=c.iter().position(|x|x.0==p[1]){c.remove(i);}else if c.len()>=n{c.remove(0);}c.push((p[1].to_string(),p[2].to_string()));}
else if let Some(i)=c.iter().position(|x|x.0==p[1]){let e=c.remove(i);println!("{}",e.1);c.push(e);}else{println!("-1")}}}
