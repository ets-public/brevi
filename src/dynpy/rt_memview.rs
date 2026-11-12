// ---------- memoryview ----------
// memoryview for dynamic builds, over bytes and bytearray (one dimension, format 'B'): a window (start, step,
// length) on the object's bytes. A view of a bytearray reads and writes its bytes in place and holds an export of
// its buffer (the bytearray cannot be resized) until release() or until the view is gone. cast() and the
// multi-dimensional attributes are refused at build time (mod.rs).

pub struct MemV {
    obj: V,
    start: usize,
    step: isize,
    len: usize,
    readonly: bool,
    hold: RefCell<Option<BufHold>>,
    released: Cell<bool>,
}

fn mv_released() -> E {
    exc("ValueError", "operation forbidden on released memoryview object".into())
}

// CPython's C-contiguous flag for a one-dimensional view: unit stride, or exactly one item
fn mv_contig(m: &MemV) -> bool {
    m.step == 1 || m.len == 1
}

// The view's bytes where CPython asks for a simple buffer (bytes_of): only a live, contiguous view gives one, so `+`,
// `+=`, extend, join, re and decode reject the others with their own TypeError, as in CPython.
pub fn mv_simple(m: &MemV) -> Option<Vec<u8>> {
    (!m.released.get() && mv_contig(m)).then(|| m.bytes())
}

// The value's bytes where CPython takes any buffer (bytes(), bytearray(), slice assignment, ==): every live view
// gives its bytes; a released one raises ValueError.
pub fn buf_full(v: &V) -> Result<Option<Vec<u8>>, E> {
    match memview_of(v) {
        Some(m) => {
            m.live()?;
            Ok(Some(m.bytes()))
        }
        None => Ok(bytes_of(v)),
    }
}

// A view passed to a bytes or bytearray method (find, split, startswith, ...): released raises ValueError, and one
// that is not contiguous raises CPython's BufferError.
pub fn mv_arg_check(a: &Args) -> Result<(), E> {
    for v in a.p.iter().chain(a.k.iter().map(|(_, v)| v)) {
        let vs: Vec<V> = match v {
            V::Tuple(t) => t.to_vec(),
            v => vec![v.clone()],
        };
        for v in &vs {
            if let Some(m) = memview_of(v) {
                m.live()?;
                if !mv_contig(m) {
                    return Err(exc("BufferError", "memoryview: underlying buffer is not C-contiguous".into()));
                }
            }
        }
    }
    Ok(())
}

pub fn memview_of(v: &V) -> Option<&MemV> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<MemV>(),
        _ => None,
    }
}

fn mv_new(obj: V, start: usize, step: isize, len: usize, readonly: bool) -> V {
    let hold = RefCell::new(BufHold::of(&obj));
    V::Ext(ExtRc::new(MemV { obj, start, step, len, readonly, hold, released: Cell::new(false) }))
}

pub fn bi_memoryview(a: Args) -> R {
    let mut a = a;
    let x = a.kw("object");
    a.no_kw("memoryview")?;
    if a.p.len() + x.is_some() as usize > 1 {
        return type_err(format!("memoryview() takes at most 1 argument ({} given)", a.p.len() + x.is_some() as usize));
    }
    let Some(x) = x.or_else(|| a.p.pop()) else {
        return type_err("memoryview() missing required argument 'object' (pos 1)".into());
    };
    if let Some(m) = memview_of(&x) {
        m.live()?;
        return Ok(mv_new(m.obj.clone(), m.start, m.step, m.len, m.readonly));
    }
    // bytes, or an instance of a bytes subclass (the view's obj is the instance, as in CPython)
    if is_bytes_v(&x) || matches!(&x, V::Obj(o) if o.cls.base_bt() == BT::Bytes) {
        let n = bytes_of(&x).map_or(0, |b| b.len());
        return Ok(mv_new(x, 0, 1, n, true));
    }
    // a bytearray, or an instance of a bytearray subclass (writes through the view reach its buffer)
    if let Some(b) = ba_val(&x).as_ref().and_then(bytearray_of) {
        let n = b.d.borrow().len();
        return Ok(mv_new(x, 0, 1, n, false));
    }
    type_err(format!("memoryview: a bytes-like object is required, not '{}'", tn(&x)))
}

impl MemV {
    fn live(&self) -> Result<(), E> {
        if self.released.get() {
            return Err(mv_released());
        }
        Ok(())
    }
    // the bytes the view shows, read from the object now
    pub fn bytes(&self) -> Vec<u8> {
        let all = bytes_of(&self.obj).unwrap_or_default();
        (0..self.len).map(|i| all[(self.start as isize + i as isize * self.step) as usize]).collect()
    }
    fn at(&self, i: &V) -> Result<usize, E> {
        let k = match i {
            V::Int(_) | V::Bool(_) => idx_of(i)?,
            V::Big(_) => return Err(exc("IndexError", "cannot fit 'int' into an index-sized integer".into())),
            V::Obj(o) if o.cls.lookup("__index__").is_some() => idx_of(i)?,
            _ => return type_err("memoryview: invalid slice key".into()),
        };
        let n = self.len as i64;
        let k = if k < 0 { k + n } else { k };
        if k < 0 || k >= n {
            return Err(exc("IndexError", "index out of bounds on dimension 1".into()));
        }
        Ok((self.start as isize + k as isize * self.step) as usize)
    }
    fn call(&self, me: &V, n: &str, a: &mut Args) -> R {
        if n == "release" {
            self.released.set(true);
            self.hold.borrow_mut().take();
            return Ok(V::None);
        }
        if n == "__exit__" {
            self.released.set(true);
            self.hold.borrow_mut().take();
            return Ok(V::Bool(false));
        }
        self.live()?;
        let none = |a: &Args| -> Result<(), E> {
            if !a.p.is_empty() || !a.k.is_empty() {
                return type_err(format!("memoryview.{n}() takes no arguments ({} given)", a.p.len() + a.k.len()));
            }
            Ok(())
        };
        match n {
            "__enter__" => Ok(me.clone()),
            "__len__" => Ok(V::Int(self.len as i64)),
            "tobytes" => {
                let order = a.kw("order").or_else(|| a.p.first().cloned());
                if let Some(o) = &order {
                    match o {
                        V::None => {}
                        V::Str(s) if matches!(s.s.as_str(), "C" | "F" | "A") => {}
                        V::Str(_) => return val_err("order must be 'C', 'F' or 'A'".into()),
                        o => return type_err(format!("tobytes() argument 'order' must be str or None, not {}", tn(o))),
                    }
                }
                Ok(bytesv(self.bytes()))
            }
            "tolist" => {
                none(a)?;
                Ok(list(self.bytes().into_iter().map(|c| V::Int(c as i64)).collect()))
            }
            "toreadonly" => {
                none(a)?;
                Ok(mv_new(self.obj.clone(), self.start, self.step, self.len, true))
            }
            "hex" => {
                let tmp = bytesv(self.bytes());
                let V::Ext(x) = &tmp else { unreachable!() };
                let mut a2 = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
                x.method(&tmp, "hex", &mut a2).unwrap_or_else(|| Ok(V::None))
            }
            "__getitem__" => {
                let i = a.p.first().cloned().unwrap_or(V::None);
                if let V::Slice(sl) = &i {
                    let (st, en, step) = slice_idx(sl, self.len)?;
                    let cnt = if step > 0 { ((en - st).max(0) + step - 1) / step } else { ((st - en).max(0) + (-step) - 1) / (-step) };
                    let start = if cnt == 0 { self.start } else { (self.start as isize + st as isize * self.step) as usize };
                    return Ok(mv_new(self.obj.clone(), start, self.step * step as isize, cnt as usize, self.readonly));
                }
                if matches!(i, V::Ellipsis) {
                    return Ok(me.clone());
                }
                let k = self.at(&i)?;
                Ok(V::Int(bytes_of(&self.obj).unwrap_or_default()[k] as i64))
            }
            "__setitem__" => {
                if self.readonly {
                    return type_err("cannot modify read-only memory".into());
                }
                let (i, v) = (a.p[0].clone(), a.p[1].clone());
                let held = ba_val(&self.obj);
                let Some(b) = held.as_ref().and_then(bytearray_of) else { return type_err("cannot modify read-only memory".into()) };
                if let V::Slice(sl) = &i {
                    let src = match buf_full(&v)? {
                        Some(s) => s,
                        None => return type_err(format!("a bytes-like object is required, not '{}'", tn(&v))),
                    };
                    let (st, en, step) = slice_idx(sl, self.len)?;
                    let cnt = if step > 0 { ((en - st).max(0) + step - 1) / step } else { ((st - en).max(0) + (-step) - 1) / (-step) };
                    if src.len() as i64 != cnt {
                        return val_err("memoryview assignment: lvalue and rvalue have different structures".into());
                    }
                    let mut d = b.d.borrow_mut();
                    for (k, c) in src.into_iter().enumerate() {
                        let pos = self.start as isize + (st + k as i64 * step) as isize * self.step;
                        d[pos as usize] = c;
                    }
                    return Ok(V::None);
                }
                let k = self.at(&i)?;
                let c = match &v {
                    V::Int(x) if (0..256).contains(x) => *x as u8,
                    V::Bool(x) => *x as u8,
                    V::Int(_) | V::Big(_) => return val_err("memoryview: invalid value for format 'B'".into()),
                    _ => return type_err(format!("memoryview: invalid type for format 'B'")),
                };
                b.d.borrow_mut()[k] = c;
                Ok(V::None)
            }
            _ => Err(exc("AttributeError", format!("'memoryview' object has no attribute '{n}'"))),
        }
    }
}

impl Ext for MemV {
    fn tname(&self) -> &'static str {
        "memoryview"
    }
    fn repr(&self) -> String {
        let at = self as *const MemV as usize;
        if self.released.get() {
            format!("<released memory at {at:#x}>")
        } else {
            format!("<memory at {at:#x}>")
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        None
    }
    fn eq(&self, o: &V) -> Option<bool> {
        if self.released.get() {
            return Some(memview_of(o).is_some_and(|m| std::ptr::eq(m, self)));
        }
        Some(match memview_of(o) {
            Some(m) if m.released.get() => false,
            Some(m) => m.bytes() == self.bytes(),
            None => bytes_of(o).is_some_and(|b| b == self.bytes()),
        })
    }
    fn truth(&self) -> bool {
        true
    }
    fn len(&self) -> Option<usize> {
        (!self.released.get()).then_some(self.len)
    }
    fn iter(&self) -> Option<Vec<V>> {
        Some(self.bytes().into_iter().map(|c| V::Int(c as i64)).collect())
    }
    // iter(view) reads each byte when it gets to it, as CPython's does
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        if let Err(e) = self.live() {
            return Some(Err(e));
        }
        let (obj, start, step, len) = (self.obj.clone(), self.start, self.step, self.len);
        let i = Cell::new(0usize);
        Some(Ok(It::Shared(Rc::new(move || {
            let k = i.get();
            if k >= len {
                return Ok(None);
            }
            i.set(k + 1);
            let all = bytes_of(&obj).unwrap_or_default();
            Ok(Some(V::Int(all[(start as isize + k as isize * step) as usize] as i64)))
        }))))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(self.call(me, n, a))
    }
    fn attr(&self, me: &V, n: &str) -> Option<R> {
        const M: &[&str] = &["tobytes", "tolist", "toreadonly", "hex", "release", "__enter__", "__exit__"];
        if M.contains(&n) {
            return Some(Ok(V::BMeth(Rc::new((me.clone(), sym(n))))));
        }
        if let Err(e) = self.live() {
            return Some(Err(e));
        }
        Some(Ok(match n {
            "obj" => self.obj.clone(),
            "nbytes" => V::Int(self.len as i64),
            "readonly" => V::Bool(self.readonly),
            "itemsize" => V::Int(1),
            "format" => s("B"),
            "ndim" => V::Int(1),
            "shape" => tup(vec![V::Int(self.len as i64)]),
            "strides" => tup(vec![V::Int(self.step as i64)]),
            "c_contiguous" | "f_contiguous" | "contiguous" => V::Bool(self.step == 1 || self.len == 1),
            _ => return None,
        }))
    }
}

// hash(view): a read-only view hashes as its bytes; a writable one cannot be hashed
pub fn memview_hash(v: &V) -> Option<Result<i64, E>> {
    let m = memview_of(v)?;
    if m.released.get() {
        return Some(Err(mv_released()));
    }
    if !m.readonly {
        return Some(Err(exc("ValueError", "cannot hash writable memoryview object".into())));
    }
    Some(hash_of(&bytesv(m.bytes())))
}
