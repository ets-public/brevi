// Runtime prelude pasted at the top of every generated Rust file.
// Axon methods map to ax_-prefixed trait methods so Axon controls their names and int types.
#![allow(warnings)]
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AxErr(pub String);
impl<E: std::fmt::Display> From<E> for AxErr {
    fn from(e: E) -> AxErr {
        AxErr(e.to_string())
    }
}
pub type R<T> = Result<T, AxErr>;

// Current Axon line, so runtime errors can point at the source.
pub static AX_LINE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[inline(always)]
pub fn ax_l(n: usize) {
    AX_LINE.store(n, std::sync::atomic::Ordering::Relaxed);
}
pub fn ax_init() {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => info.payload().downcast_ref::<String>().cloned().unwrap_or_else(|| "panic".into()),
        };
        let line = AX_LINE.load(std::sync::atomic::Ordering::Relaxed);
        eprintln!("{{\"e\":\"runtime\",\"line\":{line},\"msg\":{msg:?}}}");
    }));
}

pub trait Show {
    fn show(&self) -> String;
}
impl Show for i64 {
    fn show(&self) -> String {
        self.to_string()
    }
}
impl Show for f64 {
    fn show(&self) -> String {
        self.to_string()
    }
}
impl Show for bool {
    fn show(&self) -> String {
        self.to_string()
    }
}
impl Show for String {
    fn show(&self) -> String {
        self.clone()
    }
}
impl Show for () {
    fn show(&self) -> String {
        "()".into()
    }
}
fn show_item<T: Show + 'static>(x: &T) -> String {
    // Strings inside collections get quotes so [a, b] and ["a, b"] differ.
    match (x as &dyn std::any::Any).downcast_ref::<String>() {
        Some(s) => format!("{s:?}"),
        None => x.show(),
    }
}
impl<T: Show + 'static> Show for Vec<T> {
    fn show(&self) -> String {
        format!("[{}]", self.iter().map(show_item).collect::<Vec<_>>().join(", "))
    }
}
impl<K: Show + 'static, V: Show + 'static> Show for BTreeMap<K, V> {
    fn show(&self) -> String {
        let parts: Vec<String> = self.iter().map(|(k, v)| format!("{}: {}", show_item(k), show_item(v))).collect();
        format!("{{{}}}", parts.join(", "))
    }
}
impl<A: Show + 'static, B: Show + 'static> Show for (A, B) {
    fn show(&self) -> String {
        format!("({}, {})", show_item(&self.0), show_item(&self.1))
    }
}
impl<A: Show + 'static, B: Show + 'static, C: Show + 'static> Show for (A, B, C) {
    fn show(&self) -> String {
        format!("({}, {}, {})", show_item(&self.0), show_item(&self.1), show_item(&self.2))
    }
}

pub fn print<T: Show>(x: T) {
    println!("{}", x.show());
}
pub fn read(path: String) -> R<String> {
    std::fs::read_to_string(&path).map_err(|e| AxErr(format!("read {path}: {e}")))
}
pub fn write(path: String, text: String) -> R<()> {
    std::fs::write(&path, text).map_err(|e| AxErr(format!("write {path}: {e}")))
}
pub fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}
pub fn input() -> String {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    s.trim_end_matches(['\n', '\r']).to_string()
}
pub fn stdin() -> String {
    use std::io::Read;
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).ok();
    s
}
pub fn lines() -> Vec<String> {
    stdin().lines().map(String::from).collect()
}
pub fn ax_approx(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * (1.0 + a.abs().max(b.abs()))
}

pub trait Plus {
    fn plus(self, o: Self) -> Self;
}
impl Plus for i64 {
    fn plus(self, o: i64) -> i64 {
        self + o
    }
}
impl Plus for f64 {
    fn plus(self, o: f64) -> f64 {
        self + o
    }
}
impl Plus for String {
    fn plus(self, o: String) -> String {
        self + &o
    }
}
impl<T> Plus for Vec<T> {
    fn plus(mut self, o: Vec<T>) -> Vec<T> {
        self.extend(o);
        self
    }
}

pub trait AxNum {
    fn ax_fmt(&self, decimals: i64) -> String;
    fn ax_abs(&self) -> Self;
    fn ax_float(&self) -> f64;
    fn ax_int(&self) -> i64;
}
impl AxNum for i64 {
    fn ax_fmt(&self, decimals: i64) -> String {
        format!("{:.*}", decimals.max(0) as usize, *self as f64)
    }
    fn ax_abs(&self) -> i64 {
        self.abs()
    }
    fn ax_float(&self) -> f64 {
        *self as f64
    }
    fn ax_int(&self) -> i64 {
        *self
    }
}
impl AxNum for f64 {
    fn ax_fmt(&self, decimals: i64) -> String {
        format!("{:.*}", decimals.max(0) as usize, self)
    }
    fn ax_abs(&self) -> f64 {
        self.abs()
    }
    fn ax_float(&self) -> f64 {
        *self
    }
    fn ax_int(&self) -> i64 {
        *self as i64
    }
}
pub trait AxFloat {
    fn ax_sqrt(&self) -> f64;
    fn ax_round(&self) -> f64;
    fn ax_floor(&self) -> f64;
    fn ax_ceil(&self) -> f64;
}
impl AxFloat for f64 {
    fn ax_sqrt(&self) -> f64 {
        self.sqrt()
    }
    fn ax_round(&self) -> f64 {
        self.round()
    }
    fn ax_floor(&self) -> f64 {
        self.floor()
    }
    fn ax_ceil(&self) -> f64 {
        self.ceil()
    }
}

pub trait AxStr {
    fn ax_len(&self) -> i64;
    fn ax_is_empty(&self) -> bool;
    fn ax_words(&self) -> Vec<String>;
    fn ax_lines(&self) -> Vec<String>;
    fn ax_chars(&self) -> Vec<String>;
    fn ax_split(&self, sep: String) -> Vec<String>;
    fn ax_lower(&self) -> String;
    fn ax_upper(&self) -> String;
    fn ax_trim(&self) -> String;
    fn ax_contains(&self, s: String) -> bool;
    fn ax_starts_with(&self, s: String) -> bool;
    fn ax_ends_with(&self, s: String) -> bool;
    fn ax_replace(&self, a: String, b: String) -> String;
    fn ax_repeat(&self, n: i64) -> String;
    fn ax_int(&self) -> R<i64>;
    fn ax_float(&self) -> R<f64>;
    fn ax_ints(&self) -> R<Vec<i64>>;
}
impl AxStr for String {
    fn ax_len(&self) -> i64 {
        self.chars().count() as i64
    }
    fn ax_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn ax_words(&self) -> Vec<String> {
        self.split_whitespace().map(String::from).collect()
    }
    fn ax_lines(&self) -> Vec<String> {
        self.lines().map(String::from).collect()
    }
    fn ax_chars(&self) -> Vec<String> {
        self.chars().map(String::from).collect()
    }
    fn ax_split(&self, sep: String) -> Vec<String> {
        self.split(sep.as_str()).map(String::from).collect()
    }
    fn ax_lower(&self) -> String {
        self.to_lowercase()
    }
    fn ax_upper(&self) -> String {
        self.to_uppercase()
    }
    fn ax_trim(&self) -> String {
        self.trim().to_string()
    }
    fn ax_contains(&self, s: String) -> bool {
        self.contains(s.as_str())
    }
    fn ax_starts_with(&self, s: String) -> bool {
        self.starts_with(s.as_str())
    }
    fn ax_ends_with(&self, s: String) -> bool {
        self.ends_with(s.as_str())
    }
    fn ax_replace(&self, a: String, b: String) -> String {
        self.replace(a.as_str(), b.as_str())
    }
    fn ax_repeat(&self, n: i64) -> String {
        self.repeat(n.max(0) as usize)
    }
    fn ax_int(&self) -> R<i64> {
        self.trim().parse::<i64>().map_err(|_| AxErr(format!("not an int: {self:?}")))
    }
    fn ax_float(&self) -> R<f64> {
        self.trim().parse::<f64>().map_err(|_| AxErr(format!("not a float: {self:?}")))
    }
    fn ax_ints(&self) -> R<Vec<i64>> {
        self.split_whitespace().map(|w| w.parse::<i64>().map_err(|_| AxErr(format!("not an int: {w:?}")))).collect()
    }
}

fn ax_idx(i: i64, len: usize) -> usize {
    // Negative indexes count from the end: xs[-1] is the last item.
    let j = if i < 0 { len as i64 + i } else { i };
    if j < 0 || j as usize >= len {
        panic!("index {i} out of range for length {len}");
    }
    j as usize
}

pub trait AxList<T: Clone> {
    fn ax_len(&self) -> i64;
    fn ax_is_empty(&self) -> bool;
    fn ax_push(&mut self, x: T);
    fn ax_pop(&mut self) -> T;
    fn ax_insert(&mut self, i: i64, x: T);
    fn ax_remove(&mut self, i: i64) -> T;
    fn ax_first(&self) -> T;
    fn ax_last(&self) -> T;
    fn ax_map<U>(&self, f: impl Fn(T) -> U) -> Vec<U>;
    fn ax_filter(&self, f: impl Fn(T) -> bool) -> Vec<T>;
    fn ax_any(&self, f: impl Fn(T) -> bool) -> bool;
    fn ax_all(&self, f: impl Fn(T) -> bool) -> bool;
    fn ax_count(&self, f: impl Fn(T) -> bool) -> i64;
    fn ax_fold<A>(&self, init: A, f: impl Fn(A, T) -> A) -> A;
    fn ax_sort_by<K: PartialOrd>(&self, f: impl Fn(T) -> K) -> Vec<T>;
    fn ax_rev(&self) -> Vec<T>;
    fn ax_take(&self, n: i64) -> Vec<T>;
    fn ax_skip(&self, n: i64) -> Vec<T>;
    fn ax_enumerate(&self) -> Vec<(i64, T)>;
    fn ax_zip<U: Clone>(&self, o: Vec<U>) -> Vec<(T, U)>;
}
impl<T: Clone> AxList<T> for Vec<T> {
    fn ax_len(&self) -> i64 {
        self.len() as i64
    }
    fn ax_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn ax_push(&mut self, x: T) {
        self.push(x)
    }
    fn ax_pop(&mut self) -> T {
        self.pop().expect("pop from empty list")
    }
    fn ax_insert(&mut self, i: i64, x: T) {
        let j = if i == self.len() as i64 { self.len() } else { ax_idx(i, self.len()) };
        self.insert(j, x)
    }
    fn ax_remove(&mut self, i: i64) -> T {
        let j = ax_idx(i, self.len());
        self.remove(j)
    }
    fn ax_first(&self) -> T {
        self.first().expect("first of empty list").clone()
    }
    fn ax_last(&self) -> T {
        self.last().expect("last of empty list").clone()
    }
    fn ax_map<U>(&self, f: impl Fn(T) -> U) -> Vec<U> {
        self.iter().cloned().map(f).collect()
    }
    fn ax_filter(&self, f: impl Fn(T) -> bool) -> Vec<T> {
        self.iter().cloned().filter(|x| f(x.clone())).collect()
    }
    fn ax_any(&self, f: impl Fn(T) -> bool) -> bool {
        self.iter().cloned().any(f)
    }
    fn ax_all(&self, f: impl Fn(T) -> bool) -> bool {
        self.iter().cloned().all(f)
    }
    fn ax_count(&self, f: impl Fn(T) -> bool) -> i64 {
        self.iter().cloned().filter(|x| f(x.clone())).count() as i64
    }
    fn ax_fold<A>(&self, init: A, f: impl Fn(A, T) -> A) -> A {
        self.iter().cloned().fold(init, f)
    }
    fn ax_sort_by<K: PartialOrd>(&self, f: impl Fn(T) -> K) -> Vec<T> {
        let mut v = self.clone();
        v.sort_by(|a, b| f(a.clone()).partial_cmp(&f(b.clone())).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
    fn ax_rev(&self) -> Vec<T> {
        self.iter().rev().cloned().collect()
    }
    fn ax_take(&self, n: i64) -> Vec<T> {
        self.iter().take(n.max(0) as usize).cloned().collect()
    }
    fn ax_skip(&self, n: i64) -> Vec<T> {
        self.iter().skip(n.max(0) as usize).cloned().collect()
    }
    fn ax_enumerate(&self) -> Vec<(i64, T)> {
        self.iter().cloned().enumerate().map(|(i, x)| (i as i64, x)).collect()
    }
    fn ax_zip<U: Clone>(&self, o: Vec<U>) -> Vec<(T, U)> {
        self.iter().cloned().zip(o).collect()
    }
}
pub trait AxOrdList<T> {
    fn ax_sort(&self) -> Vec<T>;
    fn ax_min(&self) -> T;
    fn ax_max(&self) -> T;
    fn ax_contains(&self, x: T) -> bool;
    fn ax_index_of(&self, x: T) -> i64;
}
impl<T: Clone + PartialOrd> AxOrdList<T> for Vec<T> {
    fn ax_sort(&self) -> Vec<T> {
        let mut v = self.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
    fn ax_min(&self) -> T {
        self.ax_sort().first().expect("min of empty list").clone()
    }
    fn ax_max(&self) -> T {
        self.ax_sort().last().expect("max of empty list").clone()
    }
    fn ax_contains(&self, x: T) -> bool {
        self.iter().any(|y| *y == x)
    }
    fn ax_index_of(&self, x: T) -> i64 {
        self.iter().position(|y| *y == x).map_or(-1, |i| i as i64)
    }
}
pub trait AxSum<T> {
    fn ax_sum(&self) -> T;
}
impl AxSum<i64> for Vec<i64> {
    fn ax_sum(&self) -> i64 {
        self.iter().sum()
    }
}
impl AxSum<f64> for Vec<f64> {
    fn ax_sum(&self) -> f64 {
        self.iter().sum()
    }
}
pub trait AxJoin {
    fn ax_join(&self, sep: String) -> String;
}
impl AxJoin for Vec<String> {
    fn ax_join(&self, sep: String) -> String {
        self.join(&sep)
    }
}

pub trait AxMap<K, V> {
    fn ax_len(&self) -> i64;
    fn ax_is_empty(&self) -> bool;
    fn ax_has(&self, k: K) -> bool;
    fn ax_get_or(&self, k: K, d: V) -> V;
    fn ax_remove(&mut self, k: K);
    fn ax_keys(&self) -> Vec<K>;
    fn ax_values(&self) -> Vec<V>;
    fn ax_pairs(&self) -> Vec<(K, V)>;
}
impl<K: Ord + Clone, V: Clone> AxMap<K, V> for BTreeMap<K, V> {
    fn ax_len(&self) -> i64 {
        self.len() as i64
    }
    fn ax_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn ax_has(&self, k: K) -> bool {
        self.contains_key(&k)
    }
    fn ax_get_or(&self, k: K, d: V) -> V {
        self.get(&k).cloned().unwrap_or(d)
    }
    fn ax_remove(&mut self, k: K) {
        self.remove(&k);
    }
    fn ax_keys(&self) -> Vec<K> {
        self.keys().cloned().collect()
    }
    fn ax_values(&self) -> Vec<V> {
        self.values().cloned().collect()
    }
    fn ax_pairs(&self) -> Vec<(K, V)> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

pub trait AxIndex<K, V> {
    fn ax_at(&self, k: K) -> V;
    fn ax_set(&mut self, k: K, v: V);
}
pub trait AxIndexMut<K, V> {
    fn ax_at_mut(&mut self, k: K) -> &mut V;
}
impl<T: Clone> AxIndex<i64, T> for Vec<T> {
    fn ax_at(&self, i: i64) -> T {
        self[ax_idx(i, self.len())].clone()
    }
    fn ax_set(&mut self, i: i64, v: T) {
        let j = ax_idx(i, self.len());
        self[j] = v;
    }
}
impl<T: Clone> AxIndexMut<i64, T> for Vec<T> {
    fn ax_at_mut(&mut self, i: i64) -> &mut T {
        let j = ax_idx(i, self.len());
        &mut self[j]
    }
}
impl<K: Ord + Clone + Show + 'static, V: Clone> AxIndex<K, V> for BTreeMap<K, V> {
    fn ax_at(&self, k: K) -> V {
        match self.get(&k) {
            Some(v) => v.clone(),
            None => panic!("key {} not in map", show_item(&k)),
        }
    }
    fn ax_set(&mut self, k: K, v: V) {
        self.insert(k, v);
    }
}
impl<K: Ord + Clone, V: Default> AxIndexMut<K, V> for BTreeMap<K, V> {
    fn ax_at_mut(&mut self, k: K) -> &mut V {
        // Updating a missing key starts it at the default (0, "", [] ...).
        self.entry(k).or_default()
    }
}
impl AxIndex<i64, String> for String {
    fn ax_at(&self, i: i64) -> String {
        let cs: Vec<char> = self.chars().collect();
        cs[ax_idx(i, cs.len())].to_string()
    }
    fn ax_set(&mut self, _i: i64, _v: String) {
        panic!("strings cannot be changed by index")
    }
}
// ---- end of prelude ----
