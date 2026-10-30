fn main(){let mut s=String::new();std::io::stdin().read_line(&mut s).unwrap();let n:usize=s.trim().parse().unwrap();
let mut p=vec![true;n+1];let mut i=2;while i*i<=n{if p[i]{let mut j=i*i;while j<=n{p[j]=false;j+=i;}}i+=1;}
println!("{}",(2..=n).filter(|&k|p[k]).count());}
