// ---------- range over integers past 64 bits ----------
//
// range() whose start, stop or step does not fit in an i64 (range(maxsize - 5, maxsize + 5)). CPython's
// longrangeobject: the same length, items, repr, ==, hash, `in`, index, count and iteration, computed with big
// integers. A small range stays V::Range. Slicing one is refused where it happens.

pub struct BigRange {
    start: Big,
    stop: Big,
    step: Big,
    len: Big,
}
fn bigi64(i: i64) -> Big {
    Big::from_i64(i)
}
impl BigRange {
    fn new(start: Big, stop: Big, step: Big) -> BigRange {
        let zero = bigi64(0);
        let one = bigi64(1);
        let pos = step.cmp(&zero) == std::cmp::Ordering::Greater;
        let (lo, hi, st) = if pos { (start.clone(), stop.clone(), step.clone()) } else { (stop.clone(), start.clone(), step.neg()) };
        // compute_range_length: (hi - lo - 1) // step + 1 when lo < hi
        let len = if lo.cmp(&hi) == std::cmp::Ordering::Less { hi.sub(&lo).sub(&one).divmod(&st).0.add(&one) } else { zero };
        BigRange { start, stop, step, len }
    }
    fn item(&self, i: &Big) -> Big {
        self.start.add(&i.mul(&self.step))
    }
    fn contains_int(&self, x: &Big) -> bool {
        use std::cmp::Ordering::*;
        let pos = self.step.cmp(&bigi64(0)) == Greater;
        let inside = if pos { x.cmp(&self.start) != Less && x.cmp(&self.stop) == Less } else { x.cmp(&self.start) != Greater && x.cmp(&self.stop) == Greater };
        inside && x.sub(&self.start).divmod(&self.step).1.is_zero()
    }
    fn len_v(&self) -> V {
        self.len.clone().norm()
    }
}
pub fn bigrange_v(start: Big, stop: Big, step: Big) -> V {
    V::Ext(ExtRc::new(BigRange::new(start, stop, step)))
}
pub fn bigrange_of(v: &V) -> Option<&BigRange> {
    match v {
        V::Ext(x) => x.any().downcast_ref::<BigRange>(),
        _ => None,
    }
}
// The (len, start, step) a range compares and hashes by, as big integers, for a small range or a big one.
fn range_key(v: &V) -> Option<(Big, Big, Big)> {
    if let Some(r) = bigrange_of(v) {
        return Some((r.len.clone(), r.start.clone(), r.step.clone()));
    }
    if let V::Range(r) = v {
        let b = BigRange::new(bigi64(r.0), bigi64(r.1), bigi64(r.2));
        return Some((b.len, b.start, b.step));
    }
    None
}
impl Ext for BigRange {
    fn tname(&self) -> &'static str {
        "range"
    }
    fn repr(&self) -> String {
        if self.step.cmp(&bigi64(1)) == std::cmp::Ordering::Equal {
            format!("range({}, {})", self.start.to_string(), self.stop.to_string())
        } else {
            format!("range({}, {}, {})", self.start.to_string(), self.stop.to_string(), self.step.to_string())
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    // range_hash: hash((len, start, step)) with start and step None for an empty range, step None for one item
    fn hash(&self) -> Option<i64> {
        let n = self.len_v();
        let parts = if self.len.is_zero() {
            vec![n, V::None, V::None]
        } else if self.len.cmp(&bigi64(1)) == std::cmp::Ordering::Equal {
            vec![n, self.start.clone().norm(), V::None]
        } else {
            vec![n, self.start.clone().norm(), self.step.clone().norm()]
        };
        hash_tuple(&parts).ok()
    }
    // range_equals: the same length, and the same start (and step, past one item)
    fn eq(&self, o: &V) -> Option<bool> {
        let (n, s, st) = range_key(o)?;
        if n.cmp(&self.len) != std::cmp::Ordering::Equal {
            return Some(false);
        }
        if n.is_zero() {
            return Some(true);
        }
        if s.cmp(&self.start) != std::cmp::Ordering::Equal {
            return Some(false);
        }
        Some(n.cmp(&bigi64(1)) == std::cmp::Ordering::Equal || st.cmp(&self.step) == std::cmp::Ordering::Equal)
    }
    fn truth(&self) -> bool {
        !self.len.is_zero()
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        let cur = RefCell::new(self.start.clone());
        let left = RefCell::new(self.len.clone());
        let step = self.step.clone();
        Some(Ok(It::Shared(Rc::new(move || {
            if left.borrow().is_zero() {
                return Ok(None);
            }
            let v = cur.borrow().clone();
            *cur.borrow_mut() = v.add(&step);
            let l = left.borrow().sub(&bigi64(1));
            *left.borrow_mut() = l;
            Ok(Some(v.norm()))
        }))))
    }
    fn iter_class(&self, _me: &V) -> Option<Rc<Class>> {
        Some(ext_class_mod("builtins", "longrange_iterator"))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "start" => self.start.clone().norm(),
            "stop" => self.stop.clone().norm(),
            "step" => self.step.clone().norm(),
            _ => return None,
        }))
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let int_arg = |a: &Args| -> Result<V, E> {
            a.no_kw(n)?;
            a.want(n, 1, 1)?;
            Ok(a.p[0].clone())
        };
        Some((|| -> R {
            Ok(match n {
                "__len__" => {
                    a.want(n, 0, 0)?;
                    match self.len.to_i64() {
                        Some(k) => V::Int(k),
                        None => return Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
                    }
                }
                "__contains__" | "count" | "index" => {
                    let x = int_arg(a)?;
                    let Some(b) = (match &x {
                        V::Int(_) | V::Big(_) | V::Bool(_) => big_of(&x),
                        _ => None,
                    }) else {
                        return Err(unsupported("a non-int item test on a range past 64 bits"));
                    };
                    let has = self.contains_int(&b);
                    match n {
                        "__contains__" => V::Bool(has),
                        "count" => V::Int(has as i64),
                        _ if has => b.sub(&self.start).divmod(&self.step).0.norm(),
                        _ => return val_err(format!("{} is not in range", repr(&x)?)),
                    }
                }
                "__getitem__" => {
                    let k = int_arg(a)?;
                    let Some(mut i) = (match &k {
                        V::Int(_) | V::Big(_) | V::Bool(_) => big_of(&k),
                        _ => None,
                    }) else {
                        if let V::Slice(_) = k {
                            return Err(unsupported("a slice of a range past 64 bits"));
                        }
                        return type_err(format!("range indices must be integers or slices, not {}", tn(&k)));
                    };
                    if i.neg {
                        i = i.add(&self.len);
                    }
                    if i.neg || i.cmp(&self.len) != std::cmp::Ordering::Less {
                        return Err(exc("IndexError", "range object index out of range".into()));
                    }
                    self.item(&i).norm()
                }
                "__reversed__" => {
                    a.want(n, 0, 0)?;
                    let last = if self.len.is_zero() { self.start.clone() } else { self.item(&self.len.sub(&bigi64(1))) };
                    let r = BigRange::new(last.clone(), last.sub(&self.len.mul(&self.step)), self.step.neg());
                    let it = r.lazy_iter().unwrap()?;
                    new_iter(it, IterClass::Class(ext_class_mod("builtins", "longrange_iterator")))
                }
                "__iter__" => iter_v(me)?,
                "__repr__" | "__str__" => st(self.repr()),
                "__hash__" => V::Int(self.hash().unwrap()),
                "__bool__" => V::Bool(self.truth()),
                _ => return Err(exc("AttributeError", format!("'range' object has no attribute '{n}'"))),
            })
        })())
    }
}
