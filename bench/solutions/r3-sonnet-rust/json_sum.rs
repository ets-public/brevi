use std::io::*;
struct P{s:Vec<u8>,i:usize}
impl P{
fn ws(&mut self){while self.i<self.s.len()&&(self.s[self.i] as char).is_whitespace(){self.i+=1}}
fn string(&mut self)->String{self.i+=1;let mut r=vec![];while self.s[self.i]!=b'"'{if self.s[self.i]==b'\\'{self.i+=1;r.push(self.s[self.i]);}else{r.push(self.s[self.i])}self.i+=1}self.i+=1;String::from_utf8_lossy(&r).into_owned()}
// returns (sum, is_red_string)
fn val(&mut self)->(i64,bool){self.ws();match self.s[self.i]{
b'{'=>{self.i+=1;let mut sum=0;let mut red=false;loop{self.ws();if self.s[self.i]==b'}'{self.i+=1;break}if self.s[self.i]==b','{self.i+=1;continue}
self.string();self.ws();self.i+=1;let (s,r)=self.val();sum+=s;if r{red=true}}
(if red{0}else{sum},false)}
b'['=>{self.i+=1;let mut sum=0;loop{self.ws();if self.s[self.i]==b']'{self.i+=1;break}if self.s[self.i]==b','{self.i+=1;continue}let (s,_)=self.val();sum+=s}(sum,false)}
b'"'=>{let s=self.string();(0,s=="red")}
b't'=>{self.i+=4;(0,false)}
b'f'=>{self.i+=5;(0,false)}
b'n'=>{self.i+=4;(0,false)}
_=>{let st=self.i;self.i+=1;while self.i<self.s.len()&&(self.s[self.i].is_ascii_digit()||self.s[self.i]==b'.'||self.s[self.i]==b'e'||self.s[self.i]==b'E'||self.s[self.i]==b'+'||self.s[self.i]==b'-'){self.i+=1}
let x=std::str::from_utf8(&self.s[st..self.i]).unwrap();(x.parse::<i64>().unwrap_or_else(|_|x.parse::<f64>().unwrap_or(0.0) as i64),false)}}}
}
fn main(){let mut t=vec![];stdin().read_to_end(&mut t).unwrap();let mut p=P{s:t,i:0};println!("{}",p.val().0)}
