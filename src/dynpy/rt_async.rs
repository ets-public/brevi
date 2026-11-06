// ---------- coroutines (async def, await) ----------
// A coroutine object holds its function's body, not yet run. `await c` runs that body right where it is, on the
// stack of whatever awaits it: a task's coroutine runs on its own stack (rt_gen.rs Coro, started by send()),
// so when something deep inside awaits a future that is not done, the whole task stops at that point and
// send() returns the future to the event loop, as CPython's chain of `yield from`s does. Brevi's asyncio
// (py/asyncio.py, a port of CPython's) drives tasks with send() and throw() exactly as CPython's does, so
// tasks interleave in CPython's order.

enum ACState {
    // made, not started
    Fresh(Box<dyn FnOnce() -> R>),
    // being run by an await, in place
    Inline,
    // started by send(): on its own stack
    Run(Rc<RefCell<Box<Coro>>>),
    Done,
}

pub struct ACoro {
    name: Sym,
    st: RefCell<ACState>,
}

pub fn make_acoro(name: Sym, body: impl FnOnce() -> R + 'static) -> V {
    V::Ext(Rc::new(ACoro { name, st: RefCell::new(ACState::Fresh(Box::new(body))) }))
}

fn reused() -> E {
    exc("RuntimeError", "cannot reuse already awaited coroutine".into())
}

impl ACoro {
    // send(v) / throw(e): the next value the coroutine yields (a future it waits for), or its end as
    // StopIteration(value) or the exception it raised.
    fn step(&self, sent: V, thrown: Option<E>) -> R {
        let c = {
            let mut st = self.st.borrow_mut();
            match &*st {
                ACState::Fresh(_) => {
                    if let Some(e) = thrown {
                        *st = ACState::Done;
                        return Err(e);
                    }
                    if !matches!(sent, V::None) {
                        return Err(exc("TypeError", "can't send non-None value to a just-started coroutine".into()));
                    }
                    let ACState::Fresh(body) = std::mem::replace(&mut *st, ACState::Done) else { unreachable!() };
                    let c = Rc::new(RefCell::new(Coro::new(body)));
                    *st = ACState::Run(c.clone());
                    c
                }
                ACState::Run(c) => c.clone(),
                ACState::Inline => return Err(exc("RuntimeError", "coroutine is being awaited already".into())),
                ACState::Done => {
                    return match thrown {
                        Some(e) => Err(e),
                        None => Err(reused()),
                    }
                }
            }
        };
        let r = match thrown {
            Some(e) => c.borrow_mut().throw(e),
            None => c.borrow_mut().resume(sent),
        };
        match r {
            Ok(Some(y)) => Ok(y),
            Ok(None) => {
                *self.st.borrow_mut() = ACState::Done;
                Err(stop_with(V::None))
            }
            Err(e) => {
                *self.st.borrow_mut() = ACState::Done;
                Err(e)
            }
        }
    }
    // `await c`
    fn await_(&self) -> R {
        let body = {
            let mut st = self.st.borrow_mut();
            match &*st {
                ACState::Fresh(_) => {
                    let ACState::Fresh(b) = std::mem::replace(&mut *st, ACState::Inline) else { unreachable!() };
                    b
                }
                ACState::Inline => return Err(exc("RuntimeError", "coroutine is being awaited already".into())),
                ACState::Done => return Err(reused()),
                ACState::Run(c) => {
                    let c = c.clone();
                    drop(st);
                    let r = delegate(&c);
                    *self.st.borrow_mut() = ACState::Done;
                    return r;
                }
            }
        };
        let r = body();
        *self.st.borrow_mut() = ACState::Done;
        r
    }
}

fn exc_of(v: V) -> R {
    // throw(SomeError) and throw(SomeError()) both raise an instance
    match &v {
        V::Class(_) => call(&v, Args::p(vec![])),
        _ => Ok(v),
    }
}

impl Ext for ACoro {
    fn tname(&self) -> &'static str {
        "coroutine"
    }
    fn repr(&self) -> String {
        format!("<coroutine object {} at {:#x}>", &*self.name, self as *const ACoro as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some((self as *const ACoro as usize >> 4) as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if std::ptr::eq(e.any() as *const dyn std::any::Any as *const u8, self as *const ACoro as *const u8)))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        match n {
            "__name__" | "__qualname__" => Some(Ok(st(self.name.to_string()))),
            "cr_running" => Some(Ok(V::Bool(matches!(&*self.st.borrow(), ACState::Inline) || matches!(&*self.st.borrow(), ACState::Run(c) if !c.borrow().is_suspended())))),
            "send" | "throw" | "close" => None,
            "__await__" => Some(Err(unsupported("coroutine.__await__()"))),
            _ => Some(Err(exc("AttributeError", format!("'coroutine' object has no attribute '{n}'")))),
        }
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(match n {
            "send" => match a.p.len() {
                1 => self.step(a.p[0].clone(), None),
                k => Err(exc("TypeError", format!("send() takes exactly one argument ({k} given)"))),
            },
            "throw" => match a.p.first() {
                None => Err(exc("TypeError", "throw expected at least 1 argument, got 0".into())),
                Some(v) => match exc_of(v.clone()) {
                    Ok(e) => self.step(V::None, Some(E(e))),
                    Err(e) => Err(e),
                },
            },
            "close" => {
                let run = match &*self.st.borrow() {
                    ACState::Run(c) if c.borrow().is_suspended() => Some(c.clone()),
                    _ => None,
                };
                match run {
                    None => {
                        let mut st = self.st.borrow_mut();
                        if matches!(&*st, ACState::Fresh(_)) {
                            *st = ACState::Done;
                        }
                        Ok(V::None)
                    }
                    Some(_) => match self.step(V::None, Some(exc0("GeneratorExit"))) {
                        Ok(_) => Err(exc("RuntimeError", "coroutine ignored GeneratorExit".into())),
                        Err(E(e)) if is_exc(&e, "GeneratorExit") || is_exc(&e, "StopIteration") => Ok(V::None),
                        Err(e) => Err(e),
                    },
                }
            }
            _ => return None,
        })
    }
}

// `yield from` over a generator's Coro that passes throw() through: the value it returns.
fn delegate(c: &Rc<RefCell<Box<Coro>>>) -> R {
    let mut r = c.borrow_mut().resume(V::None);
    loop {
        match r {
            Ok(Some(x)) => {
                r = match gen_yield(x) {
                    Ok(s) => c.borrow_mut().resume(s),
                    Err(e) => c.borrow_mut().throw(e),
                };
            }
            Ok(None) => return Ok(V::None),
            Err(E(e)) if is_exc(&e, "StopIteration") => return Ok(match &e {
                V::Obj(o) => o.get("value").unwrap_or(V::None),
                _ => V::None,
            }),
            Err(e) => return Err(e),
        }
    }
}

fn class_has(x: &V, n: &str) -> bool {
    match x {
        V::Obj(o) => o.cls.lookup(n).is_some(),
        _ => false,
    }
}

// `await x`
pub fn aw(x: &V) -> R {
    if let V::Ext(e) = x {
        if let Some(c) = e.any().downcast_ref::<ACoro>() {
            return c.await_();
        }
    }
    // An asyncio future (what Future.__await__ does, without making a generator for it).
    if class_has(x, "_asyncio_future_blocking") {
        if !truth(&call_method(x, "done", Args::p(vec![]))?)? {
            setattr(x, "_asyncio_future_blocking", V::Bool(true))?;
            gen_yield(x.clone())?;
            if !truth(&call_method(x, "done", Args::p(vec![]))?)? {
                return Err(exc("RuntimeError", "await wasn't used with future".into()));
            }
        }
        return call_method(x, "result", Args::p(vec![]));
    }
    if !class_has(x, "__await__") {
        return Err(exc("TypeError", format!("'{}' object can't be awaited", type_name(x))));
    }
    let it = call_method(x, "__await__", Args::p(vec![]))?;
    match &it {
        V::Iter(i) if coro_of(i).is_some() => delegate(&coro_of(i).unwrap()),
        V::Iter(_) => {
            let mut i = iter(&it)?;
            while let Some(v) = i.next()? {
                gen_yield(v)?;
            }
            Ok(V::None)
        }
        V::Ext(e) if e.any().downcast_ref::<ACoro>().is_some() => Err(exc("TypeError", "__await__() returned a coroutine".into())),
        _ => Err(exc("TypeError", format!("__await__() returned non-iterator of type '{}'", type_name(&it)))),
    }
}

pub fn is_acoro(x: &V) -> bool {
    matches!(x, V::Ext(e) if e.any().downcast_ref::<ACoro>().is_some())
}

// aiter(x), anext(x), and the special methods `async with` looks up on the type
pub fn bi_aiter(a: Args) -> R {
    let x = a1(&a, "aiter")?;
    if !class_has(&x, "__aiter__") {
        return Err(exc("TypeError", format!("'async for' requires an object with __aiter__ method, got {}", type_name(&x))));
    }
    let r = call_method(&x, "__aiter__", Args::p(vec![]))?;
    if !class_has(&r, "__anext__") {
        return Err(exc("TypeError", format!("'async for' received an object from __aiter__ that does not implement __anext__: {}", type_name(&r))));
    }
    Ok(r)
}
pub fn bi_anext(a: Args) -> R {
    let x = a1(&a, "anext")?;
    if !class_has(&x, "__anext__") {
        return Err(exc("TypeError", format!("'{}' object is not an async iterator", type_name(&x))));
    }
    call_method(&x, "__anext__", Args::p(vec![]))
}
fn async_cm(x: &V, n: &str) -> R {
    match x {
        V::Obj(o) if o.cls.lookup("__aenter__").is_some() && o.cls.lookup("__aexit__").is_some() => Ok(o.cls.lookup(n).unwrap()),
        _ => Err(exc("TypeError", format!("'{}' object does not support the asynchronous context manager protocol", type_name(x)))),
    }
}
pub fn bi___brevi_aenter(a: Args) -> R {
    async_cm(&a1(&a, "__aenter__")?, "__aenter__")
}
pub fn bi___brevi_aexit(a: Args) -> R {
    async_cm(&a1(&a, "__aexit__")?, "__aexit__")
}

// The module _brevi_async, for py/asyncio.py: suspend(v) yields v from the running task (a bare `yield` is
// suspend(None)); iscoroutine(x).
pub fn brevi_async_fn(n: &str, a: Args) -> R {
    match n {
        "suspend" => {
            let v = a.p.first().cloned().unwrap_or(V::None);
            gen_yield(v)?;
            Ok(V::None)
        }
        "iscoroutine" => Ok(V::Bool(is_acoro(&a1(&a, "iscoroutine")?))),
        "isawaitable" => {
            let x = a1(&a, "isawaitable")?;
            Ok(V::Bool(is_acoro(&x) || class_has(&x, "__await__")))
        }
        _ => Err(unsupported(&format!("_brevi_async.{n}"))),
    }
}

// ---------- BaseExceptionGroup, ExceptionGroup, except* ----------

fn is_eg(v: &V) -> bool {
    matches!(v, V::Obj(o) if o.cls.is_sub(&exc_class("BaseExceptionGroup")))
}
fn eg_excs(v: &V) -> Vec<V> {
    match v {
        V::Obj(o) => match o.get("exceptions") {
            Some(V::Tuple(t)) => t.to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

// BaseExceptionGroup(message, exceptions): CPython's checks, and an ExceptionGroup when every item is an Exception.
pub fn eg_new(c: &Rc<Class>, a: &Args) -> R {
    let base = exc_class("BaseExceptionGroup");
    let cname = c.name;
    if !a.k.is_empty() {
        return Err(exc("TypeError", format!("{cname}() takes no keyword arguments")));
    }
    if a.p.len() != 2 {
        return Err(exc("TypeError", format!("BaseExceptionGroup.__new__() takes exactly 2 arguments ({} given)", a.p.len())));
    }
    let msg = &a.p[0];
    if !matches!(msg, V::Str(_)) {
        return Err(exc("TypeError", format!("BaseExceptionGroup.__new__() argument 1 must be str, not {}", type_name(msg))));
    }
    let seq = &a.p[1];
    if !matches!(seq, V::List(_) | V::Tuple(_)) {
        return Err(exc("TypeError", "second argument (exceptions) must be a sequence".into()));
    }
    let items = to_vec(seq)?;
    if items.is_empty() {
        return Err(exc("ValueError", "second argument (exceptions) must be a non-empty sequence".into()));
    }
    let exception = exc_class("Exception");
    let mut all_exc = true;
    for (i, x) in items.iter().enumerate() {
        match x {
            V::Obj(o) if o.cls.is_sub(&exc_class("BaseException")) => all_exc &= o.cls.is_sub(&exception),
            _ => return Err(exc("ValueError", format!("Item {i} of second argument (exceptions) is not an exception"))),
        }
    }
    let cls = if std::ptr::eq(&**c, &*base) {
        if all_exc {
            exc_class("ExceptionGroup")
        } else {
            base.clone()
        }
    } else {
        if c.is_sub(&exc_class("ExceptionGroup")) && !all_exc {
            return Err(exc("TypeError", "Cannot nest BaseExceptions in an ExceptionGroup".into()));
        }
        if !c.is_sub(&exception) && all_exc {
            // a subclass of BaseExceptionGroup alone may hold Exceptions
        }
        c.clone()
    };
    let o = Obj::new(cls);
    o.set("args", tup(vec![msg.clone(), seq.clone()]));
    o.set("message", msg.clone());
    o.set("exceptions", tup(items));
    Ok(V::Obj(o))
}

fn eg_str(a: Args) -> R {
    let me = a1(&a, "__str__")?;
    let n = eg_excs(&me).len();
    let m = match &me {
        V::Obj(o) => o.get("message").map(|m| to_str(&m)).transpose()?.unwrap_or_default(),
        _ => String::new(),
    };
    Ok(st(format!("{m} ({n} sub-exception{})", if n == 1 { "" } else { "s" })))
}

// What split()/subgroup() take: an exception type, a tuple of them, or a function of the exception.
fn eg_matcher(cond: &V) -> Result<Box<dyn Fn(&V) -> Result<bool, E>>, E> {
    let is_exc_type = |c: &V| matches!(c, V::Class(k) if k.is_sub(&exc_class("BaseException")));
    match cond {
        V::Class(_) if is_exc_type(cond) => {
            let c = cond.clone();
            Ok(Box::new(move |x| isinstance(x, &c)))
        }
        V::Tuple(t) if t.iter().all(is_exc_type) => {
            let c = cond.clone();
            Ok(Box::new(move |x| isinstance(x, &c)))
        }
        V::Func(_) | V::Bound(_) | V::BMeth(_) => {
            let c = cond.clone();
            Ok(Box::new(move |x| truth(&call(&c, Args::p(vec![x.clone()]))?)))
        }
        _ => Err(exc("TypeError", "expected an exception type, a tuple of exception types, or a callable (other than a class)".into())),
    }
}

// The parts of eg that match and that do not, each a group made by eg.derive() keeping eg's nesting, or None.
fn eg_split_by(eg: &V, m: &dyn Fn(&V) -> Result<bool, E>, want_rest: bool) -> Result<(Option<V>, Option<V>), E> {
    if m(eg)? {
        return Ok((Some(eg.clone()), None));
    }
    let mut yes = vec![];
    let mut no = vec![];
    for x in eg_excs(eg) {
        if is_eg(&x) {
            let (a, b) = eg_split_by(&x, m, want_rest)?;
            yes.extend(a);
            no.extend(b);
        } else if m(&x)? {
            yes.push(x);
        } else if want_rest {
            no.push(x);
        }
    }
    let derive = |v: Vec<V>| -> Result<Option<V>, E> {
        if v.is_empty() {
            return Ok(None);
        }
        let g = call_method(eg, "derive", Args::p(vec![list(v)]))?;
        if let (V::Obj(src), V::Obj(dst)) = (eg, &g) {
            for k in ["__cause__", "__context__", "__traceback__", "__notes__"] {
                if let Some(v) = src.get(k) {
                    dst.set(k, v);
                }
            }
        }
        Ok(Some(g))
    };
    Ok((derive(yes)?, if want_rest { derive(no)? } else { None }))
}

fn eg_methods(n: &str, a: Args) -> R {
    let me = a.p.first().cloned().unwrap_or(V::None);
    match n {
        "derive" => {
            let excs = a.p.get(1).cloned().ok_or_else(|| exc("TypeError", "derive() takes exactly one argument (0 given)".into()))?;
            let msg = match &me {
                V::Obj(o) => o.get("message").unwrap_or(s("")),
                _ => s(""),
            };
            instantiate(&exc_class("BaseExceptionGroup"), Args::p(vec![msg, excs]))
        }
        "split" | "subgroup" => {
            let cond = a.p.get(1).cloned().ok_or_else(|| exc("TypeError", format!("BaseExceptionGroup.{n}() takes exactly one argument (0 given)")))?;
            let m = eg_matcher(&cond)?;
            let (yes, no) = eg_split_by(&me, &*m, n == "split")?;
            Ok(if n == "split" { tup(vec![yes.unwrap_or(V::None), no.unwrap_or(V::None)]) } else { yes.unwrap_or(V::None) })
        }
        _ => Err(unsupported(&format!("BaseExceptionGroup.{n}"))),
    }
}

pub fn eg_class_setup(base: &Rc<Class>) {
    base.set("__str__", builtin("__str__", eg_str));
    for n in ["derive", "split", "subgroup"] {
        base.set(n, builtin(n, move |a: Args| eg_methods(n, a)));
    }
}

// except* T: (the part of exc that T matches, as a group, and the rest), for the parser's rewrite of except*.
pub fn bi___brevi_eg_split(a: Args) -> R {
    a.want("__brevi_eg_split", 2, 2)?;
    let (x, t) = (&a.p[0], &a.p[1]);
    if matches!(x, V::None) {
        return Ok(tup(vec![V::None, V::None]));
    }
    let bad = |c: &V| matches!(c, V::Class(k) if k.is_sub(&exc_class("BaseExceptionGroup")));
    if bad(t) || matches!(t, V::Tuple(v) if v.iter().any(bad)) {
        return Err(exc("TypeError", "catching ExceptionGroup with except* is not allowed. Use except instead.".into()));
    }
    let is_type = |c: &V| matches!(c, V::Class(k) if k.is_sub(&exc_class("BaseException")));
    if !(is_type(t) || matches!(t, V::Tuple(v) if v.iter().all(is_type))) {
        return Err(exc("TypeError", "catching classes that do not inherit from BaseException is not allowed".into()));
    }
    if !is_eg(x) {
        // a plain exception is matched as a group of one
        if isinstance(x, t)? {
            let g = instantiate(&exc_class("BaseExceptionGroup"), Args::p(vec![s(""), tup(vec![x.clone()])]))?;
            return Ok(tup(vec![g, V::None]));
        }
        return Ok(tup(vec![V::None, x.clone()]));
    }
    let tt = t.clone();
    let (yes, no) = eg_split_by(x, &move |v: &V| isinstance(v, &tt), true)?;
    Ok(tup(vec![yes.unwrap_or(V::None), no.unwrap_or(V::None)]))
}

fn eg_leaves(v: &V, out: &mut Vec<*const Obj>) {
    if is_eg(v) {
        for x in eg_excs(v) {
            eg_leaves(&x, out);
        }
    } else if let V::Obj(o) = v {
        out.push(Rc::as_ptr(o));
    }
}

// After the except* handlers: what to raise (None for nothing). `raised` holds what the handlers raised; those
// that are the groups handed to them (a bare `raise`) or the unmatched rest go back into the original group's
// shape, as CPython's _PyExc_PrepReraiseStar does.
pub fn bi___brevi_eg_reraise(a: Args) -> R {
    a.want("__brevi_eg_reraise", 4, 4)?;
    let (orig, rest, raised, given) = (&a.p[0], &a.p[1], to_vec(&a.p[2])?, to_vec(&a.p[3])?);
    if !is_eg(orig) {
        // a plain exception: at most one handler ran
        if let Some(r) = raised.into_iter().next() {
            return Err(E(r));
        }
        if !matches!(rest, V::None) {
            return Err(E(rest.clone()));
        }
        return Ok(V::None);
    }
    let same = |x: &V, y: &V| matches!((x, y), (V::Obj(p), V::Obj(q)) if Rc::ptr_eq(p, q));
    let mut keep: Vec<V> = vec![];
    let mut new: Vec<V> = vec![];
    for r in raised {
        if given.iter().any(|g| same(g, &r)) {
            keep.push(r);
        } else {
            new.push(r);
        }
    }
    if !matches!(rest, V::None) {
        keep.push(rest.clone());
    }
    let mut leaves = vec![];
    for k in &keep {
        eg_leaves(k, &mut leaves);
    }
    let kept = if leaves.is_empty() {
        None
    } else {
        eg_split_by(orig, &move |v: &V| Ok(!is_eg(v) && matches!(v, V::Obj(o) if leaves.contains(&Rc::as_ptr(o)))), false)?.0
    };
    if new.is_empty() {
        return match kept {
            Some(k) => Err(E(k)),
            None => Ok(V::None),
        };
    }
    new.extend(kept);
    if new.len() == 1 {
        return Err(E(new.pop().unwrap()));
    }
    Err(E(instantiate(&exc_class("BaseExceptionGroup"), Args::p(vec![s(""), list(new)]))?))
}
