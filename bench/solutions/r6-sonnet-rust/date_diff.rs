use std::io::*;
fn days(s:&str)->i64{let p:Vec<i64>=s.split('-').map(|x|x.parse().unwrap()).collect();let (mut y,m,d)=(p[0],p[1],p[2]);if m<=2{y-=1}
let era=y.div_euclid(400);let yoe=y-era*400;let mp=(m+9)%12;let doy=(153*mp+2)/5+d-1;let doe=yoe*365+yoe/4-yoe/100+doy;era*146097+doe}
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();
for l in s.lines(){let p:Vec<&str>=l.split_whitespace().collect();if p.len()<2{continue}println!("{}",(days(p[0])-days(p[1])).abs())}}
