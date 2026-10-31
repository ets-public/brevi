// Runtime prelude pasted at the top of every generated Rust file.
// Brevi methods map to ax_-prefixed trait methods so Brevi controls their names and int types.
#![allow(warnings)]
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

// Brevi's map: a hash map (fast lookups) that always lists its entries sorted by key.
#[derive(Default, Clone, Copy)]
pub struct AxHasher(u64);
impl Hasher for AxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        let mut c = bytes.chunks_exact(8);
        for w in &mut c {
            h = (h.rotate_left(5) ^ u64::from_le_bytes(w.try_into().unwrap())).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        for &b in c.remainder() {
            h = (h.rotate_left(5) ^ b as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        self.0 = h;
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn finish(&self) -> u64 {
        // Mix the high bits down so hashbrown's bucket choice sees them.
        let h = self.0;
        h ^ (h >> 29) ^ (h >> 47)
    }
}
#[derive(Clone, Debug)]
pub struct AxM<K, V> {
    m: HashMap<K, V, BuildHasherDefault<AxHasher>>,
    // Keys in insertion order, kept only for JSON objects, which list keys as written.
    ord: Option<Vec<K>>,
}
impl<K, V> Default for AxM<K, V> {
    fn default() -> Self {
        AxM { m: HashMap::default(), ord: None }
    }
}
impl<K: Hash + Eq + Ord, V> AxM<K, V> {
    pub fn new() -> Self {
        AxM { m: HashMap::default(), ord: None }
    }
    pub fn ordered() -> Self {
        AxM { m: HashMap::default(), ord: Some(Vec::new()) }
    }
    pub fn from<const N: usize>(xs: [(K, V); N]) -> Self {
        let mut m = AxM::new();
        for (k, v) in xs {
            m.m.insert(k, v);
        }
        m
    }
    pub fn len(&self) -> usize {
        self.m.len()
    }
    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.m.get(k)
    }
    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.m.get_mut(k)
    }
    pub fn contains_key(&self, k: &K) -> bool {
        self.m.contains_key(k)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V>
    where
        K: Clone,
    {
        if let Some(o) = &mut self.ord {
            if !self.m.contains_key(&k) {
                o.push(k.clone());
            }
        }
        self.m.insert(k, v)
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        if let Some(o) = &mut self.ord {
            o.retain(|x| x != k);
        }
        self.m.remove(k)
    }
    pub fn entry(&mut self, k: K) -> std::collections::hash_map::Entry<'_, K, V> {
        self.m.entry(k)
    }
    // Entries sorted by key (JSON objects: in insertion order).
    pub fn iter(&self) -> std::vec::IntoIter<(&K, &V)> {
        if let Some(o) = &self.ord {
            // Keys added another way (an entry) come after, in key order.
            let mut v: Vec<(&K, &V)> = o.iter().filter_map(|k| self.m.get_key_value(k)).collect();
            if v.len() < self.m.len() {
                let mut rest: Vec<(&K, &V)> = self.m.iter().filter(|(k, _)| !o.contains(k)).collect();
                rest.sort_unstable_by(|a, b| a.0.cmp(b.0));
                v.extend(rest);
            }
            return v.into_iter();
        }
        let mut v: Vec<(&K, &V)> = self.m.iter().collect();
        v.sort_unstable_by(|a, b| a.0.cmp(b.0));
        v.into_iter()
    }
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}
// `for x in xs` on a list that is used again reads its items without copying the whole list.
pub trait AxIter {
    type I<'a>: Iterator
    where
        Self: 'a;
    fn ax_iter<'a>(&'a self) -> Self::I<'a>;
}
impl<T: Clone> AxIter for Vec<T> {
    type I<'a> = std::iter::Cloned<std::slice::Iter<'a, T>> where T: 'a;
    fn ax_iter<'a>(&'a self) -> Self::I<'a> {
        self.iter().cloned()
    }
}
// What a `for` loop walks over: list items, map pairs, or a string's characters.
pub trait AxEach {
    type It: Iterator;
    fn ax_each(self) -> Self::It;
}
impl<T> AxEach for Vec<T> {
    type It = std::vec::IntoIter<T>;
    fn ax_each(self) -> Self::It {
        self.into_iter()
    }
}
impl<K: Hash + Eq + Ord, V> AxEach for AxM<K, V> {
    type It = <AxM<K, V> as IntoIterator>::IntoIter;
    fn ax_each(self) -> Self::It {
        self.into_iter()
    }
}
impl AxEach for String {
    type It = std::vec::IntoIter<String>;
    fn ax_each(self) -> Self::It {
        self.chars().map(|c| c.to_string()).collect::<Vec<_>>().into_iter()
    }
}
// A string read item by item is its list of characters, as in Python.
pub trait AxStrSeq {
    fn ax_map<U>(&self, f: impl Fn(String) -> U) -> Vec<U>;
    fn ax_filter(&self, f: impl Fn(String) -> bool) -> Vec<String>;
    fn ax_any(&self, f: impl Fn(String) -> bool) -> bool;
    fn ax_all(&self, f: impl Fn(String) -> bool) -> bool;
    fn ax_enumerate(&self) -> Vec<(i64, String)>;
}
impl AxStrSeq for String {
    fn ax_map<U>(&self, f: impl Fn(String) -> U) -> Vec<U> {
        self.ax_chars().into_iter().map(f).collect()
    }
    fn ax_filter(&self, f: impl Fn(String) -> bool) -> Vec<String> {
        self.ax_chars().into_iter().filter(|c| f(c.clone())).collect()
    }
    fn ax_any(&self, f: impl Fn(String) -> bool) -> bool {
        self.ax_chars().into_iter().any(f)
    }
    fn ax_all(&self, f: impl Fn(String) -> bool) -> bool {
        self.ax_chars().into_iter().all(f)
    }
    fn ax_enumerate(&self) -> Vec<(i64, String)> {
        self.ax_chars().into_iter().enumerate().map(|(i, c)| (i as i64, c)).collect()
    }
}
// `for c in s` reads a string's characters, as in Python.
impl AxIter for String {
    type I<'a> = std::vec::IntoIter<String>;
    fn ax_iter<'a>(&'a self) -> Self::I<'a> {
        self.chars().map(|c| c.to_string()).collect::<Vec<_>>().into_iter()
    }
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxIter for AxM<K, V> {
    type I<'a> = std::vec::IntoIter<(K, V)> where K: 'a, V: 'a;
    fn ax_iter<'a>(&'a self) -> Self::I<'a> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>().into_iter()
    }
}
impl<K: Hash + Eq + Ord, V> IntoIterator for AxM<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(mut self) -> Self::IntoIter {
        if let Some(o) = self.ord.take() {
            let mut v: Vec<(K, V)> = o.into_iter().filter_map(|k| self.m.remove(&k).map(|v| (k, v))).collect();
            let mut rest: Vec<(K, V)> = self.m.into_iter().collect();
            rest.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            v.extend(rest);
            return v.into_iter();
        }
        let mut v: Vec<(K, V)> = self.m.into_iter().collect();
        v.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        v.into_iter()
    }
}
impl<K: Hash + Eq, V: PartialEq> PartialEq for AxM<K, V> {
    fn eq(&self, o: &Self) -> bool {
        self.m == o.m
    }
}
impl<K: Hash + Eq + Ord, V: PartialOrd> PartialOrd for AxM<K, V> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        self.iter().partial_cmp(o.iter())
    }
}

#[derive(Debug, Clone)]
pub struct AxErr(pub String);
impl<E: std::fmt::Display> From<E> for AxErr {
    fn from(e: E) -> AxErr {
        AxErr(e.to_string())
    }
}
pub type R<T> = Result<T, AxErr>;

// Current Brevi line, so runtime errors can point at the source.
pub static AX_LINE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[inline(always)]
pub fn ax_l(n: usize) {
    AX_LINE.store(n, std::sync::atomic::Ordering::Relaxed);
}
// Where a line of the combined program is: lines after the main file's come from files it uses (`use "lib.bv"`).
pub fn ax_at(l: usize) -> String {
    for (f, start, n) in AX_FILES {
        if l >= *start && l < start + n {
            return format!("\"file\":{f:?},\"line\":{}", l - start + 1);
        }
    }
    format!("\"line\":{l}")
}
pub fn ax_init() {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => info.payload().downcast_ref::<String>().cloned().unwrap_or_else(|| "panic".into()),
        };
        let at = ax_at(AX_LINE.load(std::sync::atomic::Ordering::Relaxed));
        eprintln!("{{\"e\":\"runtime\",{at},\"msg\":{msg:?}}}");
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
impl Show for &str {
    fn show(&self) -> String {
        self.to_string()
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
impl<K: Hash + Eq + Ord + Show + 'static, V: Show + 'static> Show for AxM<K, V> {
    fn show(&self) -> String {
        // Written like compact JSON, so str({"error": msg}) is ready to send.
        let parts: Vec<String> = self.iter().map(|(k, v)| format!("{}:{}", show_item(k), show_item(v))).collect();
        format!("{{{}}}", parts.join(","))
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
// Python-style truth for conditions: false, 0, "", [] and {} are false.
pub trait AxTruth {
    fn ax_truth(&self) -> bool;
}
impl AxTruth for bool {
    fn ax_truth(&self) -> bool {
        *self
    }
}
impl AxTruth for i64 {
    fn ax_truth(&self) -> bool {
        *self != 0
    }
}
impl AxTruth for f64 {
    fn ax_truth(&self) -> bool {
        *self != 0.0
    }
}
impl AxTruth for String {
    fn ax_truth(&self) -> bool {
        !self.is_empty()
    }
}
impl<T> AxTruth for Vec<T> {
    fn ax_truth(&self) -> bool {
        !self.is_empty()
    }
}
impl<K, V> AxTruth for AxM<K, V> {
    fn ax_truth(&self) -> bool {
        !self.m.is_empty()
    }
}
impl AxTruth for Json {
    fn ax_truth(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool { v } => *v,
            Json::Num { v } => *v != 0.0,
            Json::Str { v } => !v.is_empty(),
            Json::Arr { v } => !v.is_empty(),
            Json::Obj { v } => !v.m.is_empty(),
        }
    }
}
impl<T> AxTruth for AxHeap<T> {
    fn ax_truth(&self) -> bool {
        !self.0.is_empty()
    }
}
impl AxTruth for Dec {
    fn ax_truth(&self) -> bool {
        self.0 != 0
    }
}
#[inline]
pub fn ax_t<T: AxTruth + ?Sized>(x: &T) -> bool {
    x.ax_truth()
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
// a // b rounds down, as in Python.
pub trait AxFloorDiv {
    fn floordiv(self, b: Self) -> Self;
}
impl AxFloorDiv for i64 {
    fn floordiv(self, b: i64) -> i64 {
        let q = self / b;
        if (self % b != 0) && ((self < 0) != (b < 0)) { q - 1 } else { q }
    }
}
impl AxFloorDiv for f64 {
    fn floordiv(self, b: f64) -> f64 {
        (self / b).floor()
    }
}
pub fn ax_floordiv<T: AxFloorDiv>(a: T, b: T) -> T {
    a.floordiv(b)
}
pub fn ax_approx(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * (1.0 + a.abs().max(b.abs()))
}

pub trait Plus<Rhs = Self> {
    type Out;
    fn plus(self, o: Rhs) -> Self::Out;
}
impl Plus for i64 {
    type Out = i64;
    fn plus(self, o: i64) -> i64 {
        self + o
    }
}
impl Plus for f64 {
    type Out = f64;
    fn plus(self, o: f64) -> f64 {
        self + o
    }
}
impl Plus for String {
    type Out = String;
    fn plus(self, o: String) -> String {
        self + &o
    }
}
impl<T> Plus for Vec<T> {
    type Out = Vec<T>;
    fn plus(mut self, o: Vec<T>) -> Vec<T> {
        self.extend(o);
        self
    }
}

// Python format specs in strings: `{x:.2f}` `{n:03d}` `{s:<10}` `{v:>8,}`.
pub trait AxSpec {
    fn ax_spec(&self, spec: String) -> String;
}
fn ax_spec_parts(spec: &str) -> (Option<char>, bool, bool, usize, bool, Option<usize>, String) {
    let mut s = spec;
    let align = s.chars().next().filter(|c| matches!(c, '<' | '>' | '^'));
    if align.is_some() {
        s = &s[1..];
    }
    let plus = s.starts_with('+');
    if plus {
        s = &s[1..];
    }
    let zero = s.starts_with('0');
    let w: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    s = &s[w.len()..];
    let comma = s.starts_with(',');
    if comma {
        s = &s[1..];
    }
    let prec = s.strip_prefix('.').map(|r| {
        let p: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
        s = &r[p.len()..];
        p.parse().unwrap_or(0)
    });
    (align, plus, zero, w.parse().unwrap_or(0), comma, prec, s.to_string())
}
fn ax_spec_pad(body: String, spec: &str, num: bool) -> String {
    let (align, _, zero, width, _, _, _) = ax_spec_parts(spec);
    let n = body.chars().count();
    if n >= width {
        return body;
    }
    let fill = width - n;
    if zero && num && align.is_none() {
        let (sign, digits) = if body.starts_with('-') || body.starts_with('+') { body.split_at(1) } else { ("", body.as_str()) };
        return format!("{sign}{}{digits}", "0".repeat(fill));
    }
    match align.unwrap_or(if num { '>' } else { '<' }) {
        '<' => format!("{body}{}", " ".repeat(fill)),
        '^' => format!("{}{body}{}", " ".repeat(fill / 2), " ".repeat(fill - fill / 2)),
        _ => format!("{}{body}", " ".repeat(fill)),
    }
}
fn ax_commas(s: &str) -> String {
    let (sign, rest) = if s.starts_with('-') { s.split_at(1) } else { ("", s) };
    let (int, frac) = match rest.find('.') { Some(i) => rest.split_at(i), None => (rest, "") };
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}{frac}")
}
fn ax_spec_num(v: f64, int: Option<i64>, spec: &str) -> String {
    let (_, plus, _, _, comma, prec, ty) = ax_spec_parts(spec);
    let mut body = match (ty.as_str(), int) {
        ("x", Some(i)) => format!("{i:x}"),
        ("%", _) => format!("{:.*}%", prec.unwrap_or(6), v * 100.0),
        ("e", _) => format!("{:.*e}", prec.unwrap_or(6), v),
        ("f", _) => format!("{:.*}", prec.unwrap_or(6), v),
        (_, Some(i)) if prec.is_none() => i.to_string(),
        _ => match prec {
            Some(p) => format!("{v:.p$}"),
            None => v.show(),
        },
    };
    if comma {
        body = ax_commas(&body);
    }
    if plus && v >= 0.0 {
        body = format!("+{body}");
    }
    ax_spec_pad(body, spec, true)
}
impl AxSpec for i64 {
    fn ax_spec(&self, spec: String) -> String {
        ax_spec_num(*self as f64, Some(*self), &spec)
    }
}
impl AxSpec for f64 {
    fn ax_spec(&self, spec: String) -> String {
        ax_spec_num(*self, None, &spec)
    }
}
impl AxSpec for Dec {
    fn ax_spec(&self, spec: String) -> String {
        let (_, _, _, _, comma, prec, _) = ax_spec_parts(&spec);
        let mut body = match prec {
            Some(p) => self.ax_fmt(p as i64),
            None => self.show(),
        };
        if comma {
            body = ax_commas(&body);
        }
        ax_spec_pad(body, &spec, true)
    }
}
impl AxSpec for String {
    fn ax_spec(&self, spec: String) -> String {
        let (_, _, _, _, _, prec, _) = ax_spec_parts(&spec);
        let body: String = match prec {
            Some(p) => self.chars().take(p).collect(),
            None => self.clone(),
        };
        ax_spec_pad(body, &spec, false)
    }
}
impl AxSpec for bool {
    fn ax_spec(&self, spec: String) -> String {
        ax_spec_pad(self.show(), &spec, false)
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
    fn ax_partition(&self, sep: String) -> (String, String, String);
    fn ax_lower(&self) -> String;
    fn ax_upper(&self) -> String;
    fn ax_trim(&self) -> String;
    fn ax_contains(&self, s: &String) -> bool;
    fn ax_starts_with(&self, s: String) -> bool;
    fn ax_ends_with(&self, s: String) -> bool;
    fn ax_replace(&self, a: String, b: String) -> String;
    fn ax_repeat(&self, n: i64) -> String;
    fn ax_int(&self) -> R<i64>;
    fn ax_float(&self) -> R<f64>;
    fn ax_ints(&self) -> R<Vec<i64>>;
    fn ax_slice(&self, a: i64, b: i64) -> String;
    fn ax_find(&self, s: String) -> i64;
    fn ax_find_from(&self, s: String, start: i64) -> i64;
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
    // As in Python: text before the first sep, sep, and the rest; ("s", "", "") when sep is missing.
    fn ax_partition(&self, sep: String) -> (String, String, String) {
        match self.find(&sep) {
            Some(i) => (self[..i].to_string(), sep.clone(), self[i + sep.len()..].to_string()),
            None => (self.clone(), String::new(), String::new()),
        }
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
    fn ax_contains(&self, s: &String) -> bool {
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
    // Python's s.find(sub, start): the index in the whole string, searching from start.
    fn ax_find_from(&self, s: String, start: i64) -> i64 {
        let n = self.chars().count() as i64;
        let st = if start < 0 { (n + start).max(0) } else { start.min(n) } as usize;
        let off: usize = self.char_indices().nth(st).map_or(self.len(), |(b, _)| b);
        match self[off..].find(s.as_str()) {
            Some(i) => (st + self[off..off + i].chars().count()) as i64,
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
    fn ax_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(T) -> K, rev: bool) -> Vec<T>;
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
    fn ax_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(T) -> K, rev: bool) -> Vec<T> {
        let mut v = self.clone();
        v.sort_by(|a, b| {
            let (x, y) = (f(a.clone()), f(b.clone()));
            let o = if rev { y.partial_cmp(&x) } else { x.partial_cmp(&y) };
            o.unwrap_or(std::cmp::Ordering::Equal)
        });
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
    fn ax_try_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(T) -> R<K>, rev: bool) -> R<Vec<T>>;
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
    fn ax_try_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(T) -> R<K>, rev: bool) -> R<Vec<T>> {
        let mut keyed: Vec<(K, T)> = self.iter().cloned().map(|x| Ok((f(x.clone())?, x))).collect::<R<_>>()?;
        keyed.sort_by(|a, b| if rev { b.0.partial_cmp(&a.0) } else { a.0.partial_cmp(&b.0) }.unwrap_or(std::cmp::Ordering::Equal));
        Ok(keyed.into_iter().map(|(_, x)| x).collect())
    }
}

pub trait AxOrdList<T> {
    fn ax_sort(&self) -> Vec<T>;
    fn ax_min(&self) -> R<T>;
    fn ax_max(&self) -> R<T>;
    fn ax_contains(&self, x: &T) -> bool;
    fn ax_index_of(&self, x: &T) -> i64;
}
impl<T: Clone + PartialOrd> AxOrdList<T> for Vec<T> {
    fn ax_sort(&self) -> Vec<T> {
        let mut v = self.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
    fn ax_min(&self) -> R<T> {
        let mut it = self.iter();
        let Some(mut m) = it.next() else { return Err(AxErr("min of an empty list".into())) };
        for x in it {
            if x < m {
                m = x;
            }
        }
        Ok(m.clone())
    }
    fn ax_max(&self) -> R<T> {
        let mut it = self.iter();
        let Some(mut m) = it.next() else { return Err(AxErr("max of an empty list".into())) };
        for x in it {
            if x > m {
                m = x;
            }
        }
        Ok(m.clone())
    }
    fn ax_contains(&self, x: &T) -> bool {
        self.iter().any(|y| y == x)
    }
    fn ax_index_of(&self, x: &T) -> i64 {
        self.iter().position(|y| y == x).map_or(-1, |i| i as i64)
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
impl<T: Show> AxJoin for Vec<T> {
    fn ax_join(&self, sep: String) -> String {
        self.iter().map(|x| x.show()).collect::<Vec<_>>().join(&sep)
    }
}

pub trait AxMap<K, V> {
    fn ax_len(&self) -> i64;
    fn ax_is_empty(&self) -> bool;
    fn ax_has(&self, k: &K) -> bool;
    fn ax_get_or(&self, k: &K, d: V) -> V;
    fn ax_remove(&mut self, k: K);
    fn ax_keys(&self) -> Vec<K>;
    fn ax_values(&self) -> Vec<V>;
    fn ax_pairs(&self) -> Vec<(K, V)>;
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxMap<K, V> for AxM<K, V> {
    fn ax_len(&self) -> i64 {
        self.len() as i64
    }
    fn ax_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn ax_has(&self, k: &K) -> bool {
        self.contains_key(k)
    }
    fn ax_get_or(&self, k: &K, d: V) -> V {
        self.get(k).cloned().unwrap_or(d)
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
// Reading or changing through an index without copying the item.
pub trait AxRef<K, V> {
    fn ax_ref(&self, k: &K) -> &V;
}
pub trait AxMut<K, V> {
    fn ax_mut(&mut self, k: &K) -> &mut V;
}
impl<T> AxRef<i64, T> for Vec<T> {
    fn ax_ref(&self, i: &i64) -> &T {
        &self[ax_idx(*i, self.len())]
    }
}
// t[0] on a tuple whose items share a type, as Python allows.
impl<T> AxRef<i64, T> for (T, T) {
    fn ax_ref(&self, i: &i64) -> &T {
        [&self.0, &self.1][ax_idx(*i, 2)]
    }
}
impl<T> AxRef<i64, T> for (T, T, T) {
    fn ax_ref(&self, i: &i64) -> &T {
        [&self.0, &self.1, &self.2][ax_idx(*i, 3)]
    }
}
impl<T> AxRef<i64, T> for (T, T, T, T) {
    fn ax_ref(&self, i: &i64) -> &T {
        [&self.0, &self.1, &self.2, &self.3][ax_idx(*i, 4)]
    }
}
impl<T> AxMut<i64, T> for Vec<T> {
    fn ax_mut(&mut self, i: &i64) -> &mut T {
        let j = ax_idx(*i, self.len());
        &mut self[j]
    }
}
impl<K: Hash + Eq + Ord + Show + 'static, V: Default + 'static> AxRef<K, V> for AxM<K, V> {
    fn ax_ref(&self, k: &K) -> &V {
        // A missing key reads as the type's zero value (0, "", [] ...), like Python's defaultdict.
        match self.get(k) {
            Some(v) => v,
            None => ax_zero::<V>(),
        }
    }
}
// One shared zero value per type, made on first use.
fn ax_zero<V: Default + 'static>() -> &'static V {
    use std::any::{Any, TypeId};
    thread_local! {
        static ZEROS: std::cell::RefCell<HashMap<TypeId, &'static dyn Any>> = std::cell::RefCell::new(HashMap::new());
    }
    ZEROS.with(|z| {
        let mut z = z.borrow_mut();
        let v: &'static dyn Any = *z.entry(TypeId::of::<V>()).or_insert_with(|| Box::leak(Box::new(V::default())));
        v.downcast_ref::<V>().unwrap()
    })
}
impl<K: Hash + Eq + Ord + Clone, V: Default> AxMut<K, V> for AxM<K, V> {
    fn ax_mut(&mut self, k: &K) -> &mut V {
        // Updating a missing key starts it at the default (0, "", [] ...); the key is copied only then.
        if let Some(v) = self.get_mut(k) {
            // Safe: the borrow ends here on this path (a known limit of Rust's borrow checker).
            return unsafe { &mut *(v as *mut V) };
        }
        self.entry(k.clone()).or_default()
    }
}
impl AxRef<i64, String> for String {
    fn ax_ref(&self, i: &i64) -> &String {
        // One-character strings live in a table (ASCII) or are kept for the program's life.
        static ASCII: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let c = if self.is_ascii() {
            self.as_bytes()[ax_idx(*i, self.len())] as char
        } else {
            let cs: Vec<char> = self.chars().collect();
            cs[ax_idx(*i, cs.len())]
        };
        if c.is_ascii() {
            &ASCII.get_or_init(|| (0..128u8).map(|b| (b as char).to_string()).collect())[c as usize]
        } else {
            Box::leak(Box::new(c.to_string()))
        }
    }
}
static AX_NULL: Json = Json::Null;
impl AxRef<String, Json> for Json {
    fn ax_ref(&self, k: &String) -> &Json {
        match self {
            Json::Obj { v } => v.get(k).unwrap_or(&AX_NULL),
            _ => &AX_NULL,
        }
    }
}
impl AxRef<i64, Json> for Json {
    fn ax_ref(&self, i: &i64) -> &Json {
        match self {
            Json::Arr { v } => {
                let j = if *i < 0 { v.len() as i64 + *i } else { *i };
                v.get(j.max(0) as usize).unwrap_or(&AX_NULL)
            }
            _ => &AX_NULL,
        }
    }
}
// j[k] with a Json key: a string looks up an object field, a number an array item.
impl AxRef<Json, Json> for Json {
    fn ax_ref(&self, k: &Json) -> &Json {
        match k {
            Json::Str { v } => self.ax_ref(v),
            Json::Num { v } => self.ax_ref(&(*v as i64)),
            _ => &AX_NULL,
        }
    }
}
impl AxIndex<Json, Json> for Json {
    fn ax_at(&self, k: Json) -> Json {
        self.ax_ref(&k).clone()
    }
    fn ax_set(&mut self, k: Json, x: Json) {
        match k {
            Json::Str { v } => self.ax_set(v, x),
            Json::Num { v } => self.ax_set(v as i64, x),
            _ => {}
        }
    }
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
impl<K: Hash + Eq + Ord + Clone + Show + 'static, V: Clone> AxIndex<K, V> for AxM<K, V> {
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
impl<K: Hash + Eq + Ord + Clone, V: Default> AxIndexMut<K, V> for AxM<K, V> {
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
    Obj { v: AxM<String, Json> },
}
// A Json value equals a string when it is that JSON string: `j["tier"] == "gold"`.
impl PartialEq<String> for Json {
    fn eq(&self, o: &String) -> bool {
        matches!(self, Json::Str { v } if v == o)
    }
}
impl PartialEq<Json> for String {
    fn eq(&self, o: &Json) -> bool {
        o == self
    }
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
            let mut m = AxM::ordered();
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


// Reading Json values directly: j.text(), j.int(), j.list(), j.is_null(), `for x in j`.
impl Json {
    pub fn ax_text(&self) -> String {
        match self {
            Json::Str { v } => v.clone(),
            Json::Null => String::new(),
            other => other.show(),
        }
    }
    pub fn ax_int(&self) -> i64 {
        match self {
            Json::Num { v } => *v as i64,
            Json::Bool { v } => *v as i64,
            _ => 0,
        }
    }
    pub fn ax_float(&self) -> f64 {
        match self {
            Json::Num { v } => *v,
            Json::Bool { v } => *v as i64 as f64,
            _ => 0.0,
        }
    }
    pub fn ax_dec(&self) -> Dec {
        match self {
            Json::Num { v } => Dec::parse(&v.to_string()).unwrap_or_default(),
            Json::Str { v } => Dec::parse(v.trim()).unwrap_or_default(),
            _ => Dec::default(),
        }
    }
    pub fn ax_bool(&self) -> bool {
        self.ax_truth()
    }
    pub fn ax_is_null(&self) -> bool {
        matches!(self, Json::Null)
    }
    pub fn ax_list(&self) -> Vec<Json> {
        match self {
            Json::Arr { v } => v.clone(),
            _ => vec![],
        }
    }
    pub fn ax_len(&self) -> i64 {
        match self {
            Json::Str { v } => v.chars().count() as i64,
            Json::Arr { v } => v.len() as i64,
            Json::Obj { v } => v.m.len() as i64,
            _ => 0,
        }
    }
    pub fn ax_is_empty(&self) -> bool {
        self.ax_len() == 0
    }
    pub fn ax_has(&self, k: &String) -> bool {
        match self {
            Json::Obj { v } => v.get(k).is_some(),
            _ => false,
        }
    }
    pub fn ax_keys(&self) -> Vec<String> {
        match self {
            Json::Obj { v } => v.iter().map(|(k, _)| k.clone()).collect(),
            _ => vec![],
        }
    }
    pub fn ax_values(&self) -> Vec<Json> {
        match self {
            Json::Obj { v } => v.iter().map(|(_, x)| x.clone()).collect(),
            Json::Arr { v } => v.clone(),
            _ => vec![],
        }
    }
}
impl AxEach for Json {
    type It = std::vec::IntoIter<Json>;
    fn ax_each(self) -> Self::It {
        self.ax_list().into_iter()
    }
}
impl AxIter for Json {
    type I<'a> = std::vec::IntoIter<Json>;
    fn ax_iter<'a>(&'a self) -> Self::I<'a> {
        self.ax_list().into_iter()
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
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    type Out = Big;
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

// ax_q(x)? works on both fallible results and plain values.
pub trait IntoR<T> {
    fn into_r(self) -> R<T>;
}
impl<T> IntoR<T> for R<T> {
    fn into_r(self) -> R<T> {
        self
    }
}
impl IntoR<i64> for i64 {
    fn into_r(self) -> R<i64> {
        Ok(self)
    }
}
impl IntoR<f64> for f64 {
    fn into_r(self) -> R<f64> {
        Ok(self)
    }
}
impl IntoR<Big> for Big {
    fn into_r(self) -> R<Big> {
        Ok(self)
    }
}
impl IntoR<Dec> for Dec {
    fn into_r(self) -> R<Dec> {
        Ok(self)
    }
}
// A failure where no error can be passed on (inside a plain lambda) stops the program.
pub fn ax_must<T>(x: R<T>) -> T {
    match x {
        Ok(v) => v,
        Err(e) => panic!("{}", e.0),
    }
}
pub fn ax_q<T, X: IntoR<T>>(x: X) -> R<T> {
    x.into_r()
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
// `.or(d)` after a step that cannot fail (`j["n"].int().or(0)`) keeps the value.
macro_rules! ax_or_plain {
    ($($t:ty),*) => {$(impl AxOr<$t> for $t {
        fn ax_or(self, _d: $t) -> $t {
            self
        }
    })*};
}
ax_or_plain!(i64, f64, bool, String, Dec);
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

// "a".code() is 97 and 97.char() is "a".
pub trait AxCode {
    fn ax_code(&self) -> i64;
}
impl AxCode for String {
    fn ax_code(&self) -> i64 {
        self.chars().next().map_or(-1, |c| c as i64)
    }
}
pub trait AxChar {
    fn ax_char(&self) -> String;
}
impl AxChar for i64 {
    fn ax_char(&self) -> String {
        char::from_u32(*self as u32).map_or(String::new(), |c| c.to_string())
    }
}
// [0].repeat(n) is a list of n zeros; unique() sorts and drops repeats.
pub trait AxListRepeat<T> {
    fn ax_repeat(&self, n: i64) -> Vec<T>;
}
impl<T: Clone> AxListRepeat<T> for Vec<T> {
    fn ax_repeat(&self, n: i64) -> Vec<T> {
        let mut v = Vec::with_capacity(self.len() * n.max(0) as usize);
        for _ in 0..n.max(0) {
            v.extend_from_slice(self);
        }
        v
    }
}
pub trait AxStep<T> {
    fn ax_step(&self, n: i64) -> Vec<T>;
}
impl<T: Clone> AxStep<T> for Vec<T> {
    fn ax_step(&self, n: i64) -> Vec<T> {
        self.iter().step_by(n.max(1) as usize).cloned().collect()
    }
}
pub trait AxUnique<T> {
    fn ax_unique(&self) -> Vec<T>;
}
impl<T: Clone + PartialOrd> AxUnique<T> for Vec<T> {
    fn ax_unique(&self) -> Vec<T> {
        let mut v = self.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v.dedup_by(|a, b| a == b);
        v
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
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Default, Hash)]
pub struct Dec(pub i128);
// A dec compares directly with an int: `price > 0`.
impl PartialEq<i64> for Dec {
    fn eq(&self, o: &i64) -> bool {
        self.0 == (*o as i128) * 1_000_000_000
    }
}
impl PartialOrd<i64> for Dec {
    fn partial_cmp(&self, o: &i64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&((*o as i128) * 1_000_000_000))
    }
}
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
    type Out = Dec;
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
// Ints mix with decs in arithmetic: `0 - amt`, `price * 2`, `total += 1`.
fn dec_of(n: i64) -> Dec {
    Dec(n as i128 * DEC_ONE)
}
impl Plus<i64> for Dec {
    type Out = Dec;
    fn plus(self, o: i64) -> Dec {
        Dec(self.0 + dec_of(o).0)
    }
}
impl Plus<Dec> for i64 {
    type Out = Dec;
    fn plus(self, o: Dec) -> Dec {
        Dec(dec_of(self).0 + o.0)
    }
}
macro_rules! dec_int_ops {
    ($($tr:ident $f:ident $atr:ident $af:ident),*) => {$(
        impl std::ops::$tr<i64> for Dec {
            type Output = Dec;
            fn $f(self, o: i64) -> Dec {
                std::ops::$tr::$f(self, dec_of(o))
            }
        }
        impl std::ops::$tr<Dec> for i64 {
            type Output = Dec;
            fn $f(self, o: Dec) -> Dec {
                std::ops::$tr::$f(dec_of(self), o)
            }
        }
        impl std::ops::$atr<i64> for Dec {
            fn $af(&mut self, o: i64) {
                *self = std::ops::$tr::$f(*self, dec_of(o))
            }
        }
    )*};
}
dec_int_ops!(Sub sub SubAssign sub_assign, Mul mul MulAssign mul_assign, Div div DivAssign div_assign);
impl PartialEq<Dec> for i64 {
    fn eq(&self, o: &Dec) -> bool {
        dec_of(*self) == *o
    }
}
impl PartialOrd<Dec> for i64 {
    fn partial_cmp(&self, o: &Dec) -> Option<std::cmp::Ordering> {
        dec_of(*self).partial_cmp(o)
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

// `x in xs`: a map key, a list item, or a substring.
pub trait AxIn<X> {
    fn ax_in(&self, x: &X) -> bool;
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxIn<K> for AxM<K, V> {
    fn ax_in(&self, x: &K) -> bool {
        self.contains_key(x)
    }
}
impl<T: PartialEq> AxIn<T> for Vec<T> {
    fn ax_in(&self, x: &T) -> bool {
        self.iter().any(|y| y == x)
    }
}
impl AxIn<String> for String {
    fn ax_in(&self, x: &String) -> bool {
        self.contains(x.as_str())
    }
}

// `x.round(n)`: n decimal places, half to even.
pub trait AxRoundTo {
    fn ax_round_to(&self, n: i64) -> Self;
}
impl AxRoundTo for Dec {
    fn ax_round_to(&self, n: i64) -> Dec {
        Dec::parse(&self.ax_fmt(n)).unwrap()
    }
}
impl AxRoundTo for f64 {
    fn ax_round_to(&self, n: i64) -> f64 {
        format!("{:.*}", n.clamp(0, 15) as usize, self).parse().unwrap_or(*self)
    }
}

// A boxed field, for types that contain themselves (an expression tree).
#[derive(Clone, PartialEq, PartialOrd, Default)]
pub struct AxB<T>(pub Box<T>);
impl<T> AxB<T> {
    pub fn new(x: T) -> Self {
        AxB(Box::new(x))
    }
}
impl<T> std::ops::Deref for AxB<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> std::ops::DerefMut for AxB<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for AxB<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// x.json(): any value as a Json value, so str(x.json()) writes it as compact JSON.
pub trait AxToJson {
    fn ax_json(&self) -> Json;
}
impl AxToJson for Json {
    fn ax_json(&self) -> Json {
        self.clone()
    }
}
impl AxToJson for i64 {
    fn ax_json(&self) -> Json {
        Json::Num { v: *self as f64 }
    }
}
impl AxToJson for f64 {
    fn ax_json(&self) -> Json {
        Json::Num { v: *self }
    }
}
impl AxToJson for Dec {
    fn ax_json(&self) -> Json {
        Json::Num { v: self.ax_float() }
    }
}
impl AxToJson for bool {
    fn ax_json(&self) -> Json {
        Json::Bool { v: *self }
    }
}
impl AxToJson for String {
    fn ax_json(&self) -> Json {
        Json::Str { v: self.clone() }
    }
}
impl AxToJson for &str {
    fn ax_json(&self) -> Json {
        Json::Str { v: self.to_string() }
    }
}
impl<T: AxToJson> AxToJson for Vec<T> {
    fn ax_json(&self) -> Json {
        Json::Arr { v: self.iter().map(|x| x.ax_json()).collect() }
    }
}
impl<T: AxToJson> AxToJson for Option<T> {
    fn ax_json(&self) -> Json {
        self.as_ref().map_or(Json::Null, |x| x.ax_json())
    }
}
impl<K: Show + Hash + Eq + Ord + Clone, V: AxToJson> AxToJson for AxM<K, V> {
    fn ax_json(&self) -> Json {
        let mut m = AxM::ordered();
        for (k, v) in self.iter() {
            m.insert(k.show(), v.ax_json());
        }
        Json::Obj { v: m }
    }
}
// A map literal whose values differ in type is a JSON object, keys in written order.
pub fn ax_obj(xs: Vec<(String, Json)>) -> Json {
    let mut m = AxM::ordered();
    for (k, v) in xs {
        m.insert(k, v);
    }
    Json::Obj { v: m }
}

// @regex: only included in programs that use regexes.
// Regular expressions (a backtracking matcher): s.re_find(p), re_full, re_all, re_sub, re_split.
#[derive(Debug)]
pub enum AxRx {
    Char(char),
    Any,
    Class(Vec<(char, char)>, bool),
    Start,
    End,
    WordB,
    Group(Box<AxRx>, Option<usize>),
    Alt(Vec<AxRx>),
    Cat(Vec<AxRx>),
    Rep(Box<AxRx>, usize, usize, bool),
}
struct AxRxP {
    c: Vec<char>,
    i: usize,
    groups: usize,
}
fn ax_rx_class(e: char) -> Option<(Vec<(char, char)>, bool)> {
    let d = vec![('0', '9')];
    let w = vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')];
    let s = vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')];
    Some(match e {
        'd' => (d, false),
        'D' => (d, true),
        'w' => (w, false),
        'W' => (w, true),
        's' => (s, false),
        'S' => (s, true),
        _ => return None,
    })
}
fn ax_rx_esc(e: char) -> char {
    match e {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        c => c,
    }
}
impl AxRxP {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn alt(&mut self) -> AxRx {
        let mut v = vec![self.cat()];
        while self.peek() == Some('|') {
            self.i += 1;
            v.push(self.cat());
        }
        if v.len() == 1 { v.pop().unwrap() } else { AxRx::Alt(v) }
    }
    fn cat(&mut self) -> AxRx {
        let mut v = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let a = self.atom();
            v.push(self.quant(a));
        }
        AxRx::Cat(v)
    }
    fn num(&mut self) -> Option<usize> {
        let s = self.i;
        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            self.i += 1;
        }
        self.c[s..self.i].iter().collect::<String>().parse().ok()
    }
    fn quant(&mut self, a: AxRx) -> AxRx {
        let (lo, hi) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let save = self.i;
                self.i += 1;
                let lo = self.num();
                let hi = if self.peek() == Some(',') {
                    self.i += 1;
                    self.num().unwrap_or(usize::MAX)
                } else {
                    lo.unwrap_or(0)
                };
                match (lo, self.peek()) {
                    (Some(lo), Some('}')) => (lo, hi),
                    _ => {
                        self.i = save;
                        return a;
                    }
                }
            }
            _ => return a,
        };
        self.i += 1;
        let lazy = self.peek() == Some('?');
        if lazy {
            self.i += 1;
        }
        let r = AxRx::Rep(Box::new(a), lo, hi, lazy);
        self.quant(r)
    }
    fn atom(&mut self) -> AxRx {
        let c = self.c[self.i];
        self.i += 1;
        match c {
            '.' => AxRx::Any,
            '^' => AxRx::Start,
            '$' => AxRx::End,
            '(' => {
                let idx = if self.c[self.i..].starts_with(&['?', ':']) {
                    self.i += 2;
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let inner = self.alt();
                if self.peek() == Some(')') {
                    self.i += 1;
                }
                AxRx::Group(Box::new(inner), idx)
            }
            '[' => {
                let neg = self.peek() == Some('^');
                if neg {
                    self.i += 1;
                }
                let mut rs = Vec::new();
                let mut first = true;
                while let Some(c) = self.peek() {
                    if c == ']' && !first {
                        break;
                    }
                    first = false;
                    self.i += 1;
                    let lo = if c == '\\' {
                        let e = self.c[self.i];
                        self.i += 1;
                        if let Some((cs, false)) = ax_rx_class(e) {
                            rs.extend(cs);
                            continue;
                        }
                        ax_rx_esc(e)
                    } else {
                        c
                    };
                    if self.peek() == Some('-') && self.c.get(self.i + 1).map_or(false, |&n| n != ']') {
                        let mut hi = self.c[self.i + 1];
                        self.i += 2;
                        if hi == '\\' {
                            hi = ax_rx_esc(self.c[self.i]);
                            self.i += 1;
                        }
                        rs.push((lo, hi));
                    } else {
                        rs.push((lo, lo));
                    }
                }
                self.i += 1;
                AxRx::Class(rs, neg)
            }
            '\\' => {
                let e = self.c[self.i];
                self.i += 1;
                if e == 'b' {
                    return AxRx::WordB;
                }
                match ax_rx_class(e) {
                    Some((cs, neg)) => AxRx::Class(cs, neg),
                    None => AxRx::Char(ax_rx_esc(e)),
                }
            }
            c => AxRx::Char(c),
        }
    }
}
type AxCaps = Vec<Option<(usize, usize)>>;
fn ax_rx_word(s: &[char], i: usize) -> bool {
    i < s.len() && (s[i].is_alphanumeric() || s[i] == '_')
}
fn ax_rx_m(n: &AxRx, s: &[char], i: usize, caps: &mut AxCaps, k: &mut dyn FnMut(usize, &mut AxCaps) -> bool) -> bool {
    match n {
        AxRx::Char(c) => i < s.len() && s[i] == *c && k(i + 1, caps),
        AxRx::Any => i < s.len() && s[i] != '\n' && k(i + 1, caps),
        AxRx::Class(rs, neg) => i < s.len() && rs.iter().any(|&(a, b)| a <= s[i] && s[i] <= b) != *neg && k(i + 1, caps),
        AxRx::Start => i == 0 && k(i, caps),
        AxRx::End => i == s.len() && k(i, caps),
        AxRx::WordB => (i > 0 && ax_rx_word(s, i - 1)) != ax_rx_word(s, i) && k(i, caps),
        AxRx::Cat(v) => ax_rx_seq(v, s, i, caps, k),
        AxRx::Alt(v) => v.iter().any(|a| ax_rx_m(a, s, i, caps, k)),
        AxRx::Group(inner, idx) => ax_rx_m(inner, s, i, caps, &mut |j, caps: &mut AxCaps| {
            let Some(g) = idx else { return k(j, caps) };
            let old = caps[*g];
            caps[*g] = Some((i, j));
            k(j, caps) || {
                caps[*g] = old;
                false
            }
        }),
        AxRx::Rep(inner, lo, hi, lazy) => ax_rx_rep(inner, *lo, *hi, *lazy, 0, s, i, caps, k),
    }
}
fn ax_rx_seq(v: &[AxRx], s: &[char], i: usize, caps: &mut AxCaps, k: &mut dyn FnMut(usize, &mut AxCaps) -> bool) -> bool {
    match v.split_first() {
        None => k(i, caps),
        Some((a, rest)) => ax_rx_m(a, s, i, caps, &mut |j, caps: &mut AxCaps| ax_rx_seq(rest, s, j, caps, k)),
    }
}
fn ax_rx_rep(n: &AxRx, lo: usize, hi: usize, lazy: bool, count: usize, s: &[char], i: usize, caps: &mut AxCaps, k: &mut dyn FnMut(usize, &mut AxCaps) -> bool) -> bool {
    if lazy && count >= lo && k(i, caps) {
        return true;
    }
    if count < hi && ax_rx_m(n, s, i, caps, &mut |j, caps: &mut AxCaps| (j != i || count < lo) && ax_rx_rep(n, lo, hi, lazy, count + 1, s, j, caps, k)) {
        return true;
    }
    !lazy && count >= lo && k(i, caps)
}
thread_local! {
    static AX_RX: std::cell::RefCell<HashMap<String, std::rc::Rc<(AxRx, usize)>>> = std::cell::RefCell::new(HashMap::new());
}
fn ax_rx(p: &str) -> std::rc::Rc<(AxRx, usize)> {
    AX_RX.with(|c| {
        c.borrow_mut()
            .entry(p.to_string())
            .or_insert_with(|| {
                let mut ps = AxRxP { c: p.chars().collect(), i: 0, groups: 0 };
                let r = ps.alt();
                std::rc::Rc::new((r, ps.groups))
            })
            .clone()
    })
}
// The first match at or after `from`: (start, end, groups).
fn ax_rx_search(p: &str, s: &[char], from: usize, full: bool) -> Option<(usize, AxCaps)> {
    let rx = ax_rx(p);
    for st in from..=s.len() {
        let mut caps: AxCaps = vec![None; rx.1 + 1];
        let mut end = None;
        if ax_rx_m(&rx.0, s, st, &mut caps, &mut |j, c: &mut AxCaps| {
            if full && j != s.len() {
                return false;
            }
            c[0] = Some((st, j));
            end = Some(j);
            true
        }) {
            let _ = end;
            return Some((st, caps));
        }
        if full {
            break;
        }
    }
    None
}
fn ax_rx_groups(s: &[char], caps: &AxCaps) -> Vec<String> {
    caps.iter().map(|g| g.map_or(String::new(), |(a, b)| s[a..b].iter().collect())).collect()
}
pub trait AxRe {
    fn ax_re_find(&self, p: String) -> Vec<String>;
    fn ax_re_full(&self, p: String) -> Vec<String>;
    fn ax_re_all(&self, p: String) -> Vec<Vec<String>>;
    fn ax_re_list(&self, p: String) -> Vec<String>;
    fn ax_re_sub(&self, p: String, r: String) -> String;
    fn ax_re_sub_with<F: FnMut(Vec<String>) -> String>(&self, p: String, f: F) -> String;
    fn ax_re_split(&self, p: String) -> Vec<String>;
}
fn ax_rx_each(s: &str, p: &str, mut f: impl FnMut(&[char], &AxCaps)) -> Vec<char> {
    let cs: Vec<char> = s.chars().collect();
    let mut from = 0;
    while from <= cs.len() {
        let Some((_, caps)) = ax_rx_search(p, &cs, from, false) else { break };
        let (a, b) = caps[0].unwrap();
        f(&cs, &caps);
        from = if b > a { b } else { b + 1 };
    }
    cs
}
impl AxRe for String {
    fn ax_re_find(&self, p: String) -> Vec<String> {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search(&p, &cs, 0, false).map_or(vec![], |(_, c)| ax_rx_groups(&cs, &c))
    }
    fn ax_re_full(&self, p: String) -> Vec<String> {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search(&p, &cs, 0, true).map_or(vec![], |(_, c)| ax_rx_groups(&cs, &c))
    }
    fn ax_re_all(&self, p: String) -> Vec<Vec<String>> {
        let mut out = vec![];
        ax_rx_each(self, &p, |cs, c| out.push(ax_rx_groups(cs, c)));
        out
    }
    // The text of every match, like Python's re.findall without groups (tokenizing: `s.re_list("\\d+|\\S")`).
    fn ax_re_list(&self, p: String) -> Vec<String> {
        self.ax_re_all(p).into_iter().map(|mut g| g.swap_remove(0)).collect()
    }
    fn ax_re_sub(&self, p: String, r: String) -> String {
        let rc: Vec<char> = r.chars().collect();
        self.ax_re_sub_with(p, |g| {
            let mut o = String::new();
            let mut i = 0;
            while i < rc.len() {
                if rc[i] == '\\' && i + 1 < rc.len() && rc[i + 1].is_ascii_digit() {
                    o.push_str(g.get(rc[i + 1] as usize - '0' as usize).map_or("", |s| s.as_str()));
                    i += 2;
                } else {
                    o.push(rc[i]);
                    i += 1;
                }
            }
            o
        })
    }
    fn ax_re_sub_with<F: FnMut(Vec<String>) -> String>(&self, p: String, mut f: F) -> String {
        let mut out = String::new();
        let mut last = 0;
        let cs = ax_rx_each(self, &p, |cs, c| {
            let (a, b) = c[0].unwrap();
            if a >= last {
                out.extend(&cs[last..a]);
                out.push_str(&f(ax_rx_groups(cs, c)));
                last = b;
            }
        });
        out.extend(&cs[last.min(cs.len())..]);
        out
    }
    fn ax_re_split(&self, p: String) -> Vec<String> {
        let mut out = vec![];
        let mut last = 0;
        let cs = ax_rx_each(self, &p, |cs, c| {
            let (a, b) = c[0].unwrap();
            if b > a {
                out.push(cs[last..a].iter().collect());
                last = b;
            }
        });
        out.push(cs[last..].iter().collect());
        out
    }
}
