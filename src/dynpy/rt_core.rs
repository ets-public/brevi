// Brevi dynamic runtime: Python values for programs that type inference cannot type.
// This file (with the other rt_*.rs files) is not part of the brevi binary's code: it is the runtime
// library that a dynamically built program links against.
#![allow(warnings)]
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::io::{BufRead, Read, Write};
use std::rc::Rc;

pub type R = Result<V, E>;
pub type Sym = &'static str;

#[derive(Clone)]
pub enum V {
    Unbound,
    None,
    Bool(bool),
    Int(i64),
    Big(Rc<Big>),
    Float(f64),
    Str(Rc<S>),
    List(Rc<RefCell<Vec<V>>>),
    Tuple(Rc<Vec<V>>),
    Dict(Rc<RefCell<Dict>>),
    Set(Rc<RefCell<PSet>>),
    FSet(Rc<PSet>),
    Func(Rc<Func>),
    Class(Rc<Class>),
    Obj(Rc<Obj>),
    Bound(Rc<(V, V)>),
    BMeth(Rc<(V, Sym)>),
    Iter(Rc<RefCell<It>>),
    Range(Rc<(i64, i64, i64)>),
    Slice(Rc<(V, V, V)>),
    Deque(Rc<RefCell<Deq>>),
    Module(Sym),
    View(Rc<(V, u8)>),
    Prop(Rc<(V, V)>),
    Ext(Rc<dyn Ext>),
    NotImpl,
    Ellipsis,
}

// Values of library types (Decimal, Fraction, date, regex match, ...) implement Ext.
pub trait Ext {
    fn tname(&self) -> &'static str;
    fn repr(&self) -> String;
    fn str_(&self) -> String {
        self.repr()
    }
    fn any(&self) -> &dyn std::any::Any;
    fn hash(&self) -> Option<i64> {
        None
    }
    fn eq(&self, _o: &V) -> Option<bool> {
        None
    }
    fn cmp(&self, _o: &V) -> Option<std::cmp::Ordering> {
        None
    }
    fn attr(&self, _me: &V, _n: &str) -> Option<R> {
        None
    }
    fn method(&self, _me: &V, _n: &str, _a: &mut Args) -> Option<R> {
        None
    }
    fn binop(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<R> {
        None
    }
    fn truth(&self) -> bool {
        true
    }
    fn to_f64(&self) -> Option<f64> {
        None
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn iter(&self) -> Option<Vec<V>> {
        None
    }
    fn fmt(&self, _spec: &str) -> Option<R> {
        None
    }
    // For values a Python library made (a dynamic build's bridge, pasted into the program): `o.n = v`,
    // `a OP b` for the ordering operators, `isinstance(me, cls)` with a builtin class, `isinstance(x, me)`
    // with me a library class, a lazy iterator, and the value as Brevi's own (a list as a list) for code
    // that needs that (json.dumps, dict(x)).
    fn set_attr(&self, _me: &V, _n: &str, _v: V) -> Option<Result<(), E>> {
        None
    }
    fn rich(&self, _me: &V, _op: &str, _o: &V, _right: bool) -> Option<Result<bool, E>> {
        None
    }
    fn isinst(&self, _cls: &Class) -> Option<bool> {
        None
    }
    fn instancecheck(&self, _x: &V) -> Option<Result<bool, E>> {
        None
    }
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        None
    }
    fn native(&self) -> Option<R> {
        None
    }
    fn is_py(&self) -> bool {
        false
    }
    fn try_truth(&self) -> Option<Result<bool, E>> {
        None
    }
}

pub struct S {
    pub s: String,
    pub ascii: bool,
}
impl S {
    pub fn nchars(&self) -> usize {
        if self.ascii {
            self.s.len()
        } else {
            self.s.chars().count()
        }
    }
}

pub struct Deq {
    pub v: VecDeque<V>,
    pub max: Option<usize>,
}

pub struct E(pub V);

thread_local! {
    pub static STR_CACHE: RefCell<Vec<V>> = RefCell::new(Vec::new());
}

pub fn s(x: &str) -> V {
    V::Str(Rc::new(S { s: x.to_string(), ascii: x.is_ascii() }))
}
pub fn st(x: String) -> V {
    let ascii = x.is_ascii();
    V::Str(Rc::new(S { s: x, ascii }))
}
pub fn ch1(c: char) -> V {
    if (c as u32) < 128 {
        return STR_CACHE.with(|cc| {
            let mut cc = cc.borrow_mut();
            if cc.is_empty() {
                for i in 0..128u8 {
                    cc.push(V::Str(Rc::new(S { s: (i as char).to_string(), ascii: true })));
                }
            }
            cc[c as usize].clone()
        });
    }
    st(c.to_string())
}
pub fn list(v: Vec<V>) -> V {
    V::List(Rc::new(RefCell::new(v)))
}
pub fn tup(v: Vec<V>) -> V {
    V::Tuple(Rc::new(v))
}
pub fn int(i: i64) -> V {
    V::Int(i)
}
pub fn flt(f: f64) -> V {
    V::Float(f)
}
pub fn bool_(b: bool) -> V {
    V::Bool(b)
}
pub fn dict_new() -> V {
    V::Dict(Rc::new(RefCell::new(Dict::new())))
}
pub fn set_new() -> V {
    V::Set(Rc::new(RefCell::new(PSet::new())))
}

// ---------- exceptions ----------

pub fn exc_class(name: &str) -> Rc<Class> {
    BUILTIN_CLASSES.with(|b| b.borrow().get(name).cloned()).unwrap_or_else(|| panic!("no exception class {name}"))
}
pub fn exc(name: &str, msg: String) -> E {
    let c = exc_class(name);
    let o = Obj::new(c);
    o.set("args", tup(vec![st(msg)]));
    E(V::Obj(o))
}
pub fn exc0(name: &str) -> E {
    let c = exc_class(name);
    let o = Obj::new(c);
    o.set("args", tup(vec![]));
    E(V::Obj(o))
}
pub fn exc_v(name: &str, v: V) -> E {
    let c = exc_class(name);
    let o = Obj::new(c);
    o.set("args", tup(vec![v]));
    E(V::Obj(o))
}
pub fn type_err<T>(m: String) -> Result<T, E> {
    Err(exc("TypeError", m))
}
pub fn val_err<T>(m: String) -> Result<T, E> {
    Err(exc("ValueError", m))
}
pub fn unsupported(what: &str) -> E {
    exc("NotImplementedError", format!("brevi: {what} is not supported in dynamic builds"))
}

// ---------- big integers ----------

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Big {
    pub neg: bool,
    pub mag: Vec<u32>,
}
fn mtrim(mut v: Vec<u32>) -> Vec<u32> {
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}
fn mcmp(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for i in (0..a.len()).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    std::cmp::Ordering::Equal
}
fn madd(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut r = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut c = 0u64;
    for i in 0..a.len().max(b.len()) {
        let s = *a.get(i).unwrap_or(&0) as u64 + *b.get(i).unwrap_or(&0) as u64 + c;
        r.push(s as u32);
        c = s >> 32;
    }
    if c > 0 {
        r.push(c as u32);
    }
    r
}
fn msub(a: &[u32], b: &[u32]) -> Vec<u32> {
    // a >= b
    let mut r = Vec::with_capacity(a.len());
    let mut br = 0i64;
    for i in 0..a.len() {
        let mut d = a[i] as i64 - *b.get(i).unwrap_or(&0) as i64 - br;
        if d < 0 {
            d += 1 << 32;
            br = 1;
        } else {
            br = 0;
        }
        r.push(d as u32);
    }
    mtrim(r)
}
fn mmul(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut r = vec![0u32; a.len() + b.len()];
    for i in 0..a.len() {
        let mut c = 0u64;
        for j in 0..b.len() {
            let t = a[i] as u64 * b[j] as u64 + r[i + j] as u64 + c;
            r[i + j] = t as u32;
            c = t >> 32;
        }
        let mut k = i + b.len();
        while c > 0 {
            let t = r[k] as u64 + c;
            r[k] = t as u32;
            c = t >> 32;
            k += 1;
        }
    }
    mtrim(r)
}
fn mdivsmall(a: &[u32], d: u32) -> (Vec<u32>, u32) {
    let mut q = vec![0u32; a.len()];
    let mut r = 0u64;
    for i in (0..a.len()).rev() {
        let cur = (r << 32) | a[i] as u64;
        q[i] = (cur / d as u64) as u32;
        r = cur % d as u64;
    }
    (mtrim(q), r as u32)
}
fn mshl(a: &[u32], n: usize) -> Vec<u32> {
    let (w, b) = (n / 32, n % 32);
    let mut r = vec![0u32; w];
    if b == 0 {
        r.extend_from_slice(a);
    } else {
        let mut c = 0u32;
        for &x in a {
            r.push((x << b) | c);
            c = x >> (32 - b);
        }
        if c > 0 {
            r.push(c);
        }
    }
    mtrim(r)
}
fn mshr(a: &[u32], n: usize) -> Vec<u32> {
    let (w, b) = (n / 32, n % 32);
    if w >= a.len() {
        return vec![];
    }
    let a = &a[w..];
    if b == 0 {
        return a.to_vec();
    }
    let mut r = vec![0u32; a.len()];
    for i in 0..a.len() {
        r[i] = (a[i] >> b) | if i + 1 < a.len() { a[i + 1] << (32 - b) } else { 0 };
    }
    mtrim(r)
}
fn mbits(a: &[u32]) -> usize {
    if a.is_empty() {
        0
    } else {
        (a.len() - 1) * 32 + (32 - a.last().unwrap().leading_zeros() as usize)
    }
}
// Long division (shift-subtract on 32-bit words via Knuth D would be faster; this is simple and exact).
fn mdivmod(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if mcmp(a, b) == std::cmp::Ordering::Less {
        return (vec![], a.to_vec());
    }
    if b.len() == 1 {
        let (q, r) = mdivsmall(a, b[0]);
        return (q, mtrim(vec![r]));
    }
    // Knuth algorithm D
    let s = b.last().unwrap().leading_zeros() as usize;
    let b = mshl(b, s);
    let mut a = mshl(a, s);
    a.push(0);
    let n = b.len();
    let m = a.len() - n - 1;
    let mut q = vec![0u32; m + 1];
    let bt = b[n - 1] as u64;
    let bs = b[n - 2] as u64;
    for j in (0..=m).rev() {
        let num = ((a[j + n] as u64) << 32) | a[j + n - 1] as u64;
        let mut qh = num / bt;
        let mut rh = num % bt;
        while qh >= (1 << 32) || qh * bs > ((rh << 32) | a[j + n - 2] as u64) {
            qh -= 1;
            rh += bt;
            if rh >= (1 << 32) {
                break;
            }
        }
        let mut borrow = 0i64;
        let mut carry = 0u64;
        for i in 0..n {
            let p = qh * b[i] as u64 + carry;
            carry = p >> 32;
            let t = a[i + j] as i64 - borrow - (p & 0xffff_ffff) as i64;
            a[i + j] = t as u32;
            borrow = if t < 0 { 1 } else { 0 };
        }
        let t = a[j + n] as i64 - borrow - carry as i64;
        a[j + n] = t as u32;
        if t < 0 {
            qh -= 1;
            let mut c = 0u64;
            for i in 0..n {
                let t = a[i + j] as u64 + b[i] as u64 + c;
                a[i + j] = t as u32;
                c = t >> 32;
            }
            a[j + n] = (a[j + n] as u64 + c) as u32;
        }
        q[j] = qh as u32;
    }
    let r = mshr(&mtrim(a[..n].to_vec()), s);
    (mtrim(q), r)
}

impl Big {
    pub fn from_i64(i: i64) -> Big {
        let neg = i < 0;
        let u = i.unsigned_abs();
        Big { neg, mag: mtrim(vec![u as u32, (u >> 32) as u32]) }
    }
    pub fn from_i128(i: i128) -> Big {
        let neg = i < 0;
        let u = i.unsigned_abs();
        Big { neg, mag: mtrim(vec![u as u32, (u >> 32) as u32, (u >> 64) as u32, (u >> 96) as u32]) }
    }
    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }
    pub fn to_i64(&self) -> Option<i64> {
        if self.mag.len() > 2 {
            return None;
        }
        let u = *self.mag.first().unwrap_or(&0) as u64 | (*self.mag.get(1).unwrap_or(&0) as u64) << 32;
        if self.neg {
            if u <= 1u64 << 63 {
                Some((u as i64).wrapping_neg())
            } else {
                None
            }
        } else if u < 1u64 << 63 {
            Some(u as i64)
        } else {
            None
        }
    }
    pub fn to_i128(&self) -> Option<i128> {
        if self.mag.len() > 4 {
            return None;
        }
        let mut u: u128 = 0;
        for (i, &w) in self.mag.iter().enumerate() {
            u |= (w as u128) << (32 * i);
        }
        if u > i128::MAX as u128 {
            return None;
        }
        Some(if self.neg { -(u as i128) } else { u as i128 })
    }
    pub fn norm(self) -> V {
        match self.to_i64() {
            Some(i) => V::Int(i),
            None => V::Big(Rc::new(if self.mag.is_empty() { Big { neg: false, mag: vec![] } } else { self })),
        }
    }
    pub fn neg(&self) -> Big {
        Big { neg: !self.neg && !self.mag.is_empty(), mag: self.mag.clone() }
    }
    pub fn add(&self, o: &Big) -> Big {
        if self.neg == o.neg {
            return Big { neg: self.neg, mag: madd(&self.mag, &o.mag) };
        }
        match mcmp(&self.mag, &o.mag) {
            std::cmp::Ordering::Less => Big { neg: o.neg, mag: msub(&o.mag, &self.mag) },
            _ => {
                let m = msub(&self.mag, &o.mag);
                Big { neg: self.neg && !m.is_empty(), mag: m }
            }
        }
    }
    pub fn sub(&self, o: &Big) -> Big {
        self.add(&o.neg())
    }
    pub fn mul(&self, o: &Big) -> Big {
        let m = mmul(&self.mag, &o.mag);
        Big { neg: (self.neg != o.neg) && !m.is_empty(), mag: m }
    }
    // Floor division and modulo, as Python.
    pub fn divmod(&self, o: &Big) -> (Big, Big) {
        let (q, r) = mdivmod(&self.mag, &o.mag);
        let mut q = Big { neg: (self.neg != o.neg) && !q.is_empty(), mag: q };
        let mut r = Big { neg: self.neg && !r.is_empty(), mag: r };
        if !r.is_zero() && r.neg != o.neg {
            q = q.sub(&Big::from_i64(1));
            r = r.add(o);
        }
        (q, r)
    }
    pub fn cmp(&self, o: &Big) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self.neg, o.neg) {
            (false, true) => Greater,
            (true, false) => Less,
            (false, false) => mcmp(&self.mag, &o.mag),
            (true, true) => mcmp(&o.mag, &self.mag),
        }
    }
    pub fn to_string(&self) -> String {
        if self.mag.is_empty() {
            return "0".into();
        }
        let mut parts = vec![];
        let mut m = self.mag.clone();
        while !m.is_empty() {
            let (q, r) = mdivsmall(&m, 1_000_000_000);
            parts.push(r);
            m = q;
        }
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s.push_str(&parts.last().unwrap().to_string());
        for p in parts.iter().rev().skip(1) {
            s.push_str(&format!("{:09}", p));
        }
        s
    }
    pub fn to_radix(&self, r: u32) -> String {
        if self.mag.is_empty() {
            return "0".into();
        }
        let mut ds = vec![];
        if r.is_power_of_two() {
            // Read the digits straight from the bits (linear; dividing over and over is quadratic).
            let k = r.trailing_zeros() as usize;
            let bits = mbits(&self.mag);
            let bit = |i: usize| (self.mag[i / 32] >> (i % 32)) & 1;
            let mut i = 0;
            while i < bits {
                let mut d = 0;
                for j in 0..k {
                    if i + j < bits {
                        d |= bit(i + j) << j;
                    }
                }
                ds.push(std::char::from_digit(d, r).unwrap());
                i += k;
            }
            let mut s = String::new();
            if self.neg {
                s.push('-');
            }
            s.extend(ds.iter().rev());
            return s;
        }
        let mut m = self.mag.clone();
        while !m.is_empty() {
            let (q, x) = mdivsmall(&m, r);
            ds.push(std::char::from_digit(x, r).unwrap());
            m = q;
        }
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s.extend(ds.iter().rev());
        s
    }
    pub fn parse(t: &str, radix: u32) -> Option<Big> {
        let (neg, t) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        if t.is_empty() {
            return None;
        }
        if radix.is_power_of_two() && radix > 1 {
            // Set the bits straight from the digits, last digit first (linear).
            let k = radix.trailing_zeros() as usize;
            let ds: Vec<u32> = t.chars().map(|c| c.to_digit(radix)).collect::<Option<Vec<u32>>>()?;
            let mut m = vec![0u32; (ds.len() * k + 31) / 32 + 1];
            for (n, d) in ds.iter().rev().enumerate() {
                for j in 0..k {
                    if (d >> j) & 1 == 1 {
                        let i = n * k + j;
                        m[i / 32] |= 1 << (i % 32);
                    }
                }
            }
            let m = mtrim(m);
            return Some(Big { neg: neg && !m.is_empty(), mag: m });
        }
        let mut m: Vec<u32> = vec![];
        for c in t.chars() {
            let d = c.to_digit(radix)?;
            let mut carry = d as u64;
            for w in m.iter_mut() {
                let x = *w as u64 * radix as u64 + carry;
                *w = x as u32;
                carry = x >> 32;
            }
            if carry > 0 {
                m.push(carry as u32);
            }
        }
        let m = mtrim(m);
        Some(Big { neg: neg && !m.is_empty(), mag: m })
    }
    pub fn to_f64(&self) -> f64 {
        let mut f = 0.0f64;
        for &w in self.mag.iter().rev() {
            f = f * 4294967296.0 + w as f64;
        }
        if self.neg {
            -f
        } else {
            f
        }
    }
    pub fn from_f64(f: f64) -> Big {
        let neg = f < 0.0;
        let mut f = f.abs().trunc();
        let mut ws = vec![];
        while f >= 1.0 {
            let r = f % 4294967296.0;
            ws.push(r as u32);
            f = ((f - r) / 4294967296.0).trunc();
        }
        let m = mtrim(ws);
        Big { neg: neg && !m.is_empty(), mag: m }
    }
    // Python's &, | and ^ on ints of any size: two's complement with an endless sign.
    pub fn bitop(&self, o: &Big, op: &str) -> Big {
        if !self.neg && !o.neg {
            // The usual case: no sign to extend.
            let (a, b) = (&self.mag, &o.mag);
            let m = match op {
                "&" => a.iter().zip(b.iter()).map(|(x, y)| x & y).collect(),
                _ => {
                    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
                    let mut v = long.clone();
                    if op == "|" {
                        v.iter_mut().zip(short.iter()).for_each(|(x, y)| *x |= y);
                    } else {
                        v.iter_mut().zip(short.iter()).for_each(|(x, y)| *x ^= y);
                    }
                    v
                }
            };
            return Big { neg: false, mag: mtrim(m) };
        }
        let n = self.mag.len().max(o.mag.len()) + 1;
        let twos = |b: &Big| -> Vec<u32> {
            let mut v = b.mag.clone();
            v.resize(n, 0);
            if b.neg {
                // -m is !(m - 1)
                for w in v.iter_mut() {
                    let (r, borrow) = w.overflowing_sub(1);
                    *w = r;
                    if !borrow {
                        break;
                    }
                }
                for w in v.iter_mut() {
                    *w = !*w;
                }
            }
            v
        };
        let (x, y) = (twos(self), twos(o));
        let mut r: Vec<u32> = x.iter().zip(&y).map(|(a, b)| match op {
            "&" => a & b,
            "|" => a | b,
            _ => a ^ b,
        }).collect();
        let neg = r[n - 1] >> 31 == 1;
        if neg {
            for w in r.iter_mut() {
                *w = !*w;
            }
            for w in r.iter_mut() {
                let (v, carry) = w.overflowing_add(1);
                *w = v;
                if !carry {
                    break;
                }
            }
        }
        let m = mtrim(r);
        Big { neg: neg && !m.is_empty(), mag: m }
    }
    pub fn shl(&self, n: usize) -> Big {
        Big { neg: self.neg, mag: mshl(&self.mag, n) }
    }
    pub fn shr(&self, n: usize) -> Big {
        if self.neg {
            // floor
            let (q, _) = self.divmod(&Big::from_i64(1).shl(n));
            q
        } else {
            Big { neg: false, mag: mshr(&self.mag, n) }
        }
    }
    pub fn bits(&self) -> usize {
        mbits(&self.mag)
    }
    pub fn pow(&self, mut e: u64) -> Big {
        let mut base = self.clone();
        let mut r = Big::from_i64(1);
        while e > 0 {
            if e & 1 == 1 {
                r = r.mul(&base);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul(&base);
            }
        }
        r
    }
}
pub fn big_of(v: &V) -> Option<Big> {
    match v {
        V::Int(i) => Some(Big::from_i64(*i)),
        V::Bool(b) => Some(Big::from_i64(*b as i64)),
        V::Big(b) => Some((**b).clone()),
        _ => None,
    }
}

// ---------- hashing (CPython-compatible for numbers and tuples) ----------

const HMOD: i64 = (1i64 << 61) - 1;
pub fn hash_int(i: i64) -> i64 {
    let neg = i < 0;
    let mut u = (i.unsigned_abs() % HMOD as u64) as i64;
    if neg {
        u = -u;
    }
    if u == -1 {
        -2
    } else {
        u
    }
}
pub fn hash_big(b: &Big) -> i64 {
    // sum of words * 2^(32k) mod (2^61-1)
    let mut acc: u128 = 0;
    for &w in b.mag.iter().rev() {
        acc = ((acc << 32) | w as u128) % HMOD as u128;
    }
    let mut h = acc as i64;
    if b.neg {
        h = -h;
    }
    if h == -1 {
        -2
    } else {
        h
    }
}
pub fn hash_float(f: f64) -> i64 {
    if f.is_nan() {
        return 0;
    }
    if f.is_infinite() {
        return if f > 0.0 { 314159 } else { -314159 };
    }
    if f.fract() == 0.0 && f.abs() < 9.2e18 {
        return hash_int(f as i64);
    }
    // _Py_HashDouble
    let (mut m, mut e) = frexp(f);
    let sign = if m < 0.0 {
        m = -m;
        -1
    } else {
        1
    };
    let mut x: u64 = 0;
    while m != 0.0 {
        x = ((x << 28) & HMOD as u64) | (x >> (61 - 28));
        m *= 268435456.0;
        e -= 28;
        let y = m as u64;
        m -= y as f64;
        x += y;
        if x >= HMOD as u64 {
            x -= HMOD as u64;
        }
    }
    let e = if e >= 0 { e % 61 } else { 61 - 1 - ((-1 - e) % 61) };
    x = ((x << e) & HMOD as u64) | (x >> (61 - e));
    let mut h = x as i64 * sign;
    if h == -1 {
        h = -2;
    }
    h
}
fn frexp(f: f64) -> (f64, i32) {
    if f == 0.0 {
        return (0.0, 0);
    }
    let bits = f.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0 {
        let (m, e) = frexp(f * 2f64.powi(64));
        return (m, e - 64);
    }
    let e = exp - 1022;
    let m = f64::from_bits((bits & !(0x7ffu64 << 52)) | (1022u64 << 52));
    (m, e)
}
fn hash_str(s: &str) -> i64 {
    // CPython randomizes string hashes; any stable hash keeps the same guarantees.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let h = (h >> 1) as i64;
    if h == -1 {
        -2
    } else {
        h
    }
}
pub fn hash_tuple(xs: &[V]) -> Result<i64, E> {
    const P1: u64 = 11400714785074694791;
    const P2: u64 = 14029467366897019727;
    const P5: u64 = 2870177450012600261;
    let mut acc = P5;
    for x in xs {
        let lane = py_hash(x)? as u64;
        acc = acc.wrapping_add(lane.wrapping_mul(P2));
        acc = acc.rotate_left(31);
        acc = acc.wrapping_mul(P1);
    }
    acc = acc.wrapping_add((xs.len() as u64) ^ (P5 ^ 3527539));
    if acc == u64::MAX {
        return Ok(1546275796);
    }
    Ok(acc as i64)
}
pub fn py_hash(v: &V) -> Result<i64, E> {
    Ok(match v {
        V::None => 0xFCA86420u32 as i64,
        V::Bool(b) => *b as i64,
        V::Int(i) => hash_int(*i),
        V::Big(b) => hash_big(b),
        V::Float(f) => hash_float(*f),
        V::Str(x) => hash_str(&x.s),
        V::Tuple(xs) => hash_tuple(xs)?,
        V::FSet(p) => {
            // frozenset hash (CPython's algorithm)
            let mut h: u64 = 0;
            for k in p.keys() {
                let x = py_hash(&k)? as u64;
                h ^= ((x ^ 89869747) ^ (x << 16)).wrapping_mul(3644798167);
            }
            h ^= (p.used as u64 + 1).wrapping_mul(1927868237);
            h ^= (h >> 11) ^ (h >> 25);
            h = h.wrapping_mul(69069).wrapping_add(907133923);
            let h = h as i64;
            if h == -1 {
                590923713
            } else {
                h
            }
        }
        V::Obj(o) => {
            let mixin = if is_enum(&o.cls) { o.inner.borrow().clone() } else { None };
            if let Some(i) = mixin {
                py_hash(&i)?
            } else if let Some(f) = o.cls.lookup("__hash__") {
                if matches!(f, V::None) {
                    return type_err(format!("unhashable type: '{}'", o.cls.name));
                }
                match call(&f, Args::p(vec![v.clone()]))? {
                    V::Int(i) => hash_int(i),
                    x => py_hash(&x)?,
                }
            } else if o.cls.lookup("__eq__").is_some() {
                return type_err(format!("unhashable type: '{}'", o.cls.name));
            } else {
                (Rc::as_ptr(o) as usize >> 4) as i64
            }
        }
        V::Class(c) => (Rc::as_ptr(c) as usize >> 4) as i64,
        V::Func(f) => (Rc::as_ptr(f) as usize >> 4) as i64,
        V::Ext(x) => match x.hash() {
            Some(h) => h,
            None => return type_err(format!("unhashable type: '{}'", x.tname())),
        },
        V::Range(r) => hash_tuple(&[V::Int(r.0), V::Int(r.1), V::Int(r.2)])?,
        V::Module(m) => hash_str(m),
        V::Ellipsis | V::NotImpl => 7,
        _ => return type_err(format!("unhashable type: '{}'", type_name(v))),
    })
}

// HashMap key wrapper: Python equality and hash (hash precomputed).
#[derive(Clone)]
pub struct K {
    pub h: i64,
    pub v: V,
}
impl Hash for K {
    fn hash<H: Hasher>(&self, st: &mut H) {
        st.write_i64(self.h);
    }
}
impl PartialEq for K {
    fn eq(&self, o: &K) -> bool {
        self.h == o.h && eq(&self.v, &o.v).unwrap_or(false)
    }
}
impl Eq for K {}

#[derive(Default)]
pub struct IdH(u64);
impl Hasher for IdH {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x100000001b3);
        }
    }
    fn write_i64(&mut self, i: i64) {
        self.0 = (i as u64).wrapping_mul(0x9E3779B97F4A7C15);
    }
}
pub type HM<KK, VV> = HashMap<KK, VV, BuildHasherDefault<IdH>>;

// ---------- dict (insertion ordered) ----------

pub struct Dict {
    pub ents: Vec<Option<(V, V)>>,
    pub idx: HM<K, usize>,
    pub n: usize,
    // 0 dict, 1 defaultdict, 2 Counter, 3 OrderedDict
    pub kind: u8,
    pub dflt: Option<V>,
    pub ver: u64,
}
impl Dict {
    pub fn new() -> Dict {
        Dict { ents: vec![], idx: HM::default(), n: 0, kind: 0, dflt: None, ver: 0 }
    }
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn get(&self, k: &V) -> Result<Option<V>, E> {
        let key = K { h: py_hash(k)?, v: k.clone() };
        Ok(self.idx.get(&key).map(|&i| self.ents[i].as_ref().unwrap().1.clone()))
    }
    pub fn contains(&self, k: &V) -> Result<bool, E> {
        let key = K { h: py_hash(k)?, v: k.clone() };
        Ok(self.idx.contains_key(&key))
    }
    pub fn set(&mut self, k: V, v: V) -> Result<(), E> {
        let key = K { h: py_hash(&k)?, v: k.clone() };
        if let Some(&i) = self.idx.get(&key) {
            self.ents[i].as_mut().unwrap().1 = v;
        } else {
            self.ents.push(Some((k, v)));
            self.idx.insert(key, self.ents.len() - 1);
            self.n += 1;
            self.ver += 1;
        }
        Ok(())
    }
    pub fn del(&mut self, k: &V) -> Result<Option<V>, E> {
        let key = K { h: py_hash(k)?, v: k.clone() };
        match self.idx.remove(&key) {
            Some(i) => {
                let (_, v) = self.ents[i].take().unwrap();
                self.n -= 1;
                self.ver += 1;
                if self.ents.len() > 16 && self.ents.len() > 2 * self.n {
                    self.compact();
                }
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }
    pub fn compact(&mut self) {
        let ents: Vec<Option<(V, V)>> = std::mem::take(&mut self.ents).into_iter().filter(|e| e.is_some()).collect();
        self.idx.clear();
        for (i, e) in ents.iter().enumerate() {
            let k = &e.as_ref().unwrap().0;
            self.idx.insert(K { h: py_hash(k).unwrap_or(0), v: k.clone() }, i);
        }
        self.ents = ents;
    }
    pub fn keys(&self) -> Vec<V> {
        self.ents.iter().flatten().map(|(k, _)| k.clone()).collect()
    }
    pub fn values(&self) -> Vec<V> {
        self.ents.iter().flatten().map(|(_, v)| v.clone()).collect()
    }
    pub fn items(&self) -> Vec<(V, V)> {
        self.ents.iter().flatten().cloned().collect()
    }
    pub fn clear(&mut self) {
        self.ents.clear();
        self.idx.clear();
        self.n = 0;
        self.ver += 1;
    }
    pub fn copy(&self) -> Dict {
        let mut d = Dict::new();
        d.kind = self.kind;
        d.dflt = self.dflt.clone();
        for (k, v) in self.items() {
            d.set(k, v).ok();
        }
        d
    }
    pub fn move_to_end(&mut self, k: &V, last: bool) -> Result<bool, E> {
        match self.del(k)? {
            None => Ok(false),
            Some(v) => {
                if last {
                    self.set(k.clone(), v)?;
                } else {
                    let mut rest = std::mem::take(&mut self.ents);
                    rest.insert(0, Some((k.clone(), v)));
                    self.ents = rest;
                    self.n += 1;
                    self.compact();
                }
                Ok(true)
            }
        }
    }
    // The i-th live entry from the start, skipping holes (for iteration by position).
    pub fn first_live(&self, from: usize) -> Option<usize> {
        (from..self.ents.len()).find(|&i| self.ents[i].is_some())
    }
}

// ---------- set (CPython's table layout, so iteration order matches for numbers and tuples) ----------

#[derive(Clone)]
pub enum Slot {
    Empty,
    Dummy,
    Full(V, i64),
}
#[derive(Clone)]
pub struct PSet {
    pub t: Vec<Slot>,
    pub used: usize,
    pub fill: usize,
    pub finger: usize,
}
const LINEAR_PROBES: usize = 9;
impl PSet {
    pub fn new() -> PSet {
        PSet { t: vec![Slot::Empty; 8], used: 0, fill: 0, finger: 0 }
    }
    pub fn len(&self) -> usize {
        self.used
    }
    fn mask(&self) -> usize {
        self.t.len() - 1
    }
    pub fn find(&self, k: &V, h: i64) -> Option<usize> {
        let mask = self.mask();
        let mut perturb = h as u64;
        let mut i = (h as u64 as usize) & mask;
        loop {
            let probes = if i + LINEAR_PROBES <= mask { LINEAR_PROBES } else { 0 };
            for j in 0..=probes {
                match &self.t[i + j] {
                    Slot::Empty => return None,
                    Slot::Full(x, xh) if *xh == h && eq(x, k).unwrap_or(false) => return Some(i + j),
                    _ => {}
                }
            }
            perturb >>= 5;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        }
    }
    pub fn contains(&self, k: &V) -> Result<bool, E> {
        let h = py_hash(k)?;
        Ok(self.find(k, h).is_some())
    }
    pub fn add(&mut self, k: V) -> Result<(), E> {
        let h = py_hash(&k)?;
        self.add_h(k, h);
        Ok(())
    }
    pub fn add_h(&mut self, k: V, h: i64) {
        let mask = self.mask();
        let mut perturb = h as u64;
        let mut i = (h as u64 as usize) & mask;
        loop {
            let probes = if i + LINEAR_PROBES <= mask { LINEAR_PROBES } else { 0 };
            for j in 0..=probes {
                match &self.t[i + j] {
                    Slot::Empty => {
                        self.t[i + j] = Slot::Full(k, h);
                        self.fill += 1;
                        self.used += 1;
                        if self.fill * 5 >= mask * 3 {
                            self.resize(if self.used > 50000 { self.used * 2 } else { self.used * 4 });
                        }
                        return;
                    }
                    Slot::Full(x, xh) if *xh == h && eq(x, &k).unwrap_or(false) => return,
                    _ => {}
                }
            }
            perturb >>= 5;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        }
    }
    fn insert_clean(t: &mut Vec<Slot>, k: V, h: i64) {
        let mask = t.len() - 1;
        let mut perturb = h as u64;
        let mut i = (h as u64 as usize) & mask;
        loop {
            if matches!(t[i], Slot::Empty) {
                t[i] = Slot::Full(k, h);
                return;
            }
            if i + LINEAR_PROBES <= mask {
                for j in 1..=LINEAR_PROBES {
                    if matches!(t[i + j], Slot::Empty) {
                        t[i + j] = Slot::Full(k, h);
                        return;
                    }
                }
            }
            perturb >>= 5;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        }
    }
    pub fn resize(&mut self, minused: usize) {
        let mut n = 8;
        while n <= minused {
            n <<= 1;
        }
        let old = std::mem::replace(&mut self.t, vec![Slot::Empty; n]);
        self.fill = self.used;
        for s in old {
            if let Slot::Full(k, h) = s {
                PSet::insert_clean(&mut self.t, k, h);
            }
        }
    }
    pub fn discard(&mut self, k: &V) -> Result<bool, E> {
        let h = py_hash(k)?;
        match self.find(k, h) {
            Some(i) => {
                self.t[i] = Slot::Dummy;
                self.used -= 1;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    pub fn keys(&self) -> Vec<V> {
        self.t.iter().filter_map(|s| if let Slot::Full(k, _) = s { Some(k.clone()) } else { None }).collect()
    }
    pub fn keys_h(&self) -> Vec<(V, i64)> {
        self.t.iter().filter_map(|s| if let Slot::Full(k, h) = s { Some((k.clone(), *h)) } else { None }).collect()
    }
    pub fn pop(&mut self) -> Option<V> {
        if self.used == 0 {
            return None;
        }
        let mask = self.mask();
        let mut i = self.finger & mask;
        loop {
            if let Slot::Full(_, _) = self.t[i] {
                break;
            }
            i += 1;
            if i > mask {
                i = 0;
            }
        }
        let s = std::mem::replace(&mut self.t[i], Slot::Dummy);
        self.used -= 1;
        self.finger = i + 1;
        if let Slot::Full(k, _) = s {
            Some(k)
        } else {
            None
        }
    }
    pub fn clear(&mut self) {
        *self = PSet::new();
    }
    // set(other_set) / s.copy(): CPython copies the table when the target is empty.
    pub fn copy(&self) -> PSet {
        let mut p = PSet::new();
        p.merge(self);
        p
    }
    pub fn merge(&mut self, o: &PSet) {
        if o.used == 0 {
            return;
        }
        if (self.fill + o.used) * 5 >= self.mask() * 3 {
            self.resize((self.used + o.used) * 2);
        }
        if self.fill == 0 && self.t.len() == o.t.len() {
            // copy keeping positions (no dummies)
            for (i, s) in o.t.iter().enumerate() {
                if let Slot::Full(k, h) = s {
                    self.t[i] = Slot::Full(k.clone(), *h);
                }
            }
            self.fill = o.used;
            self.used = o.used;
            return;
        }
        if self.fill == 0 {
            for s in &o.t {
                if let Slot::Full(k, h) = s {
                    PSet::insert_clean(&mut self.t, k.clone(), *h);
                }
            }
            self.fill = o.used;
            self.used = o.used;
            return;
        }
        for (k, h) in o.keys_h() {
            self.add_h(k, h);
        }
    }
    pub fn from_iter(xs: Vec<V>) -> Result<PSet, E> {
        let mut p = PSet::new();
        for x in xs {
            p.add(x)?;
        }
        Ok(p)
    }
}

// ---------- functions, classes, objects ----------

pub struct Args {
    pub p: Vec<V>,
    pub k: Vec<(Sym, V)>,
}
impl Args {
    pub fn p(p: Vec<V>) -> Args {
        Args { p, k: vec![] }
    }
    pub fn n(&self) -> usize {
        self.p.len()
    }
    pub fn kw(&mut self, name: &str) -> Option<V> {
        let i = self.k.iter().position(|(n, _)| *n == name)?;
        Some(self.k.remove(i).1)
    }
    pub fn no_kw(&self, f: &str) -> Result<(), E> {
        match self.k.first() {
            Some((n, _)) => type_err(format!("{f}() got an unexpected keyword argument '{n}'")),
            None => Ok(()),
        }
    }
    pub fn want(&self, f: &str, lo: usize, hi: usize) -> Result<(), E> {
        if self.p.len() < lo || self.p.len() > hi {
            if lo == hi {
                return type_err(format!("{f}() takes exactly {lo} argument{} ({} given)", if lo == 1 { "" } else { "s" }, self.p.len()));
            }
            if self.p.len() < lo {
                return type_err(format!("{f} expected at least {lo} argument{}, got {}", if lo == 1 { "" } else { "s" }, self.p.len()));
            }
            return type_err(format!("{f} expected at most {hi} argument{}, got {}", if hi == 1 { "" } else { "s" }, self.p.len()));
        }
        Ok(())
    }
}

pub type FnBox = Box<dyn Fn(Args) -> R>;
pub struct Func {
    pub name: Sym,
    pub f: FnBox,
    // 0 function, 1 staticmethod, 2 classmethod, 3 builtin
    pub kind: Cell<u8>,
    pub attrs: RefCell<Vec<(Sym, V)>>,
}
pub fn func(name: Sym, f: impl Fn(Args) -> R + 'static) -> V {
    V::Func(Rc::new(Func { name, f: Box::new(f), kind: Cell::new(0), attrs: RefCell::new(vec![]) }))
}
pub fn builtin(name: Sym, f: impl Fn(Args) -> R + 'static) -> V {
    V::Func(Rc::new(Func { name, f: Box::new(f), kind: Cell::new(3), attrs: RefCell::new(vec![]) }))
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BT {
    Object,
    Int,
    Float,
    Str,
    Bool,
    List,
    Tuple,
    Dict,
    Set,
    FSet,
    NoneT,
    Type,
    Func,
    Range,
    Exc,
    DefaultDict,
    Counter,
    OrderedDict,
    Deque,
    Other,
}

pub struct Class {
    pub name: Sym,
    pub module: Sym,
    pub bases: Vec<Rc<Class>>,
    pub mro: RefCell<Vec<Rc<Class>>>,
    pub attrs: RefCell<Vec<(Sym, V)>>,
    pub bt: BT,
    pub fields: RefCell<Vec<Sym>>,
}
impl Class {
    pub fn new(name: Sym, module: Sym, bases: Vec<Rc<Class>>, bt: BT) -> Rc<Class> {
        let c = Rc::new(Class { name, module, bases: bases.clone(), mro: RefCell::new(vec![]), attrs: RefCell::new(vec![]), bt, fields: RefCell::new(vec![]) });
        // C3 linearization
        let mut seqs: Vec<Vec<Rc<Class>>> = bases.iter().map(|b| b.mro.borrow().clone()).collect();
        seqs.push(bases.clone());
        let mut out = vec![c.clone()];
        loop {
            seqs.retain(|s| !s.is_empty());
            if seqs.is_empty() {
                break;
            }
            let mut pick = None;
            for s in &seqs {
                let h = &s[0];
                if !seqs.iter().any(|t| t[1..].iter().any(|x| Rc::ptr_eq(x, h))) {
                    pick = Some(h.clone());
                    break;
                }
            }
            let p = match pick {
                Some(p) => p,
                None => break,
            };
            for s in seqs.iter_mut() {
                if Rc::ptr_eq(&s[0], &p) {
                    s.remove(0);
                }
            }
            out.push(p);
        }
        *c.mro.borrow_mut() = out;
        c
    }
    pub fn own(&self, n: &str) -> Option<V> {
        self.attrs.borrow().iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone())
    }
    pub fn lookup(&self, n: &str) -> Option<V> {
        if let Some(v) = self.own(n) {
            return Some(v);
        }
        for c in self.mro.borrow().iter().skip(1) {
            if let Some(v) = c.own(n) {
                return Some(v);
            }
        }
        None
    }
    pub fn set(&self, n: Sym, v: V) {
        let mut a = self.attrs.borrow_mut();
        match a.iter_mut().find(|(k, _)| *k == n) {
            Some(e) => e.1 = v,
            None => a.push((n, v)),
        }
    }
    pub fn is_sub(&self, o: &Class) -> bool {
        self.mro.borrow().iter().any(|c| std::ptr::eq(&**c, o))
    }
    pub fn base_bt(&self) -> BT {
        for c in self.mro.borrow().iter() {
            if c.bt != BT::Object && c.bt != BT::Other {
                return c.bt;
            }
        }
        BT::Object
    }
}

pub struct Obj {
    pub cls: Rc<Class>,
    pub d: RefCell<Vec<(Sym, V)>>,
    // the builtin value a subclass of a builtin type wraps (class Stack(list))
    pub inner: RefCell<Option<V>>,
}
impl Obj {
    pub fn new(cls: Rc<Class>) -> Rc<Obj> {
        Rc::new(Obj { cls, d: RefCell::new(vec![]), inner: RefCell::new(None) })
    }
    pub fn get(&self, n: &str) -> Option<V> {
        self.d.borrow().iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone())
    }
    pub fn set(&self, n: Sym, v: V) {
        let mut a = self.d.borrow_mut();
        match a.iter_mut().find(|(k, _)| *k == n) {
            Some(e) => e.1 = v,
            None => a.push((n, v)),
        }
    }
    pub fn del(&self, n: &str) -> bool {
        let mut a = self.d.borrow_mut();
        match a.iter().position(|(k, _)| *k == n) {
            Some(i) => {
                a.remove(i);
                true
            }
            None => false,
        }
    }
}

thread_local! {
    pub static SYMS: RefCell<HashMap<String, Sym>> = RefCell::new(HashMap::new());
    pub static BUILTIN_CLASSES: RefCell<HashMap<&'static str, Rc<Class>>> = RefCell::new(HashMap::new());
}
pub fn sym(n: &str) -> Sym {
    SYMS.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(s) = m.get(n) {
            return *s;
        }
        let s: Sym = Box::leak(n.to_string().into_boxed_str());
        m.insert(n.to_string(), s);
        s
    })
}

pub fn bclass(name: &str) -> Rc<Class> {
    BUILTIN_CLASSES.with(|b| b.borrow().get(name).cloned()).unwrap()
}

pub fn init_builtin_classes() {
    let object = Class::new("object", "builtins", vec![], BT::Object);
    let mut m: HashMap<&'static str, Rc<Class>> = HashMap::new();
    m.insert("object", object.clone());
    for (n, bt) in [
        ("int", BT::Int),
        ("float", BT::Float),
        ("str", BT::Str),
        ("list", BT::List),
        ("tuple", BT::Tuple),
        ("dict", BT::Dict),
        ("set", BT::Set),
        ("frozenset", BT::FSet),
        ("NoneType", BT::NoneT),
        ("type", BT::Type),
        ("function", BT::Func),
        ("range", BT::Range),
        ("deque", BT::Deque),
    ] {
        m.insert(n, Class::new(n, "builtins", vec![object.clone()], bt));
    }
    let int = m["int"].clone();
    m.insert("bool", Class::new("bool", "builtins", vec![int], BT::Bool));
    let dict = m["dict"].clone();
    m.insert("defaultdict", Class::new("defaultdict", "collections", vec![dict.clone()], BT::DefaultDict));
    m.insert("Counter", Class::new("Counter", "collections", vec![dict.clone()], BT::Counter));
    m.insert("OrderedDict", Class::new("OrderedDict", "collections", vec![dict.clone()], BT::OrderedDict));
    let base = Class::new("BaseException", "builtins", vec![object.clone()], BT::Exc);
    m.insert("BaseException", base.clone());
    let tree: &[(&str, &str)] = &[
        ("Exception", "BaseException"),
        ("SystemExit", "BaseException"),
        ("KeyboardInterrupt", "BaseException"),
        ("GeneratorExit", "BaseException"),
        ("ArithmeticError", "Exception"),
        ("ZeroDivisionError", "ArithmeticError"),
        ("OverflowError", "ArithmeticError"),
        ("FloatingPointError", "ArithmeticError"),
        ("LookupError", "Exception"),
        ("KeyError", "LookupError"),
        ("IndexError", "LookupError"),
        ("ValueError", "Exception"),
        ("UnicodeError", "ValueError"),
        ("UnicodeDecodeError", "UnicodeError"),
        ("UnicodeEncodeError", "UnicodeError"),
        ("TypeError", "Exception"),
        ("AttributeError", "Exception"),
        ("NameError", "Exception"),
        ("UnboundLocalError", "NameError"),
        ("RuntimeError", "Exception"),
        ("RecursionError", "RuntimeError"),
        ("NotImplementedError", "RuntimeError"),
        ("StopIteration", "Exception"),
        ("AssertionError", "Exception"),
        ("ImportError", "Exception"),
        ("ModuleNotFoundError", "ImportError"),
        ("OSError", "Exception"),
        ("FileNotFoundError", "OSError"),
        ("PermissionError", "OSError"),
        ("IsADirectoryError", "OSError"),
        ("EOFError", "Exception"),
        ("MemoryError", "Exception"),
        ("BufferError", "Exception"),
        ("Warning", "Exception"),
        ("UserWarning", "Warning"),
        ("DeprecationWarning", "Warning"),
        ("JSONDecodeError", "ValueError"),
        ("InvalidOperation", "ArithmeticError"),
        ("DivisionByZero", "ZeroDivisionError"),
        ("error", "Exception"),
        ("StatisticsError", "ValueError"),
        ("Error", "Exception"),
    ];
    for (n, p) in tree {
        let parent = m[p].clone();
        let module = match *n {
            "JSONDecodeError" => "json.decoder",
            "InvalidOperation" | "DivisionByZero" => "decimal",
            "error" => "re",
            "StatisticsError" => "statistics",
            "Error" => "_csv",
            _ => "builtins",
        };
        m.insert(n, Class::new(n, module, vec![parent], BT::Exc));
    }
    let (ve, ie, te) = (m["ValueError"].clone(), m["IndexError"].clone(), m["TypeError"].clone());
    m.insert("AxisError", Class::new("AxisError", "numpy.exceptions", vec![ve.clone(), ie], BT::Exc));
    m.insert("UFuncTypeError", Class::new("UFuncTypeError", "numpy._core._exceptions", vec![te], BT::Exc));
    m.insert("binascii.Error", Class::new("Error", "binascii", vec![ve], BT::Exc));
    BUILTIN_CLASSES.with(|b| *b.borrow_mut() = m);
}

// ---------- type names, truth ----------

pub fn type_name(v: &V) -> String {
    match v {
        V::Unbound => "unbound".into(),
        V::None => "NoneType".into(),
        V::Bool(_) => "bool".into(),
        V::Int(_) | V::Big(_) => "int".into(),
        V::Float(_) => "float".into(),
        V::Str(_) => "str".into(),
        V::List(_) => "list".into(),
        V::Tuple(_) => "tuple".into(),
        V::Dict(d) => match d.borrow().kind {
            1 => "collections.defaultdict".into(),
            2 => "Counter".into(),
            3 => "collections.OrderedDict".into(),
            _ => "dict".into(),
        },
        V::Set(_) => "set".into(),
        V::FSet(_) => "frozenset".into(),
        V::Func(f) => if f.kind.get() == 3 { "builtin_function_or_method".into() } else { "function".into() },
        V::Class(_) => "type".into(),
        V::Obj(o) => o.cls.name.into(),
        V::Bound(_) | V::BMeth(_) => "method".into(),
        V::Iter(_) => "iterator".into(),
        V::Range(_) => "range".into(),
        V::Slice(_) => "slice".into(),
        V::Deque(_) => "collections.deque".into(),
        V::Module(_) => "module".into(),
        V::View(v) => ["dict_keys", "dict_values", "dict_items"][v.1 as usize].into(),
        V::Prop(_) => "property".into(),
        V::Ext(x) => x.tname().into(),
        V::NotImpl => "NotImplementedType".into(),
        V::Ellipsis => "ellipsis".into(),
    }
}
// The short name Python uses in "unsupported operand type(s)" messages.
pub fn tn(v: &V) -> String {
    match v {
        V::Dict(d) => match d.borrow().kind {
            1 => "collections.defaultdict".into(),
            2 => "Counter".into(),
            _ => "dict".into(),
        },
        _ => type_name(v),
    }
}

pub fn truth(v: &V) -> Result<bool, E> {
    Ok(match v {
        V::None => false,
        V::Bool(b) => *b,
        V::Int(i) => *i != 0,
        V::Big(b) => !b.is_zero(),
        V::Float(f) => *f != 0.0,
        V::Str(x) => !x.s.is_empty(),
        V::List(l) => !l.borrow().is_empty(),
        V::Tuple(t) => !t.is_empty(),
        V::Dict(d) => d.borrow().n > 0,
        V::Set(x) => x.borrow().used > 0,
        V::FSet(x) => x.used > 0,
        V::Deque(d) => !d.borrow().v.is_empty(),
        V::Range(_) => range_len(v) > 0,
        V::View(_) => len(v)? > 0,
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__bool__") {
                return truth(&call(&f, Args::p(vec![v.clone()]))?);
            }
            if let Some(f) = o.cls.lookup("__len__") {
                return truth(&call(&f, Args::p(vec![v.clone()]))?);
            }
            if let Some(i) = o.inner.borrow().as_ref() {
                return truth(i);
            }
            true
        }
        V::Ext(x) => match x.try_truth() {
            Some(r) => r?,
            None => x.truth(),
        },
        V::Unbound => false,
        _ => true,
    })
}
pub fn not(v: &V) -> R {
    Ok(V::Bool(!truth(v)?))
}

pub fn range_len(v: &V) -> usize {
    if let V::Range(r) = v {
        let (a, b, st) = **r;
        if st > 0 {
            if b > a {
                (((b as i128 - a as i128 - 1) / st as i128) + 1) as usize
            } else {
                0
            }
        } else if a > b {
            (((a as i128 - b as i128 - 1) / (-(st as i128))) + 1) as usize
        } else {
            0
        }
    } else {
        0
    }
}

// The builtin value a subclass instance wraps (class Stack(list): self.append(x)).
pub fn inner_of(v: &V) -> Option<V> {
    if let V::Obj(o) = v {
        return o.inner.borrow().clone();
    }
    None
}
