use std::io::*;
struct P{c:Vec<u8>,i:usize}
impl P{
fn ws(&mut self){while self.i<self.c.len()&&self.c[self.i]==b' '{self.i+=1}}
fn expr(&mut self)->Option<i64>{let mut v=self.term()?;loop{self.ws();if self.i<self.c.len()&&(self.c[self.i]==b'+'||self.c[self.i]==b'-'){let o=self.c[self.i];self.i+=1;let r=self.term()?;v=if o==b'+'{v.wrapping_add(r)}else{v.wrapping_sub(r)};}else{return Some(v)}}}
fn term(&mut self)->Option<i64>{let mut v=self.un()?;loop{self.ws();if self.i<self.c.len()&&(self.c[self.i]==b'*'||self.c[self.i]==b'/'){let o=self.c[self.i];self.i+=1;let r=self.un()?;if o==b'*'{v=v.wrapping_mul(r)}else{if r==0{return None}v=v.wrapping_div(r)}}else{return Some(v)}}}
fn un(&mut self)->Option<i64>{self.ws();if self.i<self.c.len(){let ch=self.c[self.i];if ch==b'-'{self.i+=1;return self.un().map(|x|x.wrapping_neg())}if ch==b'+'{self.i+=1;return self.un()}if ch==b'('{self.i+=1;let v=self.expr()?;self.ws();self.i+=1;return Some(v)}}
let mut v:i64=0;while self.i<self.c.len()&&self.c[self.i].is_ascii_digit(){v=v.wrapping_mul(10).wrapping_add((self.c[self.i]-b'0') as i64);self.i+=1}Some(v)}
}
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();for l in s.lines(){if l.trim().is_empty(){continue}let mut p=P{c:l.bytes().collect(),i:0};match p.expr(){Some(v)=>println!("{}",v),None=>println!("error")}}}
