// ---------- bytearray ----------
// bytearray for dynamic builds: a mutable byte string (an Ext value whose type is builtins.bytearray). The bytes
// live behind an Rc so that iterators and re matches see later changes, as CPython's buffer users do; `ex` counts
// the buffer's exports (re scanners and finditer iterators, memoryviews), during which a resize raises BufferError.
// Methods that do not change the bytes run bytes' own code on a copy and give bytearrays back where CPython does.

pub struct ByteArrayV {
    pub d: Rc<RefCell<Vec<u8>>>,
    pub ex: Rc<Cell<usize>>,
}

pub fn bytearrayv(b: Vec<u8>) -> V {
    V::Ext(ExtRc::new(ByteArrayV { d: Rc::new(RefCell::new(b)), ex: Rc::new(Cell::new(0)) }))
}

pub fn bytearray_of(v: &V) -> Option<&ByteArrayV> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<ByteArrayV>(),
        _ => None,
    }
}

// A held export of a bytearray's buffer: while one lives, the bytearray cannot change size.
pub struct BufHold(Rc<Cell<usize>>);
impl BufHold {
    // no export (a memoryview subject holds its own), but the subject is still read live
    pub fn none() -> BufHold {
        let c = Rc::new(Cell::new(1));
        BufHold(c)
    }
    pub fn of(v: &V) -> Option<BufHold> {
        bytearray_of(v).map(|b| {
            b.ex.set(b.ex.get() + 1);
            BufHold(b.ex.clone())
        })
    }
}
impl Drop for BufHold {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

fn ba_resize_ok(b: &ByteArrayV) -> Result<(), E> {
    if b.ex.get() > 0 {
        return Err(exc("BufferError", "Existing exports of data: object cannot be re-sized".into()));
    }
    Ok(())
}

// an int argument read as CPython's PyNumber_AsSsize_t / __index__ (bools, int subclasses and __index__ count)
fn ba_index(v: &V) -> Result<i64, E> {
    match v {
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        V::Int(_) | V::Bool(_) => idx_of(v),
        V::Obj(o) if o.cls.lookup("__index__").is_some() || inner_of(v).is_some_and(|i| matches!(i, V::Int(_) | V::Bool(_) | V::Big(_))) => idx_of(v),
        _ => type_err(format!("'{}' object cannot be interpreted as an integer", tn(v))),
    }
}

// a byte value: an int in range(0, 256)
fn ba_byte(v: &V) -> Result<u8, E> {
    let i = match v {
        V::Big(_) => return val_err("byte must be in range(0, 256)".into()),
        _ => ba_index(v)?,
    };
    if !(0..256).contains(&i) {
        return val_err("byte must be in range(0, 256)".into());
    }
    Ok(i as u8)
}

// the bytes an iterable of ints gives (bytearray(x), extend(x), slice assignment)
fn ba_ints(v: &V) -> Result<Vec<u8>, E> {
    let mut out = vec![];
    let mut it = iter(v)?;
    while let Some(x) = it.next()? {
        out.push(ba_byte(&x)?);
    }
    Ok(out)
}

// bytearray(...) and bytearray(source, encoding, errors)
pub fn bi_bytearray(a: Args) -> R {
    let mut a = a;
    let src = a.kw("source");
    let enc = a.kw("encoding");
    let errs = a.kw("errors");
    a.no_kw("bytearray")?;
    if a.p.len() > 3 {
        return type_err(format!("bytearray() takes at most 3 arguments ({} given)", a.p.len()));
    }
    let x = src.or_else(|| (!a.p.is_empty()).then(|| a.p.remove(0)));
    let enc = enc.or_else(|| (!a.p.is_empty()).then(|| a.p.remove(0)));
    let errs = errs.or_else(|| (!a.p.is_empty()).then(|| a.p.remove(0)));
    let Some(x) = x else {
        if enc.is_some() || errs.is_some() {
            return type_err("encoding or errors without sequence argument".into());
        }
        return Ok(bytearrayv(vec![]));
    };
    if let V::Str(s) = &x {
        if enc.is_none() {
            return type_err("string argument without an encoding".into());
        }
        let b = str_encode(&s.s, enc.as_ref(), errs.as_ref())?;
        return Ok(bytearrayv(bytes_of(&b).unwrap_or_default()));
    }
    if enc.is_some() {
        return type_err("encoding without a string argument".into());
    }
    if errs.is_some() {
        return type_err("errors without a string argument".into());
    }
    if let Some(b) = buf_full(&x)? {
        return Ok(bytearrayv(b));
    }
    match &x {
        V::Int(_) | V::Bool(_) | V::Big(_) => {
            let n = ba_index(&x).map_err(|_| exc("OverflowError", "cannot fit 'int' into an index-sized integer".into()))?;
            if n < 0 {
                return val_err("negative count".into());
            }
            if n > 1 << 40 {
                return Err(exc("MemoryError", String::new()));
            }
            return Ok(bytearrayv(vec![0; n as usize]));
        }
        V::Float(_) | V::None | V::Dict(_) if !matches!(x, V::Dict(_)) => {
            return type_err(format!("cannot convert '{}' object to bytearray", tn(&x)));
        }
        _ => {}
    }
    match iter(&x) {
        Ok(_) => Ok(bytearrayv(ba_ints(&x)?)),
        Err(_) => type_err(format!("cannot convert '{}' object to bytearray", tn(&x))),
    }
}

pub fn bytearray_fromhex(a: Args) -> R {
    let b = bytes_fromhex(a)?;
    Ok(bytearrayv(bytes_of(&b).unwrap_or_default()))
}

// bytes values in a result become bytearrays (split's list, partition's tuple, strip's value, ...)
fn ba_wrap(v: V) -> V {
    match &v {
        V::Ext(x) if x.any().is::<BytesV>() => bytearrayv(bytes_of(&v).unwrap_or_default()),
        V::List(l) => list(l.borrow().iter().cloned().map(ba_wrap).collect()),
        V::Tuple(t) => tup(t.iter().cloned().map(ba_wrap).collect()),
        _ => v,
    }
}

// a message bytes' code gave, as CPython words it for a bytearray
fn ba_msg(e: E) -> E {
    if let V::Obj(o) = &e.0 {
        if let Some(V::Tuple(t)) = o.get("args") {
            if let [V::Str(m)] = &t[..] {
                let fixed = m.s.replace("bytes.", "bytearray.").replace("'bytes' object", "'bytearray' object").replace("to bytes", "to bytearray");
                if fixed != m.s.as_str() {
                    o.set("args", tup(vec![st(fixed)]));
                }
            }
        }
    }
    e
}

// The methods bytes has whose result is bytes data (a bytearray here); the others (find, count, decode, hex,
// isalpha, ...) give the same result for both.
const BA_WRAPS: &[&str] = &[
    "__getitem__", "capitalize", "center", "expandtabs", "join", "ljust", "lower", "lstrip", "partition", "removeprefix",
    "removesuffix", "replace", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines", "strip", "swapcase",
    "title", "translate", "upper", "zfill",
];
// what the delegation must not answer: bytes' own conversions
const BA_NOT_BYTES: &[&str] = &["__bytes__", "__hash__", "__getnewargs__"];

impl ByteArrayV {
    fn snapshot(&self) -> V {
        bytesv(self.d.borrow().clone())
    }
    fn set_slice(&self, sl: &(V, V, V), v: &V) -> Result<(), E> {
        // the value's bytes first: assigning the bytearray to itself (x[1:1] = x) copies it before changing it
        let src = match v {
            V::Str(_) | V::Int(_) | V::Bool(_) | V::Big(_) | V::Float(_) | V::None => {
                return type_err("can assign only bytes, buffers, or iterables of ints in range(0, 256)".into())
            }
            _ => match buf_full(v)? {
                Some(b) => b,
                None => match iter(v) {
                    Ok(_) => ba_ints(v)?,
                    Err(_) => return type_err("can assign only bytes, buffers, or iterables of ints in range(0, 256)".into()),
                },
            },
        };
        let n = self.d.borrow().len();
        let (st, en, step) = slice_idx(sl, n)?;
        if step == 1 {
            let (st, en) = (st as usize, (en.max(st)) as usize);
            if en - st != src.len() {
                ba_resize_ok(self)?;
            }
            self.d.borrow_mut().splice(st..en, src);
            return Ok(());
        }
        if src.is_empty() {
            // CPython: assigning nothing to an extended slice deletes it
            return self.del_slice(sl);
        }
        let idx = ba_positions(st, en, step);
        if idx.len() != src.len() {
            return val_err(format!("attempt to assign bytes of size {} to extended slice of size {}", src.len(), idx.len()));
        }
        let mut d = self.d.borrow_mut();
        for (k, i) in idx.into_iter().enumerate() {
            d[i] = src[k];
        }
        Ok(())
    }
    fn del_slice(&self, sl: &(V, V, V)) -> Result<(), E> {
        let n = self.d.borrow().len();
        let (st, en, step) = slice_idx(sl, n)?;
        let mut idx = ba_positions(st, en, step);
        if idx.is_empty() {
            return Ok(());
        }
        ba_resize_ok(self)?;
        idx.sort_unstable();
        let mut d = self.d.borrow_mut();
        let mut k = 0;
        let mut out = Vec::with_capacity(d.len() - idx.len());
        for (i, &c) in d.iter().enumerate() {
            if k < idx.len() && idx[k] == i {
                k += 1;
            } else {
                out.push(c);
            }
        }
        *d = out;
        Ok(())
    }
    fn index_of(&self, i: &V) -> Result<usize, E> {
        let k = self.index_raw(i)?;
        self.bound(k)
    }
    // CPython converts the index first (a huge one is an IndexError), then checks the value, then the bounds
    fn index_raw(&self, i: &V) -> Result<i64, E> {
        Ok(match i {
            V::Int(_) | V::Bool(_) | V::Big(_) => idx_of(i)?,
            V::Obj(o) if o.cls.lookup("__index__").is_some() || inner_of(i).is_some() => {
                ba_index(i).map_err(|_| exc("TypeError", format!("bytearray indices must be integers or slices, not {}", tn(i))))?
            }
            o => return type_err(format!("bytearray indices must be integers or slices, not {}", tn(o))),
        })
    }
    fn bound(&self, k: i64) -> Result<usize, E> {
        let n = self.d.borrow().len() as i64;
        let k = if k < 0 { k + n } else { k };
        if k < 0 || k >= n {
            return Err(exc("IndexError", "bytearray index out of range".into()));
        }
        Ok(k as usize)
    }
    fn call(&self, me: &V, n: &str, a: &mut Args) -> R {
        let want = |a: &Args, k: usize, lo: usize| -> Result<(), E> {
            if !a.k.is_empty() {
                return type_err(format!("bytearray.{n}() takes no keyword arguments"));
            }
            if a.p.len() > k || a.p.len() < lo {
                return type_err(match (lo, k) {
                    (0, 0) => format!("bytearray.{n}() takes no arguments ({} given)", a.p.len()),
                    (1, 1) => format!("bytearray.{n}() takes exactly one argument ({} given)", a.p.len()),
                    (l, k) if l == k => format!("{n} expected {k} arguments, got {}", a.p.len()),
                    (_, k) => format!("{n} expected at most {k} argument{}, got {}", if k == 1 { "" } else { "s" }, a.p.len()),
                });
            }
            Ok(())
        };
        match n {
            "append" => {
                want(a, 1, 1)?;
                let c = ba_byte(&a.p[0])?;
                ba_resize_ok(self)?;
                self.d.borrow_mut().push(c);
                Ok(V::None)
            }
            "extend" => {
                want(a, 1, 1)?;
                let x = &a.p[0];
                if memview_of(x).is_some() && bytes_of(x).is_none() {
                    return type_err("can't set bytearray slice from memoryview".into());
                }
                let add = if let Some(b) = bytes_of(x) {
                    b
                } else {
                    match x {
                        V::Str(_) => return type_err("expected iterable of integers; got: 'str'".into()),
                        _ if iter(x).is_err() => return type_err(format!("can't extend bytearray with {}", tn(x))),
                        _ => ba_ints(x)?,
                    }
                };
                if !add.is_empty() {
                    ba_resize_ok(self)?;
                }
                self.d.borrow_mut().extend(add);
                Ok(V::None)
            }
            "insert" => {
                want(a, 2, 2)?;
                let i = ba_index(&a.p[0])?;
                let c = ba_byte(&a.p[1])?;
                ba_resize_ok(self)?;
                let mut d = self.d.borrow_mut();
                let n = d.len() as i64;
                let i = if i < 0 { (i + n).max(0) } else { i.min(n) };
                d.insert(i as usize, c);
                Ok(V::None)
            }
            "pop" => {
                want(a, 1, 0)?;
                let i = match a.p.first() {
                    Some(v) => ba_index(v)?,
                    None => -1,
                };
                let n = self.d.borrow().len() as i64;
                if n == 0 {
                    return Err(exc("IndexError", "pop from empty bytearray".into()));
                }
                let k = if i < 0 { i + n } else { i };
                if k < 0 || k >= n {
                    return Err(exc("IndexError", "pop index out of range".into()));
                }
                ba_resize_ok(self)?;
                Ok(V::Int(self.d.borrow_mut().remove(k as usize) as i64))
            }
            "remove" => {
                want(a, 1, 1)?;
                let c = ba_byte(&a.p[0])?;
                let pos = self.d.borrow().iter().position(|&x| x == c);
                match pos {
                    Some(p) => {
                        ba_resize_ok(self)?;
                        self.d.borrow_mut().remove(p);
                        Ok(V::None)
                    }
                    None => val_err("value not found in bytearray".into()),
                }
            }
            "clear" => {
                want(a, 0, 0)?;
                if !self.d.borrow().is_empty() {
                    ba_resize_ok(self)?;
                }
                self.d.borrow_mut().clear();
                Ok(V::None)
            }
            "reverse" => {
                want(a, 0, 0)?;
                self.d.borrow_mut().reverse();
                Ok(V::None)
            }
            "copy" => {
                want(a, 0, 0)?;
                Ok(bytearrayv(self.d.borrow().clone()))
            }
            "__setitem__" => {
                let (i, v) = (a.p[0].clone(), a.p[1].clone());
                if let V::Slice(sl) = &i {
                    return self.set_slice(sl, &v).map(|_| V::None);
                }
                let k = self.index_raw(&i)?;
                let c = ba_byte(&v)?;
                let k = self.bound(k)?;
                self.d.borrow_mut()[k] = c;
                Ok(V::None)
            }
            "__delitem__" => {
                let i = a.p[0].clone();
                if let V::Slice(sl) = &i {
                    return self.del_slice(sl).map(|_| V::None);
                }
                let k = self.index_of(&i)?;
                ba_resize_ok(self)?;
                self.d.borrow_mut().remove(k);
                Ok(V::None)
            }
            "__getitem__" if !matches!(a.p.first(), Some(V::Slice(_))) => {
                want(a, 1, 1)?;
                let k = self.index_of(&a.p[0])?;
                Ok(V::Int(self.d.borrow()[k] as i64))
            }
            "__len__" => Ok(V::Int(self.d.borrow().len() as i64)),
            "__alloc__" => Ok(V::Int(self.d.borrow().capacity() as i64 + 1)),
            _ if BA_NOT_BYTES.contains(&n) => Err(exc("AttributeError", format!("'bytearray' object has no attribute '{n}'"))),
            _ => {
                let tmp = self.snapshot();
                let V::Ext(x) = &tmp else { unreachable!() };
                let mut a2 = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
                let r = match x.method(&tmp, n, &mut a2) {
                    Some(r) => r.map_err(ba_msg)?,
                    None => return Err(exc("AttributeError", format!("'bytearray' object has no attribute '{n}'"))),
                };
                let _ = me;
                Ok(if BA_WRAPS.contains(&n) { ba_wrap(r) } else { r })
            }
        }
    }
}

// the positions a slice (start, stop, step from slice_idx) visits, in order
fn ba_positions(st: i64, en: i64, step: i64) -> Vec<usize> {
    let mut out = vec![];
    let mut i = st;
    while (step > 0 && i < en) || (step < 0 && i > en) {
        out.push(i as usize);
        i += step;
    }
    out
}

impl Ext for ByteArrayV {
    fn tname(&self) -> &'static str {
        "bytearray"
    }
    fn repr(&self) -> String {
        format!("bytearray({})", bytes_repr(&self.d.borrow()))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        None
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if let Some(m) = memview_of(o) {
            return m.eq(&bytesv(self.d.borrow().clone()));
        }
        Some(bytes_of(o).is_some_and(|b| b == *self.d.borrow()))
    }
    fn cmp(&self, o: &V) -> Option<std::cmp::Ordering> {
        if memview_of(o).is_some() {
            return None;
        }
        bytes_of(o).map(|b| self.d.borrow().as_slice().cmp(b.as_slice()))
    }
    fn truth(&self) -> bool {
        !self.d.borrow().is_empty()
    }
    fn len(&self) -> Option<usize> {
        Some(self.d.borrow().len())
    }
    fn iter(&self) -> Option<Vec<V>> {
        Some(self.d.borrow().iter().map(|&c| V::Int(c as i64)).collect())
    }
    // iter(bytearray) reads the bytes as it goes, so it sees appends made while iterating, as CPython's does
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        let d = self.d.clone();
        let i = Cell::new(0usize);
        Some(Ok(It::Shared(Rc::new(move || {
            let k = i.get();
            let c = d.borrow().get(k).copied();
            if c.is_some() {
                i.set(k + 1);
            } else {
                i.set(usize::MAX);
            }
            Ok(c.map(|c| V::Int(c as i64)))
        }))))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(self.call(me, n, a))
    }
    fn attr(&self, me: &V, n: &str) -> Option<R> {
        const M: &[&str] = &["append", "extend", "insert", "pop", "remove", "clear", "reverse", "copy"];
        if M.contains(&n) {
            return Some(Ok(V::BMeth(Rc::new((me.clone(), sym(n))))));
        }
        if BA_NOT_BYTES.contains(&n) {
            return None;
        }
        // bytes' methods, bound to this bytearray
        let tmp = self.snapshot();
        let V::Ext(x) = &tmp else { return None };
        match x.attr(&tmp, n) {
            Some(Ok(V::BMeth(_))) => Some(Ok(V::BMeth(Rc::new((me.clone(), sym(n)))))),
            _ => None,
        }
    }
    fn binop(&self, me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        match op {
            "+" => {
                let Some(other) = bytes_of(o) else {
                    return (!right).then(|| type_err(format!("can't concat {} to bytearray", tn(o))));
                };
                let mut out = if right { other } else { self.d.borrow().clone() };
                if right {
                    out.extend(self.d.borrow().iter());
                } else {
                    out.extend(bytes_of(o).unwrap_or_default());
                }
                // bytes + bytearray is bytes (the left operand decides), bytearray + anything is a bytearray
                Some(Ok(if right && is_bytes_v(o) { bytesv(out) } else { bytearrayv(out) }))
            }
            "*" => {
                let n = match o {
                    V::Int(n) => *n,
                    V::Bool(b) => *b as i64,
                    V::Big(_) => return Some(Err(exc("OverflowError", "cannot fit 'int' into an index-sized integer".into()))),
                    V::Obj(_) | V::Ext(_) => return None,
                    _ => return Some(type_err(format!("can't multiply sequence by non-int of type '{}'", tn(o)))),
                };
                let _ = me;
                let d = self.d.borrow();
                if (d.len() as u128) * (n.max(0) as u128) > (1u128 << 40) {
                    return Some(Err(exc("MemoryError", String::new())));
                }
                Some(Ok(bytearrayv(d.repeat(n.max(0) as usize))))
            }
            _ => None,
        }
    }
}

// x += y and x *= n on a bytearray change it in place
pub fn bytearray_iop(a: &V, b: &V, op: &str) -> Option<R> {
    let x = bytearray_of(a)?;
    Some(match op {
        "+" => match bytes_of(b) {
            Some(add) => {
                // CPython holds the other operand's buffer while it resizes, so `x += x` cannot grow x
                if !add.is_empty() && bytearray_of(b).is_some_and(|o| Rc::ptr_eq(&o.d, &x.d)) {
                    return Some(Err(exc("BufferError", "Existing exports of data: object cannot be re-sized".into())));
                }
                if !add.is_empty() {
                    if let Err(e) = ba_resize_ok(x) {
                        return Some(Err(e));
                    }
                }
                x.d.borrow_mut().extend(add);
                Ok(a.clone())
            }
            None => type_err(format!("can't concat {} to bytearray", tn(b))),
        },
        "*" => match b {
            V::Big(_) => Err(exc("OverflowError", "cannot fit 'int' into an index-sized integer".into())),
            V::Int(_) | V::Bool(_) => {
                let n = idx_of(b).unwrap_or(0).max(0) as usize;
                let len = x.d.borrow().len();
                if len as u128 * n as u128 > 1u128 << 40 {
                    return Some(Err(exc("MemoryError", String::new())));
                }
                if n != 1 && len != 0 {
                    if let Err(e) = ba_resize_ok(x) {
                        return Some(Err(e));
                    }
                }
                let r = x.d.borrow().repeat(n);
                *x.d.borrow_mut() = r;
                Ok(a.clone())
            }
            _ => type_err(format!("can't multiply sequence by non-int of type '{}'", tn(b))),
        },
        _ => return None,
    })
}
