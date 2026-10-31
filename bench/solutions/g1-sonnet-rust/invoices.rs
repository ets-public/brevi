use std::io::{stdin, Read};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum J { Null, Bool(bool), Num(String), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }

struct P { b: Vec<char>, i: usize }
impl P {
    fn ws(&mut self) { while self.i < self.b.len() && self.b[self.i].is_whitespace() { self.i += 1; } }
    fn val(&mut self) -> J {
        self.ws();
        let c = self.b[self.i];
        match c {
            '{' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b[self.i] == '}' { self.i += 1; return J::Obj(v); }
                loop {
                    self.ws();
                    let k = match self.val() { J::Str(s) => s, _ => panic!() };
                    self.ws(); self.i += 1;
                    let x = self.val();
                    v.push((k, x));
                    self.ws();
                    let d = self.b[self.i]; self.i += 1;
                    if d == '}' { return J::Obj(v); }
                }
            }
            '[' => {
                self.i += 1; let mut v = vec![];
                self.ws();
                if self.b[self.i] == ']' { self.i += 1; return J::Arr(v); }
                loop {
                    v.push(self.val());
                    self.ws();
                    let d = self.b[self.i]; self.i += 1;
                    if d == ']' { return J::Arr(v); }
                }
            }
            '"' => {
                self.i += 1; let mut s = String::new();
                loop {
                    let c = self.b[self.i]; self.i += 1;
                    match c {
                        '"' => break,
                        '\\' => {
                            let e = self.b[self.i]; self.i += 1;
                            match e {
                                'n' => s.push('\n'), 't' => s.push('\t'), 'r' => s.push('\r'),
                                'b' => s.push('\u{8}'), 'f' => s.push('\u{c}'),
                                'u' => {
                                    let h: String = self.b[self.i..self.i + 4].iter().collect();
                                    self.i += 4;
                                    s.push(char::from_u32(u32::from_str_radix(&h, 16).unwrap()).unwrap_or('?'));
                                }
                                o => s.push(o),
                            }
                        }
                        c => s.push(c),
                    }
                }
                J::Str(s)
            }
            't' => { self.i += 4; J::Bool(true) }
            'f' => { self.i += 5; J::Bool(false) }
            'n' => { self.i += 4; J::Null }
            _ => {
                let st = self.i;
                while self.i < self.b.len() && "+-.eE0123456789".contains(self.b[self.i]) { self.i += 1; }
                J::Num(self.b[st..self.i].iter().collect())
            }
        }
    }
}

fn get<'a>(j: &'a J, k: &str) -> Option<&'a J> {
    if let J::Obj(v) = j { v.iter().rev().find(|(a, _)| a == k).map(|(_, x)| x) } else { None }
}
fn text(j: &J) -> String {
    match j { J::Str(s) => s.clone(), J::Num(n) => n.clone(), J::Bool(b) => b.to_string(), _ => "null".into() }
}
fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"", '\\' => o += "\\\\", '\n' => o += "\\n", '\r' => o += "\\r", '\t' => o += "\\t",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o.push('"'); o
}
fn jval(j: &J) -> String {
    match j { J::Str(s) => jstr(s), other => text(other) }
}

fn cents(s: &str) -> i128 {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') { Some(r) => (true, r), None => (false, s) };
    let (a, b) = s.split_once('.').unwrap_or((s, ""));
    let mut f: String = b.chars().take(2).collect();
    while f.len() < 2 { f.push('0'); }
    let v = a.parse::<i128>().unwrap_or(0) * 100 + f.parse::<i128>().unwrap_or(0);
    if neg { -v } else { v }
}
// round n/d half to even, n>=0
fn rdiv(n: i128, d: i128) -> i128 {
    let q = n / d; let r = n % d;
    if r * 2 > d || (r * 2 == d && q % 2 == 1) { q + 1 } else { q }
}
fn money(c: i128) -> String { format!("{}{}.{:02}", if c < 0 { "-" } else { "" }, c.abs() / 100, c.abs() % 100) }

fn main() {
    let mut s = String::new();
    stdin().read_to_string(&mut s).unwrap();
    let mut p = P { b: s.chars().collect(), i: 0 };
    let root = p.val();
    let empty = J::Obj(vec![]);
    let products = get(&root, "products").unwrap_or(&empty);
    let customers = get(&root, "customers").unwrap_or(&empty);
    let orders = match get(&root, "orders") { Some(J::Arr(a)) => a.clone(), _ => vec![] };
    let mut results = vec![];
    let mut revenue: i128 = 0;
    for o in &orders {
        let idj = get(o, "id").cloned().unwrap_or(J::Null);
        let id = jval(&idj);
        let cid = get(o, "customer").map(text).unwrap_or_default();
        let cust = get(customers, &cid);
        let cust = match cust {
            Some(c) => c,
            None => { results.push(format!("{{\"id\":{},\"error\":{}}}", id, jstr(&format!("unknown customer {}", cid)))); continue; }
        };
        let lines = match get(o, "lines") { Some(J::Arr(a)) => a.clone(), _ => vec![] };
        let mut subtotal = 0i128;
        let mut cat_units: BTreeMap<String, i128> = BTreeMap::new();
        let mut cat_amt: BTreeMap<String, i128> = BTreeMap::new();
        let mut bad: Option<String> = None;
        for l in &lines {
            let sku = get(l, "sku").map(text).unwrap_or_default();
            let qty: i128 = get(l, "qty").map(text).and_then(|t| t.parse().ok()).unwrap_or(0);
            match get(products, &sku) {
                None => { bad = Some(sku); break; }
                Some(pr) => {
                    let price = cents(&get(pr, "price").map(text).unwrap_or_default());
                    let cat = get(pr, "category").map(text).unwrap_or_default();
                    let amt = price * qty;
                    subtotal += amt;
                    *cat_units.entry(cat.clone()).or_insert(0) += qty;
                    *cat_amt.entry(cat).or_insert(0) += amt;
                }
            }
        }
        if let Some(sku) = bad {
            results.push(format!("{{\"id\":{},\"error\":{}}}", id, jstr(&format!("unknown sku {}", sku))));
            continue;
        }
        let mut disc = 0i128;
        for (c, u) in &cat_units {
            if *u >= 3 { disc += rdiv(cat_amt[c] * 5, 100); }
        }
        let tier = get(cust, "tier").map(text).unwrap_or_default();
        let pct = match tier.as_str() { "gold" => 10, "silver" => 5, _ => 0 };
        disc += rdiv((subtotal - disc) * pct, 100);
        let country = get(cust, "country").map(text).unwrap_or_default();
        let net = subtotal - disc;
        let tax = match country.as_str() { "UK" => rdiv(net * 20, 100), "US" => rdiv(net * 825, 10000), _ => 0 };
        let total = net + tax;
        revenue += total;
        results.push(format!("{{\"id\":{},\"customer\":{},\"subtotal\":\"{}\",\"discount\":\"{}\",\"tax\":\"{}\",\"total\":\"{}\"}}",
            id, jstr(&get(cust, "name").map(text).unwrap_or_default()), money(subtotal), money(disc), money(tax), money(total)));
    }
    println!("[{}]", results.join(","));
    println!("revenue {}", money(revenue));
}
