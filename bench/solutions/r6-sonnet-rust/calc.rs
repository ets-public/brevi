use std::io::*;
struct P{c:Vec<u8>,i:usize}
impl P{
fn ws(&mut self){while self.i<self.c.len()&&self.c[self.i].is_ascii_whitespace(){self.i+=1}}
fn expr(&mut self)->Option<i128>{let mut v=self.term()?;loop{self.ws();if self.i<self.c.len()&&(self.c[self.i]==b'+'||self.c[self.i]==b'-'){let o=self.c[self.i];self.i+=1;let r=self.term()?;v=if o==b'+'{v+r}else{v-r}}else{return Some(v)}}}
fn term(&mut self)->Option<i128>{let mut v=self.un()?;loop{self.ws();if self.i<self.c.len()&&(self.c[self.i]==b'*'||self.c[self.i]==b'/'){let o=self.c[self.i];self.i+=1;let r=self.un()?;if o==b'*'{v*=r}else{if r==0{return None}v/=r}}else{return Some(v)}}}
fn un(&mut self)->Option<i128>{self.ws();if self.i<self.c.len(){match self.c[self.i]{b'-'=>{self.i+=1;return self.un().map(|x|-x)}b'+'=>{self.i+=1;return self.un()}b'('=>{self.i+=1;let v=self.expr();self.ws();self.i+=1;return v}_=>{}}}
let mut v:i128=0;while self.i<self.c.len()&&self.c[self.i].is_ascii_digit(){v=v*10+(self.c[self.i]-b'0') as i128;self.i+=1}Some(v)}
}
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();for l in s.lines(){if l.trim().is_empty(){continue}let mut p=P{c:l.bytes().collect(),i:0};match p.expr(){Some(v)=>println!("{}",v),None=>println!("error")}}}
