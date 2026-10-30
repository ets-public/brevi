use std::io::*;
struct P<'a>{s:&'a [u8],i:usize,err:bool}
impl<'a> P<'a>{
fn ws(&mut self){while self.i<self.s.len()&&self.s[self.i]==b' '{self.i+=1}}
fn expr(&mut self)->i64{let mut v=self.term();loop{self.ws();if self.i<self.s.len()&&(self.s[self.i]==b'+'||self.s[self.i]==b'-'){let o=self.s[self.i];self.i+=1;let r=self.term();v=if o==b'+'{v.wrapping_add(r)}else{v.wrapping_sub(r)};}else{return v}}}
fn term(&mut self)->i64{let mut v=self.unary();loop{self.ws();if self.i<self.s.len()&&(self.s[self.i]==b'*'||self.s[self.i]==b'/'){let o=self.s[self.i];self.i+=1;let r=self.unary();if o==b'*'{v=v.wrapping_mul(r)}else if r==0{self.err=true}else{v=v.wrapping_div(r)}}else{return v}}}
fn unary(&mut self)->i64{self.ws();if self.i<self.s.len(){match self.s[self.i]{
b'-'=>{self.i+=1;return self.unary().wrapping_neg()}
b'+'=>{self.i+=1;return self.unary()}
b'('=>{self.i+=1;let v=self.expr();self.ws();self.i+=1;return v}
_=>{}}}
let mut v:i64=0;while self.i<self.s.len()&&self.s[self.i].is_ascii_digit(){v=v.wrapping_mul(10).wrapping_add((self.s[self.i]-b'0') as i64);self.i+=1}v}
}
fn main(){let mut t=String::new();stdin().read_to_string(&mut t).unwrap();
for l in t.lines(){if l.trim().is_empty(){continue}let mut p=P{s:l.as_bytes(),i:0,err:false};let v=p.expr();if p.err{println!("error")}else{println!("{}",v)}}}
