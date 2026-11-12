// ---------- itertools.product, combinations, combinations_with_replacement, permutations (CPython state) ----------
//
// Each keeps CPython's struct: the pool tuple(s), the index (and cycle) arrays, `stopped`, and whether `result`
// exists yet (`started`). CPython's `result` tuple always equals pool[indices] and is only a reuse optimisation,
// so here a step updates the indices and builds a fresh result tuple from them.

// PyLong_AsSsize_t of a __setstate__ index: an int (or bool) only
fn comb_index(x: &V) -> Result<i64, E> {
    match x {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        _ => type_err("an integer is required".into()),
    }
}
fn ints_tuple(xs: &[usize]) -> V {
    tup(xs.iter().map(|&i| V::Int(i as i64)).collect())
}
fn pick(pool: &[V], idx: &[usize]) -> V {
    tup(idx.iter().map(|&i| pool[i].clone()).collect())
}
fn invalid_args<T>() -> Result<T, E> {
    val_err("invalid arguments".into())
}

// ----- product -----
pub struct ProductIt {
    pools: Vec<Vec<V>>,
    pools_t: V,
    indices: RefCell<Vec<usize>>,
    started: Cell<bool>,
    stopped: Cell<bool>,
}
// `pools` are the pools after `repeat`; the first `uniq` are distinct, the rest repeat them (the same tuple
// objects, as CPython shares them)
pub fn product_new(pools: Vec<Vec<V>>, uniq: usize) -> V {
    let ts: Vec<V> = pools.iter().take(uniq).map(|p| tup(p.clone())).collect();
    let n = pools.len();
    let pools_t = tup((0..n).map(|i| ts[i % uniq].clone()).collect());
    red_iter(ItKind::ItProduct, ProductIt { indices: RefCell::new(vec![0; n]), pools, pools_t, started: Cell::new(false), stopped: Cell::new(false) })
}
impl RedIt for ProductIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.stopped.get() {
            return Ok(None);
        }
        let mut idx = self.indices.borrow_mut();
        if !self.started.get() {
            self.started.set(true);
            if self.pools.iter().any(|p| p.is_empty()) {
                self.stopped.set(true);
                return Ok(None);
            }
        } else {
            let mut i = self.pools.len();
            loop {
                if i == 0 {
                    self.stopped.set(true);
                    return Ok(None);
                }
                i -= 1;
                idx[i] += 1;
                if idx[i] == self.pools[i].len() {
                    idx[i] = 0;
                } else {
                    break;
                }
            }
        }
        Ok(Some(tup(idx.iter().zip(&self.pools).map(|(&i, p)| p[i].clone()).collect())))
    }
    fn reduce(&self, cls: V) -> R {
        Ok(if self.stopped.get() {
            tup(vec![cls, tup(vec![tup(vec![])])])
        } else if !self.started.get() {
            tup(vec![cls, self.pools_t.clone()])
        } else {
            tup(vec![cls, self.pools_t.clone(), ints_tuple(&self.indices.borrow())])
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let n = self.pools.len();
            let V::Tuple(t) = st else { return invalid_args() };
            if t.len() != n {
                return invalid_args();
            }
            for i in 0..n {
                let index = comb_index(&t[i])?;
                let size = self.pools[i].len() as i64;
                if size == 0 {
                    self.stopped.set(true);
                    return Ok(());
                }
                self.indices.borrow_mut()[i] = index.clamp(0, size - 1) as usize;
            }
            self.started.set(true);
            Ok(())
        })())
    }
}

// ----- combinations -----
pub struct CombIt {
    pool: Vec<V>,
    pool_t: V,
    r: usize,
    indices: RefCell<Vec<usize>>,
    started: Cell<bool>,
    stopped: Cell<bool>,
}
pub fn combinations_new(pool: Vec<V>, r: usize) -> V {
    let n = pool.len();
    let indices = if r <= n { (0..r).collect() } else { vec![] };
    red_iter(ItKind::ItCombinations, CombIt { pool_t: tup(pool.clone()), pool, r, indices: RefCell::new(indices), started: Cell::new(false), stopped: Cell::new(r > n) })
}
impl RedIt for CombIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.stopped.get() {
            return Ok(None);
        }
        let (n, r) = (self.pool.len(), self.r);
        let mut idx = self.indices.borrow_mut();
        if !self.started.get() {
            self.started.set(true);
        } else {
            let mut i = r;
            while i > 0 && idx[i - 1] == i - 1 + n - r {
                i -= 1;
            }
            if i == 0 {
                self.stopped.set(true);
                return Ok(None);
            }
            i -= 1;
            idx[i] += 1;
            for j in i + 1..r {
                idx[j] = idx[j - 1] + 1;
            }
        }
        Ok(Some(pick(&self.pool, &idx)))
    }
    fn reduce(&self, cls: V) -> R {
        let args = |p: V| tup(vec![p, V::Int(self.r as i64)]);
        Ok(if !self.started.get() {
            tup(vec![cls, args(self.pool_t.clone())])
        } else if self.stopped.get() {
            tup(vec![cls, args(tup(vec![]))])
        } else {
            tup(vec![cls, args(self.pool_t.clone()), ints_tuple(&self.indices.borrow())])
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let n = self.pool.len() as i64;
            let r = self.r;
            let V::Tuple(t) = st else { return invalid_args() };
            if t.len() != r {
                return invalid_args();
            }
            for i in 0..r {
                let mut index = comb_index(&t[i])?;
                let max = i as i64 + n - r as i64;
                if index > max {
                    index = max;
                }
                if index < 0 {
                    index = 0;
                }
                // r > n: the iterator is stopped and the indices are never read
                if r <= self.pool.len() {
                    self.indices.borrow_mut()[i] = index as usize;
                }
            }
            self.started.set(true);
            Ok(())
        })())
    }
}

// ----- combinations_with_replacement -----
pub struct CwrIt {
    pool: Vec<V>,
    pool_t: V,
    r: usize,
    indices: RefCell<Vec<usize>>,
    started: Cell<bool>,
    stopped: Cell<bool>,
}
pub fn cwr_new(pool: Vec<V>, r: usize) -> V {
    let n = pool.len();
    red_iter(
        ItKind::ItCombinationsWithReplacement,
        CwrIt { pool_t: tup(pool.clone()), pool, r, indices: RefCell::new(vec![0; r]), started: Cell::new(false), stopped: Cell::new(n == 0 && r != 0) },
    )
}
impl RedIt for CwrIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.stopped.get() {
            return Ok(None);
        }
        let n = self.pool.len();
        let mut idx = self.indices.borrow_mut();
        if !self.started.get() {
            self.started.set(true);
        } else {
            let mut i = self.r;
            while i > 0 && idx[i - 1] + 1 == n {
                i -= 1;
            }
            if i == 0 {
                self.stopped.set(true);
                return Ok(None);
            }
            let v = idx[i - 1] + 1;
            for k in i - 1..self.r {
                idx[k] = v;
            }
        }
        Ok(Some(pick(&self.pool, &idx)))
    }
    fn reduce(&self, cls: V) -> R {
        let args = |p: V| tup(vec![p, V::Int(self.r as i64)]);
        Ok(if !self.started.get() {
            tup(vec![cls, args(self.pool_t.clone())])
        } else if self.stopped.get() {
            tup(vec![cls, args(tup(vec![]))])
        } else {
            tup(vec![cls, args(self.pool_t.clone()), ints_tuple(&self.indices.borrow())])
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let V::Tuple(t) = st else { return invalid_args() };
            if t.len() != self.r {
                return invalid_args();
            }
            let n = self.pool.len() as i64;
            for i in 0..self.r {
                let index = comb_index(&t[i])?;
                let index = if index < 0 { 0 } else if index > n - 1 { n - 1 } else { index };
                // an empty pool: CPython stores -1 and would read outside the pool; the iterator is
                // stopped then, so the value is never read
                self.indices.borrow_mut()[i] = index.max(0) as usize;
            }
            self.started.set(true);
            Ok(())
        })())
    }
}

// ----- permutations -----
pub struct PermIt {
    pool: Vec<V>,
    pool_t: V,
    r: usize,
    indices: RefCell<Vec<usize>>,
    cycles: RefCell<Vec<usize>>,
    started: Cell<bool>,
    stopped: Cell<bool>,
}
pub fn permutations_new(pool: Vec<V>, r: usize) -> V {
    let n = pool.len();
    let (cycles, stopped) = if r <= n { ((0..r).map(|i| n - i).collect(), false) } else { (vec![], true) };
    red_iter(
        ItKind::ItPermutations,
        PermIt { pool_t: tup(pool.clone()), pool, r, indices: RefCell::new((0..n).collect()), cycles: RefCell::new(cycles), started: Cell::new(false), stopped: Cell::new(stopped) },
    )
}
impl RedIt for PermIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.stopped.get() {
            return Ok(None);
        }
        let (n, r) = (self.pool.len(), self.r);
        let mut idx = self.indices.borrow_mut();
        if !self.started.get() {
            self.started.set(true);
        } else {
            if n == 0 {
                self.stopped.set(true);
                return Ok(None);
            }
            let mut cyc = self.cycles.borrow_mut();
            let mut i = r;
            loop {
                if i == 0 {
                    self.stopped.set(true);
                    return Ok(None);
                }
                i -= 1;
                cyc[i] -= 1;
                if cyc[i] == 0 {
                    idx[i..n].rotate_left(1);
                    cyc[i] = n - i;
                } else {
                    let j = cyc[i];
                    idx.swap(i, n - j);
                    break;
                }
            }
        }
        Ok(Some(pick(&self.pool, &idx[..r])))
    }
    fn reduce(&self, cls: V) -> R {
        let args = |p: V| tup(vec![p, V::Int(self.r as i64)]);
        Ok(if !self.started.get() {
            tup(vec![cls, args(self.pool_t.clone())])
        } else if self.stopped.get() {
            tup(vec![cls, args(tup(vec![]))])
        } else {
            tup(vec![cls, args(self.pool_t.clone()), tup(vec![ints_tuple(&self.indices.borrow()), ints_tuple(&self.cycles.borrow())])])
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let V::Tuple(t) = st else {
                return type_err("state is not a tuple".into());
            };
            if t.len() != 2 {
                return type_err(format!("function takes exactly 2 arguments ({} given)", t.len()));
            }
            let (V::Tuple(indices), V::Tuple(cycles)) = (&t[0], &t[1]) else {
                let k = if matches!(t[0], V::Tuple(_)) { 1 } else { 0 };
                return type_err(format!("argument {} must be tuple, not {}", k + 1, tn(&t[k])));
            };
            let n = self.pool.len() as i64;
            if indices.len() as i64 != n || cycles.len() != self.r {
                return invalid_args();
            }
            for i in 0..n as usize {
                let index = comb_index(&indices[i])?.clamp(0, n - 1);
                self.indices.borrow_mut()[i] = index as usize;
            }
            for i in 0..self.r {
                let mut index = comb_index(&cycles[i])?;
                if index < 1 {
                    index = 1;
                } else if index > n - i as i64 {
                    index = n - i as i64;
                }
                // r > n: stopped, the cycles are never read
                if self.r <= self.pool.len() {
                    self.cycles.borrow_mut()[i] = index.max(0) as usize;
                }
            }
            self.started.set(true);
            Ok(())
        })())
    }
}
