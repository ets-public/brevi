use std::io::*;use std::collections::*;
fn split(l:&str)->Vec<String>{let mut f=vec![];let mut c=String::new();let mut q=false;let ch:Vec<char>=l.chars().collect();let mut i=0;
while i<ch.len(){let x=ch[i];if q{if x=='"'{if i+1<ch.len()&&ch[i+1]=='"'{c.push('"');i+=1}else{q=false}}else{c.push(x)}}else if x=='"'{q=true}else if x==','{f.push(std::mem::take(&mut c))}else{c.push(x)}i+=1}
f.push(c);f}
fn cents(s:&str)->i64{let s=s.trim();let (neg,s)=if let Some(r)=s.strip_prefix('-'){(true,r)}else{(false,s)};
let (a,b)=match s.split_once('.'){Some((a,b))=>(a,b),None=>(s,"")};
let mut b=b.to_string();while b.len()<2{b.push('0')}
let v=a.parse::<i64>().unwrap_or(0)*100+b[..2].parse::<i64>().unwrap_or(0);if neg{-v}else{v}}
fn main(){let mut t=String::new();stdin().read_to_string(&mut t).unwrap();
let mut m:HashMap<String,i64>=HashMap::new();
for l in t.lines().skip(1){let l=l.trim_end_matches('\r');if l.trim().is_empty(){continue}let f=split(l);if f.len()<3{continue}
let q:i64=f[1].trim().parse().unwrap_or(0);*m.entry(f[0].clone()).or_insert(0)+=q*cents(&f[2]);}
let mut v:Vec<_>=m.into_iter().collect();v.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
for (k,c) in v{let s=if c<0{"-"}else{""};let c=c.abs();println!("{}: {}{}.{:02}",k,s,c/100,c%100)}}
