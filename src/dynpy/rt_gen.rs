// ---------- generators ----------
// A Python generator runs on its own stack, on the same thread: next() switches to that stack, `yield`
// switches back. The body is the function's code as written, so a generator runs lazily and in step with
// its caller, as in CPython (prints interleave, infinite generators work, exceptions surface at next()).

#[cfg(all(target_arch = "x86_64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_ctx_switch(_from: *mut *mut u8, _to: *mut u8) {
    core::arch::naked_asm!(
        "push rbp", "push rbx", "push r12", "push r13", "push r14", "push r15",
        "mov [rdi], rsp",
        "mov rsp, rsi",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop rbx", "pop rbp",
        "ret",
    )
}
#[cfg(all(target_arch = "x86_64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_coro_stub() {
    core::arch::naked_asm!("mov rdi, r12", "and rsp, -16", "call {m}", "ud2", m = sym coro_main)
}
#[cfg(not(windows))]
unsafe fn coro_init_stack(top: *mut u8, arg: *mut Coro) -> *mut u8 {
    ctx_init_stack(top, arg as usize, brevi_coro_stub as usize)
}
// A fresh stack that starts in `stub` with `arg` in r12 (x86_64) or x19 (aarch64): generators and threads.
#[cfg(all(target_arch = "x86_64", not(windows)))]
unsafe fn ctx_init_stack(top: *mut u8, arg: usize, stub: usize) -> *mut u8 {
    // Popped by brevi_ctx_switch: r15 r14 r13 r12 rbx rbp, then the return address (the stub).
    let r = ((top as usize - 16) & !15) as *mut usize;
    *r = stub;
    let regs = r.sub(6);
    for k in 0..6 {
        *regs.add(k) = 0;
    }
    *regs.add(3) = arg; // r12
    regs as *mut u8
}

#[cfg(all(target_arch = "aarch64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_ctx_switch(_from: *mut *mut u8, _to: *mut u8) {
    core::arch::naked_asm!(
        "sub sp, sp, #160",
        "stp x19, x20, [sp, #0]", "stp x21, x22, [sp, #16]", "stp x23, x24, [sp, #32]", "stp x25, x26, [sp, #48]",
        "stp x27, x28, [sp, #64]", "stp x29, x30, [sp, #80]", "stp d8, d9, [sp, #96]", "stp d10, d11, [sp, #112]",
        "stp d12, d13, [sp, #128]", "stp d14, d15, [sp, #144]",
        "mov x2, sp", "str x2, [x0]", "mov sp, x1",
        "ldp x19, x20, [sp, #0]", "ldp x21, x22, [sp, #16]", "ldp x23, x24, [sp, #32]", "ldp x25, x26, [sp, #48]",
        "ldp x27, x28, [sp, #64]", "ldp x29, x30, [sp, #80]", "ldp d8, d9, [sp, #96]", "ldp d10, d11, [sp, #112]",
        "ldp d12, d13, [sp, #128]", "ldp d14, d15, [sp, #144]",
        "add sp, sp, #160",
        "ret",
    )
}
#[cfg(all(target_arch = "aarch64", not(windows)))]
#[unsafe(naked)]
unsafe extern "C" fn brevi_coro_stub() {
    core::arch::naked_asm!("mov x0, x19", "bl {m}", "brk #0", m = sym coro_main)
}
#[cfg(all(target_arch = "aarch64", not(windows)))]
unsafe fn ctx_init_stack(top: *mut u8, arg: usize, stub: usize) -> *mut u8 {
    let sp = (((top as usize) & !15) - 160) as *mut usize;
    for k in 0..20 {
        *sp.add(k) = 0;
    }
    *sp = arg; // x19
    *sp.add(11) = stub; // x30
    sp as *mut u8
}

pub const GEN_SUPPORTED: bool = cfg!(any(target_arch = "x86_64", target_arch = "aarch64"));

// Windows has fibers, which own a stack and switch it for us (and update the TEB, so stack overflow and
// unwind work). The Unix targets use the assembly above; this is the same idea through kernel32.
#[cfg(windows)]
mod fiber {
    use super::*;
    use std::ffi::c_void;
    pub type H = *mut c_void;
    unsafe extern "system" {
        fn ConvertThreadToFiber(p: *mut c_void) -> H;
        fn CreateFiberEx(commit: usize, reserve: usize, flags: u32, start: unsafe extern "system" fn(*mut c_void), p: *mut c_void) -> H;
        fn DeleteFiber(f: H);
        fn SwitchToFiber(f: H);
        fn GetLastError() -> u32;
    }
    thread_local! {
        pub static CUR: std::cell::Cell<H> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    }
    // The thread's own fiber, created on the first generator or async task.
    pub fn ensure() -> Result<(), E> {
        if !CUR.with(|c| c.get().is_null()) {
            return Ok(());
        }
        let f = unsafe { ConvertThreadToFiber(std::ptr::null_mut()) };
        if f.is_null() {
            // 1280 = ERROR_ALREADY_FIBER: a host already made this thread a fiber and we cannot get its handle.
            return Err(exc("SystemError", "generators/async need a thread that is not already a fiber".into()));
        }
        CUR.with(|c| c.set(f));
        Ok(())
    }
    // `size` is the stack's reserve, like the Unix mmap: CreateFiber's one size is what it COMMITS up front, so
    // 16 MB each ran out of commit charge after a few hundred live generators. Commit 64 KB and let the stack
    // grow through its guard page; FIBER_FLAG_FLOAT_SWITCH (1) keeps each fiber's floating-point state its own.
    pub unsafe fn create(size: usize, start: unsafe extern "system" fn(*mut c_void), p: *mut c_void) -> H {
        CreateFiberEx(64 << 10, size, 1, start, p)
    }
    pub unsafe fn delete(h: H) {
        DeleteFiber(h)
    }
    pub unsafe fn switch(h: H) {
        SwitchToFiber(h)
    }
}

#[cfg(not(windows))]
extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32;
}
const CORO_STACK: usize = 16 << 20;
const CORO_GUARD: usize = 64 << 10;
#[cfg(target_os = "macos")]
const MAP_ANON_FLAGS: i32 = 0x0002 | 0x1000;
#[cfg(not(target_os = "macos"))]
const MAP_ANON_FLAGS: i32 = 0x0002 | 0x0020 | 0x4000;

pub struct Coro {
    // CPython's dotted __qualname__ of the generator function (__name__ is its last component)
    pub name: Sym,
    stack: *mut u8,
    sp: *mut u8,
    caller_sp: *mut u8,
    #[cfg(windows)]
    fiber: *mut std::ffi::c_void,
    #[cfg(windows)]
    caller_fiber: *mut std::ffi::c_void,
    body: Option<Box<dyn FnOnce() -> R>>,
    // A value passed across a switch: yielded out, or sent in.
    xfer: Option<V>,
    // An exception thrown in at the yield it is suspended at (throw(), a cancelled task).
    thrown: Option<E>,
    result: Option<R>,
    started: bool,
    running: bool,
    // Python frames this generator has open while suspended (counted again when it resumes).
    depth: usize,
    depth_base: usize,
    // its slot in CORO_LIVE (nulled when it ends, so removal does not scan the list)
    live_idx: usize,
    // the order it was started in: slots are reused, so close_live_gens sorts by this to close in creation
    // order exactly as before slots were reused
    live_seq: u64,
    // where its own code was when it last yielded: a warning issued in it names its line, never the caller's
    wsite: Option<ReSite>,
}

thread_local! {
    static CORO_NOW: RefCell<Vec<*mut Coro>> = RefCell::new(Vec::new());
    // Generators started and not yet finished or dropped, oldest first: closed at exit, as CPython closes
    // the ones still alive when it shuts down.
    static CORO_LIVE: RefCell<Vec<*mut Coro>> = RefCell::new(Vec::new());
    // Slots of CORO_LIVE nulled by a finished coroutine, reused by the next one to start. Without this the
    // list would grow by one entry for every coroutine ever started (8 bytes each, never reclaimed), and a
    // long-running process would leak without bound; with it the list stays at the high-water mark of
    // coroutines alive at once. A slot is only freed by the coroutine it holds (guarded by a pointer check),
    // so a slot can never be freed twice or freed while a live coroutine still occupies it.
    static CORO_LIVE_FREE: RefCell<Vec<usize>> = RefCell::new(Vec::new());
    // Start sequence numbers: only their order matters, to close suspended generators in creation order even
    // though finished ones free their CORO_LIVE slot for a later generator.
    static CORO_SEQ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static CORO_EXITING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // Stacks of finished coroutines, kept for the next one: mmap + munmap of a fresh 16 MB stack (with its
    // guard page) per short-lived coroutine is two syscalls plus page-table work, and asyncio's gather makes
    // one coroutine per child. A few are enough: concurrent tasks reuse them as fast as they are freed.
    static CORO_FREE: RefCell<Vec<*mut u8>> = RefCell::new(Vec::new());
    // The __qualname__ of the generator whose body is running now: repr()/__qualname__ of a generator called
    // from its own body cannot borrow its cells (they are held across the body), so read the name from here.
    static CUR_GEN_NAME: std::cell::Cell<Option<Sym>> = const { std::cell::Cell::new(None) };
}
const CORO_POOL_MAX: usize = 128;
#[cfg(not(windows))]
fn stack_alloc() -> Option<*mut u8> {
    if let Some(p) = CORO_FREE.with(|f| f.borrow_mut().pop()) {
        return Some(p);
    }
    let p = unsafe { mmap(std::ptr::null_mut(), CORO_STACK, 3, MAP_ANON_FLAGS, -1, 0) };
    if p.is_null() || p as isize == -1 {
        return None;
    }
    unsafe { mprotect(p, CORO_GUARD, 0) };
    Some(p)
}
#[cfg(not(windows))]
fn stack_free(p: *mut u8) {
    let kept = CORO_FREE.with(|f| {
        let mut f = f.borrow_mut();
        if f.len() < CORO_POOL_MAX {
            f.push(p);
            true
        } else {
            false
        }
    });
    if !kept {
        unsafe { munmap(p, CORO_STACK) };
    }
}
fn live_remove(c: *mut Coro) {
    let _ = CORO_LIVE.try_with(|l| {
        if let Ok(mut l) = l.try_borrow_mut() {
            let i = unsafe { (*c).live_idx };
            if l.get(i) == Some(&c) {
                l[i] = std::ptr::null_mut();
                CORO_LIVE_FREE.with(|f| f.borrow_mut().push(i));
            }
        }
    });
}
// At exit, before the output is flushed: every generator still suspended gets GeneratorExit, so its finally
// blocks and with exits run.
pub fn close_live_gens() {
    let mut live: Vec<(*mut Coro, usize, u64)> = CORO_LIVE.with(|l| {
        l.borrow()
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_null())
            .map(|(i, c)| (*c, i, unsafe { (**c).live_seq }))
            .collect()
    });
    live.sort_unstable_by_key(|t| t.2);
    for (c, i, _) in live {
        // a finally block closed (and possibly dropped) this generator while an earlier one was shutting down
        let still = CORO_LIVE.with(|l| l.borrow().get(i) == Some(&c));
        // one running in a thread that is still waiting (a daemon) is left as CPython leaves that thread's frames
        if still && !unsafe { (*c).running } {
            unsafe {
                if let Err(E(e)) = (*c).close_py() {
                    gen_unraisable(&e);
                }
            }
        }
    }
    #[cfg(not(windows))]
    for p in CORO_FREE.with(|f| f.borrow_mut().drain(..).collect::<Vec<_>>()) {
        unsafe { munmap(p, CORO_STACK) };
    }
    CORO_EXITING.with(|x| x.set(true));
}
fn gen_unraisable(e: &V) {
    err_write(&format!("Exception ignored while closing generator:\nTraceback (most recent call last):\n{}\n", exc_line(e)));
}

#[cfg(not(windows))]
extern "C" fn coro_main(c: *mut Coro) {
    unsafe {
        let body = (*c).body.take().unwrap();
        let r = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
            Ok(r) => r,
            Err(_) => Err(exc("SystemError", "a generator stopped with an internal error".into())),
        };
        (*c).result = Some(r);
        let mut dummy: *mut u8 = std::ptr::null_mut();
        brevi_ctx_switch(&mut dummy, (*c).caller_sp);
        std::process::abort();
    }
}

#[cfg(windows)]
unsafe extern "system" fn coro_fiber_proc(c: *mut std::ffi::c_void) {
    let c = c as *mut Coro;
    let body = (*c).body.take().unwrap();
    let r = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(r) => r,
        Err(_) => Err(exc("SystemError", "a generator stopped with an internal error".into())),
    };
    (*c).result = Some(r);
    let caller = (*c).caller_fiber;
    fiber::CUR.with(|x| x.set(caller));
    fiber::switch(caller);
    std::process::abort();
}

impl Coro {
    fn new(name: Sym, body: Box<dyn FnOnce() -> R>) -> Box<Coro> {
        Box::new(Coro {
            name,
            stack: std::ptr::null_mut(),
            sp: std::ptr::null_mut(),
            caller_sp: std::ptr::null_mut(),
            #[cfg(windows)]
            fiber: std::ptr::null_mut(),
            #[cfg(windows)]
            caller_fiber: std::ptr::null_mut(),
            body: Some(body),
            xfer: None,
            thrown: None,
            result: None,
            started: false,
            running: false,
            depth: 0,
            depth_base: 0,
            live_idx: 0,
            live_seq: 0,
            wsite: None,
        })
    }
    // Runs the generator to its next yield: Some(value), or None once it has returned.
    pub fn resume(&mut self, sent: V) -> Result<Option<V>, E> {
        // the generator runs at its own line and the caller gets its line back, however the switch ends
        let caller = re_site_get();
        re_site_put(self.wsite);
        let r = self.resume_in(sent);
        self.wsite = re_site_get();
        re_site_put(caller);
        r
    }
    fn resume_in(&mut self, sent: V) -> Result<Option<V>, E> {
        if self.result.is_some() {
            return Ok(None);
        }
        if self.running {
            return Err(exc("ValueError", "generator already executing".into()));
        }
        #[cfg(windows)]
        self.resume_fiber(sent)?;
        #[cfg(not(windows))]
        unsafe {
            if !self.started {
                let p = match stack_alloc() {
                    Some(p) => p,
                    None => return Err(exc("MemoryError", "no memory for a generator".into())),
                };
                self.stack = p;
                self.sp = coro_init_stack(p.add(CORO_STACK), self as *mut Coro);
                self.started = true;
                self.live_seq = CORO_SEQ.with(|s| {
                    let n = s.get();
                    s.set(n.wrapping_add(1));
                    n
                });
                let me = self as *mut Coro;
                CORO_LIVE.with(|l| {
                    let mut l = l.borrow_mut();
                    let i = CORO_LIVE_FREE.with(|f| f.borrow_mut().pop());
                    if let Some(i) = i {
                        l[i] = me;
                        self.live_idx = i;
                    } else {
                        self.live_idx = l.len();
                        l.push(me);
                    }
                });
            }
            self.xfer = Some(sent);
            self.running = true;
            self.depth_base = DEPTH.with(|d| d.get());
            DEPTH.with(|d| d.set(self.depth_base + self.depth));
            CORO_NOW.with(|s| s.borrow_mut().push(self as *mut Coro));
            CUR_GEN_NAME.with(|n| n.set(Some(self.name)));
            brevi_ctx_switch(&mut self.caller_sp, self.sp);
            CUR_GEN_NAME.with(|n| n.set(None));
            CORO_NOW.with(|s| s.borrow_mut().pop());
            self.running = false;
            self.depth = DEPTH.with(|d| d.get()).saturating_sub(self.depth_base);
            DEPTH.with(|d| d.set(self.depth_base));
        }
        match self.result.take() {
            None => Ok(Some(self.xfer.take().unwrap_or(V::None))),
            Some(r) => {
                self.result = Some(Ok(V::None));
                self.free_stack();
                live_remove(self as *mut Coro);
                match r {
                    Ok(V::None) => Ok(None),
                    Ok(v) => Err(stop_with(v)),
                    Err(e) => Err(e),
                }
            }
        }
    }
    #[cfg(windows)]
    fn resume_fiber(&mut self, sent: V) -> Result<(), E> {
        unsafe {
            if !self.started {
                fiber::ensure()?;
                let f = fiber::create(CORO_STACK, coro_fiber_proc, self as *mut Coro as *mut std::ffi::c_void);
                if f.is_null() {
                    return Err(exc("MemoryError", "no memory for a generator".into()));
                }
                self.fiber = f;
                self.started = true;
            }
            self.xfer = Some(sent);
            self.running = true;
            self.depth_base = DEPTH.with(|d| d.get());
            DEPTH.with(|d| d.set(self.depth_base + self.depth));
            let caller = fiber::CUR.with(|c| c.get());
            self.caller_fiber = caller;
            fiber::CUR.with(|c| c.set(self.fiber));
            CORO_NOW.with(|s| s.borrow_mut().push(self as *mut Coro));
            fiber::switch(self.fiber);
            CORO_NOW.with(|s| s.borrow_mut().pop());
            fiber::CUR.with(|c| c.set(caller));
            self.running = false;
            self.depth = DEPTH.with(|d| d.get()).saturating_sub(self.depth_base);
            DEPTH.with(|d| d.set(self.depth_base));
            Ok(())
        }
    }
    // throw(e): raised at the yield the generator is suspended at; one not started yet ends with it unrun.
    pub fn throw(&mut self, e: E) -> Result<Option<V>, E> {
        if self.result.is_some() || !self.started {
            if !self.running {
                self.result = Some(Ok(V::None));
                self.body = None;
            }
            return Err(e);
        }
        self.thrown = Some(e);
        self.resume(V::None)
    }
    pub fn is_suspended(&self) -> bool {
        self.started && self.result.is_none() && !self.running
    }
    // Ends it without running anything more (an async generator that is closed some other way).
    pub fn close(&mut self) {
        if !self.running {
            self.result = Some(Ok(V::None));
            self.free_stack();
            live_remove(self as *mut Coro);
        }
    }
    // Python's generator.close(): GeneratorExit raised at the yield it is suspended at, so its finally blocks
    // and with exits run. One that yields again is RuntimeError (and stays suspended); its return value is
    // close()'s (Python 3.13).
    pub fn close_py(&mut self) -> R {
        if self.running {
            return Err(exc("ValueError", "generator already executing".into()));
        }
        if !self.is_suspended() {
            self.close();
            return Ok(V::None);
        }
        match self.throw(exc0("GeneratorExit")) {
            Ok(Some(_)) => Err(exc("RuntimeError", "generator ignored GeneratorExit".into())),
            Ok(None) => Ok(V::None),
            Err(E(e)) if is_exc(&e, "GeneratorExit") => Ok(V::None),
            Err(E(e)) if is_exc(&e, "StopIteration") => Ok(exc_value(&e)),
            Err(e) => Err(e),
        }
    }
    fn free_stack(&mut self) {
        if !self.stack.is_null() {
            #[cfg(not(windows))]
            stack_free(self.stack);
            self.stack = std::ptr::null_mut();
        }
        #[cfg(windows)]
        if !self.fiber.is_null() {
            unsafe { fiber::delete(self.fiber) };
            self.fiber = std::ptr::null_mut();
        }
    }
}
impl Drop for Coro {
    // A generator dropped part way is closed, as CPython closes one when its last reference goes: its finally
    // blocks and with exits run, and an error from them is reported and ignored.
    fn drop(&mut self) {
        let alive = CORO_EXITING.try_with(|x| !x.get()).unwrap_or(false) && CORO_NOW.try_with(|_| ()).is_ok();
        if alive && self.is_suspended() && !std::thread::panicking() {
            if let Err(E(e)) = self.close_py() {
                gen_unraisable(&e);
            }
        }
        live_remove(self as *mut Coro);
        if !self.running {
            self.free_stack();
        }
    }
}

// `yield v` inside a generator body: hands v to next() and returns what send() passes back (None for next()).
pub fn gen_yield(v: V) -> R {
    #[cfg(windows)]
    {
        let c = CORO_NOW.with(|s| s.borrow().last().copied());
        let Some(c) = c else { return Err(exc("SystemError", "yield outside a generator".into())) };
        unsafe {
            (*c).xfer = Some(v);
            let caller = (*c).caller_fiber;
            fiber::CUR.with(|x| x.set(caller));
            fiber::switch(caller);
            fiber::CUR.with(|x| x.set((*c).fiber));
            if let Some(e) = (*c).thrown.take() {
                (*c).xfer = None;
                return Err(e);
            }
            Ok((*c).xfer.take().unwrap_or(V::None))
        }
    }
    #[cfg(not(windows))]
    {
        let c = CORO_NOW.with(|s| s.borrow().last().copied());
        let Some(c) = c else { return Err(exc("SystemError", "yield outside a generator".into())) };
        unsafe {
            (*c).xfer = Some(v);
            brevi_ctx_switch(&mut (*c).sp, (*c).caller_sp);
            if let Some(e) = (*c).thrown.take() {
                (*c).xfer = None;
                return Err(e);
            }
            Ok((*c).xfer.take().unwrap_or(V::None))
        }
    }
}

// `yield from it`: every item of it, then its return value.
pub fn gen_yield_from(it: &V) -> R {
    if let V::Iter(i) = it {
        if let Some(c) = coro_of(&i.it) {
            return delegate(&c);
        }
    }
    let mut i = iter(it)?;
    while let Some(x) = i.next()? {
        gen_yield(x)?;
    }
    Ok(V::None)
}

// A generator object: the function body, started at the first next().
pub fn make_gen(name: Sym, body: impl FnOnce() -> R + 'static) -> V {
    new_iter(It::Coro(Rc::new(RefCell::new(Coro::new(name, Box::new(body))))), IterClass::Kind(ItKind::Generator))
}
// The __qualname__ of the generator whose body is running now (None otherwise).
pub fn running_gen_name() -> Option<Sym> {
    CUR_GEN_NAME.with(|n| n.get())
}

pub fn coro_next(c: &Rc<RefCell<Box<Coro>>>, sent: V) -> Result<Option<V>, E> {
    let c = c.clone();
    let r = c.borrow_mut().resume(sent);
    match r {
        // A return value ends the loop as StopIteration does (it is only seen through yield from).
        Err(E(e)) if is_exc(&e, "StopIteration") => Ok(None),
        r => r,
    }
}

pub fn coro_of(it: &Rc<RefCell<It>>) -> Option<Rc<RefCell<Box<Coro>>>> {
    // an iterator mid-step (borrowed by its own next) is not a generator here: iter_step reports the re-entry
    match it.try_borrow().as_deref() {
        Ok(It::Coro(c)) => Some(c.clone()),
        _ => None,
    }
}

pub fn stop_with(v: V) -> E {
    let e = exc_v("StopIteration", v.clone());
    if let V::Obj(o) = &e.0 {
        o.set("value", v);
    }
    e
}
fn exc_value(e: &V) -> V {
    match e {
        V::Obj(o) => o.get("value").unwrap_or(V::None),
        _ => V::None,
    }
}
