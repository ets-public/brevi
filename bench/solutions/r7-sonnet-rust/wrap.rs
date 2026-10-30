use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let mut it=s.lines();let w:usize=it.next().unwrap().trim().parse().unwrap();
let mut paras:Vec<Vec<String>>=vec![];let mut cur:Vec<String>=vec![];
for l in it{if l.trim().is_empty(){if !cur.is_empty(){paras.push(std::mem::take(&mut cur))}}else{for x in l.split_whitespace(){cur.push(x.to_string())}}}
if !cur.is_empty(){paras.push(cur)}
let mut outp=vec![];
for p in paras{let mut lines:Vec<String>=vec![];let mut l=String::new();for x in p{if l.is_empty(){l=x}else if l.chars().count()+1+x.chars().count()<=w{l.push(' ');l.push_str(&x)}else{lines.push(std::mem::replace(&mut l,x))}}if !l.is_empty(){lines.push(l)}outp.push(lines.join("\n"))}
println!("{}",outp.join("\n\n"))}
