
// ---------- _brevi_col: column kernels for py/polars.py ----------
// Each function does one of the copy's per-row loops in Rust, with the same results the Python loop gives
// (same float operations in the same order). A kernel that meets a value it does not model returns None,
// and the copy then runs its own loop, so these are only ever a faster path.

fn col_items(v: &V) -> Option<std::cell::Ref<'_, Vec<V>>> {
    match v {
        V::List(l) => Some(l.borrow()),
        _ => None,
    }
}
// a number as the float Python's float(x) gives, for the values the kernels accept
#[inline]
fn col_f(v: &V) -> Option<f64> {
    match v {
        V::Float(f) => Some(*f),
        // i64 -> f64 rounds to nearest, ties to even, as int.__float__ does
        V::Int(i) => Some(*i as f64),
        V::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}
// polars-compute float_sum, as _fsum: the remainder first-to-last, then pairwise over blocks of 128
fn col_pairwise(xs: &[f64]) -> f64 {
    if xs.len() == 128 {
        let mut v = [0.0f64; 16];
        for c in (0..128).step_by(16) {
            for j in 0..16 {
                v[j] = v[j] + xs[c + j];
            }
        }
        let mut w = 16;
        while w > 4 {
            let h = w / 2;
            for j in 0..h {
                v[j] = v[j] + v[h + j];
            }
            w = h;
        }
        return (v[0] + v[2]) + (v[1] + v[3]);
    }
    let blocks = xs.len() / 128;
    let mid = (blocks / 2) * 128;
    col_pairwise(&xs[..mid]) + col_pairwise(&xs[mid..])
}
fn col_fsum(xs: &V) -> R {
    let Some(items) = col_items(xs) else { return Ok(V::None) };
    let mut fs = Vec::with_capacity(items.len());
    for x in items.iter() {
        match col_f(x) {
            Some(f) => fs.push(f),
            None => return Ok(V::None),
        }
    }
    let n = fs.len();
    let rem = n % 128;
    let mut rest = 0.0f64;
    for x in &fs[..rem] {
        rest = rest + x;
    }
    if n == rem {
        return Ok(V::Float(0.0 + rest));
    }
    Ok(V::Float(col_pairwise(&fs[rem..]) + rest))
}
// -1, 0 or 1 for two non-missing values, NaN the largest float (polars' total order); None when not modelled
#[inline]
fn col_order(x: &V, y: &V) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    let fo = |a: f64, b: f64| -> std::cmp::Ordering {
        match (a != a, b != b) {
            (true, true) => Equal,
            (true, false) => Greater,
            (false, true) => Less,
            _ => a.partial_cmp(&b).unwrap(),
        }
    };
    // ints up to 2**53 are exact as floats, so an int and a float compare as Python compares them
    let exact = |i: i64| i.unsigned_abs() <= 1u64 << 53;
    match (x, y) {
        (V::Int(a), V::Int(b)) => Some(a.cmp(b)),
        (V::Float(a), V::Float(b)) => Some(fo(*a, *b)),
        (V::Float(a), V::Int(b)) if exact(*b) => Some(fo(*a, *b as f64)),
        (V::Int(a), V::Float(b)) if exact(*a) => Some(fo(*a as f64, *b)),
        (V::Bool(a), V::Bool(b)) => Some(a.cmp(b)),
        (V::Str(a), V::Str(b)) => Some(a.text().as_bytes().cmp(b.text().as_bytes())),
        _ => None,
    }
}
// _compare's loop: op is ==, !=, eqm, nem (missing-aware), <, <=, >, >=
fn col_cmp(op: &str, av: &V, bv: &V) -> R {
    let (Some(a), Some(b)) = (col_items(av), col_items(bv)) else { return Ok(V::None) };
    let missing = matches!(op, "eqm" | "nem");
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    for (x, y) in a.iter().zip(b.iter()) {
        let (xn, yn) = (matches!(x, V::None), matches!(y, V::None));
        if xn || yn {
            if missing {
                let r = xn == yn;
                out.push(V::Bool(if op == "eqm" { r } else { !r }));
            } else {
                out.push(V::None);
            }
            continue;
        }
        let Some(c) = col_order(x, y) else { return Ok(V::None) };
        let r = match op {
            "==" | "eqm" => c.is_eq(),
            "!=" | "nem" => c.is_ne(),
            "<" => c.is_lt(),
            "<=" => c.is_le(),
            ">" => c.is_gt(),
            ">=" => c.is_ge(),
            _ => return Ok(V::None),
        };
        out.push(V::Bool(r));
    }
    Ok(list(out))
}
// _binop's float loop for + - * on a Float64 result
fn col_farith(op: &str, av: &V, bv: &V) -> R {
    let (Some(a), Some(b)) = (col_items(av), col_items(bv)) else { return Ok(V::None) };
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    for (x, y) in a.iter().zip(b.iter()) {
        if matches!(x, V::None) || matches!(y, V::None) {
            out.push(V::None);
            continue;
        }
        let (Some(x), Some(y)) = (col_f(x), col_f(y)) else { return Ok(V::None) };
        out.push(V::Float(match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            _ => return Ok(V::None),
        }));
    }
    Ok(list(out))
}
// _binop's integer loop for + - * with _wrap to a dtype of `bits` bits (signed or not)
fn col_iarith(op: &str, av: &V, bv: &V, bits: i64, signed: bool) -> R {
    let (Some(a), Some(b)) = (col_items(av), col_items(bv)) else { return Ok(V::None) };
    if !(1..=64).contains(&bits) || (!signed && bits == 64) {
        return Ok(V::None);
    }
    let (lo, hi): (i128, i128) = if signed { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) } else { (0, (1i128 << bits) - 1) };
    let m = 1i128 << bits;
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    for (x, y) in a.iter().zip(b.iter()) {
        if matches!(x, V::None) || matches!(y, V::None) {
            out.push(V::None);
            continue;
        }
        let iv = |v: &V| match v {
            V::Int(i) => Some(*i as i128),
            V::Bool(b) => Some(*b as i128),
            _ => None,
        };
        let (Some(x), Some(y)) = (iv(x), iv(y)) else { return Ok(V::None) };
        let r = match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            _ => return Ok(V::None),
        };
        // _wrap: values in range stay, others are taken modulo 2**bits (Python's floored %)
        let w = if lo <= r && r <= hi {
            r
        } else {
            let mut v = r.rem_euclid(m);
            if signed && v > hi {
                v -= m;
            }
            v
        };
        out.push(V::Int(w as i64));
    }
    Ok(list(out))
}
// [vals[i] for i in idx]
fn col_take(vals: &V, idx: &V) -> R {
    let (Some(v), Some(ix)) = (col_items(vals), col_items(idx)) else { return Ok(V::None) };
    let mut out = Vec::with_capacity(ix.len());
    for i in ix.iter() {
        let V::Int(i) = i else { return Ok(V::None) };
        let n = v.len() as i64;
        let j = if *i < 0 { *i + n } else { *i };
        if j < 0 || j >= n {
            return Err(exc("IndexError", "list index out of range".into()));
        }
        out.push(v[j as usize].clone());
    }
    Ok(list(out))
}
// [i for i, k in enumerate(mv) if k is True]
fn col_mask(mv: &V) -> R {
    let Some(m) = col_items(mv) else { return Ok(V::None) };
    let mut out = Vec::new();
    for (i, k) in m.iter().enumerate() {
        if matches!(k, V::Bool(true)) {
            out.push(V::Int(i as i64));
        }
    }
    Ok(list(out))
}
// _group_rows for one key column: the row indices of each distinct key, in order of first appearance, with
// _key_of's rules (every NaN one key, -0.0 the same key as 0.0); a list value is not modelled
fn col_group1(kv: &V) -> R {
    let Some(k) = col_items(kv) else { return Ok(V::None) };
    let nan_key = tup(vec![s("nan")]);
    let mut groups = Dict::new();
    let mut order: Vec<Vec<V>> = Vec::new();
    for (i, v) in k.iter().enumerate() {
        let key = match v {
            V::Float(f) if f.is_nan() => nan_key.clone(),
            V::Float(f) if *f == 0.0 => V::Float(0.0),
            V::List(_) | V::Obj(_) | V::Ext(_) => return Ok(V::None),
            v => v.clone(),
        };
        match groups.get(&key)? {
            Some(V::Int(g)) => order[g as usize].push(V::Int(i as i64)),
            _ => {
                groups.set(key, V::Int(order.len() as i64))?;
                order.push(vec![V::Int(i as i64)]);
            }
        }
    }
    Ok(list(order.into_iter().map(list).collect()))
}
// _from_values without a dtype, for a plain list of None, bools, ints, floats or strings: the dtype polars
// infers ("bool", "int", "float" or "str", from the first value) and the cast values; anything the copy
// would turn into an error or another dtype (mixed kinds, big ints, nested values, all None) is not modelled
fn col_infer(vals: &V) -> R {
    let Some(v) = col_items(vals) else { return Ok(V::None) };
    let Some(first) = v.iter().find(|x| !matches!(x, V::None)) else { return Ok(V::None) };
    let kind = match first {
        V::Bool(_) => "bool",
        V::Int(_) => "int",
        V::Float(_) => "float",
        V::Str(_) => "str",
        _ => return Ok(V::None),
    };
    let mut out = Vec::with_capacity(v.len());
    for x in v.iter() {
        let y = match (kind, x) {
            (_, V::None) => V::None,
            ("bool", V::Bool(_)) | ("int", V::Int(_)) | ("float", V::Float(_)) | ("str", V::Str(_)) => x.clone(),
            ("int", V::Bool(b)) => V::Int(*b as i64),
            ("float", V::Int(i)) => V::Float(*i as f64),
            ("float", V::Bool(b)) => V::Float(if *b { 1.0 } else { 0.0 }),
            _ => return Ok(V::None),
        };
        out.push(y);
    }
    Ok(tup(vec![s(kind), list(out)]))
}
pub fn brevi_col_fn(n: &str, a: Args) -> R {
    let p = &a.p;
    let arg = |i: usize| p.get(i).cloned().unwrap_or(V::None);
    let op = |i: usize| match p.get(i) {
        Some(V::Str(x)) => x.text().to_string(),
        _ => String::new(),
    };
    match n {
        "fsum" => col_fsum(&arg(0)),
        "cmp" => col_cmp(&op(0), &arg(1), &arg(2)),
        "farith" => col_farith(&op(0), &arg(1), &arg(2)),
        "iarith" => {
            let bits = match arg(3) {
                V::Int(b) => b,
                _ => return Ok(V::None),
            };
            col_iarith(&op(0), &arg(1), &arg(2), bits, truth(&arg(4))?)
        }
        "take" => col_take(&arg(0), &arg(1)),
        "mask" => col_mask(&arg(0)),
        "group1" => col_group1(&arg(0)),
        "infer" => col_infer(&arg(0)),
        _ => Err(unsupported(&format!("_brevi_col.{n}"))),
    }
}
