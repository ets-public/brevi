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
