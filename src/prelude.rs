// Runtime prelude pasted at the top of every generated Rust file.
// Brevi methods map to ax_-prefixed trait methods so Brevi controls their names and int types.
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
    // Keys in insertion order, kept for JSON objects, which list keys as written, and in Python mode,
    // where dicts keep insertion order.
    ord: Option<Vec<K>>,
    // A Python defaultdict or Counter: a missing key reads as the value type's zero instead of raising KeyError.
    lax: bool,
}
impl<K, V> Default for AxM<K, V> {
    fn default() -> Self {
        AxM { m: HashMap::default(), ord: if ax_py_on() { Some(Vec::new()) } else { None }, lax: false }
    }
}
impl<K: Hash + Eq + Ord, V> AxM<K, V> {
    pub fn new() -> Self {
        AxM::default()
    }
    pub fn ordered() -> Self {
        AxM { m: HashMap::default(), ord: Some(Vec::new()), lax: false }
    }
    pub fn from<const N: usize>(xs: [(K, V); N]) -> Self
    where
        K: Clone,
    {
        let mut m = AxM::new();
        for (k, v) in xs {
            m.insert(k, v);
        }
        m
    }
    pub fn len(&self) -> usize {
        self.m.len()
    }
    // Python's defaultdict(...) and Counter(): missing keys read as zero.
    pub fn ax_lax(mut self) -> Self {
        self.lax = true;
        self
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
    pub fn entry(&mut self, k: K) -> std::collections::hash_map::Entry<'_, K, V>
    where
        K: Clone,
    {
        if let Some(o) = &mut self.ord {
            if !self.m.contains_key(&k) {
                o.push(k.clone());
            }
        }
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
    fn ax_any(&self, f: impl FnMut(String) -> bool) -> bool;
    fn ax_all(&self, f: impl FnMut(String) -> bool) -> bool;
    fn ax_enumerate(&self) -> Vec<(i64, String)>;
}
impl AxStrSeq for String {
    fn ax_map<U>(&self, f: impl Fn(String) -> U) -> Vec<U> {
        self.ax_chars().into_iter().map(f).collect()
    }
    fn ax_filter(&self, f: impl Fn(String) -> bool) -> Vec<String> {
        self.ax_chars().into_iter().filter(|c| f(c.clone())).collect()
    }
    fn ax_any(&self, mut f: impl FnMut(String) -> bool) -> bool {
        self.ax_chars().into_iter().any(f)
    }
    fn ax_all(&self, mut f: impl FnMut(String) -> bool) -> bool {
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
    for (f, start, n) in AX_FILES_REG.get().copied().unwrap_or(&[]) {
        if l >= *start && l < start + n {
            return format!("\"file\":{f:?},\"line\":{}", l - start + 1);
        }
    }
    format!("\"line\":{l}")
}
// The program's file table, set by ax_init, so the prelude can also be a separately compiled library.
pub static AX_FILES_REG: std::sync::OnceLock<&'static [(&'static str, usize, usize)]> = std::sync::OnceLock::new();
pub fn ax_init(files: &'static [(&'static str, usize, usize)], fs: AxFs) {
    AX_FILES_REG.set(files).ok();
    AX_FS_REG.set(fs).ok();
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => info.payload().downcast_ref::<String>().cloned().unwrap_or_else(|| "panic".into()),
        };
        let at = ax_at(AX_LINE.load(std::sync::atomic::Ordering::Relaxed));
        eprintln!("{{\"e\":\"runtime\",{at},\"msg\":{msg:?}}}");
    }));
}

// Python mode: a program written as Python (it uses def, class or global) prints as Python does:
// True/False, 2.0, ['a', 'b'], {'k': 1}. Brevi-style programs keep Brevi's output.
pub static AX_PY_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn ax_py_mode() {
    AX_PY_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
}
// A real .py file, not a .bv program written in Python style: those keep the output they always had.
pub static AX_PY_FILE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn ax_py_file() {
    AX_PY_FILE.store(true, std::sync::atomic::Ordering::Relaxed);
}
// Held by main in a Python file. The typed build raises most exceptions as panics, and Rust exits 101 after one
// that reaches main; CPython exits 1. Exiting here, not in the panic hook, leaves a panic that a catch_unwind
// stops (a Brevi callback run by a Python library) alone. process::exit flushes stdout.
pub struct AxPyExit;
impl Drop for AxPyExit {
    fn drop(&mut self) {
        if std::thread::panicking() {
            std::process::exit(1);
        }
    }
}
#[inline]
fn ax_py_on() -> bool {
    AX_PY_MODE.load(std::sync::atomic::Ordering::Relaxed)
}
// Python's repr of a string: single quotes unless the text has a ' and no ".
fn ax_py_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (0x7f..0xa0).contains(&(c as u32)) || c as u32 == 0xad => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}
// Python's str of a float: always a decimal point (2.0), inf and nan as Python spells them.
fn ax_py_float(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let a = x.abs();
    // Rust's shortest digits round a tie between two shortest candidates up; Python takes the exact value
    // rounded half-to-even (-254.62014770507812, not ...813).
    // (Two shortest candidates fit only when 15+ digits are needed.)
    let mut t = format!("{x:e}");
    let (nd, ex) = match t.split_once('e') {
        Some((m, e)) => (m.bytes().filter(|c| c.is_ascii_digit()).count(), e.parse::<i32>().unwrap_or(0)),
        None => (0, 0),
    };
    {
        if nd >= 15 {
            let r = format!("{:.*e}", nd - 1, x);
            if r != t && r.parse::<f64>() == Ok(x) {
                t = r;
                if !(a >= 1e16 || a < 1e-4) {
                    let fd = (nd as i32 - 1 - ex).max(0) as usize;
                    let p = format!("{:.*}", fd, x);
                    return if p.contains('.') { p } else { format!("{p}.0") };
                }
            }
        }
    }
    if a != 0.0 && (a >= 1e16 || a < 1e-4) {
        // 1e-05 and 1e+16, as Python writes them.
        let (m, e) = t.split_once('e').unwrap();
        let n: i32 = e.parse().unwrap();
        let m = if m.contains('.') { m.to_string() } else { m.to_string() };
        return format!("{m}e{}{:02}", if n < 0 { '-' } else { '+' }, n.abs());
    }
    format!("{x:?}")
}
// Reading a field of a Python object that is None.
#[cold]
pub fn ax_none_attr() -> ! {
    panic!("'NoneType' object has no attribute")
}
// A new object for a class constructor to fill in.
pub trait AxFresh {
    fn ax_fresh() -> Self;
}
// Values Python and Brevi both share: objects, dicts made records, mixed-list rows, and lists of them.
pub trait AxShared {}
impl<T> AxShared for AxRow<T> {}
impl<T: AxShared> AxShared for Vec<T> {}
impl<K, V: AxShared> AxShared for AxM<K, V> {}
pub fn ax_shared<T: AxShared>(_: &T) {}
// A value that is itself shared (an object, a row), so a second name for it sees appends and item changes.
pub trait AxWhole {}
impl<T> AxWhole for AxRow<T> {}
pub fn ax_shared_whole<T: AxWhole>(_: &T) {}
// A value `x += v` sets anew (a number, text, an object) rather than changing in place, as a list does.
pub trait AxAug {}
impl<T> AxAug for AxRow<T> {}
impl AxAug for i64 {}
impl AxAug for f64 {}
impl AxAug for bool {}
impl AxAug for String {}
impl AxAug for Big {}
impl AxAug for Dec {}
pub fn ax_shared_aug<T: AxAug>(_: &T) {}
pub trait AxIsNoneHandle {
    fn ax_null(&self) -> bool;
}
pub trait Show {
    fn show(&self) -> String;
    // repr() when it differs from str(): a Python class with both __str__ and __repr__ (or a dataclass's
    // generated one) shows its repr inside lists, dicts and tuples, and for repr() and !r.
    fn show_repr(&self) -> Option<String> {
        None
    }
}
impl Show for i64 {
    fn show(&self) -> String {
        // None (see AxNone) prints as Python does.
        if *self == i64::MIN && ax_py_on() {
            return "None".into();
        }
        self.to_string()
    }
}
impl Show for f64 {
    fn show(&self) -> String {
        // f64::MIN stands for None in a Python float slot.
        if ax_py_on() && *self == f64::MIN {
            "None".into()
        } else if ax_py_on() {
            ax_py_float(*self)
        } else {
            self.to_string()
        }
    }
}
impl Show for bool {
    fn show(&self) -> String {
        match (ax_py_on(), *self) {
            (true, true) => "True".into(),
            (true, false) => "False".into(),
            _ => self.to_string(),
        }
    }
}
impl Show for String {
    fn show(&self) -> String {
        if self == "\u{0}None" {
            return "None".into();
        }
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
        // A Python function that gives no value gives None.
        if ax_py_on() { "None".into() } else { "()".into() }
    }
}
pub trait AxPyRepr {
    fn ax_py_repr(&self) -> String;
}
impl<T: Show + 'static> AxPyRepr for T {
    fn ax_py_repr(&self) -> String {
        show_item(self)
    }
}
fn show_item<T: Show + 'static>(x: &T) -> String {
    if let Some(r) = x.show_repr() {
        return r;
    }
    // Strings inside collections get quotes so [a, b] and ["a, b"] differ.
    let any = x as &dyn std::any::Any;
    match any.downcast_ref::<String>() {
        Some(s) if s == "\u{0}None" => "None".into(),
        Some(s) if ax_py_on() => ax_py_repr(s),
        Some(s) => format!("{s:?}"),
        // Python's repr of a Decimal inside a list or dict: Decimal('1.50').
        None if ax_py_on() && any.is::<Dec>() => match any.downcast_ref::<Dec>().filter(|d| d.is_frac()).map(|d| d.frac_parts()) {
            Some((n, d)) => format!("Fraction({n}, {d})"),
            None => format!("Decimal('{}')", x.show()),
        },
        None => match any.downcast_ref::<Json>() {
            Some(j) if ax_py_on() => ax_json_py(j, false),
            // A Python library value (see the pybridge runtime): its repr.
            _ => AX_SHOW_ITEM.get().and_then(|f| f(any)).unwrap_or_else(|| x.show()),
        },
    }
}
pub static AX_SHOW_ITEM: std::sync::OnceLock<fn(&dyn std::any::Any) -> Option<String>> = std::sync::OnceLock::new();
impl<T: Show + 'static> Show for Vec<T> {
    fn show(&self) -> String {
        format!("[{}]", self.iter().map(show_item).collect::<Vec<_>>().join(", "))
    }
}
impl<K: Hash + Eq + Ord + Show + 'static, V: Show + 'static> Show for AxM<K, V> {
    fn show(&self) -> String {
        if ax_py_on() {
            let parts: Vec<String> = self.iter().map(|(k, v)| format!("{}: {}", show_item(k), show_item(v))).collect();
            return format!("{{{}}}", parts.join(", "));
        }
        // Written like compact JSON, so str({"error": msg}) is ready to send.
        let parts: Vec<String> = self.iter().map(|(k, v)| format!("{}:{}", show_item(k), show_item(v))).collect();
        format!("{{{}}}", parts.join(","))
    }
}
thread_local! {
    // Tuple types that hold Python lists of mixed kinds (`[oid, sku, n]`): they print as lists.
    static AX_ROWS: std::cell::RefCell<std::collections::HashSet<std::any::TypeId>> = std::cell::RefCell::new(std::collections::HashSet::new());
}
pub fn ax_row<T: 'static>(t: T) -> AxRow<T> {
    let id = std::any::TypeId::of::<T>();
    AX_ROWS.with(|r| {
        if !r.borrow().contains(&id) {
            r.borrow_mut().insert(id);
        }
    });
    AxRow(Box::into_raw(Box::new(t)))
}
// A Python list of mixed kinds (`[oid, sku, n]`), shared as Python shares lists: a handle to a tuple that
// lives for the whole run. Copies of the handle (in other lists, names, results) see the same row.
pub struct AxRow<T>(*mut T);
impl<T> Clone for AxRow<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for AxRow<T> {}
impl<T: 'static> AxRow<T> {
    #[inline]
    pub fn ax_d(&self) -> &'static mut T {
        if self.0.is_null() {
            ax_none_attr()
        }
        unsafe { &mut *self.0 }
    }
}
impl<T: 'static> std::ops::Deref for AxRow<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.ax_d()
    }
}
impl<T: 'static> std::ops::DerefMut for AxRow<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.ax_d()
    }
}
impl<T: Default + 'static> Default for AxRow<T> {
    fn default() -> Self {
        ax_row(T::default())
    }
}
impl<T: PartialEq + 'static> PartialEq for AxRow<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0 || (!self.0.is_null() && !o.0.is_null() && **self == **o)
    }
}
impl<T: Eq + 'static> Eq for AxRow<T> {}
impl<T: PartialOrd + 'static> PartialOrd for AxRow<T> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(&**o)
    }
}
impl<T: Ord + 'static> Ord for AxRow<T> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (**self).cmp(&**o)
    }
}
impl<T: Hash + 'static> Hash for AxRow<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        (**self).hash(h)
    }
}
impl<T: std::fmt::Debug + 'static> std::fmt::Debug for AxRow<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.0.is_null() {
            return write!(f, "None");
        }
        (**self).fmt(f)
    }
}
impl<T: Show + 'static> Show for AxRow<T> {
    fn show(&self) -> String {
        if self.0.is_null() {
            return "None".into();
        }
        (**self).show()
    }
}
// A list is true unless it is None (a row is never empty).
impl<T> AxTruth for AxRow<T> {
    fn ax_truth(&self) -> bool {
        !self.0.is_null()
    }
}
impl<T> AxNone for AxRow<T> {
    fn ax_none() -> Self {
        AxRow(std::ptr::null_mut())
    }
    fn ax_is_none(&self) -> bool
    where
        Self: PartialEq + Sized,
    {
        self.0.is_null()
    }
}
impl<T: AxToJson + 'static> AxToJson for AxRow<T> {
    // A mixed Python list (kept as a tuple inside): a JSON list.
    fn ax_json(&self) -> Json {
        match (**self).ax_json() {
            Json::Tup { v } => Json::Arr { v },
            j => j,
        }
    }
}
impl<T: Clone + 'static, U> AxUnpack<U> for AxRow<T>
where
    T: AxUnpack<U>,
{
    fn ax_unpack(self) -> U {
        (*self).clone().ax_unpack()
    }
}
fn ax_wrap<T: 'static>(parts: &[String]) -> String {
    if ax_py_on() && AX_ROWS.with(|r| r.borrow().contains(&std::any::TypeId::of::<T>())) {
        return format!("[{}]", parts.join(", "));
    }
    format!("({})", parts.join(", "))
}
impl<A: Show + 'static, B: Show + 'static> Show for (A, B) {
    fn show(&self) -> String {
        let parts = [show_item(&self.0), show_item(&self.1)];
        // A tuple that is still None (each slot None) prints as Python's None.
        if ax_py_on() && parts.iter().all(|p| p == "None") {
            return "None".to_string();
        }
        ax_wrap::<Self>(&parts)
    }
}
impl<A: Show + 'static, B: Show + 'static, C: Show + 'static> Show for (A, B, C) {
    fn show(&self) -> String {
        let parts = [show_item(&self.0), show_item(&self.1), show_item(&self.2)];
        // A tuple that is still None (each slot None) prints as Python's None.
        if ax_py_on() && parts.iter().all(|p| p == "None") {
            return "None".to_string();
        }
        ax_wrap::<Self>(&parts)
    }
}
impl<A: Show + 'static, B: Show + 'static, C: Show + 'static, D: Show + 'static> Show for (A, B, C, D) {
    fn show(&self) -> String {
        let parts = [show_item(&self.0), show_item(&self.1), show_item(&self.2), show_item(&self.3)];
        // A tuple that is still None (each slot None) prints as Python's None.
        if ax_py_on() && parts.iter().all(|p| p == "None") {
            return "None".to_string();
        }
        ax_wrap::<Self>(&parts)
    }
}
impl<A: Show + 'static, B: Show + 'static, C: Show + 'static, D: Show + 'static, E: Show + 'static> Show for (A, B, C, D, E) {
    fn show(&self) -> String {
        let parts = [show_item(&self.0), show_item(&self.1), show_item(&self.2), show_item(&self.3), show_item(&self.4)];
        // A tuple that is still None (each slot None) prints as Python's None.
        if ax_py_on() && parts.iter().all(|p| p == "None") {
            return "None".to_string();
        }
        ax_wrap::<Self>(&parts)
    }
}

// A tuple of one type walked as a list (Python's `for i in (0, 1, 2)`).
pub trait AxTupleItems<T> {
    fn ax_tuple_items(&self) -> Vec<T>;
}
impl AxTupleItems<Json> for Json {
    fn ax_tuple_items(&self) -> Vec<Json> {
        self.ax_list()
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T, T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T, T, T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone(), self.6.clone()]
    }
}
impl<T: Clone> AxTupleItems<T> for (T, T, T, T, T, T, T, T) {
    fn ax_tuple_items(&self) -> Vec<T> {
        vec![self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone(), self.6.clone(), self.7.clone()]
    }
}
pub fn print<T: Show>(x: T) {
    #[cfg(windows)]
    {
        // CPython on Windows writes text streams with CRLF; match it, and translate every LF (even one
        // already followed by CR), as its TextIOWrapper does.
        use std::io::Write;
        let s = x.show().replace('\n', "\r\n");
        let mut o = std::io::stdout();
        let _ = o.write_all(s.as_bytes());
        let _ = o.write_all(b"\r\n");
        let _ = o.flush();
    }
    #[cfg(not(windows))]
    println!("{}", x.show());
}
// Python-style truth for conditions: false, 0, "", [] and {} are false.
pub trait AxTruth {
    fn ax_truth(&self) -> bool;
}
impl AxTruth for bool {
    #[inline]
    fn ax_truth(&self) -> bool {
        *self
    }
}
// Python's None (held as i64::MIN, f64::MIN or "\0None") is false too.
impl AxTruth for i64 {
    #[inline]
    fn ax_truth(&self) -> bool {
        *self != 0 && *self != i64::MIN
    }
}
impl AxTruth for f64 {
    #[inline]
    fn ax_truth(&self) -> bool {
        *self != 0.0 && *self != f64::MIN
    }
}
impl AxTruth for String {
    fn ax_truth(&self) -> bool {
        !self.is_empty() && self != "\u{0}None"
    }
}
// A tuple is true unless it is the None of a `best = None` that was never set (Python: any tuple is true).
impl<A: AxNone + PartialEq, B: AxNone + PartialEq> AxTruth for (A, B) {
    fn ax_truth(&self) -> bool {
        !(self.0.ax_is_none() && self.1.ax_is_none())
    }
}
impl<A: AxNone + PartialEq, B: AxNone + PartialEq, C: AxNone + PartialEq> AxTruth for (A, B, C) {
    fn ax_truth(&self) -> bool {
        !(self.0.ax_is_none() && self.1.ax_is_none() && self.2.ax_is_none())
    }
}
impl<A: AxNone + PartialEq, B: AxNone + PartialEq, C: AxNone + PartialEq, D: AxNone + PartialEq> AxTruth for (A, B, C, D) {
    fn ax_truth(&self) -> bool {
        !(self.0.ax_is_none() && self.1.ax_is_none() && self.2.ax_is_none() && self.3.ax_is_none())
    }
}
impl<A: AxNone + PartialEq, B: AxNone + PartialEq, C: AxNone + PartialEq, D: AxNone + PartialEq, E: AxNone + PartialEq> AxTruth for (A, B, C, D, E) {
    fn ax_truth(&self) -> bool {
        !(self.0.ax_is_none() && self.1.ax_is_none() && self.2.ax_is_none() && self.3.ax_is_none() && self.4.ax_is_none())
    }
}
impl<A: AxNone + PartialEq, B: AxNone + PartialEq, C: AxNone + PartialEq, D: AxNone + PartialEq, E: AxNone + PartialEq, F: AxNone + PartialEq> AxTruth for (A, B, C, D, E, F) {
    fn ax_truth(&self) -> bool {
        !(self.0.ax_is_none() && self.1.ax_is_none() && self.2.ax_is_none() && self.3.ax_is_none() && self.4.ax_is_none() && self.5.ax_is_none())
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
            Json::Num { v, .. } => *v != 0.0,
            Json::Str { v } => !v.is_empty(),
            Json::Arr { v } | Json::Tup { v } => !v.is_empty(),
            Json::Obj { v } => !v.m.is_empty(),
        }
    }
}
impl<T> AxTruth for AxHeap<T> {
    fn ax_truth(&self) -> bool {
        !self.0.is_empty()
    }
}
// A date is always true; only a date still None is false.
impl AxTruth for Date {
    fn ax_truth(&self) -> bool {
        self.days != i64::MIN
    }
}
impl AxTruth for Dec {
    fn ax_truth(&self) -> bool {
        self.nd().0 != 0
    }
}
#[inline]
pub fn ax_t<T: AxTruth + ?Sized>(x: &T) -> bool {
    x.ax_truth()
}
// The caller's file grants, set by ax_init from the program's AX_FS: (read held to roots, the roots, write held
// to roots, the roots). Unset (no ax_init yet) refuses every file.
pub type AxFs = (bool, &'static [&'static str], bool, &'static [&'static str]);
pub static AX_FS_REG: std::sync::OnceLock<AxFs> = std::sync::OnceLock::new();
pub fn ax_fs_path(path: &str, write: bool) -> R<String> {
    let Some(&(rs, rr, ws, wr)) = AX_FS_REG.get() else { return Err(AxErr(format!("{} {path}: brevi: no file access before the program starts", if write { "write" } else { "read" }))) };
    let (scoped, roots, cap) = if write { (ws, wr, "write") } else { (rs, rr, "read") };
    if !scoped {
        return Ok(path.to_string());
    }
    brevi_scoped(path, roots, cap).map_err(AxErr)
}
pub fn read(path: String) -> R<String> {
    let p = ax_fs_path(&path, false)?;
    std::fs::read_to_string(&p).map_err(|e| AxErr(format!("read {path}: {e}")))
}
pub fn write(path: String, text: String) -> R<()> {
    let p = ax_fs_path(&path, true)?;
    brevi_new_file(&p);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut f = opts.open(&p).map_err(|e| AxErr(format!("write {path}: {e}")))?;
    brevi_file_room(&p, 0, text.len() as u64);
    use std::io::Write;
    f.write_all(text.as_bytes()).map_err(|e| AxErr(format!("write {path}: {e}")))
}
// open(path).read() where a missing file reads as "" (the typed build's fix_opens).
pub fn ax_try_read(path: String) -> String {
    ax_fs_path(&path, false).ok().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default()
}
pub fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}
// Python's int("1_000") and float("1_0.5"): an underscore between two digits is dropped (a misplaced one
// is kept, so the parse fails as Python's does).
pub fn ax_py_digits(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('_') || !ax_py_on() {
        return std::borrow::Cow::Borrowed(s);
    }
    let cs: Vec<char> = s.chars().collect();
    let ok = cs.iter().enumerate().all(|(i, c)| *c != '_' || (i > 0 && i + 1 < cs.len() && cs[i - 1].is_ascii_alphanumeric() && cs[i + 1].is_ascii_alphanumeric()));
    if ok { std::borrow::Cow::Owned(s.replace('_', "")) } else { std::borrow::Cow::Borrowed(s) }
}
pub fn input() -> String {
    let mut s = String::new();
    // Python's input() at the end of the input raises EOFError.
    if matches!(std::io::stdin().read_line(&mut s), Ok(0)) && ax_py_on() {
        panic!("EOFError: EOF when reading a line");
    }
    s.trim_end_matches(['\n', '\r']).to_string()
}
pub fn stdin() -> String {
    use std::io::Read;
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).ok();
    s
}
pub fn ax_readline() -> String {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    s
}
pub fn lines() -> Vec<String> {
    stdin().lines().map(String::from).collect()
}
// a // b rounds down, as in Python.
pub trait AxFloorDiv<B = Self> {
    fn floordiv(self, b: B) -> Self;
}
impl AxFloorDiv for i64 {
    #[inline]
    fn floordiv(self, b: i64) -> i64 {
        let q = self / b;
        if (self % b != 0) && ((self < 0) != (b < 0)) { q - 1 } else { q }
    }
}
impl AxFloorDiv for f64 {
    #[inline]
    fn floordiv(self, b: f64) -> f64 {
        (self / b).floor()
    }
}
// Decimal a // b: the whole part of the quotient (toward zero, as Python's Decimal does; with a % b it gives a back).
impl AxFloorDiv for Dec {
    fn floordiv(self, b: Dec) -> Dec {
        // Fraction // floors (Python gives an int); Decimal truncates toward zero.
        if let Some(((a, b), (c, d))) = frac_pair(self, b) {
            return Dec(frac_floor_div(frac_mul(a, d), frac_mul(c, b)) * DEC_ONE, 0);
        }
        let q = self / b;
        Dec(q.0 / DEC_ONE * DEC_ONE, 0)
    }
}
impl AxFloorDiv<i64> for Dec {
    fn floordiv(self, b: i64) -> Dec {
        self.floordiv(dec_of(b))
    }
}
pub fn ax_floordiv<T: AxFloorDiv<B>, B>(a: T, b: B) -> T {
    a.floordiv(b)
}
// The same division, but a zero divisor is a ZeroDivisionError a Python try can catch.
pub trait AxFloorDivTry<B = Self>: Sized {
    fn ax_floordiv_try(self, b: B) -> R<Self>;
}
impl AxFloorDivTry for i64 {
    fn ax_floordiv_try(self, b: i64) -> R<i64> {
        if b == 0 {
            return Err(ax_zd_err("integer division or modulo by zero"));
        }
        Ok(self.floordiv(b))
    }
}
impl AxFloorDivTry for f64 {
    fn ax_floordiv_try(self, b: f64) -> R<f64> {
        if b == 0.0 {
            return Err(ax_zd_err("float floor division by zero"));
        }
        Ok(self.floordiv(b))
    }
}
// a % b in Python mode takes the sign of b, as Python's does (-7 % 3 is 2); Brevi's own % keeps Rust's.
pub trait AxPyMod<B = Self> {
    fn ax_pymod(self, b: B) -> Self;
}
// Decimal a % b: the sign follows a, as Python's Decimal does (a - b * trunc(a / b)).
impl AxPyMod for Dec {
    fn ax_pymod(self, b: Dec) -> Dec {
        if let Some(((a, b), (c, d))) = frac_pair(self, b) {
            // a/b - (c/d) * floor((a/b) / (c/d))
            let q = frac_floor_div(frac_mul(a, d), frac_mul(c, b));
            return frac_new(frac_add(frac_mul(a, d), -frac_mul(frac_mul(c, b), q)), frac_mul(b, d));
        }
        let q = self / b;
        let t = Dec(q.0 / DEC_ONE * DEC_ONE, 0);
        self - b * t
    }
}
impl AxPyMod<i64> for Dec {
    fn ax_pymod(self, b: i64) -> Dec {
        self.ax_pymod(dec_of(b))
    }
}
impl AxPyMod for i64 {
    #[inline]
    fn ax_pymod(self, b: i64) -> i64 {
        let r = self % b;
        if ax_py_on() && r != 0 && ((r < 0) != (b < 0)) { r + b } else { r }
    }
}
impl AxPyMod for f64 {
    #[inline]
    fn ax_pymod(self, b: f64) -> f64 {
        let r = self % b;
        if !ax_py_on() {
            return r;
        }
        if b == 0.0 {
            panic!("ZeroDivisionError: float modulo");
        }
        // A zero remainder takes b's sign too: -0.0 % 5.0 is 0.0 and 0.0 % -2.0 is -0.0.
        if r == 0.0 {
            return 0.0f64.copysign(b);
        }
        if (r < 0.0) != (b < 0.0) { r + b } else { r }
    }
}
// The same `%`, but a zero divisor is a ZeroDivisionError a Python try can catch.
pub trait AxPyModTry<B = Self>: Sized {
    fn ax_pymod_try(self, b: B) -> R<Self>;
}
impl AxPyModTry for i64 {
    fn ax_pymod_try(self, b: i64) -> R<i64> {
        if b == 0 {
            return Err(ax_zd_err("integer modulo by zero"));
        }
        Ok(self.ax_pymod(b))
    }
}
impl AxPyModTry for f64 {
    fn ax_pymod_try(self, b: f64) -> R<f64> {
        if b == 0.0 {
            return Err(ax_zd_err("float modulo by zero"));
        }
        Ok(self.ax_pymod(b))
    }
}
// A Python list mixed with numbers (its items are Json) divides the same way: a zero divisor is catchable.
impl AxFloorDivTry for Json {
    fn ax_floordiv_try(self, b: Json) -> R<Json> {
        let f = ax_jf(&self) || ax_jf(&b);
        if ax_jnum(&b) == 0.0 {
            return Err(ax_zd_err(if f { "float floor division by zero" } else { "integer division or modulo by zero" }));
        }
        Ok(ax_jnk((ax_jnum(&self) / ax_jnum(&b)).floor(), f))
    }
}
impl AxPyModTry for Json {
    fn ax_pymod_try(self, b: Json) -> R<Json> {
        let f = ax_jf(&self) || ax_jf(&b);
        let (a, b) = (ax_jnum(&self), ax_jnum(&b));
        if b == 0.0 {
            return Err(ax_zd_err(if f { "float modulo by zero" } else { "integer modulo by zero" }));
        }
        Ok(if f { ax_jnk(a.ax_pymod(b), true) } else { ax_jnk(a - b * (a / b).floor(), false) })
    }
}
impl AxPyMod for Json {
    fn ax_pymod(self, b: Json) -> Json {
        self % b
    }
}
// Python's math functions (math.log(x) is x.ax_m_log()), on floats and ints; a domain error is Python's
// ValueError, as math raises it (Rust would give NaN or an infinity).
// bin(x), hex(x), oct(x): Python's prefixed text, a minus sign first for negatives.
pub trait AxRadix {
    fn ax_bin_text(&self) -> String;
    fn ax_hex_text(&self) -> String;
    fn ax_oct_text(&self) -> String;
}
impl AxRadix for i64 {
    fn ax_bin_text(&self) -> String {
        if *self < 0 { format!("-0b{:b}", self.unsigned_abs()) } else { format!("0b{:b}", self) }
    }
    fn ax_hex_text(&self) -> String {
        if *self < 0 { format!("-0x{:x}", self.unsigned_abs()) } else { format!("0x{:x}", self) }
    }
    fn ax_oct_text(&self) -> String {
        if *self < 0 { format!("-0o{:o}", self.unsigned_abs()) } else { format!("0o{:o}", self) }
    }
}
impl AxRadix for bool {
    fn ax_bin_text(&self) -> String {
        (*self as i64).ax_bin_text()
    }
    fn ax_hex_text(&self) -> String {
        (*self as i64).ax_hex_text()
    }
    fn ax_oct_text(&self) -> String {
        (*self as i64).ax_oct_text()
    }
}
// s.startswith(p, start) and s.endswith(p, end_from): as s[start:].startswith(p) (start may be negative).
pub trait AxStrAt {
    fn ax_starts_with_at(&self, p: String, start: i64) -> bool;
    fn ax_ends_with_at(&self, p: String, start: i64) -> bool;
}
impl AxStrAt for String {
    fn ax_starts_with_at(&self, p: String, start: i64) -> bool {
        let n = self.chars().count() as i64;
        if start > n {
            return false;
        }
        let st = if start < 0 { (n + start).max(0) } else { start } as usize;
        let off: usize = self.char_indices().nth(st).map_or(self.len(), |(b, _)| b);
        self[off..].starts_with(p.as_str())
    }
    fn ax_ends_with_at(&self, p: String, start: i64) -> bool {
        let n = self.chars().count() as i64;
        if start > n {
            return false;
        }
        let st = if start < 0 { (n + start).max(0) } else { start } as usize;
        let off: usize = self.char_indices().nth(st).map_or(self.len(), |(b, _)| b);
        self[off..].ends_with(p.as_str())
    }
}
// csv.writer(sys.stdout, ...) in the typed build is its delimiter followed by its line ending; writerow
// prints one row as CPython's csv module writes it (QUOTE_MINIMAL: a field holding the delimiter, a quote,
// \r, \n or a line-ending character is quoted, quotes doubled; None is empty; a row of one empty field is "").
pub trait AxCsvW {
    fn ax_csv_writerow<T: Show>(&self, row: Vec<T>);
    fn ax_csv_writerows<T: Show>(&self, rows: Vec<Vec<T>>);
}
fn ax_csv_line<T: Show>(w: &str, row: Vec<T>) -> String {
    let mut cs = w.chars();
    let d = cs.next().unwrap_or(',');
    let term: String = cs.collect();
    let n = row.len();
    let mut out = String::new();
    for (i, x) in row.into_iter().enumerate() {
        if i > 0 {
            out.push(d);
        }
        let mut t = x.show();
        if t == "\u{0}None" {
            t = String::new();
        }
        if (n == 1 && t.is_empty()) || t.chars().any(|c| c == d || c == '"' || c == '\r' || c == '\n' || term.contains(c)) {
            out.push('"');
            out.push_str(&t.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&t);
        }
    }
    out.push_str(&term);
    out
}
impl AxCsvW for String {
    fn ax_csv_writerow<T: Show>(&self, row: Vec<T>) {
        print!("{}", ax_csv_line(self, row));
    }
    fn ax_csv_writerows<T: Show>(&self, rows: Vec<Vec<T>>) {
        for r in rows {
            self.ax_csv_writerow(r);
        }
    }
}
pub trait AxMath {
    fn ax_f(&self) -> f64;
    fn ax_m_log(&self) -> f64 {
        let x = self.ax_f();
        if !(x > 0.0) && !x.is_nan() {
            panic!("ValueError: math domain error");
        }
        x.ln()
    }
    fn ax_m_logb<B: AxMath>(&self, b: B) -> f64 {
        let d = b.ax_m_log();
        if d == 0.0 {
            panic!("ZeroDivisionError: float division by zero");
        }
        self.ax_m_log() / d
    }
    fn ax_m_log2(&self) -> f64 {
        self.ax_m_log();
        self.ax_f().log2()
    }
    fn ax_m_log10(&self) -> f64 {
        self.ax_m_log();
        self.ax_f().log10()
    }
    fn ax_m_log1p(&self) -> f64 {
        let x = self.ax_f();
        if x <= -1.0 {
            panic!("ValueError: math domain error");
        }
        x.ln_1p()
    }
    fn ax_m_exp(&self) -> f64 {
        let r = self.ax_f().exp();
        if r.is_infinite() && self.ax_f().is_finite() {
            panic!("OverflowError: math range error");
        }
        r
    }
    fn ax_m_expm1(&self) -> f64 {
        let r = self.ax_f().exp_m1();
        if r.is_infinite() && self.ax_f().is_finite() {
            panic!("OverflowError: math range error");
        }
        r
    }
    fn ax_m_trig(&self) -> f64 {
        let x = self.ax_f();
        if x.is_infinite() {
            panic!("ValueError: math domain error");
        }
        x
    }
    fn ax_m_sin(&self) -> f64 {
        self.ax_m_trig().sin()
    }
    fn ax_m_cos(&self) -> f64 {
        self.ax_m_trig().cos()
    }
    fn ax_m_tan(&self) -> f64 {
        self.ax_m_trig().tan()
    }
    fn ax_m_asin(&self) -> f64 {
        let x = self.ax_f();
        if x.abs() > 1.0 {
            panic!("ValueError: math domain error");
        }
        x.asin()
    }
    fn ax_m_acos(&self) -> f64 {
        let x = self.ax_f();
        if x.abs() > 1.0 {
            panic!("ValueError: math domain error");
        }
        x.acos()
    }
    fn ax_m_atan(&self) -> f64 {
        self.ax_f().atan()
    }
    fn ax_m_atan2<B: AxMath>(&self, x: B) -> f64 {
        self.ax_f().atan2(x.ax_f())
    }
    fn ax_m_sinh(&self) -> f64 {
        self.ax_f().sinh()
    }
    fn ax_m_cosh(&self) -> f64 {
        self.ax_f().cosh()
    }
    fn ax_m_tanh(&self) -> f64 {
        self.ax_f().tanh()
    }
    fn ax_m_hypot<B: AxMath>(&self, y: B) -> f64 {
        self.ax_f().hypot(y.ax_f())
    }
    fn ax_m_fabs(&self) -> f64 {
        self.ax_f().abs()
    }
    fn ax_m_trunc(&self) -> i64 {
        let x = self.ax_f();
        if !x.is_finite() {
            panic!("{}", if x.is_nan() { "ValueError: cannot convert float NaN to integer" } else { "OverflowError: cannot convert float infinity to integer" });
        }
        x.trunc() as i64
    }
    fn ax_m_degrees(&self) -> f64 {
        self.ax_f().to_degrees()
    }
    fn ax_m_radians(&self) -> f64 {
        self.ax_f().to_radians()
    }
    fn ax_m_copysign<B: AxMath>(&self, y: B) -> f64 {
        self.ax_f().copysign(y.ax_f())
    }
    fn ax_m_fmod<B: AxMath>(&self, y: B) -> f64 {
        let (x, y) = (self.ax_f(), y.ax_f());
        if y == 0.0 || x.is_infinite() {
            panic!("ValueError: math domain error");
        }
        x % y
    }
    fn ax_m_isnan(&self) -> bool {
        self.ax_f().is_nan()
    }
    fn ax_m_isinf(&self) -> bool {
        self.ax_f().is_infinite()
    }
    fn ax_m_isfinite(&self) -> bool {
        self.ax_f().is_finite()
    }
    fn ax_m_inf(&self) -> f64 {
        f64::INFINITY
    }
    fn ax_m_nan(&self) -> f64 {
        f64::NAN
    }
    fn ax_m_time(&self) -> f64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
    }
    fn ax_m_perf_counter(&self) -> f64 {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
    }
    fn ax_m_monotonic(&self) -> f64 {
        self.ax_m_perf_counter()
    }
}
impl AxMath for f64 {
    fn ax_f(&self) -> f64 {
        *self
    }
}
impl AxMath for i64 {
    fn ax_f(&self) -> f64 {
        *self as f64
    }
}
impl AxMath for bool {
    fn ax_f(&self) -> f64 {
        *self as i64 as f64
    }
}
impl AxMath for Json {
    fn ax_f(&self) -> f64 {
        match self {
            Json::Num { v, .. } => *v,
            Json::Bool { v } => *v as i64 as f64,
            _ => panic!("TypeError: must be real number"),
        }
    }
}
// Python's math.isqrt(n): the largest whole number whose square is at most n.
pub trait AxIsqrt {
    fn ax_isqrt(&self) -> i64;
}
impl AxIsqrt for i64 {
    fn ax_isqrt(&self) -> i64 {
        if *self < 0 {
            panic!("ValueError: isqrt() argument must be nonnegative");
        }
        let mut r = (*self as f64).sqrt() as i64;
        while r * r > *self {
            r -= 1;
        }
        while (r + 1) * (r + 1) <= *self {
            r += 1;
        }
        r
    }
}
// The step of a range counted through directly (range(a, b, k) with k > 0).
pub fn ax_step_by(k: i64) -> usize {
    if k <= 0 {
        panic!("ValueError: range() step must be positive here, got {k}");
    }
    k as usize
}
// A Python set as print shows it.
pub fn ax_show_set<T: Show + 'static>(xs: &Vec<T>) -> String {
    if xs.is_empty() {
        return "set()".into();
    }
    format!("{{{}}}", xs.iter().map(show_item).collect::<Vec<_>>().join(", "))
}
// Python's int(s, base): sign, then digits in that base (a 0x/0o/0b prefix allowed for 16/8/2, and needed
// for base 0), underscores between digits, spaces around.
pub trait AxIntBase {
    fn ax_int_base(&self, base: i64) -> R<i64>;
}
impl AxIntBase for String {
    fn ax_int_base(&self, base: i64) -> R<i64> {
        let bad = || AxErr(format!("invalid literal for int() with base {base}: {}", ax_py_repr(self)));
        let t = self.trim();
        let (neg, t) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let low = t.to_ascii_lowercase();
        let pre = |p: &str| low.starts_with(p);
        let (b, digits) = match base {
            0 if pre("0x") => (16, &t[2..]),
            0 if pre("0o") => (8, &t[2..]),
            0 if pre("0b") => (2, &t[2..]),
            0 => (10, t),
            16 if pre("0x") => (16, &t[2..]),
            8 if pre("0o") => (8, &t[2..]),
            2 if pre("0b") => (2, &t[2..]),
            2..=36 => (base as u32, t),
            _ => return Err(AxErr("int() base must be >= 2 and <= 36, or 0".into())),
        };
        let digits = digits.strip_prefix('_').filter(|_| digits.len() < t.len()).unwrap_or(digits);
        if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
            return Err(bad());
        }
        // base 10 takes any Unicode decimal digit (`int("१२३", 0)` is 123)
        let digits: String = digits.chars().filter(|&c| c != '_').map(|c| if b == 10 { ax_uni_digit(c) } else { c }).collect();
        // base 0 reads a decimal literal as Python source does: no leading zeros unless the value is 0
        if base == 0 && b == 10 && digits.len() > 1 && digits.starts_with('0') && digits.bytes().any(|c| c != b'0') {
            return Err(bad());
        }
        let v = i64::from_str_radix(&digits, b).map_err(|_| bad())?;
        Ok(if neg { -v } else { v })
    }
}
/// A Unicode decimal digit (category Nd) as its ASCII digit; any other char unchanged.
pub fn ax_uni_digit(c: char) -> char {
    const ZEROS: &[u32] = &[
        0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66, 0xde6, 0xe50, 0xed0, 0xf20, 0x1040,
        0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10, 0x104a0, 0x10d30, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0, 0x11650,
        0x116c0, 0x11730, 0x118e0, 0x11950, 0x11c50, 0x11d50, 0x11da0, 0x11f50, 0x16a60, 0x16ac0, 0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2,
        0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e950, 0x1fbf0,
    ];
    if c.is_ascii() {
        return c;
    }
    let u = c as u32;
    match ZEROS.iter().find(|&&z| u >= z && u < z + 10) {
        Some(z) => char::from_digit(u - z, 10).unwrap(),
        None => c,
    }
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
    #[inline]
    fn plus(self, o: i64) -> i64 {
        self + o
    }
}
impl Plus for f64 {
    type Out = f64;
    #[inline]
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

#[derive(Clone, Copy)]
pub struct AxPfSpec {
    fill: char,
    align: Option<char>,
    sign: char,
    alt: bool,
    zero: bool,
    width: usize,
    group: Option<char>,
    prec: Option<usize>,
    ty: Option<char>,
}
fn ax_pf_parse(spec: &str) -> AxPfSpec {
    let c: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut sp = AxPfSpec { fill: ' ', align: None, sign: '-', alt: false, zero: false, width: 0, group: None, prec: None, ty: None };
    if c.len() >= 2 && matches!(c[1], '<' | '>' | '^' | '=') {
        sp.fill = c[0];
        sp.align = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && matches!(c[0], '<' | '>' | '^' | '=') {
        sp.align = Some(c[0]);
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        sp.sign = c[i];
        i += 1;
    }
    if i < c.len() && c[i] == 'z' {
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        sp.alt = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        sp.zero = true;
        i += 1;
    }
    let st = i;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i > st {
        sp.width = c[st..i].iter().collect::<String>().parse().unwrap_or(0);
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        sp.group = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == st {
            panic!("ValueError: Format specifier missing precision");
        }
        sp.prec = Some(c[st..i].iter().collect::<String>().parse().unwrap_or(0));
    }
    if i < c.len() {
        sp.ty = Some(c[i]);
        i += 1;
    }
    if i < c.len() {
        panic!("ValueError: Invalid format specifier '{spec}' for object");
    }
    sp
}
fn ax_pf_group_digits(int_part: &str, sep: char, every: usize) -> String {
    let n = int_part.len();
    let mut out = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (n - i) % every == 0 {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}
fn ax_pf_pad(body: String, sign: &str, sp: &AxPfSpec, default_right: bool) -> String {
    let len = body.chars().count() + sign.chars().count();
    if len >= sp.width {
        return format!("{sign}{body}");
    }
    let n = sp.width - len;
    let (fill, align) = if sp.zero && sp.align.is_none() { ('0', '=') } else { (sp.fill, sp.align.unwrap_or(if default_right { '>' } else { '<' })) };
    let f = |k: usize| std::iter::repeat(fill).take(k).collect::<String>();
    match align {
        '<' => format!("{sign}{body}{}", f(n)),
        '>' => format!("{}{sign}{body}", f(n)),
        '^' => format!("{}{sign}{body}{}", f(n / 2), f(n - n / 2)),
        _ => format!("{sign}{}{body}", f(n)),
    }
}
// Fixed/exponent digits for a float with Python's rules.
fn ax_pf_float_body(x: f64, ty: char, prec: Option<usize>, alt: bool) -> String {
    if x.is_nan() {
        return if ty.is_ascii_uppercase() { "NAN".into() } else { "nan".into() };
    }
    if x.is_infinite() {
        return if ty.is_ascii_uppercase() { "INF".into() } else { "inf".into() };
    }
    match ty {
        'f' | 'F' => {
            let p = prec.unwrap_or(6);
            let mut s = format!("{:.*}", p, x.abs());
            if alt && p == 0 {
                s.push('.');
            }
            s
        }
        'e' | 'E' => {
            let p = prec.unwrap_or(6);
            let s = format!("{:.*e}", p, x.abs());
            let (m, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let mut m = m.to_string();
            if alt && p == 0 {
                m.push('.');
            }
            let r = format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs());
            if ty == 'E' {
                r.to_uppercase()
            } else {
                r
            }
        }
        '%' => {
            let p = prec.unwrap_or(6);
            format!("{:.*}%", p, x.abs() * 100.0)
        }
        'g' | 'G' | 'n' => {
            let p = prec.unwrap_or(6).max(1);
            if x == 0.0 {
                return if alt { format!("{:.*}", p - 1, 0.0) } else { "0".into() };
            }
            let s = format!("{:.*e}", p - 1, x.abs());
            let (_, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let r = if e >= -4 && e < p as i32 {
                let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                if !alt && s.contains('.') {
                    s = s.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                s
            } else {
                let (m, _) = s.split_once('e').unwrap();
                let mut m = m.to_string();
                if !alt && m.contains('.') {
                    m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
            };
            if ty == 'G' {
                r.to_uppercase()
            } else {
                r
            }
        }
        _ => {
            // no type: repr-like, or 'g'-like with at least one decimal when a precision is given
            match prec {
                None => ax_py_float(x.abs()),
                Some(p) => {
                    let p = p.max(1);
                    let s = format!("{:.*e}", p - 1, x.abs());
                    let (_, e) = s.split_once('e').unwrap();
                    let e: i32 = e.parse().unwrap();
                    if e >= -4 && e < p as i32 {
                        let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                        if s.contains('.') {
                            s = s.trim_end_matches('0').to_string();
                            if s.ends_with('.') {
                                s.push('0');
                            }
                        } else {
                            s.push_str(".0");
                        }
                        s
                    } else {
                        let (m, _) = s.split_once('e').unwrap();
                        let mut m = m.to_string();
                        if m.contains('.') {
                            m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                        }
                        format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
                    }
                }
            }
        }
    }
}
fn ax_pf_sign(neg: bool, sp: &AxPfSpec) -> &'static str {
    if neg {
        "-"
    } else {
        match sp.sign {
            '+' => "+",
            ' ' => " ",
            _ => "",
        }
    }
}
fn ax_pf_group_num(body: &str, sp: &AxPfSpec) -> String {
    match sp.group {
        None => body.to_string(),
        Some(g) => {
            let (ip, rest) = match body.find(|c: char| !c.is_ascii_digit()) {
                Some(i) => (&body[..i], &body[i..]),
                None => (body, ""),
            };
            format!("{}{}", ax_pf_group_digits(ip, g, 3), rest)
        }
    }
}

// Python's format(v, spec) for the typed build's values, as CPython writes them.
pub fn ax_pf_str(x: &str, spec: &str) -> String {
    let sp = ax_pf_parse(spec);
    if let Some(t) = sp.ty {
        if t != 's' {
            panic!("ValueError: Unknown format code '{t}' for object of type 'str'");
        }
    }
    if sp.sign != '-' {
        panic!("ValueError: Sign not allowed in string format specifier");
    }
    if sp.align == Some('=') {
        panic!("ValueError: '=' alignment not allowed in string format specifier");
    }
    let body: String = match sp.prec {
        Some(p) => x.chars().take(p).collect(),
        None => x.to_string(),
    };
    ax_pf_pad(body, "", &sp, false)
}
pub fn ax_pf_int(v: i64, spec: &str) -> String {
    // `{i:06d}`, `{i:5}`, `{i:d}`: plain widths, written straight away.
    let b = spec.as_bytes();
    let digits = b.strip_suffix(b"d").unwrap_or(b);
    if digits.len() <= 3 && digits.iter().all(|c| c.is_ascii_digit()) {
        let zero = digits.first() == Some(&b'0');
        let w: usize = std::str::from_utf8(digits).unwrap().parse().unwrap_or(0);
        return if zero { format!("{v:0w$}") } else { format!("{v:>w$}") };
    }
    let sp = ax_pf_parse(spec);
    if matches!(sp.ty, Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%')) {
        return ax_pf_float(v as f64, spec, true);
    }
    if sp.prec.is_some() {
        panic!("ValueError: Precision not allowed in integer format specifier");
    }
    let neg = v < 0;
    let a = v.unsigned_abs();
    let (body, prefix) = match sp.ty {
        None | Some('d') | Some('n') => (a.to_string(), ""),
        Some('x') => (format!("{a:x}"), "0x"),
        Some('X') => (format!("{a:X}"), "0X"),
        Some('o') => (format!("{a:o}"), "0o"),
        Some('b') => (format!("{a:b}"), "0b"),
        Some('c') => (char::from_u32(v as u32).unwrap_or('?').to_string(), ""),
        Some(t) => panic!("ValueError: Unknown format code '{t}' for object of type 'int'"),
    };
    let body = match sp.group {
        Some(g) => ax_pf_group_digits(&body, g, if matches!(sp.ty, Some('x' | 'X' | 'o' | 'b')) { 4 } else { 3 }),
        None => body,
    };
    let sign = format!("{}{}", ax_pf_sign(neg, &sp), if sp.alt { prefix } else { "" });
    if sp.zero && sp.align.is_none() && sp.group.is_some() && body.len() + sign.len() < sp.width {
        let digits: String = body.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let g = sp.group.unwrap();
        let mut d = digits;
        loop {
            let t = ax_pf_group_digits(&d, g, 3);
            if t.len() + sign.len() >= sp.width {
                return format!("{sign}{t}");
            }
            d.insert(0, '0');
        }
    }
    ax_pf_pad(body, &sign, &sp, true)
}
pub fn ax_pf_float(x: f64, spec: &str, from_int: bool) -> String {
    let sp = ax_pf_parse(spec);
    if let Some(t) = sp.ty {
        if !matches!(t, 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | 'n' | '%') {
            panic!("ValueError: Unknown format code '{t}' for object of type 'float'");
        }
    }
    let neg = x.is_sign_negative() && !x.is_nan() && !(x == 0.0 && from_int);
    let mut body = ax_pf_float_body(x, sp.ty.unwrap_or('\0'), sp.prec, sp.alt);
    body = ax_pf_group_num(&body, &sp);
    let sign = ax_pf_sign(neg, &sp);
    if sp.zero && sp.align.is_none() && sp.group.is_some() {
        let mut b = body.clone();
        while b.len() + sign.len() < sp.width {
            let ip: String = b.split(|c: char| c == '.' || c == 'e' || c == '%').next().unwrap().chars().filter(|c| c.is_ascii_digit()).collect();
            let rest = b[b.find(|c: char| c == '.' || c == 'e' || c == '%').unwrap_or(b.len())..].to_string();
            let t = ax_pf_group_digits(&format!("0{ip}"), sp.group.unwrap(), 3);
            b = format!("{t}{rest}");
        }
        return format!("{sign}{b}");
    }
    ax_pf_pad(body, sign, &sp, true)
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
        if ax_py_on() {
            return ax_pf_int(*self, &spec);
        }
        ax_spec_num(*self as f64, Some(*self), &spec)
    }
}
impl AxSpec for f64 {
    fn ax_spec(&self, spec: String) -> String {
        if ax_py_on() {
            return ax_pf_float(*self, &spec, false);
        }
        ax_spec_num(*self, None, &spec)
    }
}
impl AxSpec for Dec {
    fn ax_spec(&self, spec: String) -> String {
        let (_, plus, _, _, comma, prec, _) = ax_spec_parts(&spec);
        let mut body = match prec {
            Some(p) => self.ax_fmt(p as i64),
            None => self.show(),
        };
        if comma {
            body = ax_commas(&body);
        }
        if plus && !body.starts_with('-') {
            body = format!("+{body}");
        }
        ax_spec_pad(body, &spec, true)
    }
}
impl AxSpec for String {
    fn ax_spec(&self, spec: String) -> String {
        if ax_py_on() {
            return ax_pf_str(self, &spec);
        }
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
        if ax_py_on() {
            return if spec.is_empty() { self.show() } else { ax_pf_int(*self as i64, &spec) };
        }
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
        // Python's round(x) goes to the even neighbour on a tie: round(2.5) is 2.
        if ax_py_on() { self.round_ties_even() } else { self.round() }
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
        // A one-byte separator: a byte search, not the general substring search.
        if sep.len() == 1 {
            return self.split(sep.as_bytes()[0] as char).map(String::from).collect();
        }
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
        let t = self.trim();
        let t: std::borrow::Cow<str> = if t.is_ascii() { ax_py_digits(t) } else { std::borrow::Cow::Owned(t.chars().map(ax_uni_digit).collect()) };
        t.parse::<i64>().map_err(|_| {
            if ax_py_on() {
                AxErr(format!("invalid literal for int() with base 10: {}", ax_py_repr(self)))
            } else {
                AxErr(format!("not an int: {self:?}"))
            }
        })
    }
    fn ax_float(&self) -> R<f64> {
        ax_py_digits(self.trim()).parse::<f64>().map_err(|_| if ax_py_on() { AxErr(format!("could not convert string to float: {}", ax_py_repr(self))) } else { AxErr(format!("not a float: {self:?}")) })
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

#[inline]
fn ax_idx(i: i64, len: usize) -> usize {
    // Negative indexes count from the end: xs[-1] is the last item.
    let j = if i < 0 { len as i64 + i } else { i };
    if j < 0 || j as usize >= len {
        panic!("index {i} out of range for length {len}");
    }
    j as usize
}
// Python's IndexError and ZeroDivisionError as catchable errors (a Rust panic is not one), with the class
// chain `except A, B:` matching uses and the message CPython prints.
#[inline]
pub fn ax_idx_err(msg: &str) -> AxErr {
    AxErr(ax_exc("IndexError,LookupError,Exception,BaseException".into(), msg.into()))
}
#[inline]
pub fn ax_zd_err(msg: &str) -> AxErr {
    AxErr(ax_exc("ZeroDivisionError,ArithmeticError,Exception,BaseException".into(), msg.into()))
}
#[inline]
pub fn ax_checked_idx(i: i64, len: usize, msg: &str) -> R<usize> {
    let j = if i < 0 { len as i64 + i } else { i };
    if j < 0 || j as usize >= len {
        return Err(ax_idx_err(msg));
    }
    Ok(j as usize)
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
    fn ax_any(&self, f: impl FnMut(T) -> bool) -> bool;
    fn ax_all(&self, f: impl FnMut(T) -> bool) -> bool;
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
    fn ax_any(&self, mut f: impl FnMut(T) -> bool) -> bool {
        self.iter().cloned().any(f)
    }
    fn ax_all(&self, mut f: impl FnMut(T) -> bool) -> bool {
        self.iter().cloned().all(f)
    }
    fn ax_count(&self, f: impl Fn(T) -> bool) -> i64 {
        self.iter().cloned().filter(|x| f(x.clone())).count() as i64
    }
    fn ax_fold<A>(&self, init: A, f: impl Fn(A, T) -> A) -> A {
        self.iter().cloned().fold(init, f)
    }
    // The key is worked out once per item, as in Python, not twice per comparison.
    fn ax_sort_by<K: PartialOrd>(&self, f: impl Fn(T) -> K) -> Vec<T> {
        self.ax_sort_by_rev(f, false)
    }
    fn ax_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(T) -> K, rev: bool) -> Vec<T> {
        let mut v: Vec<(K, usize)> = self.iter().enumerate().map(|(i, x)| (f(x.clone()), i)).collect();
        v.sort_by(|(x, _), (y, _)| {
            let o = if rev { y.partial_cmp(x) } else { x.partial_cmp(y) };
            o.unwrap_or(std::cmp::Ordering::Equal)
        });
        v.into_iter().map(|(_, i)| self[i].clone()).collect()
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
// CPython 3.12+ sums floats with compensation (Neumaier), and so does bi_sum in the dynamic build.
fn ax_fsum(start: f64, xs: impl Iterator<Item = f64>) -> f64 {
    let (mut f, mut c) = (start, 0.0f64);
    for x in xs {
        let t = f + x;
        c += if f.abs() >= x.abs() { (f - t) + x } else { (x - t) + f };
        f = t;
    }
    // Adding the correction to an overflowed sum would make it nan.
    if c != 0.0 && c.is_finite() {
        f += c;
    }
    f
}
impl AxSum<f64> for Vec<f64> {
    fn ax_sum(&self) -> f64 {
        ax_fsum(0.0, self.iter().copied())
    }
}
// sum(xs, start) of floats: the start is the sum's first term.
pub trait AxSumFrom {
    fn ax_sum_from(&self, start: f64) -> f64;
}
impl AxSumFrom for Vec<f64> {
    fn ax_sum_from(&self, start: f64) -> f64 {
        ax_fsum(start, self.iter().copied())
    }
}
// Numbers held as JSON (a function returning `v` or `0`): their sum, as JSON.
impl AxSum<Json> for Vec<Json> {
    fn ax_sum(&self) -> Json {
        Json::Num { v: ax_fsum(0.0, self.iter().map(|j| if let Json::Num { v, .. } = j { *v } else { 0.0 })), f: self.iter().any(|j| matches!(j, Json::Num { f: true, .. })) }
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

// d.get(k, {}) and d.get(k, []) on a map: the value, else an empty one of its type.
pub trait AxGetEmpty<K, V> {
    fn ax_get_or_emap(&self, k: &K) -> V;
    fn ax_get_or_elist(&self, k: &K) -> V;
}
impl<K: Hash + Eq + Ord + Clone, V: Clone + Default> AxGetEmpty<K, V> for AxM<K, V> {
    fn ax_get_or_emap(&self, k: &K) -> V {
        self.get(k).cloned().unwrap_or_default()
    }
    fn ax_get_or_elist(&self, k: &K) -> V {
        self.get(k).cloned().unwrap_or_default()
    }
}
// A key to look up in a map keyed by K: a K, or (in a map keyed by str) a JSON value, which matches no key
// unless it is a str.
pub trait AxKeyOf<K: Clone> {
    fn ax_key_of(&self) -> Option<std::borrow::Cow<'_, K>>;
}
impl<K: Clone> AxKeyOf<K> for K {
    #[inline]
    fn ax_key_of(&self) -> Option<std::borrow::Cow<'_, K>> {
        Some(std::borrow::Cow::Borrowed(self))
    }
}
impl AxKeyOf<String> for Json {
    fn ax_key_of(&self) -> Option<std::borrow::Cow<'_, String>> {
        match self {
            Json::Str { v } => Some(std::borrow::Cow::Borrowed(v)),
            _ => None,
        }
    }
}
pub trait AxMap<K, V> {
    fn ax_len(&self) -> i64;
    fn ax_is_empty(&self) -> bool;
    fn ax_has(&self, k: &K) -> bool;
    fn ax_get_or<Q: AxKeyOf<K> + ?Sized>(&self, k: &Q, d: V) -> V
    where
        K: Clone;
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
    fn ax_get_or<Q: AxKeyOf<K> + ?Sized>(&self, k: &Q, d: V) -> V {
        match k.ax_key_of() {
            Some(k) => self.get(&*k).cloned().unwrap_or(d),
            None => d,
        }
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
            // A plain Python dict raises KeyError (Brevi's own maps and defaultdicts read zero).
            None if ax_py_on() && !self.lax => panic!("KeyError: {}", show_item(k)),
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
// Is s all ASCII (so its i-th character is its i-th byte)? A long text is scanned once: the answer is kept
// for its block of memory until that block is freed or moved, so `s[i]` in a loop stays constant time.
#[inline]
pub fn ax_ascii(s: &String) -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    if s.len() <= 256 {
        return s.is_ascii();
    }
    let key = [s.as_ptr() as usize, s.len(), s.capacity()];
    if (0..3).all(|k| AX_ASCII[k].load(Relaxed) == key[k]) {
        return true;
    }
    let a = s.is_ascii();
    if a {
        AX_ASCII[0].store(0, Relaxed);
        AX_ASCII[1].store(key[1], Relaxed);
        AX_ASCII[2].store(key[2], Relaxed);
        AX_ASCII[0].store(key[0], Relaxed);
    }
    a
}
#[inline]
pub fn ax_char_at(s: &String, i: i64) -> char {
    if ax_ascii(s) {
        s.as_bytes()[ax_idx(i, s.len())] as char
    } else {
        let n = s.chars().count();
        s.chars().nth(ax_idx(i, n)).unwrap()
    }
}
impl AxRef<i64, String> for String {
    fn ax_ref(&self, i: &i64) -> &String {
        // One-character strings live in a table (ASCII) or are kept for the program's life.
        static ASCII: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let c = ax_char_at(self, *i);
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
            Json::Arr { v } | Json::Tup { v } => {
                let j = if *i < 0 { v.len() as i64 + *i } else { *i };
                match v.get(j.max(0) as usize) {
                    Some(x) if j >= 0 => x,
                    _ if ax_py_on() => panic!("IndexError: {} index out of range", if matches!(self, Json::Tup { .. }) { "tuple" } else { "list" }),
                    _ => &AX_NULL,
                }
            }
            // s[i] on dynamic text: the character, as a one-character string.
            Json::Str { v } => {
                let n = v.chars().count() as i64;
                let j = if *i < 0 { n + *i } else { *i };
                match v.chars().nth(j.max(0) as usize) {
                    Some(c) if j >= 0 => ax_char_json(c),
                    _ if ax_py_on() => panic!("IndexError: string index out of range"),
                    _ => &AX_NULL,
                }
            }
            _ => &AX_NULL,
        }
    }
}
// One shared Json string per character, so s[i] can hand out a reference (each character is made once).
fn ax_char_json(c: char) -> &'static Json {
    thread_local! {
        static CHARS: std::cell::RefCell<HashMap<char, &'static Json>> = std::cell::RefCell::new(HashMap::new());
    }
    CHARS.with(|m| *m.borrow_mut().entry(c).or_insert_with(|| Box::leak(Box::new(Json::Str { v: c.to_string() }))))
}
// j[k] with a Json key: a string looks up an object field, a number an array item.
impl AxRef<Json, Json> for Json {
    fn ax_ref(&self, k: &Json) -> &Json {
        match k {
            Json::Str { v } => self.ax_ref(v),
            Json::Num { v, .. } => self.ax_ref(&(*v as i64)),
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
            Json::Num { v, .. } => self.ax_set(v as i64, x),
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
        ax_char_at(self, i).to_string()
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
    // f: written as a float (1.0), not an int (1); both are kept as f64.
    Num { v: f64, f: bool },
    Str { v: String },
    Arr { v: Vec<Json> },
    // A Python tuple whose length differs by path (`("room", n, d)` or `("capacity", 9)`).
    Tup { v: Vec<Json> },
    Obj { v: AxM<String, Json> },
}
// Json values can be map keys and set items (`qmap[q["id"]] = q` with q a mixed-type record).
impl Eq for Json {}
impl Hash for Json {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match self {
            Json::Null => 0u8.hash(h),
            Json::Bool { v } => v.hash(h),
            // Whole numbers hash like the int they equal; others by their bits.
            Json::Num { v, .. } => {
                if v.fract() == 0.0 && v.abs() < 9e15 {
                    (*v as i64).hash(h)
                } else {
                    v.to_bits().hash(h)
                }
            }
            Json::Str { v } => v.hash(h),
            Json::Arr { v } | Json::Tup { v } => v.hash(h),
            // Equal objects may hold their keys in a different order, so only the size is hashed.
            Json::Obj { v } => v.m.len().hash(h),
        }
    }
}
impl Ord for Json {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        let rank = |j: &Json| match j {
            Json::Null => 0,
            Json::Bool { .. } => 1,
            Json::Num { .. } => 2,
            Json::Str { .. } => 3,
            Json::Arr { .. } => 4, Json::Tup { .. } => 6,
            Json::Obj { .. } => 5,
        };
        self.partial_cmp(o).unwrap_or_else(|| rank(self).cmp(&rank(o)).then_with(|| self.show().cmp(&o.show())))
    }
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
// A Python list of ints and strings holds Json values: arithmetic and comparisons work on their numbers.
fn ax_jnum(j: &Json) -> f64 {
    match j {
        Json::Num { v, .. } => *v,
        Json::Bool { v } => *v as i64 as f64,
        _ => panic!("unsupported operand type: {} is not a number", j.show()),
    }
}
fn ax_jn(v: f64) -> Json {
    // A dynamic number is a float: a whole number past 2^53 would lose digits, so stop instead.
    if v.fract() == 0.0 && v.abs() > 9007199254740992.0 {
        panic!("number too big for a value of mixed type: {v}");
    }
    Json::Num { v, f: false }
}
// The same, marked as a float when either side was one (or, for true division, always).
fn ax_jnk(v: f64, f: bool) -> Json {
    match ax_jn(v) {
        Json::Num { v, .. } => Json::Num { v, f },
        j => j,
    }
}
fn ax_jf(j: &Json) -> bool {
    matches!(j, Json::Num { f: true, .. })
}
impl Plus for Json {
    type Out = Json;
    fn plus(self, o: Json) -> Json {
        match (self, o) {
            (Json::Str { v: a }, Json::Str { v: b }) => Json::Str { v: a + &b },
            (Json::Arr { v: mut a }, Json::Arr { v: b }) => {
                a.extend(b);
                Json::Arr { v: a }
            }
            // `(tm,) + t` with t a tuple held as JSON: one tuple.
            (Json::Tup { v: mut a } | Json::Arr { v: mut a }, Json::Tup { v: b }) | (Json::Tup { v: mut a }, Json::Arr { v: b }) => {
                a.extend(b);
                Json::Tup { v: a }
            }
            (a, b) => ax_jnk(ax_jnum(&a) + ax_jnum(&b), ax_jf(&a) || ax_jf(&b)),
        }
    }
}
impl Plus<String> for Json {
    type Out = Json;
    fn plus(self, o: String) -> Json {
        self.plus(Json::Str { v: o })
    }
}
impl Plus<Json> for String {
    type Out = Json;
    fn plus(self, o: Json) -> Json {
        Json::Str { v: self }.plus(o)
    }
}
macro_rules! ax_json_num_ops {
    ($($t:ty),*) => {$(
        impl Plus<$t> for Json {
            type Out = Json;
            fn plus(self, o: $t) -> Json {
                ax_jnk(ax_jnum(&self) + o as f64, ax_jf(&self) || ax_is_f64::<$t>())
            }
        }
        impl Plus<Json> for $t {
            type Out = Json;
            fn plus(self, o: Json) -> Json {
                ax_jnk(self as f64 + ax_jnum(&o), ax_jf(&o) || ax_is_f64::<$t>())
            }
        }
        impl std::ops::Sub<$t> for Json { type Output = Json; fn sub(self, o: $t) -> Json { ax_jnk(ax_jnum(&self) - o as f64, ax_jf(&self) || ax_is_f64::<$t>()) } }
        impl std::ops::Sub<Json> for $t { type Output = Json; fn sub(self, o: Json) -> Json { ax_jnk(self as f64 - ax_jnum(&o), ax_jf(&o) || ax_is_f64::<$t>()) } }
        impl std::ops::Mul<$t> for Json { type Output = Json; fn mul(self, o: $t) -> Json { ax_jnk(ax_jnum(&self) * o as f64, ax_jf(&self) || ax_is_f64::<$t>()) } }
        impl std::ops::Mul<Json> for $t { type Output = Json; fn mul(self, o: Json) -> Json { ax_jnk(self as f64 * ax_jnum(&o), ax_jf(&o) || ax_is_f64::<$t>()) } }
        impl std::ops::Div<$t> for Json { type Output = Json; fn div(self, o: $t) -> Json { ax_jnk(ax_jnum(&self) / o as f64, true) } }
        impl std::ops::Div<Json> for $t { type Output = Json; fn div(self, o: Json) -> Json { ax_jnk(self as f64 / ax_jnum(&o), true) } }
        impl PartialEq<Json> for $t { fn eq(&self, o: &Json) -> bool { o == self } }
        impl PartialOrd<$t> for Json {
            fn partial_cmp(&self, o: &$t) -> Option<std::cmp::Ordering> {
                ax_jnum(self).partial_cmp(&(*o as f64))
            }
        }
        impl PartialOrd<Json> for $t {
            fn partial_cmp(&self, o: &Json) -> Option<std::cmp::Ordering> {
                (*self as f64).partial_cmp(&ax_jnum(o))
            }
        }
    )*};
}
fn ax_is_f64<T: 'static>() -> bool {
    std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>()
}
ax_json_num_ops!(i64, f64);
impl std::ops::Sub for Json { type Output = Json; fn sub(self, o: Json) -> Json { ax_jnk(ax_jnum(&self) - ax_jnum(&o), ax_jf(&self) || ax_jf(&o)) } }
impl std::ops::Mul for Json {
    type Output = Json;
    fn mul(self, o: Json) -> Json {
        match (self, o) {
            (Json::Str { v }, n @ Json::Num { .. }) | (n @ Json::Num { .. }, Json::Str { v }) => Json::Str { v: v.repeat(ax_jnum(&n).max(0.0) as usize) },
            (a, b) => ax_jnk(ax_jnum(&a) * ax_jnum(&b), ax_jf(&a) || ax_jf(&b)),
        }
    }
}
// Dividing by zero raises in Python, as it does for a plain number.
fn ax_jzero(b: f64, f: bool, what: &str) {
    if b == 0.0 && ax_py_on() {
        panic!("ZeroDivisionError: {}", if f { format!("float {what}") } else if what == "modulo" { "integer modulo by zero".into() } else { "integer division or modulo by zero".to_string() });
    }
}
impl std::ops::Div for Json { type Output = Json; fn div(self, o: Json) -> Json { let b = ax_jnum(&o); if b == 0.0 && ax_py_on() { panic!("ZeroDivisionError: division by zero") } ax_jnk(ax_jnum(&self) / b, true) } }
impl std::ops::Rem for Json { type Output = Json; fn rem(self, o: Json) -> Json { let (a, b) = (ax_jnum(&self), ax_jnum(&o)); let f = ax_jf(&self) || ax_jf(&o); ax_jzero(b, f, "modulo"); if f { ax_jnk(a.ax_pymod(b), true) } else { ax_jnk(a - b * (a / b).floor(), false) } } }
impl std::ops::Neg for Json { type Output = Json; fn neg(self) -> Json { let f = ax_jf(&self); ax_jnk(-ax_jnum(&self), f) } }
impl std::ops::AddAssign for Json { fn add_assign(&mut self, o: Json) { *self = std::mem::take(self).plus(o) } }
impl std::ops::SubAssign for Json { fn sub_assign(&mut self, o: Json) { *self = std::mem::take(self) - o } }
impl std::ops::MulAssign for Json { fn mul_assign(&mut self, o: Json) { *self = std::mem::take(self) * o } }
impl std::ops::DivAssign for Json { fn div_assign(&mut self, o: Json) { *self = std::mem::take(self) / o } }
impl AxFloorDiv for Json {
    fn floordiv(self, b: Json) -> Json {
        let f = ax_jf(&self) || ax_jf(&b);
        ax_jzero(ax_jnum(&b), f, "floor division by zero");
        ax_jnk((ax_jnum(&self) / ax_jnum(&b)).floor(), f)
    }
}
impl AxNum for Json {
    fn ax_fmt(&self, decimals: i64) -> String {
        format!("{:.*}", decimals.max(0) as usize, ax_jnum(self))
    }
    fn ax_abs(&self) -> Json {
        ax_jnk(ax_jnum(self).abs(), ax_jf(self))
    }
    fn ax_float(&self) -> f64 {
        ax_jnum(self)
    }
    fn ax_int(&self) -> i64 {
        ax_jnum(self) as i64
    }
}
impl AxIn<Json> for Vec<String> {
    fn ax_in(&self, x: &Json) -> bool {
        self.iter().any(|y| x == y)
    }
}
// `ans in spec` with spec a JSON value: an item of a list, a key of an object, or a substring of a string.
impl AxIn<String> for Json {
    fn ax_in(&self, x: &String) -> bool {
        match self {
            Json::Arr { v } | Json::Tup { v } => v.iter().any(|y| y == x),
            Json::Obj { v } => v.contains_key(x),
            Json::Str { v } => v.contains(x.as_str()),
            _ => false,
        }
    }
}
// `ln["sku"] in prods` with both JSON values.
impl AxIn<Json> for Json {
    fn ax_in(&self, x: &Json) -> bool {
        match (self, x) {
            (Json::Arr { v } | Json::Tup { v }, x) => v.iter().any(|y| y == x),
            (Json::Obj { v }, Json::Str { v: k }) => v.contains_key(k),
            (Json::Obj { .. }, _) => false,
            (Json::Str { v }, Json::Str { v: k }) => v.contains(k.as_str()),
            (Json::Str { .. }, _) => panic!("TypeError: 'in <string>' requires string as left operand"),
            _ => panic!("TypeError: argument of type is not iterable"),
        }
    }
}
// Python's `a <= b` (`a < b` when strict) on sets.
pub fn ax_subset<T: PartialEq>(a: Vec<T>, b: Vec<T>, strict: bool) -> bool {
    a.iter().all(|x| b.contains(x)) && (!strict || b.iter().any(|x| !a.contains(x)))
}
// A JSON value used where a plain value is expected (a Python slot that holds a different type per row).
pub fn ax_json_dec(j: Json) -> Dec {
    j.ax_dec()
}
// A decimal next to a JSON value (a Python slot holding a decimal pair on some rows): `abs(a - value) <= tol`.
impl std::ops::Sub<Json> for Dec {
    type Output = Dec;
    fn sub(self, o: Json) -> Dec {
        self - o.ax_dec()
    }
}
impl PartialEq<Json> for Dec {
    fn eq(&self, o: &Json) -> bool {
        *self == o.ax_dec()
    }
}
impl PartialOrd<Json> for Dec {
    fn partial_cmp(&self, o: &Json) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&o.ax_dec())
    }
}

pub fn ax_json_strs(j: Json) -> Vec<String> {
    match j {
        Json::Arr { v } | Json::Tup { v } => v.iter().map(|x| x.ax_text()).collect(),
        Json::Str { v } => v.chars().map(|c| c.to_string()).collect(),
        Json::Obj { v } => v.keys().cloned().collect(),
        _ => vec![],
    }
}
pub fn ax_json_ints(j: Json) -> Vec<i64> {
    match j {
        Json::Arr { v } | Json::Tup { v } => v.iter().map(|x| x.ax_int()).collect(),
        _ => vec![],
    }
}
// Changing a JSON value in place, as Python changes a dict or list: `rec["pages"] += 10`, `rec["tags"].append(x)`.
impl AxMut<String, Json> for Json {
    fn ax_mut(&mut self, k: &String) -> &mut Json {
        if matches!(self, Json::Null) {
            *self = Json::Obj { v: AxM::ordered() };
        }
        match self {
            Json::Obj { v } => {
                if !v.contains_key(k) {
                    v.insert(k.clone(), Json::Null);
                }
                v.get_mut(k).unwrap()
            }
            _ => panic!("{} is not a dict, so it has no key {k:?}", self.show()),
        }
    }
}
impl AxMut<i64, Json> for Json {
    fn ax_mut(&mut self, i: &i64) -> &mut Json {
        match self {
            Json::Arr { v } => {
                let j = ax_idx(*i, v.len());
                &mut v[j]
            }
            _ => panic!("{} is not a list", self.show()),
        }
    }
}
// The same reads and writes, but an index out of range is a Python IndexError a try can catch instead of a
// Rust panic. Reads and writes that do not raise (map entries, JSON objects) keep their AxRef/AxMut behavior.
pub trait AxRefTry<K, V> {
    fn ax_try_ref(&self, k: &K) -> R<&V>;
}
pub trait AxMutTry<K, V> {
    fn ax_try_mut(&mut self, k: &K) -> R<&mut V>;
}
impl<T> AxRefTry<i64, T> for Vec<T> {
    fn ax_try_ref(&self, i: &i64) -> R<&T> {
        let j = ax_checked_idx(*i, self.len(), "list index out of range")?;
        Ok(&self[j])
    }
}
impl<T> AxMutTry<i64, T> for Vec<T> {
    fn ax_try_mut(&mut self, i: &i64) -> R<&mut T> {
        let j = ax_checked_idx(*i, self.len(), "list assignment index out of range")?;
        Ok(&mut self[j])
    }
}
impl<T> AxRefTry<i64, T> for (T, T) {
    fn ax_try_ref(&self, i: &i64) -> R<&T> {
        let j = ax_checked_idx(*i, 2, "tuple index out of range")?;
        Ok([&self.0, &self.1][j])
    }
}
impl<T> AxRefTry<i64, T> for (T, T, T) {
    fn ax_try_ref(&self, i: &i64) -> R<&T> {
        let j = ax_checked_idx(*i, 3, "tuple index out of range")?;
        Ok([&self.0, &self.1, &self.2][j])
    }
}
impl<T> AxRefTry<i64, T> for (T, T, T, T) {
    fn ax_try_ref(&self, i: &i64) -> R<&T> {
        let j = ax_checked_idx(*i, 4, "tuple index out of range")?;
        Ok([&self.0, &self.1, &self.2, &self.3][j])
    }
}
impl<K: Hash + Eq + Ord + Show + 'static, V: Default + 'static> AxRefTry<K, V> for AxM<K, V> {
    fn ax_try_ref(&self, k: &K) -> R<&V> {
        Ok(self.ax_ref(k))
    }
}
impl<K: Hash + Eq + Ord + Clone, V: Default> AxMutTry<K, V> for AxM<K, V> {
    fn ax_try_mut(&mut self, k: &K) -> R<&mut V> {
        Ok(self.ax_mut(k))
    }
}
impl AxRefTry<i64, String> for String {
    fn ax_try_ref(&self, i: &i64) -> R<&String> {
        if ax_ascii(self) {
            ax_checked_idx(*i, self.len(), "string index out of range")?;
        } else {
            ax_checked_idx(*i, self.chars().count(), "string index out of range")?;
        }
        Ok(self.ax_ref(i))
    }
}
impl AxRefTry<String, Json> for Json {
    fn ax_try_ref(&self, k: &String) -> R<&Json> {
        Ok(self.ax_ref(k))
    }
}
impl AxRefTry<i64, Json> for Json {
    fn ax_try_ref(&self, i: &i64) -> R<&Json> {
        match self {
            Json::Arr { v } | Json::Tup { v } => {
                let msg = if matches!(self, Json::Tup { .. }) { "tuple index out of range" } else { "list index out of range" };
                let j = ax_checked_idx(*i, v.len(), msg)?;
                Ok(&v[j])
            }
            Json::Str { v } => {
                let j = ax_checked_idx(*i, v.chars().count(), "string index out of range")?;
                Ok(ax_char_json(v.chars().nth(j).unwrap()))
            }
            _ => Ok(&AX_NULL),
        }
    }
}
impl AxRefTry<Json, Json> for Json {
    fn ax_try_ref(&self, k: &Json) -> R<&Json> {
        match k {
            Json::Str { v } => Ok(self.ax_ref(v)),
            Json::Num { v, .. } => self.ax_try_ref(&(*v as i64)),
            _ => Ok(&AX_NULL),
        }
    }
}
impl AxMutTry<String, Json> for Json {
    fn ax_try_mut(&mut self, k: &String) -> R<&mut Json> {
        Ok(self.ax_mut(k))
    }
}
impl AxMutTry<i64, Json> for Json {
    fn ax_try_mut(&mut self, i: &i64) -> R<&mut Json> {
        match self {
            Json::Arr { v } => {
                let j = ax_checked_idx(*i, v.len(), "list assignment index out of range")?;
                Ok(&mut v[j])
            }
            _ => panic!("{} is not a list", self.show()),
        }
    }
}
impl Json {
    pub fn ax_push<T: AxToJson>(&mut self, x: T) {
        match self {
            Json::Arr { v } => v.push(x.ax_json()),
            _ => panic!("{} is not a list, so it has no append", self.show()),
        }
    }
}
// Python mode prints a JSON value as Python prints what json.loads gives: None, True, 'x', [1, 2.5], {'k': 1}.
fn ax_json_py(j: &Json, top: bool) -> String {
    match j {
        Json::Null => "None".into(),
        Json::Bool { v } => if *v { "True".into() } else { "False".into() },
        Json::Num { v, f } if v.fract() == 0.0 && v.abs() < 1e16 && !(*f && ax_py_on()) => format!("{}", *v as i64),
        Json::Num { v, .. } => ax_py_float(*v),
        Json::Str { v } if top => v.clone(),
        Json::Str { v } => ax_py_repr(v),
        Json::Tup { .. } if ax_json_sent(j) => "<object object>".into(),
        Json::Tup { v } if v.len() == 1 => format!("({},)", ax_json_py(&v[0], false)),
        Json::Tup { v } => format!("({})", v.iter().map(|x| ax_json_py(x, false)).collect::<Vec<_>>().join(", ")),
        Json::Arr { v } => format!("[{}]", v.iter().map(|x| ax_json_py(x, false)).collect::<Vec<_>>().join(", ")),
        Json::Obj { v } => format!("{{{}}}", v.iter().map(|(k, x)| format!("{}: {}", ax_py_repr(k), ax_json_py(x, false))).collect::<Vec<_>>().join(", ")),
    }
}
impl Show for Json {
    fn show(&self) -> String {
        if ax_py_on() {
            return ax_json_py(self, true);
        }
        match self {
            Json::Null => "null".into(),
            Json::Bool { v } => v.to_string(),
            Json::Num { v, .. } => v.to_string(),
            Json::Str { v } => format!("{v:?}"),
            Json::Tup { .. } if ax_json_sent(self) => "<object object>".into(),
            Json::Arr { v } | Json::Tup { v } => format!("[{}]", v.iter().map(|x| x.show()).collect::<Vec<_>>().join(",")),
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
            t.parse::<f64>().map(|v| Json::Num { v, f: t.contains(['.', 'e', 'E']) }).map_err(|_| bad(start))
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
            Json::Arr { v } | Json::Tup { v } => {
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


// A JSON object key: text as it is, or a JSON value read from other data (`prods[ln["sku"]]`).
pub trait AxJKey {
    fn ax_jkey(&self) -> String;
}
impl AxJKey for str {
    fn ax_jkey(&self) -> String {
        self.to_string()
    }
}
impl AxJKey for String {
    fn ax_jkey(&self) -> String {
        self.clone()
    }
}
impl AxJKey for &str {
    fn ax_jkey(&self) -> String {
        self.to_string()
    }
}
impl AxJKey for Json {
    fn ax_jkey(&self) -> String {
        match self {
            Json::Str { v } => v.clone(),
            other => ax_json_py(other, true),
        }
    }
}
// Reading Json values directly: j.text(), j.int(), j.list(), j.is_null(), `for x in j`.
impl Json {
    pub fn ax_text(&self) -> String {
        match self {
            Json::Str { v } => v.clone(),
            // Python's str(None).
            Json::Null if ax_py_on() => "None".into(),
            Json::Null => String::new(),
            other => other.show(),
        }
    }
    pub fn ax_int(&self) -> i64 {
        match self {
            Json::Num { v, .. } => *v as i64,
            Json::Bool { v } => *v as i64,
            // Python's int("12"); anything else is an error there, so it stops the run here too.
            Json::Str { v } if ax_py_on() => match ax_py_digits(v.trim()).parse::<i64>() {
                Ok(n) => n,
                Err(_) => panic!("ValueError: invalid literal for int() with base 10: {}", ax_py_repr(v)),
            },
            other if ax_py_on() => panic!("TypeError: int() argument must be a string or a number, not {}", other.ax_kind()),
            _ => 0,
        }
    }
    pub fn ax_float(&self) -> f64 {
        match self {
            Json::Num { v, .. } => *v,
            Json::Bool { v } => *v as i64 as f64,
            Json::Str { v } if ax_py_on() => match v.trim().parse::<f64>() {
                Ok(n) => n,
                Err(_) => panic!("ValueError: could not convert string to float: {}", ax_py_repr(v)),
            },
            other if ax_py_on() => panic!("TypeError: float() argument must be a string or a number, not {}", other.ax_kind()),
            _ => 0.0,
        }
    }
    pub fn ax_dec(&self) -> Dec {
        match self {
            Json::Num { v, .. } => Dec::parse(&v.to_string()).unwrap_or_default(),
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
    // `sorted(q['answer'])` on a JSON list (or set) value.
    pub fn ax_sort(&self) -> Vec<Json> {
        let mut v = self.ax_list();
        v.sort();
        v
    }
    pub fn ax_list(&self) -> Vec<Json> {
        match self {
            Json::Arr { v } | Json::Tup { v } => v.clone(),
            _ => vec![],
        }
    }
    pub fn ax_len(&self) -> i64 {
        match self {
            Json::Tup { .. } if ax_json_sent(self) => panic!("TypeError: object of type 'object' has no len()"),
            Json::Str { v } => v.chars().count() as i64,
            Json::Arr { v } | Json::Tup { v } => v.len() as i64,
            Json::Obj { v } => v.m.len() as i64,
            _ => 0,
        }
    }
    // Reached from Python's `j == []` / `j == {}`: an empty list or object (not null, 0, False or "").
    pub fn ax_is_empty(&self) -> bool {
        match self {
            Json::Arr { v } => v.is_empty(),
            Json::Obj { v } => v.is_empty(),
            _ => false,
        }
    }
    // d.get(k, default) on a JSON object: the value under k, else the default as JSON.
    pub fn ax_get_or<K: AxJKey + ?Sized, D: AxToJson>(&self, k: &K, d: D) -> Json {
        match self {
            Json::Obj { v } => v.get(&k.ax_jkey()).cloned().unwrap_or_else(|| d.ax_json()),
            _ => d.ax_json(),
        }
    }
    // d.get(k, {}) and d.get(k, []) on a JSON object.
    pub fn ax_get_or_emap<K: AxJKey + ?Sized>(&self, k: &K) -> Json {
        match self {
            Json::Obj { v } if v.get(&k.ax_jkey()).is_some() => v.get(&k.ax_jkey()).cloned().unwrap(),
            _ => Json::Obj { v: AxM::ordered() },
        }
    }
    pub fn ax_get_or_elist<K: AxJKey + ?Sized>(&self, k: &K) -> Json {
        match self {
            Json::Obj { v } if v.get(&k.ax_jkey()).is_some() => v.get(&k.ax_jkey()).cloned().unwrap(),
            _ => Json::Arr { v: vec![] },
        }
    }
    pub fn ax_has<K: AxJKey + ?Sized>(&self, k: &K) -> bool {
        match self {
            Json::Obj { v } => v.get(&k.ax_jkey()).is_some(),
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
            Json::Arr { v } | Json::Tup { v } => v.clone(),
            _ => vec![],
        }
    }
}
impl Json {
    // What `for x in j` walks, as in Python: a list's items, an object's keys, a string's characters.
    pub fn ax_walk(&self) -> Vec<Json> {
        match self {
            Json::Arr { v } | Json::Tup { v } => v.clone(),
            Json::Obj { v } => v.keys().map(|k| Json::Str { v: k.clone() }).collect(),
            Json::Str { v } => v.chars().map(|c| Json::Str { v: c.to_string() }).collect(),
            _ => vec![],
        }
    }
}
impl AxEach for Json {
    type It = std::vec::IntoIter<Json>;
    fn ax_each(self) -> Self::It {
        self.ax_walk().into_iter()
    }
}
impl AxIter for Json {
    type I<'a> = std::vec::IntoIter<Json>;
    fn ax_iter<'a>(&'a self) -> Self::I<'a> {
        self.ax_walk().into_iter()
    }
}

// ---- CSV ----
pub fn csv(text: String) -> Vec<Vec<String>> {
    AxCsvIter { text, pos: 0 }.collect()
}
// CSV rows one at a time. Fields may be "quoted", with "" for a literal quote; quoted fields may hold
// commas and newlines. A line without quotes is split straight from the text; empty lines are skipped.
pub struct AxCsvIter {
    text: String,
    pos: usize,
}
impl Iterator for AxCsvIter {
    type Item = Vec<String>;
    fn next(&mut self) -> Option<Vec<String>> {
        loop {
            if self.pos >= self.text.len() {
                return None;
            }
            let rest = &self.text[self.pos..];
            let end = rest.find('\n').unwrap_or(rest.len());
            let line = &rest[..end];
            if !line.contains('"') {
                self.pos += end + 1;
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    // Python's csv.reader gives [] for a blank line; Brevi's csv() skips it.
                    if ax_py_on() {
                        return Some(Vec::new());
                    }
                    continue;
                }
                return Some(line.split(',').map(|f| f.replace('\r', "")).collect());
            }
            let (row, used) = csv_quoted_row(rest);
            self.pos += used;
            if row.len() == 1 && row[0].is_empty() {
                if ax_py_on() {
                    return Some(Vec::new());
                }
                continue;
            }
            return Some(row);
        }
    }
}
impl AxEach for AxCsvIter {
    type It = AxCsvIter;
    fn ax_each(self) -> Self::It {
        self
    }
}
// One row that has quotes in it, and the bytes it used (its ending newline included).
fn csv_quoted_row(text: &str) -> (Vec<String>, usize) {
    let mut row = Vec::new();
    let mut cur = String::new();
    let mut inq = false;
    let mut it = text.char_indices().peekable();
    while let Some((i, ch)) = it.next() {
        match ch {
            '"' if inq && it.peek().map(|p| p.1) == Some('"') => {
                it.next();
                cur.push('"');
            }
            '"' => inq = !inq,
            ',' if !inq => row.push(std::mem::take(&mut cur)),
            '\r' if !inq => {}
            '\n' if !inq => {
                row.push(cur);
                return (row, i + 1);
            }
            c => cur.push(c),
        }
    }
    row.push(cur);
    (row, text.len())
}
// `for row in csv.reader(src)` reads the rows one at a time instead of building them all first.
pub trait AxCsvStream {
    fn ax_csv_iter(self) -> AxCsvIter;
}
impl AxCsvStream for String {
    fn ax_csv_iter(self) -> AxCsvIter {
        AxCsvIter { text: self, pos: 0 }
    }
}
impl AxCsvStream for Vec<String> {
    fn ax_csv_iter(self) -> AxCsvIter {
        AxCsvIter { text: self.join("\n"), pos: 0 }
    }
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
        if self.days == i64::MIN && ax_py_on() {
            return "None".into();
        }
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
fn big_from_radix(digits: &str, base: u32) -> Option<Big> {
    if digits.is_empty() {
        return None;
    }
    let mut mag = vec![0u32];
    for c in digits.chars() {
        let d = c.to_digit(base)?;
        let mut carry = d as u64;
        for w in mag.iter_mut() {
            let x = *w as u64 * base as u64 + carry;
            *w = (x % BIG_BASE) as u32;
            carry = x / BIG_BASE;
        }
        while carry > 0 {
            mag.push((carry % BIG_BASE) as u32);
            carry /= BIG_BASE;
        }
    }
    Some(Big::make(false, mag))
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
// Magnitude division: (quotient, remainder), long division in base 10^9 with each digit found by bisection.
fn mag_divmod(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if mag_cmp(a, b) == std::cmp::Ordering::Less {
        return (vec![0], a.to_vec());
    }
    if b.len() == 1 {
        let d = b[0] as u64;
        let mut q = vec![0u32; a.len()];
        let mut r = 0u64;
        for i in (0..a.len()).rev() {
            let cur = r * BIG_BASE + a[i] as u64;
            q[i] = (cur / d) as u32;
            r = cur % d;
        }
        return (mag_trim(q), vec![r as u32]);
    }
    let mut q = vec![0u32; a.len()];
    let mut r: Vec<u32> = vec![0];
    for i in (0..a.len()).rev() {
        r.insert(0, a[i]);
        r = mag_trim(r);
        let (mut lo, mut hi) = (0u32, (BIG_BASE - 1) as u32);
        while lo < hi {
            let mid = lo + (hi - lo + 1) / 2;
            if mag_cmp(&mag_mul(b, &[mid]), &r) != std::cmp::Ordering::Greater {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        if lo > 0 {
            r = mag_sub(&r, &mag_mul(b, &[lo]));
        }
        q[i] = lo;
    }
    (mag_trim(q), mag_trim(r))
}
impl Big {
    fn is_zero(&self) -> bool {
        self.mag.iter().all(|d| *d == 0)
    }
    // Python's floor division and modulo: the remainder takes the divisor's sign.
    fn py_divmod(&self, o: &Big) -> (Big, Big) {
        if o.is_zero() {
            panic!("ZeroDivisionError: integer division or modulo by zero");
        }
        let (q, r) = mag_divmod(&self.mag, &o.mag);
        let (mut q, mut r) = (Big::make(self.neg != o.neg, q), Big::make(self.neg, r));
        if !r.is_zero() && self.neg != o.neg {
            q = q - Big::from_i64(1);
            r = r.plus(o.clone());
        }
        (q, r)
    }
}
impl AxFloorDiv for Big {
    fn floordiv(self, b: Big) -> Big {
        self.py_divmod(&b).0
    }
}
impl AxFloorDiv<i64> for Big {
    fn floordiv(self, b: i64) -> Big {
        self.py_divmod(&Big::from_i64(b)).0
    }
}
impl AxPyMod for Big {
    fn ax_pymod(self, b: Big) -> Big {
        self.py_divmod(&b).1
    }
}
impl AxPyMod<i64> for Big {
    fn ax_pymod(self, b: i64) -> Big {
        self.py_divmod(&Big::from_i64(b)).1
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
impl AxToBig for Big {
    fn ax_big(&self) -> Big {
        self.clone()
    }
}
// int(x) of a float next to big integers: truncated toward zero, as Python's int() does.
impl AxToBig for f64 {
    fn ax_big(&self) -> Big {
        if self.is_nan() {
            panic!("ValueError: cannot convert float NaN to integer");
        }
        if self.is_infinite() {
            panic!("OverflowError: cannot convert float infinity to integer");
        }
        let t = self.trunc();
        if t.abs() < 9.0e18 {
            Big::from_i64(t as i64)
        } else {
            Big::parse(&format!("{t:.0}")).unwrap_or_else(|_| Big::from_i64(0))
        }
    }
}
// int(b) where an int is needed: the value of a function that returns a big integer, used as an index or count.
pub trait AxBigInt {
    fn ax_int(&self) -> R<i64>;
}
impl AxBigInt for Big {
    fn ax_int(&self) -> R<i64> {
        let mut v: i128 = 0;
        for d in self.mag.iter().rev() {
            v = v * BIG_BASE as i128 + *d as i128;
            if v > i64::MAX as i128 + 1 {
                return Err(AxErr("OverflowError: Python int too large to convert to C long".into()));
            }
        }
        let v = if self.neg { -v } else { v };
        i64::try_from(v).map_err(|_| AxErr("OverflowError: Python int too large to convert to C long".into()))
    }
}
pub trait AxStrBig {
    fn ax_big(&self) -> R<Big>;
    fn ax_big_base(&self, base: i64) -> R<Big>;
}
impl AxStrBig for String {
    fn ax_big(&self) -> R<Big> {
        if ax_py_on() {
            ax_big_base_impl(self, 10)
        } else {
            Big::parse(self)
        }
    }
    fn ax_big_base(&self, base: i64) -> R<Big> {
        ax_big_base_impl(self, base)
    }
}

// Python's int(s, base) read into a big integer, so any number of digits works. The checks match AxIntBase.
fn ax_big_base_impl(s: &str, base: i64) -> R<Big> {
    let bad = || AxErr(format!("invalid literal for int() with base {base}: {}", ax_py_repr(s)));
    let t = s.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let low = t.to_ascii_lowercase();
    let pre = |p: &str| low.starts_with(p);
    let (b, digits) = match base {
        0 if pre("0x") => (16, &t[2..]),
        0 if pre("0o") => (8, &t[2..]),
        0 if pre("0b") => (2, &t[2..]),
        0 => (10, t),
        16 if pre("0x") => (16, &t[2..]),
        8 if pre("0o") => (8, &t[2..]),
        2 if pre("0b") => (2, &t[2..]),
        2..=36 => (base as u32, t),
        _ => return Err(AxErr("int() base must be >= 2 and <= 36, or 0".into())),
    };
    let digits = digits.strip_prefix('_').filter(|_| digits.len() < t.len()).unwrap_or(digits);
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return Err(bad());
    }
    // base 10 takes any Unicode decimal digit (`int("१२३", 0)` is 123)
    let digits: String = digits.chars().filter(|&c| c != '_').map(|c| if b == 10 { ax_uni_digit(c) } else { c }).collect();
    // base 0 reads a decimal literal as Python source does: no leading zeros unless the value is 0
    if base == 0 && b == 10 && digits.len() > 1 && digits.starts_with('0') && digits.bytes().any(|c| c != b'0') {
        return Err(bad());
    }
    let v = big_from_radix(&digits, b).ok_or_else(bad)?;
    Ok(if neg { Big::make(true, v.mag) } else { v })
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
// A function stored in a map: `ops = {"+": add}`, then `ops["+"](1, 2)` or `f = ops[k]; f(1, 2)`.
#[derive(Clone, Copy, PartialEq)]
pub struct AxFn<F>(pub Option<F>);
impl<F> Default for AxFn<F> {
    fn default() -> Self {
        AxFn(None)
    }
}
impl<F> std::ops::Deref for AxFn<F> {
    type Target = F;
    fn deref(&self) -> &F {
        self.0.as_ref().expect("no function stored for that key")
    }
}
impl<F> Show for AxFn<F> {
    fn show(&self) -> String {
        "<fn>".to_string()
    }
}
// Python's None in an int, float, str or tuple slot: a value no program makes by itself.
pub trait AxNone {
    fn ax_none() -> Self;
    fn ax_is_none(&self) -> bool
    where
        Self: PartialEq + Sized,
    {
        *self == Self::ax_none()
    }
}
pub fn ax_none<T: AxNone>() -> T {
    T::ax_none()
}
impl AxNone for i64 {
    #[inline]
    fn ax_none() -> Self {
        i64::MIN
    }
}
impl AxNone for f64 {
    #[inline]
    fn ax_none() -> Self {
        f64::MIN
    }
}
impl AxNone for String {
    fn ax_none() -> Self {
        "\u{0}None".to_string()
    }
}
impl AxNone for Date {
    fn ax_none() -> Self {
        Date { days: i64::MIN }
    }
}
impl AxNone for bool {
    #[inline]
    fn ax_none() -> Self {
        false
    }
}
// A second unset value, for `MISSING = object()` next to None in the same slot. The value's type keeps its
// own None representation (`"\0None"` for a str, Json::Null for JSON), and the sentinel is a distinct value of
// the same type, so `v is MISSING` and `v is None` stay apart. N numbers the sentinels of the program.
pub struct AxSentVal<const N: usize>;
impl<const N: usize> AxSentVal<N> {
    pub fn make<T: AxSentBase<N>>() -> T {
        T::ax_sent()
    }
    pub fn is<T: AxSentBase<N>>(v: &T) -> bool {
        v.ax_is_sent()
    }
}
pub trait AxSentBase<const N: usize>: Sized {
    fn ax_sent() -> Self;
    fn ax_is_sent(&self) -> bool;
}
pub fn ax_sent_str(n: usize) -> String {
    format!("\u{0}sent{n}")
}
// A str slot's sentinel is a string no input the program reads makes by itself, like its None string.
impl<const N: usize> AxSentBase<N> for String {
    fn ax_sent() -> Self {
        ax_sent_str(N)
    }
    fn ax_is_sent(&self) -> bool {
        *self == ax_sent_str(N)
    }
}
fn ax_json_sent(j: &Json) -> bool {
    matches!(j, Json::Tup { v } if v.len() == 1 && matches!(&v[0], Json::Str { v } if v.starts_with("\u{0}sent")))
}
// A JSON slot's sentinel is a one-item internal tuple no json.loads value is (a Tup is only made from a
// Python tuple), holding the sentinel's marker string.
impl<const N: usize> AxSentBase<N> for Json {
    fn ax_sent() -> Self {
        Json::Tup { v: vec![Json::Str { v: ax_sent_str(N) }] }
    }
    fn ax_is_sent(&self) -> bool {
        matches!(self, Json::Tup { v } if v.len() == 1 && matches!(&v[0], Json::Str { v } if *v == ax_sent_str(N)))
    }
}
impl<A: AxNone, B: AxNone> AxNone for (A, B) {
    fn ax_none() -> Self {
        (A::ax_none(), B::ax_none())
    }
}
impl<A: AxNone, B: AxNone, C: AxNone> AxNone for (A, B, C) {
    fn ax_none() -> Self {
        (A::ax_none(), B::ax_none(), C::ax_none())
    }
}
// "abc"[::-1] is "cba"; "AKQ".index_of("K") is 1 (-1 when missing), counted in characters.
// Python's heapq on a plain list: the smallest item is always at index 0.
pub trait AxHeapq<T> {
    fn ax_heap_push(&mut self, x: T);
    fn ax_heap_pop(&mut self) -> T;
    fn ax_heapify(&mut self);
    fn ax_heap_push_pop(&mut self, x: T) -> T;
    fn ax_heap_replace(&mut self, x: T) -> T;
}
fn ax_less<T: PartialOrd>(a: &T, b: &T) -> bool {
    a.partial_cmp(b) == Some(std::cmp::Ordering::Less)
}
fn ax_sift_down<T: PartialOrd>(v: &mut [T], mut i: usize) {
    loop {
        let (l, r) = (2 * i + 1, 2 * i + 2);
        let mut m = i;
        if l < v.len() && ax_less(&v[l], &v[m]) {
            m = l;
        }
        if r < v.len() && ax_less(&v[r], &v[m]) {
            m = r;
        }
        if m == i {
            return;
        }
        v.swap(i, m);
        i = m;
    }
}
impl<T: PartialOrd> AxHeapq<T> for Vec<T> {
    fn ax_heap_push(&mut self, x: T) {
        self.push(x);
        let mut i = self.len() - 1;
        while i > 0 && ax_less(&self[i], &self[(i - 1) / 2]) {
            self.swap(i, (i - 1) / 2);
            i = (i - 1) / 2;
        }
    }
    fn ax_heap_pop(&mut self) -> T {
        let n = self.len();
        assert!(n > 0, "index out of range: heappop from an empty list");
        self.swap(0, n - 1);
        let x = self.pop().unwrap();
        ax_sift_down(self, 0);
        x
    }
    fn ax_heapify(&mut self) {
        for i in (0..self.len() / 2).rev() {
            ax_sift_down(self, i);
        }
    }
    fn ax_heap_push_pop(&mut self, x: T) -> T {
        if self.is_empty() || !ax_less(&self[0], &x) {
            return x;
        }
        let top = std::mem::replace(&mut self[0], x);
        ax_sift_down(self, 0);
        top
    }
    fn ax_heap_replace(&mut self, x: T) -> T {
        assert!(!self.is_empty(), "index out of range: heapreplace on an empty list");
        let top = std::mem::replace(&mut self[0], x);
        ax_sift_down(self, 0);
        top
    }
}
// Python reads a string item by item: sorted(s), enumerate(s, 1); sorted(m) gives a map's sorted keys.
pub trait AxStrItems {
    fn ax_sort(&self) -> Vec<String>;
    fn ax_enumerate_from(&self, n: i64) -> Vec<(i64, String)>;
    fn ax_join(&self, sep: String) -> String;
}
impl AxStrItems for String {
    fn ax_sort(&self) -> Vec<String> {
        let mut v: Vec<String> = self.chars().map(String::from).collect();
        v.sort();
        v
    }
    fn ax_enumerate_from(&self, n: i64) -> Vec<(i64, String)> {
        self.chars().enumerate().map(|(i, c)| (i as i64 + n, c.to_string())).collect()
    }
    fn ax_join(&self, sep: String) -> String {
        self.chars().map(String::from).collect::<Vec<_>>().join(&sep)
    }
}
pub trait AxMapSort<K> {
    fn ax_sort(&self) -> Vec<K>;
}
impl<K: Hash + Eq + Ord + Clone, V> AxMapSort<K> for AxM<K, V> {
    fn ax_sort(&self) -> Vec<K> {
        let mut v: Vec<K> = self.m.keys().cloned().collect();
        v.sort();
        v
    }
}
// Python's xs.pop(i) and m.pop(k): take the item out and give it back.
pub trait AxPopAt<I, T> {
    fn ax_pop_at(&mut self, i: I) -> T;
}
impl<T> AxPopAt<i64, T> for Vec<T> {
    fn ax_pop_at(&mut self, i: i64) -> T {
        let j = ax_idx(i, self.len());
        self.remove(j)
    }
}
impl<K: Hash + Eq + Ord + Clone + Show + 'static, V> AxPopAt<K, V> for AxM<K, V> {
    fn ax_pop_at(&mut self, k: K) -> V {
        match self.remove(&k) {
            Some(v) => v,
            None => panic!("KeyError: {}", show_item(&k)),
        }
    }
}
// OrderedDict: move_to_end(k) makes k the newest entry; popitem(last) takes the newest (or oldest) out.
pub trait AxOrdered<K, V> {
    fn ax_move_to_end(&mut self, k: K);
    fn ax_pop_item(&mut self, last: bool) -> (K, V);
}
impl<K: Hash + Eq + Ord + Clone, V> AxOrdered<K, V> for AxM<K, V> {
    fn ax_move_to_end(&mut self, k: K) {
        if let Some(o) = &mut self.ord {
            if let Some(i) = o.iter().position(|x| *x == k) {
                let x = o.remove(i);
                o.push(x);
            }
        }
    }
    fn ax_pop_item(&mut self, last: bool) -> (K, V) {
        let k = match &self.ord {
            Some(o) => if last { o.last().cloned() } else { o.first().cloned() },
            None => if last { self.m.keys().max().cloned() } else { self.m.keys().min().cloned() },
        };
        let k = k.expect("KeyError: popitem(): dictionary is empty");
        let v = self.remove(&k).unwrap();
        (k, v)
    }
}
// Python's Counter(xs), dict(pairs) and c.most_common(n).
pub trait AxTally<T> {
    fn ax_tally(&self) -> AxM<T, i64>;
}
impl<T: Hash + Eq + Ord + Clone> AxTally<T> for Vec<T> {
    fn ax_tally(&self) -> AxM<T, i64> {
        let mut m = AxM::default().ax_lax();
        for x in self {
            match m.get_mut(x) {
                Some(n) => *n += 1,
                None => {
                    m.insert(x.clone(), 1);
                }
            }
        }
        m
    }
}
impl AxTally<String> for String {
    fn ax_tally(&self) -> AxM<String, i64> {
        self.chars().map(|c| c.to_string()).collect::<Vec<_>>().ax_tally()
    }
}
// sorted(m, key=f): the keys, ordered by f.
pub trait AxMapSortBy<K> {
    fn ax_sort_by<U: PartialOrd>(&self, f: impl Fn(K) -> U) -> Vec<K>;
    fn ax_sort_by_rev<U: PartialOrd>(&self, f: impl Fn(K) -> U, rev: bool) -> Vec<K>;
}
impl<K: Hash + Eq + Ord + Clone, V> AxMapSortBy<K> for AxM<K, V> {
    fn ax_sort_by<U: PartialOrd>(&self, f: impl Fn(K) -> U) -> Vec<K> {
        self.ax_sort_by_rev(f, false)
    }
    fn ax_sort_by_rev<U: PartialOrd>(&self, f: impl Fn(K) -> U, rev: bool) -> Vec<K> {
        let ks: Vec<K> = self.iter().map(|(k, _)| k.clone()).collect();
        ks.ax_sort_by_rev(f, rev)
    }
}
// Python's `xs[a:b] = ys`: the items from a up to b replaced by ys (indexes clamped as Python does).
pub trait AxSplice<T> {
    fn ax_splice(&mut self, a: i64, b: i64, v: Vec<T>);
}
impl<T> AxSplice<T> for Vec<T> {
    fn ax_splice(&mut self, a: i64, b: i64, v: Vec<T>) {
        let n = self.len() as i64;
        let at = |i: i64| (if i < 0 { (i + n).max(0) } else { i.min(n) }) as usize;
        let a = at(a);
        let b = at(b).max(a);
        self.splice(a..b, v);
    }
}
// xs.clear() and m.clear().
pub trait AxClear {
    fn ax_clear(&mut self);
}
impl<T> AxClear for Vec<T> {
    fn ax_clear(&mut self) {
        self.clear();
    }
}
impl<K: Hash + Eq + Ord + Clone, V> AxClear for AxM<K, V> {
    fn ax_clear(&mut self) {
        let keys: Vec<K> = self.keys().cloned().collect();
        for k in keys {
            self.remove(&k);
        }
    }
}
pub trait AxTallyWords {
    fn ax_tally_words(&self) -> AxM<String, i64>;
}
impl AxTallyWords for String {
    fn ax_tally_words(&self) -> AxM<String, i64> {
        let mut at: HashMap<&str, usize, BuildHasherDefault<AxHasher>> = HashMap::default();
        let mut seen: Vec<(&str, i64)> = Vec::new();
        for w in self.split_whitespace() {
            match at.get(w) {
                Some(&i) => seen[i].1 += 1,
                None => {
                    at.insert(w, seen.len());
                    seen.push((w, 1));
                }
            }
        }
        let mut m = AxM::default().ax_lax();
        for (w, n) in seen {
            m.insert(w.to_string(), n);
        }
        m
    }
}
// `dict(v)` on a dynamic value: a copy (a dict stays a dict; pairs become one).
pub trait AxJsonToDict {
    fn ax_to_dict(&self) -> Json;
}
impl AxJsonToDict for Json {
    fn ax_to_dict(&self) -> Json {
        match self {
            Json::Obj { .. } => self.clone(),
            other => panic!("TypeError: cannot convert {} to a dict", ax_json_py(other, true)),
        }
    }
}
pub trait AxToDict<K, V> {
    fn ax_to_dict(&self) -> AxM<K, V>;
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxToDict<K, V> for Vec<(K, V)> {
    fn ax_to_dict(&self) -> AxM<K, V> {
        let mut m = AxM::default();
        for (k, v) in self {
            m.insert(k.clone(), v.clone());
        }
        m
    }
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxToDict<K, V> for AxM<K, V> {
    fn ax_to_dict(&self) -> AxM<K, V> {
        let mut m = AxM::default();
        for (k, v) in self.iter() {
            m.insert(k.clone(), v.clone());
        }
        m
    }
}
pub trait AxMostCommon<K> {
    fn ax_most_common(&self, n: i64) -> Vec<(K, i64)>;
    fn ax_most_common_all(&self) -> Vec<(K, i64)> {
        self.ax_most_common(-1)
    }
}
impl<K: Hash + Eq + Ord + Clone> AxMostCommon<K> for AxM<K, i64> {
    fn ax_most_common(&self, n: i64) -> Vec<(K, i64)> {
        let mut v: Vec<(K, i64)> = self.iter().map(|(k, c)| (k.clone(), *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        if n >= 0 {
            v.truncate(n as usize);
        }
        v
    }
}
// zfill, title, center, capitalize as in Python.
pub trait AxStrFmtPy {
    fn ax_zfill(&self, n: i64) -> String;
    fn ax_title(&self) -> String;
    fn ax_center(&self, n: i64) -> String;
    fn ax_center_with(&self, n: i64, fill: impl AsRef<str>) -> String;
    fn ax_capitalize(&self) -> String;
    fn ax_count_sub(&self, sub: impl AsRef<str>) -> i64;
}
impl AxStrFmtPy for String {
    fn ax_zfill(&self, n: i64) -> String {
        let len = self.chars().count() as i64;
        if len >= n {
            return self.clone();
        }
        let zeros = "0".repeat((n - len) as usize);
        match self.chars().next() {
            Some(c @ ('-' | '+')) => format!("{c}{zeros}{}", &self[1..]),
            _ => format!("{zeros}{self}"),
        }
    }
    fn ax_title(&self) -> String {
        let mut out = String::new();
        let mut prev = false;
        for c in self.chars() {
            if c.is_alphabetic() {
                if prev { out.extend(c.to_lowercase()) } else { out.extend(c.to_uppercase()) }
                prev = true;
            } else {
                out.push(c);
                prev = false;
            }
        }
        out
    }
    fn ax_center(&self, n: i64) -> String {
        self.ax_center_with(n, " ")
    }
    fn ax_center_with(&self, n: i64, fill: impl AsRef<str>) -> String {
        let len = self.chars().count() as i64;
        if len >= n {
            return self.clone();
        }
        let marg = n - len;
        let left = marg / 2 + (marg & n & 1);
        let f = fill.as_ref();
        format!("{}{self}{}", f.repeat(left as usize), f.repeat((marg - left) as usize))
    }
    fn ax_count_sub(&self, sub: impl AsRef<str>) -> i64 {
        let sub = sub.as_ref();
        if sub.is_empty() { self.chars().count() as i64 + 1 } else { self.matches(sub).count() as i64 }
    }
    fn ax_capitalize(&self) -> String {
        let mut cs = self.chars();
        match cs.next() {
            Some(c) => c.to_uppercase().chain(cs.flat_map(|c| c.to_lowercase())).collect(),
            None => String::new(),
        }
    }
}
// sys.exit(n) and sys.exit("message").
pub trait AxExit {
    fn ax_exit_with(&self) -> !;
}
impl AxExit for i64 {
    fn ax_exit_with(&self) -> ! {
        std::process::exit(*self as i32)
    }
}
impl AxExit for String {
    fn ax_exit_with(&self) -> ! {
        eprintln!("{self}");
        std::process::exit(1)
    }
}
impl AxExit for &str {
    fn ax_exit_with(&self) -> ! {
        eprintln!("{self}");
        std::process::exit(1)
    }
}
// next(it, default) on a list being read from the front.
pub trait AxNextOr<T> {
    fn ax_next_or(&mut self, d: T) -> T;
    fn ax_next_or_empty(&mut self) -> T;
}
impl<T: Default> AxNextOr<T> for Vec<T> {
    fn ax_next_or(&mut self, d: T) -> T {
        if self.is_empty() { d } else { self.remove(0) }
    }
    fn ax_next_or_empty(&mut self) -> T {
        if self.is_empty() { T::default() } else { self.remove(0) }
    }
}
// next(gen, None): the first item, or Python's None (in the slot type's own form) when there is none.
pub trait AxNextOrNone<T> {
    fn ax_next_or_none(&mut self) -> T;
}
impl<T: AxNone> AxNextOrNone<T> for Vec<T> {
    fn ax_next_or_none(&mut self) -> T {
        if self.is_empty() { T::ax_none() } else { self.remove(0) }
    }
}
// Python's d2 - d1 between dates: `.days` gives the number of days between them.
impl std::ops::Sub for Date {
    type Output = Date;
    fn sub(self, o: Date) -> Date {
        Date { days: self.days - o.days }
    }
}
// csv.reader(...) over a whole text or a list of lines.
pub trait AxCsvRows {
    fn ax_csv_rows(&self) -> Vec<Vec<String>>;
}
impl AxCsvRows for String {
    fn ax_csv_rows(&self) -> Vec<Vec<String>> {
        csv(self.clone())
    }
}
impl AxCsvRows for Vec<String> {
    fn ax_csv_rows(&self) -> Vec<Vec<String>> {
        csv(self.join("\n"))
    }
}
// csv.DictReader(src): each row after the header as a dict from column name to field, in column order.
// Blank lines are skipped. A row with fewer or more fields than the header has None values (or a None key)
// in CPython, which a dict of strings cannot hold, so it stops with an error instead.
pub trait AxCsvDicts {
    fn ax_csv_dicts(&self) -> Vec<AxM<String, String>>;
}
fn ax_csv_dicts_of(rows: Vec<Vec<String>>) -> Vec<AxM<String, String>> {
    let mut it = rows.into_iter().filter(|r| !r.is_empty());
    let Some(head) = it.next() else { return Vec::new() };
    it.map(|r| {
        if r.len() != head.len() {
            panic!("ValueError: csv.DictReader row has {} fields where the header has {} (CPython fills them with None)", r.len(), head.len());
        }
        let mut m = AxM::ordered();
        for (k, v) in head.iter().zip(r) {
            m.insert(k.clone(), v);
        }
        m
    })
    .collect()
}
impl AxCsvDicts for String {
    fn ax_csv_dicts(&self) -> Vec<AxM<String, String>> {
        ax_csv_dicts_of(csv(self.clone()))
    }
}
impl AxCsvDicts for Vec<String> {
    fn ax_csv_dicts(&self) -> Vec<AxM<String, String>> {
        ax_csv_dicts_of(csv(self.join("\n")))
    }
}
// Python's strip family: rstrip()/lstrip() trim whitespace at one end; strip(cs)/rstrip(cs)/lstrip(cs)
// trim any of the characters in cs. isupper/islower/isalnum as in Python.
pub trait AxStrPy {
    fn ax_trim_of(&self, cs: impl AsRef<str>) -> String;
    fn ax_trim_end(&self) -> String;
    fn ax_trim_end_of(&self, cs: impl AsRef<str>) -> String;
    fn ax_trim_start(&self) -> String;
    fn ax_trim_start_of(&self, cs: impl AsRef<str>) -> String;
    fn ax_is_upper(&self) -> bool;
    fn ax_is_lower(&self) -> bool;
    fn ax_is_alnum(&self) -> bool;
}
impl AxStrPy for String {
    fn ax_trim_of(&self, cs: impl AsRef<str>) -> String {
        let cs = cs.as_ref();
        self.trim_matches(|c| cs.contains(c)).to_string()
    }
    fn ax_trim_end(&self) -> String {
        self.trim_end().to_string()
    }
    fn ax_trim_end_of(&self, cs: impl AsRef<str>) -> String {
        let cs = cs.as_ref();
        self.trim_end_matches(|c| cs.contains(c)).to_string()
    }
    fn ax_trim_start(&self) -> String {
        self.trim_start().to_string()
    }
    fn ax_trim_start_of(&self, cs: impl AsRef<str>) -> String {
        let cs = cs.as_ref();
        self.trim_start_matches(|c| cs.contains(c)).to_string()
    }
    fn ax_is_upper(&self) -> bool {
        self.chars().any(|c| c.is_alphabetic()) && !self.chars().any(|c| c.is_lowercase())
    }
    fn ax_is_lower(&self) -> bool {
        self.chars().any(|c| c.is_alphabetic()) && !self.chars().any(|c| c.is_uppercase())
    }
    fn ax_is_alnum(&self) -> bool {
        !self.is_empty() && self.chars().all(|c| c.is_alphanumeric())
    }
}
pub trait AxStrRev {
    fn ax_rev(&self) -> String;
    fn ax_index_of(&self, s: impl AsRef<str>) -> i64;
}
impl AxStrRev for String {
    fn ax_rev(&self) -> String {
        self.chars().rev().collect()
    }
    fn ax_index_of(&self, s: impl AsRef<str>) -> i64 {
        match self.find(s.as_ref()) {
            Some(b) => self[..b].chars().count() as i64,
            None => -1,
        }
    }
}
// Python's enumerate(xs, 1) counts from 1.
pub trait AxEnumFrom<T> {
    fn ax_enumerate_from(&self, n: i64) -> Vec<(i64, T)>;
}
impl<T: Clone> AxEnumFrom<T> for Vec<T> {
    fn ax_enumerate_from(&self, n: i64) -> Vec<(i64, T)> {
        self.iter().cloned().enumerate().map(|(i, x)| (i as i64 + n, x)).collect()
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
// Python's xs[a:b:k] (AX_NOSL for a missing a or b): the indexes picked, in order.
pub const AX_NOSL: i64 = i64::MIN + 1;
pub fn ax_slice_idx(n: usize, a: i64, b: i64, k: i64) -> Vec<usize> {
    if k == 0 {
        panic!("ValueError: slice step cannot be zero");
    }
    let n = n as i64;
    let norm = |x: i64, dflt: i64| -> i64 {
        if x == AX_NOSL {
            return dflt;
        }
        let x = if x < 0 { x + n } else { x };
        if k > 0 { x.clamp(0, n) } else { x.clamp(-1, n - 1) }
    };
    let (mut i, j) = if k > 0 { (norm(a, 0), norm(b, n)) } else { (norm(a, n - 1), norm(b, -1)) };
    let mut out = vec![];
    while (k > 0 && i < j) || (k < 0 && i > j) {
        out.push(i as usize);
        i += k;
    }
    out
}
pub trait AxSliceStep {
    fn ax_slice_step(&self, a: i64, b: i64, k: i64) -> Self;
}
impl<T: Clone> AxSliceStep for Vec<T> {
    fn ax_slice_step(&self, a: i64, b: i64, k: i64) -> Vec<T> {
        ax_slice_idx(self.len(), a, b, k).into_iter().map(|i| self[i].clone()).collect()
    }
}
impl AxSliceStep for String {
    fn ax_slice_step(&self, a: i64, b: i64, k: i64) -> String {
        let cs: Vec<char> = self.chars().collect();
        ax_slice_idx(cs.len(), a, b, k).into_iter().map(|i| cs[i]).collect()
    }
}
// [*xs for xs in xss] joins the lists; on strings it gives their characters.
pub trait AxFlatten<T> {
    fn ax_flatten(&self) -> Vec<T>;
}
impl<T: Clone> AxFlatten<T> for Vec<Vec<T>> {
    fn ax_flatten(&self) -> Vec<T> {
        self.concat()
    }
}
impl AxFlatten<String> for Vec<String> {
    fn ax_flatten(&self) -> Vec<String> {
        self.iter().flat_map(|s| s.chars().map(|c| c.to_string())).collect()
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
// The second field is the number of decimal places written, as Python's Decimal keeps it:
// "1.50" stays 1.50 and quantize sets it. Only the value counts when comparing or hashing.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dec(pub i128, pub u8);
impl PartialEq for Dec {
    fn eq(&self, o: &Dec) -> bool {
        self.cmp(o) == std::cmp::Ordering::Equal
    }
}
impl Eq for Dec {}
impl PartialOrd for Dec {
    fn partial_cmp(&self, o: &Dec) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Dec {
    fn cmp(&self, o: &Dec) -> std::cmp::Ordering {
        if !self.is_frac() && !o.is_frac() {
            return self.0.cmp(&o.0);
        }
        // None (ax_none) next to a Fraction: equal only to None.
        let none = |x: &Dec| x.0 == i128::MIN && !x.is_frac();
        if none(self) || none(o) {
            return none(o).cmp(&none(self));
        }
        let ((a, b), (c, d)) = (self.nd(), o.nd());
        match (a.checked_mul(d), c.checked_mul(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            _ => (a as f64 / b as f64).partial_cmp(&(c as f64 / d as f64)).unwrap_or(std::cmp::Ordering::Equal),
        }
    }
}
impl Hash for Dec {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        // By the reduced fraction, so a Fraction and a Decimal of the same value hash alike.
        self.frac_parts().hash(h)
    }
}
// A dec compares directly with an int: `price > 0`.
impl PartialEq<i64> for Dec {
    fn eq(&self, o: &i64) -> bool {
        let (n, d) = self.nd();
        n == (*o as i128) * d
    }
}
impl PartialOrd<i64> for Dec {
    fn partial_cmp(&self, o: &i64) -> Option<std::cmp::Ordering> {
        let (n, d) = self.nd();
        n.partial_cmp(&((*o as i128) * d))
    }
}
const DEC_ONE: i128 = 1_000_000_000;
// Python's Fraction is a Dec marked FRAC whose i128 holds a reduced numerator (high 64 bits)
// over a positive denominator (low 64 bits). Arithmetic runs in i128 and reduces; a result whose
// numerator or denominator needs more than 64 bits stops the program loudly rather than rounding.
const FRAC: u8 = 255;
fn frac_fail(what: String) -> ! {
    panic!("{what} is too large for Brevi's typed Fraction (numerator and denominator must fit in 64 bits); run this program with CPython")
}
fn frac_mul(a: i128, b: i128) -> i128 {
    a.checked_mul(b).unwrap_or_else(|| frac_fail("a Fraction".into()))
}
fn frac_add(a: i128, b: i128) -> i128 {
    a.checked_add(b).unwrap_or_else(|| frac_fail("a Fraction".into()))
}
// n/d reduced, as a Fraction.
pub fn frac_new(n: i128, d: i128) -> Dec {
    if d == 0 {
        panic!("Fraction({n}, 0)");
    }
    let g = ax_gcd128(n, d).max(1);
    let (mut n, mut d) = (n / g, d / g);
    if d < 0 {
        n = -n;
        d = -d;
    }
    if n < i64::MIN as i128 || n > i64::MAX as i128 || d > i64::MAX as i128 {
        frac_fail(format!("Fraction({n}, {d})"))
    }
    Dec(((n as i64 as i128) << 64) | d, FRAC)
}
fn frac_pair(a: Dec, b: Dec) -> Option<((i128, i128), (i128, i128))> {
    if a.is_frac() || b.is_frac() { Some((a.frac_of().nd(), b.frac_of().nd())) } else { None }
}
fn frac_floor_div(x: i128, y: i128) -> i128 {
    if y == 0 {
        panic!("division by zero");
    }
    x / y - (x % y != 0 && (x < 0) != (y < 0)) as i128
}
fn ax_gcd128(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { ax_gcd128(b, a % b) }
}
impl Dec {
    pub fn ax_frac(&self) -> Dec {
        self.frac_of()
    }
    // Fraction(a, b)
    pub fn ax_frac_div(&self, o: Dec) -> Dec {
        self.frac_of() / o.frac_of()
    }
    fn is_frac(&self) -> bool {
        self.1 == FRAC
    }
    // The value as numerator over a positive denominator: a Fraction's own pair, or a Decimal's units over DEC_ONE.
    fn nd(&self) -> (i128, i128) {
        if self.is_frac() { (self.0 >> 64, self.0 & 0xFFFF_FFFF_FFFF_FFFF) } else { (self.0, DEC_ONE) }
    }
    fn frac_of(&self) -> Dec {
        if self.is_frac() {
            return *self;
        }
        frac_new(self.0, DEC_ONE)
    }
    // (numerator, denominator), reduced.
    fn frac_parts(&self) -> (i128, i128) {
        let (n, d) = self.nd();
        let g = ax_gcd128(n, d).max(1);
        (n / g, d / g)
    }
    // Python's x.numerator / x.denominator.
    pub fn ax_numerator(&self) -> i64 {
        self.frac_parts().0 as i64
    }
    pub fn ax_denominator(&self) -> i64 {
        self.frac_parts().1 as i64
    }
    fn parse(s: &str) -> R<Dec> {
        let t = s.trim();
        // Fraction("3/4"): an exact ratio of two whole numbers.
        if let Some((a, b)) = t.split_once('/') {
            let whole = |x: &str| x.strip_prefix(['-', '+']).unwrap_or(x).chars().all(|c| c.is_ascii_digit()) && !x.is_empty();
            if whole(a) && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()) {
                if let (Ok(n), Ok(d)) = (a.parse::<i128>(), b.parse::<i128>()) {
                    if d == 0 {
                        panic!("Fraction({n}, 0)");
                    }
                    return Ok(frac_new(n, d));
                }
            }
            return Err(AxErr(format!("Invalid literal for Fraction: {:?}", s)));
        }
        let (neg, body) = match t.strip_prefix('-') { Some(b) => (true, b), None => (false, t.strip_prefix('+').unwrap_or(t)) };
        let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
        let ok = !(ip.is_empty() && fp.is_empty()) && ip.chars().all(|c| c.is_ascii_digit()) && fp.chars().all(|c| c.is_ascii_digit()) && fp.len() <= 9;
        if !ok {
            return Err(AxErr(format!("not a decimal: {:?}", s)));
        }
        let i: i128 = if ip.is_empty() { 0 } else { ip.parse().map_err(|_| AxErr(format!("decimal too large: {:?}", s)))? };
        let f: i128 = format!("{:0<9}", fp).parse().unwrap_or(0);
        let v = i * DEC_ONE + f;
        Ok(Dec(if neg { -v } else { v }, fp.len() as u8))
    }
    pub fn ax_fmt(&self, decimals: i64) -> String {
        let d = decimals.clamp(0, 9) as u32;
        let (n, unit) = self.nd();
        let v = frac_mul(n, 10i128.pow(d));
        let (q, r) = (v.div_euclid(unit), v.rem_euclid(unit));
        // Round half to even, like Python's Decimal.
        let q = if r * 2 > unit || (r * 2 == unit && q % 2 != 0) { q + 1 } else { q };
        let neg = q < 0;
        let a = q.abs();
        let p = 10i128.pow(d);
        let s = if d == 0 { format!("{}", a / p) } else { format!("{}.{:0w$}", a / p, a % p, w = d as usize) };
        if neg { format!("-{s}") } else { s }
    }
    pub fn ax_abs(&self) -> Dec {
        if self.is_frac() {
            let (n, d) = self.nd();
            return frac_new(n.abs(), d);
        }
        Dec(self.0.abs(), self.1)
    }
    // Python's math.floor / math.ceil (and `x.__floor__()`) on a Decimal or Fraction: an int.
    pub fn ax_floor(&self) -> i64 {
        let (n, d) = self.nd();
        n.div_euclid(d) as i64
    }
    pub fn ax_ceil(&self) -> i64 {
        let (n, d) = self.nd();
        -((-n).div_euclid(d)) as i64
    }
    pub fn ax_float(&self) -> f64 {
        let (n, d) = self.nd();
        n as f64 / d as f64
    }
    pub fn ax_int(&self) -> i64 {
        let (n, d) = self.nd();
        (n / d) as i64
    }
    pub fn ax_round(&self) -> Dec {
        Dec::parse(&self.ax_fmt(0)).unwrap()
    }
    // Python's d.quantize(Decimal("0.01")) (half to even), or with rounding=ROUND_HALF_UP (half away from zero).
    pub fn ax_quantize(&self, q: Dec, half_up: bool) -> Dec {
        let d = q.1.min(9) as u32;
        let unit = 10i128.pow(9 - d);
        let (n, r) = (self.0.div_euclid(unit), self.0.rem_euclid(unit));
        let up = if half_up {
            if self.0 >= 0 { r * 2 >= unit } else { r * 2 > unit }
        } else {
            r * 2 > unit || (r * 2 == unit && n % 2 != 0)
        };
        Dec((n + up as i128) * unit, d as u8)
    }
    // Fewest places that write the value exactly.
    fn places(&self) -> u8 {
        let mut v = self.0.abs();
        let mut p = 9u8;
        while p > 0 && v % 10 == 0 {
            v /= 10;
            p -= 1;
        }
        p
    }
}
impl Show for Dec {
    fn show(&self) -> String {
        if self.0 == i128::MIN && ax_py_on() {
            return "None".into();
        }
        if self.is_frac() {
            let (n, d) = self.frac_parts();
            return if d == 1 { format!("{n}") } else { format!("{n}/{d}") };
        }
        if ax_py_on() {
            return self.ax_fmt(self.1.max(self.places()) as i64);
        }
        let s = self.ax_fmt(9);
        let s = s.trim_end_matches('0');
        s.trim_end_matches('.').to_string()
    }
}
impl Plus for Dec {
    type Out = Dec;
    fn plus(self, o: Dec) -> Dec {
        if let Some(((a, b), (c, d))) = frac_pair(self, o) {
            return frac_new(frac_add(frac_mul(a, d), frac_mul(c, b)), frac_mul(b, d));
        }
        Dec(self.0 + o.0, self.1.max(o.1))
    }
}
impl std::ops::Sub for Dec {
    type Output = Dec;
    fn sub(self, o: Dec) -> Dec {
        if let Some(((a, b), (c, d))) = frac_pair(self, o) {
            return frac_new(frac_add(frac_mul(a, d), -frac_mul(c, b)), frac_mul(b, d));
        }
        Dec(self.0 - o.0, self.1.max(o.1))
    }
}
impl std::ops::Mul for Dec {
    type Output = Dec;
    fn mul(self, o: Dec) -> Dec {
        if let Some(((a, b), (c, d))) = frac_pair(self, o) {
            // Cross-reduce first so the products stay small.
            let (g1, g2) = (ax_gcd128(a, d).max(1), ax_gcd128(c, b).max(1));
            return frac_new(frac_mul(a / g1, c / g2), frac_mul(b / g2, d / g1));
        }
        Dec(self.0 * o.0 / DEC_ONE, (self.1 + o.1).min(9))
    }
}
impl std::ops::Div for Dec {
    type Output = Dec;
    fn div(self, o: Dec) -> Dec {
        if let Some(((a, b), (c, d))) = frac_pair(self, o) {
            if c == 0 {
                panic!("Fraction({}, 0)", a * d);
            }
            let (g1, g2) = (ax_gcd128(a, c).max(1), ax_gcd128(d, b).max(1));
            return frac_new(frac_mul(a / g1, d / g2), frac_mul(b / g2, c / g1));
        }
        if o.0 == 0 {
            panic!("division by zero");
        }
        let v = self.0 * DEC_ONE / o.0;
        // An exact quotient keeps Python's places (a's minus b's, or as many as it needs); otherwise all 9.
        let r = Dec(v, 0);
        let p = if (self.0 * DEC_ONE) % o.0 == 0 { (self.1.saturating_sub(o.1)).max(r.places()) } else { 9 };
        Dec(v, p)
    }
}
// Ints mix with decs in arithmetic: `0 - amt`, `price * 2`, `total += 1`.
fn dec_of(n: i64) -> Dec {
    Dec(n as i128 * DEC_ONE, 0)
}
impl Plus<i64> for Dec {
    type Out = Dec;
    fn plus(self, o: i64) -> Dec {
        if self.is_frac() {
            return self.plus(dec_of(o));
        }
        Dec(self.0 + dec_of(o).0, self.1)
    }
}
impl Plus<Dec> for i64 {
    type Out = Dec;
    fn plus(self, o: Dec) -> Dec {
        if o.is_frac() {
            return dec_of(self).plus(o);
        }
        Dec(dec_of(self).0 + o.0, o.1)
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
        if self.is_frac() {
            let (n, d) = self.nd();
            return frac_new(-n, d);
        }
        Dec(-self.0, self.1)
    }
}
impl std::ops::SubAssign for Dec {
    fn sub_assign(&mut self, o: Dec) {
        *self = *self - o
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
        if self.iter().any(|d| d.is_frac()) {
            return self.iter().fold(frac_new(0, 1), |a, b| a.plus(*b));
        }
        Dec(self.iter().map(|d| d.0).sum(), self.iter().map(|d| d.1).max().unwrap_or(0))
    }
}
pub trait AxToDec {
    fn ax_dec(&self) -> Dec;
}
impl AxToDec for i64 {
    fn ax_dec(&self) -> Dec {
        Dec(*self as i128 * DEC_ONE, 0)
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
// ---- hash-indexed list: a Python set (or a list only added to, searched and walked) whose `in` is O(1) ----
// The items stay in a Vec in the order they were added, so everything that reads the list sees what it
// would see in a plain Vec; past a few items a count of each item answers `in` without a scan.
pub struct AxSet<T> {
    v: Vec<T>,
    // Built on the first search past AX_SET_SCAN items, then kept up to date; a cell so `x in s` can
    // build it through a shared reference.
    n: std::cell::UnsafeCell<Option<HashMap<T, u32, BuildHasherDefault<AxHasher>>>>,
}
const AX_SET_SCAN: usize = 12;
impl<T: Hash + Eq + Clone> AxSet<T> {
    fn ax_ix(&self) -> &HashMap<T, u32, BuildHasherDefault<AxHasher>> {
        // Only this type touches the cell, and no reference into it outlives a call.
        let n = unsafe { &mut *self.n.get() };
        if n.is_none() {
            let mut m: HashMap<T, u32, BuildHasherDefault<AxHasher>> = HashMap::default();
            m.reserve(self.v.len());
            for x in &self.v {
                *m.entry(x.clone()).or_insert(0) += 1;
            }
            *n = Some(m);
        }
        n.as_ref().unwrap()
    }
    fn ax_drop(&mut self, x: &T) {
        if let Some(m) = self.n.get_mut().as_mut() {
            if let Some(c) = m.get_mut(x) {
                *c -= 1;
                if *c == 0 {
                    m.remove(x);
                }
            }
        }
    }
    pub fn ax_contains(&self, x: &T) -> bool {
        if self.v.len() <= AX_SET_SCAN && unsafe { (*self.n.get()).is_none() } {
            return self.v.iter().any(|y| y == x);
        }
        self.ax_ix().contains_key(x)
    }
    pub fn ax_push(&mut self, x: T) {
        if let Some(m) = self.n.get_mut().as_mut() {
            *m.entry(x.clone()).or_insert(0) += 1;
        }
        self.v.push(x);
    }
    pub fn ax_pop(&mut self) -> T {
        let x = self.v.pop().expect("pop from empty list");
        self.ax_drop(&x);
        x
    }
    pub fn ax_remove(&mut self, i: i64) -> T {
        let j = ax_idx(i, self.v.len());
        let x = self.v.remove(j);
        self.ax_drop(&x);
        x
    }
}
impl<T> std::ops::Deref for AxSet<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.v
    }
}
impl<T> From<Vec<T>> for AxSet<T> {
    fn from(v: Vec<T>) -> Self {
        AxSet { v, n: Default::default() }
    }
}
impl<T> Default for AxSet<T> {
    fn default() -> Self {
        AxSet { v: Vec::new(), n: Default::default() }
    }
}
impl<T: Clone> Clone for AxSet<T> {
    // A copy starts without the index; it is built again when searched.
    fn clone(&self) -> Self {
        AxSet { v: self.v.clone(), n: Default::default() }
    }
}
impl<T: PartialEq> PartialEq for AxSet<T> {
    fn eq(&self, o: &Self) -> bool {
        self.v == o.v
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for AxSet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.v.fmt(f)
    }
}
impl<T: Show + 'static> Show for AxSet<T> {
    fn show(&self) -> String {
        self.v.show()
    }
}
impl<T> AxTruth for AxSet<T> {
    fn ax_truth(&self) -> bool {
        !self.v.is_empty()
    }
}
impl<T> AxEach for AxSet<T> {
    type It = std::vec::IntoIter<T>;
    fn ax_each(self) -> Self::It {
        self.v.into_iter()
    }
}
impl<'a, T> IntoIterator for &'a AxSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.v.iter()
    }
}
impl<T: Hash + Eq + Clone> AxIn<T> for AxSet<T> {
    fn ax_in(&self, x: &T) -> bool {
        self.ax_contains(x)
    }
}
impl<T: Hash + Eq + Clone> Plus<Vec<T>> for AxSet<T> {
    type Out = AxSet<T>;
    fn plus(mut self, o: Vec<T>) -> AxSet<T> {
        for x in o {
            self.ax_push(x);
        }
        self
    }
}
impl<T: PartialOrd> PartialOrd for AxSet<T> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        self.v.partial_cmp(&o.v)
    }
}
impl<T: Hash> Hash for AxSet<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.v.hash(h)
    }
}
impl<T> AxToJson for AxSet<T> where Vec<T>: AxToJson {
    fn ax_json(&self) -> Json {
        self.v.ax_json()
    }
}
// `(x,) = xs`: the only item; Python's ValueError otherwise.
pub fn ax_one<I: IntoIterator>(xs: I) -> I::Item {
    let mut it = xs.into_iter();
    let first = it.next();
    match (first, it.next()) {
        (Some(x), None) => x,
        (None, _) => panic!("not enough values to unpack (expected 1, got 0)"),
        (Some(_), Some(_)) => panic!("too many values to unpack (expected 1)"),
    }
}
// ---- t[a:b] with number bounds: on a tuple, a shorter tuple; on a list, string or JSON value, as
// ax_slice. B = usize::MAX is "to the end". ----
pub trait AxTSlice<const A: usize, const B: usize> {
    type Out;
    fn ax_tslice(&self) -> Self::Out;
}
impl<T: Clone, const A: usize, const B: usize> AxTSlice<A, B> for Vec<T> {
    type Out = Vec<T>;
    fn ax_tslice(&self) -> Vec<T> {
        AxList::ax_slice(self, A as i64, if B == usize::MAX { i64::MAX } else { B as i64 })
    }
}
impl<'a, T: AxTSlice<A, B>, const A: usize, const B: usize> AxTSlice<A, B> for &'a T {
    type Out = T::Out;
    fn ax_tslice(&self) -> T::Out {
        (**self).ax_tslice()
    }
}
impl<'a, T: AxTSlice<A, B>, const A: usize, const B: usize> AxTSlice<A, B> for &'a mut T {
    type Out = T::Out;
    fn ax_tslice(&self) -> T::Out {
        (**self).ax_tslice()
    }
}
impl<const A: usize, const B: usize> AxTSlice<A, B> for String {
    type Out = String;
    fn ax_tslice(&self) -> String {
        AxStr::ax_slice(self, A as i64, if B == usize::MAX { i64::MAX } else { B as i64 })
    }
}
impl<const A: usize, const B: usize> AxTSlice<A, B> for Json {
    type Out = Json;
    fn ax_tslice(&self) -> Json {
        self.ax_slice(A as i64, if B == usize::MAX { i64::MAX } else { B as i64 })
    }
}
impl<A: Clone, B: Clone> AxTSlice<0, 2> for (A, B) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone> AxTSlice<0, {usize::MAX}> for (A, B) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone, C: Clone> AxTSlice<0, 2> for (A, B, C) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone, C: Clone> AxTSlice<0, 3> for (A, B, C) { type Out = (A, B, C); fn ax_tslice(&self) -> (A, B, C) { (self.0.clone(), self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone> AxTSlice<0, {usize::MAX}> for (A, B, C) { type Out = (A, B, C); fn ax_tslice(&self) -> (A, B, C) { (self.0.clone(), self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone> AxTSlice<1, 3> for (A, B, C) { type Out = (B, C); fn ax_tslice(&self) -> (B, C) { (self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone> AxTSlice<1, {usize::MAX}> for (A, B, C) { type Out = (B, C); fn ax_tslice(&self) -> (B, C) { (self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<0, 2> for (A, B, C, D) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<0, 3> for (A, B, C, D) { type Out = (A, B, C); fn ax_tslice(&self) -> (A, B, C) { (self.0.clone(), self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<0, 4> for (A, B, C, D) { type Out = (A, B, C, D); fn ax_tslice(&self) -> (A, B, C, D) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<0, {usize::MAX}> for (A, B, C, D) { type Out = (A, B, C, D); fn ax_tslice(&self) -> (A, B, C, D) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<1, 3> for (A, B, C, D) { type Out = (B, C); fn ax_tslice(&self) -> (B, C) { (self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<1, 4> for (A, B, C, D) { type Out = (B, C, D); fn ax_tslice(&self) -> (B, C, D) { (self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<1, {usize::MAX}> for (A, B, C, D) { type Out = (B, C, D); fn ax_tslice(&self) -> (B, C, D) { (self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<2, 4> for (A, B, C, D) { type Out = (C, D); fn ax_tslice(&self) -> (C, D) { (self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone> AxTSlice<2, {usize::MAX}> for (A, B, C, D) { type Out = (C, D); fn ax_tslice(&self) -> (C, D) { (self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<0, 2> for (A, B, C, D, E) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<0, 3> for (A, B, C, D, E) { type Out = (A, B, C); fn ax_tslice(&self) -> (A, B, C) { (self.0.clone(), self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<0, 4> for (A, B, C, D, E) { type Out = (A, B, C, D); fn ax_tslice(&self) -> (A, B, C, D) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<0, 5> for (A, B, C, D, E) { type Out = (A, B, C, D, E); fn ax_tslice(&self) -> (A, B, C, D, E) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<0, {usize::MAX}> for (A, B, C, D, E) { type Out = (A, B, C, D, E); fn ax_tslice(&self) -> (A, B, C, D, E) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<1, 3> for (A, B, C, D, E) { type Out = (B, C); fn ax_tslice(&self) -> (B, C) { (self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<1, 4> for (A, B, C, D, E) { type Out = (B, C, D); fn ax_tslice(&self) -> (B, C, D) { (self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<1, 5> for (A, B, C, D, E) { type Out = (B, C, D, E); fn ax_tslice(&self) -> (B, C, D, E) { (self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<1, {usize::MAX}> for (A, B, C, D, E) { type Out = (B, C, D, E); fn ax_tslice(&self) -> (B, C, D, E) { (self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<2, 4> for (A, B, C, D, E) { type Out = (C, D); fn ax_tslice(&self) -> (C, D) { (self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<2, 5> for (A, B, C, D, E) { type Out = (C, D, E); fn ax_tslice(&self) -> (C, D, E) { (self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<2, {usize::MAX}> for (A, B, C, D, E) { type Out = (C, D, E); fn ax_tslice(&self) -> (C, D, E) { (self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<3, 5> for (A, B, C, D, E) { type Out = (D, E); fn ax_tslice(&self) -> (D, E) { (self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone> AxTSlice<3, {usize::MAX}> for (A, B, C, D, E) { type Out = (D, E); fn ax_tslice(&self) -> (D, E) { (self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, 2> for (A, B, C, D, E, F) { type Out = (A, B); fn ax_tslice(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, 3> for (A, B, C, D, E, F) { type Out = (A, B, C); fn ax_tslice(&self) -> (A, B, C) { (self.0.clone(), self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, 4> for (A, B, C, D, E, F) { type Out = (A, B, C, D); fn ax_tslice(&self) -> (A, B, C, D) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, 5> for (A, B, C, D, E, F) { type Out = (A, B, C, D, E); fn ax_tslice(&self) -> (A, B, C, D, E) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, 6> for (A, B, C, D, E, F) { type Out = (A, B, C, D, E, F); fn ax_tslice(&self) -> (A, B, C, D, E, F) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<0, {usize::MAX}> for (A, B, C, D, E, F) { type Out = (A, B, C, D, E, F); fn ax_tslice(&self) -> (A, B, C, D, E, F) { (self.0.clone(), self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<1, 3> for (A, B, C, D, E, F) { type Out = (B, C); fn ax_tslice(&self) -> (B, C) { (self.1.clone(), self.2.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<1, 4> for (A, B, C, D, E, F) { type Out = (B, C, D); fn ax_tslice(&self) -> (B, C, D) { (self.1.clone(), self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<1, 5> for (A, B, C, D, E, F) { type Out = (B, C, D, E); fn ax_tslice(&self) -> (B, C, D, E) { (self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<1, 6> for (A, B, C, D, E, F) { type Out = (B, C, D, E, F); fn ax_tslice(&self) -> (B, C, D, E, F) { (self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<1, {usize::MAX}> for (A, B, C, D, E, F) { type Out = (B, C, D, E, F); fn ax_tslice(&self) -> (B, C, D, E, F) { (self.1.clone(), self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<2, 4> for (A, B, C, D, E, F) { type Out = (C, D); fn ax_tslice(&self) -> (C, D) { (self.2.clone(), self.3.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<2, 5> for (A, B, C, D, E, F) { type Out = (C, D, E); fn ax_tslice(&self) -> (C, D, E) { (self.2.clone(), self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<2, 6> for (A, B, C, D, E, F) { type Out = (C, D, E, F); fn ax_tslice(&self) -> (C, D, E, F) { (self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<2, {usize::MAX}> for (A, B, C, D, E, F) { type Out = (C, D, E, F); fn ax_tslice(&self) -> (C, D, E, F) { (self.2.clone(), self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<3, 5> for (A, B, C, D, E, F) { type Out = (D, E); fn ax_tslice(&self) -> (D, E) { (self.3.clone(), self.4.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<3, 6> for (A, B, C, D, E, F) { type Out = (D, E, F); fn ax_tslice(&self) -> (D, E, F) { (self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<3, {usize::MAX}> for (A, B, C, D, E, F) { type Out = (D, E, F); fn ax_tslice(&self) -> (D, E, F) { (self.3.clone(), self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<4, 6> for (A, B, C, D, E, F) { type Out = (E, F); fn ax_tslice(&self) -> (E, F) { (self.4.clone(), self.5.clone()) } }
impl<A: Clone, B: Clone, C: Clone, D: Clone, E: Clone, F: Clone> AxTSlice<4, {usize::MAX}> for (A, B, C, D, E, F) { type Out = (E, F); fn ax_tslice(&self) -> (E, F) { (self.4.clone(), self.5.clone()) } }
// ---- sets of strings: CPython draws str hashes afresh each run (PYTHONHASHSEED), so a set of strings (or
// of tuples led by one) has no order of its own there, and any order the typed build walks it in is one a
// CPython run could give. typed_guard::check_soft lets such walks through; these marks make rustc prove the
// element type. ----
pub trait AxRandElem {}
impl AxRandElem for String {}
impl AxRandElem for &str {}
impl<T: AxRandElem> AxRandElem for Vec<T> {}
impl<A: AxRandElem> AxRandElem for (A,) {}
impl<A: AxRandElem, B> AxRandElem for (A, B) {}
impl<A: AxRandElem, B, C> AxRandElem for (A, B, C) {}
impl<A: AxRandElem, B, C, D> AxRandElem for (A, B, C, D) {}
impl<A: AxRandElem, B, C, D, E> AxRandElem for (A, B, C, D, E) {}
pub trait AxRandSet {}
impl<T: AxRandElem> AxRandSet for Vec<T> {}
impl<T: AxRandElem> AxRandSet for AxSet<T> {}
impl<T: AxRandElem> AxRandSet for AxQueue<T> {}
impl<K: AxRandElem, V> AxRandSet for AxM<K, V> {}
// set(word): its letters.
impl AxRandSet for String {}
pub trait AxRandHolder {}
impl<K, S: AxRandSet> AxRandHolder for AxM<K, S> {}
impl<S: AxRandSet> AxRandHolder for Vec<S> {}
pub fn ax_rand_set<T: AxRandSet + ?Sized>(_: &T) {}
pub fn ax_rand_holder<T: AxRandHolder + ?Sized>(_: &T) {}
// ---- queue: a Python deque (or a list taken from the front) whose popleft is O(1) ----
pub struct AxQueue<T>(std::collections::VecDeque<T>);
impl<T: Clone + PartialEq> AxQueue<T> {
    pub fn ax_push(&mut self, x: T) {
        self.0.push_back(x)
    }
    pub fn ax_pop(&mut self) -> T {
        self.0.pop_back().expect("pop from an empty deque")
    }
    pub fn ax_pop_at(&mut self, i: i64) -> T {
        if i == 0 {
            return self.0.pop_front().expect("pop from an empty deque");
        }
        let j = ax_idx(i, self.0.len());
        self.0.remove(j).unwrap()
    }
    pub fn ax_insert(&mut self, i: i64, x: T) {
        if i == 0 {
            return self.0.push_front(x);
        }
        let j = if i == self.0.len() as i64 { self.0.len() } else { ax_idx(i, self.0.len()) };
        self.0.insert(j, x)
    }
    pub fn ax_len(&self) -> i64 {
        self.0.len() as i64
    }
    pub fn ax_is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn ax_contains(&self, x: &T) -> bool {
        self.0.iter().any(|y| y == x)
    }
    pub fn ax_ref(&self, i: &i64) -> &T {
        &self.0[ax_idx(*i, self.0.len())]
    }
}
impl<T> From<Vec<T>> for AxQueue<T> {
    fn from(v: Vec<T>) -> Self {
        AxQueue(v.into())
    }
}
impl<T> Default for AxQueue<T> {
    fn default() -> Self {
        AxQueue(Default::default())
    }
}
impl<T: Clone> Clone for AxQueue<T> {
    fn clone(&self) -> Self {
        AxQueue(self.0.clone())
    }
}
impl<T: PartialEq> PartialEq for AxQueue<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for AxQueue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T> AxTruth for AxQueue<T> {
    fn ax_truth(&self) -> bool {
        !self.0.is_empty()
    }
}
impl<T> AxEach for AxQueue<T> {
    type It = std::collections::vec_deque::IntoIter<T>;
    fn ax_each(self) -> Self::It {
        self.0.into_iter()
    }
}
impl<T: PartialEq> AxIn<T> for AxQueue<T> {
    fn ax_in(&self, x: &T) -> bool {
        self.0.iter().any(|y| y == x)
    }
}
impl<T> Plus<Vec<T>> for AxQueue<T> {
    type Out = AxQueue<T>;
    fn plus(mut self, o: Vec<T>) -> AxQueue<T> {
        self.0.extend(o);
        self
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
impl<V: Clone> AxIn<Json> for AxM<String, V> {
    fn ax_in(&self, x: &Json) -> bool {
        matches!(x, Json::Str { v } if self.contains_key(v))
    }
}
// `c in (R, Q)` on a tuple of like items.
impl<T: PartialEq> AxIn<T> for (T, T) {
    fn ax_in(&self, x: &T) -> bool {
        self.0 == *x || self.1 == *x
    }
}
impl<T: PartialEq> AxIn<T> for (T, T, T) {
    fn ax_in(&self, x: &T) -> bool {
        self.0 == *x || self.1 == *x || self.2 == *x
    }
}
impl<T: PartialEq> AxIn<T> for (T, T, T, T) {
    fn ax_in(&self, x: &T) -> bool {
        self.0 == *x || self.1 == *x || self.2 == *x || self.3 == *x
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
        if self.is_frac() {
            // round(Fraction, n) stays a Fraction.
            return Dec::parse(&self.ax_fmt(n.clamp(0, 9))).unwrap().frac_of();
        }
        Dec::parse(&self.ax_fmt(n.clamp(0, 9))).unwrap()
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
// A plain value put into a Python list that also holds other types.
pub fn ax_to_json<T: AxToJson>(v: T) -> Json {
    v.ax_json()
}
impl AxToJson for Json {
    fn ax_json(&self) -> Json {
        self.clone()
    }
}
impl AxToJson for i64 {
    fn ax_json(&self) -> Json {
        Json::Num { v: *self as f64, f: false }
    }
}
impl AxToJson for f64 {
    fn ax_json(&self) -> Json {
        Json::Num { v: *self, f: true }
    }
}
impl AxToJson for Dec {
    fn ax_json(&self) -> Json {
        Json::Num { v: self.ax_float(), f: true }
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
// A tuple is a JSON list, as json.dumps writes it.
impl<A: AxToJson, B: AxToJson> AxToJson for (A, B) {
    fn ax_json(&self) -> Json {
        ax_tuple_json(vec![self.0.ax_json(), self.1.ax_json()])
    }
}
impl<A: AxToJson, B: AxToJson, C: AxToJson> AxToJson for (A, B, C) {
    fn ax_json(&self) -> Json {
        ax_tuple_json(vec![self.0.ax_json(), self.1.ax_json(), self.2.ax_json()])
    }
}
impl<A: AxToJson, B: AxToJson, C: AxToJson, D: AxToJson> AxToJson for (A, B, C, D) {
    fn ax_json(&self) -> Json {
        ax_tuple_json(vec![self.0.ax_json(), self.1.ax_json(), self.2.ax_json(), self.3.ax_json()])
    }
}
impl<A: AxToJson, B: AxToJson, C: AxToJson, D: AxToJson, E: AxToJson> AxToJson for (A, B, C, D, E) {
    fn ax_json(&self) -> Json {
        ax_tuple_json(vec![self.0.ax_json(), self.1.ax_json(), self.2.ax_json(), self.3.ax_json(), self.4.ax_json()])
    }
}
impl<A: AxToJson, B: AxToJson, C: AxToJson, D: AxToJson, E: AxToJson, F: AxToJson> AxToJson for (A, B, C, D, E, F) {
    fn ax_json(&self) -> Json {
        ax_tuple_json(vec![self.0.ax_json(), self.1.ax_json(), self.2.ax_json(), self.3.ax_json(), self.4.ax_json(), self.5.ax_json()])
    }
}
// `total += c[3]` with c a JSON tuple: only a whole number keeps total an int, as in Python.
pub fn ax_jint(o: Json) -> i64 {
    match o {
        Json::Num { v, .. } if v.fract() == 0.0 && v.abs() < 1e15 => v as i64,
        o => panic!("cannot add {} to an int here", ax_json_py(&o, false)),
    }
}
// A Python form Brevi cannot give the same answer for: stop the run instead of printing something else.
pub fn ax_unsupported(what: String) -> String {
    panic!("{what}")
}
// A dynamic value given where an int or a str is taken.
pub fn ax_jas_int(o: Json) -> i64 {
    match o {
        Json::Num { v, f } if v.fract() == 0.0 && v.abs() < 1e15 && !(f && ax_py_on()) => v as i64,
        o => panic!("expected an int here, got {}", ax_json_py(&o, true)),
    }
}
// A dynamic value looked for among strs: a non-str matches none of them.
pub fn ax_jkey_str(o: Json) -> String {
    match o {
        Json::Str { v } => v,
        _ => "\u{0}\u{1}not a str".to_string(),
    }
}
pub fn ax_jas_str(o: Json) -> String {
    match o {
        Json::Str { v } => v,
        o => panic!("expected a str here, got {}", ax_json_py(&o, true)),
    }
}
// Python errors with a class: "\u{1}Brk,Exception\u{1}message" (the class and the ones above it).
pub fn ax_exc(chain: String, msg: String) -> String {
    format!("\u{1}{chain}\u{1}{msg}")
}
// An error class's object raised with the error (`raise RErr(msg, line)`): kept here, its number in the error.
thread_local! {
    static AX_EXC_OBJS: std::cell::RefCell<Vec<Box<dyn std::any::Any>>> = std::cell::RefCell::new(Vec::new());
}
pub fn ax_exc_obj<T: 'static>(chain: String, msg: String, obj: T) -> String {
    let id = AX_EXC_OBJS.with(|o| {
        let mut o = o.borrow_mut();
        o.push(Box::new(obj));
        o.len() - 1
    });
    format!("\u{1}{chain};{id}\u{1}{msg}")
}
pub fn ax_exc_get<T: 'static + Clone>(e: &String) -> T {
    let id = e.strip_prefix('\u{1}').and_then(|r| r.split_once('\u{1}')).and_then(|(c, _)| c.rsplit_once(';')).and_then(|(_, n)| n.parse::<usize>().ok());
    match id.and_then(|id| AX_EXC_OBJS.with(|o| o.borrow().get(id).and_then(|b| b.downcast_ref::<T>().cloned()))) {
        Some(v) => v,
        None => panic!("this except clause caught an error of another class than it names (not supported yet)"),
    }
}
pub fn ax_exc_msg(e: &String) -> String {
    match e.strip_prefix('\u{1}').and_then(|r| r.split_once('\u{1}')) {
        Some((_, m)) => m.to_string(),
        None => e.clone(),
    }
}
// The uncaught-error report: str(e) is the empty message of `raise ValueError`, but the report still names
// the class it was raised as.
pub fn ax_exc_report(e: &String) -> String {
    let msg = ax_exc_msg(e);
    if !msg.is_empty() {
        return msg;
    }
    e.strip_prefix('\u{1}').and_then(|r| r.split_once('\u{1}')).map(|(chain, _)| {
        let chain = chain.rsplit_once(';').map_or(chain, |(c, _)| c);
        chain.split(',').next().unwrap_or("error").to_string()
    }).unwrap_or_else(|| "error".to_string())
}
// `except A, B:` for this error: one of its classes is named (Exception catches every class); an error without
// a class (a failed call) is caught by any built-in error name.
pub fn ax_exc_is(e: &String, names: String, builtin: bool) -> bool {
    let names: Vec<&str> = names.split(',').collect();
    if names.iter().any(|n| matches!(*n, "Exception" | "BaseException")) {
        return true;
    }
    match e.strip_prefix('\u{1}').and_then(|r| r.split_once('\u{1}')) {
        Some((chain, _)) => {
            let chain = chain.rsplit_once(';').map_or(chain, |(c, _)| c);
            let mut cs: Vec<&str> = chain.split(',').collect();
            // Built-in classes above built-in errors.
            let more: Vec<&str> = cs.iter().flat_map(|c| match *c {
                "KeyError" | "IndexError" => vec!["LookupError"],
                "ZeroDivisionError" | "OverflowError" => vec!["ArithmeticError"],
                "FileNotFoundError" => vec!["OSError", "IOError"],
                _ => vec![],
            }).collect();
            cs.extend(more);
            names.iter().any(|n| cs.contains(n))
        }
        None => builtin,
    }
}
// A tuple as JSON: in Python files it stays a tuple (prints `('lit', 1)`; json.dumps writes a list).
pub fn ax_tuple_json(v: Vec<Json>) -> Json {
    if ax_py_on() { Json::Tup { v } } else { Json::Arr { v } }
}
// A tuple returned with other lengths on other paths: a JSON tuple, printed as Python prints it.
pub fn ax_tup_json<T: AxToJson>(x: T) -> Json {
    match x.ax_json() {
        Json::Arr { v } => Json::Tup { v },
        j => j,
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
// json.dumps: any value Brevi can turn into JSON, written as Python's json module writes it.
pub trait AxDumps {
    fn ax_py_dumps<A: AsRef<str>, B: AsRef<str>>(&self, item: A, key: B, indent: i64, sort: bool, ascii: bool) -> String;
}
impl<T: AxToJson> AxDumps for T {
    fn ax_py_dumps<A: AsRef<str>, B: AsRef<str>>(&self, item: A, key: B, indent: i64, sort: bool, ascii: bool) -> String {
        let mut out = String::new();
        ax_dump(&self.ax_json(), item.as_ref(), key.as_ref(), indent, sort, ascii, 0, &mut out);
        out
    }
}
fn ax_dump_str(s: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (ascii && (c as u32) > 0x7f) => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
fn ax_dump(j: &Json, item: &str, key: &str, indent: i64, sort: bool, ascii: bool, depth: usize, out: &mut String) {
    let nl = |out: &mut String, d: usize| {
        if indent >= 0 {
            out.push('\n');
            out.push_str(&" ".repeat(indent as usize * d));
        }
    };
    match j {
        Json::Null => out.push_str("null"),
        Json::Bool { v } => out.push_str(if *v { "true" } else { "false" }),
        Json::Num { v, .. } if v.is_nan() => out.push_str("NaN"),
        Json::Num { v, .. } if v.is_infinite() => out.push_str(if *v > 0.0 { "Infinity" } else { "-Infinity" }),
        Json::Num { v, f } if v.fract() == 0.0 && v.abs() < 1e16 && !(*f && ax_py_on()) => out.push_str(&format!("{}", *v as i64)),
        Json::Num { v, .. } => out.push_str(&ax_py_float(*v)),
        Json::Str { v } => ax_dump_str(v, ascii, out),
        Json::Arr { v } | Json::Tup { v } if v.is_empty() => out.push_str("[]"),
        Json::Arr { v } | Json::Tup { v } => {
            out.push('[');
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    out.push_str(item);
                }
                nl(out, depth + 1);
                ax_dump(x, item, key, indent, sort, ascii, depth + 1, out);
            }
            nl(out, depth);
            out.push(']');
        }
        Json::Obj { v } if v.is_empty() => out.push_str("{}"),
        Json::Obj { v } => {
            let mut kv: Vec<(&String, &Json)> = v.iter().collect();
            if sort {
                kv.sort_by(|a, b| a.0.cmp(b.0));
            }
            out.push('{');
            for (i, (k, x)) in kv.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(item);
                }
                nl(out, depth + 1);
                ax_dump_str(k, ascii, out);
                out.push_str(key);
                ax_dump(x, item, key, indent, sort, ascii, depth + 1, out);
            }
            nl(out, depth);
            out.push('}');
        }
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

// `a | b` and `a & b`: bitwise on ints; union and intersection (sorted, no repeats) on lists used as sets.
pub trait AxBits {
    fn ax_bor(self, o: Self) -> Self;
    fn ax_band(self, o: Self) -> Self;
}
impl AxBits for i64 {
    fn ax_bor(self, o: i64) -> i64 {
        self | o
    }
    fn ax_band(self, o: i64) -> i64 {
        self & o
    }
}
// a | b and a & b on JSON values: bools give a bool, ints an int, as in Python; a float is a TypeError.
impl AxBits for bool {
    fn ax_bor(self, o: bool) -> bool {
        self | o
    }
    fn ax_band(self, o: bool) -> bool {
        self & o
    }
}
impl<T: Clone + PartialOrd> AxBits for Vec<T> {
    fn ax_bor(self, o: Vec<T>) -> Vec<T> {
        let mut v = self;
        v.extend(o);
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v.dedup_by(|a, b| a == b);
        v
    }
    fn ax_band(self, o: Vec<T>) -> Vec<T> {
        let o = ax_sorted_for_search(o);
        let mut v: Vec<T> = self.into_iter().filter(|x| ax_sorted_has(&o, x)).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v.dedup_by(|a, b| a == b);
        v
    }
}
// Set algebra on lists: the other side sorted once, then binary searched (O(n log m), not O(n * m)).
fn ax_sorted_for_search<T: PartialOrd>(mut o: Vec<T>) -> Vec<T> {
    if o.len() > 8 {
        o.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    }
    o
}
fn ax_sorted_has<T: PartialOrd>(o: &[T], x: &T) -> bool {
    if o.len() <= 8 {
        return o.iter().any(|y| y == x);
    }
    match o.binary_search_by(|y| y.partial_cmp(x).unwrap_or(std::cmp::Ordering::Equal)) {
        Ok(i) => o[i] == *x,
        Err(_) => false,
    }
}
pub trait AxSetSub {
    fn ax_set_sub(self, o: Self) -> Self;
}
impl<T: Clone + PartialOrd> AxSetSub for Vec<T> {
    fn ax_set_sub(self, o: Vec<T>) -> Vec<T> {
        let o = ax_sorted_for_search(o);
        let mut v: Vec<T> = self.into_iter().filter(|x| !ax_sorted_has(&o, x)).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v.dedup_by(|a, b| a == b);
        v
    }
}
// Python string methods on a value whose type is only known to Rust: s.count(sub), set(s), ljust/rjust.
// s.splitlines(True), and a file's lines as `for line in f` gives them: each keeps its "\n".
pub trait AxLinesKeep {
    fn ax_lines_keep(&self) -> Vec<String>;
}
impl AxLinesKeep for String {
    fn ax_lines_keep(&self) -> Vec<String> {
        self.split_inclusive('\n').map(String::from).collect()
    }
}
pub trait AxStrMore {
    fn ax_count(&self, sub: impl AsRef<str>) -> i64;
    fn ax_count_of(&self, sub: impl AsRef<str>) -> i64;
    fn ax_unique(&self) -> Vec<String>;
    fn ax_ljust(&self, n: i64) -> String;
    fn ax_skip(&self, n: i64) -> Vec<String>;
    fn ax_split_n(&self, sep: impl AsRef<str>, n: i64) -> Vec<String>;
    fn ax_rsplit_n(&self, sep: impl AsRef<str>, n: i64) -> Vec<String>;
    fn ax_rjust(&self, n: i64) -> String;
    fn ax_ljust_with(&self, n: i64, fill: impl AsRef<str>) -> String;
    fn ax_rjust_with(&self, n: i64, fill: impl AsRef<str>) -> String;
}
impl AxStrMore for String {
    fn ax_count(&self, sub: impl AsRef<str>) -> i64 {
        self.ax_count_sub(sub)
    }
    fn ax_count_of(&self, sub: impl AsRef<str>) -> i64 {
        self.ax_count_sub(sub)
    }
    fn ax_unique(&self) -> Vec<String> {
        let mut v: Vec<String> = self.chars().map(|c| c.to_string()).collect();
        v.sort();
        v.dedup();
        v
    }
    fn ax_ljust(&self, n: i64) -> String {
        self.ax_ljust_with(n, " ")
    }
    fn ax_skip(&self, n: i64) -> Vec<String> {
        self.chars().skip(n.max(0) as usize).map(|c| c.to_string()).collect()
    }
    // s.split(sep, n): at most n splits, as in Python (n < 0 means all).
    fn ax_split_n(&self, sep: impl AsRef<str>, n: i64) -> Vec<String> {
        if n < 0 {
            return self.split(sep.as_ref()).map(String::from).collect();
        }
        self.splitn(n as usize + 1, sep.as_ref()).map(String::from).collect()
    }
    fn ax_rsplit_n(&self, sep: impl AsRef<str>, n: i64) -> Vec<String> {
        if n < 0 {
            return self.split(sep.as_ref()).map(String::from).collect();
        }
        let mut v: Vec<String> = self.rsplitn(n as usize + 1, sep.as_ref()).map(String::from).collect();
        v.reverse();
        v
    }
    fn ax_rjust(&self, n: i64) -> String {
        self.ax_rjust_with(n, " ")
    }
    fn ax_ljust_with(&self, n: i64, fill: impl AsRef<str>) -> String {
        let pad = (n - self.chars().count() as i64).max(0) as usize;
        format!("{self}{}", fill.as_ref().repeat(pad))
    }
    fn ax_rjust_with(&self, n: i64, fill: impl AsRef<str>) -> String {
        let pad = (n - self.chars().count() as i64).max(0) as usize;
        format!("{}{self}", fill.as_ref().repeat(pad))
    }
}
// A map read item by item is its list of keys, as in Python (`[k for k in d if d[k] > 0]`).
pub trait AxMapSeq<K> {
    fn ax_map<U>(&self, f: impl Fn(K) -> U) -> Vec<U>;
    fn ax_filter(&self, f: impl Fn(K) -> bool) -> Vec<K>;
    fn ax_any(&self, f: impl FnMut(K) -> bool) -> bool;
    fn ax_all(&self, f: impl FnMut(K) -> bool) -> bool;
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxMapSeq<K> for AxM<K, V> {
    fn ax_map<U>(&self, f: impl Fn(K) -> U) -> Vec<U> {
        self.ax_keys().into_iter().map(f).collect()
    }
    fn ax_filter(&self, f: impl Fn(K) -> bool) -> Vec<K> {
        self.ax_keys().into_iter().filter(|k| f(k.clone())).collect()
    }
    fn ax_any(&self, mut f: impl FnMut(K) -> bool) -> bool {
        self.ax_keys().into_iter().any(f)
    }
    fn ax_all(&self, mut f: impl FnMut(K) -> bool) -> bool {
        self.ax_keys().into_iter().all(f)
    }
}
// A list is never Python's None; a None list slot reads as an empty list.
impl<T> AxNone for Vec<T> {
    fn ax_none() -> Self {
        Vec::new()
    }
}
// Nor is a map: `d = m.get(k)` with k missing reads as an empty map (`if not d:` sees it as missing).
impl<K, V> AxNone for AxM<K, V> {
    fn ax_none() -> Self {
        Default::default()
    }
}
// `a, b = xs`: a tuple as it is, or a list of exactly that many items, as Python unpacks them.
pub trait AxUnpack<T> {
    fn ax_unpack(self) -> T;
}
impl<A, B> AxUnpack<(A, B)> for (A, B) {
    fn ax_unpack(self) -> (A, B) {
        self
    }
}
impl<A, B, C> AxUnpack<(A, B, C)> for (A, B, C) {
    fn ax_unpack(self) -> (A, B, C) {
        self
    }
}
impl<A, B, C, D> AxUnpack<(A, B, C, D)> for (A, B, C, D) {
    fn ax_unpack(self) -> (A, B, C, D) {
        self
    }
}
fn ax_unpack_n<T>(xs: Vec<T>, n: usize) -> std::vec::IntoIter<T> {
    if xs.len() != n {
        panic!("cannot unpack {} items into {n} names", xs.len());
    }
    xs.into_iter()
}
// `value, tol = q.data` with data a JSON list.
impl AxUnpack<(Json, Json)> for Json {
    fn ax_unpack(self) -> (Json, Json) {
        let v = self.ax_list();
        (v.get(0).cloned().unwrap_or(Json::Null), v.get(1).cloned().unwrap_or(Json::Null))
    }
}
impl AxUnpack<(Json, Json, Json)> for Json {
    fn ax_unpack(self) -> (Json, Json, Json) {
        let v = self.ax_list();
        let g = |i: usize| v.get(i).cloned().unwrap_or(Json::Null);
        (g(0), g(1), g(2))
    }
}
// `_, frm, d, to, key = c` with c a dynamic tuple: as many items as names, or the run stops (Python's ValueError).
fn ax_junpack(j: Json, n: usize) -> Vec<Json> {
    let v = j.ax_list();
    if v.len() != n {
        panic!("ValueError: expected {n} values to unpack, got {}", v.len());
    }
    v
}
impl AxUnpack<(Json, Json, Json, Json)> for Json {
    fn ax_unpack(self) -> (Json, Json, Json, Json) {
        let mut v = ax_junpack(self, 4).into_iter();
        (v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap())
    }
}
impl AxUnpack<(Json, Json, Json, Json, Json)> for Json {
    fn ax_unpack(self) -> (Json, Json, Json, Json, Json) {
        let mut v = ax_junpack(self, 5).into_iter();
        (v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap())
    }
}
impl AxUnpack<(Json, Json, Json, Json, Json, Json)> for Json {
    fn ax_unpack(self) -> (Json, Json, Json, Json, Json, Json) {
        let mut v = ax_junpack(self, 6).into_iter();
        (v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap(), v.next().unwrap())
    }
}
impl<T> AxUnpack<(T, T)> for Vec<T> {
    fn ax_unpack(self) -> (T, T) {
        let mut i = ax_unpack_n(self, 2);
        (i.next().unwrap(), i.next().unwrap())
    }
}
impl<T> AxUnpack<(T, T, T)> for Vec<T> {
    fn ax_unpack(self) -> (T, T, T) {
        let mut i = ax_unpack_n(self, 3);
        (i.next().unwrap(), i.next().unwrap(), i.next().unwrap())
    }
}
impl<T> AxUnpack<(T, T, T, T)> for Vec<T> {
    fn ax_unpack(self) -> (T, T, T, T) {
        let mut i = ax_unpack_n(self, 4);
        (i.next().unwrap(), i.next().unwrap(), i.next().unwrap(), i.next().unwrap())
    }
}
// Longer unpacks (`ip, ts, meth, path, proto, st, size, ref, ua = m.groups()`).
macro_rules! ax_unpack_many {
    ($n:expr; $($t:ident)+) => {
        impl<$($t),+> AxUnpack<($($t,)+)> for ($($t,)+) {
            fn ax_unpack(self) -> ($($t,)+) {
                self
            }
        }
        impl<T> AxUnpack<($(ax_unpack_many!(@t $t),)+)> for Vec<T> {
            fn ax_unpack(self) -> ($(ax_unpack_many!(@t $t),)+) {
                let mut i = ax_unpack_n(self, $n);
                ($(ax_unpack_many!(@n i $t),)+)
            }
        }
    };
    (@t $x:ident) => { T };
    (@n $i:ident $x:ident) => { $i.next().unwrap() };
}
ax_unpack_many!(5; A B C D E);
ax_unpack_many!(6; A B C D E F);
ax_unpack_many!(7; A B C D E F G);
ax_unpack_many!(8; A B C D E F G H);
ax_unpack_many!(9; A B C D E F G H I);
ax_unpack_many!(10; A B C D E F G H I J);
ax_unpack_many!(11; A B C D E F G H I J K);
ax_unpack_many!(12; A B C D E F G H I J K L);
// Python's `P, N, K = "PNK"`: a string unpacks into its characters.
impl<U> AxUnpack<U> for String
where
    Vec<String>: AxUnpack<U>,
{
    fn ax_unpack(self) -> U {
        self.chars().map(|c| c.to_string()).collect::<Vec<String>>().ax_unpack()
    }
}
// Python's `a ** b`: an int to a whole power stays an int; with a float on either side it is a float.
pub trait AxPow<B> {
    type Out;
    fn ax_pow(self, b: B) -> Self::Out;
}
impl AxPow<i64> for i64 {
    type Out = i64;
    fn ax_pow(self, b: i64) -> i64 {
        if b < 0 {
            panic!("int ** negative power gives a fraction: write a float, like 2.0 ** {b}");
        }
        match u32::try_from(b) {
            Ok(e) => self.pow(e),
            Err(_) if self == 0 || self == 1 => self,
            Err(_) if self == -1 => if b % 2 == 0 { 1 } else { -1 },
            Err(_) => panic!("attempt to multiply with overflow"),
        }
    }
}
// Python's float ** float (C's pow): 0.0 ** -1 and an infinite result from finite numbers raise; a negative
// number to a fractional power is complex in Python, which is refused rather than giving nan.
fn ax_fpow(a: f64, b: f64) -> f64 {
    let r = a.powf(b);
    if ax_py_on() && a.is_finite() && b.is_finite() {
        if a == 0.0 && b < 0.0 {
            panic!("ZeroDivisionError: 0.0 cannot be raised to a negative power");
        }
        if a < 0.0 && b.fract() != 0.0 {
            panic!("a negative number to a fractional power is a complex number in Python, which Brevi does not support");
        }
        if r.is_infinite() {
            panic!("OverflowError: (34, 'Numerical result out of range')");
        }
    }
    r
}
impl AxPow<f64> for f64 {
    type Out = f64;
    fn ax_pow(self, b: f64) -> f64 {
        ax_fpow(self, b)
    }
}
impl AxPow<i64> for f64 {
    type Out = f64;
    fn ax_pow(self, b: i64) -> f64 {
        ax_fpow(self, b as f64)
    }
}
impl AxPow<f64> for i64 {
    type Out = f64;
    fn ax_pow(self, b: f64) -> f64 {
        ax_fpow(self as f64, b)
    }
}
// Python 3's `a / b`: always a float.
pub trait AxTrueDiv<B> {
    fn ax_truediv(self, b: B) -> f64;
}
impl AxTrueDiv<i64> for i64 {
    fn ax_truediv(self, b: i64) -> f64 {
        if b == 0 {
            panic!("division by zero");
        }
        self as f64 / b as f64
    }
}
impl AxTrueDiv<f64> for i64 {
    fn ax_truediv(self, b: f64) -> f64 {
        self as f64 / b
    }
}
impl AxTrueDiv<i64> for f64 {
    fn ax_truediv(self, b: i64) -> f64 {
        self / b as f64
    }
}
impl AxTrueDiv<f64> for f64 {
    fn ax_truediv(self, b: f64) -> f64 {
        self / b
    }
}
// The same division, but a zero divisor is a ZeroDivisionError a Python try can catch.
pub trait AxTrueDivTry<B> {
    fn ax_truediv_try(self, b: B) -> R<f64>;
}
impl AxTrueDivTry<i64> for i64 {
    fn ax_truediv_try(self, b: i64) -> R<f64> {
        if b == 0 {
            return Err(ax_zd_err("division by zero"));
        }
        Ok(self as f64 / b as f64)
    }
}
impl AxTrueDivTry<f64> for i64 {
    fn ax_truediv_try(self, b: f64) -> R<f64> {
        if b == 0.0 {
            return Err(ax_zd_err("float division by zero"));
        }
        Ok(self as f64 / b)
    }
}
impl AxTrueDivTry<i64> for f64 {
    fn ax_truediv_try(self, b: i64) -> R<f64> {
        if b == 0 {
            return Err(ax_zd_err("float division by zero"));
        }
        Ok(self / b as f64)
    }
}
impl AxTrueDivTry<f64> for f64 {
    fn ax_truediv_try(self, b: f64) -> R<f64> {
        if b == 0.0 {
            return Err(ax_zd_err("float division by zero"));
        }
        Ok(self / b)
    }
}
// Python's s.update(xs) on a set() list, m.update(other) on a dict, c.update(xs) on a Counter.
pub trait AxUpdate<O> {
    fn ax_update(&mut self, o: O);
}
impl<T: PartialEq> AxUpdate<Vec<T>> for Vec<T> {
    fn ax_update(&mut self, o: Vec<T>) {
        for x in o {
            if !self.contains(&x) {
                self.push(x);
            }
        }
    }
}
impl<K: Hash + Eq + Ord + Clone, V: Clone> AxUpdate<AxM<K, V>> for AxM<K, V> {
    fn ax_update(&mut self, o: AxM<K, V>) {
        for (k, v) in o.iter() {
            self.insert(k.clone(), v.clone());
        }
    }
}
impl<K: Hash + Eq + Ord + Clone> AxUpdate<Vec<K>> for AxM<K, i64> {
    fn ax_update(&mut self, o: Vec<K>) {
        for k in o {
            match self.get_mut(&k) {
                Some(n) => *n += 1,
                None => {
                    self.insert(k, 1);
                }
            }
        }
    }
}
// timedelta(days=n) is a Date n days from the epoch, so d + timedelta(...) and (d2 - d1).days work.
pub trait AxDelta {
    fn ax_delta_days(&self) -> Date;
}
impl AxDelta for i64 {
    fn ax_delta_days(&self) -> Date {
        Date { days: *self }
    }
}
impl Plus for Date {
    type Out = Date;
    fn plus(self, o: Date) -> Date {
        Date { days: self.days + o.days }
    }
}
// calendar.monthrange(y, m) and calendar.isleap(y) on the year.
pub trait AxCalendar {
    fn ax_month_range(&self, m: i64) -> (i64, i64);
    fn ax_is_leap(&self) -> bool;
}
impl AxCalendar for i64 {
    fn ax_month_range(&self, m: i64) -> (i64, i64) {
        let first = days_from_civil(*self, m, 1);
        let next = if m == 12 { days_from_civil(*self + 1, 1, 1) } else { days_from_civil(*self, m + 1, 1) };
        ((first + 3).rem_euclid(7), next - first)
    }
    fn ax_is_leap(&self) -> bool {
        let y = *self;
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
    }
}
// Python's float.is_integer() (and int's, always True since 3.12).
pub trait AxIsInteger {
    fn ax_is_integer(&self) -> bool;
}
impl AxIsInteger for f64 {
    fn ax_is_integer(&self) -> bool {
        self.is_finite() && self.fract() == 0.0
    }
}
impl AxIsInteger for i64 {
    fn ax_is_integer(&self) -> bool {
        true
    }
}
impl AxIsInteger for Json {
    fn ax_is_integer(&self) -> bool {
        match self {
            Json::Num { v, .. } => v.ax_is_integer(),
            Json::Bool { .. } => true,
            _ => panic!("AttributeError: object has no attribute 'is_integer'"),
        }
    }
}
pub trait AxIsAscii {
    fn ax_is_ascii(&self) -> bool;
}
impl AxIsAscii for String {
    fn ax_is_ascii(&self) -> bool {
        self.is_ascii()
    }
}
// float(d), int(d), abs(d) on a decimal, as Python allows.
impl AxNum for Dec {
    fn ax_fmt(&self, decimals: i64) -> String {
        Dec::ax_fmt(self, decimals)
    }
    fn ax_abs(&self) -> Self {
        Dec::ax_abs(self)
    }
    fn ax_float(&self) -> f64 {
        Dec::ax_float(self)
    }
    fn ax_int(&self) -> i64 {
        Dec::ax_int(self)
    }
}
// isinstance(v, int) and isinstance(v, (str, list)): kinds is a comma-separated list of Python type names.
// Fixed types answer from their own type; a JSON value answers from what it holds (bool counts as int, as in Python).
pub trait AxIsInstance {
    fn ax_kind(&self) -> &'static str;
    fn ax_dyn(&self) -> bool {
        false
    }
    // `type(a) is type(b)`: the names of the two kinds.
    fn ax_type_kind(&self) -> String {
        self.ax_kind().to_string()
    }
    fn ax_isinstance(&self, kinds: String) -> bool {
        let k = self.ax_kind();
        // `=int` (from `type(v) is int`) is the exact kind.
        kinds.split(',').any(|t| match t.strip_prefix('=') {
            Some(e) => e == k,
            // A typed tuple may be kept as a list; a dynamic value knows whether it is a tuple.
            None => t == k || (t == "int" && k == "bool") || (t == "object") || (t == "tuple" && k == "list" && !self.ax_dyn()),
        })
    }
}
impl AxIsInstance for i64 {
    fn ax_kind(&self) -> &'static str {
        "int"
    }
}
impl AxIsInstance for f64 {
    fn ax_kind(&self) -> &'static str {
        "float"
    }
}
impl AxIsInstance for bool {
    fn ax_kind(&self) -> &'static str {
        "bool"
    }
}
impl AxIsInstance for String {
    fn ax_kind(&self) -> &'static str {
        // A sentinel held in a str slot is an object, not a string, to isinstance.
        if self.starts_with("\u{0}sent") {
            "object"
        } else {
            "str"
        }
    }
}
impl AxIsInstance for Dec {
    fn ax_kind(&self) -> &'static str {
        if self.is_frac() {
            return "Fraction";
        }
        "Decimal"
    }
}
impl<T> AxIsInstance for Vec<T> {
    fn ax_kind(&self) -> &'static str {
        "list"
    }
}
impl<K, V> AxIsInstance for AxM<K, V> {
    fn ax_kind(&self) -> &'static str {
        "dict"
    }
}
impl AxIsInstance for Json {
    fn ax_dyn(&self) -> bool {
        true
    }
    fn ax_kind(&self) -> &'static str {
        match self {
            Json::Null => "NoneType",
            Json::Bool { .. } => "bool",
            Json::Num { v, f } => if v.fract() == 0.0 && v.abs() < 1e15 && !(*f && ax_py_on()) { "int" } else { "float" },
            Json::Str { .. } => "str",
            Json::Arr { .. } => "list", Json::Tup { .. } if ax_json_sent(self) => "object", Json::Tup { .. } => "tuple",
            Json::Obj { .. } => "dict",
        }
    }
}
// Python code that reads json.loads values directly: `v is None`, `v == 0`, `for x in v`, `sum(f(x) for x in v)`.
impl AxNone for Json {
    fn ax_none() -> Self {
        Json::Null
    }
}
// As in Python, False == 0 and True == 1.
impl PartialEq<i64> for Json {
    fn eq(&self, o: &i64) -> bool {
        matches!(self, Json::Num { v, .. } if *v == *o as f64) || matches!(self, Json::Bool { v } if (*v as i64) == *o)
    }
}
impl PartialEq<f64> for Json {
    fn eq(&self, o: &f64) -> bool {
        matches!(self, Json::Num { v, .. } if *v == *o) || matches!(self, Json::Bool { v } if (*v as i64 as f64) == *o)
    }
}
impl AxTryList<Json> for Json {
    fn ax_try_map<U>(&self, f: impl Fn(Json) -> R<U>) -> R<Vec<U>> {
        self.ax_list().ax_try_map(f)
    }
    fn ax_try_filter(&self, f: impl Fn(Json) -> R<bool>) -> R<Vec<Json>> {
        self.ax_list().ax_try_filter(f)
    }
    fn ax_try_any(&self, f: impl Fn(Json) -> R<bool>) -> R<bool> {
        self.ax_list().ax_try_any(f)
    }
    fn ax_try_all(&self, f: impl Fn(Json) -> R<bool>) -> R<bool> {
        self.ax_list().ax_try_all(f)
    }
    fn ax_try_count(&self, f: impl Fn(Json) -> R<bool>) -> R<i64> {
        self.ax_list().ax_try_count(f)
    }
    fn ax_try_fold<A>(&self, init: A, f: impl Fn(A, Json) -> R<A>) -> R<A> {
        self.ax_list().ax_try_fold(init, f)
    }
    fn ax_try_sort_by<K: PartialOrd>(&self, f: impl Fn(Json) -> R<K>) -> R<Vec<Json>> {
        self.ax_list().ax_try_sort_by(f)
    }
    fn ax_try_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(Json) -> R<K>, rev: bool) -> R<Vec<Json>> {
        self.ax_list().ax_try_sort_by_rev(f, rev)
    }
}
// Text and list methods on a dynamic value (a parameter or slot whose type could not be fixed): each works on
// the text or list inside, as Python's would; any other kind stops the run with Python's TypeError.
impl Json {
    fn ax_jtext(&self, m: &str) -> &String {
        match self {
            Json::Str { v } => v,
            other => panic!("TypeError: {} has no method {m}", other.ax_kind()),
        }
    }
    // sum(j) of a dynamic list.
    pub fn ax_sum(&self) -> Json {
        let xs = self.ax_seq();
        if xs.iter().any(ax_jf) && xs.iter().all(|x| matches!(x, Json::Num { .. })) {
            return ax_jnk(ax_fsum(0.0, xs.iter().map(ax_jnum)), true);
        }
        xs.into_iter().fold(Json::Num { v: 0.0, f: false }, |a, b| Plus::plus(a, b))
    }
    // What `for x in j` walks: a list's items, a text's characters, a dict's keys.
    fn ax_seq(&self) -> Vec<Json> {
        match self {
            Json::Str { v } => v.chars().map(|c| Json::Str { v: c.to_string() }).collect(),
            Json::Obj { v } => v.m.keys().map(|k| Json::Str { v: k.clone() }).collect(),
            _ => self.ax_list(),
        }
    }
    fn ax_jitems(&self, m: &str) -> Vec<Json> {
        match self {
            Json::Arr { v } | Json::Tup { v } => v.clone(),
            Json::Str { v } => v.chars().map(|c| Json::Str { v: c.to_string() }).collect(),
            Json::Obj { v } => v.m.keys().map(|k| Json::Str { v: k.clone() }).collect(),
            other => panic!("TypeError: {} has no method {m}", other.ax_kind()),
        }
    }
    pub fn ax_slice(&self, a: i64, b: i64) -> Json {
        match self {
            Json::Str { v } => Json::Str { v: AxStr::ax_slice(v, a, b) },
            Json::Arr { v } => Json::Arr { v: AxList::ax_slice(v, a, b) },
            Json::Tup { v } => Json::Tup { v: AxList::ax_slice(v, a, b) },
            other => panic!("TypeError: {} cannot be sliced", other.ax_kind()),
        }
    }
    pub fn ax_lower(&self) -> String {
        self.ax_jtext("lower").ax_lower()
    }
    pub fn ax_upper(&self) -> String {
        self.ax_jtext("upper").ax_upper()
    }
    pub fn ax_trim(&self) -> String {
        self.ax_jtext("strip").ax_trim()
    }
    pub fn ax_split(&self, sep: String) -> Vec<String> {
        self.ax_jtext("split").ax_split(sep)
    }
    pub fn ax_words(&self) -> Vec<String> {
        self.ax_jtext("split").ax_words()
    }
    pub fn ax_lines(&self) -> Vec<String> {
        self.ax_jtext("splitlines").ax_lines()
    }
    pub fn ax_chars(&self) -> Vec<String> {
        self.ax_jtext("chars").ax_chars()
    }
    pub fn ax_starts_with(&self, x: String) -> bool {
        self.ax_jtext("startswith").ax_starts_with(x)
    }
    pub fn ax_ends_with(&self, x: String) -> bool {
        self.ax_jtext("endswith").ax_ends_with(x)
    }
    pub fn ax_replace(&self, a: String, b: String) -> String {
        self.ax_jtext("replace").ax_replace(a, b)
    }
    pub fn ax_find(&self, x: String) -> i64 {
        self.ax_jtext("find").ax_find(x)
    }
    pub fn ax_is_digit(&self) -> bool {
        self.ax_jtext("isdigit").ax_is_digit()
    }
    pub fn ax_is_alpha(&self) -> bool {
        self.ax_jtext("isalpha").ax_is_alpha()
    }
    pub fn ax_is_space(&self) -> bool {
        self.ax_jtext("isspace").ax_is_space()
    }
    pub fn ax_is_ascii(&self) -> bool {
        self.ax_jtext("isascii").ax_is_ascii()
    }
    pub fn ax_sort_by<K: PartialOrd>(&self, f: impl Fn(Json) -> K) -> Vec<Json> {
        self.ax_jitems("sort").ax_sort_by(f)
    }
    pub fn ax_sort_by_rev<K: PartialOrd>(&self, f: impl Fn(Json) -> K, rev: bool) -> Vec<Json> {
        self.ax_jitems("sort").ax_sort_by_rev(f, rev)
    }
    pub fn ax_rev(&self) -> Json {
        match self {
            Json::Str { v } => Json::Str { v: AxStrRev::ax_rev(v) },
            Json::Tup { v } => Json::Tup { v: v.iter().rev().cloned().collect() },
            other => Json::Arr { v: other.ax_jitems("reversed").into_iter().rev().collect() },
        }
    }
    // sep.join(xs) with xs dynamic: its items' text.
    pub fn ax_join(&self, sep: String) -> String {
        self.ax_jitems("join").iter().map(|x| x.ax_text()).collect::<Vec<_>>().join(&sep)
    }
    pub fn ax_index_of(&self, x: &Json) -> i64 {
        self.ax_jitems("index").iter().position(|y| y == x).map_or(-1, |i| i as i64)
    }
}
// A JSON list read item by item, as Python code does with json.loads values.
pub trait AxJsonSeq {
    fn ax_map<U>(&self, f: impl Fn(Json) -> U) -> Vec<U>;
    fn ax_filter(&self, f: impl Fn(Json) -> bool) -> Vec<Json>;
    fn ax_any(&self, f: impl FnMut(Json) -> bool) -> bool;
    fn ax_all(&self, f: impl FnMut(Json) -> bool) -> bool;
    fn ax_enumerate(&self) -> Vec<(i64, Json)>;
}
impl AxJsonSeq for Json {
    fn ax_map<U>(&self, f: impl Fn(Json) -> U) -> Vec<U> {
        self.ax_seq().into_iter().map(f).collect()
    }
    fn ax_filter(&self, f: impl Fn(Json) -> bool) -> Vec<Json> {
        self.ax_seq().into_iter().filter(|x| f(x.clone())).collect()
    }
    fn ax_any(&self, mut f: impl FnMut(Json) -> bool) -> bool {
        self.ax_seq().into_iter().any(f)
    }
    fn ax_all(&self, mut f: impl FnMut(Json) -> bool) -> bool {
        self.ax_seq().into_iter().all(f)
    }
    fn ax_enumerate(&self) -> Vec<(i64, Json)> {
        self.ax_seq().into_iter().enumerate().map(|(i, x)| (i as i64, x)).collect()
    }
}
// `total = None` then a Decimal later: the unset value is a sentinel no sum reaches.
impl AxNone for Dec {
    fn ax_none() -> Self {
        Dec(i128::MIN, 0)
    }
}
pub trait AxPopOr<K, V> {
    fn ax_pop_or(&mut self, k: K, d: V) -> V;
}
impl<K: Hash + Eq + Ord, V> AxPopOr<K, V> for AxM<K, V> {
    fn ax_pop_or(&mut self, k: K, d: V) -> V {
        self.remove(&k).unwrap_or(d)
    }
}
impl AxToJson for Date {
    fn ax_json(&self) -> Json {
        Json::Str { v: self.show() }
    }
}
impl<A: AxNone, B: AxNone, C: AxNone, D: AxNone> AxNone for (A, B, C, D) {
    fn ax_none() -> Self {
        (A::ax_none(), B::ax_none(), C::ax_none(), D::ax_none())
    }
}
impl<A: AxNone, B: AxNone, C: AxNone, D: AxNone, E: AxNone> AxNone for (A, B, C, D, E) {
    fn ax_none() -> Self {
        (A::ax_none(), B::ax_none(), C::ax_none(), D::ax_none(), E::ax_none())
    }
}
impl<A: AxNone, B: AxNone, C: AxNone, D: AxNone, E: AxNone, F: AxNone> AxNone for (A, B, C, D, E, F) {
    fn ax_none() -> Self {
        (A::ax_none(), B::ax_none(), C::ax_none(), D::ax_none(), E::ax_none(), F::ax_none())
    }
}
// itertools.groupby: runs of neighbouring items with the same key, as (key, items) pairs.
pub trait AxGroupRuns<T> {
    fn ax_group_runs<K: PartialEq>(&self, f: impl Fn(T) -> K) -> Vec<(K, Vec<T>)>;
}
impl<T: Clone> AxGroupRuns<T> for Vec<T> {
    fn ax_group_runs<K: PartialEq>(&self, f: impl Fn(T) -> K) -> Vec<(K, Vec<T>)> {
        let mut out: Vec<(K, Vec<T>)> = Vec::new();
        for x in self {
            let k = f(x.clone());
            match out.last_mut() {
                Some((lk, run)) if *lk == k => run.push(x.clone()),
                _ => out.push((k, vec![x.clone()])),
            }
        }
        out
    }
}
impl AxGroupRuns<String> for String {
    fn ax_group_runs<K: PartialEq>(&self, f: impl Fn(String) -> K) -> Vec<(K, Vec<String>)> {
        self.chars().map(|c| c.to_string()).collect::<Vec<_>>().ax_group_runs(f)
    }
}
// Python's xs.count(x), min(xs, default=d), max(xs, default=d) and a sorted list's first item or d.
pub trait AxCountOf<T> {
    fn ax_count_of(&self, x: &T) -> i64;
    fn ax_first_or(&self, d: T) -> T;
}
impl<T: Clone + PartialEq> AxCountOf<T> for Vec<T> {
    fn ax_count_of(&self, x: &T) -> i64 {
        self.iter().filter(|y| *y == x).count() as i64
    }
    fn ax_first_or(&self, d: T) -> T {
        self.first().cloned().unwrap_or(d)
    }
}
pub trait AxMinOr<T> {
    fn ax_min_or(&self, d: T) -> T;
    fn ax_max_or(&self, d: T) -> T;
}
impl<T: Clone + PartialOrd> AxMinOr<T> for Vec<T> {
    fn ax_min_or(&self, d: T) -> T {
        if self.is_empty() { d } else { self.ax_min().unwrap_or(d) }
    }
    fn ax_max_or(&self, d: T) -> T {
        if self.is_empty() { d } else { self.ax_max().unwrap_or(d) }
    }
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
    // Python's `.` under re.DOTALL, `^`/`$` under re.MULTILINE, and plain `$` (the end, or before a final newline).
    AnyNl,
    LineStart,
    LineEnd,
    EndNl,
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
    // (?s) and (?m) at the front: re.DOTALL and re.MULTILINE.
    dotall: bool,
    multiline: bool,
}
impl AxRxP {
    // The character after a backslash (already read): `\x00`, `\u00e9` and `\n` name characters.
    fn esc(&mut self, e: char) -> char {
        let n = match e {
            'x' => 2,
            'u' => 4,
            'U' => 8,
            'f' => return '\x0c',
            'v' => return '\x0b',
            'a' => return '\x07',
            _ => return ax_rx_esc(e),
        };
        let h: String = self.c.iter().skip(self.i).take(n).collect();
        match u32::from_str_radix(&h, 16).ok().filter(|_| h.len() == n).and_then(char::from_u32) {
            Some(c) => {
                self.i += n;
                c
            }
            None => e,
        }
    }
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
            '.' if self.dotall => AxRx::AnyNl,
            '.' => AxRx::Any,
            '^' if self.multiline => AxRx::LineStart,
            '^' => AxRx::Start,
            '$' if self.multiline => AxRx::LineEnd,
            '$' if ax_py_on() => AxRx::EndNl,
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
                        self.esc(e)
                    } else {
                        c
                    };
                    if self.peek() == Some('-') && self.c.get(self.i + 1).map_or(false, |&n| n != ']') {
                        let mut hi = self.c[self.i + 1];
                        self.i += 2;
                        if hi == '\\' {
                            let e = self.c[self.i];
                            self.i += 1;
                            hi = self.esc(e);
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
                // `\Z` and `\A`: the end and start of the text.
                if e == 'Z' {
                    return AxRx::End;
                }
                if e == 'A' {
                    return AxRx::Start;
                }
                match ax_rx_class(e) {
                    Some((cs, neg)) => AxRx::Class(cs, neg),
                    None => AxRx::Char(self.esc(e)),
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
        AxRx::AnyNl => i < s.len() && k(i + 1, caps),
        AxRx::LineStart => (i == 0 || s[i - 1] == '\n') && k(i, caps),
        AxRx::LineEnd => (i == s.len() || s[i] == '\n') && k(i, caps),
        AxRx::EndNl => (i == s.len() || (i + 1 == s.len() && s[i] == '\n')) && k(i, caps),
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
// One character matched by a single-character node, or None when the node is not one.
fn ax_rx_one(n: &AxRx, c: char) -> Option<bool> {
    match n {
        AxRx::Char(x) => Some(c == *x),
        AxRx::Any => Some(c != '\n'),
        AxRx::AnyNl => Some(true),
        AxRx::Class(rs, neg) => Some(rs.iter().any(|&(a, b)| a <= c && c <= b) != *neg),
        _ => None,
    }
}
fn ax_rx_rep(n: &AxRx, lo: usize, hi: usize, lazy: bool, count: usize, s: &[char], i: usize, caps: &mut AxCaps, k: &mut dyn FnMut(usize, &mut AxCaps) -> bool) -> bool {
    // `\S+`, `\d*`, `.*` and the like: count the run in a loop, then try the lengths in order,
    // instead of one nested call per character.
    if count == 0 && ax_rx_one(n, 'a').is_some() {
        let mut run = 0;
        while run < hi && i + run < s.len() && ax_rx_one(n, s[i + run]) == Some(true) {
            run += 1;
        }
        if run < lo {
            return false;
        }
        if lazy {
            return (lo..=run).any(|m| k(i + m, caps));
        }
        return (lo..=run).rev().any(|m| k(i + m, caps));
    }
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
// Leading inline flags `(?s)`, `(?m)`, `(?sm)` (the parser writes re.S and re.M this way): (dotall,
// multiline, the rest of the pattern).
fn ax_rx_flags(p: &str) -> (bool, bool, &str) {
    if let Some(r) = p.strip_prefix("(?") {
        if let Some(k) = r.find(')') {
            let f = &r[..k];
            if !f.is_empty() && f.chars().all(|c| c == 's' || c == 'm') {
                return (f.contains('s'), f.contains('m'), &r[k + 1..]);
            }
        }
    }
    (false, false, p)
}
fn ax_rx(p: &str) -> std::rc::Rc<(AxRx, usize)> {
    AX_RX.with(|c| {
        if let Some(r) = c.borrow().get(p) {
            return r.clone();
        }
        c.borrow_mut()
            .entry(p.to_string())
            .or_insert_with(|| {
                let (dotall, multiline, body) = ax_rx_flags(p);
                let mut ps = AxRxP { c: body.chars().collect(), i: 0, groups: 0, dotall, multiline };
                let r = ps.alt();
                std::rc::Rc::new((r, ps.groups))
            })
            .clone()
    })
}
// The first match at or after `from`: (start, end, groups).
fn ax_rx_search(p: &str, s: &[char], from: usize, full: bool) -> Option<(usize, AxCaps)> {
    ax_rx_search_at(p, s, from, full, full)
}
// `anchored`: only a match starting at `from` counts (Python's re.match).
fn ax_rx_search_at(p: &str, s: &[char], from: usize, full: bool, anchored: bool) -> Option<(usize, AxCaps)> {
    // Python's re.IGNORECASE, written as `(?i)` in front: match lowercased text with a lowercased
    // pattern (escapes such as \D keep their case); positions are the same in the original text.
    if let Some(rest) = p.strip_prefix("(?i)") {
        let low: Vec<char> = s.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
        let mut lp = String::new();
        let mut esc = false;
        for c in rest.chars() {
            if esc {
                lp.push(c);
                esc = false;
            } else if c == '\\' {
                lp.push(c);
                esc = true;
            } else {
                lp.extend(c.to_lowercase());
            }
        }
        return ax_rx_search_at(&lp, &low, from, full, anchored);
    }
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
        if anchored {
            break;
        }
    }
    None
}
fn ax_rx_groups(s: &[char], caps: &AxCaps) -> Vec<String> {
    // A group that took no part in the match is None in Python (the None string; findall and sub read it as "").
    let none = || if ax_py_on() { String::ax_none() } else { String::new() };
    caps.iter().map(|g| g.map_or_else(none, |(a, b)| s[a..b].iter().collect())).collect()
}
// A Python match: its groups ([whole, group1, ...], empty for None) and where each one starts and ends. It is
// not a list (len(m), `for x in m` and m[1:] are errors in Python too), and a list holds nothing but its items.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AxMatch {
    groups: Vec<String>,
    spans: Vec<(i64, i64)>,
}
// Group n's place in the lists: a match has no group -1, and None has no groups at all.
impl AxMatch {
    fn ax_group(&self, n: i64) -> usize {
        if self.groups.is_empty() {
            ax_none_attr()
        }
        if n < 0 || n as usize >= self.groups.len() {
            panic!("IndexError: no such group");
        }
        n as usize
    }
}
impl AxRef<i64, String> for AxMatch {
    fn ax_ref(&self, n: &i64) -> &String {
        &self.groups[self.ax_group(*n)]
    }
}
impl AxRefTry<i64, String> for AxMatch {
    fn ax_try_ref(&self, n: &i64) -> R<&String> {
        if self.groups.is_empty() {
            ax_none_attr();
        }
        if *n < 0 || *n as usize >= self.groups.len() {
            return Err(ax_idx_err("no such group"));
        }
        Ok(&self.groups[*n as usize])
    }
}
impl AxTruth for AxMatch {
    fn ax_truth(&self) -> bool {
        !self.groups.is_empty()
    }
}
impl AxNone for AxMatch {
    fn ax_none() -> Self {
        AxMatch::default()
    }
}
impl Show for AxMatch {
    fn show(&self) -> String {
        // A .bv program in Python style has always printed a match as its list of groups.
        if !AX_PY_FILE.load(std::sync::atomic::Ordering::Relaxed) {
            return self.groups.show();
        }
        match (self.groups.first(), self.spans.first()) {
            (Some(g), Some((a, b))) => format!("<re.Match object; span=({a}, {b}), match={}>", ax_py_repr(g)),
            _ => "None".to_string(),
        }
    }
}
fn ax_rx_match(s: &[char], caps: &AxCaps) -> AxMatch {
    let spans = caps.iter().map(|g| g.map_or((-1, -1), |(a, b)| (a as i64, b as i64))).collect();
    AxMatch { groups: ax_rx_groups(s, caps), spans }
}
// Python's m.start(n), m.end(n), m.span(n) and m.groups().
impl AxMatch {
    pub fn ax_re_span(&self, n: i64) -> (i64, i64) {
        self.spans[self.ax_group(n)]
    }
    pub fn ax_re_start(&self, n: i64) -> i64 {
        self.ax_re_span(n).0
    }
    pub fn ax_re_end(&self, n: i64) -> i64 {
        self.ax_re_span(n).1
    }
    pub fn ax_re_groups(&self) -> Vec<String> {
        self.ax_group(0);
        self.groups[1..].to_vec()
    }
}
pub trait AxRe {
    fn ax_re_find(&self, p: String) -> Vec<String>;
    fn ax_re_full(&self, p: String) -> Vec<String>;
    fn ax_re_all(&self, p: String) -> Vec<Vec<String>>;
    fn ax_re_list(&self, p: String) -> Vec<String>;
    fn ax_re_sub(&self, p: String, r: String) -> String;
    fn ax_re_sub_with<F: FnMut(Vec<String>) -> String>(&self, p: String, f: F) -> String;
    fn ax_try_re_sub_with<F: FnMut(Vec<String>) -> R<String>>(&self, p: String, f: F) -> R<String>;
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
fn ax_rx_sub(s: &str, p: &str, mut f: impl FnMut(&[char], &AxCaps) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    let cs = ax_rx_each(s, p, |cs, c| {
        let (a, b) = c[0].unwrap();
        if a >= last {
            out.extend(&cs[last..a]);
            out.push_str(&f(cs, c));
            last = b;
        }
    });
    out.extend(&cs[last.min(cs.len())..]);
    out
}
// The same with a function that can raise: the first error stops the substitution and is passed on.
fn ax_rx_try_sub(s: &str, p: &str, mut f: impl FnMut(&[char], &AxCaps) -> R<String>) -> R<String> {
    let mut err = None;
    let out = ax_rx_sub(s, p, |cs, c| {
        if err.is_some() {
            return String::new();
        }
        match f(cs, c) {
            Ok(s) => s,
            Err(e) => {
                err = Some(e);
                String::new()
            }
        }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(out),
    }
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
                    o.push_str(g.get(rc[i + 1] as usize - '0' as usize).filter(|s| !s.ax_is_none()).map_or("", |s| s.as_str()));
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
        ax_rx_sub(self, &p, |cs, c| f(ax_rx_groups(cs, c)))
    }
    fn ax_try_re_sub_with<F: FnMut(Vec<String>) -> R<String>>(&self, p: String, mut f: F) -> R<String> {
        ax_rx_try_sub(self, &p, |cs, c| f(ax_rx_groups(cs, c)))
    }
    fn ax_re_split(&self, p: String) -> Vec<String> {
        let mut out = vec![];
        let mut last = 0;
        let cs = ax_rx_each(self, &p, |cs, c| {
            let (a, b) = c[0].unwrap();
            // Python (3.7 on) splits at empty matches too.
            if b > a || ax_py_on() {
                out.push(cs[last..a].iter().collect());
                // Python's re.split also gives each group's text between the pieces (None for a group that
                // took no part).
                if ax_py_on() {
                    for g in &c[1..] {
                        out.push(g.map_or_else(|| String::ax_none(), |(x, y)| cs[x..y].iter().collect()));
                    }
                }
                last = b;
            }
        });
        out.push(cs[last..].iter().collect());
        out
    }
}
// What Python programs call: re.search, re.fullmatch, re.finditer and re.sub with a function, each giving
// matches. (Programs written in Brevi get plain lists from re_find, re_full and re_all.)
pub trait AxReM {
    fn ax_re_m_find(&self, p: impl AsRef<str>) -> AxMatch;
    fn ax_re_m_full(&self, p: impl AsRef<str>) -> AxMatch;
    fn ax_re_m_all(&self, p: impl AsRef<str>) -> Vec<AxMatch>;
    fn ax_re_m_sub_with<F: FnMut(AxMatch) -> String>(&self, p: impl AsRef<str>, f: F) -> String;
    fn ax_try_re_m_sub_with<F: FnMut(AxMatch) -> R<String>>(&self, p: impl AsRef<str>, f: F) -> R<String>;
}
impl AxReM for String {
    fn ax_re_m_find(&self, p: impl AsRef<str>) -> AxMatch {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search(p.as_ref(), &cs, 0, false).map_or_else(AxMatch::default, |(_, c)| ax_rx_match(&cs, &c))
    }
    fn ax_re_m_full(&self, p: impl AsRef<str>) -> AxMatch {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search(p.as_ref(), &cs, 0, true).map_or_else(AxMatch::default, |(_, c)| ax_rx_match(&cs, &c))
    }
    fn ax_re_m_all(&self, p: impl AsRef<str>) -> Vec<AxMatch> {
        let mut out = vec![];
        ax_rx_each(self, p.as_ref(), |cs, c| out.push(ax_rx_match(cs, c)));
        out
    }
    fn ax_re_m_sub_with<F: FnMut(AxMatch) -> String>(&self, p: impl AsRef<str>, mut f: F) -> String {
        ax_rx_sub(self, p.as_ref(), |cs, c| f(ax_rx_match(cs, c)))
    }
    fn ax_try_re_m_sub_with<F: FnMut(AxMatch) -> R<String>>(&self, p: impl AsRef<str>, mut f: F) -> R<String> {
        ax_rx_try_sub(self, p.as_ref(), |cs, c| f(ax_rx_match(cs, c)))
    }
}
impl AxReM for Json {
    fn ax_re_m_find(&self, p: impl AsRef<str>) -> AxMatch {
        self.ax_jtext("search").ax_re_m_find(p)
    }
    fn ax_re_m_full(&self, p: impl AsRef<str>) -> AxMatch {
        self.ax_jtext("fullmatch").ax_re_m_full(p)
    }
    fn ax_re_m_all(&self, p: impl AsRef<str>) -> Vec<AxMatch> {
        self.ax_jtext("finditer").ax_re_m_all(p)
    }
    fn ax_re_m_sub_with<F: FnMut(AxMatch) -> String>(&self, p: impl AsRef<str>, f: F) -> String {
        self.ax_jtext("sub").ax_re_m_sub_with(p, f)
    }
    fn ax_try_re_m_sub_with<F: FnMut(AxMatch) -> R<String>>(&self, p: impl AsRef<str>, f: F) -> R<String> {
        self.ax_jtext("sub").ax_try_re_m_sub_with(p, f)
    }
}
// Python's re.match (anchored at the start) and re.findall (group 1 when the pattern has one group).
// re on a JSON value: its text (re.search(p, item["title"]) over loaded JSON, #195).
impl AxRe for Json {
    fn ax_re_find(&self, p: String) -> Vec<String> {
        self.ax_jtext("search").ax_re_find(p)
    }
    fn ax_re_full(&self, p: String) -> Vec<String> {
        self.ax_jtext("fullmatch").ax_re_full(p)
    }
    fn ax_re_all(&self, p: String) -> Vec<Vec<String>> {
        self.ax_jtext("finditer").ax_re_all(p)
    }
    fn ax_re_list(&self, p: String) -> Vec<String> {
        self.ax_jtext("findall").ax_re_list(p)
    }
    fn ax_re_sub(&self, p: String, r: String) -> String {
        self.ax_jtext("sub").ax_re_sub(p, r)
    }
    fn ax_re_sub_with<F: FnMut(Vec<String>) -> String>(&self, p: String, f: F) -> String {
        self.ax_jtext("sub").ax_re_sub_with(p, f)
    }
    fn ax_try_re_sub_with<F: FnMut(Vec<String>) -> R<String>>(&self, p: String, f: F) -> R<String> {
        self.ax_jtext("sub").ax_try_re_sub_with(p, f)
    }
    fn ax_re_split(&self, p: String) -> Vec<String> {
        self.ax_jtext("split").ax_re_split(p)
    }
}
pub trait AxRePy {
    fn ax_re_match(&self, p: impl AsRef<str>) -> Vec<String>;
    fn ax_re_m_match(&self, p: impl AsRef<str>) -> AxMatch;
    fn ax_re_m_match_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch;
    fn ax_re_m_find_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch;
    fn ax_re_findall(&self, p: impl AsRef<str>) -> Vec<String>;
}
impl AxRePy for String {
    fn ax_re_match(&self, p: impl AsRef<str>) -> Vec<String> {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search_at(p.as_ref(), &cs, 0, false, true).map_or(vec![], |(_, c)| ax_rx_groups(&cs, &c))
    }
    fn ax_re_m_match(&self, p: impl AsRef<str>) -> AxMatch {
        let cs: Vec<char> = self.chars().collect();
        ax_rx_search_at(p.as_ref(), &cs, 0, false, true).map_or_else(AxMatch::default, |(_, c)| ax_rx_match(&cs, &c))
    }
    // pat.match(s, pos) and pat.search(s, pos): from char pos (clamped to the text, as Python does) of the whole
    // text, so spans count from its start and `\b` sees the char before pos.
    fn ax_re_m_match_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch {
        let cs: Vec<char> = self.chars().collect();
        let from = pos.clamp(0, cs.len() as i64) as usize;
        ax_rx_search_at(p.as_ref(), &cs, from, false, true).map_or_else(AxMatch::default, |(_, c)| ax_rx_match(&cs, &c))
    }
    fn ax_re_m_find_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch {
        let cs: Vec<char> = self.chars().collect();
        let from = pos.clamp(0, cs.len() as i64) as usize;
        ax_rx_search_at(p.as_ref(), &cs, from, false, false).map_or_else(AxMatch::default, |(_, c)| ax_rx_match(&cs, &c))
    }
    fn ax_re_findall(&self, p: impl AsRef<str>) -> Vec<String> {
        let all = self.ax_re_all(p.as_ref().to_string());
        all.into_iter().map(|g| if g.len() == 2 { if g[1].ax_is_none() { String::new() } else { g[1].clone() } } else { g[0].clone() }).collect()
    }
}
impl AxRePy for Json {
    fn ax_re_match(&self, p: impl AsRef<str>) -> Vec<String> {
        match self { Json::Str { v } => v.ax_re_match(p), _ => panic!("TypeError: expected string or bytes-like object") }
    }
    fn ax_re_m_match(&self, p: impl AsRef<str>) -> AxMatch {
        match self { Json::Str { v } => v.ax_re_m_match(p), _ => panic!("TypeError: expected string or bytes-like object") }
    }
    fn ax_re_m_match_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch {
        match self { Json::Str { v } => v.ax_re_m_match_at(p, pos), _ => panic!("TypeError: expected string or bytes-like object") }
    }
    fn ax_re_m_find_at(&self, p: impl AsRef<str>, pos: i64) -> AxMatch {
        match self { Json::Str { v } => v.ax_re_m_find_at(p, pos), _ => panic!("TypeError: expected string or bytes-like object") }
    }
    fn ax_re_findall(&self, p: impl AsRef<str>) -> Vec<String> {
        match self { Json::Str { v } => v.ax_re_findall(p), _ => panic!("TypeError: expected string or bytes-like object") }
    }
}

// Python's `v is True` / `v is False`: only the bool itself (a JSON 1 == True, but is not True).
pub trait AxIsBool {
    fn ax_bool_obj(&self) -> Option<bool>;
    fn ax_is_true_obj(&self) -> bool {
        self.ax_bool_obj() == Some(true)
    }
    fn ax_is_false_obj(&self) -> bool {
        self.ax_bool_obj() == Some(false)
    }
    fn ax_not_true_obj(&self) -> bool {
        !self.ax_is_true_obj()
    }
    fn ax_not_false_obj(&self) -> bool {
        !self.ax_is_false_obj()
    }
}
impl AxIsBool for bool {
    fn ax_bool_obj(&self) -> Option<bool> {
        Some(*self)
    }
}
impl AxIsBool for Json {
    fn ax_bool_obj(&self) -> Option<bool> {
        match self {
            Json::Bool { v } => Some(*v),
            _ => None,
        }
    }
}
macro_rules! ax_not_bool { ($($t:ty),*) => { $(impl AxIsBool for $t { fn ax_bool_obj(&self) -> Option<bool> { None } })* } }
ax_not_bool!(i64, f64, String, Dec);
impl<T> AxIsBool for Vec<T> {
    fn ax_bool_obj(&self) -> Option<bool> {
        None
    }
}
impl<'a, T: AxIsBool + ?Sized> AxIsBool for &'a T {
    fn ax_bool_obj(&self) -> Option<bool> {
        (**self).ax_bool_obj()
    }
}
// Python's `v == True`: a bool, or a number equal to 1 (0 for False).
impl PartialEq<bool> for Json {
    fn eq(&self, o: &bool) -> bool {
        match self {
            Json::Bool { v } => v == o,
            Json::Num { v, .. } => *v == if *o { 1.0 } else { 0.0 },
            _ => false,
        }
    }
}
