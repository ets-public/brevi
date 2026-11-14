// ---------- sys.settrace (rt_trace.rs) ----------
//
// CPython 3.13's legacy tracing (Python/legacy_tracing.c over sys.monitoring), for a program the build has
// checked (dynpy/trace_check.rs): only code bodies whose line events are known exactly are compiled, and each
// one calls these hooks. A frame entered while a trace function is set gets a "call" event sent to the global
// trace function; what that returns (unless None) is the frame's f_trace, which then receives "line",
// "return" and "exception" events while a global trace function is still set (trace_trampoline: a result of
// None keeps the old f_trace). Nothing is traced while a trace function runs, a frame entered with no trace
// function set is never traced, and a trace function that raises turns tracing off (settrace(None)) and
// clears the frame's f_trace, the exception then raised where the event was.
// A line event fires at an instruction that starts a line when the frame's previous instruction was on
// another line (the first line after the call always fires: the previous instruction is RESUME).

pub struct TraceCode {
    pub name: &'static str,
    pub first: i64,
    pub flags: u32,
    pub file: &'static str,
}

thread_local! {
    static TRACE_FN: RefCell<Option<V>> = const { RefCell::new(None) };
    static TRACING: Cell<bool> = const { Cell::new(false) };
    // set by a program the build compiled with trace hooks; any other program refuses sys.settrace
    static TRACE_BUILD: Cell<bool> = const { Cell::new(false) };
    // one code object per code body, as CPython has one per def (`f.__code__ is frame.f_code`)
    static TRACE_CODES: RefCell<Vec<(usize, V)>> = const { RefCell::new(Vec::new()) };
}

pub fn trace_build_on() {
    TRACE_BUILD.with(|c| c.set(true));
}

pub fn trace_code_v(tc: &'static TraceCode) -> V {
    let k = tc as *const TraceCode as usize;
    TRACE_CODES.with(|c| {
        if let Some((_, v)) = c.borrow().iter().find(|(a, _)| *a == k) {
            return v.clone();
        }
        let v = V::Ext(ExtRc::new(CodeObj { name: sym(tc.name), flags: tc.flags, first: tc.first }));
        c.borrow_mut().push((k, v.clone()));
        v
    })
}

// a def's function value with its code object (the one its frames name)
pub fn with_tcode(f: V, tc: &'static TraceCode) -> V {
    if let V::Func(x) = &f {
        x.tcode.set(Some(tc));
    }
    f
}

pub struct FrameObj {
    code: &'static TraceCode,
    line: Cell<i64>,
    // the line of the frame's previous instruction (-1: RESUME, before the first)
    last: Cell<i64>,
    ftrace: RefCell<Option<V>>,
    // the exception this frame last sent an "exception" event for (a re-raise, as after a finally, sends none)
    last_exc: RefCell<Option<V>>,
}
impl Ext for FrameObj {
    fn tname(&self) -> &'static str {
        "frame"
    }
    fn repr(&self) -> String {
        format!("<frame at {:#x}, file '{}', line {}, code {}>", self as *const FrameObj as usize, self.code.file, self.line.get(), self.code.name)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        Some(match n {
            "f_code" => Ok(trace_code_v(self.code)),
            "f_lineno" => Ok(V::Int(self.line.get())),
            _ => Err(exc("AttributeError", format!("'frame' object has no attribute '{n}'"))),
        })
    }
}

// the traceback an "exception" event passes (the build refuses reading anything from it)
struct TraceTb;
impl Ext for TraceTb {
    fn tname(&self) -> &'static str {
        "traceback"
    }
    fn repr(&self) -> String {
        format!("<traceback object at {:#x}>", self as *const TraceTb as usize)
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}

pub type TF = Option<V>;

fn fobj(fr: &V) -> &FrameObj {
    match fr {
        V::Ext(x) => x.any().downcast_ref::<FrameObj>().expect("a trace frame"),
        _ => unreachable!(),
    }
}

fn trace_cb(f: &V, fr: &V, ev: &str, arg: V) -> R {
    TRACING.with(|t| t.set(true));
    let r = call(f, Args::p(vec![fr.clone(), s(ev), arg]));
    TRACING.with(|t| t.set(false));
    r
}

fn trace_off() {
    TRACE_FN.with(|t| *t.borrow_mut() = None);
}

// a code body starts: the "call" event, when a trace function is set
pub fn trace_enter(tc: &'static TraceCode) -> Result<TF, E> {
    if TRACING.with(|t| t.get()) {
        return Ok(None);
    }
    let Some(g) = TRACE_FN.with(|t| t.borrow().clone()) else { return Ok(None) };
    let fr = V::Ext(ExtRc::new(FrameObj { code: tc, line: Cell::new(tc.first), last: Cell::new(-1), ftrace: RefCell::new(None), last_exc: RefCell::new(None) }));
    match trace_cb(&g, &fr, "call", V::None) {
        Ok(V::None) => {}
        Ok(r) => *fobj(&fr).ftrace.borrow_mut() = Some(r),
        Err(e) => {
            trace_off();
            return Err(e);
        }
    }
    Ok(Some(fr))
}

fn deliver(fr: &V, ev: &str, arg: V) -> Result<(), E> {
    if TRACING.with(|t| t.get()) || TRACE_FN.with(|t| t.borrow().is_none()) {
        return Ok(());
    }
    let fo = fobj(fr);
    let Some(cb) = fo.ftrace.borrow().clone() else { return Ok(()) };
    match trace_cb(&cb, fr, ev, arg) {
        Ok(V::None) => Ok(()),
        Ok(r) => {
            *fo.ftrace.borrow_mut() = Some(r);
            Ok(())
        }
        Err(e) => {
            trace_off();
            *fo.ftrace.borrow_mut() = None;
            Err(e)
        }
    }
}

// an instruction starting `line` runs
pub fn trace_line(tf: &TF, line: i64) -> R {
    if let Some(fr) = tf {
        let fo = fobj(fr);
        fo.line.set(line);
        if fo.last.get() != line {
            fo.last.set(line);
            deliver(fr, "line", V::None)?;
        }
    }
    Ok(V::None)
}

fn trace_exc(fr: &V, e: &E) -> Result<(), E> {
    let fo = fobj(fr);
    if fo.last_exc.borrow().as_ref().is_some_and(|x| is(x, &e.0)) {
        return Ok(());
    }
    *fo.last_exc.borrow_mut() = Some(e.0.clone());
    let t = V::Class(type_of(&e.0)?);
    deliver(fr, "exception", tup(vec![t, e.0.clone(), V::Ext(ExtRc::new(TraceTb))]))
}

// an exception reaches a finally block of a traced frame (CPython's "exception" event fires where it is raised,
// before the finally runs)
pub fn trace_fin(tf: &TF, fl: Flow) -> Flow {
    if let (Some(fr), Flow::Exc(e)) = (tf, &fl) {
        if let Err(e2) = trace_exc(fr, e) {
            return Flow::Exc(e2);
        }
    }
    fl
}

// a code body ends: the "return" event (with None when an exception leaves it, after its "exception" event)
pub fn trace_leave(tf: &TF, r: R) -> R {
    let Some(fr) = tf else { return r };
    match r {
        Ok(v) => {
            deliver(fr, "return", v.clone())?;
            Ok(v)
        }
        Err(e) => {
            trace_exc(fr, &e)?;
            deliver(fr, "return", V::None)?;
            Err(e)
        }
    }
}

pub fn sys_settrace(a: Args) -> R {
    if !TRACE_BUILD.with(|c| c.get()) {
        return Err(unsupported("sys.settrace"));
    }
    if !a.k.is_empty() {
        return type_err("sys.settrace() takes no keyword arguments".into());
    }
    if a.p.len() != 1 {
        return type_err(format!("sys.settrace() takes exactly one argument ({} given)", a.p.len()));
    }
    let f = a.p.into_iter().next().unwrap();
    TRACE_FN.with(|t| *t.borrow_mut() = if matches!(f, V::None) { None } else { Some(f) });
    Ok(V::None)
}

pub fn sys_gettrace(a: Args) -> R {
    if !TRACE_BUILD.with(|c| c.get()) {
        return Err(unsupported("sys.gettrace"));
    }
    if !a.k.is_empty() {
        return type_err("sys.gettrace() takes no keyword arguments".into());
    }
    if !a.p.is_empty() {
        return type_err(format!("sys.gettrace() takes no arguments ({} given)", a.p.len()));
    }
    Ok(TRACE_FN.with(|t| t.borrow().clone()).unwrap_or(V::None))
}
