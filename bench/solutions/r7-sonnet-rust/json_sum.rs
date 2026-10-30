use std::io::*;
struct P{c:Vec<u8>,i:usize}
enum V{N(i64),S(String),A(Vec<V>),O(Vec<(String,V)>),Z}
impl P{
fn ws(&mut self){while self.i<self.c.len()&&(self.c[self.i] as char).is_whitespace(){self.i+=1}}
fn st(&mut self)->String{self.i+=1;let mut o=vec![];while self.c[self.i]!=b'"'{if self.c[self.i]==b'\\'{self.i+=1;match self.c[self.i]{b'n'=>o.push(b'\n'),b't'=>o.push(b'\t'),b'u'=>{o.push(b'?');self.i+=4}x=>o.push(x)}}else{o.push(self.c[self.i])}self.i+=1}self.i+=1;String::from_utf8_lossy(&o).into_owned()}
fn val(&mut self)->V{self.ws();let ch=self.c[self.i];match ch{
b'"'=>V::S(self.st()),
b'['=>{self.i+=1;let mut v=vec![];loop{self.ws();if self.c[self.i]==b']'{self.i+=1;break}if self.c[self.i]==b','{self.i+=1;continue}v.push(self.val())}V::A(v)}
b'{'=>{self.i+=1;let mut v=vec![];loop{self.ws();if self.c[self.i]==b'}'{self.i+=1;break}if self.c[self.i]==b','{self.i+=1;continue}let k=self.st();self.ws();self.i+=1;let x=self.val();v.push((k,x))}V::O(v)}
b'-'|b'0'..=b'9'=>{let s=self.i;self.i+=1;while self.i<self.c.len()&&(self.c[self.i].is_ascii_digit()||matches!(self.c[self.i],b'.'|b'e'|b'E'|b'+'|b'-')){self.i+=1}V::N(std::str::from_utf8(&self.c[s..self.i]).unwrap().parse::<i64>().unwrap_or(0))}
_=>{while self.i<self.c.len()&&self.c[self.i].is_ascii_alphabetic(){self.i+=1}V::Z}}}}
fn sum(v:&V)->i64{match v{V::N(n)=>*n,V::A(a)=>a.iter().map(sum).sum(),V::O(o)=>{if o.iter().any(|(_,x)|matches!(x,V::S(s) if s=="red")){0}else{o.iter().map(|(_,x)|sum(x)).sum()}}_=>0}}
fn main(){let mut s=Vec::new();stdin().read_to_end(&mut s).unwrap();let mut p=P{c:s,i:0};let v=p.val();println!("{}",sum(&v))}
