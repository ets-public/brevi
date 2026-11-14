// ---------- iterators CPython pickles and copies (__reduce__, __setstate__) ----------
//
// CPython 3.13 gives most itertools iterators, and map, filter, zip and enumerate, a __reduce__ that names the
// iterator's own class and its state, and (for some) a __setstate__. pickle, copy.copy and copy.deepcopy all go
// through them, and the itertools ones issue a DeprecationWarning each time (removed in 3.14). Here each such
// iterator keeps its state the way CPython's C struct does (It::Red), so __reduce__ returns CPython's tuple,
// calling the class rebuilds the iterator, and __setstate__ restores it: a pickle made by this program loads as
// Brevi's own iterator again, with the same type, and copies are independent of the original.

// One iterator kind's state and its three operations. `step` holds no borrow of the state while it calls user
// code or steps an input, so a callback may re-enter the iterator, as with CPython's C iterators.
pub trait RedIt {
    fn step(&self) -> Result<Option<V>, E>;
    // CPython's __reduce__() value; `cls` is the iterator's class (Py_TYPE(self))
    fn reduce(&self, cls: V) -> R;
    // __setstate__(state); None: the type has no __setstate__
    fn setstate(&self, _st: &V) -> Option<Result<(), E>> {
        None
    }
    // repr(); None: the default `<module.name object at 0x...>`
    fn repr(&self) -> Option<Result<String, E>> {
        None
    }
    // the shared state of a groupby iterator, which itertools._grouper(parent, key) needs
    fn groupby_state(&self) -> Option<Rc<GbState>> {
        None
    }
}

pub fn red_iter(kind: ItKind, r: impl RedIt + 'static) -> V {
    new_iter(It::Red(Rc::new(r)), IterClass::Kind(kind))
}

// An iterator held by another one, as CPython holds the object PyObject_GetIter returned: `obj` is what
// __reduce__ names (the iterator object itself), `it` is what is stepped.
#[derive(Clone)]
pub struct In {
    pub obj: V,
    pub it: Rc<RefCell<It>>,
}
impl In {
    #[inline]
    pub fn next(&self) -> Result<Option<V>, E> {
        iter_step(&self.it)
    }
}
// PyIter_Check: the value is an iterator (it has __next__), as __setstate__ checks its arguments.
pub fn is_iterator(v: &V) -> bool {
    match v {
        V::Iter(_) => true,
        V::Obj(o) => o.cls.lookup("__next__").is_some(),
        V::Ext(_) => !is_bytes_v(v) && getattr(v, "__next__").is_ok(),
        _ => false,
    }
}
// An iterator value as an input, unchanged (it is its own iterator).
pub fn in_of_iterator(v: &V) -> Result<In, E> {
    Ok(match v {
        V::Iter(c) => {
            // the wrapper iter() makes around an iterator object of the program names that object
            let inner = match &*c.it.borrow() {
                It::Obj(o) => Some(o.clone()),
                _ => None,
            };
            In { obj: inner.unwrap_or_else(|| v.clone()), it: c.it.clone() }
        }
        V::Obj(_) => In { obj: v.clone(), it: Rc::new(RefCell::new(It::Obj(v.clone()))) },
        _ => In { obj: v.clone(), it: Rc::new(RefCell::new(iter(v)?)) },
    })
}
// PyObject_GetIter(x) as an input.
pub fn get_iter(x: &V) -> Result<In, E> {
    // an object of the program goes through its __iter__ (one with __next__ and no __iter__ is not iterable)
    if matches!(x, V::Iter(_)) || (matches!(x, V::Ext(_)) && is_iterator(x)) {
        return in_of_iterator(x);
    }
    let v = iter_v(x)?;
    in_of_iterator(&v)
}

thread_local! {
    // the line of CPython's copy.py running now, while copy.copy or copy.deepcopy calls __reduce_ex__ (or
    // __setstate__): the frame CPython's warning names then
    static COPY_LINE: Cell<Option<usize>> = const { Cell::new(None) };
}
const PICKLE_MSG: &str = "Pickle, copy, and deepcopy support will be removed from itertools in Python 3.14.";
// CPython 3.13's warning in every itertools __reduce__ and __setstate__, at the Python frame that called it: the
// program's line, or copy.py's when the copy module made the call.
pub fn it_pickle_warn() -> Result<(), E> {
    match COPY_LINE.get() {
        Some(line) => re_warn_at("DeprecationWarning", PICKLE_MSG.into(), "<copy>", line, "copy"),
        None => re_warn("DeprecationWarning", PICKLE_MSG.into()),
    }
}
struct CopyLine(Option<usize>);
impl Drop for CopyLine {
    fn drop(&mut self) {
        COPY_LINE.set(self.0);
    }
}
fn copy_line(line: Option<usize>) -> CopyLine {
    CopyLine(COPY_LINE.replace(line))
}
fn is_itertools_kind(c: &IterCell) -> bool {
    match &c.kind {
        IterClass::Kind(k) => matches!(it_class(*k), Ok(cls) if cls.module == "itertools"),
        IterClass::Class(_) => false,
    }
}
fn cls_of(c: &IterCell) -> Result<V, E> {
    Ok(V::Class(match &c.kind {
        IterClass::Kind(k) => it_class(*k)?,
        IterClass::Class(k) => k.clone(),
    }))
}
fn red_of(c: &IterCell) -> Option<Rc<dyn RedIt>> {
    match c.it.try_borrow().as_deref() {
        Ok(It::Red(r)) => Some(r.clone()),
        _ => None,
    }
}
fn bv(name: &str) -> V {
    builtin_value(name).unwrap_or(V::None)
}
// CPython's copyreg error, naming the type as `module.name` (just `name` for builtins)
fn cannot_pickle(v: &V) -> E {
    let name = match v {
        V::Iter(c) => match cls_of(c) {
            Ok(V::Class(k)) if k.module == "builtins" => k.name.to_string(),
            Ok(V::Class(k)) => format!("{}.{}", k.module, k.name),
            _ => tn(v),
        },
        _ => tn(v),
    };
    exc("TypeError", format!("cannot pickle '{name}' object"))
}

// `x.__reduce__()` (and `__reduce_ex__(protocol)`, the same for every type here) of an iterator of the program:
// CPython's value, with Brevi's own `iter` and classes in it.
pub fn iter_reduce_v(v: &V) -> R {
    let V::Iter(cell) = v else {
        return Err(cannot_pickle(v));
    };
    if let Some(r) = red_of(cell) {
        if is_itertools_kind(cell) {
            it_pickle_warn()?;
        }
        return r.reduce(cls_of(cell)?);
    }
    let tee = match cell.it.try_borrow().as_deref() {
        Ok(It::Tee(p)) => Some(p.clone()),
        _ => None,
    };
    if let Some(p) = tee {
        return tee_reduce(&p);
    }
    enum Plan {
        At(V, usize),
        Rev(V, i64),
        Seq(Vec<V>),
        Range(i64, i64, i64),
        Dict(Rc<RefCell<Dict>>, usize, u8, u64, usize),
        Set(Rc<RefCell<PSet>>, usize, i64),
        Repeat(V, Option<i64>),
        Deq(V, i64),
    }
    let plan = {
        let Ok(it) = cell.it.try_borrow() else {
            return Err(exc("ValueError", "iterator already executing".into()));
        };
        match &*it {
            It::List(l, i) => Plan::At(V::List(l.clone()), *i),
            // -1 once it has given every item but not yet said so, as CPython's index reads then
            It::RevList(l, i) => Plan::Rev(V::List(l.clone()), *i as i64 - 1),
            It::Tuple(t, i) => Plan::At(tup(t.to_vec()), *i),
            // the iterator keeps a byte offset; CPython's index counts characters
            It::Str(s, i) if s.is_lone() => Plan::At(V::Str(s.clone()), *i),
            It::Str(s, i) => Plan::At(st(s.text().to_string()), s.text()[..*i].chars().count()),
            It::Range(a, b, st) => Plan::Range(*a, *b, *st),
            It::Dict(d, i, kind, ver, n0) => Plan::Dict(d.clone(), *i, *kind, *ver, *n0),
            // reversed() of a dict or its views: CPython pickles it as iter(list of the rest)
            It::RevDict(d, i, kind, used, ver) => {
                let mut sim = It::RevDict(d.clone(), *i, *kind, *used, *ver);
                let mut out = vec![];
                while let Some(x) = sim.next()? {
                    out.push(x);
                }
                Plan::Seq(out)
            }
            // an OrderedDict iterator: CPython's odictiter_reduce pickles it as iter(list of the rest)
            It::ODict(d, i, kind, ver, n0) => {
                let mut sim = It::ODict(d.clone(), *i, *kind, *ver, *n0);
                let mut out = vec![];
                while let Some(x) = sim.next()? {
                    out.push(x);
                }
                Plan::Seq(out)
            }
            It::Set(s, i, n0) => Plan::Set(s.clone(), *i, *n0),
            It::Repeat(x, n) => Plan::Repeat(x.clone(), *n),
            // CPython's dequeiter_reduce: (type(it), (its deque, len(deque) - items left)). An iterator over a
            // deque subclass instance would name the instance, which the iterator here does not keep.
            It::Deq(d, _, left, _, false) if matches!(cell.kind, IterClass::Kind(ItKind::DequeIter)) => {
                if d.borrow().sub {
                    return Err(unsupported("pickling or copying an iterator over an instance of a deque subclass"));
                }
                let n = d.borrow().v.len() as i64;
                Plan::Deq(V::Deque(d.clone()), n - *left as i64)
            }
            It::Done if matches!(cell.kind, IterClass::Kind(ItKind::ListReverseIter)) => Plan::Rev(V::None, -1),
            It::Done if !matches!(cell.kind, IterClass::Kind(ItKind::Generator)) && !is_itertools_kind(cell) => Plan::Seq(vec![]),
            // reversed() of a dict or its views keeps the rest as It::Vec; CPython pickles those iterators as
            // iter(list of the rest), as here
            It::Vec(rest) if matches!(cell.kind, IterClass::Kind(ItKind::DictReverseKeys | ItKind::DictReverseValues | ItKind::DictReverseItems)) => {
                Plan::Seq(rest.as_slice().to_vec())
            }
            // a frozenset's iterator keeps the rest of its keys: CPython's setiter_reduce pickles it as
            // iter(list of the rest), as a set's
            It::Vec(rest) if matches!(cell.kind, IterClass::Kind(ItKind::SetIter)) => Plan::Seq(rest.as_slice().to_vec()),
            // generators, and the iterators CPython gives no __reduce__ (pairwise, batched)
            It::Coro(_) | It::GenExp(..) => return Err(cannot_pickle(v)),
            _ if matches!(cell.kind, IterClass::Kind(ItKind::Generator | ItKind::ItPairwise | ItKind::ItBatched)) => return Err(cannot_pickle(v)),
            _ => return Err(unsupported(&format!("pickling or copying a '{}' object", tn(v)))),
        }
    };
    let iter_f = bv("iter");
    Ok(match plan {
        Plan::At(whole, i) => tup(vec![iter_f, tup(vec![whole]), V::Int(i as i64)]),
        // CPython 3.13 listreviter_reduce: once the index is below 0 (every item given, said or not), reversed([])
        Plan::Rev(_, i) if i < 0 => tup(vec![bv("reversed"), tup(vec![list(vec![])])]),
        Plan::Rev(whole, i) => tup(vec![bv("reversed"), tup(vec![whole]), V::Int(i)]),
        Plan::Seq(xs) => tup(vec![iter_f, tup(vec![list(xs)])]),
        // CPython: (iter, (range(start, start + len * step, step),), None)
        Plan::Range(a, b, step) => {
            let n = range_len(&V::Range(Rc::new((a, b, step)))) as i64;
            let Some(stop) = n.checked_mul(step).and_then(|d| a.checked_add(d)) else {
                return Err(unsupported("pickling a range iterator whose stop is past 64 bits"));
            };
            tup(vec![iter_f, tup(vec![V::Range(Rc::new((a, stop, step)))]), V::None])
        }
        Plan::Dict(d, i, kind, ver, n0) => {
            let mut sim = It::Dict(d, i, kind, ver, n0);
            let mut out = vec![];
            while let Some(x) = sim.next()? {
                out.push(x);
            }
            tup(vec![iter_f, tup(vec![list(out)])])
        }
        Plan::Set(s, i, n0) => {
            let mut sim = It::Set(s, i, n0);
            let mut out = vec![];
            while let Some(x) = sim.next()? {
                out.push(x);
            }
            tup(vec![iter_f, tup(vec![list(out)])])
        }
        Plan::Deq(d, i) => tup(vec![cls_of(cell)?, tup(vec![d, V::Int(i)])]),
        // repeat: (repeat, (element, times)) while it counts, (repeat, (element,)) when it is endless
        Plan::Repeat(x, n) => {
            it_pickle_warn()?;
            match n {
                Some(n) => tup(vec![cls_of(cell)?, tup(vec![x, V::Int(n)])]),
                None => tup(vec![cls_of(cell)?, tup(vec![x])]),
            }
        }
    })
}

// PyLong_AsSsize_t of a __setstate__ index (an int, or an object with __index__)
fn state_index(st: &V) -> Result<i64, E> {
    match st {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        _ => idx_of(st),
    }
}

// A container a Python library handed over (pickle.loads builds the state in Python) as the program's own value,
// so __setstate__ checks it as CPython does (`PyList_Check`, a tuple of items).
fn native_of(v: &V) -> V {
    if let V::Ext(x) = v {
        if let Some(Ok(n)) = x.native() {
            return n;
        }
    }
    v.clone()
}
// `x.__setstate__(state)` of an iterator of the program.
pub fn iter_setstate_v(v: &V, st: &V) -> R {
    let st = &match native_of(st) {
        V::Tuple(t) => tup(t.iter().map(native_of).collect()),
        x => x,
    };
    let V::Iter(cell) = v else {
        return Err(exc("AttributeError", format!("'{}' object has no attribute '__setstate__'", tn(v))));
    };
    let tee = match cell.it.try_borrow().as_deref() {
        Ok(It::Tee(p)) => Some(p.clone()),
        _ => None,
    };
    if let Some(p) = tee {
        return tee_setstate(&p, st).map(|_| V::None);
    }
    if let Some(r) = red_of(cell) {
        let warn = is_itertools_kind(cell);
        return match r.setstate(st) {
            Some(res) => {
                if warn {
                    it_pickle_warn()?;
                }
                res.map(|_| V::None)
            }
            None => Err(exc("AttributeError", format!("'{}' object has no attribute '__setstate__'", tn(v)))),
        };
    }
    let has = matches!(&*cell.it.borrow(), It::List(..) | It::RevList(..) | It::Tuple(..) | It::Str(..) | It::Range(..))
        || (matches!(&*cell.it.borrow(), It::Done) && matches!(cell.kind, IterClass::Kind(ItKind::ListIter | ItKind::TupleIter | ItKind::StrIter | ItKind::StrAsciiIter | ItKind::ListReverseIter | ItKind::RangeIter)));
    if !has {
        if matches!(cell.kind, IterClass::Kind(ItKind::Generator | ItKind::ItPairwise | ItKind::ItBatched | ItKind::ItRepeat | ItKind::ItCount) | IterClass::Kind(ItKind::Map | ItKind::Filter | ItKind::Enumerate)) {
            return Err(exc("AttributeError", format!("'{}' object has no attribute '__setstate__'", tn(v))));
        }
        return Err(unsupported(&format!("__setstate__ of a '{}' object", tn(v))));
    }
    let idx = state_index(st)?;
    let mut g = cell.it.try_borrow_mut().map_err(|_| exc("ValueError", "iterator already executing".into()))?;
    match &mut *g {
        // silently clipped to the sequence, as CPython does
        It::List(l, i) => *i = idx.clamp(0, l.borrow().len() as i64) as usize,
        It::Tuple(t, i) => *i = idx.clamp(0, t.len() as i64) as usize,
        It::Str(s, i) if s.is_lone() => *i = (idx.max(0) as usize).min(s.nchars()),
        It::Str(s, i) => {
            let n = idx.max(0) as usize;
            *i = s.text().char_indices().nth(n).map_or(s.text().len(), |(b, _)| b);
        }
        // CPython's index is the next position (-1: exhausted); Brevi keeps it plus one
        It::RevList(l, i) => *i = (idx.clamp(-1, l.borrow().len() as i64 - 1) + 1) as usize,
        It::Range(a, b, step) => {
            let n = range_len(&V::Range(Rc::new((*a, *b, *step)))) as i64;
            let k = idx.clamp(0, n);
            *a = k.checked_mul(*step).and_then(|d| a.checked_add(d)).unwrap_or(*b);
        }
        _ => {}
    }
    Ok(V::None)
}

// copy.copy and copy.deepcopy of an iterator, as CPython's copy module makes them: __reduce_ex__(4), then
// `cls(*args)` and `__setstate__(state)` (args and state deep-copied first for deepcopy).
pub fn iter_copy(x: &V, memo: Option<&mut Vec<(usize, V)>>) -> R {
    // copy.py 3.13.13: `rv = reductor(4)` is line 88 in copy(), 152 in deepcopy(); `y = func(*args)` 254 and
    // `y.__setstate__(state)` 262 in _reconstruct
    let rv = {
        let _g = copy_line(Some(if memo.is_some() { 152 } else { 88 }));
        iter_reduce_v(x)?
    };
    let V::Tuple(t) = &rv else {
        return Err(unsupported("copying an iterator"));
    };
    let f = t.first().cloned().unwrap_or(V::None);
    let mut args = t.get(1).cloned().unwrap_or_else(|| tup(vec![]));
    let mut state = t.get(2).cloned().filter(|s| !matches!(s, V::None));
    let y = match memo {
        Some(memo) => {
            args = deepcopy(&args, memo)?;
            let y = {
                let _g = copy_line(None);
                call(&f, Args::p(to_vec(&args)?))?
            };
            if let V::Iter(c) = x {
                memo.push((Rc::as_ptr(c) as *const u8 as usize, y.clone()));
            }
            if let Some(s) = state.take() {
                state = Some(deepcopy(&s, memo)?);
            }
            y
        }
        None => {
            let _g = copy_line(None);
            call(&f, Args::p(to_vec(&args)?))?
        }
    };
    if let Some(s) = state {
        let _g = copy_line(Some(262));
        call_method(&y, "__setstate__", Args::p(vec![s]))?;
    }
    Ok(y)
}

// Calling the class of one of Brevi's own iterators (what __reduce__ names, so what pickle and copy call):
// the builtin or itertools function of the same name. None: CPython cannot call that class either.
pub fn iter_class_call(c: &Class, a: Args) -> Option<R> {
    match (c.module, c.name) {
        ("itertools", "_grouper") => Some(grouper_new(a)),
        ("itertools", "_tee") => Some(tee_new(a)),
        ("itertools", "_tee_dataobject") => Some(teedata_new(a)),
        ("itertools", n) => Some(itertools_fn(if n == "chain" { "chain" } else { n }, a)),
        ("builtins", "map") => Some(bi_map(a)),
        ("builtins", "filter") => Some(bi_filter(a)),
        ("builtins", "zip") => Some(bi_zip(a)),
        ("builtins", "enumerate") => Some(bi_enumerate(a)),
        ("builtins", "reversed") => Some(bi_reversed(a)),
        ("collections", "_deque_iterator") => Some(deque_iter_new(a)),
        _ => None,
    }
}

// The class pickle names for `module.name` when it is one of these iterator classes (itertools.chain, map, ...):
// what a pickle made here loads, by reference, as CPython loads its own.
pub fn iter_class_named(m: &str, n: &str) -> Option<V> {
    const KINDS: &[ItKind] = &[
        ItKind::ItCount, ItKind::ItChain, ItKind::ItIslice, ItKind::ItRepeat, ItKind::ItCycle, ItKind::ItAccumulate, ItKind::ItZipLongest,
        ItKind::ItStarmap, ItKind::ItTakewhile, ItKind::ItDropwhile, ItKind::ItCompress, ItKind::ItFilterfalse, ItKind::ItGroupby,
        ItKind::ItGrouper, ItKind::ItProduct, ItKind::ItPermutations, ItKind::ItCombinations, ItKind::ItCombinationsWithReplacement,
        ItKind::ItTee, ItKind::Map, ItKind::Filter, ItKind::Zip, ItKind::Enumerate,
    ];
    if !matches!(m, "itertools" | "builtins") {
        return None;
    }
    if (m, n) == ("itertools", "_tee_dataobject") {
        return Some(V::Class(teedata_class()));
    }
    KINDS.iter().find_map(|k| match it_class(*k) {
        Ok(c) if c.module == m && c.name == n => Some(V::Class(c)),
        _ => None,
    })
}

// ---------- the kinds ----------

// itertools.chain: `source` iterates the iterables, `active` the one being read (CPython's chainobject).
pub struct ChainIt {
    source: RefCell<Option<In>>,
    active: RefCell<Option<In>>,
}
pub fn chain_new(source: In) -> V {
    red_iter(ItKind::ItChain, ChainIt { source: RefCell::new(Some(source)), active: RefCell::new(None) })
}
impl RedIt for ChainIt {
    fn step(&self) -> Result<Option<V>, E> {
        loop {
            // the common case: an item from the active input (only its cell is held, so a re-entrant
            // __setstate__ from the input's own code can replace `active`)
            let cur = self.active.borrow().as_ref().map(|a| a.it.clone());
            if let Some(it) = cur {
                match iter_step(&it) {
                    Ok(Some(x)) => return Ok(Some(x)),
                    Ok(None) => {}
                    Err(E(e)) if is_exc(&e, "StopIteration") => {}
                    Err(e) => return Err(e),
                }
                *self.active.borrow_mut() = None;
            }
            let Some(src) = self.source.borrow().clone() else { return Ok(None) };
            let act = self.active.borrow().clone();
            let act = match act {
                Some(a) => a,
                None => {
                    let x = match src.next() {
                        Ok(Some(x)) => x,
                        Ok(None) => {
                            *self.source.borrow_mut() = None;
                            return Ok(None);
                        }
                        Err(e) => return Err(e),
                    };
                    match get_iter(&x) {
                        Ok(a) => {
                            *self.active.borrow_mut() = Some(a.clone());
                            a
                        }
                        Err(e) => {
                            *self.source.borrow_mut() = None;
                            return Err(e);
                        }
                    }
                }
            };
            match act.next() {
                Ok(Some(x)) => return Ok(Some(x)),
                Ok(None) => {}
                Err(E(e)) if is_exc(&e, "StopIteration") => {}
                Err(e) => return Err(e),
            }
            *self.active.borrow_mut() = None;
        }
    }
    fn reduce(&self, cls: V) -> R {
        Ok(match (&*self.source.borrow(), &*self.active.borrow()) {
            (Some(s), Some(a)) => tup(vec![cls, tup(vec![]), tup(vec![s.obj.clone(), a.obj.clone()])]),
            (Some(s), None) => tup(vec![cls, tup(vec![]), tup(vec![s.obj.clone()])]),
            (None, _) => tup(vec![cls, tup(vec![])]),
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let V::Tuple(t) = st else {
                return type_err("state is not a tuple".into());
            };
            if t.is_empty() {
                return type_err("function takes at least 1 argument (0 given)".into());
            }
            if t.len() > 2 {
                return type_err(format!("function takes at most 2 arguments ({} given)", t.len()));
            }
            if !is_iterator(&t[0]) || (t.len() == 2 && !is_iterator(&t[1])) {
                return type_err("Arguments must be iterators.".into());
            }
            *self.source.borrow_mut() = Some(in_of_iterator(&t[0])?);
            *self.active.borrow_mut() = match t.get(1) {
                Some(a) => Some(in_of_iterator(a)?),
                None => None,
            };
            Ok(())
        })())
    }
}

// itertools.cycle: CPython's cycleobject (it, saved, index, firstpass).
pub struct CycleIt {
    it: RefCell<Option<In>>,
    saved: RefCell<Rc<RefCell<Vec<V>>>>,
    index: Cell<usize>,
    firstpass: Cell<bool>,
}
pub fn cycle_new(it: In) -> V {
    red_iter(ItKind::ItCycle, CycleIt { it: RefCell::new(Some(it)), saved: RefCell::new(Rc::new(RefCell::new(vec![]))), index: Cell::new(0), firstpass: Cell::new(false) })
}
impl RedIt for CycleIt {
    fn step(&self) -> Result<Option<V>, E> {
        let src = self.it.borrow().clone();
        if let Some(src) = src {
            match src.next()? {
                Some(x) => {
                    if !self.firstpass.get() {
                        let saved = self.saved.borrow().clone();
                        saved.borrow_mut().push(x.clone());
                    }
                    return Ok(Some(x));
                }
                None => *self.it.borrow_mut() = None,
            }
        }
        let saved = self.saved.borrow().clone();
        let s = saved.borrow();
        if s.is_empty() {
            return Ok(None);
        }
        let i = self.index.get().min(s.len() - 1);
        let x = s[i].clone();
        self.index.set(if i + 1 >= s.len() { 0 } else { i + 1 });
        Ok(Some(x))
    }
    fn reduce(&self, cls: V) -> R {
        let saved = self.saved.borrow().clone();
        Ok(match &*self.it.borrow() {
            // a new cycle over iter(saved) advanced to index, with the saved items and firstpass set
            None => {
                let it = new_iter(It::List(saved.clone(), self.index.get().min(saved.borrow().len())), IterClass::Kind(ItKind::ListIter));
                tup(vec![cls, tup(vec![it]), tup(vec![V::List(saved), V::Bool(true)])])
            }
            Some(i) => tup(vec![cls, tup(vec![i.obj.clone()]), tup(vec![V::List(saved), V::Bool(self.firstpass.get())])]),
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
            let V::List(saved) = &t[0] else {
                return type_err(format!("argument 1 must be list, not {}", tn(&t[0])));
            };
            let first = match &t[1] {
                V::Int(i) => *i != 0,
                V::Bool(b) => *b,
                V::Big(_) => return Err(exc("OverflowError", "signed integer is greater than maximum".into())),
                V::Float(_) => return type_err("'float' object cannot be interpreted as an integer".into()),
                x => idx_of(x).map_err(|_| exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(x))))? != 0,
            };
            *self.saved.borrow_mut() = saved.clone();
            self.firstpass.set(first);
            self.index.set(0);
            Ok(())
        })())
    }
}

// itertools.count: CPython's countobject. `cnt` is the count while it fits (step 1, int start), i64::MAX once
// it runs on `long_cnt` with any `step` (CPython's slow mode).
pub struct CountIt {
    cnt: Cell<i64>,
    long_cnt: RefCell<Option<V>>,
    step: V,
}
pub fn count_new(start: Option<V>, step: Option<V>) -> R {
    let number = |v: &V| !matches!(v, V::Str(_) | V::None | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) | V::FSet(_) | V::Func(_) | V::Class(_)) && !is_bytes_v(v);
    if start.as_ref().is_some_and(|v| !number(v)) || step.as_ref().is_some_and(|v| !number(v)) {
        return type_err("a number is required".into());
    }
    let is_int = |v: &V| matches!(v, V::Int(_) | V::Bool(_) | V::Big(_));
    let mut fast = start.as_ref().map_or(true, is_int) && step.as_ref().map_or(true, is_int);
    let mut cnt = 0i64;
    if fast {
        match &start {
            Some(V::Int(i)) => cnt = *i,
            Some(V::Bool(b)) => cnt = *b as i64,
            Some(_) => fast = false,
            None => {}
        }
    }
    let step_v = step.unwrap_or(V::Int(1));
    if fast && !matches!(step_v, V::Int(1) | V::Bool(true)) {
        fast = false;
    }
    let (cnt, long_cnt) = if fast { (cnt, None) } else { (i64::MAX, Some(start.unwrap_or(V::Int(0)))) };
    Ok(red_iter(ItKind::ItCount, CountIt { cnt: Cell::new(cnt), long_cnt: RefCell::new(long_cnt), step: step_v }))
}
impl RedIt for CountIt {
    fn step(&self) -> Result<Option<V>, E> {
        let c = self.cnt.get();
        if c != i64::MAX {
            self.cnt.set(c + 1);
            return Ok(Some(V::Int(c)));
        }
        let cur = self.long_cnt.borrow().clone().unwrap_or(V::Int(i64::MAX));
        let next = add(&cur, &self.step)?;
        *self.long_cnt.borrow_mut() = Some(next);
        Ok(Some(cur))
    }
    fn reduce(&self, cls: V) -> R {
        if self.cnt.get() == i64::MAX {
            let cur = self.long_cnt.borrow().clone().unwrap_or(V::Int(i64::MAX));
            return Ok(tup(vec![cls, tup(vec![cur, self.step.clone()])]));
        }
        Ok(tup(vec![cls, tup(vec![V::Int(self.cnt.get())])]))
    }
    fn repr(&self) -> Option<Result<String, E>> {
        Some((|| {
            let Some(cur) = self.long_cnt.borrow().clone() else {
                return Ok(format!("count({})", self.cnt.get()));
            };
            if matches!(self.step, V::Int(1) | V::Bool(true)) {
                return Ok(format!("count({})", repr(&cur)?));
            }
            Ok(format!("count({}, {})", repr(&cur)?, repr(&self.step)?))
        })())
    }
}

// itertools.islice: CPython's isliceobject (it, next, stop, step, cnt; stop -1 = None, it None = exhausted).
pub struct IsliceIt {
    it: RefCell<Option<In>>,
    next: Cell<i64>,
    stop: i64,
    step: i64,
    cnt: Cell<i64>,
}
pub fn islice_new(it: In, start: i64, stop: i64, step: i64) -> V {
    red_iter(ItKind::ItIslice, IsliceIt { it: RefCell::new(Some(it)), next: Cell::new(start), stop, step, cnt: Cell::new(0) })
}
impl RedIt for IsliceIt {
    fn step(&self) -> Result<Option<V>, E> {
        let Some(it) = self.it.borrow().clone() else { return Ok(None) };
        let stop = self.stop;
        let empty = |me: &Self| {
            *me.it.borrow_mut() = None;
            Ok(None)
        };
        while self.cnt.get() < self.next.get() {
            match it.next() {
                Ok(Some(_)) => self.cnt.set(self.cnt.get() + 1),
                Ok(None) => return empty(self),
                Err(e) => {
                    *self.it.borrow_mut() = None;
                    return Err(e);
                }
            }
        }
        if stop != -1 && self.cnt.get() >= stop {
            return empty(self);
        }
        let x = match it.next() {
            Ok(Some(x)) => x,
            Ok(None) => return empty(self),
            Err(e) => {
                *self.it.borrow_mut() = None;
                return Err(e);
            }
        };
        self.cnt.set(self.cnt.get() + 1);
        let old = self.next.get();
        let mut n = old.wrapping_add(self.step);
        if n < old || (stop != -1 && n > stop) {
            n = stop;
        }
        self.next.set(n);
        Ok(Some(x))
    }
    fn reduce(&self, cls: V) -> R {
        let Some(it) = self.it.borrow().clone() else {
            let empty = new_iter(It::List(Rc::new(RefCell::new(vec![])), 0), IterClass::Kind(ItKind::ListIter));
            return Ok(tup(vec![cls, tup(vec![empty, V::Int(0)]), V::Int(0)]));
        };
        let stop = if self.stop == -1 { V::None } else { V::Int(self.stop) };
        Ok(tup(vec![cls, tup(vec![it.obj, V::Int(self.next.get()), stop, V::Int(self.step)]), V::Int(self.cnt.get())]))
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some(state_index(st).map(|c| self.cnt.set(c)))
    }
}

// map: CPython's mapobject (func, iters); no __setstate__.
pub struct MapIt {
    func: V,
    its: Vec<In>,
}
pub fn map_new(func: V, its: Vec<In>) -> V {
    red_iter(ItKind::Map, MapIt { func, its })
}
impl RedIt for MapIt {
    fn step(&self) -> Result<Option<V>, E> {
        let mut args = Vec::with_capacity(self.its.len());
        for it in &self.its {
            match it.next()? {
                Some(x) => args.push(x),
                None => return Ok(None),
            }
        }
        Ok(Some(call(&self.func, Args::p(args))?))
    }
    fn reduce(&self, cls: V) -> R {
        let mut args = vec![self.func.clone()];
        args.extend(self.its.iter().map(|i| i.obj.clone()));
        Ok(tup(vec![cls, tup(args)]))
    }
}

// filter: CPython's filterobject (func, it); func None keeps the true items.
pub struct FilterIt {
    func: V,
    it: In,
}
pub fn filter_new(func: V, it: In) -> V {
    red_iter(ItKind::Filter, FilterIt { func, it })
}
impl RedIt for FilterIt {
    fn step(&self) -> Result<Option<V>, E> {
        loop {
            let Some(x) = self.it.next()? else { return Ok(None) };
            let keep = if matches!(self.func, V::None) || matches!(&self.func, V::Class(c) if c.module == "builtins" && c.name == "bool") {
                truth(&x)?
            } else {
                truth(&call(&self.func, Args::p(vec![x.clone()]))?)?
            };
            if keep {
                return Ok(Some(x));
            }
        }
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.func.clone(), self.it.obj.clone()])]))
    }
}

// zip: CPython's zipobject (ittuple, strict); __setstate__ sets strict.
pub struct ZipIt {
    its: Vec<In>,
    strict: Cell<bool>,
}
pub fn zip_new(its: Vec<In>, strict: bool) -> V {
    red_iter(ItKind::Zip, ZipIt { its, strict: Cell::new(strict) })
}
impl RedIt for ZipIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.its.is_empty() {
            return Ok(None);
        }
        let strict = self.strict.get();
        let mut row = Vec::with_capacity(self.its.len());
        for (i, it) in self.its.iter().enumerate() {
            match it.next()? {
                Some(x) => row.push(x),
                None => {
                    if strict && i > 0 {
                        let plural = if i == 1 { " " } else { "s 1-" };
                        return val_err(format!("zip() argument {} is shorter than argument{plural}{}", i + 1, i));
                    }
                    if strict {
                        for (j, rest) in self.its.iter().enumerate().skip(1) {
                            if rest.next()?.is_some() {
                                let plural = if j == 1 { " " } else { "s 1-" };
                                return val_err(format!("zip() argument {} is longer than argument{plural}{}", j + 1, j));
                            }
                        }
                    }
                    return Ok(None);
                }
            }
        }
        Ok(Some(tup(row)))
    }
    fn reduce(&self, cls: V) -> R {
        let its = tup(self.its.iter().map(|i| i.obj.clone()).collect());
        Ok(if self.strict.get() { tup(vec![cls, its, V::Bool(true)]) } else { tup(vec![cls, its]) })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some(truth(st).map(|t| self.strict.set(t)))
    }
}

// enumerate: CPython's enumobject (it, index); no __setstate__.
pub struct EnumIt {
    it: In,
    index: RefCell<V>,
}
pub fn enumerate_new(it: In, start: V) -> V {
    red_iter(ItKind::Enumerate, EnumIt { it, index: RefCell::new(start) })
}
impl RedIt for EnumIt {
    fn step(&self) -> Result<Option<V>, E> {
        let Some(x) = self.it.next()? else { return Ok(None) };
        let i = self.index.borrow().clone();
        let n = match &i {
            V::Int(k) if *k < i64::MAX => V::Int(k + 1),
            _ => add(&i, &V::Int(1))?,
        };
        *self.index.borrow_mut() = n;
        Ok(Some(tup(vec![i, x])))
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.it.obj.clone(), self.index.borrow().clone()])]))
    }
}

// itertools._grouper(parent, tgtkey), as __reduce__ names it: a grouper that becomes the parent's current one
pub fn grouper_new(a: Args) -> R {
    if !a.k.is_empty() {
        return type_err("_grouper() takes no keyword arguments".into());
    }
    if a.p.len() != 2 {
        return type_err(format!("_grouper expected 2 arguments, got {}", a.p.len()));
    }
    let st = match &a.p[0] {
        V::Iter(c) => red_of(c).and_then(|r| r.groupby_state()),
        _ => None,
    };
    let Some(st) = st else {
        return type_err(format!("_grouper() argument 1 must be itertools.groupby, not {}", tn(&a.p[0])));
    };
    Ok(grouper_make(st, a.p[0].clone(), a.p[1].clone()))
}

// itertools.accumulate: CPython's accumulateobject (it, binop, total, initial). `total` None is NULL (nothing
// summed yet); Some(None) is a running total that is the value None.
pub struct AccumulateIt {
    it: In,
    binop: Option<V>,
    total: RefCell<Option<V>>,
    initial: RefCell<V>,
}
pub fn accumulate_new(it: In, binop: Option<V>, initial: V) -> V {
    red_iter(ItKind::ItAccumulate, AccumulateIt { it, binop, total: RefCell::new(None), initial: RefCell::new(initial) })
}
impl RedIt for AccumulateIt {
    fn step(&self) -> Result<Option<V>, E> {
        let init = self.initial.borrow().clone();
        if !matches!(init, V::None) {
            *self.total.borrow_mut() = Some(init.clone());
            *self.initial.borrow_mut() = V::None;
            return Ok(Some(init));
        }
        let Some(x) = self.it.next()? else { return Ok(None) };
        let total = self.total.borrow().clone();
        let Some(total) = total else {
            *self.total.borrow_mut() = Some(x.clone());
            return Ok(Some(x));
        };
        let n = match &self.binop {
            None => add(&total, &x)?,
            Some(f) => call(f, Args::p(vec![total, x]))?,
        };
        *self.total.borrow_mut() = Some(n.clone());
        Ok(Some(n))
    }
    fn reduce(&self, cls: V) -> R {
        let binop = self.binop.clone().unwrap_or(V::None);
        let init = self.initial.borrow().clone();
        if !matches!(init, V::None) {
            // accumulate(chain((initial,), it), binop) with no total
            let it = chain_new(in_of_iterator(&new_iter(It::Tuple(Tup::from(vec![tup(vec![init]), self.it.obj.clone()]), 0), IterClass::Kind(ItKind::TupleIter)))?);
            return Ok(tup(vec![cls, tup(vec![it, binop]), V::None]));
        }
        let total = self.total.borrow().clone();
        if let Some(V::None) = total {
            // a running total of None: islice(accumulate(chain((None,), it), binop), 1, None)
            let it = chain_new(in_of_iterator(&new_iter(It::Tuple(Tup::from(vec![tup(vec![V::None]), self.it.obj.clone()]), 0), IterClass::Kind(ItKind::TupleIter)))?);
            let acc = accumulate_new(in_of_iterator(&it)?, self.binop.clone(), V::None);
            return Ok(tup(vec![V::Class(it_class(ItKind::ItIslice)?), tup(vec![acc, V::Int(1), V::None])]));
        }
        Ok(tup(vec![cls, tup(vec![self.it.obj.clone(), binop]), total.unwrap_or(V::None)]))
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        *self.total.borrow_mut() = Some(st.clone());
        Some(Ok(()))
    }
}

// itertools.starmap: (func, it); each item is the argument tuple.
pub struct StarmapIt {
    func: V,
    it: In,
}
pub fn starmap_new(func: V, it: In) -> V {
    red_iter(ItKind::ItStarmap, StarmapIt { func, it })
}
impl RedIt for StarmapIt {
    fn step(&self) -> Result<Option<V>, E> {
        let Some(x) = self.it.next()? else { return Ok(None) };
        Ok(Some(call(&self.func, Args::p(to_vec(&x)?))?))
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.func.clone(), self.it.obj.clone()])]))
    }
}

// itertools.takewhile (`flag` is CPython's stop) and dropwhile (`flag` is start).
pub struct WhileIt {
    func: V,
    it: In,
    flag: Cell<bool>,
    take: bool,
}
pub fn while_new(take: bool, func: V, it: In) -> V {
    red_iter(if take { ItKind::ItTakewhile } else { ItKind::ItDropwhile }, WhileIt { func, it, flag: Cell::new(false), take })
}
impl RedIt for WhileIt {
    fn step(&self) -> Result<Option<V>, E> {
        if self.take {
            if self.flag.get() {
                return Ok(None);
            }
            let Some(x) = self.it.next()? else { return Ok(None) };
            if truth(&call(&self.func, Args::p(vec![x.clone()]))?)? {
                return Ok(Some(x));
            }
            self.flag.set(true);
            return Ok(None);
        }
        loop {
            let Some(x) = self.it.next()? else { return Ok(None) };
            if self.flag.get() {
                return Ok(Some(x));
            }
            if !truth(&call(&self.func, Args::p(vec![x.clone()]))?)? {
                self.flag.set(true);
                return Ok(Some(x));
            }
        }
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.func.clone(), self.it.obj.clone()]), V::Int(self.flag.get() as i64)]))
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some(truth(st).map(|t| self.flag.set(t)))
    }
}

// itertools.compress: (data, selectors), stepped data first.
pub struct CompressIt {
    data: In,
    sel: In,
}
pub fn compress_new(data: In, sel: In) -> V {
    red_iter(ItKind::ItCompress, CompressIt { data, sel })
}
impl RedIt for CompressIt {
    fn step(&self) -> Result<Option<V>, E> {
        loop {
            let Some(x) = self.data.next()? else { return Ok(None) };
            let Some(s) = self.sel.next()? else { return Ok(None) };
            if truth(&s)? {
                return Ok(Some(x));
            }
        }
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.data.obj.clone(), self.sel.obj.clone()])]))
    }
}

// itertools.filterfalse: (func, it); func None (or bool) tests the item itself.
pub struct FilterfalseIt {
    func: V,
    it: In,
}
pub fn filterfalse_new(func: V, it: In) -> V {
    red_iter(ItKind::ItFilterfalse, FilterfalseIt { func, it })
}
impl RedIt for FilterfalseIt {
    fn step(&self) -> Result<Option<V>, E> {
        loop {
            let Some(x) = self.it.next()? else { return Ok(None) };
            let t = if matches!(self.func, V::None) || matches!(&self.func, V::Class(c) if c.module == "builtins" && c.name == "bool") {
                truth(&x)?
            } else {
                truth(&call(&self.func, Args::p(vec![x.clone()]))?)?
            };
            if !t {
                return Ok(Some(x));
            }
        }
    }
    fn reduce(&self, cls: V) -> R {
        Ok(tup(vec![cls, tup(vec![self.func.clone(), self.it.obj.clone()])]))
    }
}

// itertools.zip_longest: CPython's ziplongestobject (ittuple with exhausted slots cleared, numactive, fillvalue).
pub struct ZipLongestIt {
    its: RefCell<Vec<Option<In>>>,
    active: Cell<usize>,
    fill: RefCell<V>,
}
pub fn zip_longest_new(its: Vec<In>, fill: V) -> V {
    let n = its.len();
    red_iter(ItKind::ItZipLongest, ZipLongestIt { its: RefCell::new(its.into_iter().map(Some).collect()), active: Cell::new(n), fill: RefCell::new(fill) })
}
impl RedIt for ZipLongestIt {
    fn step(&self) -> Result<Option<V>, E> {
        let n = self.its.borrow().len();
        if n == 0 || self.active.get() == 0 {
            return Ok(None);
        }
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            let slot = self.its.borrow()[i].clone();
            let item = match slot {
                None => self.fill.borrow().clone(),
                Some(it) => match it.next() {
                    Ok(Some(x)) => x,
                    r => {
                        self.active.set(self.active.get() - 1);
                        if self.active.get() == 0 || r.is_err() {
                            self.active.set(0);
                            return r.map(|_| None);
                        }
                        self.its.borrow_mut()[i] = None;
                        self.fill.borrow().clone()
                    }
                },
            };
            row.push(item);
        }
        Ok(Some(tup(row)))
    }
    fn reduce(&self, cls: V) -> R {
        let args = self.its.borrow().iter().map(|s| match s {
            Some(i) => i.obj.clone(),
            None => tup(vec![]),
        }).collect();
        Ok(tup(vec![cls, tup(args), self.fill.borrow().clone()]))
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        *self.fill.borrow_mut() = st.clone();
        Some(Ok(()))
    }
}

// itertools.groupby and its _grouper: CPython's groupbyobject (it, keyfunc, tgtkey, currkey, currvalue, and
// the current grouper, which only that grouper may read from).
pub struct GbState {
    it: In,
    keyfunc: V,
    tgtkey: RefCell<Option<V>>,
    currkey: RefCell<Option<V>>,
    currvalue: RefCell<Option<V>>,
    // the id of the current grouper (0: none) and the last id given
    curr: Cell<u64>,
    ids: Cell<u64>,
}
pub struct GroupbyIt {
    st: Rc<GbState>,
    // the groupby iterator itself, which its groupers name as their parent (set right after it is made), and
    // its state cell: a `for` loop keeps only the cell, so the parent is remade around it when the first is gone
    me: RefCell<Option<std::rc::Weak<IterCell>>>,
    cell: RefCell<std::rc::Weak<RefCell<It>>>,
}
pub fn groupby_new(it: In, keyfunc: V) -> V {
    let st = Rc::new(GbState { it, keyfunc, tgtkey: RefCell::new(None), currkey: RefCell::new(None), currvalue: RefCell::new(None), curr: Cell::new(0), ids: Cell::new(0) });
    let g = Rc::new(GroupbyIt { st, me: RefCell::new(None), cell: RefCell::new(std::rc::Weak::new()) });
    let v = new_iter(It::Red(g.clone()), IterClass::Kind(ItKind::ItGroupby));
    if let V::Iter(c) = &v {
        *g.me.borrow_mut() = Some(Rc::downgrade(c));
        *g.cell.borrow_mut() = Rc::downgrade(&c.it);
    }
    v
}
fn gb_step(st: &GbState) -> Result<bool, E> {
    let Some(v) = st.it.next()? else { return Ok(false) };
    let k = if matches!(st.keyfunc, V::None) { v.clone() } else { call(&st.keyfunc, Args::p(vec![v.clone()]))? };
    *st.currvalue.borrow_mut() = Some(v);
    *st.currkey.borrow_mut() = Some(k);
    Ok(true)
}
impl RedIt for GroupbyIt {
    fn step(&self) -> Result<Option<V>, E> {
        let st = &self.st;
        st.curr.set(0);
        loop {
            let cur = st.currkey.borrow().clone();
            let tgt = st.tgtkey.borrow().clone();
            match (cur, tgt) {
                (None, _) => {}
                (Some(_), None) => break,
                (Some(c), Some(t)) => {
                    if !same_or_eq(&t, &c)? {
                        break;
                    }
                }
            }
            if !gb_step(st)? {
                return Ok(None);
            }
        }
        let k = st.currkey.borrow().clone().unwrap_or(V::None);
        *st.tgtkey.borrow_mut() = Some(k.clone());
        let alive = self.me.borrow().as_ref().and_then(|w| w.upgrade());
        let parent = match alive {
            Some(c) => V::Iter(c),
            None => match self.cell.borrow().upgrade() {
                Some(it) => {
                    let c = Rc::new(IterCell { kind: IterClass::Kind(ItKind::ItGroupby), it });
                    *self.me.borrow_mut() = Some(Rc::downgrade(&c));
                    V::Iter(c)
                }
                None => V::None,
            },
        };
        let g = grouper_make(st.clone(), parent, k.clone());
        Ok(Some(tup(vec![k, g])))
    }
    fn reduce(&self, cls: V) -> R {
        let st = &self.st;
        let (t, c, v) = (st.tgtkey.borrow().clone(), st.currkey.borrow().clone(), st.currvalue.borrow().clone());
        Ok(match (t, c, v) {
            (Some(t), Some(c), Some(v)) => tup(vec![cls, tup(vec![st.it.obj.clone(), st.keyfunc.clone()]), tup(vec![c, v, t])]),
            _ => tup(vec![cls, tup(vec![st.it.obj.clone(), st.keyfunc.clone()])]),
        })
    }
    fn setstate(&self, st: &V) -> Option<Result<(), E>> {
        Some((|| {
            let V::Tuple(t) = st else {
                return type_err("state is not a tuple".into());
            };
            if t.len() != 3 {
                return type_err(format!("function takes exactly 3 arguments ({} given)", t.len()));
            }
            *self.st.currkey.borrow_mut() = Some(t[0].clone());
            *self.st.currvalue.borrow_mut() = Some(t[1].clone());
            *self.st.tgtkey.borrow_mut() = Some(t[2].clone());
            Ok(())
        })())
    }
    fn groupby_state(&self) -> Option<Rc<GbState>> {
        Some(self.st.clone())
    }
}
pub struct GrouperIt {
    st: Rc<GbState>,
    parent: V,
    tgtkey: V,
    id: u64,
}
fn grouper_make(st: Rc<GbState>, parent: V, tgtkey: V) -> V {
    let id = st.ids.get() + 1;
    st.ids.set(id);
    st.curr.set(id);
    red_iter(ItKind::ItGrouper, GrouperIt { st, parent, tgtkey, id })
}
impl RedIt for GrouperIt {
    fn step(&self) -> Result<Option<V>, E> {
        let st = &self.st;
        if st.curr.get() != self.id {
            return Ok(None);
        }
        if st.currvalue.borrow().is_none() && !gb_step(st)? {
            return Ok(None);
        }
        let c = st.currkey.borrow().clone().unwrap_or(V::None);
        if !same_or_eq(&self.tgtkey, &c)? {
            return Ok(None);
        }
        let r = st.currvalue.borrow_mut().take();
        *st.currkey.borrow_mut() = None;
        Ok(r)
    }
    fn reduce(&self, cls: V) -> R {
        if self.st.curr.get() != self.id {
            return Ok(tup(vec![bv("iter"), tup(vec![tup(vec![])])]));
        }
        Ok(tup(vec![cls, tup(vec![self.parent.clone(), self.tgtkey.clone()])]))
    }
}
