use std::collections::HashMap;
use std::io::{self, Read, Write};

// ---------- minimal JSON parser ----------

#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn lit(&mut self, word: &str) -> Option<()> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Some(())
        } else {
            None
        }
    }

    fn value(&mut self) -> Option<J> {
        self.ws();
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(J::Str),
            b't' => self.lit("true").map(|_| J::Bool(true)),
            b'f' => self.lit("false").map(|_| J::Bool(false)),
            b'n' => self.lit("null").map(|_| J::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn digits(&mut self) -> Option<usize> {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i > start {
            Some(self.i - start)
        } else {
            None
        }
    }

    fn number(&mut self) -> Option<J> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        self.digits()?;
        if self.peek() == Some(b'.') {
            self.i += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            self.digits()?;
        }
        Some(J::Num(String::from_utf8_lossy(&self.s[start..self.i]).into_owned()))
    }

    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    match e {
                        b'"' => buf.push(b'"'),
                        b'\\' => buf.push(b'\\'),
                        b'/' => buf.push(b'/'),
                        b'b' => buf.push(8),
                        b'f' => buf.push(12),
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return None;
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4]).ok()?;
                            let code = u32::from_str_radix(hex, 16).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                            let mut tmp = [0u8; 4];
                            buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).ok()
    }

    fn array(&mut self) -> Option<J> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Some(J::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Some(J::Arr(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self) -> Option<J> {
        self.i += 1;
        let mut fields = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Some(J::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.string()?;
            self.ws();
            if self.peek()? != b':' {
                return None;
            }
            self.i += 1;
            let val = self.value()?;
            fields.push((key, val));
            self.ws();
            match self.peek()? {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Some(J::Obj(fields));
                }
                _ => return None,
            }
        }
    }
}

fn parse_json(s: &str) -> Option<J> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i == p.s.len() {
        Some(v)
    } else {
        None
    }
}

fn get<'a>(obj: &'a J, key: &str) -> Option<&'a J> {
    match obj {
        J::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn as_string(v: Option<&J>) -> Option<String> {
    match v? {
        J::Str(s) => Some(s.clone()),
        J::Num(s) => Some(s.clone()),
        _ => None,
    }
}

fn as_int(v: Option<&J>) -> Option<i128> {
    match v? {
        J::Num(s) if !s.contains(['.', 'e', 'E']) => s.parse::<i128>().ok(),
        _ => None,
    }
}

fn esc(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// "12.50" -> 1250 cents.
fn to_cents(s: &str) -> Option<i128> {
    let (ip, fp) = match s.split_once('.') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let whole: i128 = if ip.is_empty() { 0 } else { ip.parse().ok()? };
    let mut frac = String::from(fp);
    while frac.len() < 2 {
        frac.push('0');
    }
    let frac: i128 = frac[..2].parse().ok()?;
    Some(whole * 100 + frac)
}

// Integer division rounding half to even; num and den are non-negative.
fn round_div(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r > den || (2 * r == den && q % 2 == 1) {
        q + 1
    } else {
        q
    }
}

fn money(c: i128) -> String {
    format!("{}.{:02}", c / 100, c % 100)
}

struct Product {
    cents: i128,
    category: String,
}

struct Customer {
    name: String,
    tier: String,
    country: String,
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let root = match parse_json(input.trim()) {
        Some(r) => r,
        None => {
            writeln!(out, "[]").unwrap();
            writeln!(out, "revenue 0.00").unwrap();
            return;
        }
    };

    let mut products: HashMap<String, Product> = HashMap::new();
    if let Some(J::Obj(fields)) = get(&root, "products") {
        for (sku, p) in fields {
            let cents = as_string(get(p, "price")).and_then(|s| to_cents(&s)).unwrap_or(0);
            let category = as_string(get(p, "category")).unwrap_or_default();
            products.insert(sku.clone(), Product { cents, category });
        }
    }

    let mut customers: HashMap<String, Customer> = HashMap::new();
    if let Some(J::Obj(fields)) = get(&root, "customers") {
        for (cid, c) in fields {
            customers.insert(
                cid.clone(),
                Customer {
                    name: as_string(get(c, "name")).unwrap_or_default(),
                    tier: as_string(get(c, "tier")).unwrap_or_default(),
                    country: as_string(get(c, "country")).unwrap_or_default(),
                },
            );
        }
    }

    let orders: Vec<J> = match get(&root, "orders") {
        Some(J::Arr(v)) => v.clone(),
        _ => Vec::new(),
    };

    let mut invoices: Vec<String> = Vec::new();
    let mut revenue: i128 = 0;

    for order in &orders {
        let id = as_string(get(order, "id")).unwrap_or_default();
        let id_json = esc(&id);
        let cust_key = as_string(get(order, "customer")).unwrap_or_default();

        let customer = match customers.get(&cust_key) {
            Some(c) => c,
            None => {
                invoices.push(format!(
                    "{{\"id\":{},\"error\":{}}}",
                    id_json,
                    esc(&format!("unknown customer {}", cust_key))
                ));
                continue;
            }
        };

        let lines: Vec<(String, i128)> = match get(order, "lines") {
            Some(J::Arr(ls)) => ls
                .iter()
                .map(|l| {
                    (
                        as_string(get(l, "sku")).unwrap_or_default(),
                        as_int(get(l, "qty")).unwrap_or(0),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };

        // Unknown SKU check, first one in line order.
        let mut bad_sku = None;
        for (sku, _) in &lines {
            if !products.contains_key(sku) {
                bad_sku = Some(sku.clone());
                break;
            }
        }
        if let Some(sku) = bad_sku {
            invoices.push(format!(
                "{{\"id\":{},\"error\":{}}}",
                id_json,
                esc(&format!("unknown sku {}", sku))
            ));
            continue;
        }

        // Units per category.
        let mut units: HashMap<&str, i128> = HashMap::new();
        for (sku, qty) in &lines {
            let p = &products[sku];
            *units.entry(p.category.as_str()).or_insert(0) += qty;
        }

        let mut subtotal: i128 = 0;
        let mut cat_discount: i128 = 0;
        for (sku, qty) in &lines {
            let p = &products[sku];
            let amount = p.cents * qty;
            subtotal += amount;
            if units[p.category.as_str()] >= 3 {
                cat_discount += round_div(amount * 5, 100);
            }
        }

        let after_cat = subtotal - cat_discount;
        let tier_pct: i128 = match customer.tier.as_str() {
            "gold" => 10,
            "silver" => 5,
            _ => 0,
        };
        let tier_discount = round_div(after_cat * tier_pct, 100);
        let discount = cat_discount + tier_discount;
        let discounted = subtotal - discount;

        let (rate_num, rate_den): (i128, i128) = match customer.country.as_str() {
            "UK" => (20, 100),
            "US" => (825, 10000),
            _ => (0, 1),
        };
        let tax = round_div(discounted * rate_num, rate_den);
        let total = discounted + tax;
        revenue += total;

        invoices.push(format!(
            "{{\"id\":{},\"customer\":{},\"subtotal\":\"{}\",\"discount\":\"{}\",\"tax\":\"{}\",\"total\":\"{}\"}}",
            id_json,
            esc(&customer.name),
            money(subtotal),
            money(discount),
            money(tax),
            money(total)
        ));
    }

    writeln!(out, "[{}]", invoices.join(",")).unwrap();
    writeln!(out, "revenue {}", money(revenue)).unwrap();
    out.flush().unwrap();
}
