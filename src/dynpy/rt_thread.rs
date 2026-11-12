// ---------- threading: threads, Lock, RLock, Event ----------
// A threading.Thread runs on a stack of its own, as a generator does (rt_gen.rs), on the program's one OS thread.
// Threads take turns: the running one goes on until it waits (a lock another thread holds, Event.wait, join,
// time.sleep while other threads are alive, ...) or ends, and then the thread that has been ready longest runs.
// start() runs the new thread at once, up to its first wait, and the starter goes on after it, so a thread that
// never waits prints exactly where it did before threads took turns. CPython also switches every few milliseconds,
// which only reorders output the program does not synchronise; whatever a program orders with locks, events,
// barriers and joins comes out in CPython's order. Every thread has its own recursion depth, generator chain,
// exception being handled, warning line and decimal context, as in CPython. A wait no thread can ever end (every
// thread waiting, none with a timeout) would hang CPython for ever: the program stops with an error instead.

pub struct WaitQ(RefCell<std::collections::VecDeque<usize>>);
impl WaitQ {
    pub fn new() -> WaitQ {
        WaitQ(RefCell::new(std::collections::VecDeque::new()))
    }
    // the thread that has waited longest goes on (true), or nobody was waiting (false)
    pub fn wake_one(&self) -> bool {
        let i = self.0.borrow_mut().pop_front();
        match i {
            Some(i) => {
                gt_wake(i);
                true
            }
            None => false,
        }
    }
    pub fn wake_all(&self) {
        let all: Vec<usize> = self.0.borrow_mut().drain(..).collect();
        for i in all {
            gt_wake(i);
        }
    }
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

#[derive(PartialEq, Clone, Copy)]
enum TState {
    Ready,
    Running,
    Waiting,
    Done,
}

struct GThread {
    sp: *mut u8,
    #[cfg(windows)]
    fiber: *mut std::ffi::c_void,
    stack: *mut u8,
    started: bool,
    body: Option<Box<dyn FnOnce()>>,
    state: TState,
    // monotonic ns at which a wait ends by itself
    deadline: Option<i64>,
    // the wait ended because another thread woke it (not by its deadline)
    woken: bool,
    daemon: bool,
    joiners: WaitQ,
    // the thread's own context while another thread runs
    coro_now: Vec<*mut Coro>,
    depth: usize,
    gen_name: Option<Sym>,
    site: Option<ReSite>,
    exc: Vec<(u64, V)>,
    repr: Vec<usize>,
    prepr: Vec<usize>,
    dprec: usize,
    dround: u8,
}
impl GThread {
    fn new(body: Option<Box<dyn FnOnce()>>, state: TState) -> GThread {
        GThread {
            sp: std::ptr::null_mut(),
            #[cfg(windows)]
            fiber: std::ptr::null_mut(),
            stack: std::ptr::null_mut(),
            started: body.is_none(),
            body,
            state,
            deadline: None,
            woken: false,
            daemon: false,
            joiners: WaitQ::new(),
            coro_now: Vec::new(),
            depth: 0,
            gen_name: None,
            site: None,
            exc: Vec::new(),
            repr: Vec::new(),
            prepr: Vec::new(),
            dprec: 28,
            dround: 0,
        }
    }
}

thread_local! {
    // every thread started, the main thread first; boxed so a pointer to one stays good, and never freed (a
    // finished thread gives back its stack, and keeps only this small record, which its Thread object names)
    static GT: RefCell<Vec<*mut GThread>> = const { RefCell::new(Vec::new()) };
    static GT_CUR: Cell<usize> = const { Cell::new(0) };
    static GT_READY: RefCell<std::collections::VecDeque<usize>> = const { RefCell::new(std::collections::VecDeque::new()) };
    // finished threads whose stacks the next thread to run gives back (a stack cannot free itself)
    static GT_DEAD: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    // calls from a Python library (a dynamic build's bridge) into the program now running: CPython's own frames
    // are on this stack, so it may not switch to another thread
    pub static GT_CB: Cell<usize> = const { Cell::new(0) };
    static GT_COUNTER: Cell<u64> = const { Cell::new(0) };
}

fn gt(i: usize) -> *mut GThread {
    GT.with(|g| g.borrow()[i])
}
fn gt_cur() -> usize {
    gt_init();
    GT_CUR.with(|c| c.get())
}
fn gt_init() {
    GT.with(|g| {
        let mut g = g.borrow_mut();
        if g.is_empty() {
            g.push(Box::into_raw(Box::new(GThread::new(None, TState::Running))));
        }
    });
}
// any thread besides the running one that has not ended
fn gt_others_alive() -> bool {
    let me = gt_cur();
    GT.with(|g| g.borrow().iter().enumerate().any(|(i, t)| i != me && unsafe { (**t).state } != TState::Done))
}
fn gt_wake(i: usize) {
    let t = gt(i);
    unsafe {
        if (*t).state == TState::Waiting {
            (*t).state = TState::Ready;
            (*t).woken = true;
            (*t).deadline = None;
            GT_READY.with(|r| r.borrow_mut().push_back(i));
        }
    }
}
// The thread to run next: the one ready longest, else the waiting one whose deadline comes first (after
// sleeping until then); None when every thread waits without a deadline.
fn gt_next() -> Option<usize> {
    loop {
        let now = clock_ns(false);
        let n = GT.with(|g| g.borrow().len());
        let mut first: Option<(i64, usize)> = None;
        for i in 0..n {
            let t = gt(i);
            unsafe {
                if (*t).state == TState::Waiting {
                    if let Some(d) = (*t).deadline {
                        if d <= now {
                            (*t).state = TState::Ready;
                            (*t).woken = false;
                            (*t).deadline = None;
                            GT_READY.with(|r| r.borrow_mut().push_back(i));
                        } else if first.map_or(true, |(f, _)| d < f) {
                            first = Some((d, i));
                        }
                    }
                }
            }
        }
        if let Some(i) = GT_READY.with(|r| r.borrow_mut().pop_front()) {
            return Some(i);
        }
        let (d, _) = first?;
        std::thread::sleep(std::time::Duration::from_nanos((d - now).max(0) as u64));
    }
}
fn gt_reap() {
    let me = GT_CUR.with(|c| c.get());
    let dead: Vec<usize> = GT_DEAD.with(|d| {
        let mut d = d.borrow_mut();
        let (gone, keep): (Vec<usize>, Vec<usize>) = d.drain(..).partition(|i| *i != me);
        *d = keep;
        gone
    });
    for i in dead {
        let t = gt(i);
        unsafe {
            #[cfg(not(windows))]
            if !(*t).stack.is_null() {
                stack_free((*t).stack);
                (*t).stack = std::ptr::null_mut();
            }
            #[cfg(windows)]
            if !(*t).fiber.is_null() {
                fiber::delete((*t).fiber);
                (*t).fiber = std::ptr::null_mut();
            }
            (*t).coro_now = Vec::new();
            (*t).exc = Vec::new();
        }
    }
}

// Saves the running thread's context, puts back thread `to`'s, and switches to its stack. Returns when some
// thread switches back to this one.
unsafe fn gt_switch(to: usize) -> Result<(), E> {
    let from = GT_CUR.with(|c| c.get());
    let t = gt(to);
    (*t).state = TState::Running;
    if from == to {
        return Ok(());
    }
    let f = gt(from);
    if !(*t).started {
        #[cfg(not(windows))]
        {
            let Some(p) = stack_alloc() else { return Err(exc("RuntimeError", "can't start new thread".into())) };
            (*t).stack = p;
            (*t).sp = ctx_init_stack(p.add(CORO_STACK), t as usize, brevi_thread_stub as usize);
        }
        #[cfg(windows)]
        {
            fiber::ensure()?;
            let h = fiber::create(CORO_STACK, gt_fiber_proc, t as *mut std::ffi::c_void);
            if h.is_null() {
                return Err(exc("RuntimeError", "can't start new thread".into()));
            }
            (*t).fiber = h;
        }
        (*t).started = true;
    }
    (*f).coro_now = CORO_NOW.with(|s| std::mem::take(&mut *s.borrow_mut()));
    (*f).depth = DEPTH.with(|d| d.get());
    (*f).gen_name = CUR_GEN_NAME.with(|n| n.get());
    (*f).site = re_site_get();
    (*f).exc = EXC_STACK.with(|s| std::mem::take(&mut *s.borrow_mut()));
    (*f).repr = REPR_DEPTH.with(|s| std::mem::take(&mut *s.borrow_mut()));
    (*f).prepr = PARTIAL_REPR.with(|s| std::mem::take(&mut *s.borrow_mut()));
    (*f).dprec = DPREC.with(|p| p.get());
    (*f).dround = DROUND.with(|p| p.get());
    CORO_NOW.with(|s| *s.borrow_mut() = std::mem::take(&mut (*t).coro_now));
    DEPTH.with(|d| d.set((*t).depth));
    CUR_GEN_NAME.with(|n| n.set((*t).gen_name));
    re_site_put((*t).site);
    EXC_STACK.with(|s| *s.borrow_mut() = std::mem::take(&mut (*t).exc));
    REPR_DEPTH.with(|s| *s.borrow_mut() = std::mem::take(&mut (*t).repr));
    PARTIAL_REPR.with(|s| *s.borrow_mut() = std::mem::take(&mut (*t).prepr));
    DPREC.with(|p| p.set((*t).dprec));
    DROUND.with(|p| p.set((*t).dround));
    GT_CUR.with(|c| c.set(to));
    #[cfg(not(windows))]
    brevi_ctx_switch(&mut (*f).sp, (*t).sp);
    #[cfg(windows)]
    {
        (*f).fiber = fiber::CUR.with(|c| c.get());
        fiber::CUR.with(|c| c.set((*t).fiber));
        fiber::switch((*t).fiber);
    }
    gt_reap();
    Ok(())
}

fn gt_deadlock() -> ! {
    out_flush();
    err_write("brevi: every thread is waiting and none can go on (CPython would wait for ever)\n");
    std::process::exit(1);
}

// The running thread waits, in `q` (or nowhere: a sleep), until another thread wakes it (true) or the deadline
// passes (false).
pub fn gt_wait(q: Option<&WaitQ>, deadline: Option<i64>) -> Result<bool, E> {
    let me = gt_cur();
    let t = gt(me);
    unsafe {
        (*t).state = TState::Waiting;
        (*t).deadline = deadline;
        (*t).woken = false;
    }
    if let Some(q) = q {
        q.0.borrow_mut().push_back(me);
    }
    let undo = |t: *mut GThread| unsafe {
        (*t).state = TState::Running;
        (*t).deadline = None;
        if let Some(q) = q {
            q.0.borrow_mut().retain(|i| *i != me);
        }
    };
    let Some(next) = gt_next() else {
        undo(t);
        gt_deadlock()
    };
    if next != me && GT_CB.with(|c| c.get()) > 0 {
        undo(t);
        GT_READY.with(|r| r.borrow_mut().push_front(next));
        unsafe { (*gt(next)).state = TState::Ready };
        return Err(unsupported("a thread waiting inside a call from a Python library"));
    }
    if let Err(e) = unsafe { gt_switch(next) } {
        // the thread to run could not start: it stays ready, and this one goes on with the error
        GT_READY.with(|r| r.borrow_mut().push_front(next));
        unsafe { (*gt(next)).state = TState::Ready };
        undo(t);
        unsafe { (*gt(me)).state = TState::Running };
        return Err(e);
    }
    let woken = unsafe { (*t).woken };
    if !woken {
        if let Some(q) = q {
            q.0.borrow_mut().retain(|i| *i != me);
        }
    }
    Ok(woken)
}

// a timeout in seconds as a deadline in monotonic ns (None: wait for ever)
pub fn gt_deadline(secs: Option<f64>) -> Option<i64> {
    secs.map(|s| clock_ns(false).saturating_add((s.max(0.0) * 1e9).min(4e18) as i64))
}

// time.sleep: other threads run meanwhile
pub fn gt_sleep(secs: f64) -> Result<(), E> {
    if GT.with(|g| g.borrow().len()) > 1 && gt_others_alive() {
        gt_wait(None, gt_deadline(Some(secs)))?;
        return Ok(());
    }
    std::thread::sleep(std::time::Duration::from_secs_f64(secs.min(1e9)));
    Ok(())
}

// Before atexit: CPython waits for every thread that is not a daemon.
pub fn gt_shutdown() {
    if GT.with(|g| g.borrow().len()) <= 1 {
        return;
    }
    loop {
        let n = GT.with(|g| g.borrow().len());
        let next = (1..n).find(|i| unsafe { (*gt(*i)).state != TState::Done && !(*gt(*i)).daemon });
        let Some(i) = next else { break };
        if gt_join(i, None).is_err() {
            break;
        }
    }
}

fn gt_join(i: usize, deadline: Option<i64>) -> Result<(), E> {
    loop {
        let t = gt(i);
        if unsafe { (*t).state } == TState::Done {
            return Ok(());
        }
        let q: &WaitQ = unsafe { &(*t).joiners };
        if !gt_wait(Some(q), deadline)? {
            return Ok(());
        }
    }
}

#[cfg(all(target_arch = "x86_64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_thread_stub() {
    core::arch::naked_asm!("mov rdi, r12", "and rsp, -16", "call {m}", "ud2", m = sym gt_entry)
}
#[cfg(all(target_arch = "aarch64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_thread_stub() {
    core::arch::naked_asm!("mov x0, x19", "bl {m}", "brk #0", m = sym gt_entry)
}
#[cfg(not(windows))]
extern "C" fn gt_entry(t: *mut GThread) {
    unsafe { gt_run(t) }
}
#[cfg(windows)]
unsafe extern "system" fn gt_fiber_proc(t: *mut std::ffi::c_void) {
    gt_run(t as *mut GThread)
}
unsafe fn gt_run(t: *mut GThread) -> ! {
    gt_reap();
    if let Some(body) = (*t).body.take() {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).is_err() {
            out_flush();
            err_write("brevi: a thread stopped with an internal error\n");
        }
    }
    (*t).state = TState::Done;
    (*t).joiners.wake_all();
    let me = GT_CUR.with(|c| c.get());
    GT_DEAD.with(|d| d.borrow_mut().push(me));
    match gt_next() {
        Some(n) => {
            let _ = gt_switch(n);
        }
        None => gt_deadlock(),
    }
    std::process::abort()
}

fn timeout_arg(v: Option<V>) -> Result<Option<f64>, E> {
    match v {
        None | Some(V::None) => Ok(None),
        Some(v) => Ok(Some(to_float(&v)?)),
    }
}

pub struct Thread {
    target: RefCell<V>,
    args: RefCell<V>,
    kwargs: RefCell<V>,
    name: RefCell<String>,
    daemon: Cell<bool>,
    idx: Cell<Option<usize>>,
}
impl Thread {
    fn alive(&self) -> bool {
        self.idx.get().is_some_and(|i| unsafe { (*gt(i)).state } != TState::Done)
    }
    fn run_target(target: V, args: V, kwargs: V) -> R {
        if matches!(target, V::None) {
            return Ok(V::None);
        }
        let mut a = Args::p(to_vec(&args)?);
        if !matches!(kwargs, V::None) {
            star_kwargs(&mut a, &kwargs)?;
        }
        call(&target, a)?;
        Ok(V::None)
    }
    fn take_run(&self) -> (V, V, V) {
        (
            std::mem::replace(&mut *self.target.borrow_mut(), V::None),
            std::mem::replace(&mut *self.args.borrow_mut(), tup(vec![])),
            std::mem::replace(&mut *self.kwargs.borrow_mut(), V::None),
        )
    }
    fn start(&self) -> R {
        if self.idx.get().is_some() {
            return Err(exc("RuntimeError", "threads can only be started once".into()));
        }
        let me = gt_cur();
        let (target, args, kwargs) = self.take_run();
        let name = self.name.borrow().clone();
        let body: Box<dyn FnOnce()> = Box::new(move || {
            if let Err(E(e)) = Thread::run_target(target, args, kwargs) {
                if !is_exc(&e, "SystemExit") {
                    out_flush();
                    err_write(&format!("Exception in thread {name}:\nTraceback (most recent call last):\n{}\n", exc_line(&e)));
                }
            }
        });
        let mut g = GThread::new(Some(body), TState::Ready);
        g.daemon = self.daemon.get();
        let i = GT.with(|v| {
            let mut v = v.borrow_mut();
            v.push(Box::into_raw(Box::new(g)));
            v.len() - 1
        });
        self.idx.set(Some(i));
        // the new thread runs now; the starter is the first to go on after it waits or ends
        unsafe { (*gt(me)).state = TState::Ready };
        GT_READY.with(|r| r.borrow_mut().push_front(me));
        if let Err(e) = unsafe { gt_switch(i) } {
            GT_READY.with(|r| r.borrow_mut().retain(|x| *x != me));
            unsafe {
                (*gt(me)).state = TState::Running;
                (*gt(i)).state = TState::Done;
            }
            return Err(e);
        }
        Ok(V::None)
    }
}
impl Ext for Thread {
    fn tname(&self) -> &'static str {
        "threading.Thread"
    }
    fn repr(&self) -> String {
        let st = match self.idx.get() {
            None => "initial".to_string(),
            Some(i) if self.alive() => format!("started {}", i + 1),
            Some(i) => format!("stopped {}", i + 1),
        };
        let d = if self.daemon.get() { " daemon" } else { "" };
        format!("<Thread({}, {st}{d})>", self.name.borrow())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(Ok(match n {
            "name" => s(&self.name.borrow()),
            "daemon" => V::Bool(self.daemon.get()),
            "ident" | "native_id" => match self.idx.get() {
                Some(i) => V::Int(i as i64 + 1),
                None => V::None,
            },
            _ => return None,
        }))
    }
    fn set_attr(&self, _me: &V, n: &str, v: V) -> Option<Result<(), E>> {
        Some(match n {
            "name" => to_str(&v).map(|x| *self.name.borrow_mut() = x),
            "daemon" => {
                if self.idx.get().is_some() {
                    Err(exc("RuntimeError", "cannot set daemon status of active thread".into()))
                } else {
                    truth(&v).map(|b| self.daemon.set(b))
                }
            }
            _ => return None,
        })
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(match n {
            "start" => self.start(),
            "run" => {
                let (t, x, k) = self.take_run();
                Thread::run_target(t, x, k)
            }
            "join" => (|| {
                let Some(i) = self.idx.get() else { return Err(exc("RuntimeError", "cannot join thread before it is started".into())) };
                if i == gt_cur() {
                    return Err(exc("RuntimeError", "cannot join current thread".into()));
                }
                let t = timeout_arg(a.kw("timeout").or_else(|| a.p.first().cloned()))?;
                gt_join(i, gt_deadline(t))?;
                Ok(V::None)
            })(),
            "is_alive" => Ok(V::Bool(self.alive())),
            "getName" => Ok(s(&self.name.borrow())),
            "isDaemon" => Ok(V::Bool(self.daemon.get())),
            "setDaemon" => match a.p.first() {
                Some(v) => match truth(v) {
                    Ok(b) if self.idx.get().is_none() => {
                        self.daemon.set(b);
                        Ok(V::None)
                    }
                    Ok(_) => Err(exc("RuntimeError", "cannot set daemon status of active thread".into())),
                    Err(e) => Err(e),
                },
                None => Ok(V::None),
            },
            _ => Err(exc("AttributeError", format!("'Thread' object has no attribute '{n}'"))),
        })
    }
}

fn lock_args(a: &mut Args) -> Result<(bool, Option<f64>), E> {
    let blocking = match a.kw("blocking").or_else(|| a.p.first().cloned()) {
        None => true,
        Some(v) => truth(&v)?,
    };
    let timeout = match a.kw("timeout").or_else(|| a.p.get(1).cloned()) {
        None => -1.0,
        Some(v) => to_float(&v)?,
    };
    if !blocking && timeout != -1.0 {
        return Err(exc("ValueError", "can't specify a timeout for a non-blocking call".into()));
    }
    if timeout < 0.0 && timeout != -1.0 {
        return Err(exc("ValueError", "timeout value must be a non-negative number".into()));
    }
    Ok((blocking, if timeout == -1.0 { None } else { Some(timeout) }))
}

pub struct Lock {
    held: Cell<bool>,
    q: WaitQ,
}
impl Lock {
    pub fn acquire(&self, blocking: bool, timeout: Option<f64>) -> Result<bool, E> {
        if !self.held.get() {
            self.held.set(true);
            return Ok(true);
        }
        if !blocking {
            return Ok(false);
        }
        let deadline = gt_deadline(timeout);
        loop {
            let woken = gt_wait(Some(&self.q), deadline)?;
            if !self.held.get() {
                self.held.set(true);
                return Ok(true);
            }
            if !woken {
                return Ok(false);
            }
        }
    }
    pub fn release(&self) -> Result<(), E> {
        if !self.held.get() {
            return Err(exc("RuntimeError", "release unlocked lock".into()));
        }
        self.held.set(false);
        self.q.wake_one();
        Ok(())
    }
}
impl Ext for Lock {
    fn tname(&self) -> &'static str {
        "_thread.lock"
    }
    fn repr(&self) -> String {
        format!("<{} _thread.lock object>", if self.held.get() { "locked" } else { "unlocked" })
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(match n {
            "acquire" | "acquire_lock" => lock_args(a).and_then(|(b, t)| self.acquire(b, t)).map(V::Bool),
            "__enter__" => self.acquire(true, None).map(V::Bool),
            "release" | "release_lock" | "__exit__" => self.release().map(|_| V::None),
            "locked" | "locked_lock" => Ok(V::Bool(self.held.get())),
            _ => Err(exc("AttributeError", format!("'_thread.lock' object has no attribute '{n}'"))),
        })
    }
}

pub struct RLock {
    owner: Cell<Option<usize>>,
    count: Cell<usize>,
    q: WaitQ,
}
impl RLock {
    fn acquire(&self, blocking: bool, timeout: Option<f64>) -> Result<bool, E> {
        let me = gt_cur();
        match self.owner.get() {
            Some(o) if o == me => {
                self.count.set(self.count.get() + 1);
                return Ok(true);
            }
            None => {
                self.owner.set(Some(me));
                self.count.set(1);
                return Ok(true);
            }
            Some(_) if !blocking => return Ok(false),
            Some(_) => {}
        }
        let deadline = gt_deadline(timeout);
        loop {
            let woken = gt_wait(Some(&self.q), deadline)?;
            if self.owner.get().is_none() {
                self.owner.set(Some(me));
                self.count.set(1);
                return Ok(true);
            }
            if !woken {
                return Ok(false);
            }
        }
    }
    fn release(&self) -> Result<(), E> {
        if self.owner.get() != Some(gt_cur()) {
            return Err(exc("RuntimeError", "cannot release un-acquired lock".into()));
        }
        let c = self.count.get() - 1;
        self.count.set(c);
        if c == 0 {
            self.owner.set(None);
            self.q.wake_one();
        }
        Ok(())
    }
}
impl Ext for RLock {
    fn tname(&self) -> &'static str {
        "_thread.RLock"
    }
    fn repr(&self) -> String {
        let (st, owner) = match self.owner.get() {
            Some(o) => ("locked", o + 1),
            None => ("unlocked", 0),
        };
        format!("<{st} _thread.RLock object owner={owner} count={}>", self.count.get())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(match n {
            "acquire" => lock_args(a).and_then(|(b, t)| self.acquire(b, t)).map(V::Bool),
            "__enter__" => self.acquire(true, None).map(V::Bool),
            "release" | "__exit__" => self.release().map(|_| V::None),
            _ => Err(exc("AttributeError", format!("'_thread.RLock' object has no attribute '{n}'"))),
        })
    }
}

pub struct Event {
    flag: Cell<bool>,
    q: WaitQ,
}
impl Ext for Event {
    fn tname(&self) -> &'static str {
        "threading.Event"
    }
    fn repr(&self) -> String {
        format!("<threading.Event {}>", if self.flag.get() { "set" } else { "unset" })
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        Some(match n {
            "is_set" | "isSet" => Ok(V::Bool(self.flag.get())),
            "set" => {
                self.flag.set(true);
                self.q.wake_all();
                Ok(V::None)
            }
            "clear" => {
                self.flag.set(false);
                Ok(V::None)
            }
            "wait" => (|| {
                if self.flag.get() {
                    return Ok(V::Bool(true));
                }
                let t = timeout_arg(a.kw("timeout").or_else(|| a.p.first().cloned()))?;
                // Condition.wait: a timeout of 0 or less does not wait
                if t.is_some_and(|t| t <= 0.0) {
                    return Ok(V::Bool(false));
                }
                gt_wait(Some(&self.q), gt_deadline(t))?;
                Ok(V::Bool(self.flag.get()))
            })(),
            _ => Err(exc("AttributeError", format!("'Event' object has no attribute '{n}'"))),
        })
    }
}

pub fn threading_fn(n: &str, mut a: Args) -> R {
    match n {
        "stack_size" => Ok(V::Int(0)),
        "Thread" => {
            let target = a.kw("target").unwrap_or(V::None);
            let args = a.kw("args").unwrap_or(tup(vec![]));
            let kwargs = a.kw("kwargs").unwrap_or(V::None);
            let daemon = match a.kw("daemon") {
                None | Some(V::None) => false,
                Some(v) => truth(&v)?,
            };
            let name = match a.kw("name") {
                Some(v) if !matches!(v, V::None) => to_str(&v)?,
                _ => {
                    let k = GT_COUNTER.with(|c| {
                        c.set(c.get() + 1);
                        c.get()
                    });
                    match getattr(&target, "__name__") {
                        Ok(t) if !matches!(target, V::None) => format!("Thread-{k} ({})", to_str(&t)?),
                        _ => format!("Thread-{k}"),
                    }
                }
            };
            Ok(V::Ext(ExtRc::new(Thread {
                target: RefCell::new(target),
                args: RefCell::new(args),
                kwargs: RefCell::new(kwargs),
                name: RefCell::new(name),
                daemon: Cell::new(daemon),
                idx: Cell::new(None),
            })))
        }
        "Lock" => Ok(V::Ext(ExtRc::new(Lock { held: Cell::new(false), q: WaitQ::new() }))),
        "RLock" => Ok(V::Ext(ExtRc::new(RLock { owner: Cell::new(None), count: Cell::new(0), q: WaitQ::new() }))),
        "Event" => Ok(V::Ext(ExtRc::new(Event { flag: Cell::new(false), q: WaitQ::new() }))),
        "current_thread" | "main_thread" => Ok(V::None),
        "active_count" => {
            gt_init();
            Ok(V::Int(GT.with(|g| g.borrow().iter().filter(|t| unsafe { (***t).state } != TState::Done).count()) as i64))
        }
        _ => Err(unsupported(&format!("threading.{n}"))),
    }
}
