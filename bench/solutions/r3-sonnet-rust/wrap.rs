use std::io::*;
fn main(){let mut t=String::new();stdin().read_to_string(&mut t).unwrap();
let mut ls=t.lines();let w:usize=ls.next().unwrap().trim().parse().unwrap();
let mut paras:Vec<Vec<String>>=vec![];let mut cur:Vec<String>=vec![];
for l in ls{let ws:Vec<&str>=l.split_whitespace().collect();if ws.is_empty(){if !cur.is_empty(){paras.push(std::mem::take(&mut cur))}}else{for x in ws{cur.push(x.to_string())}}}
if !cur.is_empty(){paras.push(cur)}
let mut outp=vec![];
for p in paras{let mut lines:Vec<String>=vec![];let mut l=String::new();
for x in p{if l.is_empty(){l=x}else if l.chars().count()+1+x.chars().count()<=w{l.push(' ');l.push_str(&x)}else{lines.push(std::mem::replace(&mut l,x))}}
if !l.is_empty(){lines.push(l)}outp.push(lines.join("\n"))}
println!("{}",outp.join("\n\n"));}
