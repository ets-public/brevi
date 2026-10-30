use std::io::*;
struct P{c:Vec<u8>,i:usize}
impl P{
fn ws(&mut self){while self.i<self.c.len()&&self.c[self.i].is_ascii_whitespace(){self.i+=1}}
fn st(&mut self)->String{self.i+=1;let mut o=vec![];while self.c[self.i]!=b'"'{if self.c[self.i]==b'\\'{self.i+=1;let e=self.c[self.i];match e{b'n'=>o.push(b'\n'),b't'=>o.push(b'\t'),b'u'=>{self.i+=4;o.push(b'?')}_=>o.push(e)}}else{o.push(self.c[self.i])}self.i+=1}self.i+=1;String::from_utf8_lossy(&o).to_string()}
// returns (sum, is_red_string)
fn val(&mut self)->(i64,bool){self.ws();match self.c[self.i]{
b'{'=>{self.i+=1;let mut s=0;let mut red=false;loop{self.ws();if self.c[self.i]==b'}'{self.i+=1;break}if self.c[self.i]==b','{self.i+=1;continue}self.st();self.ws();self.i+=1;let (v,r)=self.val();s+=v;if r{red=true}}(if red{0}else{s},false)}
b'['=>{self.i+=1;let mut s=0;loop{self.ws();if self.c[self.i]==b']'{self.i+=1;break}if self.c[self.i]==b','{self.i+=1;continue}s+=self.val().0}(s,false)}
b'"'=>{let t=self.st();(0,t=="red")}
b't'=>{self.i+=4;(0,false)}b'f'=>{self.i+=5;(0,false)}b'n'=>{self.i+=4;(0,false)}
_=>{let st=self.i;if self.c[self.i]==b'-'{self.i+=1}while self.i<self.c.len()&&(self.c[self.i].is_ascii_digit()||matches!(self.c[self.i],b'.'|b'e'|b'E'|b'+'|b'-')){self.i+=1}let t=std::str::from_utf8(&self.c[st..self.i]).unwrap();(t.parse::<i64>().unwrap_or_else(|_|t.parse::<f64>().unwrap_or(0.0) as i64),false)}}}
}
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let mut p=P{c:s.into_bytes(),i:0};println!("{}",p.val().0)}
