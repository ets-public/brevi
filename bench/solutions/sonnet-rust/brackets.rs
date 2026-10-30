use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();for l in s.lines(){let mut st=vec![];let mut ok=true;for c in l.chars(){match c{'('|'['|'{'=>st.push(c),')'|']'|'}'=>{let o=match c{')'=>'(',']'=>'[',_=>'{'};if st.pop()!=Some(o){ok=false;break;}}_=>{}}}if !st.is_empty(){ok=false;}println!("{}",if ok{"yes"}else{"no"});}}
