use std::io::*;
fn main(){let mut s=String::new();stdin().read_to_string(&mut s).unwrap();let t=[(1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),(100,"C"),(90,"XC"),(50,"L"),(40,"XL"),(10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I")];for l in s.lines(){if let Ok(mut n)=l.trim().parse::<u32>(){let mut r=String::new();for &(v,x) in &t{while n>=v{r+=x;n-=v;}}println!("{}",r);}}}
