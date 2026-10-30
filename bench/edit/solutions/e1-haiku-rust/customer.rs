use std::collections::BTreeMap;
use std::io::{self, BufRead};

struct Product {
    sku: String,
    name: String,
    price: i64, // cents
    stock: i64,
}

struct Order {
    id: String,
    customer: String,
    lines: Vec<(String, i64)>,
    total: i64,
    cancelled: bool,
}

struct Shop {
    products: BTreeMap<String, Product>,
    orders: BTreeMap<String, Order>,
}

fn parse_cents(s: &str) -> i64 {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    let frac: String = format!("{}00", frac).chars().take(2).collect();
    whole.parse::<i64>().unwrap() * 100 + frac.parse::<i64>().unwrap()
}

fn money(c: i64) -> String {
    format!("{}.{:02}", c / 100, c % 100)
}

impl Shop {
    fn add(&mut self, args: &[&str]) {
        let sku = args[0].to_string();
        if self.products.contains_key(&sku) {
            println!("error: {} exists", sku);
            return;
        }
        let p = Product { sku: sku.clone(), name: args[1].to_string(), price: parse_cents(args[2]), stock: args[3].parse().unwrap() };
        self.products.insert(sku, p);
    }

    fn restock(&mut self, args: &[&str]) {
        match self.products.get_mut(args[0]) {
            Some(p) => p.stock += args[1].parse::<i64>().unwrap(),
            None => println!("error: unknown product {}", args[0]),
        }
    }

    fn order(&mut self, args: &[&str]) {
        let oid = args[0].to_string();
        if self.orders.contains_key(&oid) {
            println!("error: order {} exists", oid);
            return;
        }
        let customer = args[1].to_string();
        let lines: Vec<(String, i64)> = args[2..].chunks(2).map(|c| (c[0].to_string(), c[1].parse().unwrap())).collect();
        for (sku, qty) in &lines {
            match self.products.get(sku) {
                None => {
                    println!("error: unknown product {}", sku);
                    return;
                }
                Some(p) if p.stock < *qty => {
                    println!("error: order {}: not enough {}", oid, sku);
                    return;
                }
                _ => {}
            }
        }
        let mut total = 0;
        for (sku, qty) in &lines {
            let p = self.products.get_mut(sku).unwrap();
            p.stock -= qty;
            total += p.price * qty;
        }
        println!("order {}: {}", oid, money(total));
        self.orders.insert(oid.clone(), Order { id: oid, customer, lines, total, cancelled: false });
    }

    fn cancel(&mut self, args: &[&str]) {
        let Some(o) = self.orders.get_mut(args[0]) else {
            println!("error: unknown order {}", args[0]);
            return;
        };
        for (sku, qty) in &o.lines {
            self.products.get_mut(sku).unwrap().stock += qty;
        }
        o.cancelled = true;
        println!("cancelled {}", o.id);
    }

    fn report(&self) {
        let mut value = 0;
        for p in self.products.values() {
            println!("{} {} {}", p.sku, p.name, p.stock);
            value += p.price * p.stock;
        }
        println!("stock value {}", money(value));
    }

    fn customer(&self, args: &[&str]) {
        let name = args[0];
        let mine: Vec<&Order> = self.orders.values().filter(|o| !o.cancelled && o.customer == name).collect();
        let total: i64 = mine.iter().map(|o| o.total).sum();
        println!("{} {} {}", name, mine.len(), money(total));
    }

    fn revenue(&self) {
        let total: i64 = self.orders.values().filter(|o| !o.cancelled).map(|o| o.total).sum();
        println!("revenue {}", money(total));
    }
}

fn main() {
    let mut shop = Shop { products: BTreeMap::new(), orders: BTreeMap::new() };
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let args = &words[1..];
        match words[0] {
            "add" => shop.add(args),
            "restock" => shop.restock(args),
            "order" => shop.order(args),
            "cancel" => shop.cancel(args),
            "report" => shop.report(),
            "revenue" => shop.revenue(),
            "customer" => shop.customer(args),
            other => println!("error: unknown command {}", other),
        }
    }
}
