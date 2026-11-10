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
    // "coroutine", or what an async generator's asend()/athrow() return
    kind: &'static str,
    st: RefCell<ACState>,
    // asend()/athrow(): its async generator (close() before it runs closes the generator)
    agen: Option<V>,
}

pub fn make_acoro(name: Sym, body: impl FnOnce() -> R + 'static) -> V {
    V::Ext(ExtRc::new(ACoro { name, kind: "coroutine", st: RefCell::new(ACState::Fresh(Box::new(body))), agen: None }))
}

impl ACoro {
    fn reused(&self) -> E {
        exc("RuntimeError", match self.kind {
            "async_generator_asend" => "cannot reuse already awaited __anext__()/asend()".into(),
            "async_generator_athrow" => "cannot reuse already awaited aclose()/athrow()".into(),
            _ => "cannot reuse already awaited coroutine".into(),
        })
    }
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
                    let c = Rc::new(RefCell::new(Coro::new(self.name, body)));
                    *st = ACState::Run(c.clone());
                    c
                }
                ACState::Run(c) => c.clone(),
                ACState::Inline => return Err(exc("RuntimeError", "coroutine is being awaited already".into())),
                ACState::Done => {
                    return match thrown {
                        Some(e) => Err(e),
                        None => Err(self.reused()),
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
                ACState::Done => return Err(self.reused()),
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
        self.kind
    }
    fn repr(&self) -> String {
        if self.kind != "coroutine" {
            return format!("<{} object at {:#x}>", self.kind, self as *const ACoro as usize);
        }
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
                            drop(st);
                            if let Some(g) = self.agen.as_ref().and_then(ag_of) {
                                g.close_now();
                            }
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
        V::Ext(e) => e.any().downcast_ref::<AGen>().is_some() && matches!(n, "__aiter__" | "__anext__"),
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
    if ag_of(&x).is_some() {
        return Ok(x);
    }
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
    if a.p.len() == 2 {
        return Err(unsupported("anext() with a default"));
    }
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
        "isasyncgen" => Ok(V::Bool(ag_of(&a1(&a, "isasyncgen")?).is_some())),
        "isawaitable" => {
            let x = a1(&a, "isawaitable")?;
            Ok(V::Bool(is_acoro(&x) || class_has(&x, "__await__")))
        }
        "set_hooks" | "track" | "untrack" | "tracked" => ag_hook_fn(n, &a),
        "set_task_hooks" => {
            a.want("set_task_hooks", 9, 9)?;
            let mut p = a.p.iter();
            let (reg, unreg) = (p.next().unwrap().clone(), p.next().unwrap().clone());
            ASYNC_UNREGISTER.with(|s| *s.borrow_mut() = Some(unreg));
            ASYNC_REGISTER.with(|s| *s.borrow_mut() = Some(reg));
            ASYNC_NEXTNAME.with(|s| *s.borrow_mut() = Some(p.next().unwrap().clone()));
            let take_class = |v: &V| -> Option<Rc<Class>> { match v { V::Class(c) => Some(c.clone()), _ => None } };
            let (hcls, lcls, fcls, tcls) = (take_class(p.next().unwrap()), take_class(p.next().unwrap()), take_class(p.next().unwrap()), take_class(p.next().unwrap()));
            ASYNC_CANCELLED.with(|s| *s.borrow_mut() = take_class(p.next().unwrap()));
            ASYNC_INVALID.with(|s| *s.borrow_mut() = take_class(p.next().unwrap()));
            // The port's own methods, captured now (before any user code could replace them) so the native
            // fast paths compare against the originals and an override falls back to the Python method.
            let take_func = |c: &Option<Rc<Class>>, n: &str| -> Option<Rc<Func>> { c.as_ref().and_then(|c| match c.lookup(n) { Some(V::Func(f)) => Some(f), _ => None }) };
            ASYNC_HANDLE.with(|s| *s.borrow_mut() = hcls.clone());
            ASYNC_LOOP.with(|s| *s.borrow_mut() = lcls.clone());
            ASYNC_FUTURE.with(|s| *s.borrow_mut() = fcls.clone());
            ASYNC_TASK.with(|s| *s.borrow_mut() = tcls.clone());
            ASYNC_CALLSOON.with(|s| *s.borrow_mut() = take_func(&lcls, "_call_soon"));
            ASYNC_CREATETASK.with(|s| *s.borrow_mut() = take_func(&lcls, "create_task"));
            ASYNC_STEPF.with(|s| *s.borrow_mut() = take_func(&tcls, "_step"));
            ASYNC_WAKEUP.with(|s| *s.borrow_mut() = take_func(&tcls, "_wakeup"));
            ASYNC_SCHEDCB.with(|s| *s.borrow_mut() = take_func(&fcls, "_schedule_callbacks"));
            ASYNC_RUNF.with(|s| *s.borrow_mut() = take_func(&hcls, "_run"));
            Ok(V::None)
        }
        "make_handle" => {
            a.want("make_handle", 5, 5)?;
            let cls = a.p[0].clone();
            let Some(h) = handle_new(cls, a.p[1].clone(), a.p[2].clone(), a.p[3].clone(), a.p[4].clone()) else {
                return Err(unsupported("_brevi_async.make_handle without asyncio"));
            };
            Ok(h)
        }
        "run_ready" => {
            a.want("run_ready", 2, 2)?;
            let loop_ = a.p[0].clone();
            let n = match &a.p[1] {
                V::Int(i) => *i as usize,
                _ => return Err(exc("TypeError", "run_ready takes an int".into())),
            };
            run_ready(&loop_, n)
        }
        "task_step" => {
            a.want("task_step", 1, 2)?;
            let excv = match a.p.get(1).cloned() {
                Some(e) if !matches!(&e, V::None) => Some(e),
                _ => None,
            };
            task_step(&a.p[0], excv)
        }
        "task_wakeup" => {
            a.want("task_wakeup", 2, 2)?;
            task_wakeup(&a.p[0], &a.p[1])
        }
        "task_init" => {
            a.want("task_init", 6, 6)?;
            Ok(V::Bool(task_init(&a.p[0], &a.p[1], &a.p[2], &a.p[3], &a.p[4], &a.p[5])?))
        }
        "set_gather_class" => {
            a.want("set_gather_class", 1, 1)?;
            ASYNC_GATHER.with(|s| *s.borrow_mut() = match &a.p[0] { V::Class(c) => Some(c.clone()), _ => None });
            Ok(V::None)
        }
        "gather" => {
            a.want("gather", 6, 6)?;
            native_gather(&a.p[0], &a.p[1], &a.p[2], &a.p[3], &a.p[4], &a.p[5])
        }
        "schedule_callbacks" => schedule_callbacks(&a1(&a, "schedule_callbacks")?),
        "fut_set_result" => {
            a.want("fut_set_result", 2, 2)?;
            fut_set_result(&a.p[0], a.p[1].clone())
        }
        "fut_set_exception" => {
            a.want("fut_set_exception", 2, 2)?;
            fut_set_exception(&a.p[0], a.p[1].clone())
        }
        _ => Err(unsupported(&format!("_brevi_async.{n}"))),
    }
}

// ---------- native asyncio core (Task._step, the ready queue, Future scheduling) ----------
// py/asyncio.py's Task._step, Handle._run and Future._schedule_callbacks are the hot path of every task
// switch; these native forms do the same work without the V-level method dispatch and per-step allocations
// of the Python port. The logic is CPython's, line for line; anything a subclass overrides still runs
// through the Python method (the Func identity checks below).

thread_local! {
    static ASYNC_HANDLE: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_TASK: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_LOOP: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_FUTURE: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_CANCELLED: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_INVALID: RefCell<Option<Rc<Class>>> = RefCell::new(None);
    static ASYNC_CALLSOON: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_CREATETASK: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_STEPF: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_WAKEUP: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_SCHEDCB: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_RUNF: RefCell<Option<Rc<Func>>> = RefCell::new(None);
    static ASYNC_UNREGISTER: RefCell<Option<V>> = RefCell::new(None);
    static ASYNC_REGISTER: RefCell<Option<V>> = RefCell::new(None);
    static ASYNC_NEXTNAME: RefCell<Option<V>> = RefCell::new(None);
    static ASYNC_GATHER: RefCell<Option<Rc<Class>>> = RefCell::new(None);
}
fn async_class(slot: &'static std::thread::LocalKey<RefCell<Option<Rc<Class>>>>) -> Option<Rc<Class>> {
    slot.with(|s| s.borrow().clone())
}
fn async_func(slot: &'static std::thread::LocalKey<RefCell<Option<Rc<Func>>>>, cls: &Rc<Class>, name: &str) -> Option<Rc<Func>> {
    slot.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = match cls.lookup(name) {
                Some(V::Func(f)) => Some(f),
                _ => None,
            };
        }
        s.clone()
    })
}
fn async_unregister() -> Option<V> {
    ASYNC_UNREGISTER.with(|s| s.borrow().clone())
}
// an exception class py/asyncio.py defines (InvalidStateError): not in BUILTIN_CLASSES
fn asyncio_exc(name: &str, msg: String) -> E {
    let cls = match name {
        "InvalidStateError" => async_class(&ASYNC_INVALID),
        "CancelledError" => async_class(&ASYNC_CANCELLED),
        _ => None,
    };
    match cls {
        Some(c) => {
            let o = Obj::new(c);
            o.set("args", tup(vec![st(msg)]));
            E(V::Obj(o))
        }
        None => exc("RuntimeError", msg),
    }
}
fn is_cancelled(e: &V) -> bool {
    match e {
        V::Obj(o) => async_class(&ASYNC_CANCELLED).is_some_and(|c| o.cls.is_sub(&c)),
        _ => false,
    }
}
// the task's _state, which sits on Future's class until the future finishes
fn future_pending(o: &Obj) -> bool {
    match o.get("_state") {
        Some(v) => matches!(&v, V::Str(s) if s.s == "PENDING"),
        None => matches!(&o.cls.lookup("_state").unwrap_or(V::None), V::Str(s) if s.s == "PENDING"),
    }
}
// o.name if the object (or its class) has it, None otherwise; getattr without the AttributeError
fn opt_attr(x: &V, n: &str) -> V {
    match x {
        V::Obj(o) => match o.get(n) {
            Some(v) => v,
            None => o.cls.lookup(n).unwrap_or(V::None),
        },
        _ => getattr(x, n).ok().unwrap_or(V::None),
    }
}
fn si_value(e: &V) -> V {
    match e {
        V::Obj(o) => o.get("value").unwrap_or(V::None),
        _ => V::None,
    }
}
fn first_of(args: &V) -> Option<V> {
    match args {
        V::Tuple(t) => t.first().cloned(),
        _ => None,
    }
}

// Handle(callback, args, loop, context), made natively: a ready-queue handle whose callback is a Task's
// _step or _wakeup (not overridden) is marked so run_ready steps the task without method dispatch.
fn handle_new(cls: V, callback: V, args: V, loop_: V, context: V) -> Option<V> {
    let V::Class(c) = cls else { return None };
    let o = Obj::new(c);
    o.set("_context", context);
    o.set("_loop", loop_);
    o.set("_callback", callback.clone());
    o.set("_args", args.clone());
    o.set("_cancelled", V::Bool(false));
    if let V::Bound(b) = &callback {
        if let V::Func(f) = &b.1 {
            if let V::Obj(t) = &b.0 {
                if let Some(tc) = async_class(&ASYNC_TASK) {
                    if t.cls.is_sub(&tc) {
                        let mk = if f.name.get() == "_step" && async_func(&ASYNC_STEPF, &tc, "_step").is_some_and(|wf| Rc::ptr_eq(f, &wf)) {
                            match first_of(&args) {
                                Some(e) => Some(("_brevi_step", tup(vec![b.0.clone(), e]))),
                                None => Some(("_brevi_step", b.0.clone())),
                            }
                        } else if f.name.get() == "_wakeup" && async_func(&ASYNC_WAKEUP, &tc, "_wakeup").is_some_and(|wf| Rc::ptr_eq(f, &wf)) {
                            Some(("_brevi_wake", tup(vec![b.0.clone(), first_of(&args).unwrap_or(V::None)])))
                        } else {
                            None
                        };
                        if let Some((k, v)) = mk {
                            o.set(k, v);
                        }
                    }
                }
            }
        }
    }
    Some(V::Obj(o))
}
fn ready_push(loop_: &V, h: V) -> R {
    let ready = getattr(loop_, "_ready")?;
    if let V::Deque(d) = &ready {
        let mut d = d.borrow_mut();
        if d.max.is_none() {
            d.v.push_back(h);
            return Ok(V::None);
        }
    }
    call_method(&ready, "append", Args::p(vec![h]))
}
fn ready_pop(loop_: &V) -> Option<V> {
    let ready = getattr(loop_, "_ready").ok()?;
    if let V::Deque(d) = &ready {
        let mut d = d.borrow_mut();
        if d.max.is_none() {
            return d.v.pop_front();
        }
    }
    call_method(&ready, "popleft", Args::p(vec![])).ok()
}
fn loop_plain_call_soon(loop_: &V) -> bool {
    let V::Obj(o) = loop_ else { return false };
    let Some(bel) = async_class(&ASYNC_LOOP) else { return false };
    if !o.cls.is_sub(&bel) {
        return false;
    }
    let Some(f) = async_func(&ASYNC_CALLSOON, &bel, "_call_soon") else { return false };
    matches!(o.cls.lookup("_call_soon"), Some(V::Func(g)) if Rc::ptr_eq(&g, &f))
}
// loop._call_soon(cb, args, context): through the loop's own method when it is overridden, natively otherwise
fn push_callback(loop_: &V, cb: V, args: V, context: V) -> R {
    if loop_plain_call_soon(loop_) {
        let Some(h) = async_class(&ASYNC_HANDLE).map(V::Class).and_then(|c| handle_new(c, cb, args, loop_.clone(), context)) else {
            return Err(unsupported("_brevi_async without asyncio"));
        };
        return ready_push(loop_, h);
    }
    call_method(loop_, "_call_soon", Args::p(vec![cb, args, context]))
}
// Task.__step scheduled: _call_soon (result None) or call_soon (an error to throw in)
fn push_step(loop_: &V, task: V, exc: Option<V>) -> R {
    let cb = getattr(&task, "_step")?;
    let args = match &exc {
        Some(e) => tup(vec![e.clone()]),
        None => tup(vec![]),
    };
    let context = opt_attr(&task, "_context");
    if loop_plain_call_soon(loop_) {
        let Some(h) = async_class(&ASYNC_HANDLE).map(V::Class).and_then(|c| handle_new(c, cb, args, loop_.clone(), context)) else {
            return Err(unsupported("_brevi_async without asyncio"));
        };
        return ready_push(loop_, h);
    }
    match exc {
        Some(e) => call_method(loop_, "call_soon", Args { p: vec![cb, e], k: vec![("context", context)] }),
        None => call_method(loop_, "_call_soon", Args::p(vec![cb, args, context])),
    }
}
fn run_handle_error(loop_: &V, handle: &V, e: &V) -> R {
    // Handle._run: SystemExit and KeyboardInterrupt are re-raised, anything else goes to the loop's handler
    if is_exc(e, "SystemExit") || is_exc(e, "KeyboardInterrupt") {
        return Err(E(e.clone()));
    }
    let d = Rc::new(RefCell::new(Dict::new()));
    d.borrow_mut().set(s("message"), s("Exception in callback"))?;
    d.borrow_mut().set(s("exception"), e.clone())?;
    d.borrow_mut().set(s("handle"), handle.clone())?;
    call_method(loop_, "call_exception_handler", Args::p(vec![V::Dict(d)]))?;
    Ok(V::None)
}
// The port's Handle._run, without the Python frame: callback(*args) with Handle._run's exception handling.
// A subclass that overrides _run still runs through the Python method (the Func identity check).
fn run_plain_handle(loop_: &V, handle: &V) -> R {
    let V::Obj(h) = handle else { return call_method(handle, "_run", Args::p(vec![])) };
    let Some(hl) = async_class(&ASYNC_HANDLE) else { return call_method(handle, "_run", Args::p(vec![])) };
    if !h.cls.is_sub(&hl) {
        return call_method(handle, "_run", Args::p(vec![]));
    }
    let Some(f) = async_func(&ASYNC_RUNF, &hl, "_run") else { return call_method(handle, "_run", Args::p(vec![])) };
    if !matches!(h.cls.lookup("_run"), Some(V::Func(g)) if Rc::ptr_eq(&g, &f)) {
        return call_method(handle, "_run", Args::p(vec![]));
    }
    let Some(cb) = h.get("_callback") else { return call_method(handle, "_run", Args::p(vec![])) };
    let Some(args) = h.get("_args") else { return call_method(handle, "_run", Args::p(vec![])) };
    let r = match args {
        V::Tuple(t) if t.is_empty() => call0(&cb),
        V::Tuple(t) if t.len() == 1 => call1(&cb, t[0].clone()),
        V::Tuple(t) => call(&cb, Args::p(t.to_vec())),
        _ => return call_method(handle, "_run", Args::p(vec![])),
    };
    match r {
        Err(E(e)) => run_handle_error(loop_, handle, &e),
        Ok(_) => Ok(V::None),
    }
}
pub fn run_ready(loop_: &V, n: usize) -> R {
    for _ in 0..n {
        let Some(handle) = ready_pop(loop_) else { break };
        if truth(&opt_attr(&handle, "_cancelled"))? {
            continue;
        }
        if let V::Obj(h) = &handle {
            if let Some(mark) = h.get("_brevi_step") {
                let (task, exc) = match &mark {
                    V::Tuple(t) => (t.first().cloned().unwrap_or(V::None), t.get(1).cloned()),
                    _ => (mark.clone(), None),
                };
                if let Err(E(e)) = task_step(&task, exc) {
                    run_handle_error(loop_, &handle, &e)?;
                }
                continue;
            }
            if let Some(mark) = h.get("_brevi_wake") {
                let (task, fut) = match &mark {
                    V::Tuple(t) => (t.first().cloned().unwrap_or(V::None), t.get(1).cloned().unwrap_or(V::None)),
                    _ => (mark, V::None),
                };
                if let Err(E(e)) = task_wakeup(&task, &fut) {
                    run_handle_error(loop_, &handle, &e)?;
                }
                continue;
            }
        }
        run_plain_handle(loop_, &handle)?;
    }
    Ok(V::None)
}

// Future._schedule_callbacks: callbacks already run are removed first, each is _call_soon'd with (self,).
pub fn schedule_callbacks(fut: &V) -> R {
    let V::Obj(o) = fut else { return call_method(fut, "_schedule_callbacks", Args::p(vec![])) };
    let Some(cbs) = o.get("_callbacks") else { return call_method(fut, "_schedule_callbacks", Args::p(vec![])) };
    let V::List(l) = &cbs else { return call_method(fut, "_schedule_callbacks", Args::p(vec![])) };
    let callbacks = {
        let mut l = l.borrow_mut();
        std::mem::take(&mut *l)
    };
    if callbacks.is_empty() {
        return Ok(V::None);
    }
    let loop_ = opt_attr(fut, "_loop");
    for x in callbacks {
        let (cb, ctx) = match x {
            V::Tuple(t) => (t.first().cloned().unwrap_or(V::None), t.get(1).cloned().unwrap_or(V::None)),
            _ => (V::None, V::None),
        };
        push_callback(&loop_, cb, tup(vec![fut.clone()]), ctx)?;
    }
    Ok(V::None)
}
// self._schedule_callbacks(), the virtual call the Future methods make (a subclass may override it)
fn sched_virtual(fut: &V) -> R {
    if let V::Obj(o) = fut {
        if let Some(fc) = async_class(&ASYNC_FUTURE) {
            if o.cls.is_sub(&fc) {
                if let Some(f) = async_func(&ASYNC_SCHEDCB, &fc, "_schedule_callbacks") {
                    if matches!(o.cls.lookup("_schedule_callbacks"), Some(V::Func(g)) if Rc::ptr_eq(&g, &f)) {
                        return schedule_callbacks(fut);
                    }
                }
            }
        }
    }
    call_method(fut, "_schedule_callbacks", Args::p(vec![]))
}
fn invalid_state(o: &Obj, fut: &V) -> E {
    let state = o.get("_state").map(|s| to_str(&s).unwrap_or_default()).unwrap_or_default();
    asyncio_exc("InvalidStateError", format!("{state}: {}", repr(fut).unwrap_or_default()))
}
pub fn fut_set_result(fut: &V, result: V) -> R {
    let V::Obj(o) = fut else { return call_method(fut, "set_result", Args::p(vec![result])) };
    if !future_pending(o) {
        return Err(invalid_state(o, fut));
    }
    o.set("_result", result);
    o.set("_state", s("FINISHED"));
    sched_virtual(fut)
}
pub fn fut_set_exception(fut: &V, exception: V) -> R {
    let V::Obj(o) = fut else { return call_method(fut, "set_exception", Args::p(vec![exception])) };
    if !future_pending(o) {
        return Err(invalid_state(o, fut));
    }
    let mut exception = exception;
    if matches!(&exception, V::Class(_)) {
        exception = call(&exception, Args::p(vec![]))?;
    }
    if is_exc(&exception, "StopIteration") {
        exception = exc_v("RuntimeError", s("StopIteration interacts badly with generators and cannot be raised into a Future")).0;
    }
    o.set("_exception", exception);
    o.set("_state", s("FINISHED"));
    sched_virtual(fut)?;
    o.set("_log_traceback", V::Bool(true));
    Ok(V::None)
}
// Future.cancel(f, msg=...): what Future.cancel(self) does when the task step calls it on the class
fn fut_cancel_body(fut: &V, msg: V) -> R {
    let V::Obj(o) = fut else { return call_method(fut, "cancel", Args::p(vec![msg])) };
    o.set("_log_traceback", V::Bool(false));
    if !future_pending(o) {
        return Ok(V::Bool(false));
    }
    o.set("_state", s("CANCELLED"));
    o.set("_cancel_message", msg);
    sched_virtual(fut)?;
    Ok(V::Bool(true))
}
fn make_cancelled_error(task: &V) -> V {
    if let V::Obj(o) = task {
        if let Some(e) = o.get("_cancelled_exc") {
            o.set("_cancelled_exc", V::None);
            return e;
        }
    }
    let cls = async_class(&ASYNC_CANCELLED);
    match opt_attr(task, "_cancel_message") {
        V::None => call(&V::Class(cls.unwrap_or_else(|| exc_class("BaseException"))), Args::p(vec![])).unwrap_or_else(|e| e.0),
        m => call(&V::Class(cls.unwrap_or_else(|| exc_class("BaseException"))), Args::p(vec![m])).unwrap_or_else(|e| e.0),
    }
}
fn unregister_task(task: &V) -> R {
    let Some(f) = async_unregister() else { return Ok(V::None) };
    call(&f, Args::p(vec![task.clone()]))?;
    Ok(V::None)
}
// Task.__init__(coro, loop=..., name=..., context=...), natively when nothing the Python __init__ would do
// differently is in play (a subclass, an explicit name, a closed plain loop, ...): the same attribute layout
// and the same _call_soon/register calls, minus the Python method frame and its per-attribute setattr.
fn task_init(me: &V, coro: &V, loop_: &V, name: &V, context: &V, eager: &V) -> Result<bool, E> {
    let V::Obj(o) = me else { return Ok(false) };
    let Some(tc) = async_class(&ASYNC_TASK) else { return Ok(false) };
    if !Rc::ptr_eq(&o.cls, &tc) || !is_acoro(coro) {
        return Ok(false);
    }
    if matches!(loop_, V::None) || !matches!(name, V::None) || truth(eager)? {
        return Ok(false);
    }
    if loop_plain_call_soon(loop_) && truth(&opt_attr(loop_, "_closed"))? {
        return Ok(false);
    }
    let Some(nf) = ASYNC_NEXTNAME.with(|s| s.borrow().clone()) else { return Ok(false) };
    let name_v = call(&nf, Args::p(vec![]))?;
    o.set("_loop", loop_.clone());
    o.set("_callbacks", list(vec![]));
    o.set("_name", name_v);
    o.set("_num_cancels_requested", V::Int(0));
    o.set("_must_cancel", V::Bool(false));
    o.set("_fut_waiter", V::None);
    o.set("_coro", coro.clone());
    o.set("_context", context.clone());
    push_callback(loop_, getattr(me, "_step")?, tup(vec![]), context.clone())?;
    if let Some(r) = ASYNC_REGISTER.with(|s| s.borrow().clone()) {
        call(&r, Args::p(vec![me.clone()]))?;
    }
    Ok(true)
}
// ---------- native gather ----------
// asyncio.gather(*coros, ...) for the plain case: every argument a coroutine, return_exceptions falsy, and the
// globals the Python gather uses still the port's own. The same tasks, the same outer _GatheringFuture and the
// same _done_callback logic as the Python gather, natively. Anything else returns None so the Python gather runs.
struct GatherState {
    nfuts: usize,
    nfinished: Cell<usize>,
    outer: V,
    children: V, // the list _GatheringFuture._children and the callback share, as in Python
}
fn fut_state_is(f: &V, want: &str) -> bool {
    match f {
        V::Obj(o) => {
            let st = o.get("_state").unwrap_or_else(|| o.cls.lookup("_state").unwrap_or(V::None));
            matches!(&st, V::Str(s) if s.s == want)
        }
        _ => false,
    }
}
fn fut_cancelled(f: &V) -> bool {
    fut_state_is(f, "CANCELLED")
}
fn fut_pending(f: &V) -> bool {
    fut_state_is(f, "PENDING")
}
// Future.exception() on a plain finished future: clears _log_traceback, returns _exception
fn fut_exception_value(f: &V) -> R {
    match f {
        V::Obj(o) => {
            o.set("_log_traceback", V::Bool(false));
            Ok(o.get("_exception").unwrap_or(V::None))
        }
        _ => call_method(f, "exception", Args::p(vec![])),
    }
}
fn fut_result_value(f: &V) -> R {
    match f {
        V::Obj(o) => {
            o.set("_log_traceback", V::Bool(false));
            Ok(o.get("_result").unwrap_or(V::None))
        }
        _ => call_method(f, "result", Args::p(vec![])),
    }
}
// gather's _done_callback with return_exceptions False, as the Python one is written
fn gather_done(st: &GatherState, fut: &V) -> R {
    st.nfinished.set(st.nfinished.get() + 1);
    if !fut_pending(&st.outer) {
        if !fut_cancelled(fut) {
            fut_exception_value(fut)?;
        }
        return Ok(V::None);
    }
    if fut_cancelled(fut) {
        let exc = make_cancelled_error(fut);
        return fut_set_exception(&st.outer, exc);
    }
    let exc = fut_exception_value(fut)?;
    if !matches!(exc, V::None) {
        return fut_set_exception(&st.outer, exc);
    }
    if st.nfinished.get() == st.nfuts {
        let children: Vec<V> = match &st.children {
            V::List(l) => l.borrow().iter().cloned().collect(),
            _ => vec![],
        };
        let mut results = Vec::with_capacity(children.len());
        for f in &children {
            if fut_cancelled(f) {
                let m = match opt_attr(f, "_cancel_message") {
                    V::None => s(""),
                    m => m,
                };
                let Some(c) = async_class(&ASYNC_CANCELLED) else { return Err(unsupported("_brevi_async.gather without asyncio")) };
                results.push(call(&V::Class(c), Args::p(vec![m]))?);
            } else {
                let e = fut_exception_value(f)?;
                results.push(if matches!(e, V::None) { fut_result_value(f)? } else { e });
            }
        }
        if truth(&opt_attr(&st.outer, "_cancel_requested"))? {
            let exc = make_cancelled_error(fut);
            fut_set_exception(&st.outer, exc)?;
        } else {
            fut_set_result(&st.outer, list(results))?;
        }
    }
    Ok(V::None)
}
pub fn native_gather(args: &V, ret_exc: &V, task_cls: &V, cancelled_cls: &V, gather_cls: &V, getloop: &V) -> R {
    let V::Tuple(t) = args else { return Ok(V::None) };
    if t.is_empty() || truth(ret_exc)? || !t.iter().all(is_acoro) {
        return Ok(V::None);
    }
    let Some(tc) = async_class(&ASYNC_TASK) else { return Ok(V::None) };
    if !matches!(task_cls, V::Class(c) if Rc::ptr_eq(c, &tc)) {
        return Ok(V::None);
    }
    let Some(cc) = async_class(&ASYNC_CANCELLED) else { return Ok(V::None) };
    if !matches!(cancelled_cls, V::Class(c) if Rc::ptr_eq(c, &cc)) {
        return Ok(V::None);
    }
    let Some(gc) = async_class(&ASYNC_GATHER) else { return Ok(V::None) };
    if !matches!(gather_cls, V::Class(c) if Rc::ptr_eq(c, &gc)) {
        return Ok(V::None);
    }
    let loop_ = call(getloop, Args::p(vec![]))?;
    if !loop_plain_call_soon(&loop_) {
        return Ok(V::None);
    }
    let Some(bel) = async_class(&ASYNC_LOOP) else { return Ok(V::None) };
    let Some(cf) = async_func(&ASYNC_CREATETASK, &bel, "create_task") else { return Ok(V::None) };
    if !matches!(bel.lookup("create_task"), Some(V::Func(g)) if Rc::ptr_eq(&g, &cf)) {
        return Ok(V::None);
    }
    if !matches!(opt_attr(&loop_, "_task_factory"), V::None) {
        return Ok(V::None);
    }
    if truth(&opt_attr(&loop_, "_closed"))? {
        return Err(exc("RuntimeError", "Event loop is closed".into()));
    }
    // ensure_future for each coroutine, deduplicated as arg_to_fut's dict is (coroutines key by identity)
    let mut children: Vec<V> = Vec::with_capacity(t.len());
    let mut uniq: Vec<(usize, V)> = Vec::new();
    let mut nfuts = 0usize;
    for arg in t.iter() {
        let key = match arg {
            V::Ext(e) => ExtRc::as_ptr(e) as usize,
            _ => return Ok(V::None),
        };
        let fut = match uniq.iter().find(|(k, _)| *k == key) {
            Some((_, f)) => f.clone(),
            None => {
                let me = V::Obj(Obj::new(tc.clone()));
                if !task_init(&me, arg, &loop_, &V::None, &V::None, &V::Bool(false))? {
                    return Ok(V::None);
                }
                uniq.push((key, me.clone()));
                nfuts += 1;
                me
            }
        };
        children.push(fut);
    }
    // outer = _GatheringFuture(children, loop=loop)
    let children_v = list(children.clone());
    let outer = V::Obj(Obj::new(gc));
    if let V::Obj(o) = &outer {
        o.set("_loop", loop_.clone());
        o.set("_callbacks", list(vec![]));
        o.set("_children", children_v.clone());
        o.set("_cancel_requested", V::Bool(false));
    }
    for (_, f) in &uniq {
        if let V::Obj(o) = f {
            o.set("_log_destroy_pending", V::Bool(false));
        }
    }
    let st = Rc::new(GatherState { nfuts, nfinished: Cell::new(0), outer: outer.clone(), children: children_v });
    let cb = func("_brevi_gather_done", move |a: Args| {
        let fut = a1(&a, "gather done")?;
        gather_done(&st, &fut)
    });
    for (_, f) in &uniq {
        match f {
            V::Obj(o) => match o.get("_callbacks") {
                Some(V::List(l)) => l.borrow_mut().push(tup(vec![cb.clone(), V::None])),
                _ => return Ok(V::None),
            },
            _ => return Ok(V::None),
        }
    }
    Ok(outer)
}
fn coro_send(coro: &V, sent: V) -> R {
    if let V::Ext(x) = coro {
        if let Some(c) = x.any().downcast_ref::<ACoro>() {
            return c.step(sent, None);
        }
    }
    call_method(coro, "send", Args::p(vec![sent]))
}
fn coro_throw(coro: &V, e: V) -> R {
    let e = match &e {
        V::Class(_) => match call(&e, Args::p(vec![])) {
            Ok(v) => v,
            Err(e) => return Err(e),
        },
        _ => e,
    };
    if let V::Ext(x) = coro {
        if let Some(c) = x.any().downcast_ref::<ACoro>() {
            return c.step(V::None, Some(E(e)));
        }
    }
    call_method(coro, "throw", Args::p(vec![e]))
}
// Task._step(exc=None) and Task._step_run_and_handle_result, as CPython's tasks.py writes them.
pub fn task_step(task: &V, excv: Option<V>) -> R {
    let V::Obj(o) = task else { return call_method(task, "_step", Args::p(vec![excv.unwrap_or(V::None)])) };
    if !future_pending(o) {
        return Err(asyncio_exc("InvalidStateError", "_step(): already done".into()));
    }
    let mut excv = excv;
    if truth(&opt_attr(task, "_must_cancel"))? {
        if !matches!(&excv, Some(e) if is_cancelled(e)) {
            excv = Some(make_cancelled_error(task));
        }
        o.set("_must_cancel", V::Bool(false));
    }
    o.set("_fut_waiter", V::None);
    let loop_ = opt_attr(task, "_loop");
    if !matches!(opt_attr(&loop_, "_current_task"), V::None) {
        return Err(exc("RuntimeError", "Cannot enter into task while another task is being executed.".into()));
    }
    setattr(&loop_, "_current_task", task.clone())?;
    let r = step_run(o, task, &loop_, excv);
    let cur = opt_attr(&loop_, "_current_task");
    if !is(&cur, task) {
        return Err(exc("RuntimeError", "Leaving task does not match the current task.".into()));
    }
    setattr(&loop_, "_current_task", V::None)?;
    r
}
fn step_run(o: &Obj, task: &V, loop_: &V, excv: Option<V>) -> R {
    let coro = opt_attr(task, "_coro");
    let result = match &excv {
        None => coro_send(&coro, V::None),
        Some(e) => coro_throw(&coro, e.clone()),
    };
    match result {
        Err(E(e)) if is_exc(&e, "StopIteration") => {
            let value = si_value(&e);
            if truth(&opt_attr(task, "_must_cancel"))? {
                o.set("_must_cancel", V::Bool(false));
                fut_cancel_body(task, opt_attr(task, "_cancel_message"))?;
            } else {
                fut_set_result(task, value)?;
            }
            unregister_task(task)?;
            Ok(V::None)
        }
        Err(E(e)) if is_cancelled(&e) => {
            o.set("_cancelled_exc", e.clone());
            fut_cancel_body(task, V::None)?;
            unregister_task(task)?;
            Ok(V::None)
        }
        Err(E(e)) if is_exc(&e, "KeyboardInterrupt") || is_exc(&e, "SystemExit") => {
            fut_set_exception(task, e.clone())?;
            unregister_task(task)?;
            Err(E(e))
        }
        Err(E(e)) => {
            fut_set_exception(task, e.clone())?;
            unregister_task(task)?;
            Ok(V::None)
        }
        Ok(result) => {
            if matches!(&result, V::None) {
                push_step(loop_, task.clone(), None)?;
                return Ok(V::None);
            }
            let blocking = opt_attr(&result, "_asyncio_future_blocking");
            if !matches!(&blocking, V::None) {
                let rloop = call_method(&result, "get_loop", Args::p(vec![]))?;
                if !is(&rloop, loop_) {
                    push_step(loop_, task.clone(), Some(exc_v("RuntimeError", s("Task got Future attached to a different loop")).0))?;
                } else if truth(&blocking)? {
                    if is(&result, task) {
                        push_step(loop_, task.clone(), Some(exc_v("RuntimeError", s("Task cannot await on itself")).0))?;
                    } else {
                        setattr(&result, "_asyncio_future_blocking", V::Bool(false))?;
                        let cb = getattr(task, "_wakeup")?;
                        let context = opt_attr(task, "_context");
                        call_method(&result, "add_done_callback", Args { p: vec![cb], k: vec![("context", context)] })?;
                        o.set("_fut_waiter", result.clone());
                        if truth(&opt_attr(task, "_must_cancel"))? {
                            let msg = opt_attr(task, "_cancel_message");
                            let r = call_method(&result, "cancel", Args { p: vec![], k: vec![("msg", msg)] })?;
                            if truth(&r)? {
                                o.set("_must_cancel", V::Bool(false));
                            }
                        }
                    }
                } else {
                    push_step(loop_, task.clone(), Some(exc_v("RuntimeError", s("yield was used instead of yield from in task")).0))?;
                }
            } else {
                push_step(loop_, task.clone(), Some(exc_v("RuntimeError", st(format!("Task got bad yield: {}", repr(&result).unwrap_or_default()))).0))?;
            }
            Ok(V::None)
        }
    }
}
// Task._wakeup(future): future.result(), thrown in as the next step's exception
pub fn task_wakeup(task: &V, fut: &V) -> R {
    match call_method(fut, "result", Args::p(vec![])) {
        Ok(_) => task_step(task, None),
        Err(E(e)) => task_step(task, Some(e)),
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

// The next item of an `async for` in a comprehension: None at its end.
pub fn afor_next(it: &V) -> Result<Option<V>, E> {
    let r = match ag_of(it) {
        Some(_) => {
            ag_hooks(it)?;
            ag_drive(it, AOp::Send(V::None))
        }
        None => bi_anext(Args::p(vec![it.clone()])).and_then(|f| aw(&f)),
    };
    match r {
        Ok(v) => Ok(Some(v)),
        Err(E(e)) if is_exc(&e, "StopAsyncIteration") => Ok(None),
        Err(e) => Err(e),
    }
}

// ---------- async generators (async def with yield) ----------
// The body runs on its own stack (a Coro). It stops there for two reasons: a `yield` (agen_yield, which sets
// AG_YIELDED), or an await deep inside it of a future that is not done (a plain gen_yield of the future). The
// awaitable asend()/athrow() return runs it (ag_drive): a yielded value is the await's result; a future is passed
// on up to the task, and what the task is resumed with is passed back down. As in CPython, the running loop's
// hooks (set_hooks) see each generator at its first step and when it is dropped part way, so the loop closes it
// (aclose() in a task) and asyncio.run() closes the ones still open at its end.

thread_local! {
    static AG_YIELDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AG_HOOKS: RefCell<(V, V)> = const { RefCell::new((V::None, V::None)) };
    static AG_TRACKED: RefCell<Vec<(i64, std::rc::Weak<Box<dyn Ext>>)>> = const { RefCell::new(Vec::new()) };
}

struct AGState {
    coro: Rc<RefCell<Box<Coro>>>,
    running: bool,
    hooked: bool,
    finalizer: V,
    closed: bool,
}

pub struct AGen {
    name: Sym,
    st: RefCell<AGState>,
}

pub fn make_agen(name: Sym, body: impl FnOnce() -> R + 'static) -> V {
    V::Ext(ExtRc::new(AGen { name, st: RefCell::new(AGState { coro: Rc::new(RefCell::new(Coro::new(name, Box::new(body)))), running: false, hooked: false, finalizer: V::None, closed: false }) }))
}

fn ag_of(x: &V) -> Option<&AGen> {
    match x {
        V::Ext(e) => e.any().downcast_ref::<AGen>(),
        _ => None,
    }
}

// `yield v` in an async generator
pub fn agen_yield(v: V) -> R {
    AG_YIELDED.with(|f| f.set(true));
    gen_yield(v)
}

enum AOp {
    Send(V),
    Throw(E),
    Close,
}

// The loop's firstiter hook, at a generator's first asend()/athrow()/aclose().
fn ag_hooks(me: &V) -> Result<(), E> {
    let ag = ag_of(me).unwrap();
    if ag.st.borrow().hooked {
        return Ok(());
    }
    let (first, fin) = AG_HOOKS.with(|h| h.borrow().clone());
    {
        let mut st = ag.st.borrow_mut();
        st.hooked = true;
        st.finalizer = fin;
    }
    if !matches!(first, V::None) {
        call(&first, Args::p(vec![me.clone()]))?;
    }
    Ok(())
}

fn ag_drive(me: &V, op: AOp) -> R {
    let ag = ag_of(me).unwrap();
    let (is_close, is_throw) = (matches!(op, AOp::Close), matches!(op, AOp::Throw(_)));
    let coro = {
        let mut st = ag.st.borrow_mut();
        if st.running {
            let who = if is_close { "aclose()" } else if is_throw { "athrow()" } else { "anext()" };
            return Err(exc("RuntimeError", format!("{who}: asynchronous generator is already running")));
        }
        if st.closed {
            return match op {
                AOp::Send(_) => Err(exc0("StopAsyncIteration")),
                _ => Ok(V::None),
            };
        }
        if let AOp::Send(v) = &op {
            if !matches!(v, V::None) && !st.coro.borrow().is_suspended() {
                return Err(exc("TypeError", "can't send non-None value to a just-started async generator".into()));
            }
        }
        st.running = true;
        st.coro.clone()
    };
    let mut r = match op {
        AOp::Send(v) => coro.borrow_mut().resume(v),
        AOp::Throw(e) => coro.borrow_mut().throw(e),
        AOp::Close => coro.borrow_mut().throw(exc0("GeneratorExit")),
    };
    let end = |closed: bool| {
        let mut st = ag.st.borrow_mut();
        st.running = false;
        st.closed |= closed;
    };
    loop {
        match r {
            Ok(Some(y)) => {
                if AG_YIELDED.with(|f| f.replace(false)) {
                    end(false);
                    if is_close {
                        return Err(exc("RuntimeError", "async generator ignored GeneratorExit".into()));
                    }
                    return Ok(y);
                }
                r = match gen_yield(y) {
                    Ok(s) => coro.borrow_mut().resume(s),
                    Err(e) => coro.borrow_mut().throw(e),
                };
            }
            Ok(None) => {
                end(true);
                return if is_close { Ok(V::None) } else { Err(exc0("StopAsyncIteration")) };
            }
            Err(E(e)) => {
                end(true);
                if is_close && is_exc(&e, "GeneratorExit") {
                    return Ok(V::None);
                }
                for n in ["StopAsyncIteration", "StopIteration"] {
                    if is_exc(&e, n) {
                        return Err(exc("RuntimeError", format!("async generator raised {n}")));
                    }
                }
                return Err(E(e));
            }
        }
    }
}

// What asend()/athrow()/aclose()/__anext__() return: awaiting it runs the generator one step.
fn ag_step(me: &V, op: AOp, kind: &'static str) -> R {
    ag_hooks(me)?;
    let me = me.clone();
    let name = ag_of(&me).unwrap().name.clone();
    let agen = Some(me.clone());
    Ok(V::Ext(ExtRc::new(ACoro { name, kind, st: RefCell::new(ACState::Fresh(Box::new(move || ag_drive(&me, op)))), agen })))
}

impl Ext for AGen {
    fn tname(&self) -> &'static str {
        "async_generator"
    }
    fn repr(&self) -> String {
        format!("<async_generator object {} at {:#x}>", &*self.name, self as *const AGen as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn hash(&self) -> Option<i64> {
        Some((self as *const AGen as usize >> 4) as i64)
    }
    fn eq(&self, o: &V) -> Option<bool> {
        Some(matches!(o, V::Ext(e) if std::ptr::eq(e.any() as *const dyn std::any::Any as *const u8, self as *const AGen as *const u8)))
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        match n {
            "__name__" | "__qualname__" => Some(Ok(st(self.name.to_string()))),
            "ag_running" => Some(Ok(V::Bool(self.st.borrow().running))),
            "asend" | "athrow" | "aclose" | "__anext__" | "__aiter__" => None,
            _ => Some(Err(exc("AttributeError", format!("'async_generator' object has no attribute '{n}'")))),
        }
    }
    fn method(&self, me: &V, n: &str, a: &mut Args) -> Option<R> {
        let nargs = a.p.len();
        let want = |k: usize| -> Result<(), E> {
            if nargs != k || !a.k.is_empty() {
                let what = if k == 0 { "takes no arguments".to_string() } else { "takes exactly one argument".to_string() };
                return Err(exc("TypeError", format!("{n}() {what} ({nargs} given)")));
            }
            Ok(())
        };
        Some(match n {
            "__aiter__" => want(0).map(|_| me.clone()),
            "__anext__" => want(0).and_then(|_| ag_step(me, AOp::Send(V::None), "async_generator_asend")),
            "asend" => want(1).and_then(|_| ag_step(me, AOp::Send(a.p[0].clone()), "async_generator_asend")),
            "aclose" => want(0).and_then(|_| ag_step(me, AOp::Close, "async_generator_athrow")),
            "athrow" => {
                if nargs != 1 || !a.k.is_empty() {
                    return Some(Err(unsupported("athrow() with more than one argument")));
                }
                match exc_of(a.p[0].clone()) {
                    Ok(e) if is_exc(&e, "BaseException") => ag_step(me, AOp::Throw(E(e)), "async_generator_athrow"),
                    Ok(_) => Err(exc("TypeError", "exceptions must be classes or instances deriving from BaseException, not ".to_string() + &type_name(&a.p[0]))),
                    Err(e) => Err(e),
                }
            }
            _ => return None,
        })
    }
}

impl AGen {
    // Closed at once, not in a task: GeneratorExit thrown in where it is suspended, so its finally blocks run.
    fn close_now(&self) {
        let c = {
            let mut st = self.st.borrow_mut();
            if st.closed || st.running {
                return;
            }
            st.closed = true;
            st.coro.clone()
        };
        if c.borrow().is_suspended() {
            let r = c.borrow_mut().throw(exc0("GeneratorExit"));
            AG_YIELDED.with(|f| f.set(false));
            let _ = r;
        } else {
            c.borrow_mut().close();
        }
    }
}

impl Drop for AGen {
    // Dropped part way: the loop's finalizer hook gets it (as a new object holding the same stack) and closes it in
    // a task, as CPython's does. With no hook, it is closed here and now, so its finally blocks run.
    fn drop(&mut self) {
        let st = self.st.get_mut();
        if st.closed || st.running || std::thread::panicking() || !st.coro.borrow().is_suspended() {
            return;
        }
        let fin = std::mem::replace(&mut st.finalizer, V::None);
        if matches!(fin, V::None) {
            self.close_now();
            return;
        }
        let again = AGen { name: self.name.clone(), st: RefCell::new(AGState { coro: st.coro.clone(), running: false, hooked: true, finalizer: V::None, closed: false }) };
        let _ = call(&fin, Args::p(vec![V::Ext(ExtRc::new(again))]));
    }
}

// _brevi_async.set_hooks / track / untrack / tracked: sys.set_asyncgen_hooks and the loop's set of open async
// generators (a WeakSet in CPython), for py/asyncio.py.
fn ag_hook_fn(n: &str, a: &Args) -> R {
    match n {
        "set_hooks" => {
            a.want("set_hooks", 2, 2)?;
            let old = AG_HOOKS.with(|h| std::mem::replace(&mut *h.borrow_mut(), (a.p[0].clone(), a.p[1].clone())));
            Ok(tup(vec![old.0, old.1]))
        }
        "track" | "untrack" => {
            a.want(n, 2, 2)?;
            let key = match &a.p[0] { V::Int(i) => *i as i64, _ => 0 };
            let V::Ext(e) = &a.p[1] else { return Ok(V::None) };
            let w = ExtRc::downgrade(e);
            AG_TRACKED.with(|t| {
                let mut t = t.borrow_mut();
                t.retain(|(k, x)| x.strong_count() > 0 && !(*k == key && std::rc::Weak::ptr_eq(x, &w)));
                if n == "track" {
                    t.push((key, w));
                }
            });
            Ok(V::None)
        }
        "tracked" => {
            a.want("tracked", 1, 1)?;
            let key = match &a.p[0] { V::Int(i) => *i as i64, _ => 0 };
            let out: Vec<V> = AG_TRACKED.with(|t| {
                let mut t = t.borrow_mut();
                let out = t.iter().filter(|(k, _)| *k == key).filter_map(|(_, x)| x.upgrade()).map(|r| V::Ext(ExtRc(r))).collect();
                t.retain(|(k, x)| *k != key && x.strong_count() > 0);
                out
            });
            Ok(list(out))
        }
        _ => Err(unsupported(&format!("_brevi_async.{n}"))),
    }
}
