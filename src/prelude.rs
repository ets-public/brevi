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
    fn ax_slice(&self, a: i64, b: i64) -> String;
    fn ax_find(&self, s: String) -> i64;
    fn ax_is_digit(&self) -> bool;
    fn ax_is_alpha(&self) -> bool;
    fn ax_is_space(&self) -> bool;
}
fn ax_range(a: i64, b: i64, len: usize) -> (usize, usize) {
    // Python-style slice bounds: negatives count from the end, out-of-range is clamped.
    let fix = |i: i64| if i < 0 { (len as i64 + i).max(0) as usize } else { (i as usize).min(len) };
    let (x, y) = (fix(a), fix(b));
    (x, y.max(x))
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
    fn ax_slice(&self, a: i64, b: i64) -> String {
        let cs: Vec<char> = self.chars().collect();
        let (x, y) = ax_range(a, b, cs.len());
        cs[x..y].iter().collect()
    }
    fn ax_find(&self, s: String) -> i64 {
        match self.find(s.as_str()) {
            Some(i) => self[..i].chars().count() as i64,
            None => -1,
        }
    }
    fn ax_is_digit(&self) -> bool {
        !self.is_empty() && self.chars().all(|c| c.is_ascii_digit())
    }
    fn ax_is_alpha(&self) -> bool {
        !self.is_empty() && self.chars().all(|c| c.is_alphabetic())
    }
    fn ax_is_space(&self) -> bool {
        !self.is_empty() && self.chars().all(|c| c.is_whitespace())
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
    fn ax_slice(&self, a: i64, b: i64) -> Vec<T>;
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
    fn ax_slice(&self, a: i64, b: i64) -> Vec<T> {
        let (x, y) = ax_range(a, b, self.len());
        self[x..y].to_vec()
    }
}
pub trait AxTryList<T: Clone> {
    fn ax_try_map<U>(&self, f: impl Fn(T) -> R<U>) -> R<Vec<U>>;
    fn ax_try_filter(&self, f: impl Fn(T) -> R<bool>) -> R<Vec<T>>;
    fn ax_try_any(&self, f: impl Fn(T) -> R<bool>) -> R<bool>;
    fn ax_try_all(&self, f: impl Fn(T) -> R<bool>) -> R<bool>;
    fn ax_try_count(&self, f: impl Fn(T) -> R<bool>) -> R<i64>;
    fn ax_try_fold<A>(&self, init: A, f: impl Fn(A, T) -> R<A>) -> R<A>;
    fn ax_try_sort_by<K: PartialOrd>(&self, f: impl Fn(T) -> R<K>) -> R<Vec<T>>;
}
impl<T: Clone> AxTryList<T> for Vec<T> {
    fn ax_try_map<U>(&self, f: impl Fn(T) -> R<U>) -> R<Vec<U>> {
        self.iter().cloned().map(f).collect()
    }
    fn ax_try_filter(&self, f: impl Fn(T) -> R<bool>) -> R<Vec<T>> {
        let mut out = Vec::new();
        for x in self.iter().cloned() {
            if f(x.clone())? {
                out.push(x);
            }
        }
        Ok(out)
    }
    fn ax_try_any(&self, f: impl Fn(T) -> R<bool>) -> R<bool> {
        for x in self.iter().cloned() {
            if f(x)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn ax_try_all(&self, f: impl Fn(T) -> R<bool>) -> R<bool> {
        for x in self.iter().cloned() {
            if !f(x)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn ax_try_count(&self, f: impl Fn(T) -> R<bool>) -> R<i64> {
        Ok(self.ax_try_filter(f)?.len() as i64)
    }
    fn ax_try_fold<A>(&self, init: A, f: impl Fn(A, T) -> R<A>) -> R<A> {
        let mut acc = init;
        for x in self.iter().cloned() {
            acc = f(acc, x)?;
        }
        Ok(acc)
    }
    fn ax_try_sort_by<K: PartialOrd>(&self, f: impl Fn(T) -> R<K>) -> R<Vec<T>> {
        let keyed: Vec<(K, T)> = self.iter().cloned().map(|x| Ok((f(x.clone())?, x))).collect::<R<_>>()?;
        let mut keyed = keyed;
        keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(keyed.into_iter().map(|(_, x)| x).collect())
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

// ---- JSON ----
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Json {
    Null,
    Bool { v: bool },
    Num { v: f64 },
    Str { v: String },
    Arr { v: Vec<Json> },
    Obj { v: BTreeMap<String, Json> },
}
impl Default for Json {
    fn default() -> Json {
        Json::Null
    }
}
impl Show for Json {
    fn show(&self) -> String {
        match self {
            Json::Null => "null".into(),
            Json::Bool { v } => v.to_string(),
            Json::Num { v } => v.to_string(),
            Json::Str { v } => format!("{v:?}"),
            Json::Arr { v } => format!("[{}]", v.iter().map(|x| x.show()).collect::<Vec<_>>().join(",")),
            Json::Obj { v } => format!("{{{}}}", v.iter().map(|(k, x)| format!("{k:?}:{}", x.show())).collect::<Vec<_>>().join(",")),
        }
    }
}
pub fn json(text: String) -> R<Json> {
    let c: Vec<char> = text.chars().collect();
    let mut i = 0;
    let v = json_value(&c, &mut i)?;
    json_ws(&c, &mut i);
    if i != c.len() {
        return Err(AxErr(format!("json: unexpected text at char {i}")));
    }
    Ok(v)
}
fn json_ws(c: &[char], i: &mut usize) {
    while *i < c.len() && c[*i].is_whitespace() {
        *i += 1;
    }
}
fn json_value(c: &[char], i: &mut usize) -> R<Json> {
    json_ws(c, i);
    let bad = |i: usize| AxErr(format!("json: invalid value at char {i}"));
    match c.get(*i) {
        Some('{') => {
            *i += 1;
            let mut m = BTreeMap::new();
            json_ws(c, i);
            if c.get(*i) == Some(&'}') {
                *i += 1;
                return Ok(Json::Obj { v: m });
            }
            loop {
                let k = match json_value(c, i)? {
                    Json::Str { v } => v,
                    _ => return Err(AxErr(format!("json: object key must be a string at char {i}"))),
                };
                json_ws(c, i);
                if c.get(*i) != Some(&':') {
                    return Err(AxErr(format!("json: expected ':' at char {i}")));
                }
                *i += 1;
                let v = json_value(c, i)?;
                m.insert(k, v);
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some('}') => {
                        *i += 1;
                        return Ok(Json::Obj { v: m });
                    }
                    _ => return Err(AxErr(format!("json: expected ',' or '}}' at char {i}"))),
                }
            }
        }
        Some('[') => {
            *i += 1;
            let mut a = Vec::new();
            json_ws(c, i);
            if c.get(*i) == Some(&']') {
                *i += 1;
                return Ok(Json::Arr { v: a });
            }
            loop {
                a.push(json_value(c, i)?);
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some(']') => {
                        *i += 1;
                        return Ok(Json::Arr { v: a });
                    }
                    _ => return Err(AxErr(format!("json: expected ',' or ']' at char {i}"))),
                }
            }
        }
        Some('"') => {
            *i += 1;
            let mut s = String::new();
            loop {
                match c.get(*i) {
                    None => return Err(AxErr("json: unterminated string".into())),
                    Some('"') => {
                        *i += 1;
                        return Ok(Json::Str { v: s });
                    }
                    Some('\\') => {
                        *i += 1;
                        match c.get(*i) {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('r') => s.push('\r'),
                            Some('b') => s.push('\u{8}'),
                            Some('f') => s.push('\u{c}'),
                            Some('u') => {
                                let hex: String = c.get(*i + 1..*i + 5).ok_or_else(|| bad(*i))?.iter().collect();
                                s.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('\u{fffd}'));
                                *i += 4;
                            }
                            Some(&ch) => s.push(ch),
                            None => return Err(bad(*i)),
                        }
                        *i += 1;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        *i += 1;
                    }
                }
            }
        }
        Some('t') if c.get(*i..*i + 4) == Some(&['t', 'r', 'u', 'e']) => {
            *i += 4;
            Ok(Json::Bool { v: true })
        }
        Some('f') if c.get(*i..*i + 5) == Some(&['f', 'a', 'l', 's', 'e']) => {
            *i += 5;
            Ok(Json::Bool { v: false })
        }
        Some('n') if c.get(*i..*i + 4) == Some(&['n', 'u', 'l', 'l']) => {
            *i += 4;
            Ok(Json::Null)
        }
        Some(ch) if *ch == '-' || ch.is_ascii_digit() => {
            let start = *i;
            *i += 1;
            while *i < c.len() && (c[*i].is_ascii_digit() || "+-.eE".contains(c[*i])) {
                *i += 1;
            }
            let t: String = c[start..*i].iter().collect();
            t.parse::<f64>().map(|v| Json::Num { v }).map_err(|_| bad(start))
        }
        _ => Err(bad(*i)),
    }
}
// j["key"] and j[0]; a missing key or index gives null.
impl AxIndex<String, Json> for Json {
    fn ax_at(&self, k: String) -> Json {
        match self {
            Json::Obj { v } => v.get(&k).cloned().unwrap_or(Json::Null),
            _ => Json::Null,
        }
    }
    fn ax_set(&mut self, k: String, x: Json) {
        if let Json::Obj { v } = self {
            v.insert(k, x);
        }
    }
}
impl AxIndex<i64, Json> for Json {
    fn ax_at(&self, i: i64) -> Json {
        match self {
            Json::Arr { v } => {
                let j = if i < 0 { v.len() as i64 + i } else { i };
                v.get(j.max(0) as usize).cloned().unwrap_or(Json::Null)
            }
            _ => Json::Null,
        }
    }
    fn ax_set(&mut self, i: i64, x: Json) {
        if let Json::Arr { v } = self {
            let j = ax_idx(i, v.len());
            v[j] = x;
        }
    }
}

// ---- CSV ----
pub fn csv(text: String) -> Vec<Vec<String>> {
    // Fields may be "quoted", with "" for a literal quote; quoted fields may hold commas and newlines.
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cur = String::new();
    let mut inq = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if inq && chars.peek() == Some(&'"') => {
                chars.next();
                cur.push('"');
            }
            '"' => inq = !inq,
            ',' if !inq => row.push(std::mem::take(&mut cur)),
            '\r' if !inq => {}
            '\n' if !inq => {
                row.push(std::mem::take(&mut cur));
                if !(row.len() == 1 && row[0].is_empty()) {
                    rows.push(std::mem::take(&mut row));
                }
                row.clear();
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows
}

// ---- dates (proleptic Gregorian) ----
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Date {
    pub days: i64,
}
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}
pub fn date(text: String) -> R<Date> {
    let bad = || AxErr(format!("not a date (want YYYY-MM-DD): {text:?}"));
    let p: Vec<&str> = text.trim().split('-').collect();
    if p.len() != 3 {
        return Err(bad());
    }
    let (y, m, d) = (p[0].parse::<i64>().map_err(|_| bad())?, p[1].parse::<i64>().map_err(|_| bad())?, p[2].parse::<i64>().map_err(|_| bad())?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&m) || d < 1 || d > dim[(m - 1) as usize] {
        return Err(bad());
    }
    Ok(Date { days: days_from_civil(y, m, d) })
}
impl Show for Date {
    fn show(&self) -> String {
        let (y, m, d) = civil_from_days(self.days);
        format!("{y:04}-{m:02}-{d:02}")
    }
}
pub trait AxDate {
    fn ax_days(&self) -> i64;
    fn ax_add_days(&self, n: i64) -> Date;
    fn ax_year(&self) -> i64;
    fn ax_month(&self) -> i64;
    fn ax_day(&self) -> i64;
    fn ax_weekday(&self) -> i64;
}
impl AxDate for Date {
    fn ax_days(&self) -> i64 {
        self.days
    }
    fn ax_add_days(&self, n: i64) -> Date {
        Date { days: self.days + n }
    }
    fn ax_year(&self) -> i64 {
        civil_from_days(self.days).0
    }
    fn ax_month(&self) -> i64 {
        civil_from_days(self.days).1
    }
    fn ax_day(&self) -> i64 {
        civil_from_days(self.days).2
    }
    fn ax_weekday(&self) -> i64 {
        // 1 = Monday ... 7 = Sunday; 1970-01-01 was a Thursday.
        (self.days + 3).rem_euclid(7) + 1
    }
}

// ---- big integers (sign + base 1e9 limbs, little-endian) ----
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Big {
    neg: bool,
    mag: Vec<u32>,
}
const BIG_BASE: u64 = 1_000_000_000;
fn mag_trim(mut v: Vec<u32>) -> Vec<u32> {
    while v.len() > 1 && *v.last().unwrap() == 0 {
        v.pop();
    }
    if v.is_empty() {
        v.push(0);
    }
    v
}
fn mag_cmp(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.iter().rev().cmp(b.iter().rev()))
}
fn mag_add(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut carry = 0u64;
    for i in 0..a.len().max(b.len()) {
        let s = *a.get(i).unwrap_or(&0) as u64 + *b.get(i).unwrap_or(&0) as u64 + carry;
        out.push((s % BIG_BASE) as u32);
        carry = s / BIG_BASE;
    }
    if carry > 0 {
        out.push(carry as u32);
    }
    mag_trim(out)
}
fn mag_sub(a: &[u32], b: &[u32]) -> Vec<u32> {
    // requires a >= b
    let mut out = Vec::new();
    let mut borrow = 0i64;
    for i in 0..a.len() {
        let mut d = a[i] as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
        borrow = if d < 0 { 1 } else { 0 };
        if d < 0 {
            d += BIG_BASE as i64;
        }
        out.push(d as u32);
    }
    mag_trim(out)
}
fn mag_mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u64; a.len() + b.len() + 1];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, &y) in b.iter().enumerate() {
            let cur = out[i + j] + x as u64 * y as u64 + carry;
            out[i + j] = cur % BIG_BASE;
            carry = cur / BIG_BASE;
        }
        let mut k = i + b.len();
        while carry > 0 {
            let cur = out[k] + carry;
            out[k] = cur % BIG_BASE;
            carry = cur / BIG_BASE;
            k += 1;
        }
    }
    mag_trim(out.into_iter().map(|x| x as u32).collect())
}
impl Big {
    fn make(neg: bool, mag: Vec<u32>) -> Big {
        let mag = mag_trim(mag);
        let zero = mag == vec![0];
        Big { neg: neg && !zero, mag }
    }
    pub fn from_i64(n: i64) -> Big {
        let mut m = n.unsigned_abs();
        let mut mag = Vec::new();
        while m > 0 {
            mag.push((m % BIG_BASE) as u32);
            m /= BIG_BASE;
        }
        Big::make(n < 0, mag)
    }
    pub fn parse(s: &str) -> R<Big> {
        let t = s.trim();
        let (neg, digits) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(AxErr(format!("not an int: {s:?}")));
        }
        let bytes = digits.as_bytes();
        let mut mag = Vec::new();
        let mut end = bytes.len();
        while end > 0 {
            let start = end.saturating_sub(9);
            mag.push(std::str::from_utf8(&bytes[start..end]).unwrap().parse::<u32>().unwrap());
            end = start;
        }
        Ok(Big::make(neg, mag))
    }
}
impl PartialOrd for Big {
    fn partial_cmp(&self, o: &Big) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        Some(match (self.neg, o.neg) {
            (false, true) => Greater,
            (true, false) => Less,
            (false, false) => mag_cmp(&self.mag, &o.mag),
            (true, true) => mag_cmp(&o.mag, &self.mag),
        })
    }
}
impl Default for Big {
    fn default() -> Big {
        Big::from_i64(0)
    }
}
impl Show for Big {
    fn show(&self) -> String {
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s.push_str(&self.mag.last().unwrap().to_string());
        for limb in self.mag.iter().rev().skip(1) {
            s.push_str(&format!("{limb:09}"));
        }
        s
    }
}
impl Plus for Big {
    fn plus(self, o: Big) -> Big {
        if self.neg == o.neg {
            return Big::make(self.neg, mag_add(&self.mag, &o.mag));
        }
        match mag_cmp(&self.mag, &o.mag) {
            std::cmp::Ordering::Less => Big::make(o.neg, mag_sub(&o.mag, &self.mag)),
            _ => Big::make(self.neg, mag_sub(&self.mag, &o.mag)),
        }
    }
}
impl std::ops::Sub for Big {
    type Output = Big;
    fn sub(self, o: Big) -> Big {
        self.plus(Big { neg: !o.neg, mag: o.mag.clone() }.normal())
    }
}
impl Big {
    fn normal(self) -> Big {
        Big::make(self.neg, self.mag)
    }
}
impl std::ops::Mul for Big {
    type Output = Big;
    fn mul(self, o: Big) -> Big {
        Big::make(self.neg != o.neg, mag_mul(&self.mag, &o.mag))
    }
}
impl std::ops::Neg for Big {
    type Output = Big;
    fn neg(self) -> Big {
        Big::make(!self.neg, self.mag)
    }
}
impl std::ops::SubAssign for Big {
    fn sub_assign(&mut self, o: Big) {
        *self = self.clone() - o;
    }
}
impl std::ops::MulAssign for Big {
    fn mul_assign(&mut self, o: Big) {
        *self = self.clone() * o;
    }
}
pub trait AxToBig {
    fn ax_big(&self) -> Big;
}
impl AxToBig for i64 {
    fn ax_big(&self) -> Big {
        Big::from_i64(*self)
    }
}
pub trait AxStrBig {
    fn ax_big(&self) -> R<Big>;
}
impl AxStrBig for String {
    fn ax_big(&self) -> R<Big> {
        Big::parse(self)
    }
}

// x.or(default): use the default instead of failing.
pub trait AxOr<T> {
    fn ax_or(self, d: T) -> T;
}
impl<T> AxOr<T> for R<T> {
    fn ax_or(self, d: T) -> T {
        self.unwrap_or(d)
    }
}
pub trait AxDigit {
    fn ax_digit(&self) -> i64;
}
impl AxDigit for String {
    fn ax_digit(&self) -> i64 {
        // Value of a single digit character, or -1.
        match self.chars().next() {
            Some(c) if self.chars().count() == 1 => c.to_digit(10).map_or(-1, |d| d as i64),
            _ => -1,
        }
    }
}

// ---- free numeric helpers ----
pub fn abs<T: AxNum>(x: T) -> T {
    x.ax_abs()
}
pub fn min<T: PartialOrd>(a: T, b: T) -> T {
    if b < a { b } else { a }
}
pub fn max<T: PartialOrd>(a: T, b: T) -> T {
    if b > a { b } else { a }
}
// ---- exact decimals: fixed point with 9 places ----
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Default)]
pub struct Dec(pub i128);
const DEC_ONE: i128 = 1_000_000_000;
impl Dec {
    fn parse(s: &str) -> R<Dec> {
        let t = s.trim();
        let (neg, body) = match t.strip_prefix('-') { Some(b) => (true, b), None => (false, t.strip_prefix('+').unwrap_or(t)) };
        let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
        let ok = !(ip.is_empty() && fp.is_empty()) && ip.chars().all(|c| c.is_ascii_digit()) && fp.chars().all(|c| c.is_ascii_digit()) && fp.len() <= 9;
        if !ok {
            return Err(AxErr(format!("not a decimal: {:?}", s)));
        }
        let i: i128 = if ip.is_empty() { 0 } else { ip.parse().map_err(|_| AxErr(format!("decimal too large: {:?}", s)))? };
        let f: i128 = format!("{:0<9}", fp).parse().unwrap_or(0);
        let v = i * DEC_ONE + f;
        Ok(Dec(if neg { -v } else { v }))
    }
    pub fn ax_fmt(&self, decimals: i64) -> String {
        let d = decimals.clamp(0, 9) as u32;
        let unit = 10i128.pow(9 - d);
        let (q, r) = (self.0.div_euclid(unit), self.0.rem_euclid(unit));
        // Round half to even, like Python's Decimal.
        let q = if r * 2 > unit || (r * 2 == unit && q % 2 != 0) { q + 1 } else { q };
        let neg = q < 0;
        let a = q.abs();
        let p = 10i128.pow(d);
        let s = if d == 0 { format!("{}", a / p) } else { format!("{}.{:0w$}", a / p, a % p, w = d as usize) };
        if neg { format!("-{s}") } else { s }
    }
    pub fn ax_abs(&self) -> Dec {
        Dec(self.0.abs())
    }
    pub fn ax_float(&self) -> f64 {
        self.0 as f64 / DEC_ONE as f64
    }
    pub fn ax_int(&self) -> i64 {
        (self.0 / DEC_ONE) as i64
    }
    pub fn ax_round(&self) -> Dec {
        Dec::parse(&self.ax_fmt(0)).unwrap()
    }
}
impl Show for Dec {
    fn show(&self) -> String {
        let s = self.ax_fmt(9);
        let s = s.trim_end_matches('0');
        s.trim_end_matches('.').to_string()
    }
}
impl Plus for Dec {
    fn plus(self, o: Dec) -> Dec {
        Dec(self.0 + o.0)
    }
}
impl std::ops::Sub for Dec {
    type Output = Dec;
    fn sub(self, o: Dec) -> Dec {
        Dec(self.0 - o.0)
    }
}
impl std::ops::Mul for Dec {
    type Output = Dec;
    fn mul(self, o: Dec) -> Dec {
        Dec(self.0 * o.0 / DEC_ONE)
    }
}
impl std::ops::Div for Dec {
    type Output = Dec;
    fn div(self, o: Dec) -> Dec {
        if o.0 == 0 {
            panic!("division by zero");
        }
        Dec(self.0 * DEC_ONE / o.0)
    }
}
impl std::ops::Neg for Dec {
    type Output = Dec;
    fn neg(self) -> Dec {
        Dec(-self.0)
    }
}
impl std::ops::SubAssign for Dec {
    fn sub_assign(&mut self, o: Dec) {
        self.0 -= o.0
    }
}
impl std::ops::MulAssign for Dec {
    fn mul_assign(&mut self, o: Dec) {
        *self = *self * o
    }
}
impl std::ops::DivAssign for Dec {
    fn div_assign(&mut self, o: Dec) {
        *self = *self / o
    }
}
impl AxSum<Dec> for Vec<Dec> {
    fn ax_sum(&self) -> Dec {
        Dec(self.iter().map(|d| d.0).sum())
    }
}
pub trait AxToDec {
    fn ax_dec(&self) -> Dec;
}
impl AxToDec for i64 {
    fn ax_dec(&self) -> Dec {
        Dec(*self as i128 * DEC_ONE)
    }
}
pub trait AxStrDec {
    fn ax_dec(&self) -> R<Dec>;
}
impl AxStrDec for String {
    fn ax_dec(&self) -> R<Dec> {
        Dec::parse(self)
    }
}

// ---- min-heap: pop gives the smallest item ----
#[derive(Debug, Clone, Default)]
pub struct AxHeap<T>(Vec<T>);
pub fn heap<T: PartialOrd + Clone>() -> AxHeap<T> {
    AxHeap(Vec::new())
}
impl<T: PartialOrd + Clone> AxHeap<T> {
    pub fn ax_push(&mut self, x: T) {
        let v = &mut self.0;
        v.push(x);
        let mut i = v.len() - 1;
        while i > 0 {
            let p = (i - 1) / 2;
            if v[i] < v[p] {
                v.swap(i, p);
                i = p;
            } else {
                break;
            }
        }
    }
    pub fn ax_pop(&mut self) -> T {
        let v = &mut self.0;
        if v.is_empty() {
            panic!("pop from an empty heap");
        }
        let last = v.len() - 1;
        v.swap(0, last);
        let top = v.pop().unwrap();
        let mut i = 0;
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < v.len() && v[l] < v[m] {
                m = l;
            }
            if r < v.len() && v[r] < v[m] {
                m = r;
            }
            if m == i {
                break;
            }
            v.swap(i, m);
            i = m;
        }
        top
    }
    pub fn ax_first(&self) -> T {
        self.0.first().cloned().expect("first of an empty heap")
    }
    pub fn ax_len(&self) -> i64 {
        self.0.len() as i64
    }
    pub fn ax_is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl<T: Show + PartialOrd + Clone + 'static> Show for AxHeap<T> {
    fn show(&self) -> String {
        let mut v = self.0.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v.show()
    }
}

// Script-level variables that functions use live in statics.
pub fn ax_gp<T>(p: *mut Option<T>, name: &str) -> &'static mut T {
    match unsafe { (*p).as_mut() } {
        Some(v) => v,
        None => panic!("{name} is used before it is set"),
    }
}
// ---- end of prelude ----
