
// ---------- stdin / stdout ----------

pub struct Out {
    buf: Vec<u8>,
    tty: bool,
}
thread_local! {
    pub static OUT: RefCell<Out> = RefCell::new(Out { buf: Vec::with_capacity(8192), tty: { use std::io::IsTerminal; std::io::stdout().is_terminal() } });
    pub static STDIN: RefCell<Option<std::io::BufReader<std::io::Stdin>>> = RefCell::new(None);
    pub static ARGV: RefCell<Vec<String>> = RefCell::new(vec![]);
}
pub fn out_write(t: &str) {
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        if o.buf.len() + t.len() > 8192 {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            if t.len() >= 8192 {
                std::io::stdout().write_all(t.as_bytes()).ok();
                std::io::stdout().flush().ok();
                return;
            }
        }
        o.buf.extend_from_slice(t.as_bytes());
        if o.tty && t.contains('\n') {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            std::io::stdout().flush().ok();
        }
    });
}
pub fn out_flush() {
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        let b = std::mem::take(&mut o.buf);
        std::io::stdout().write_all(&b).ok();
        std::io::stdout().flush().ok();
    });
}
pub fn err_write(t: &str) {
    std::io::stderr().write_all(t.as_bytes()).ok();
}
fn with_stdin<T>(f: impl FnOnce(&mut std::io::BufReader<std::io::Stdin>) -> T) -> T {
    STDIN.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = Some(std::io::BufReader::new(std::io::stdin()));
        }
        f(s.as_mut().unwrap())
    })
}
pub fn read_line() -> Option<String> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) => None,
            Ok(_) => Some(String::from_utf8_lossy(&buf).into_owned()),
            Err(_) => None,
        }
    })
}
pub fn read_line_bytes() -> Option<Vec<u8>> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(buf),
        }
    })
}
pub fn read_all_bytes() -> Vec<u8> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).ok();
        buf
    })
}
pub fn read_n_bytes(n: usize) -> Vec<u8> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        std::io::Read::take(r, n as u64).read_to_end(&mut buf).ok();
        buf
    })
}
// sys.stdout.buffer.write: raw bytes through the same buffer as text writes, so the two stay in order.
pub fn out_write_bytes(b: &[u8]) {
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.buf.extend_from_slice(b);
        if o.buf.len() > 8192 || (o.tty && b.contains(&b'\n')) {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            std::io::stdout().flush().ok();
        }
    });
}
pub fn read_all() -> String {
    with_stdin(|r| {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).ok();
        String::from_utf8_lossy(&buf).into_owned()
    })
}
fn file_target(f: &V) -> Result<u8, E> {
    match f {
        V::None => Ok(1),
        V::Module(&ModName("sys.stdout")) => Ok(1),
        V::Module(&ModName("sys.stderr")) => Ok(2),
        V::Obj(_) | V::Ext(_) => Ok(3),
        _ => Err(unsupported("print(file=...) to this file")),
    }
}
pub fn bi_print(mut a: Args) -> R {
    let sep = match a.kw("sep") {
        Some(V::None) | None => " ".to_string(),
        Some(V::Str(x)) => x.s.clone(),
        Some(x) => return type_err(format!("sep must be None or a string, not {}", tn(&x))),
    };
    let end = match a.kw("end") {
        Some(V::None) | None => "\n".to_string(),
        Some(V::Str(x)) => x.s.clone(),
        Some(x) => return type_err(format!("end must be None or a string, not {}", tn(&x))),
    };
    let file = a.kw("file").unwrap_or(V::None);
    let flush = a.kw("flush").map(|f| truth(&f).unwrap_or(false)).unwrap_or(false);
    a.no_kw("print")?;
    let mut t = String::new();
    for (i, x) in a.p.iter().enumerate() {
        if i > 0 {
            t.push_str(&sep);
        }
        match x {
            V::Str(x) => t.push_str(&x.s),
            _ => t.push_str(&to_str(x)?),
        }
    }
    t.push_str(&end);
    match file_target(&file)? {
        1 => {
            out_write(&t);
            if flush {
                out_flush();
            }
        }
        2 => err_write(&t),
        _ => {
            call_method(&file, "write", Args::p(vec![st(t)]))?;
        }
    }
    Ok(V::None)
}
pub fn bi_input(a: Args) -> R {
    if let Some(p) = a.p.first() {
        out_write(&to_str(p)?);
    }
    out_flush();
    match read_line() {
        None => Err(exc("EOFError", "EOF when reading a line".into())),
        Some(mut l) => {
            if l.ends_with('\n') {
                l.pop();
                if l.ends_with('\r') {
                    // Python keeps \r on Unix when reading text with universal newlines translated
                }
            }
            Ok(st(l))
        }
    }
}
pub fn stdin_lines() -> V {
    gen_iter(|| Ok(read_line().map(st)))
}

// ---------- builtin functions ----------

pub fn a1(a: &Args, f: &str) -> Result<V, E> {
    a.want(f, 1, 1)?;
    Ok(a.p[0].clone())
}
pub fn bi_len(a: Args) -> R {
    a.no_kw("len")?;
    Ok(V::Int(len(&a1(&a, "len")?)? as i64))
}
pub fn bi_repr(a: Args) -> R {
    Ok(st(repr(&a1(&a, "repr")?)?))
}
pub fn bi_ascii(a: Args) -> R {
    let r = repr(&a1(&a, "ascii")?)?;
    let mut o = String::new();
    for c in r.chars() {
        if (c as u32) < 128 {
            o.push(c);
        } else if (c as u32) < 256 {
            o.push_str(&format!("\\x{:02x}", c as u32));
        } else if (c as u32) < 65536 {
            o.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            o.push_str(&format!("\\U{:08x}", c as u32));
        }
    }
    Ok(st(o))
}
pub fn bi_str(mut a: Args) -> R {
    if a.p.is_empty() {
        return Ok(s(""));
    }
    if a.p.len() > 1 || !a.k.is_empty() {
        return Err(unsupported("str() with an encoding"));
    }
    str_v(&a.p[0])
}
pub fn bi_int(mut a: Args) -> R {
    let base = a.kw("base");
    a.no_kw("int")?;
    if a.p.len() > 2 {
        return type_err(format!("int() takes at most 2 arguments ({} given)", a.p.len()));
    }
    if a.p.is_empty() {
        if base.is_some() {
            return type_err("int() missing string argument".into());
        }
        return Ok(V::Int(0));
    }
    let base = match (a.p.get(1).cloned(), base) {
        // `int("0", base=2 ** 234)`: out of range, as any base past 36.
        (Some(V::Big(_)), _) | (None, Some(V::Big(_))) => Some(i64::MAX),
        (Some(b), _) | (None, Some(b)) => Some(idx_of(&b)?),
        _ => None,
    };
    match (base, &a.p[0]) {
        (None, x) => to_int(x),
        (Some(b), V::Str(x)) => {
            if b != 0 && !(2..=36).contains(&b) {
                return val_err("int() base must be >= 2 and <= 36, or 0".into());
            }
            int_text_limit(&x.s, b as u32, parse_int(&x.s, b as u32))?.ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base {}: {}", b, str_repr(&x.s))))
        }
        (Some(_), x) => type_err("int() can't convert non-string with explicit base".into()),
    }
}
// IntEnum and IntFlag members given to a numeric builtin (round, abs, range, ...): their int.
fn enum_nums(mut a: Args) -> Args {
    for x in a.p.iter_mut() {
        if let V::Obj(o) = x {
            if is_enum(&o.cls) {
                if let Some(i @ (V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_))) = inner_of(x) {
                    *x = i;
                }
            }
        }
    }
    a
}
pub fn bi_float(a: Args) -> R {
    let a = enum_nums(a);
    a.no_kw("float")?;
    if a.p.is_empty() {
        return Ok(V::Float(0.0));
    }
    Ok(V::Float(to_float(&a.p[0])?))
}
pub fn bi_bool(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(V::Bool(false));
    }
    Ok(V::Bool(truth(&a.p[0])?))
}
pub fn bi_list(a: Args) -> R {
    if !a.k.is_empty() {
        return type_err("list() takes no keyword arguments".into());
    }
    if a.p.is_empty() {
        return Ok(list(vec![]));
    }
    Ok(list(to_vec(&a.p[0])?))
}
pub fn bi_tuple(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(tup(vec![]));
    }
    if let V::Tuple(_) = &a.p[0] {
        return Ok(a.p[0].clone());
    }
    Ok(tup(to_vec(&a.p[0])?))
}
pub fn bi_set(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(set_new());
    }
    match &a.p[0] {
        V::Set(_) | V::FSet(_) => Ok(V::Set(Rc::new(RefCell::new(set_of(&a.p[0]).unwrap().copy())))),
        V::Dict(d) => {
            let ks = d.borrow().keys();
            let mut p = PSet::new();
            let n = ks.len();
            if n > 0 && (p.fill + n) * 5 >= 7 * 3 {
                p.resize(n * 2);
            }
            for k in ks {
                p.add(k)?;
            }
            Ok(V::Set(Rc::new(RefCell::new(p))))
        }
        x => Ok(V::Set(Rc::new(RefCell::new(PSet::from_iter(to_vec(x)?)?)))),
    }
}
pub fn bi_frozenset(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(V::FSet(Rc::new(PSet::new())));
    }
    match bi_set(a)? {
        V::Set(s) => Ok(V::FSet(Rc::new(s.borrow().clone()))),
        _ => unreachable!(),
    }
}
pub fn bi_dict(mut a: Args) -> R {
    let mut d = Dict::new();
    if let Some(x) = a.p.first() {
        for (k, v) in dict_items_of(x)? {
            d.set(k, v)?;
        }
    }
    for (k, v) in a.k.drain(..) {
        d.set(s(k), v)?;
    }
    Ok(V::Dict(Rc::new(RefCell::new(d))))
}
pub fn bi_range(a: Args) -> R {
    let a = enum_nums(a);
    a.no_kw("range")?;
    let g = |i: usize| -> Result<i64, E> {
        match &a.p[i] {
            V::Int(x) => Ok(*x),
            V::Bool(b) => Ok(*b as i64),
            V::Big(_) => Err(unsupported("range() over very large integers")),
            x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
        }
    };
    let (s0, e, st) = match a.p.len() {
        1 => (0, g(0)?, 1),
        2 => (g(0)?, g(1)?, 1),
        3 => (g(0)?, g(1)?, g(2)?),
        0 => return type_err("range expected at least 1 argument, got 0".into()),
        n => return type_err(format!("range expected at most 3 arguments, got {n}")),
    };
    if st == 0 {
        return val_err("range() arg 3 must not be zero".into());
    }
    Ok(V::Range(Rc::new((s0, e, st))))
}
pub fn bi_abs(a: Args) -> R {
    let a = enum_nums(a);
    let x = a1(&a, "abs")?;
    match &x {
        V::Int(i) => Ok(match i.checked_abs() {
            Some(r) => V::Int(r),
            None => Big::from_i64(*i).neg().norm(),
        }),
        V::Bool(b) => Ok(V::Int(*b as i64)),
        V::Big(b) => Ok(Big { neg: false, mag: b.mag.clone() }.norm()),
        V::Float(f) => Ok(V::Float(f.abs())),
        V::Obj(o) => match o.cls.lookup("__abs__") {
            Some(f) => call(&f, Args::p(vec![x.clone()])),
            None => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
        },
        V::Ext(e) => match e.method(&x, "__abs__", &mut Args::p(vec![])) {
            Some(r) => r,
            None => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
        },
        _ => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
    }
}
fn minmax(mut a: Args, want_max: bool) -> R {
    let name = if want_max { "max" } else { "min" };
    let key = a.kw("key");
    let dflt = a.kw("default");
    a.no_kw(name)?;
    let items = if a.p.len() == 1 {
        to_vec(&a.p[0])?
    } else if a.p.is_empty() {
        return type_err(format!("{name} expected at least 1 argument, got 0"));
    } else {
        if dflt.is_some() {
            return type_err(format!("Cannot specify a default for {name}() with multiple positional arguments"));
        }
        a.p.clone()
    };
    if items.is_empty() {
        return match dflt {
            Some(d) => Ok(d),
            None => val_err(format!("{name}() iterable argument is empty")),
        };
    }
    let key = key.filter(|k| !matches!(k, V::None));
    let kf = |x: &V| -> R {
        match &key {
            Some(k) => call(k, Args::p(vec![x.clone()])),
            None => Ok(x.clone()),
        }
    };
    let mut best = items[0].clone();
    let mut bk = kf(&best)?;
    for x in items.into_iter().skip(1) {
        let k = kf(&x)?;
        let better = if want_max { rich(&k, &bk, ">")? } else { rich(&k, &bk, "<")? };
        if better {
            best = x;
            bk = k;
        }
    }
    Ok(best)
}
pub fn bi_min(a: Args) -> R {
    minmax(a, false)
}
pub fn bi_max(a: Args) -> R {
    minmax(a, true)
}
pub fn bi_sum(mut a: Args) -> R {
    let start = a.kw("start");
    let mut acc = a.p.get(1).cloned().or(start).unwrap_or(V::Int(0));
    if let V::Str(_) = acc {
        return type_err("sum() can't sum strings [use ''.join(seq) instead]".into());
    }
    let xs = to_vec(a.p.first().ok_or_else(|| exc("TypeError", "sum() takes at least 1 positional argument (0 given)".into()))?)?;
    // float sums use Neumaier-free plain addition in order, as CPython does for mixed; CPython 3.12+ uses
    // compensated summation for floats.
    let all_float = xs.iter().all(|x| matches!(x, V::Float(_) | V::Int(_))) && xs.iter().any(|x| matches!(x, V::Float(_))) && matches!(acc, V::Int(_) | V::Float(_));
    if all_float {
        // CPython 3.12+: compensated (Neumaier) summation once a float appears
        let mut i_acc: Option<i64> = None;
        let mut f = 0.0f64;
        let mut c = 0.0f64;
        let mut in_float = false;
        let mut ip = match acc {
            V::Int(i) => i,
            _ => 0,
        };
        if let V::Float(x) = acc {
            f = x;
            in_float = true;
        }
        for x in &xs {
            match x {
                V::Int(i) if !in_float => match ip.checked_add(*i) {
                    Some(r) => ip = r,
                    None => return Err(unsupported("very large integer sums mixed with floats")),
                },
                V::Int(i) => {
                    let y = *i as f64;
                    let t = f + y;
                    if f.abs() >= y.abs() {
                        c += (f - t) + y;
                    } else {
                        c += (y - t) + f;
                    }
                    f = t;
                }
                V::Float(y) => {
                    if !in_float {
                        in_float = true;
                        f = ip as f64;
                    }
                    let y = *y;
                    let t = f + y;
                    if f.abs() >= y.abs() {
                        c += (f - t) + y;
                    } else {
                        c += (y - t) + f;
                    }
                    f = t;
                }
                _ => {}
            }
        }
        let _ = i_acc;
        if c != 0.0 && c.is_finite() {
            f += c;
        }
        return Ok(V::Float(f));
    }
    for x in xs {
        acc = add(&acc, &x)?;
    }
    Ok(acc)
}
pub fn bi_sorted(mut a: Args) -> R {
    let key = a.kw("key");
    let rev = a.kw("reverse").map(|r| truth(&r)).transpose()?.unwrap_or(false);
    a.no_kw("sorted")?;
    let mut v = to_vec(&a1(&a, "sorted")?)?;
    sort_values(&mut v, key.as_ref(), rev)?;
    Ok(list(v))
}
pub fn bi_reversed(a: Args) -> R {
    let x = a1(&a, "reversed")?;
    if let V::Obj(o) = &x {
        if let Some(f) = o.cls.lookup("__reversed__") {
            return call(&f, Args::p(vec![x.clone()]));
        }
    }
    if let V::Range(r) = &x {
        let n = range_len(&x) as i64;
        let last = r.0 + (n - 1) * r.2;
        return Ok(V::Iter(Rc::new(RefCell::new(It::Range(last, r.0 - r.2, -r.2)))));
    }
    if matches!(x, V::Dict(_) | V::List(_) | V::Tuple(_) | V::Str(_) | V::Deque(_) | V::View(_)) || inner_of(&x).is_some() {
        let mut v = to_vec(&x)?;
        v.reverse();
        return Ok(vec_iter(v));
    }
    type_err(format!("'{}' object is not reversible", tn(&x)))
}
pub fn bi_enumerate(mut a: Args) -> R {
    let start = a.kw("start");
    let mut k = match a.p.get(1).cloned().or(start) {
        Some(x) => idx_of(&x)?,
        None => 0,
    };
    let mut it = iter(a.p.first().ok_or_else(|| exc("TypeError", "enumerate() missing required argument 'iterable'".into()))?)?;
    Ok(gen_iter(move || {
        Ok(match it.next()? {
            Some(x) => {
                k += 1;
                Some(tup(vec![V::Int(k - 1), x]))
            }
            None => None,
        })
    }))
}
pub fn bi_zip(mut a: Args) -> R {
    let strict = a.kw("strict").map(|x| truth(&x).unwrap_or(false)).unwrap_or(false);
    let mut its: Vec<It> = vec![];
    for x in &a.p {
        its.push(iter(x)?);
    }
    if its.is_empty() {
        return Ok(vec_iter(vec![]));
    }
    Ok(gen_iter(move || {
        let mut row = Vec::with_capacity(its.len());
        for (i, it) in its.iter_mut().enumerate() {
            match it.next()? {
                Some(x) => row.push(x),
                None => {
                    if strict && i > 0 {
                        return val_err(format!("zip() argument {} is shorter than argument 1", i + 1));
                    }
                    return Ok(None);
                }
            }
        }
        Ok(Some(tup(row)))
    }))
}
pub fn bi_map(a: Args) -> R {
    if a.p.len() < 2 {
        return type_err("map() must have at least two arguments.".into());
    }
    let f = a.p[0].clone();
    let mut its: Vec<It> = vec![];
    for x in &a.p[1..] {
        its.push(iter(x)?);
    }
    Ok(gen_iter(move || {
        let mut args = Vec::with_capacity(its.len());
        for it in its.iter_mut() {
            match it.next()? {
                Some(x) => args.push(x),
                None => return Ok(None),
            }
        }
        Ok(Some(call(&f, Args::p(args))?))
    }))
}
pub fn bi_filter(a: Args) -> R {
    a.want("filter", 2, 2)?;
    let f = a.p[0].clone();
    let mut it = iter(&a.p[1])?;
    Ok(gen_iter(move || loop {
        match it.next()? {
            Some(x) => {
                let keep = if matches!(f, V::None) { truth(&x)? } else { truth(&call(&f, Args::p(vec![x.clone()]))?)? };
                if keep {
                    return Ok(Some(x));
                }
            }
            None => return Ok(None),
        }
    }))
}
pub fn bi_any(a: Args) -> R {
    let mut it = iter(&a1(&a, "any")?)?;
    while let Some(x) = it.next()? {
        if truth(&x)? {
            return Ok(V::Bool(true));
        }
    }
    Ok(V::Bool(false))
}
pub fn bi_all(a: Args) -> R {
    let mut it = iter(&a1(&a, "all")?)?;
    while let Some(x) = it.next()? {
        if !truth(&x)? {
            return Ok(V::Bool(false));
        }
    }
    Ok(V::Bool(true))
}
pub fn bi_iter(a: Args) -> R {
    if a.p.len() == 2 {
        let f = a.p[0].clone();
        let sentinel = a.p[1].clone();
        return Ok(gen_iter(move || {
            let v = call(&f, Args::p(vec![]))?;
            if eq(&v, &sentinel)? {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }));
    }
    iter_v(&a1(&a, "iter")?)
}
pub fn bi_next(a: Args) -> R {
    a.want("next", 1, 2)?;
    let r = match &a.p[0] {
        V::Iter(it) => match coro_of(it) {
            // A generator's return value travels on the StopIteration that next() raises.
            Some(c) => {
                let r = c.borrow_mut().resume(V::None);
                match r {
                    Ok(Some(v)) => Some(v),
                    Ok(None) if a.p.len() == 1 => return Err(stop_with(V::None)),
                    Ok(None) => None,
                    Err(E(e)) if is_exc(&e, "StopIteration") && a.p.len() == 1 => return Err(E(e)),
                    Err(E(e)) if is_exc(&e, "StopIteration") => None,
                    Err(e) => return Err(e),
                }
            }
            None => it.borrow_mut().next()?,
        },
        V::Obj(o) => match o.cls.lookup("__next__") {
            Some(f) => match call(&f, Args::p(vec![a.p[0].clone()])) {
                Ok(v) => Some(v),
                Err(E(e)) if is_exc(&e, "StopIteration") => None,
                Err(e) => return Err(e),
            },
            None => return type_err(format!("'{}' object is not an iterator", tn(&a.p[0]))),
        },
        x => return type_err(format!("'{}' object is not an iterator", tn(x))),
    };
    match r {
        Some(v) => Ok(v),
        None => match a.p.get(1) {
            Some(d) => Ok(d.clone()),
            None => Err(exc0("StopIteration")),
        },
    }
}
pub fn bi_isinstance(a: Args) -> R {
    a.want("isinstance", 2, 2)?;
    Ok(V::Bool(isinstance(&a.p[0], &a.p[1])?))
}
pub fn isinstance(x: &V, c: &V) -> Result<bool, E> {
    match c {
        V::Tuple(t) => {
            for k in t.iter() {
                if isinstance(x, k)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        V::Class(k) => {
            if let V::Obj(o) = x {
                return Ok(o.cls.is_sub(k) || (k.name == "object" && k.module == "builtins"));
            }
            if let V::Ext(e) = x {
                if let Some(b) = e.isinst(k) {
                    return Ok(b);
                }
            }
            Ok(type_of(x).is_sub(k))
        }
        V::Ext(e) => match e.instancecheck(x) {
            Some(r) => r,
            None => type_err("isinstance() arg 2 must be a type, a tuple of types, or a union".into()),
        },
        // the builtin slice is a function here (builtin_value), the type of slice objects
        V::Func(f) if &*f.name == "slice" => Ok(matches!(x, V::Slice(_))),
        _ => type_err("isinstance() arg 2 must be a type, a tuple of types, or a union".into()),
    }
}
pub fn type_of(x: &V) -> Rc<Class> {
    match x {
        V::Obj(o) => o.cls.clone(),
        V::Bool(_) => tclass(0, "bool"),
        V::Int(_) | V::Big(_) => tclass(1, "int"),
        V::Float(_) => tclass(2, "float"),
        V::Str(_) => tclass(3, "str"),
        V::List(_) => tclass(4, "list"),
        V::Tuple(_) => tclass(5, "tuple"),
        V::Dict(d) => match d.borrow().kind {
            1 => tclass(6, "defaultdict"),
            2 => tclass(7, "Counter"),
            3 => tclass(8, "OrderedDict"),
            _ => tclass(9, "dict"),
        },
        V::Set(_) => tclass(10, "set"),
        V::FSet(_) => tclass(11, "frozenset"),
        V::None => tclass(12, "NoneType"),
        V::Class(_) => tclass(13, "type"),
        V::Func(_) | V::Bound(_) | V::BMeth(_) => tclass(14, "function"),
        V::Range(_) => tclass(15, "range"),
        V::Deque(_) => tclass(16, "deque"),
        V::View(x) if x.1 == 3 => tclass(17, "mappingproxy"),
        V::Ext(e) => ext_class(e.tname()),
        _ => tclass(18, "object"),
    }
}
thread_local! {
    // type_of's builtin classes, filled on first use (init_builtin_classes empties it)
    pub static TYPE_CACHE: RefCell<[Option<Rc<Class>>; 19]> = RefCell::new(Default::default());
}
#[inline]
fn tclass(i: usize, n: &'static str) -> Rc<Class> {
    TYPE_CACHE.with(|c| {
        if let Some(k) = &c.borrow()[i] {
            return k.clone();
        }
        let k = bclass(n);
        c.borrow_mut()[i] = Some(k.clone());
        k
    })
}
thread_local! {
    pub static EXT_CLASSES: RefCell<HashMap<&'static str, Rc<Class>>> = RefCell::new(HashMap::new());
}
pub fn ext_class(n: &'static str) -> Rc<Class> {
    EXT_CLASSES.with(|m| {
        let mut m = m.borrow_mut();
        m.entry(n).or_insert_with(|| Class::new(n, "builtins", vec![bclass("object")], BT::Other)).clone()
    })
}
pub fn bi_type(a: Args) -> R {
    if a.p.len() == 3 {
        let name = sym(&to_str(&a.p[0])?);
        let bases = to_vec(&a.p[1])?;
        let attrs: Vec<(Sym, V)> = dict_items_of(&a.p[2])?.into_iter().map(|(k, v)| (sym(&to_str(&k).unwrap_or_default()), v)).collect();
        return make_class(name, "__main__", bases, attrs);
    }
    Ok(V::Class(type_of(&a1(&a, "type")?)))
}
pub fn bi_issubclass(a: Args) -> R {
    a.want("issubclass", 2, 2)?;
    let sub = match &a.p[0] {
        V::Class(c) => c.clone(),
        _ => return type_err("issubclass() arg 1 must be a class".into()),
    };
    let check = |k: &V| -> Result<bool, E> {
        match k {
            V::Class(k) => Ok(sub.is_sub(k)),
            _ => type_err("issubclass() arg 2 must be a class, a tuple of classes, or a union".into()),
        }
    };
    match &a.p[1] {
        V::Tuple(t) => {
            for k in t.iter() {
                if check(k)? {
                    return Ok(V::Bool(true));
                }
            }
            Ok(V::Bool(false))
        }
        k => Ok(V::Bool(check(k)?)),
    }
}
pub fn bi_hasattr(a: Args) -> R {
    a.want("hasattr", 2, 2)?;
    Ok(V::Bool(hasattr(&a.p[0], &to_str(&a.p[1])?)?))
}
pub fn bi_getattr(a: Args) -> R {
    a.want("getattr", 2, 3)?;
    let n = to_str(&a.p[1])?;
    // getattr(f, "name", default) on a def without that attribute (jinja2's filters, pydantic's validators):
    // the default, without making and catching an AttributeError. Dunder names keep the general path.
    if a.p.len() == 3 && !n.starts_with("__") {
        if let V::Func(f) = &a.p[0] {
            let found = f.attrs.borrow().iter().find(|(k, _)| **k == *n).map(|(_, v)| v.clone());
            return Ok(found.unwrap_or_else(|| a.p[2].clone()));
        }
    }
    match getattr(&a.p[0], &n) {
        Ok(v) => Ok(v),
        Err(E(e)) if is_exc(&e, "AttributeError") && a.p.len() == 3 => Ok(a.p[2].clone()),
        Err(e) => Err(e),
    }
}
pub fn bi_setattr(a: Args) -> R {
    a.want("setattr", 3, 3)?;
    setattr(&a.p[0], sym(&to_str(&a.p[1])?), a.p[2].clone())?;
    Ok(V::None)
}
pub fn bi_delattr(a: Args) -> R {
    a.want("delattr", 2, 2)?;
    delattr(&a.p[0], &to_str(&a.p[1])?)?;
    Ok(V::None)
}
pub fn bi_callable(a: Args) -> R {
    let x = a1(&a, "callable")?;
    Ok(V::Bool(match &x {
        V::Func(_) | V::Bound(_) | V::BMeth(_) | V::Class(_) => true,
        V::Obj(o) => o.cls.lookup("__call__").is_some(),
        _ => false,
    }))
}
pub fn bi_ord(a: Args) -> R {
    match &a1(&a, "ord")? {
        V::Str(x) if x.nchars() == 1 => Ok(V::Int(x.s.chars().next().unwrap() as i64)),
        V::Str(x) => type_err(format!("ord() expected a character, but string of length {} found", x.nchars())),
        x => type_err(format!("ord() expected string of length 1, but {} found", tn(x))),
    }
}
pub fn bi_chr(a: Args) -> R {
    let a = enum_nums(a);
    let i = idx_of(&a1(&a, "chr")?)?;
    match u32::try_from(i).ok().and_then(char::from_u32) {
        Some(c) => Ok(ch1(c)),
        None => val_err("chr() arg not in range(0x110000)".into()),
    }
}
fn radix(a: Args, f: &str, r: u32, p: &str) -> R {
    let x = a1(&a, f)?;
    let b = big_of(&x).ok_or_else(|| exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(&x))))?;
    let body = Big { neg: false, mag: b.mag.clone() }.to_radix(r);
    Ok(st(format!("{}{}{}", if b.neg { "-" } else { "" }, p, body)))
}
pub fn bi_hex(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "hex", 16, "0x")
}
pub fn bi_oct(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "oct", 8, "0o")
}
pub fn bi_bin(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "bin", 2, "0b")
}
pub fn bi_round(a: Args) -> R {
    let mut a = enum_nums(a);
    let nd = a.kw("ndigits");
    a.want("round", 1, 2)?;
    let x = a.p[0].clone();
    let nd = a.p.get(1).cloned().or(nd).filter(|n| !matches!(n, V::None));
    if let V::Obj(o) = &x {
        if let Some(f) = o.cls.lookup("__round__") {
            let mut args = vec![x.clone()];
            if let Some(n) = nd {
                args.push(n);
            }
            return call(&f, Args::p(args));
        }
    }
    if let V::Ext(e) = &x {
        let mut args = Args::p(nd.into_iter().collect());
        if let Some(r) = e.method(&x, "__round__", &mut args) {
            return r;
        }
        return type_err(format!("type {} doesn't define __round__ method", tn(&x)));
    }
    match (&x, nd) {
        (V::Int(_) | V::Big(_) | V::Bool(_), None) => to_int(&x),
        (V::Int(i), Some(n)) => {
            let n = idx_of(&n)?;
            if n >= 0 {
                return Ok(V::Int(*i));
            }
            if n < -18 {
                return Ok(V::Int(0));
            }
            let p = 10i64.pow((-n) as u32);
            let q = i.div_euclid(p);
            let r = i.rem_euclid(p);
            let q = if r * 2 > p || (r * 2 == p && q % 2 != 0) { q + 1 } else { q };
            Ok(V::Int(q * p))
        }
        (V::Float(f), None) => {
            if f.is_nan() {
                return val_err("cannot convert float NaN to integer".into());
            }
            if f.is_infinite() {
                return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
            }
            to_int(&V::Float(f.round_ties_even()))
        }
        (V::Float(f), Some(n)) => {
            let n = idx_of(&n)?;
            if !f.is_finite() {
                return Ok(x.clone());
            }
            if n >= 0 {
                if n > 300 {
                    return Ok(x.clone());
                }
                let t = format!("{:.*}", n as usize, f);
                Ok(V::Float(t.parse::<f64>().unwrap()))
            } else {
                let p = 10f64.powi((-n) as i32);
                let y = (f / p).round_ties_even() * p;
                Ok(V::Float(y))
            }
        }
        _ => type_err(format!("type {} doesn't define __round__ method", tn(&x))),
    }
}
pub fn bi_divmod(a: Args) -> R {
    let a = enum_nums(a);
    a.want("divmod", 2, 2)?;
    let q = binop(&a.p[0], &a.p[1], "//")?;
    let r = binop(&a.p[0], &a.p[1], "%")?;
    Ok(tup(vec![q, r]))
}
pub fn bi_pow(a: Args) -> R {
    let a = enum_nums(a);
    a.want("pow", 2, 3)?;
    if a.p.len() == 3 && !matches!(a.p[2], V::None) {
        let (b, e, m) = (big_of(&a.p[0]), big_of(&a.p[1]), big_of(&a.p[2]));
        let (mut b, mut e, m) = match (b, e, m) {
            (Some(b), Some(e), Some(m)) => (b, e, m),
            _ => return type_err("pow() 3rd argument not allowed unless all arguments are integers".into()),
        };
        if m.is_zero() {
            return val_err("pow() 3rd argument cannot be 0".into());
        }
        if e.neg {
            return Err(unsupported("pow() with a negative exponent and a modulus"));
        }
        let mut r = Big::from_i64(1).divmod(&m).1;
        b = b.divmod(&m).1;
        while !e.is_zero() {
            if e.mag[0] & 1 == 1 {
                r = r.mul(&b).divmod(&m).1;
            }
            e = e.shr(1);
            b = b.mul(&b).divmod(&m).1;
        }
        return Ok(r.norm());
    }
    binop(&a.p[0], &a.p[1], "**")
}
pub fn bi_hash(a: Args) -> R {
    Ok(V::Int(py_hash(&a1(&a, "hash")?)?))
}
pub fn bi_id(a: Args) -> R {
    let x = a1(&a, "id")?;
    Ok(V::Int(match &x {
        V::Obj(o) => Rc::as_ptr(o) as *const u8 as i64,
        V::List(o) => Rc::as_ptr(o) as *const u8 as i64,
        V::Dict(o) => Rc::as_ptr(o) as *const u8 as i64,
        V::Str(o) => Rc::as_ptr(o) as *const u8 as i64,
        V::Tuple(o) => Tup::as_ptr(o) as *const u8 as i64,
        V::Int(i) => *i * 32 + 1,
        _ => 1,
    }))
}
pub fn bi_format(a: Args) -> R {
    a.want("format", 1, 2)?;
    let spec = match a.p.get(1) {
        Some(x) => to_str(x)?,
        None => String::new(),
    };
    fmt_v(&a.p[0], &spec)
}
pub fn bi_vars(a: Args) -> R {
    getattr(&a1(&a, "vars")?, "__dict__")
}
pub fn bi_exit(a: Args) -> R {
    let code = a.p.first().cloned().unwrap_or(V::None);
    let c = exc_class("SystemExit");
    let o = Obj::new(c);
    o.set("args", tup(vec![code.clone()]));
    o.set("code", code);
    Err(E(V::Obj(o)))
}
pub fn bi_open(_a: Args) -> R {
    Err(unsupported("open()"))
}
pub fn bi_staticmethod(a: Args) -> R {
    let f = a1(&a, "staticmethod")?;
    Ok(match &f {
        V::Func(x) => {
            let g = rewrap(x, 1);
            g
        }
        _ => f,
    })
}
pub fn bi_classmethod(a: Args) -> R {
    let f = a1(&a, "classmethod")?;
    Ok(match &f {
        V::Func(x) => rewrap(x, 2),
        _ => f,
    })
}
fn rewrap(x: &Rc<Func>, kind: u8) -> V {
    let inner = V::Func(x.clone());
    let g = func(x.name, move |a: Args| call(&inner, a));
    if let V::Func(gf) = &g {
        gf.kind.set(kind);
        // the wrapped function's attributes (__doc__, __bv_np__) read through, as through CPython's __func__
        *gf.attrs.borrow_mut() = x.attrs.borrow().clone();
    }
    g
}
pub fn bi_property(mut a: Args) -> R {
    let getter = a.p.first().cloned().or(a.kw("fget")).unwrap_or(V::None);
    let setter = a.p.get(1).cloned().or(a.kw("fset")).unwrap_or(V::None);
    Ok(V::Prop(Rc::new((getter, setter))))
}
pub fn bi_object(_a: Args) -> R {
    Ok(V::Obj(Obj::new(bclass("object"))))
}
pub fn bi_globals(_a: Args) -> R {
    Err(unsupported("globals()"))
}
pub fn bi_slice(a: Args) -> R {
    Ok(match a.p.len() {
        1 => slice(V::None, a.p[0].clone(), V::None),
        2 => slice(a.p[0].clone(), a.p[1].clone(), V::None),
        _ => slice(a.p[0].clone(), a.p[1].clone(), a.p[2].clone()),
    })
}

pub fn builtin_fn(name: &str) -> Option<fn(Args) -> R> {
    Some(match name {
        "print" => bi_print,
        "input" => bi_input,
        "len" => bi_len,
        "repr" => bi_repr,
        "ascii" => bi_ascii,
        "abs" => bi_abs,
        "min" => bi_min,
        "max" => bi_max,
        "sum" => bi_sum,
        "sorted" => bi_sorted,
        "reversed" => bi_reversed,
        "enumerate" => bi_enumerate,
        "zip" => bi_zip,
        "map" => bi_map,
        "filter" => bi_filter,
        "any" => bi_any,
        "all" => bi_all,
        "iter" => bi_iter,
        "next" => bi_next,
        "isinstance" => bi_isinstance,
        "issubclass" => bi_issubclass,
        "hasattr" => bi_hasattr,
        "getattr" => bi_getattr,
        "setattr" => bi_setattr,
        "delattr" => bi_delattr,
        "callable" => bi_callable,
        "ord" => bi_ord,
        "chr" => bi_chr,
        "hex" => bi_hex,
        "oct" => bi_oct,
        "bin" => bi_bin,
        "round" => bi_round,
        "divmod" => bi_divmod,
        "pow" => bi_pow,
        "hash" => bi_hash,
        "id" => bi_id,
        "format" => bi_format,
        "vars" => bi_vars,
        "exit" | "quit" => bi_exit,
        "open" => bi_open,
        "staticmethod" => bi_staticmethod,
        "classmethod" => bi_classmethod,
        "property" => bi_property,
        "globals" | "locals" => bi_globals,
        "aiter" => bi_aiter,
        "anext" => bi_anext,
        "__brevi_aenter" => bi___brevi_aenter,
        "__brevi_aexit" => bi___brevi_aexit,
        "__brevi_eg_split" => bi___brevi_eg_split,
        "__brevi_eg_reraise" => bi___brevi_eg_reraise,
        _ => return None,
    })
}
// Builtin names that are types (callable classes).
pub fn builtin_type_name(name: &str) -> bool {
    matches!(name, "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "slice")
}
thread_local! {
    // builtin classes found by builtin_value, by the address of the name asked for (a hit is checked against
    // the class's own name, so a reused address cannot give a wrong class); emptied by init_builtin_classes
    pub static BV_CACHE: RefCell<[(usize, Option<Rc<Class>>); 16]> = RefCell::new(Default::default());
}
pub fn builtin_value(name: &str) -> Option<V> {
    let addr = name.as_ptr() as usize;
    let slot = (addr >> 3 ^ addr >> 8 ^ name.len()) & 15;
    let hit = BV_CACHE.with(|c| match &c.borrow()[slot] {
        (a, Some(k)) if *a == addr && same_name(k.name, name) => Some(k.clone()),
        _ => None,
    });
    if let Some(k) = hit {
        return Some(V::Class(k));
    }
    if builtin_type_name(name) && name != "slice" && name != "bytes" {
        let k = bclass(name);
        if same_name(k.name, name) {
            BV_CACHE.with(|c| c.borrow_mut()[slot] = (addr, Some(k.clone())));
        }
        return Some(V::Class(k));
    }
    if name == "bytes" {
        return Some(V::Class(ext_class("bytes")));
    }
    if builtin_type_name(name) && name != "slice" {
        return Some(V::Class(bclass(name)));
    }
    if name == "slice" {
        return Some(builtin("slice", bi_slice));
    }
    if let Some(f) = builtin_fn(name) {
        let n: Sym = sym(name);
        return Some(builtin(n, f));
    }
    match name {
        "None" => Some(V::None),
        "True" => Some(V::Bool(true)),
        "False" => Some(V::Bool(false)),
        "NotImplemented" => Some(V::NotImpl),
        "Ellipsis" => Some(V::Ellipsis),
        _ => BUILTIN_CLASSES.with(|b| b.borrow().get(name).cloned()).map(V::Class),
    }
}
// int(x), list(x), dict(...) and so on when called through the class value.
pub fn builtin_type_call(c: &Rc<Class>, a: &mut Args) -> Option<R> {
    if c.name == "bytes" && c.module == "builtins" {
        return Some(bi_bytes(Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) }));
    }
    // Other builtin classes (the exceptions) keep their arguments for the caller.
    if !matches!(c.name, "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "defaultdict" | "Counter" | "OrderedDict" | "deque") {
        return None;
    }
    let a = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
    Some(match c.name {
        "int" => bi_int(a),
        "float" => bi_float(a),
        "str" => bi_str(a),
        "bool" => bi_bool(a),
        "list" => bi_list(a),
        "tuple" => bi_tuple(a),
        "dict" => bi_dict(a),
        "set" => bi_set(a),
        "frozenset" => bi_frozenset(a),
        "object" => bi_object(a),
        "type" => bi_type(a),
        "range" => bi_range(a),
        "defaultdict" => col_defaultdict(a),
        "Counter" => col_counter(a),
        "OrderedDict" => {
            let d = bi_dict(a);
            if let Ok(V::Dict(x)) = &d {
                x.borrow_mut().kind = 3;
            }
            d
        }
        "deque" => col_deque(a),
        _ => return None,
    })
}
pub fn builtin_class_attr(c: &Rc<Class>, name: &str) -> Option<V> {
    // dict.fromkeys, str.maketrans, int.from_bytes ... and unbound methods (str.lower)
    match (c.name, name) {
        ("bytes", "fromhex") => Some(builtin("fromhex", bytes_fromhex)),
        // an instance of a plain class, its __init__ not run
        ("object", "__new__") => Some(builtin("__new__", |a: Args| match a.p.first() {
            Some(V::Class(c)) if matches!(c.base_bt(), BT::Object | BT::Other) && c.module != "typing" && !is_enum(c) => Ok(V::Obj(Obj::new(c.clone()))),
            Some(V::Class(c)) => Err(unsupported(&format!("object.__new__({})", c.name))),
            _ => type_err("object.__new__(X): X is not a type object".into()),
        })),
        ("object", "__init_subclass__") => Some(builtin("__init_subclass__", |_a: Args| Ok(V::None))),
        // the lookup without the class's __getattr__ fallback
        ("object", "__getattribute__") => Some(builtin("__getattribute__", |a: Args| {
            let (Some(o), Some(V::Str(n))) = (a.p.first(), a.p.get(1)) else {
                return type_err("object.__getattribute__(o, name): name must be a string".into());
            };
            match o {
                V::Obj(ob) => {
                    if n.s == "__dict__" && ob.cls.lookup(&n.s).is_none() {
                        return ob.dict();
                    }
                    if let Some(v) = ob.get_attr(&n.s)? {
                        return Ok(v);
                    }
                    if ob.cls.lookup(&n.s).is_some() || n.s == "__class__" || n.s == "__dict__" {
                        return getattr(o, &n.s);
                    }
                    Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, n.s)))
                }
                _ => getattr(o, &n.s),
            }
        })),
        ("dict", "fromkeys") | ("OrderedDict", "fromkeys") | ("Counter", "fromkeys") => Some(builtin("fromkeys", |a: Args| {
            let v = a.p.get(1).cloned().unwrap_or(V::None);
            let mut d = Dict::new();
            for k in to_vec(&a.p[0])? {
                d.set(k, v.clone())?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        })),
        // only defaultdict has __missing__ (dict subclasses test `hasattr(dict, "__missing__")`)
        _ if name == "__missing__" && c.name != "defaultdict" => None,
        _ => {
            let n = sym(name);
            let cname = c.name;
            Some(builtin(n, move |mut a: Args| {
                if a.p.is_empty() {
                    return type_err(format!("descriptor '{n}' needs an argument"));
                }
                let me = a.p.remove(0);
                // Base.method(self, ...) on an instance of a subclass: the base's own method, not an override
                if let V::Obj(o) = &me {
                    let mro = o.cls.mro.borrow().clone();
                    if let Some(i) = mro.iter().position(|k| k.name == cname && (k.module == "builtins" || k.module == "collections")) {
                        if i > 0 {
                            let f = super_attr(&V::Class(mro[i - 1].clone()), &me, n)?;
                            return call(&f, a);
                        }
                    }
                }
                call_method(&me, n, a)
            }))
        }
    }
}

// ---------- collections ----------

pub fn col_defaultdict(mut a: Args) -> R {
    let f = if a.p.is_empty() { V::None } else { a.p.remove(0) };
    let d = bi_dict(a)?;
    if let V::Dict(x) = &d {
        let mut x = x.borrow_mut();
        x.kind = 1;
        x.dflt = if matches!(f, V::None) { None } else { Some(f) };
    }
    Ok(d)
}
pub fn col_counter(mut a: Args) -> R {
    let mut d = Dict::new();
    d.kind = 2;
    let v = V::Dict(Rc::new(RefCell::new(d)));
    counter_update(&v, a, true)?;
    Ok(v)
}
fn counter_update(c: &V, mut a: Args, add: bool) -> R {
    let d = match c {
        V::Dict(d) => d.clone(),
        _ => return Ok(V::None),
    };
    let mut pairs: Vec<(V, V)> = vec![];
    if let Some(src) = a.p.first() {
        match src {
            V::Dict(_) => pairs = dict_items_of(src)?,
            x => {
                for k in to_vec(x)? {
                    pairs.push((k, V::Int(1)));
                }
            }
        }
    }
    for (k, v) in a.k.drain(..) {
        pairs.push((s(k), v));
    }
    for (k, v) in pairs {
        let cur = Dict::get_cell(&d, &k)?.unwrap_or(V::Int(0));
        let n = if add { binop(&cur, &v, "+")? } else { binop(&cur, &v, "-")? };
        Dict::set_cell(&d, k, n)?;
    }
    Ok(V::None)
}
pub fn col_deque(mut a: Args) -> R {
    let max = match a.kw("maxlen").or_else(|| a.p.get(1).cloned()) {
        Some(V::None) | None => None,
        Some(m) => Some(idx_of(&m)? as usize),
    };
    let mut v = VecDeque::new();
    if let Some(x) = a.p.first() {
        for y in to_vec(x)? {
            v.push_back(y);
            if let Some(m) = max {
                if v.len() > m {
                    v.pop_front();
                }
            }
        }
    }
    Ok(V::Deque(Rc::new(RefCell::new(Deq { v, max }))))
}

// ---------- methods of builtin types ----------

pub fn has_method(o: &V, name: &str) -> bool {
    let names: &[&str] = match o {
        V::Str(_) => &["split", "rsplit", "strip", "lstrip", "rstrip", "join", "replace", "find", "rfind", "index", "rindex", "count", "startswith", "endswith", "upper", "lower", "title", "capitalize", "swapcase", "isdigit", "isalpha", "isalnum", "isspace", "isupper", "islower", "isnumeric", "isdecimal", "isidentifier", "istitle", "isprintable", "isascii", "ljust", "rjust", "center", "zfill", "partition", "rpartition", "splitlines", "format", "casefold", "removeprefix", "removesuffix", "expandtabs", "encode", "translate", "maketrans", "format_map"],
        V::List(_) => &["append", "extend", "insert", "pop", "remove", "index", "count", "sort", "reverse", "copy", "clear"],
        V::Tuple(_) => &["count", "index"],
        V::Dict(_) => &["get", "setdefault", "pop", "popitem", "keys", "values", "items", "update", "copy", "clear", "most_common", "elements", "subtract", "total", "move_to_end", "fromkeys"],
        V::Set(_) | V::FSet(_) => &["add", "remove", "discard", "pop", "union", "intersection", "difference", "symmetric_difference", "issubset", "issuperset", "isdisjoint", "update", "intersection_update", "difference_update", "symmetric_difference_update", "copy", "clear"],
        V::Deque(_) => &["append", "appendleft", "pop", "popleft", "extend", "extendleft", "rotate", "clear", "copy", "count", "index", "remove", "reverse", "insert"],
        V::Int(_) | V::Big(_) | V::Bool(_) => &["bit_length", "to_bytes", "conjugate", "bit_count", "as_integer_ratio", "is_integer"],
        V::Float(_) => &["is_integer", "as_integer_ratio", "hex", "conjugate"],
        V::Iter(_) => &["__next__", "__iter__", "send", "throw", "close"],
        V::Ext(_) => return true,
        _ => &[],
    };
    // Dunder methods (`__len__`, `__add__`, ...) are there; dunder data that only functions, classes and modules
    // carry is not: `hasattr("s", "__name__")` is False, as in CPython (argparse tests it on every value).
    const NOT_ON_VALUES: &[&str] = &["__name__", "__qualname__", "__dict__", "__module__", "__wrapped__", "__self__", "__func__", "__code__", "__defaults__", "__kwdefaults__", "__annotations__", "__file__", "__all__", "__mro__", "__bases__", "__members__", "__dataclass_fields__", "__slots__", "__origin__", "__args__", "__version__", "__path__", "__closure__", "__globals__", "__match_args__"];
    // Protocol methods numbers and None do not have, and the ones containers do not (hasattr(3, "__iter__") is
    // False: requests and httpx test values that way).
    const NOT_ON_ANY: &[&str] = &["__next__", "__call__", "__enter__", "__exit__", "__await__", "__aiter__", "__anext__", "__aenter__", "__aexit__", "__fspath__", "__missing__", "read", "__get__", "__set__", "__html__", "__html_format__", "__conform__"];
    const NOT_ON_SCALARS: &[&str] = &["__iter__", "__len__", "__getitem__", "__contains__", "__setitem__", "__delitem__", "__reversed__"];
    if names.contains(&name) {
        return true;
    }
    if name.starts_with("__") || name == "read" {
        let scalar = matches!(o, V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_) | V::None);
        // (iterators and generators have __next__ and __iter__, listed above, and none of the others)
        let container = matches!(o, V::Str(_) | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) | V::FSet(_) | V::Deque(_) | V::Iter(_));
        if (scalar || container) && NOT_ON_ANY.contains(&name) {
            return false;
        }
        // sqlite3's adaptation protocol: no library value has it
        if name == "__conform__" && matches!(o, V::Ext(_)) {
            return false;
        }
        if scalar && NOT_ON_SCALARS.contains(&name) {
            return false;
        }
        if matches!(o, V::Set(_) | V::FSet(_)) && matches!(name, "__getitem__" | "__setitem__" | "__reversed__") {
            return false;
        }
    }
    names.contains(&name) || (name.starts_with("__") && !NOT_ON_VALUES.contains(&name))
}
fn bad_method(o: &V, name: &str) -> R {
    Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", tn(o), name)))
}
fn arg_str(v: &V, f: &str) -> Result<Rc<S>, E> {
    match v {
        V::Str(x) => Ok(x.clone()),
        x => type_err(format!("{f} arg must be None or str, not {}", tn(x))),
    }
}
fn opt_idx(a: &Args, i: usize) -> Result<Option<i64>, E> {
    match a.p.get(i) {
        None | Some(V::None) => Ok(None),
        // `xs.index(v, -4 * sys.maxsize)`: past 64 bits, clamped as CPython clamps it to the ends.
        Some(V::Big(b)) => Ok(Some(if b.neg { i64::MIN / 4 } else { i64::MAX / 4 })),
        Some(x) => Ok(Some(idx_of(x)?)),
    }
}
// char positions <-> byte offsets
fn char_range(st: &S, a: Option<i64>, b: Option<i64>) -> (usize, usize) {
    let n = st.nchars() as i64;
    let fix = |x: i64| -> i64 {
        if x < 0 {
            (x + n).max(0)
        } else {
            x.min(n)
        }
    };
    let s0 = fix(a.unwrap_or(0)) as usize;
    let e0 = fix(b.unwrap_or(n)) as usize;
    (s0, e0)
}
fn byte_at(st: &S, ci: usize) -> usize {
    if st.ascii {
        ci.min(st.s.len())
    } else {
        st.s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(st.s.len())
    }
}
fn char_at_byte(st: &S, b: usize) -> usize {
    if st.ascii {
        b
    } else {
        st.s[..b].chars().count()
    }
}
fn py_split_ws(t: &str, maxsplit: i64) -> Vec<V> {
    let mut out = vec![];
    let mut rest = t.trim_start_matches(|c: char| c.is_whitespace());
    let mut n = 0;
    while !rest.is_empty() {
        if maxsplit >= 0 && n >= maxsplit {
            out.push(s(rest));
            return out;
        }
        match rest.find(|c: char| c.is_whitespace()) {
            Some(i) => {
                out.push(s(&rest[..i]));
                rest = rest[i..].trim_start_matches(|c: char| c.is_whitespace());
                n += 1;
            }
            None => {
                out.push(s(rest));
                rest = "";
            }
        }
    }
    out
}
fn py_rsplit_ws(t: &str, maxsplit: i64) -> Vec<V> {
    let mut out = vec![];
    let mut rest = t.trim_end_matches(|c: char| c.is_whitespace());
    let mut n = 0;
    while !rest.is_empty() {
        if maxsplit >= 0 && n >= maxsplit {
            out.push(s(rest));
            break;
        }
        match rest.rfind(|c: char| c.is_whitespace()) {
            Some(i) => {
                let c = rest[i..].chars().next().unwrap();
                out.push(s(&rest[i + c.len_utf8()..]));
                rest = rest[..i].trim_end_matches(|c: char| c.is_whitespace());
                n += 1;
            }
            None => {
                out.push(s(rest));
                rest = "";
            }
        }
    }
    out.reverse();
    out
}
fn is_linebreak(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
}
fn py_title(t: &str) -> String {
    let mut out = String::new();
    let mut prev_cased = false;
    for c in t.chars() {
        if c.is_alphabetic() {
            if prev_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            prev_cased = true;
        } else {
            out.push(c);
            prev_cased = false;
        }
    }
    out
}
fn strip_set(a: &Args, f: &str) -> Result<Option<Vec<char>>, E> {
    match a.p.first() {
        None | Some(V::None) => Ok(None),
        Some(x) => Ok(Some(arg_str(x, f)?.s.chars().collect())),
    }
}
pub fn str_method(me: &Rc<S>, o: &V, name: &str, mut a: Args) -> R {
    let t = me.s.as_str();
    // str subclasses (StrEnum members) as arguments: their str
    for x in a.p.iter_mut().chain(a.k.iter_mut().map(|(_, v)| v)) {
        if let V::Obj(_) = x {
            if let Some(i @ V::Str(_)) = inner_of(x) {
                *x = i;
            }
        }
    }
    Ok(match name {
        "split" | "rsplit" => {
            let sep = a.kw("sep").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
            let maxsplit = match a.kw("maxsplit").or_else(|| a.p.get(1).cloned()) {
                Some(m) => idx_of(&m)?,
                None => -1,
            };
            match sep {
                V::None => list(if name == "split" { py_split_ws(t, maxsplit) } else { py_rsplit_ws(t, maxsplit) }),
                V::Str(sep) => {
                    if sep.s.is_empty() {
                        return val_err("empty separator".into());
                    }
                    let parts: Vec<V> = if maxsplit < 0 {
                        t.split(sep.s.as_str()).map(s).collect()
                    } else if name == "split" {
                        t.splitn(maxsplit as usize + 1, sep.s.as_str()).map(s).collect()
                    } else {
                        let mut v: Vec<V> = t.rsplitn(maxsplit as usize + 1, sep.s.as_str()).map(s).collect();
                        v.reverse();
                        v
                    };
                    list(parts)
                }
                x => return type_err(format!("must be str or None, not {}", tn(&x))),
            }
        }
        "strip" | "lstrip" | "rstrip" => {
            let cs = strip_set(&a, name)?;
            let f = |c: char| match &cs {
                Some(v) => v.contains(&c),
                None => c.is_whitespace(),
            };
            let r = match name {
                "strip" => t.trim_matches(f),
                "lstrip" => t.trim_start_matches(f),
                _ => t.trim_end_matches(f),
            };
            if r.len() == t.len() {
                return Ok(o.clone());
            }
            s(r)
        }
        "join" => {
            let items = to_vec(&a1(&a, "join")?)?;
            let mut out = String::new();
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(t);
                }
                match x {
                    V::Str(y) => out.push_str(&y.s),
                    // a library str (numpy's np.str_)
                    V::Ext(e) if matches!(e.native(), Some(Ok(V::Str(_)))) => {
                        if let Some(Ok(V::Str(y))) = e.native() {
                            out.push_str(&y.s);
                        }
                    }
                    // a str subclass (StrEnum)
                    V::Obj(_) if matches!(inner_of(x), Some(V::Str(_))) => {
                        if let Some(V::Str(y)) = inner_of(x) {
                            out.push_str(&y.s);
                        }
                    }
                    y => return type_err(format!("sequence item {i}: expected str instance, {} found", tn(y))),
                }
            }
            st(out)
        }
        "replace" => {
            a.want("replace", 2, 3)?;
            let (old, new) = (arg_str(&a.p[0], "replace")?, arg_str(&a.p[1], "replace")?);
            let count = opt_idx(&a, 2)?.unwrap_or(-1);
            if count < 0 {
                if old.s.is_empty() {
                    let mut out = new.s.clone();
                    for c in t.chars() {
                        out.push(c);
                        out.push_str(&new.s);
                    }
                    st(out)
                } else {
                    st(t.replace(old.s.as_str(), &new.s))
                }
            } else {
                st(t.replacen(old.s.as_str(), &new.s, count as usize))
            }
        }
        "find" | "rfind" | "index" | "rindex" | "count" => {
            let sub = arg_str(a.p.first().ok_or_else(|| exc("TypeError", format!("{name}() takes at least 1 argument (0 given)")))?, name)?;
            let (cs, ce) = char_range(me, opt_idx(&a, 1)?, opt_idx(&a, 2)?);
            if cs > ce {
                return Ok(V::Int(if name == "count" { 0 } else if name.ends_with("index") { return val_err("substring not found".into()) } else { -1 }));
            }
            let (bs, be) = (byte_at(me, cs), byte_at(me, ce));
            let hay = &t[bs..be];
            if name == "count" {
                if sub.s.is_empty() {
                    return Ok(V::Int((ce - cs + 1) as i64));
                }
                return Ok(V::Int(hay.matches(sub.s.as_str()).count() as i64));
            }
            let r = if name.starts_with('r') { hay.rfind(sub.s.as_str()) } else { hay.find(sub.s.as_str()) };
            match r {
                Some(b) => V::Int(char_at_byte(me, bs + b) as i64),
                None => {
                    if name.ends_with("index") {
                        return val_err("substring not found".into());
                    }
                    V::Int(-1)
                }
            }
        }
        "startswith" | "endswith" => {
            let (cs, ce) = char_range(me, opt_idx(&a, 1)?, opt_idx(&a, 2)?);
            let hay = if cs <= ce { &t[byte_at(me, cs)..byte_at(me, ce)] } else { "" };
            let pats: Vec<V> = match a.p.first() {
                Some(V::Tuple(x)) => x.to_vec(),
                Some(x) => vec![x.clone()],
                None => return type_err(format!("{name}() takes at least 1 argument (0 given)")),
            };
            let mut r = false;
            for p in pats {
                let p = match &p {
                    V::Str(p) => p.clone(),
                    x => return type_err(format!("{name} first arg must be str or a tuple of str, not {}", tn(x))),
                };
                if cs > ce {
                    continue;
                }
                if if name == "startswith" { hay.starts_with(p.s.as_str()) } else { hay.ends_with(p.s.as_str()) } {
                    r = true;
                    break;
                }
            }
            V::Bool(r)
        }
        "upper" => st(t.to_uppercase()),
        "lower" => st(t.to_lowercase()),
        "casefold" => st(t.to_lowercase()),
        "title" => st(py_title(t)),
        "capitalize" => {
            let mut cs = t.chars();
            match cs.next() {
                Some(c) => st(c.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase()),
                None => s(""),
            }
        }
        "swapcase" => st(t.chars().map(|c| if c.is_uppercase() { c.to_lowercase().collect::<String>() } else { c.to_uppercase().collect() }).collect()),
        "isdigit" | "isdecimal" | "isnumeric" => V::Bool(!t.is_empty() && t.chars().all(|c| if name == "isnumeric" { c.is_numeric() } else { c.is_ascii_digit() || (c.is_numeric() && c.to_digit(10).is_some()) || (name == "isdigit" && "²³¹⁰⁴⁵⁶⁷⁸⁹".contains(c)) })),
        "isalpha" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_alphabetic())),
        "isalnum" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_alphanumeric())),
        "isspace" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_whitespace())),
        "isupper" => V::Bool(t.chars().any(|c| c.is_uppercase()) && !t.chars().any(|c| c.is_lowercase())),
        "islower" => V::Bool(t.chars().any(|c| c.is_lowercase()) && !t.chars().any(|c| c.is_uppercase())),
        "istitle" => V::Bool(t.chars().any(|c| c.is_alphabetic()) && py_title(t) == t),
        "isascii" => V::Bool(t.is_ascii()),
        "isprintable" => V::Bool(t.chars().all(|c| !c.is_control())),
        "isidentifier" => V::Bool({
            let mut cs = t.chars();
            match cs.next() {
                Some(c) if c == '_' || c.is_alphabetic() => cs.all(|c| c == '_' || c.is_alphanumeric()),
                _ => false,
            }
        }),
        "ljust" | "rjust" | "center" => {
            a.want(name, 1, 2)?;
            let w = idx_of(&a.p[0])?.max(0) as usize;
            let fill = match a.p.get(1) {
                Some(V::Str(f)) if f.nchars() == 1 => f.s.chars().next().unwrap(),
                Some(_) => return type_err("The fill character must be exactly one character long".into()),
                None => ' ',
            };
            let n = me.nchars();
            if n >= w {
                return Ok(o.clone());
            }
            let k = w - n;
            let f = |k: usize| std::iter::repeat(fill).take(k).collect::<String>();
            st(match name {
                "ljust" => format!("{t}{}", f(k)),
                "rjust" => format!("{}{t}", f(k)),
                _ => {
                    let left = k / 2 + (k & w & 1);
                    format!("{}{t}{}", f(left), f(k - left))
                }
            })
        }
        "zfill" => {
            let w = idx_of(&a1(&a, "zfill")?)?.max(0) as usize;
            let n = me.nchars();
            if n >= w {
                return Ok(o.clone());
            }
            let (sign, body) = if t.starts_with('-') || t.starts_with('+') { (&t[..1], &t[1..]) } else { ("", t) };
            st(format!("{sign}{}{body}", "0".repeat(w - n)))
        }
        "partition" | "rpartition" => {
            let sep = arg_str(&a1(&a, name)?, name)?;
            if sep.s.is_empty() {
                return val_err("empty separator".into());
            }
            let r = if name == "partition" { t.find(sep.s.as_str()) } else { t.rfind(sep.s.as_str()) };
            match r {
                Some(i) => tup(vec![s(&t[..i]), V::Str(sep.clone()), s(&t[i + sep.s.len()..])]),
                None => {
                    if name == "partition" {
                        tup(vec![o.clone(), s(""), s("")])
                    } else {
                        tup(vec![s(""), s(""), o.clone()])
                    }
                }
            }
        }
        "splitlines" => {
            let keep = a.kw("keepends").or_else(|| a.p.first().cloned()).map(|k| truth(&k)).transpose()?.unwrap_or(false);
            let mut out = vec![];
            let cs: Vec<char> = t.chars().collect();
            let mut cur = String::new();
            let mut i = 0;
            while i < cs.len() {
                let c = cs[i];
                if is_linebreak(c) {
                    let mut brk = c.to_string();
                    if c == '\r' && i + 1 < cs.len() && cs[i + 1] == '\n' {
                        brk.push('\n');
                        i += 1;
                    }
                    if keep {
                        cur.push_str(&brk);
                    }
                    out.push(st(std::mem::take(&mut cur)));
                } else {
                    cur.push(c);
                }
                i += 1;
            }
            if !cur.is_empty() {
                out.push(st(cur));
            }
            list(out)
        }
        "format" => st(str_format(t, &a)?),
        "format_map" => {
            let m = a1(&a, "format_map")?;
            let items = dict_items_of(&m)?;
            let k: Vec<(Sym, V)> = items.into_iter().map(|(k, v)| (sym(&to_str(&k).unwrap_or_default()), v)).collect();
            st(str_format(t, &Args { p: vec![], k })?)
        }
        "removeprefix" => {
            let p = arg_str(&a1(&a, name)?, name)?;
            match t.strip_prefix(p.s.as_str()) {
                Some(r) => s(r),
                None => o.clone(),
            }
        }
        "removesuffix" => {
            let p = arg_str(&a1(&a, name)?, name)?;
            match t.strip_suffix(p.s.as_str()) {
                Some(r) if !p.s.is_empty() => s(r),
                _ => o.clone(),
            }
        }
        "expandtabs" => {
            let ts = opt_idx(&a, 0)?.unwrap_or(8).max(0) as usize;
            let mut out = String::new();
            let mut col = 0;
            for c in t.chars() {
                if c == '\t' {
                    if ts > 0 {
                        let k = ts - col % ts;
                        out.push_str(&" ".repeat(k));
                        col += k;
                    }
                } else if c == '\n' || c == '\r' {
                    out.push(c);
                    col = 0;
                } else {
                    out.push(c);
                    col += 1;
                }
            }
            st(out)
        }
        "translate" => {
            let table = a1(&a, "translate")?;
            let mut out = String::new();
            for c in t.chars() {
                let r = match &table {
                    V::Dict(d) => d.borrow().get(&V::Int(c as i64))?,
                    _ => match getitem(&table, &V::Int(c as i64)) {
                        Ok(v) => Some(v),
                        Err(E(e)) if is_exc(&e, "LookupError") => None,
                        Err(e) => return Err(e),
                    },
                };
                match r {
                    None => out.push(c),
                    Some(V::None) => {}
                    Some(V::Int(i)) => out.push(char::from_u32(i as u32).unwrap_or('?')),
                    Some(V::Str(x)) => out.push_str(&x.s),
                    Some(x) => return type_err(format!("character mapping must return integer, None or str")),
                }
            }
            st(out)
        }
        "maketrans" => {
            let mut d = Dict::new();
            match a.p.len() {
                1 => {
                    for (k, v) in dict_items_of(&a.p[0])? {
                        let k = match &k {
                            V::Str(x) => V::Int(x.s.chars().next().unwrap_or('\0') as i64),
                            x => x.clone(),
                        };
                        d.set(k, v)?;
                    }
                }
                _ => {
                    let (x, y) = (arg_str(&a.p[0], name)?, arg_str(&a.p[1], name)?);
                    if x.nchars() != y.nchars() {
                        return val_err("the first two maketrans arguments must have equal length".into());
                    }
                    for (p, q) in x.s.chars().zip(y.s.chars()) {
                        d.set(V::Int(p as i64), V::Int(q as i64))?;
                    }
                    if let Some(z) = a.p.get(2) {
                        for p in arg_str(z, name)?.s.chars() {
                            d.set(V::Int(p as i64), V::None)?;
                        }
                    }
                }
            }
            V::Dict(Rc::new(RefCell::new(d)))
        }
        "encode" => {
            let mut a = a;
            let enc = a.kw("encoding").or_else(|| a.p.first().cloned());
            let errs = a.kw("errors").or_else(|| a.p.get(1).cloned());
            a.no_kw("encode")?;
            str_encode(&me.s, enc.as_ref(), errs.as_ref())?
        }
        "__len__" => V::Int(me.nchars() as i64),
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__lt__" => V::Bool(lt(o, &a1(&a, name)?)?),
        "__hash__" => V::Int(py_hash(o)?),
        "__str__" => o.clone(),
        "__repr__" => st(repr(o)?),
        "__add__" => add(o, &a1(&a, name)?)?,
        "__mul__" => binop(o, &a1(&a, name)?, "*")?,
        "__mod__" => binop(o, &a1(&a, name)?, "%")?,
        "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?)?,
        _ => return bad_method(o, name),
    })
}
// "{} {name} {0:>5}".format(...)
pub fn str_format(t: &str, a: &Args) -> Result<String, E> {
    let c: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut auto = 0usize;
    while i < c.len() {
        if c[i] == '{' {
            if i + 1 < c.len() && c[i + 1] == '{' {
                out.push('{');
                i += 2;
                continue;
            }
            // find matching }
            let mut depth = 1;
            let mut j = i + 1;
            while j < c.len() {
                if c[j] == '{' {
                    depth += 1;
                } else if c[j] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if j >= c.len() {
                return val_err("Single '{' encountered in format string".into());
            }
            let field: String = c[i + 1..j].iter().collect();
            i = j + 1;
            let (head, spec) = match field.find(':') {
                Some(p) => (field[..p].to_string(), field[p + 1..].to_string()),
                None => (field.clone(), String::new()),
            };
            let (head, conv) = match head.find('!') {
                Some(p) => (head[..p].to_string(), Some(head[p + 1..].to_string())),
                None => (head, None),
            };
            // nested fields in the spec
            let spec = if spec.contains('{') { str_format(&spec, &Args { p: a.p.clone(), k: a.k.clone() })? } else { spec };
            // first part: index or name, then .attr / [key]
            let name_end = head.find(|ch: char| ch == '.' || ch == '[').unwrap_or(head.len());
            let first = &head[..name_end];
            let mut v = if first.is_empty() {
                let v = a.p.get(auto).cloned().ok_or_else(|| exc("IndexError", format!("Replacement index {auto} out of range for positional args tuple")))?;
                auto += 1;
                v
            } else if let Ok(k) = first.parse::<usize>() {
                a.p.get(k).cloned().ok_or_else(|| exc("IndexError", format!("Replacement index {k} out of range for positional args tuple")))?
            } else {
                a.k.iter().find(|(n, _)| *n == first).map(|(_, v)| v.clone()).ok_or_else(|| exc_v("KeyError", s(first)))?
            };
            let mut rest = &head[name_end..];
            while !rest.is_empty() {
                if let Some(r) = rest.strip_prefix('.') {
                    let e = r.find(|ch: char| ch == '.' || ch == '[').unwrap_or(r.len());
                    v = getattr(&v, &r[..e])?;
                    rest = &r[e..];
                } else if let Some(r) = rest.strip_prefix('[') {
                    let e = r.find(']').unwrap_or(r.len());
                    let key = &r[..e];
                    let k = match key.parse::<i64>() {
                        Ok(n) => V::Int(n),
                        Err(_) => s(key),
                    };
                    v = getitem(&v, &k)?;
                    rest = &r[(e + 1).min(r.len())..];
                } else {
                    break;
                }
            }
            let v = match conv.as_deref() {
                Some("r") => st(repr(&v)?),
                Some("s") => st(to_str(&v)?),
                Some("a") => bi_ascii(Args::p(vec![v]))?,
                _ => v,
            };
            out.push_str(&format_spec(&v, &spec)?);
        } else if c[i] == '}' {
            if i + 1 < c.len() && c[i + 1] == '}' {
                out.push('}');
                i += 2;
                continue;
            }
            return val_err("Single '}' encountered in format string".into());
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    Ok(out)
}
// `xs.clear(None)`: CPython's TypeError for a method that takes no arguments.
fn no_args(ty: &str, name: &str, a: &Args) -> Result<(), E> {
    if !a.k.is_empty() {
        return type_err(format!("{ty}.{name}() takes no keyword arguments"));
    }
    if !a.p.is_empty() {
        return type_err(format!("{ty}.{name}() takes no arguments ({} given)", a.p.len()));
    }
    Ok(())
}

pub fn list_method(l: &Rc<RefCell<Vec<V>>>, o: &V, name: &str, mut a: Args) -> R {
    if matches!(name, "reverse" | "copy" | "clear" | "__len__") {
        no_args("list", name, &a)?;
    }
    Ok(match name {
        "append" => {
            let x = a1(&a, "append")?;
            l.borrow_mut().push(x);
            V::None
        }
        // `a.__init__([4, 5, 6])` called on an existing list replaces its items
        "__init__" => {
            a.want("__init__", 0, 1)?;
            let v = match a.p.first() { Some(x) => to_vec(x)?, None => vec![] };
            *l.borrow_mut() = v;
            V::None
        }
        "extend" => {
            let x = a1(&a, "extend")?;
            let v = to_vec(&x)?;
            l.borrow_mut().extend(v);
            V::None
        }
        "insert" => {
            a.want("insert", 2, 2)?;
            let i = idx_of(&a.p[0])?;
            let mut l = l.borrow_mut();
            let n = l.len() as i64;
            let j = if i < 0 { (i + n).max(0) } else { i.min(n) } as usize;
            l.insert(j, a.p[1].clone());
            V::None
        }
        "pop" => {
            if a.p.len() > 1 || !a.k.is_empty() {
                return type_err(format!("pop expected at most 1 argument, got {}", a.p.len() + a.k.len()));
            }
            let mut l = l.borrow_mut();
            if l.is_empty() {
                return Err(exc("IndexError", "pop from empty list".into()));
            }
            match a.p.first() {
                None => l.pop().unwrap(),
                Some(i) => {
                    let i = idx_of(i)?;
                    let n = l.len() as i64;
                    let j = if i < 0 { i + n } else { i };
                    if j < 0 || j >= n {
                        return Err(exc("IndexError", "pop index out of range".into()));
                    }
                    l.remove(j as usize)
                }
            }
        }
        "remove" => {
            let x = a1(&a, "remove")?;
            let v = l.borrow().clone();
            for (i, y) in v.iter().enumerate() {
                if is(y, &x) || eq(y, &x)? {
                    l.borrow_mut().remove(i);
                    return Ok(V::None);
                }
            }
            return val_err("list.remove(x): x not in list".into());
        }
        "index" => {
            let x = a.p.first().cloned().ok_or_else(|| exc("TypeError", "index expected at least 1 argument, got 0".into()))?;
            let v = l.borrow().clone();
            let n = v.len() as i64;
            let fix = |i: i64| if i < 0 { (i + n).max(0) } else { i.min(n) };
            let st0 = fix(opt_idx(&a, 1)?.unwrap_or(0)) as usize;
            let e0 = fix(opt_idx(&a, 2)?.unwrap_or(n)) as usize;
            for i in st0..e0.max(st0) {
                if is(&v[i], &x) || eq(&v[i], &x)? {
                    return Ok(V::Int(i as i64));
                }
            }
            return val_err(format!("{} is not in list", repr(&x)?));
        }
        "count" => {
            let x = a1(&a, "count")?;
            let v = l.borrow().clone();
            let mut k = 0;
            for y in v.iter() {
                if is(y, &x) || eq(y, &x)? {
                    k += 1;
                }
            }
            V::Int(k)
        }
        "sort" => {
            let key = a.kw("key");
            let rev = a.kw("reverse").map(|r| truth(&r)).transpose()?.unwrap_or(false);
            a.no_kw("sort")?;
            if !a.p.is_empty() {
                return type_err("sort() takes no positional arguments".into());
            }
            let mut v = std::mem::take(&mut *l.borrow_mut());
            let r = sort_values(&mut v, key.as_ref(), rev);
            let changed = !l.borrow().is_empty();
            *l.borrow_mut() = v;
            r?;
            if changed {
                return val_err("list modified during sort".into());
            }
            V::None
        }
        "reverse" => {
            l.borrow_mut().reverse();
            V::None
        }
        "copy" => list(l.borrow().clone()),
        "clear" => {
            // The items go last first, as CPython's list_clear frees them, and after the list is released.
            let old = std::mem::take(&mut *l.borrow_mut());
            for x in old.into_iter().rev() {
                drop(x);
            }
            V::None
        }
        "__len__" => V::Int(l.borrow().len() as i64),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__setitem__" => {
            a.want(name, 2, 2)?;
            setitem(o, &a.p[0], a.p[1].clone())?;
            V::None
        }
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__iter__" => iter_v(o)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__add__" => add(o, &a1(&a, name)?)?,
        "__iadd__" => iop(o, &a1(&a, name)?, "+")?,
        "__delitem__" => {
            delitem(o, &a1(&a, name)?)?;
            V::None
        }
        "__repr__" | "__str__" => st(repr(o)?),
        _ => return bad_method(o, name),
    })
}
pub fn dict_method(d: &Rc<RefCell<Dict>>, o: &V, name: &str, mut a: Args) -> R {
    if matches!(name, "keys" | "values" | "items" | "copy" | "clear" | "__len__") {
        no_args("dict", name, &a)?;
    }
    Ok(match name {
        "get" => {
            a.want("get", 1, 2)?;
            let r = Dict::get_cell(d, &a.p[0])?;
            r.unwrap_or_else(|| a.p.get(1).cloned().unwrap_or(V::None))
        }
        "setdefault" => {
            a.want("setdefault", 1, 2)?;
            let v = a.p.get(1).cloned().unwrap_or(V::None);
            let k = a.p[0].clone();
            Dict::set_default_cell(d, k, v)?
        }
        "pop" => {
            a.want("pop", 1, 2)?;
            let r = Dict::del_cell(d, &a.p[0])?;
            match r {
                Some(v) => v,
                None => match a.p.get(1) {
                    Some(x) => x.clone(),
                    None => return Err(exc_v("KeyError", a.p[0].clone())),
                },
            }
        }
        "popitem" => {
            if d.borrow().kind != 3 {
                no_args("dict", name, &a)?;
            }
            let last = a.kw("last").map(|x| truth(&x)).transpose()?.unwrap_or(true);
            let k = {
                let d = d.borrow();
                let ks = d.ents.iter().flatten().map(|(k, _)| k.clone());
                if last {
                    ks.last()
                } else {
                    ks.into_iter().next()
                }
            };
            match k {
                Some(k) => {
                    let v = Dict::del_cell(d, &k)?.unwrap();
                    tup(vec![k, v])
                }
                None => return Err(exc("KeyError", "popitem(): dictionary is empty".into())),
            }
        }
        "keys" => V::View(Rc::new((o.clone(), 0))),
        "values" => V::View(Rc::new((o.clone(), 1))),
        "items" => V::View(Rc::new((o.clone(), 2))),
        "update" => {
            if d.borrow().kind == 2 {
                return counter_update(o, a, true);
            }
            if let Some(x) = a.p.first() {
                let other = if let V::Dict(y) = x { Some((y.clone(), y.borrow().n)) } else { None };
                for (k, v) in dict_items_of(x)? {
                    Dict::set_cell(d, k, v)?;
                    if let Some((y, n0)) = &other {
                        if y.try_borrow().map_or(false, |y| y.n != *n0) {
                            return Err(exc("RuntimeError", "dict mutated during update".into()));
                        }
                    }
                }
            }
            for (k, v) in a.k.drain(..) {
                Dict::set_cell(d, s(k), v)?;
            }
            V::None
        }
        "subtract" => return counter_update(o, a, false),
        "__or__" | "__ror__" => {
            let x = a1(&a, name)?;
            let V::Dict(y) = &x else { return Ok(V::NotImpl) };
            let (first, second) = if name == "__or__" { (d, y) } else { (y, d) };
            let mut r = first.borrow().copy();
            let items = second.borrow().items();
            for (k, v) in items {
                r.set(k, v)?;
            }
            V::Dict(Rc::new(RefCell::new(r)))
        }
        "__ior__" => {
            let x = a1(&a, name)?;
            for (k, v) in dict_items_of(&x)? {
                Dict::set_cell(d, k, v)?;
            }
            o.clone()
        }
        "copy" => V::Dict(Rc::new(RefCell::new(d.borrow().copy()))),
        "clear" => {
            d.borrow_mut().clear();
            V::None
        }
        "most_common" => {
            let mut items = d.borrow().items();
            sort_by_key_desc_count(&mut items)?;
            let n = match a.p.first() {
                Some(V::None) | None => items.len(),
                Some(x) => idx_of(x)?.max(0) as usize,
            };
            list(items.into_iter().take(n).map(|(k, v)| tup(vec![k, v])).collect())
        }
        "elements" => {
            let mut out = vec![];
            for (k, v) in d.borrow().items() {
                let n = idx_of(&v).unwrap_or(0);
                for _ in 0..n.max(0) {
                    out.push(k.clone());
                }
            }
            vec_iter(out)
        }
        "total" => {
            let mut t = V::Int(0);
            for v in d.borrow().values() {
                t = add(&t, &v)?;
            }
            t
        }
        "move_to_end" => {
            let last = a.kw("last").or_else(|| a.p.get(1).cloned()).map(|x| truth(&x)).transpose()?.unwrap_or(true);
            let k = a.p.first().cloned().ok_or_else(|| exc("TypeError", "move_to_end() missing key".into()))?;
            if !Dict::move_to_end_cell(d, &k, last)? {
                return Err(exc_v("KeyError", k));
            }
            V::None
        }
        "__len__" => V::Int(d.borrow().n as i64),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__setitem__" => {
            a.want(name, 2, 2)?;
            setitem(o, &a.p[0], a.p[1].clone())?;
            V::None
        }
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__delitem__" => {
            delitem(o, &a1(&a, name)?)?;
            V::None
        }
        "__iter__" => iter_v(o)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__repr__" | "__str__" => st(repr(o)?),
        _ => return bad_method(o, name),
    })
}
pub fn set_method(o: &V, name: &str, a: Args) -> R {
    if matches!(name, "copy" | "clear" | "pop") {
        no_args(if matches!(o, V::FSet(_)) { "frozenset" } else { "set" }, name, &a)?;
    }
    // A copy only for the methods that read the whole set (set_of copies; add must not).
    let me = || set_of(o).unwrap();
    let others = |a: &Args| -> Result<Vec<PSet>, E> {
        let mut v = vec![];
        for x in &a.p {
            v.push(match x {
                V::Set(_) | V::FSet(_) => set_of(x).unwrap(),
                _ => PSet::from_iter(to_vec(x)?)?,
            });
        }
        Ok(v)
    };
    let wrap = |p: PSet| -> V {
        if matches!(o, V::FSet(_)) {
            V::FSet(Rc::new(p))
        } else {
            V::Set(Rc::new(RefCell::new(p)))
        }
    };
    let mutable = |o: &V| -> Result<Rc<RefCell<PSet>>, E> {
        match o {
            V::Set(x) => Ok(x.clone()),
            _ => Err(exc("AttributeError", format!("'frozenset' object has no attribute '{name}'"))),
        }
    };
    Ok(match name {
        "add" => {
            let x = a1(&a, "add")?;
            mutable(o)?.borrow_mut().add(x)?;
            V::None
        }
        "remove" => {
            let x = a1(&a, "remove")?;
            if !mutable(o)?.borrow_mut().discard(&x)? {
                return Err(exc_v("KeyError", x));
            }
            V::None
        }
        "discard" => {
            let x = a1(&a, "discard")?;
            mutable(o)?.borrow_mut().discard(&x)?;
            V::None
        }
        "pop" => match mutable(o)?.borrow_mut().pop() {
            Some(v) => v,
            None => return Err(exc("KeyError", "pop from an empty set".into())),
        },
        "clear" => {
            mutable(o)?.borrow_mut().clear();
            V::None
        }
        "copy" => wrap(me().copy()),
        "union" | "intersection" | "difference" | "symmetric_difference" => {
            let op = match name {
                "union" => "|",
                "intersection" => "&",
                "difference" => "-",
                _ => "^",
            };
            let mut r = me().copy();
            for x in others(&a)? {
                r = set_op(&r, &x, op)?;
            }
            wrap(r)
        }
        "update" | "intersection_update" | "difference_update" | "symmetric_difference_update" => {
            let m = mutable(o)?;
            let op = match name {
                "update" => "|",
                "intersection_update" => "&",
                "difference_update" => "-",
                _ => "^",
            };
            for x in others(&a)? {
                match op {
                    "|" => {
                        for (k, h) in x.keys_h() {
                            m.borrow_mut().add_h(k, h);
                        }
                    }
                    "-" => {
                        for k in x.keys() {
                            m.borrow_mut().discard(&k)?;
                        }
                    }
                    _ => {
                        let r = set_op(&m.borrow(), &x, op)?;
                        *m.borrow_mut() = r;
                    }
                }
            }
            V::None
        }
        "issubset" | "issuperset" | "isdisjoint" => {
            let x = others(&a)?.pop().unwrap_or_else(PSet::new);
            let me = me();
            V::Bool(match name {
                "issubset" => me.keys().iter().all(|k| x.contains(k).unwrap_or(false)),
                "issuperset" => x.keys().iter().all(|k| me.contains(k).unwrap_or(false)),
                _ => !me.keys().iter().any(|k| x.contains(k).unwrap_or(false)),
            })
        }
        "__len__" => V::Int(match o {
            V::Set(x) => x.borrow().used,
            V::FSet(x) => x.used,
            _ => 0,
        } as i64),
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__iter__" => iter_v(o)?,
        _ => return bad_method(o, name),
    })
}
pub fn deque_method(d: &Rc<RefCell<Deq>>, o: &V, name: &str, a: Args) -> R {
    let push = |d: &mut Deq, x: V, left: bool| {
        if left {
            d.v.push_front(x);
            if let Some(m) = d.max {
                if d.v.len() > m {
                    d.v.pop_back();
                }
            }
        } else {
            d.v.push_back(x);
            if let Some(m) = d.max {
                if d.v.len() > m {
                    d.v.pop_front();
                }
            }
        }
    };
    Ok(match name {
        "append" | "appendleft" => {
            let x = a1(&a, name)?;
            push(&mut d.borrow_mut(), x, name == "appendleft");
            V::None
        }
        "extend" | "extendleft" => {
            let xs = to_vec(&a1(&a, name)?)?;
            let mut d = d.borrow_mut();
            for x in xs {
                push(&mut d, x, name == "extendleft");
            }
            V::None
        }
        "pop" => match d.borrow_mut().v.pop_back() {
            Some(v) => v,
            None => return Err(exc("IndexError", "pop from an empty deque".into())),
        },
        "popleft" => match d.borrow_mut().v.pop_front() {
            Some(v) => v,
            None => return Err(exc("IndexError", "pop from an empty deque".into())),
        },
        "rotate" => {
            let n = opt_idx(&a, 0)?.unwrap_or(1);
            let mut d = d.borrow_mut();
            let len = d.v.len() as i64;
            if len > 0 {
                let k = n.rem_euclid(len) as usize;
                d.v.rotate_right(k);
            }
            V::None
        }
        "clear" => {
            d.borrow_mut().v.clear();
            V::None
        }
        "copy" => V::Deque(Rc::new(RefCell::new(Deq { v: d.borrow().v.clone(), max: d.borrow().max }))),
        "reverse" => {
            let mut d = d.borrow_mut();
            let v: VecDeque<V> = d.v.drain(..).rev().collect();
            d.v = v;
            V::None
        }
        "count" | "index" | "remove" => {
            let x = a1(&a, name)?;
            let v: Vec<V> = d.borrow().v.iter().cloned().collect();
            let mut k = 0;
            for (i, y) in v.iter().enumerate() {
                if eq(y, &x)? {
                    match name {
                        "index" => return Ok(V::Int(i as i64)),
                        "remove" => {
                            d.borrow_mut().v.remove(i);
                            return Ok(V::None);
                        }
                        _ => k += 1,
                    }
                }
            }
            match name {
                "count" => V::Int(k),
                "index" => return val_err(format!("{} is not in deque", repr(&x)?)),
                _ => return val_err("deque.remove(x): x not in deque".into()),
            }
        }
        "insert" => {
            a.want("insert", 2, 2)?;
            let i = idx_of(&a.p[0])?;
            let mut d = d.borrow_mut();
            let n = d.v.len() as i64;
            let j = if i < 0 { (i + n).max(0) } else { i.min(n) } as usize;
            d.v.insert(j, a.p[1].clone());
            V::None
        }
        "__len__" => V::Int(d.borrow().v.len() as i64),
        _ => return bad_method(o, name),
    })
}
pub fn builtin_method(o: &V, name: &str, a: Args) -> R {
    match o {
        V::Str(x) => str_method(x, o, name, a),
        V::List(l) => list_method(l, o, name, a),
        V::Dict(d) => dict_method(d, o, name, a),
        V::Set(_) | V::FSet(_) => set_method(o, name, a),
        V::Deque(d) => deque_method(d, o, name, a),
        V::Tuple(t) => match name {
            "count" => {
                let x = a1(&a, name)?;
                let mut k = 0;
                for y in t.iter() {
                    if is(y, &x) || eq(y, &x)? {
                        k += 1;
                    }
                }
                Ok(V::Int(k))
            }
            "index" => {
                let x = a.p.first().cloned().ok_or_else(|| exc("TypeError", "index expected at least 1 argument, got 0".into()))?;
                for (i, y) in t.iter().enumerate() {
                    if is(y, &x) || eq(y, &x)? {
                        return Ok(V::Int(i as i64));
                    }
                }
                val_err("tuple.index(x): x not in tuple".into())
            }
            "__len__" => Ok(V::Int(t.len() as i64)),
            "__getitem__" => getitem(o, &a1(&a, name)?),
            "__contains__" => Ok(V::Bool(contains(o, &a1(&a, name)?)?)),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            _ => bad_method(o, name),
        },
        V::Int(_) | V::Big(_) | V::Bool(_) => match name {
            "bit_length" => Ok(V::Int(big_of(o).unwrap().bits() as i64)),
            "bit_count" => Ok(V::Int(big_of(o).unwrap().mag.iter().map(|w| w.count_ones() as i64).sum())),
            "conjugate" | "__int__" | "__index__" | "__trunc__" | "__floor__" | "__ceil__" => to_int(o),
            "is_integer" => Ok(V::Bool(true)),
            "as_integer_ratio" => Ok(tup(vec![to_int(o)?, V::Int(1)])),
            "__add__" | "__radd__" => add(o, &a1(&a, name)?),
            "__sub__" => sub(o, &a1(&a, name)?),
            "__mul__" | "__rmul__" => binop(o, &a1(&a, name)?, "*"),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__str__" | "__repr__" => str_v(o),
            "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?),
            "__neg__" => neg(o),
            "__abs__" => bi_abs(Args::p(vec![o.clone()])),
            "__float__" => Ok(V::Float(to_float(o)?)),
            "__bool__" => Ok(V::Bool(truth(o)?)),
            _ => bad_method(o, name),
        },
        V::Float(f) => match name {
            "is_integer" => Ok(V::Bool(f.is_finite() && f.fract() == 0.0)),
            "conjugate" | "__float__" => Ok(o.clone()),
            "__int__" | "__trunc__" => to_int(o),
            "__floor__" => to_int(&V::Float(f.floor())),
            "__ceil__" => to_int(&V::Float(f.ceil())),
            "__round__" => bi_round(Args { p: [vec![o.clone()], a.p].concat(), k: vec![] }),
            "as_integer_ratio" => {
                if !f.is_finite() {
                    return val_err("cannot convert Infinity to integer ratio".into());
                }
                let (m, e) = frexp(*f);
                let mut num = m;
                let mut e = e;
                for _ in 0..300 {
                    if num == num.floor() {
                        break;
                    }
                    num *= 2.0;
                    e -= 1;
                }
                let mut n = Big::from_f64(num);
                let mut d = Big::from_i64(1);
                if e > 0 {
                    n = n.shl(e as usize);
                } else {
                    d = d.shl((-e) as usize);
                }
                // reduce
                let g = big_gcd(&n, &d);
                Ok(tup(vec![n.divmod(&g).0.norm(), d.divmod(&g).0.norm()]))
            }
            "hex" => Err(unsupported("float.hex()")),
            "__add__" | "__radd__" => add(o, &a1(&a, name)?),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__str__" | "__repr__" => str_v(o),
            "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?),
            "__neg__" => neg(o),
            "__abs__" => Ok(V::Float(f.abs())),
            "__bool__" => Ok(V::Bool(*f != 0.0)),
            _ => bad_method(o, name),
        },
        V::Iter(it) => match name {
            "__next__" => bi_next(Args::p(vec![o.clone()])),
            "__iter__" => Ok(o.clone()),
            "send" => {
                let c = match &*it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return bad_method(o, name),
                };
                let r = c.borrow_mut().resume(a1(&a, name)?);
                match r? {
                    Some(v) => Ok(v),
                    None => Err(stop_with(V::None)),
                }
            }
            // throw(exc) or throw(type[, value[, tb]]): raised at the yield it is suspended at.
            "throw" => {
                let c = match &*it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return bad_method(o, name),
                };
                if a.p.is_empty() || a.p.len() > 3 || !a.k.is_empty() {
                    return type_err(format!("throw expected at least 1 argument, got {}", a.p.len()));
                }
                let e = match (&a.p[0], a.p.get(1)) {
                    (V::Class(c), Some(v)) if !matches!(v, V::None) => match make_exc(v.clone()) {
                        Ok(e) if isinstance(&e.0, &V::Class(c.clone()))? => e,
                        _ => E(instantiate(c, Args::p(vec![v.clone()]))?),
                    },
                    (x, _) => make_exc(x.clone())?,
                };
                let r = c.borrow_mut().throw(e);
                match r? {
                    Some(v) => Ok(v),
                    None => Err(stop_with(V::None)),
                }
            }
            "close" => {
                let c = match &*it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return Ok(V::None),
                };
                let r = c.borrow_mut().close_py();
                r
            }
            _ => bad_method(o, name),
        },
        V::View(v) => match name {
            "__len__" => Ok(V::Int(len(o)? as i64)),
            "__iter__" => iter_v(o),
            "isdisjoint" if v.1 == 0 || v.1 == 2 => {
                let other = to_vec(&a1(&a, name)?)?;
                for x in other {
                    if contains(o, &x)? {
                        return Ok(V::Bool(false));
                    }
                }
                Ok(V::Bool(true))
            }
            "mapping" if v.1 != 3 => {
                let m = V::View(Rc::new((v.0.clone(), 3)));
                call(&m, a)
            }
            "get" | "copy" if v.1 == 3 => call_method(&v.0, name, a),
            "keys" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 0)))),
            "values" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 1)))),
            "items" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 2)))),
            _ => bad_method(o, name),
        },
        V::Ext(x) => {
            let mut a = a;
            match x.method(o, name, &mut a) {
                Some(r) => r,
                None => bad_method(o, name),
            }
        }
        V::None if name == "__eq__" => Ok(V::Bool(matches!(a.p.first(), Some(V::None)))),
        V::Range(_) => match name {
            "count" => Ok(V::Int(if contains(o, &a1(&a, name)?)? { 1 } else { 0 })),
            "index" => {
                let x = a1(&a, name)?;
                if let (V::Range(r), V::Int(i)) = (o, &x) {
                    if contains(o, &x)? {
                        return Ok(V::Int((i - r.0) / r.2));
                    }
                }
                val_err(format!("{} is not in range", repr(&x)?))
            }
            _ => bad_method(o, name),
        },
        _ => bad_method(o, name),
    }
}
pub fn big_gcd(a: &Big, b: &Big) -> Big {
    let mut x = Big { neg: false, mag: a.mag.clone() };
    let mut y = Big { neg: false, mag: b.mag.clone() };
    while !y.is_zero() {
        let r = x.divmod(&y).1;
        x = y;
        y = r;
    }
    x
}
