use std::io::*;use std::collections::*;
fn fields(l:&str)->Vec<String>{let mut r=vec![];let mut c=String::new();let mut q=false;let mut it=l.chars().peekable();
while let Some(ch)=it.next(){if q{if ch=='"'{if it.peek()==Some(&'"'){it.next();c.push('"')}else{q=false}}else{c.push(ch)}}else if ch=='"'{q=true}else if ch==','{r.push(std::mem::take(&mut c))}else{c.push(ch)}}r.push(c);r}
fn cents(s:&str)->i64{let s=s.trim();let (neg,s)=if let Some(r)=s.strip_prefix('-'){(true,r)}else{(false,s)};
let (a,b)=s.split_once('.').unwrap_or((s,""));let mut b=b.to_string();while b.len()<2{b.push('0')}
let v=a.parse::<i64>().unwrap_or(0)*100+b[..2].parse::<i64>().unwrap_or(0);if neg{-v}else{v}}
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
let mut m:BTreeMap<String,i64>=BTreeMap::new();
for l in s.lines().skip(1){if l.trim().is_empty(){continue}let f=fields(l);if f.len()<3{continue}
let q:i64=f[1].trim().parse().unwrap_or(0);*m.entry(f[0].trim().to_string()).or_insert(0)+=q*cents(&f[2]);}
let mut v:Vec<_>=m.into_iter().collect();v.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
for (k,c) in v{let sg=if c<0{"-"}else{""};let c=c.abs();println!("{}: {}{}.{:02}",k,sg,c/100,c%100)}}
