
// ---------- gc and weakref ----------
// Weak references over the runtime's Rc values, and the cycle collector behind gc.collect(). Brevi's values
// are Rc-based, so a reference drops as soon as its last strong owner drops; a cycle never does. CPython's
// gc.collect() collects cycles. Brevi starts from the targets of the weakrefs the program made, walks the
// value graph those targets can reach, and breaks every cycle that has no strong reference from outside
// itself. A cycle nobody weakly referenced is invisible to a program (it has no way to observe the leak), so
// starting at weakref targets is enough for every observable cycle.
//
// The collector is deliberately conservative: any edge it cannot see (inside a closure, an Ext value, a
// class's attributes) shows up as an extra strong count, which marks the target reachable and so keeps it.
// It never collects a value that is still reachable; it may leak a cycle it could not walk.

thread_local! {
    static WEAKREFS: RefCell<Vec<std::rc::Weak<Box<dyn Ext>>>> = const { RefCell::new(Vec::new()) };
}

// The value a weak reference points at, for every Brevi value CPython allows a weak reference to.
enum Wt {
    WObj(std::rc::Weak<Obj>),
    WClass(std::rc::Weak<Class>),
    WFunc(std::rc::Weak<Func>),
    WBound(std::rc::Weak<(V, V)>),
    WBMeth(std::rc::Weak<(V, Sym)>),
    WSet(std::rc::Weak<RefCell<PSet>>),
    WFSet(std::rc::Weak<PSet>),
    // a compiled re.Pattern (CPython's Pattern type takes weak references; Match and the other library
    // values do not)
    WExt(std::rc::Weak<Box<dyn Ext>>),
    // a generator or an itertools.tee iterator (the iterators CPython gives a weakref list)
    WIter(std::rc::Weak<IterCell>),
}

impl Wt {
    fn upgrade(&self) -> Option<V> {
        match self {
            Wt::WObj(w) => w.upgrade().map(V::Obj),
            Wt::WClass(w) => w.upgrade().map(V::Class),
            Wt::WFunc(w) => w.upgrade().map(V::Func),
            Wt::WBound(w) => w.upgrade().map(V::Bound),
            Wt::WBMeth(w) => w.upgrade().map(V::BMeth),
            Wt::WSet(w) => w.upgrade().map(V::Set),
            Wt::WFSet(w) => w.upgrade().map(V::FSet),
            Wt::WExt(w) => w.upgrade().map(|r| V::Ext(ExtRc(r))),
            Wt::WIter(w) => w.upgrade().map(V::Iter),
        }
    }
}

pub struct WeakRefV {
    w: Wt,
    cb: RefCell<Option<V>>,
}

impl WeakRefV {
    fn fire(&self, me: &V) {
        let cb = self.cb.borrow_mut().take();
        if let Some(cb) = cb {
            let _ = call(&cb, Args::p(vec![me.clone()]));
        }
    }
}

impl Ext for WeakRefV {
    fn tname(&self) -> &'static str {
        "weakref.ReferenceType"
    }
    fn repr(&self) -> String {
        if let Some(v) = self.w.upgrade() {
            let at = gc_id(&v).unwrap_or(self as *const WeakRefV as usize);
            format!("<weakref at {:#x}; to '{}' at {:#x}>", self as *const WeakRefV as usize, tn(&v), at)
        } else {
            format!("<weakref at {:#x}; dead>", self as *const WeakRefV as usize)
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        match n {
            "__callback__" => Some(Ok(self.cb.borrow().clone().unwrap_or(V::None))),
            _ => None,
        }
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        if n != "__call__" {
            return None;
        }
        if !a.p.is_empty() || !a.k.is_empty() {
            return Some(type_err("weakref object takes no arguments".into()));
        }
        let v = self.w.upgrade();
        if v.is_none() {
            self.fire(me);
        }
        Some(Ok(v.unwrap_or(V::None)))
    }
}

pub fn weakref_fn(n: &str, a: Args) -> R {
    match n {
        "ref" => {
            let mut a = a;
            if !a.k.is_empty() {
                return type_err("ref() takes no keyword arguments".into());
            }
            a.want("ref", 1, 2)?;
            let cb = if a.p.len() == 2 { a.p[1].clone() } else { V::None };
            let w = weak_target(&a.p[0])?;
            let r = ExtRc::new(WeakRefV { w, cb: RefCell::new(if matches!(cb, V::None) { None } else { Some(cb) }) });
            WEAKREFS.with(|ws| {
                let mut ws = ws.borrow_mut();
                ws.retain(|w| w.strong_count() > 0);
                ws.push(ExtRc::downgrade(&r));
            });
            Ok(V::Ext(r))
        }
        "proxy" => {
            let mut a = a;
            if !a.k.is_empty() {
                return type_err("proxy() takes no keyword arguments".into());
            }
            a.want("proxy", 1, 2)?;
            let w = weak_target(&a.p[0])?;
            // CPython calls a proxy's callback the moment its referent is freed; Brevi frees by reference count
            // and has no hook at that moment, so a callback is refused rather than called late or never.
            if a.p.len() == 2 && !matches!(a.p[1], V::None) {
                return Err(unsupported("weakref.proxy with a callback"));
            }
            // CPython's proxy converts through the referent's __index__/__int__/__float__/__complex__ in every
            // builtin that takes a number (hex, range, chr, math, ...); those Brevi builtins read the object's
            // class directly, so a proxy of such an object is refused instead of failing where CPython converts.
            if let V::Obj(o) = &a.p[0] {
                if let Some(d) = ["__index__", "__int__", "__float__", "__complex__"].into_iter().find(|d| o.cls.lookup(d).is_some()) {
                    return Err(unsupported(&format!("weakref.proxy of an object with {d}")));
                }
            }
            let callable = truth(&bi_callable(Args::p(vec![a.p[0].clone()]))?)?;
            Ok(V::Ext(ExtRc::new(ProxyV { w, callable })))
        }
        _ => Err(unsupported(&format!("weakref.{n}"))),
    }
}

// The weak target for ref() and proxy(): every Brevi value CPython takes a weak reference to.
fn weak_target(x: &V) -> Result<Wt, E> {
    Ok(match x {
        V::Obj(o) => Wt::WObj(Rc::downgrade(o)),
        V::Class(c) => Wt::WClass(Rc::downgrade(c)),
        V::Func(f) => Wt::WFunc(Rc::downgrade(f)),
        V::Bound(b) => Wt::WBound(Rc::downgrade(b)),
        V::BMeth(b) => Wt::WBMeth(Rc::downgrade(b)),
        V::Set(s) => Wt::WSet(Rc::downgrade(s)),
        V::FSet(s) => Wt::WFSet(Rc::downgrade(s)),
        V::Ext(e) if e.tname() == "re.Pattern" => Wt::WExt(ExtRc::downgrade(e)),
        V::Iter(c) if matches!(c.kind, IterClass::Kind(ItKind::Generator | ItKind::ItTee)) => Wt::WIter(Rc::downgrade(c)),
        _ => return type_err(format!("cannot create weak reference to '{}' object", proxy_tn(x))),
    })
}

// weakref.proxy(x): CPython's ProxyType / CallableProxyType. Every operation goes to the referent while it
// lives (attributes, calls, str, truth, len, iteration, items, numbers, comparisons) and raises ReferenceError
// once it is gone; its own are its type, its repr, identity, and hash (a proxy is unhashable). The runtime's
// generic operations unwrap a proxy through proxy_deref where an Ext hook could not carry the error.
pub struct ProxyV {
    w: Wt,
    callable: bool,
}

impl ProxyV {
    fn get(&self) -> R {
        self.w.upgrade().ok_or_else(|| exc("ReferenceError", "weakly-referenced object no longer exists".into()))
    }
}

// The referent of a weakref.proxy (ReferenceError when it is gone); None for any other value.
pub fn proxy_deref(v: &V) -> Option<R> {
    if let V::Ext(x) = v {
        if let Some(p) = x.any().downcast_ref::<ProxyV>() {
            return Some(p.get());
        }
    }
    None
}
// A proxy operand of a binary operation or comparison is its referent, as CPython's proxy slots unwrap both.
fn proxy_arg(v: &V) -> R {
    match proxy_deref(v) {
        Some(r) => r,
        None => Ok(v.clone()),
    }
}

impl Ext for ProxyV {
    fn tname(&self) -> &'static str {
        if self.callable { "weakref.CallableProxyType" } else { "weakref.ProxyType" }
    }
    fn repr(&self) -> String {
        let me = self as *const ProxyV as usize;
        match self.w.upgrade() {
            Some(v) => format!("<weakproxy at {:#x}; to '{}' at {:#x}>", me, tn(&v), gc_id(&v).unwrap_or(me)),
            None => format!("<weakproxy at {:#x}; dead>", me),
        }
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(self.get().and_then(|v| getattr(&v, n)))
    }
    fn set_attr(&self, _me: &V, n: &str, v: V) -> Option<Result<(), E>> {
        Some(self.get().and_then(|t| setattr(&t, sym(n), v)))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let a = std::mem::replace(a, Args::p(vec![]));
        if n == "__call__" && !self.callable {
            return Some(type_err(format!("'{}' object is not callable", self.tname())));
        }
        Some(self.get().and_then(|t| if n == "__call__" { call(&t, a) } else { call_method(&t, n, a) }))
    }
    fn binop(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        Some(self.get().and_then(|t| match op {
            "neg" => neg(&t),
            "pos" => pos(&t),
            "invert" => invert(&t),
            _ => {
                let o = proxy_arg(o)?;
                if right { binop(&o, &t, op) } else { binop(&t, &o, op) }
            }
        }))
    }
    fn eq_rich(&self, _me: &V, o: &V, right: bool) -> Option<Result<bool, E>> {
        Some(self.get().and_then(|t| {
            let o = proxy_arg(o)?;
            if right { eq(&o, &t) } else { eq(&t, &o) }
        }))
    }
    fn rich(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<Result<bool, E>> {
        Some(self.get().and_then(|t| {
            let o = proxy_arg(o)?;
            match op {
                "==" | "!=" | "<" | ">" | "<=" | ">=" => if right { cmp_op(&o, &t, op) } else { cmp_op(&t, &o, op) },
                _ => if right { rich(&o, &t, op) } else { rich(&t, &o, op) },
            }
        }))
    }
    fn cmp_value(&self, _me: &V, op: &str, o: &V, right: bool) -> Option<R> {
        Some(self.get().and_then(|t| {
            let o = proxy_arg(o)?;
            if right { cmp_v(&o, &t, op) } else { cmp_v(&t, &o, op) }
        }))
    }
    fn try_truth(&self) -> Option<Result<bool, E>> {
        Some(self.get().and_then(|t| truth(&t)))
    }
    fn lazy_iter(&self) -> Option<Result<It, E>> {
        Some(self.get().and_then(|t| iter(&t)))
    }
    fn next_item(&self) -> Option<R> {
        Some(self.get().and_then(|t| bi_next(Args::p(vec![t]))))
    }
    fn fmt(&self, spec: &str) -> Option<R> {
        if !spec.is_empty() {
            return Some(type_err(format!("unsupported format string passed to {}.__format__", self.tname())));
        }
        Some(self.get().and_then(|t| to_str(&t)).map(|x| st(x)))
    }
}

pub fn gc_fn(n: &str, a: Args) -> R {
    match n {
        "collect" => {
            let mut a = a;
            let g = a.kw("generation").unwrap_or(V::None);
            a.no_kw("collect")?;
            a.want("collect", 0, 1)?;
            if a.p.len() == 1 && !matches!(g, V::None) {
                return type_err("collect() got multiple values for argument 'generation'".into());
            }
            let freed = gc_collect();
            wr_fire_dead();
            Ok(V::Int(freed as i64))
        }
        _ => Err(unsupported(&format!("gc.{n}"))),
    }
}

fn wr_fire_dead() {
    let ws: Vec<std::rc::Weak<Box<dyn Ext>>> = WEAKREFS.with(|ws| {
        let mut ws = ws.borrow_mut();
        ws.retain(|w| w.strong_count() > 0);
        ws.clone()
    });
    for w in ws {
        if let Some(r) = w.upgrade() {
            if let Some(wr) = r.any().downcast_ref::<WeakRefV>() {
                if wr.w.upgrade().is_none() {
                    let me = V::Ext(ExtRc(r.clone()));
                    wr.fire(&me);
                }
            }
        }
    }
}

// One node of the walk: the value kept alive for the walk (exactly one clone per node), its outgoing nodes,
// and the number of incoming edges from other walked nodes.
struct GcNode {
    v: V,
    out: Vec<usize>,
    indeg: usize,
}

fn gc_id(v: &V) -> Option<usize> {
    Some(match v {
        V::List(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Tuple(x) => Tup::as_ptr(x) as usize,
        V::Dict(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Set(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::FSet(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Obj(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Bound(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::BMeth(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Iter(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Range(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Slice(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Deque(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::View(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Prop(x) => Rc::as_ptr(x) as *const u8 as usize,
        _ => return None,
    })
}

fn strong_of(v: &V) -> usize {
    match v {
        V::List(x) => Rc::strong_count(x),
        V::Tuple(x) => Tup::strong_count(x),
        V::Dict(x) => Rc::strong_count(x),
        V::Set(x) => Rc::strong_count(x),
        V::FSet(x) => Rc::strong_count(x),
        V::Obj(x) => Rc::strong_count(x),
        V::Bound(x) => Rc::strong_count(x),
        V::BMeth(x) => Rc::strong_count(x),
        V::Iter(x) => Rc::strong_count(x),
        V::Range(x) => Rc::strong_count(x),
        V::Slice(x) => Rc::strong_count(x),
        V::Deque(x) => Rc::strong_count(x),
        V::View(x) => Rc::strong_count(x),
        V::Prop(x) => Rc::strong_count(x),
        _ => 0,
    }
}

// The values a container holds strongly. An edge that is not listed here is not lost: it still shows up in
// the target's strong count, so the target looks reachable from outside and the collector keeps it. A
// container currently borrowed by running user code (an iterator's __next__ that called gc.collect, say) is
// treated as opaque: keeping everything it points to is conservative.
fn gc_edges(v: &V) -> Vec<V> {
    match v {
        V::Obj(o) => {
            let Ok(d) = o.d.try_borrow() else { return vec![] };
            let mut out: Vec<V> = d.attrs.iter().map(|(_, x)| x.clone()).collect();
            // a materialized __dict__ is a container of its own: it is an edge, and the walk visits it
            if let Some(dd) = &d.dict {
                out.push(V::Dict(dd.clone()));
            }
            drop(d);
            if let Some(x) = o.inner() {
                out.push(x);
            }
            out
        }
        V::List(l) => match l.try_borrow() {
            Ok(l) => l.iter().cloned().collect(),
            Err(_) => vec![],
        },
        V::Tuple(t) => t.iter().cloned().collect(),
        V::Dict(d) => {
            let Ok(d) = d.try_borrow() else { return vec![] };
            let mut out = Vec::with_capacity(d.n * 2 + d.idx.len());
            for (k, x) in d.ents.iter().flatten() {
                out.push(k.clone());
                out.push(x.clone());
            }
            for k in d.idx.keys() {
                out.push(k.v.clone());
            }
            if let Some(x) = &d.dflt {
                out.push(x.clone());
            }
            if let Some(a) = &d.attrs {
                out.push(V::Dict(a.clone()));
            }
            out
        }
        V::Set(s) => match s.try_borrow() {
            Ok(s) => s.keys(),
            Err(_) => vec![],
        },
        V::FSet(s) => s.keys(),
        V::Deque(d) => match d.try_borrow() {
            Ok(d) => d.v.iter().cloned().collect(),
            Err(_) => vec![],
        },
        V::View(x) => vec![x.0.clone()],
        V::Slice(x) => vec![x.0.clone(), x.1.clone(), x.2.clone()],
        V::Bound(x) => vec![x.0.clone(), x.1.clone()],
        V::BMeth(x) => vec![x.0.clone()],
        V::Prop(x) => vec![x.0.clone(), x.1.clone()],
        V::Range(_) => vec![],
        V::Iter(it) => {
            let Ok(it) = it.it.try_borrow() else { return vec![] };
            let mut out = vec![];
            match &*it {
                It::List(x, _) | It::RevList(x, _) => out.push(V::List(x.clone())),
                It::Tuple(x, _) => out.push(V::Tuple(x.clone())),
                It::Dict(x, ..) | It::RevDict(x, ..) | It::ODict(x, ..) => out.push(V::Dict(x.clone())),
                It::Set(x, ..) => out.push(V::Set(x.clone())),
                It::Obj(x) => out.push(x.clone()),
                _ => {}
            }
            out
        }
        _ => vec![],
    }
}

// Drop every reference a garbage container holds. Only mutable containers are cleared; a Python-constructible
// cycle always passes through one of them, so breaking the mutable links breaks every cycle. Immutable
// containers in the garbage are released with the walk's own last clones.
fn gc_clear(v: &V) {
    // Move the contents out first and drop them after the borrow is released: dropping a value can run code
    // (an Ext's Drop, a generator's finally), which must not happen while the container is still borrowed.
    // A container currently borrowed by running user code is left alone (it cannot be garbage anyway, and
    // clearing it now would panic).
    match v {
        V::Obj(o) => {
            let Ok(mut d) = o.d.try_borrow_mut() else { return };
            let old = std::mem::replace(&mut *d, AStore::new());
            drop(d);
            let mut hold: Vec<V> = old.attrs.into_vec().into_iter().map(|(_, x)| x).collect();
            if let Some(dd) = old.dict {
                hold.push(V::Dict(dd));
            }
            if let Some(x) = o.take_inner() {
                hold.push(*x);
            }
            drop(hold);
        }
        V::List(l) => {
            let Ok(mut l) = l.try_borrow_mut() else { return };
            let old = std::mem::take(&mut *l);
            drop(l);
            drop(old);
        }
        V::Dict(d) => {
            let Ok(mut d) = d.try_borrow_mut() else { return };
            let old = std::mem::replace(&mut *d, Dict::new());
            drop(d);
            drop(old);
        }
        V::Set(s) => {
            let Ok(mut s) = s.try_borrow_mut() else { return };
            let old = std::mem::replace(&mut *s, PSet::new());
            drop(s);
            drop(old);
        }
        V::Deque(d) => {
            let Ok(mut d) = d.try_borrow_mut() else { return };
            let old = std::mem::take(&mut d.v);
            drop(d);
            drop(old);
        }
        V::Iter(it) => {
            let Ok(mut it) = it.it.try_borrow_mut() else { return };
            let old = std::mem::replace(&mut *it, It::Done);
            drop(it);
            drop(old);
        }
        _ => {}
    }
}

fn gc_add(v: V, nodes: &mut Vec<GcNode>, at: &mut HashMap<usize, usize>) -> usize {
    if let Some(id) = gc_id(&v) {
        if let Some(&i) = at.get(&id) {
            return i;
        }
        let i = nodes.len();
        at.insert(id, i);
        nodes.push(GcNode { v, out: vec![], indeg: 0 });
        i
    } else {
        usize::MAX
    }
}

fn gc_collect() -> usize {
    let mut nodes: Vec<GcNode> = vec![];
    let mut at: HashMap<usize, usize> = HashMap::new();
    // The targets of the program's weak references, as the walk's starting points.
    let seeds: Vec<V> = WEAKREFS.with(|ws| {
        let mut ws = ws.borrow_mut();
        ws.retain(|w| w.strong_count() > 0);
        let mut out = vec![];
        for w in ws.iter() {
            if let Some(r) = w.upgrade() {
                if let Some(wr) = r.any().downcast_ref::<WeakRefV>() {
                    if let Some(v) = wr.w.upgrade() {
                        out.push(v);
                    }
                }
            }
        }
        out
    });
    for v in seeds {
        let _ = gc_add(v, &mut nodes, &mut at);
    }
    let mut k = 0;
    while k < nodes.len() {
        let edges = gc_edges(&nodes[k].v);
        for v in edges {
            if gc_id(&v).is_some() {
                let j = gc_add(v, &mut nodes, &mut at);
                nodes[k].out.push(j);
                nodes[j].indeg += 1;
            }
        }
        k += 1;
    }
    // A walked node has exactly one clone in `nodes`; the rest of its strong count is program references.
    // One with no reference outside the walk except its incoming walked edges is unreachable. Nodes with any
    // other incoming reference are conservatively live, and everything they point to is live too.
    let n = nodes.len();
    let mut live = vec![false; n];
    for i in 0..n {
        let outside = strong_of(&nodes[i].v).saturating_sub(1);
        if outside > nodes[i].indeg {
            live[i] = true;
        }
    }
    let mut stack: Vec<usize> = (0..n).filter(|&i| live[i]).collect();
    while let Some(i) = stack.pop() {
        for &j in &nodes[i].out {
            if !live[j] {
                live[j] = true;
                stack.push(j);
            }
        }
    }
    let mut freed = 0;
    for i in 0..n {
        if !live[i] {
            freed += 1;
            gc_clear(&nodes[i].v);
        }
    }
    drop(nodes);
    drop(at);
    freed
}
