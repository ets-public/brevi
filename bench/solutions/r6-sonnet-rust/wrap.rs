use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut ls=s.lines();let w:usize=ls.next().unwrap().trim().parse().unwrap();
let mut paras:Vec<Vec<String>>=vec![];let mut cur:Vec<String>=vec![];
for l in ls{if l.trim().is_empty(){if !cur.is_empty(){paras.push(std::mem::take(&mut cur))}}else{for x in l.split_whitespace(){cur.push(x.to_string())}}}
if !cur.is_empty(){paras.push(cur)}
let mut out=vec![];
for p in paras{let mut lines=vec![];let mut l=String::new();for x in p{if l.is_empty(){l=x}else if l.len()+1+x.len()<=w{l.push(' ');l.push_str(&x)}else{lines.push(std::mem::take(&mut l));l=x}}if !l.is_empty(){lines.push(l)}out.push(lines.join("\n"))}
println!("{}",out.join("\n\n"))}
