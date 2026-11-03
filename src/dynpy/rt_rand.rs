
// ---------- _brevi_mt: the Mersenne Twister core behind Brevi's own random (src/dynpy/py/random.py) ----------
// The same generator and draws as CPython's _random, so a seeded program draws the same numbers; random.py
// keeps the Python-level functions (randrange, choice, sample, ...) and calls these for the hot parts.
pub struct MtV(RefCell<([u32; 624], usize)>);
impl Ext for MtV {
    fn tname(&self) -> &'static str {
        "_brevi_mt.state"
    }
    fn repr(&self) -> String {
        "<random state>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}
fn mt_init(mt: &mut [u32; 624], s: u32) {
    mt[0] = s;
    for i in 1..624 {
        let p = mt[i - 1];
        mt[i] = 1812433253u32.wrapping_mul(p ^ (p >> 30)).wrapping_add(i as u32);
    }
}
fn mt_seed(st: &mut ([u32; 624], usize), key: &[u32]) {
    let mt = &mut st.0;
    mt_init(mt, 19650218);
    let (mut i, mut j) = (1usize, 0usize);
    let mut k = 624.max(key.len());
    while k > 0 {
        let p = mt[i - 1];
        mt[i] = (mt[i] ^ (p ^ (p >> 30)).wrapping_mul(1664525)).wrapping_add(key[j]).wrapping_add(j as u32);
        i += 1;
        j += 1;
        if i >= 624 {
            mt[0] = mt[623];
            i = 1;
        }
        if j >= key.len() {
            j = 0;
        }
        k -= 1;
    }
    k = 623;
    while k > 0 {
        let p = mt[i - 1];
        mt[i] = (mt[i] ^ (p ^ (p >> 30)).wrapping_mul(1566083941)).wrapping_sub(i as u32);
        i += 1;
        if i >= 624 {
            mt[0] = mt[623];
            i = 1;
        }
        k -= 1;
    }
    mt[0] = 0x8000_0000;
    st.1 = 624;
}
fn mt_next(st: &mut ([u32; 624], usize)) -> u32 {
    if st.1 >= 624 {
        let mt = &mut st.0;
        for kk in 0..624 {
            let y = (mt[kk] & 0x8000_0000) | (mt[(kk + 1) % 624] & 0x7FFF_FFFF);
            let mut v = mt[(kk + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                v ^= 0x9908_B0DF;
            }
            mt[kk] = v;
        }
        st.1 = 0;
    }
    let mut y = st.0[st.1];
    st.1 += 1;
    y ^= y >> 11;
    y ^= (y << 7) & 0x9D2C_5680;
    y ^= (y << 15) & 0xEFC6_0000;
    y ^= y >> 18;
    y
}
fn mt_random(st: &mut ([u32; 624], usize)) -> f64 {
    let a = (mt_next(st) >> 5) as f64;
    let b = (mt_next(st) >> 6) as f64;
    (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)
}
// getrandbits(k) for 0 < k <= 63.
fn mt_bits(st: &mut ([u32; 624], usize), k: u32) -> u64 {
    if k <= 32 {
        return (mt_next(st) >> (32 - k)) as u64;
    }
    let lo = mt_next(st) as u64;
    let hi = (mt_next(st) >> (64 - k)) as u64;
    lo | (hi << 32)
}
fn mt_below(st: &mut ([u32; 624], usize), n: u64) -> u64 {
    let k = 64 - n.leading_zeros();
    loop {
        let r = mt_bits(st, k);
        if r < n {
            return r;
        }
    }
}
fn mt_of(v: Option<&V>) -> Result<&MtV, E> {
    match v {
        Some(V::Ext(x)) => x.any().downcast_ref::<MtV>().ok_or_else(|| exc("TypeError", "not a random state".into())),
        _ => type_err("not a random state".into()),
    }
}
pub fn brevi_mt_fn(n: &str, a: Args) -> R {
    match n {
        "new" => Ok(V::Ext(Rc::new(MtV(RefCell::new(([0u32; 624], 624)))))),
        "seed" => {
            let st = mt_of(a.p.first())?;
            let mut key = vec![];
            for x in to_vec(a.p.get(1).unwrap_or(&V::None))? {
                match x {
                    V::Int(i) if (0..=u32::MAX as i64).contains(&i) => key.push(i as u32),
                    _ => return val_err("seed key words must be 32-bit ints".into()),
                }
            }
            if key.is_empty() {
                key.push(0);
            }
            mt_seed(&mut st.0.borrow_mut(), &key);
            Ok(V::None)
        }
        "next" => Ok(V::Int(mt_next(&mut mt_of(a.p.first())?.0.borrow_mut()) as i64)),
        "random" => Ok(V::Float(mt_random(&mut mt_of(a.p.first())?.0.borrow_mut()))),
        // bits(st, k) for 0 < k <= 63; below(st, n) for 0 < n < 2**63: the caller keeps bigger ones in Python.
        "bits" => match a.p.get(1) {
            Some(V::Int(k)) if (1..=63).contains(k) => Ok(V::Int(mt_bits(&mut mt_of(a.p.first())?.0.borrow_mut(), *k as u32) as i64)),
            _ => val_err("bits() takes 1 to 63".into()),
        },
        "below" => match a.p.get(1) {
            Some(V::Int(k)) if *k > 0 => Ok(V::Int(mt_below(&mut mt_of(a.p.first())?.0.borrow_mut(), *k as u64) as i64)),
            _ => val_err("below() takes a positive int".into()),
        },
        // shuffle(st, xs): CPython's shuffle of a list, in place.
        "shuffle" => {
            let st = mt_of(a.p.first())?;
            match a.p.get(1) {
                Some(V::List(l)) => {
                    let mut s = st.0.borrow_mut();
                    let n = l.borrow().len();
                    let mut i = n;
                    while i > 1 {
                        i -= 1;
                        let j = mt_below(&mut s, (i + 1) as u64) as usize;
                        l.borrow_mut().swap(i, j);
                    }
                    Ok(V::None)
                }
                _ => val_err("shuffle() takes a list".into()),
            }
        }
        _ => Err(unsupported(&format!("_brevi_mt.{n}"))),
    }
}
