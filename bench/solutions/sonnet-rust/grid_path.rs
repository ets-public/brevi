use std::io::*;use std::collections::VecDeque;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let g:Vec<Vec<char>>=s.lines().map(|l|l.trim_end().chars().collect()).collect();let mut st=None;let mut en=None;for (i,r) in g.iter().enumerate(){for (j,&c) in r.iter().enumerate(){if c=='S'{st=Some((i,j));}if c=='E'{en=Some((i,j));}}}
let (st,en)=match(st,en){(Some(a),Some(b))=>(a,b),_=>{println!("-1");return;}};
let mut d=vec![vec![-1i64;g.iter().map(|r|r.len()).max().unwrap_or(0)];g.len()];d[st.0][st.1]=0;let mut q=VecDeque::new();q.push_back(st);
while let Some((i,j))=q.pop_front(){if (i,j)==en{break;}let dirs=[(0i32,1i32),(1,0),(0,-1),(-1,0)];for (a,b) in dirs{let ni=i as i32+a;let nj=j as i32+b;if ni<0||nj<0{continue;}let (ni,nj)=(ni as usize,nj as usize);if ni>=g.len()||nj>=g[ni].len(){continue;}if g[ni][nj]=='#'||d[ni][nj]>=0{continue;}d[ni][nj]=d[i][j]+1;q.push_back((ni,nj));}}
println!("{}",d[en.0][en.1]);}
