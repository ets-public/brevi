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
#[cfg(all(target_arch = "x86_64", not(windows)))]
unsafe fn coro_init_stack(top: *mut u8, arg: *mut Coro) -> *mut u8 {
    // Popped by brevi_ctx_switch: r15 r14 r13 r12 rbx rbp, then the return address (the stub).
    let r = ((top as usize - 16) & !15) as *mut usize;
    *r = brevi_coro_stub as usize;
    let regs = r.sub(6);
    for k in 0..6 {
        *regs.add(k) = 0;
    }
    *regs.add(3) = arg as usize; // r12
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
unsafe fn coro_init_stack(top: *mut u8, arg: *mut Coro) -> *mut u8 {
    let sp = (((top as usize) & !15) - 160) as *mut usize;
    for k in 0..20 {
        *sp.add(k) = 0;
    }
    *sp = arg as usize; // x19
    *sp.add(11) = brevi_coro_stub as usize; // x30
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
        fn CreateFiber(size: usize, start: unsafe extern "system" fn(*mut c_void), p: *mut c_void) -> H;
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
    pub unsafe fn create(size: usize, start: unsafe extern "system" fn(*mut c_void), p: *mut c_void) -> H {
        CreateFiber(size, start, p)
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
}

thread_local! {
    static CORO_NOW: RefCell<Vec<*mut Coro>> = RefCell::new(Vec::new());
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
    fn new(body: Box<dyn FnOnce() -> R>) -> Box<Coro> {
        Box::new(Coro {
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
        })
    }
    // Runs the generator to its next yield: Some(value), or None once it has returned.
    pub fn resume(&mut self, sent: V) -> Result<Option<V>, E> {
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
                let p = mmap(std::ptr::null_mut(), CORO_STACK, 3, MAP_ANON_FLAGS, -1, 0);
                if p.is_null() || p as isize == -1 {
                    return Err(exc("MemoryError", "no memory for a generator".into()));
                }
                mprotect(p, CORO_GUARD, 0);
                self.stack = p;
                self.sp = coro_init_stack(p.add(CORO_STACK), self as *mut Coro);
                self.started = true;
            }
            self.xfer = Some(sent);
            self.running = true;
            self.depth_base = DEPTH.with(|d| d.get());
            DEPTH.with(|d| d.set(self.depth_base + self.depth));
            CORO_NOW.with(|s| s.borrow_mut().push(self as *mut Coro));
            brevi_ctx_switch(&mut self.caller_sp, self.sp);
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
    pub fn throw(&mut self, e: E) -> Result<Option<V>, E> {        if self.result.is_some() || !self.started {
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
    pub fn close(&mut self) {
        if !self.running {
            self.result = Some(Ok(V::None));
            self.free_stack();
        }
    }
    fn free_stack(&mut self) {
        if !self.stack.is_null() {
            #[cfg(not(windows))]
            unsafe {
                munmap(self.stack, CORO_STACK)
            };
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
    // A generator dropped part way: its stack is unmapped and what it held is not dropped (a small leak).
    fn drop(&mut self) {
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
        if let Some(c) = coro_of(i) {
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
pub fn make_gen(body: impl FnOnce() -> R + 'static) -> V {
    V::Iter(Rc::new(RefCell::new(It::Coro(Rc::new(RefCell::new(Coro::new(Box::new(body))))))))
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
    match &*it.borrow() {
        It::Coro(c) => Some(c.clone()),
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
