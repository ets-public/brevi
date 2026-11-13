
// ---------- stdin / stdout ----------

pub struct Out {
    buf: Vec<u8>,
    tty: bool,
}
thread_local! {
    pub static OUT: RefCell<Out> = RefCell::new(Out { buf: Vec::with_capacity(8192), tty: { use std::io::IsTerminal; std::io::stdout().is_terminal() } });
    pub static STDIN: RefCell<Option<std::io::BufReader<std::io::Stdin>>> = RefCell::new(None);
    pub static ARGV: RefCell<Vec<String>> = RefCell::new(vec![]);
}
// sys.stdout as the program set it (`sys.stdout = io.StringIO()`, as test.support.captured_output does): None while
// it is the real stream. print() and input() write to it, and reading sys.stdout gives it back, as in CPython.
// Only for programs that run nothing in CPython: a bridged library would still print to the real stream.
thread_local! {
    pub static STDOUT_SET: RefCell<Option<V>> = const { RefCell::new(None) };
}
static BRIDGED_RUN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_bridged() {
    BRIDGED_RUN.store(true, std::sync::atomic::Ordering::Relaxed);
}
pub fn stdout_set() -> Option<V> {
    STDOUT_SET.with(|s| s.borrow().clone())
}
pub fn sys_stream_assign(name: &str, v: V) -> Result<(), E> {
    match name {
        "stdout" => {
            if BRIDGED_RUN.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(unsupported("assigning sys.stdout in a program that runs a library in CPython"));
            }
            out_flush();
            let real = matches!(v, V::Module(&ModName("sys.stdout")));
            STDOUT_SET.with(|s| *s.borrow_mut() = if real { None } else { Some(v) });
            Ok(())
        }
        _ => Err(unsupported(&format!("assigning sys.{name}"))),
    }
}
pub fn out_write(t: &str) {
    // CPython's text streams on Windows write CRLF; match them (every LF, as its TextIOWrapper does).
    #[cfg(windows)]
    let t = t.replace('\n', "\r\n");
    #[cfg(windows)]
    let t = t.as_str();
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        if o.buf.len() + t.len() > 8192 {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            if t.len() >= 8192 {
                std::io::stdout().write_all(t.as_bytes()).ok();
                std::io::stdout().flush().ok();
                return;
            }
        }
        o.buf.extend_from_slice(t.as_bytes());
        if o.tty && t.contains('\n') {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            std::io::stdout().flush().ok();
        }
    });
}
pub fn out_flush() {
    // try_with: a library value dropped by another thread-local's destructor at thread exit (a module CPython
    // runs, kept in sys.modules) flushes after OUT itself is gone, when there is nothing left to flush
    let _ = OUT.try_with(|o| {
        let mut o = o.borrow_mut();
        let b = std::mem::take(&mut o.buf);
        std::io::stdout().write_all(&b).ok();
        std::io::stdout().flush().ok();
    });
}
pub fn err_write(t: &str) {
    #[cfg(windows)]
    let t = t.replace('\n', "\r\n");
    #[cfg(windows)]
    let t = t.as_str();
    std::io::stderr().write_all(t.as_bytes()).ok();
}
fn with_stdin<T>(f: impl FnOnce(&mut std::io::BufReader<std::io::Stdin>) -> T) -> T {
    STDIN.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = Some(std::io::BufReader::new(std::io::stdin()));
        }
        f(s.as_mut().unwrap())
    })
}
pub fn read_line() -> Option<String> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) => None,
            Ok(_) => Some(String::from_utf8_lossy(&buf).into_owned()),
            Err(_) => None,
        }
    })
}
pub fn read_line_bytes() -> Option<Vec<u8>> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(buf),
        }
    })
}
pub fn read_all_bytes() -> Vec<u8> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).ok();
        buf
    })
}
pub fn read_n_bytes(n: usize) -> Vec<u8> {
    with_stdin(|r| {
        let mut buf = Vec::new();
        std::io::Read::take(r, n as u64).read_to_end(&mut buf).ok();
        buf
    })
}
// sys.stdout.buffer.write: raw bytes through the same buffer as text writes, so the two stay in order.
pub fn out_write_bytes(b: &[u8]) {
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.buf.extend_from_slice(b);
        if o.buf.len() > 8192 || (o.tty && b.contains(&b'\n')) {
            let b = std::mem::take(&mut o.buf);
            std::io::stdout().write_all(&b).ok();
            std::io::stdout().flush().ok();
        }
    });
}
pub fn read_all() -> String {
    with_stdin(|r| {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).ok();
        String::from_utf8_lossy(&buf).into_owned()
    })
}
fn file_target(f: &V) -> Result<u8, E> {
    match f {
        V::None => Ok(1),
        V::Module(&ModName("sys.stdout")) => Ok(1),
        V::Module(&ModName("sys.stderr")) => Ok(2),
        V::Obj(_) | V::Ext(_) => Ok(3),
        _ => Err(unsupported("print(file=...) to this file")),
    }
}
// What stderr shows for a str: CPython's stderr writes a lone surrogate with errors='backslashreplace'.
pub fn err_text(x: &S) -> std::borrow::Cow<'_, str> {
    if !x.is_lone() {
        return std::borrow::Cow::Borrowed(x.text());
    }
    let b = str_encode(x, None, Some(&s("backslashreplace"))).ok().and_then(|b| bytes_of(&b)).unwrap_or_default();
    std::borrow::Cow::Owned(String::from_utf8(b).unwrap_or_default())
}
// The error handler python3's stdout would encode with, when Brevi can name it (None: a lone surrogate at stdout
// stops). Naming it reads the environment (the locale, PYTHONUTF8, PYTHONIOENCODING), which the runtime never does
// itself (caps.rs tests it, for --sandbox): outside the sandbox dynpy::to_rust sets the reader (STDOUT_ERRS_FN in
// mod.rs); under it the hook stays unset and the program stops instead of guessing.
pub static STDOUT_ERRS: std::sync::OnceLock<fn() -> Option<&'static str>> = std::sync::OnceLock::new();
fn stdout_errors() -> Option<&'static str> {
    static ERRS: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *ERRS.get_or_init(|| STDOUT_ERRS.get().and_then(|f| f()))
}
// print/sys.stdout.write of a str with a lone surrogate: python3's stdout encodes it (stdout_errors) and raises
// UnicodeEncodeError if it cannot; Ok(bytes) are written as they are (surrogateescape gives back raw bytes).
pub fn out_lone(x: &S) -> Result<Vec<u8>, E> {
    let Some(errs) = stdout_errors() else { lone_stop(std::panic::Location::caller()) };
    let b = str_encode(x, None, Some(&s(errs)))?;
    Ok(bytes_of(&b).unwrap_or_default())
}
pub fn bi_print(mut a: Args) -> R {
    let sep = match a.kw("sep") {
        Some(V::None) | None => s(" "),
        Some(x @ V::Str(_)) => x,
        Some(x) => return type_err(format!("sep must be None or a string, not {}", tn(&x))),
    };
    let end = match a.kw("end") {
        Some(V::None) | None => s("\n"),
        Some(x @ V::Str(_)) => x,
        Some(x) => return type_err(format!("end must be None or a string, not {}", tn(&x))),
    };
    let file = match a.kw("file").unwrap_or(V::None) {
        V::None => stdout_set().unwrap_or(V::None),
        f => f,
    };
    let flush = a.kw("flush").map(|f| truth(&f).unwrap_or(false)).unwrap_or(false);
    a.no_kw("print")?;
    // the pieces in CPython's order (arg, sep, arg, ..., end), each str() taken only when it is reached, so an error
    // in str() of a later argument (or a lone surrogate stdout cannot encode) comes after the earlier pieces
    let n = a.p.len();
    // k = 0, 2, 4, ...: the arguments; the odd k between them: sep; the last k: end
    let piece = |k: usize| -> R {
        if k + 1 == (2 * n).max(1) {
            return Ok(end.clone());
        }
        if k % 2 == 1 {
            return Ok(sep.clone());
        }
        match &a.p[k / 2] {
            x @ V::Str(_) => Ok(x.clone()),
            x => Ok(st(to_str(x)?)),
        }
    };
    let ks = 0..(2 * n).max(1);
    let target = file_target(&file)?;
    // a file object: one write() per piece and then flush()
    if target == 3 {
        for k in ks {
            call_method(&file, "write", Args::p(vec![piece(k)?]))?;
        }
        if flush {
            call_method(&file, "flush", Args::p(vec![]))?;
        }
        return Ok(V::None);
    }
    let mut t = String::new();
    for k in ks {
        let p = piece(k);
        let p = match p {
            Ok(p) => p,
            Err(e) => {
                if target == 1 {
                    out_write(&t);
                }
                return Err(e);
            }
        };
        let V::Str(x) = &p else { unreachable!() };
        if x.is_lone() {
            match target {
                1 => {
                    out_write(&t);
                    t.clear();
                    out_write_bytes(&out_lone(x)?);
                }
                _ => t.push_str(&err_text(x)),
            }
        } else {
            t.push_str(x.text());
        }
    }
    match target {
        1 => {
            out_write(&t);
            if flush {
                out_flush();
            }
        }
        _ => err_write(&t),
    }
    Ok(V::None)
}
pub fn bi_input(a: Args) -> R {
    // a replaced sys.stdout gets the prompt and a flush, as CPython's input() falls back to it
    if let Some(o) = stdout_set() {
        if let Some(p) = a.p.first() {
            call_method(&o, "write", Args::p(vec![st(to_str(p)?)]))?;
        }
        call_method(&o, "flush", Args::p(vec![]))?;
    } else {
        if let Some(p) = a.p.first() {
            out_write(&to_str(p)?);
        }
        out_flush();
    }
    match read_line() {
        None => Err(exc("EOFError", "EOF when reading a line".into())),
        Some(mut l) => {
            if l.ends_with('\n') {
                l.pop();
                if l.ends_with('\r') {
                    // Python keeps \r on Unix when reading text with universal newlines translated
                }
            }
            Ok(st(l))
        }
    }
}
pub fn stdin_lines() -> V {
    gen_iter(ItKind::Unknown, || Ok(read_line().map(st)))
}

// ---------- builtin functions ----------

pub fn a1(a: &Args, f: &str) -> Result<V, E> {
    a.want(f, 1, 1)?;
    Ok(a.p[0].clone())
}
pub fn bi_len(a: Args) -> R {
    a.no_kw("len")?;
    Ok(V::Int(len(&a1(&a, "len")?)? as i64))
}
pub fn bi_repr(a: Args) -> R {
    Ok(st(repr(&a1(&a, "repr")?)?))
}
pub fn bi_ascii(a: Args) -> R {
    let r = repr(&a1(&a, "ascii")?)?;
    let mut o = String::new();
    for c in r.chars() {
        if (c as u32) < 128 {
            o.push(c);
        } else if (c as u32) < 256 {
            o.push_str(&format!("\\x{:02x}", c as u32));
        } else if (c as u32) < 65536 {
            o.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            o.push_str(&format!("\\U{:08x}", c as u32));
        }
    }
    Ok(st(o))
}
pub fn bi_str(mut a: Args) -> R {
    if a.p.is_empty() {
        return Ok(s(""));
    }
    if a.p.len() > 1 || !a.k.is_empty() {
        return Err(unsupported("str() with an encoding"));
    }
    str_v(&a.p[0])
}
pub fn bi_int(mut a: Args) -> R {
    let base = a.kw("base");
    a.no_kw("int")?;
    if a.p.len() > 2 {
        return type_err(format!("int() takes at most 2 arguments ({} given)", a.p.len()));
    }
    if a.p.is_empty() {
        if base.is_some() {
            return type_err("int() missing string argument".into());
        }
        return Ok(V::Int(0));
    }
    // int(a library value): CPython's int decides (its __int__ or __index__ first, then its buffer, as for
    // int(array('B', b'42')), and "can't convert non-string with explicit base" with a base)
    if let V::Ext(x) = &a.p[0] {
        if x.is_py() {
            if let Some(f) = PY_BUILTIN.with(|h| h.get()) {
                let mut p = a.p.clone();
                if let (1, Some(b)) = (p.len(), &base) {
                    p.push(b.clone());
                }
                return call(&f("int")?, Args::p(p));
            }
        }
    }
    let base = match (a.p.get(1).cloned(), base) {
        // `int("0", base=2 ** 234)`: out of range, as any base past 36.
        (Some(V::Big(_)), _) | (None, Some(V::Big(_))) => Some(i64::MAX),
        (Some(b), _) | (None, Some(b)) => Some(idx_of(&b)?),
        _ => None,
    };
    // with a base, an instance of a str, bytes or bytearray subclass is read as its value (CPython never
    // calls its __int__ or __index__ then)
    let x0 = match &a.p[0] {
        V::Obj(o) if base.is_some() => match o.inner() {
            Some(i @ V::Str(_)) => i,
            Some(i) if matches!(o.cls.base_bt(), BT::Bytes | BT::ByteArray) => i,
            _ => a.p[0].clone(),
        },
        x => x.clone(),
    };
    match (base, &x0) {
        (None, x) => to_int(x),
        (Some(b), V::Str(x)) => {
            if b != 0 && !(2..=36).contains(&b) {
                return val_err("int() base must be >= 2 and <= 36, or 0".into());
            }
            let t = x.text_sub('?');
            int_text_limit(&t, b as u32, parse_int(&t, b as u32))?.ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base {}: {}", b, repr_s(x))))
        }
        (Some(b), V::Ext(x)) if x.any().is::<BytesV>() || x.any().is::<ByteArrayV>() => {
            if b != 0 && !(2..=36).contains(&b) {
                return val_err("int() base must be >= 2 and <= 36, or 0".into());
            }
            bytes_int(&bytes_of(&x0).unwrap_or_default(), b as u32)
        }
        (Some(_), _) => type_err("int() can't convert non-string with explicit base".into()),
    }
}
// IntEnum and IntFlag members given to a numeric builtin (round, abs, range, ...): their int.
fn enum_nums(mut a: Args) -> Args {
    for x in a.p.iter_mut() {
        if let V::Obj(o) = x {
            if is_enum(&o.cls) {
                if let Some(i @ (V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_))) = inner_of(x) {
                    *x = i;
                }
            }
        }
    }
    a
}
pub fn bi_float(a: Args) -> R {
    let a = enum_nums(a);
    a.no_kw("float")?;
    if a.p.is_empty() {
        return Ok(V::Float(0.0));
    }
    // float(a library value): CPython's float decides, with its own errors (a buffer that is not a number is a
    // ValueError there)
    if let V::Ext(x) = &a.p[0] {
        if x.is_py() {
            if let Some(f) = PY_BUILTIN.with(|h| h.get()) {
                return call(&f("float")?, Args::p(a.p.clone()));
            }
        }
    }
    Ok(V::Float(to_float(&a.p[0])?))
}
pub fn bi_bool(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(V::Bool(false));
    }
    Ok(V::Bool(truth(&a.p[0])?))
}
pub fn bi_list(a: Args) -> R {
    if !a.k.is_empty() {
        return type_err("list() takes no keyword arguments".into());
    }
    if a.p.is_empty() {
        return Ok(list(vec![]));
    }
    Ok(list(to_vec(&a.p[0])?))
}
pub fn bi_tuple(a: Args) -> R {
    if a.p.is_empty() {
        return Ok(tup(vec![]));
    }
    if let V::Tuple(_) = &a.p[0] {
        return Ok(a.p[0].clone());
    }
    Ok(tup(to_vec(&a.p[0])?))
}
pub fn bi_set(a: Args) -> R {
    set_args("set", &a)?;
    if a.p.is_empty() {
        return Ok(set_new());
    }
    match &a.p[0] {
        V::Set(_) | V::FSet(_) => Ok(V::Set(Rc::new(RefCell::new(set_of(&a.p[0]).unwrap().copy())))),
        // CPython's set_update_dict: the dict's stored hashes, no key is hashed again
        V::Dict(d) => {
            let ks = d.borrow().keys_h();
            Ok(V::Set(Rc::new(RefCell::new(PSet::from_dict(ks)?))))
        }
        x => Ok(V::Set(Rc::new(RefCell::new(PSet::from_iter(to_vec(x)?)?)))),
    }
}
// set() and frozenset() take at most one argument, and none by keyword
fn set_args(name: &str, a: &Args) -> Result<(), E> {
    if !a.k.is_empty() {
        return type_err(format!("{name}() takes no keyword arguments"));
    }
    if a.p.len() > 1 {
        return type_err(format!("{name} expected at most 1 argument, got {}", a.p.len()));
    }
    Ok(())
}
pub fn bi_frozenset(a: Args) -> R {
    set_args("frozenset", &a)?;
    if a.p.is_empty() {
        return Ok(V::FSet(Rc::new(PSet::new())));
    }
    match bi_set(a)? {
        V::Set(s) => Ok(V::FSet(Rc::new(s.borrow().clone()))),
        _ => unreachable!(),
    }
}
pub fn bi_dict(mut a: Args) -> R {
    let mut d = Dict::new();
    if let Some(x) = a.p.first() {
        for (k, v) in dict_items_of(x)? {
            d.set(k, v)?;
        }
    }
    for (k, v) in a.k.drain(..) {
        d.set(s(k), v)?;
    }
    Ok(V::Dict(Rc::new(RefCell::new(d))))
}
pub fn bi_range(a: Args) -> R {
    let a = enum_nums(a);
    a.no_kw("range")?;
    if (1..=3).contains(&a.p.len()) && a.p.iter().any(|x| matches!(x, V::Big(_))) {
        let mut bs = Vec::new();
        for x in &a.p {
            match big_of(x) {
                Some(b) => bs.push(b),
                None => return type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
            }
        }
        let (s0, e, st) = match bs.len() {
            1 => (Big::from_i64(0), bs[0].clone(), Big::from_i64(1)),
            2 => (bs[0].clone(), bs[1].clone(), Big::from_i64(1)),
            _ => (bs[0].clone(), bs[1].clone(), bs[2].clone()),
        };
        if st.is_zero() {
            return val_err("range() arg 3 must not be zero".into());
        }
        return Ok(bigrange_v(s0, e, st));
    }
    let g = |i: usize| -> Result<i64, E> {
        match &a.p[i] {
            V::Int(x) => Ok(*x),
            V::Bool(b) => Ok(*b as i64),
            V::Big(_) => Err(unsupported("range() over very large integers")),
            x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
        }
    };
    let (s0, e, st) = match a.p.len() {
        1 => (0, g(0)?, 1),
        2 => (g(0)?, g(1)?, 1),
        3 => (g(0)?, g(1)?, g(2)?),
        0 => return type_err("range expected at least 1 argument, got 0".into()),
        n => return type_err(format!("range expected at most 3 arguments, got {n}")),
    };
    if st == 0 {
        return val_err("range() arg 3 must not be zero".into());
    }
    Ok(V::Range(Rc::new((s0, e, st))))
}
pub fn bi_abs(a: Args) -> R {
    let a = enum_nums(a);
    let x = a1(&a, "abs")?;
    if let Some(p) = proxy_deref(&x) {
        return bi_abs(Args::p(vec![p?]));
    }
    match &x {
        V::Int(i) => Ok(match i.checked_abs() {
            Some(r) => V::Int(r),
            None => Big::from_i64(*i).neg().norm(),
        }),
        V::Bool(b) => Ok(V::Int(*b as i64)),
        V::Big(b) => Ok(Big { neg: false, mag: b.mag.clone() }.norm()),
        V::Float(f) => Ok(V::Float(fresh_float(f.abs()))),
        V::Obj(o) => match o.cls.lookup("__abs__") {
            Some(f) => call(&f, Args::p(vec![x.clone()])),
            None => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
        },
        V::Ext(e) => match e.method(&x, "__abs__", &mut Args::p(vec![])) {
            Some(r) => r,
            None => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
        },
        _ => type_err(format!("bad operand type for abs(): '{}'", tn(&x))),
    }
}
fn minmax(mut a: Args, want_max: bool) -> R {
    let name = if want_max { "max" } else { "min" };
    let key = a.kw("key");
    let dflt = a.kw("default");
    a.no_kw(name)?;
    let items = if a.p.len() == 1 {
        to_vec(&a.p[0])?
    } else if a.p.is_empty() {
        return type_err(format!("{name} expected at least 1 argument, got 0"));
    } else {
        if dflt.is_some() {
            return type_err(format!("Cannot specify a default for {name}() with multiple positional arguments"));
        }
        a.p.clone()
    };
    if items.is_empty() {
        return match dflt {
            Some(d) => Ok(d),
            None => val_err(format!("{name}() iterable argument is empty")),
        };
    }
    let key = key.filter(|k| !matches!(k, V::None));
    let kf = |x: &V| -> R {
        match &key {
            Some(k) => call(k, Args::p(vec![x.clone()])),
            None => Ok(x.clone()),
        }
    };
    let mut best = items[0].clone();
    let mut bk = kf(&best)?;
    for x in items.into_iter().skip(1) {
        let k = kf(&x)?;
        let better = if want_max { rich(&k, &bk, ">")? } else { rich(&k, &bk, "<")? };
        if better {
            best = x;
            bk = k;
        }
    }
    Ok(best)
}
pub fn bi_min(a: Args) -> R {
    minmax(a, false)
}
pub fn bi_max(a: Args) -> R {
    minmax(a, true)
}
pub fn bi_sum(mut a: Args) -> R {
    let start = a.kw("start");
    let mut acc = a.p.get(1).cloned().or(start).unwrap_or(V::Int(0));
    if let V::Str(_) = acc {
        return type_err("sum() can't sum strings [use ''.join(seq) instead]".into());
    }
    let xs = to_vec(a.p.first().ok_or_else(|| exc("TypeError", "sum() takes at least 1 positional argument (0 given)".into()))?)?;
    // float sums use Neumaier-free plain addition in order, as CPython does for mixed; CPython 3.12+ uses
    // compensated summation for floats.
    let all_float = xs.iter().all(|x| matches!(x, V::Float(_) | V::Int(_))) && xs.iter().any(|x| matches!(x, V::Float(_))) && matches!(acc, V::Int(_) | V::Float(_));
    if all_float {
        // CPython 3.12+: compensated (Neumaier) summation once a float appears
        let mut i_acc: Option<i64> = None;
        let mut f = 0.0f64;
        let mut c = 0.0f64;
        let mut in_float = false;
        let mut ip = match acc {
            V::Int(i) => i,
            _ => 0,
        };
        if let V::Float(x) = acc {
            f = x;
            in_float = true;
        }
        for x in &xs {
            match x {
                V::Int(i) if !in_float => match ip.checked_add(*i) {
                    Some(r) => ip = r,
                    None => return Err(unsupported("very large integer sums mixed with floats")),
                },
                V::Int(i) => {
                    let y = *i as f64;
                    let t = f + y;
                    if f.abs() >= y.abs() {
                        c += (f - t) + y;
                    } else {
                        c += (y - t) + f;
                    }
                    f = t;
                }
                V::Float(y) => {
                    if !in_float {
                        in_float = true;
                        f = ip as f64;
                    }
                    let y = *y;
                    let t = f + y;
                    if f.abs() >= y.abs() {
                        c += (f - t) + y;
                    } else {
                        c += (y - t) + f;
                    }
                    f = t;
                }
                _ => {}
            }
        }
        let _ = i_acc;
        if c != 0.0 && c.is_finite() {
            f += c;
        }
        return Ok(V::Float(fresh_float(f)));
    }
    for x in xs {
        acc = add(&acc, &x)?;
    }
    Ok(acc)
}
pub fn bi_sorted(mut a: Args) -> R {
    let key = a.kw("key");
    let rev = a.kw("reverse").map(|r| truth(&r)).transpose()?.unwrap_or(false);
    a.no_kw("sorted")?;
    let mut v = to_vec(&a1(&a, "sorted")?)?;
    sort_values(&mut v, key.as_ref(), rev)?;
    Ok(list(v))
}
pub fn bi_reversed(a: Args) -> R {
    let x = a1(&a, "reversed")?;
    if let V::Obj(o) = &x {
        if let Some(f) = o.cls.lookup("__reversed__") {
            return call(&f, Args::p(vec![x.clone()]));
        }
    }
    if bigrange_of(&x).is_some() {
        return call_method(&x, "__reversed__", Args::p(vec![]));
    }
    if let V::Range(r) = &x {
        let n = range_len(&x) as i64;
        let last = r.0 + (n - 1) * r.2;
        // CPython falls back to its long range iterator when start - step leaves the C long range
        let (Some(stop), Some(neg)) = (r.0.checked_sub(r.2), r.2.checked_neg()) else {
            let b = bigrange_v(Big::from_i64(r.0), Big::from_i64(r.1), Big::from_i64(r.2));
            return call_method(&b, "__reversed__", Args::p(vec![]));
        };
        return Ok(new_iter(It::Range(last, stop, neg), IterClass::Kind(ItKind::RangeIter)));
    }
    // list, dict and dict views have their own reverse iterators; every other reversible sequence gets
    // the generic `reversed` object, as in CPython.
    let kind = match &x {
        V::List(_) => ItKind::ListReverseIter,
        V::Dict(_) => ItKind::DictReverseKeys,
        V::View(v) => match v.1 {
            0 | 3 => ItKind::DictReverseKeys,
            1 => ItKind::DictReverseValues,
            2 => ItKind::DictReverseItems,
            _ => ItKind::Reversed,
        },
        V::Obj(_) => match inner_of(&x) {
            Some(V::List(_)) => ItKind::ListReverseIter,
            Some(V::Dict(_)) => ItKind::DictReverseKeys,
            _ => ItKind::Reversed,
        },
        _ => ItKind::Reversed,
    };
    if let V::List(l) = &x {
        let n = l.borrow().len();
        return Ok(new_iter(It::RevList(l.clone(), n), IterClass::Kind(kind)));
    }
    if let V::Deque(d) = &x {
        return Ok(new_iter(deque_iter(d, true), IterClass::Kind(kind)));
    }
    // a dict (not an OrderedDict, whose reverse iterator is its own), its views, or a dict subclass instance:
    // live, as CPython's dict reverse iterators
    let dict_of = match &x {
        V::Dict(d) => Some((d.clone(), 0u8)),
        V::View(v) if v.1 <= 3 => match &v.0 {
            V::Dict(d) => Some((d.clone(), v.1)),
            _ => None,
        },
        V::Obj(_) => match inner_of(&x) {
            Some(V::Dict(d)) => Some((d.clone(), 0)),
            _ => None,
        },
        _ => None,
    };
    if let Some((d, vk)) = dict_of {
        // an OrderedDict's own reverse iterator: live, as CPython's odict_iterator
        if vk <= 2 && d.borrow().kind == 3 {
            return Ok(new_iter(odict_iter(&d, vk | 4), IterClass::Kind(ItKind::OdictIter)));
        }
        let (n, used, ver, odict) = {
            let b = d.borrow();
            (b.ents.len(), b.n as i64, b.ver, b.kind == 3)
        };
        if !odict && matches!(kind, ItKind::DictReverseKeys | ItKind::DictReverseValues | ItKind::DictReverseItems) {
            return Ok(new_iter(It::RevDict(d, n, vk, used, ver), IterClass::Kind(kind)));
        }
    }
    if matches!(x, V::Dict(_) | V::List(_) | V::Tuple(_) | V::Str(_) | V::Deque(_) | V::View(_)) || inner_of(&x).is_some() {
        let mut v = to_vec(&x)?;
        v.reverse();
        return Ok(vec_iter(kind, v));
    }
    type_err(format!("'{}' object is not reversible", proxy_tn(&x)))
}
pub fn bi_enumerate(mut a: Args) -> R {
    let start = a.kw("start");
    let k = match a.p.get(1).cloned().or(start) {
        Some(x @ V::Big(_)) => x,
        Some(x) => V::Int(idx_of(&x)?),
        None => V::Int(0),
    };
    let it = get_iter(a.p.first().ok_or_else(|| exc("TypeError", "enumerate() missing required argument 'iterable'".into()))?)?;
    Ok(enumerate_new(it, k))
}
// zip and map hold no borrow while they step their inputs, so a callback may re-enter them, as in CPython
pub fn bi_zip(mut a: Args) -> R {
    // zip_new parses "|$p:zip": strict is the only keyword, and a truth test that fails raises
    let strict = match a.kw("strict") {
        Some(x) => truth(&x)?,
        None => false,
    };
    if let Some((k, _)) = a.k.first() {
        return type_err(format!("zip() got an unexpected keyword argument '{k}'"));
    }
    let mut its = vec![];
    for x in &a.p {
        its.push(get_iter(x)?);
    }
    Ok(zip_new(its, strict))
}
pub fn bi_map(a: Args) -> R {
    if a.p.len() < 2 {
        return type_err("map() must have at least two arguments.".into());
    }
    let f = a.p[0].clone();
    let mut its = vec![];
    for x in &a.p[1..] {
        its.push(get_iter(x)?);
    }
    Ok(map_new(f, its))
}
pub fn bi_filter(a: Args) -> R {
    a.want("filter", 2, 2)?;
    Ok(filter_new(a.p[0].clone(), get_iter(&a.p[1])?))
}
pub fn bi_any(a: Args) -> R {
    let mut it = iter(&a1(&a, "any")?)?;
    while let Some(x) = it.next()? {
        if truth(&x)? {
            return Ok(V::Bool(true));
        }
    }
    Ok(V::Bool(false))
}
pub fn bi_all(a: Args) -> R {
    let mut it = iter(&a1(&a, "all")?)?;
    while let Some(x) = it.next()? {
        if !truth(&x)? {
            return Ok(V::Bool(false));
        }
    }
    Ok(V::Bool(true))
}
pub fn bi_iter(a: Args) -> R {
    if a.p.len() == 2 {
        let f = a.p[0].clone();
        let sentinel = a.p[1].clone();
        return Ok(gen_iter(ItKind::CallableIterator, move || {
            let v = call(&f, Args::p(vec![]))?;
            if eq(&v, &sentinel)? {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }));
    }
    let v = a1(&a, "iter")?;
    // iter(x) is what x.__iter__() returned when that is an iterator of the program's own (an iterator's
    // __iter__ returning self keeps `iter(it) is it`, as in CPython)
    if let V::Obj(o) = &v {
        if let Some(f) = o.cls.lookup("__iter__") {
            let r = call(&f, Args::p(vec![v.clone()]))?;
            if let V::Obj(ro) = &r {
                if ro.cls.lookup("__next__").is_none() {
                    return type_err(format!("iter() returned non-iterator of type '{}'", tn(&r)));
                }
                return Ok(r);
            }
            if !matches!(r, V::Iter(_) | V::Ext(_)) {
                return type_err(format!("iter() returned non-iterator of type '{}'", tn(&r)));
            }
            return iter_v(&r);
        }
    }
    iter_v(&v)
}
pub fn bi_next(a: Args) -> R {
    a.want("next", 1, 2)?;
    let r = match &a.p[0] {
        V::Iter(it) => match coro_of(&it.it) {
            // A generator's return value travels on the StopIteration that next() raises.
            Some(c) => {
                let r = c.borrow_mut().resume(V::None);
                match r {
                    Ok(Some(v)) => Some(v),
                    Ok(None) if a.p.len() == 1 => return Err(stop_with(V::None)),
                    Ok(None) => None,
                    Err(E(e)) if is_exc(&e, "StopIteration") && a.p.len() == 1 => return Err(E(e)),
                    Err(E(e)) if is_exc(&e, "StopIteration") => None,
                    Err(e) => return Err(e),
                }
            }
            None => iter_step(&it.it)?,
        },
        V::Obj(o) => match o.cls.lookup("__next__") {
            Some(f) => match call(&f, Args::p(vec![a.p[0].clone()])) {
                Ok(v) => Some(v),
                Err(E(e)) if is_exc(&e, "StopIteration") => None,
                Err(e) => return Err(e),
            },
            None => return type_err(format!("'{}' object is not an iterator", tn(&a.p[0]))),
        },
        V::Ext(x) => match x.next_item() {
            Some(r) => match r {
                Ok(v) => Some(v),
                Err(E(e)) if is_exc(&e, "StopIteration") => None,
                Err(e) => return Err(e),
            },
            None => return type_err(format!("'{}' object is not an iterator", tn(&a.p[0]))),
        },
        x => return type_err(format!("'{}' object is not an iterator", tn(x))),
    };
    match r {
        Some(v) => Ok(v),
        None => match a.p.get(1) {
            Some(d) => Ok(d.clone()),
            None => Err(exc0("StopIteration")),
        },
    }
}
pub fn bi_isinstance(a: Args) -> R {
    a.want("isinstance", 2, 2)?;
    Ok(V::Bool(isinstance(&a.p[0], &a.p[1])?))
}
// A library class or value (a bridged array.array, say) checked against one of Brevi's collections.abc ABCs:
// CPython's own collections.abc class of the same name answers, as Brevi's ABC cannot see a library class's
// methods. None for any other value or class.
pub fn abc_bridge(k: &Rc<Class>, x: &V, sub: bool) -> Option<Result<bool, E>> {
    if !(k.module == "collections.abc" && k.abc.get()) {
        return None;
    }
    if !matches!(x, V::Ext(e) if e.is_py()) {
        return None;
    }
    ABC_BRIDGE.with(|h| h.get()).and_then(|f| f(&k.name, x, sub))
}
pub fn isinstance(x: &V, c: &V) -> Result<bool, E> {
    // a proxy is an instance of its own type and of its referent's class (CPython checks __class__ too)
    if let Some(__p) = proxy_deref(x).filter(|_| !matches!(c, V::Tuple(_))) {
        if let (V::Class(k), V::Ext(e)) = (c, x) {
            if Rc::ptr_eq(k, &ext_class(e.tname())) {
                return Ok(true);
            }
        }
        return isinstance(&__p?, c);
    }
    match c {
        V::Tuple(t) => {
            for k in t.iter() {
                if isinstance(x, k)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        V::Class(k) => {
            if k.name == "object" && k.module == "builtins" {
                return Ok(true);
            }
            if let Some(r) = abc_bridge(k, x, false) {
                return r;
            }
            if k.abc.get() {
                // CPython's exact-type shortcut comes before the metaclass's __instancecheck__
                if let V::Obj(o) = x {
                    if Rc::ptr_eq(&o.cls, k) {
                        return Ok(true);
                    }
                }
                if is_bytestring(k) {
                    bytestring_warn()?;
                }
                return abc_instancecheck(k, x);
            }
            if let V::Obj(o) = x {
                return Ok(o.cls.is_sub(k) || (k.name == "object" && k.module == "builtins"));
            }
            if let V::Ext(e) = x {
                if let Some(b) = e.isinst(k) {
                    return Ok(b);
                }
            }
            match type_of(x) {
                Ok(t) => Ok(t.is_sub(k)),
                // an iterator whose CPython class Brevi cannot name is an instance of none of the classes the
                // program can name, except the iterator classes type() gives (which could be its own)
                Err(e) => if is_iter_class(k) { Err(e) } else { Ok(false) },
            }
        }
        V::Ext(e) => match e.instancecheck(x) {
            Some(r) => r,
            None => type_err("isinstance() arg 2 must be a type, a tuple of types, or a union".into()),
        },
        // the builtin slice is a function here (builtin_value), the type of slice objects
        V::Func(f) if &*f.name.get() == "slice" => Ok(matches!(x, V::Slice(_))),
        _ => type_err("isinstance() arg 2 must be a type, a tuple of types, or a union".into()),
    }
}
fn abc_has(v: &[Rc<Class>], c: &Rc<Class>) -> bool {
    v.iter().any(|x| Rc::ptr_eq(x, c))
}
// ABCMeta.__subclasscheck__ (the C _abc_subclasscheck): cache, __subclasshook__, direct subclass,
// registered classes, then subclasses, recursively.
pub fn abc_subclasscheck(cls: &Rc<Class>, sub: &Rc<Class>) -> Result<bool, E> {
    if abc_has(&cls.abc_cache.borrow(), sub) {
        return Ok(true);
    }
    let token = ABC_TOKEN.load(std::sync::atomic::Ordering::Relaxed);
    if cls.abc_neg_ver.get() < token {
        cls.abc_neg_cache.borrow_mut().clear();
        cls.abc_neg_ver.set(token);
    } else if abc_has(&cls.abc_neg_cache.borrow(), sub) {
        return Ok(false);
    }
    let hook = match getattr(&V::Class(cls.clone()), "__subclasshook__") {
        Ok(h) => Some(h),
        Err(E(e)) if is_exc(&e, "AttributeError") => None,
        Err(e) => return Err(e),
    };
    if let Some(h) = hook {
        let r = call(&h, Args::p(vec![V::Class(sub.clone())]))?;
        if !matches!(r, V::NotImpl) {
            let ok = truth(&r)?;
            let mut cache = if ok { cls.abc_cache.borrow_mut() } else { cls.abc_neg_cache.borrow_mut() };
            if !abc_has(&cache, sub) {
                cache.push(sub.clone());
            }
            drop(cache);
            return Ok(ok);
        }
    }
    if sub.is_sub(cls) {
        if !abc_has(&cls.abc_cache.borrow(), sub) {
            cls.abc_cache.borrow_mut().push(sub.clone());
        }
        return Ok(true);
    }
    // collections registers deque as a MutableSequence (CPython's collections/__init__.py); Brevi's deque is the
    // runtime's own class, which a program can import without Brevi's collections module ever running
    if cls.module == "collections.abc" && cls.name == "MutableSequence" && sub.is_sub(&tclass(16, "deque")) {
        if !abc_has(&cls.abc_cache.borrow(), sub) {
            cls.abc_cache.borrow_mut().push(sub.clone());
        }
        return Ok(true);
    }
    let registered = cls.abc_registry.borrow().clone();
    for r in registered {
        let ok = if r.abc.get() { abc_subclasscheck(&r, sub)? } else { sub.is_sub(&r) };
        if ok {
            if !abc_has(&cls.abc_cache.borrow(), sub) {
                cls.abc_cache.borrow_mut().push(sub.clone());
            }
            return Ok(true);
        }
    }
    let subs: Vec<Rc<Class>> = cls.subs.borrow().iter().filter_map(|w| w.upgrade()).collect();
    for s in subs {
        let ok = if s.abc.get() { abc_subclasscheck(&s, sub)? } else { sub.is_sub(&s) };
        if ok {
            if !abc_has(&cls.abc_cache.borrow(), sub) {
                cls.abc_cache.borrow_mut().push(sub.clone());
            }
            return Ok(true);
        }
    }
    if !abc_has(&cls.abc_neg_cache.borrow(), sub) {
        cls.abc_neg_cache.borrow_mut().push(sub.clone());
    }
    Ok(false)
}
// collections.abc.ByteString and its subclasses have CPython 3.13's _DeprecateByteStringMeta, which warns
// (stacklevel 3: at the program's line) on isinstance and when a subclass is made.
pub fn is_bytestring(k: &Class) -> bool {
    k.abc.get() && k.mro.borrow().iter().any(|c| c.name == "ByteString" && c.module == "collections.abc")
}
pub fn bytestring_warn() -> Result<(), E> {
    re_warn("DeprecationWarning", "'collections.abc.ByteString' is deprecated and slated for removal in Python 3.17".into())
}
// ABCMeta.__instancecheck__ (the C _abc_instancecheck).
fn abc_instancecheck(cls: &Rc<Class>, x: &V) -> Result<bool, E> {
    // an iterator whose CPython class Brevi cannot name is still an iterator (not a generator: those are named)
    let subtype = match type_of(x) {
        Ok(t) => t,
        Err(_) if matches!(x, V::Iter(_)) => ext_class("iterator"),
        Err(e) => return Err(e),
    };
    let subclass = match x {
        V::Obj(o) => o.cls.clone(),
        _ => subtype.clone(),
    };
    if abc_has(&cls.abc_cache.borrow(), &subclass) {
        return Ok(true);
    }
    if Rc::ptr_eq(&subclass, &subtype) {
        let token = ABC_TOKEN.load(std::sync::atomic::Ordering::Relaxed);
        if cls.abc_neg_ver.get() == token && abc_has(&cls.abc_neg_cache.borrow(), &subclass) {
            return Ok(false);
        }
        return abc_subclasscheck(cls, &subclass);
    }
    Ok(abc_subclasscheck(cls, &subclass)? || abc_subclasscheck(cls, &subtype)?)
}
// _check_methods in _collections_abc.py scans the MRO's __dict__s. Brevi's builtin classes have no complete
// __dict__, so this table says, for each builtin type, which of the names collections.abc's __subclasshook__s
// ask about its __dict__ chain has (CPython 3.13; None values, as list.__hash__, count as absent). Some(None):
// a builtin type this table does not know, which _check_methods refuses rather than guess about.
const ABC_HOOK_NAMES: &[&str] = &["__hash__", "__await__", "__aiter__", "__anext__", "__iter__", "__next__", "__reversed__", "__len__", "__contains__", "__call__", "__buffer__", "send", "throw", "close", "asend", "athrow", "aclose"];
fn builtin_hook_methods(c: &Rc<Class>) -> Option<Option<&'static [&'static str]>> {
    const SCALAR: &[&str] = &["__hash__"];
    const CALLABLE: &[&str] = &["__hash__", "__call__"];
    const ITER: &[&str] = &["__hash__", "__iter__", "__next__"];
    if c.module == "builtins" || c.module == "collections" {
        return Some(Some(match c.name {
            "NoneType" | "bool" | "int" | "float" | "complex" | "object" | "slice" | "ellipsis" | "NotImplementedType" | "cell" => SCALAR,
            "str" => &["__hash__", "__iter__", "__len__", "__contains__"],
            "bytes" => &["__hash__", "__iter__", "__len__", "__contains__", "__buffer__"],
            "bytearray" => &["__iter__", "__len__", "__contains__", "__buffer__"],
            "memoryview" => &["__hash__", "__iter__", "__len__", "__buffer__"],
            "list" | "dict" | "defaultdict" | "Counter" | "OrderedDict" | "deque" | "dict_keys" | "dict_items" | "odict_keys" | "odict_items" => &["__iter__", "__reversed__", "__len__", "__contains__"],
            "tuple" | "frozenset" => &["__hash__", "__iter__", "__len__", "__contains__"],
            "set" => &["__iter__", "__len__", "__contains__"],
            "range" => &["__hash__", "__iter__", "__reversed__", "__len__", "__contains__"],
            "dict_values" | "odict_values" => &["__hash__", "__iter__", "__reversed__", "__len__"],
            "type" | "function" | "builtin_function_or_method" | "method" => CALLABLE,
            "generator" => &["__hash__", "__iter__", "__next__", "send", "throw", "close"],
            "coroutine" => &["__hash__", "__await__", "send", "throw", "close"],
            "async_generator" => &["__hash__", "__aiter__", "__anext__", "asend", "athrow", "aclose"],
            "iterator" => ITER,
            // library types Brevi names by their dotted CPython name: plain hashable values
            "datetime.date" | "datetime.datetime" | "datetime.time" | "datetime.timedelta" | "datetime.timezone" | "decimal.Decimal" | "fractions.Fraction" | "zoneinfo.ZoneInfo" | "re.Pattern" | "re.Match" => SCALAR,
            // cmp_to_key's K objects: callable, with __hash__ = None
            "functools.KeyWrapper" => &["__call__"],
            _ if c.name.ends_with("iterator") => ITER,
            _ => return Some(None),
        }));
    }
    if c.module == "itertools" {
        return Some(Some(ITER));
    }
    None
}
fn class_has_method(c: &Rc<Class>, name: &str) -> Result<bool, E> {
    for k in c.mro.borrow().iter() {
        match builtin_hook_methods(k) {
            // the table already folds in what the builtin type inherits
            Some(Some(names)) => {
                if ABC_HOOK_NAMES.contains(&name) {
                    return Ok(names.contains(&name));
                }
                return Err(exc("NotImplementedError", format!("brevi: collections.abc cannot check '{name}' on {} in dynamic builds", k.name)));
            }
            Some(None) => return Err(exc("NotImplementedError", format!("brevi: collections.abc checks of {} objects are not supported in dynamic builds", k.name))),
            None => {
                if let Some(v) = k.own(name) {
                    return Ok(!matches!(v, V::None));
                }
            }
        }
    }
    Ok(false)
}
pub fn abc_check_methods(a: Args) -> R {
    let Some(V::Class(c)) = a.p.first() else {
        return type_err("_check_methods() arg 1 must be a class".into());
    };
    for x in a.p.iter().skip(1) {
        let name = to_str(x)?;
        if !class_has_method(c, &name)? {
            return Ok(V::NotImpl);
        }
    }
    Ok(V::Bool(true))
}
// ABCMeta.register: weak-set semantics without weak refs (the registered classes are program globals).
pub fn abc_register(a: Args) -> R {
    a.want("_abc_register", 2, 2)?;
    let (V::Class(c), V::Class(s)) = (&a.p[0], &a.p[1]) else {
        return type_err("Can only register classes".into());
    };
    if s.is_sub(c) {
        return Ok(a.p[1].clone());
    }
    if c.is_sub(s) {
        return Err(exc("RuntimeError", "Refusing to create an inheritance cycle".into()));
    }
    if !abc_has(&c.abc_registry.borrow(), s) {
        c.abc_registry.borrow_mut().push(s.clone());
    }
    ABC_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(a.p[1].clone())
}
pub fn abc_meta_class() -> Rc<Class> {
    thread_local! {
        static META: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
    }
    META.with(|m| {
        if let Some(c) = m.borrow().as_ref() {
            if c.bases.first().is_some_and(|b| Rc::ptr_eq(b, &tclass(13, "type"))) {
                return c.clone();
            }
        }
        let c = Class::new("ABCMeta", "abc", vec![tclass(13, "type")], BT::Other);
        *m.borrow_mut() = Some(c.clone());
        c
    })
}
pub fn type_of(x: &V) -> Result<Rc<Class>, E> {
    Ok(match x {
        V::Obj(o) => o.cls.clone(),
        V::Bool(_) => tclass(0, "bool"),
        V::Int(_) | V::Big(_) => tclass(1, "int"),
        V::Float(_) => tclass(2, "float"),
        V::Str(_) => tclass(3, "str"),
        V::List(_) => tclass(4, "list"),
        V::Tuple(_) => tclass(5, "tuple"),
        V::Dict(d) => match d.borrow().kind {
            1 => tclass(6, "defaultdict"),
            2 => tclass(7, "Counter"),
            3 => tclass(8, "OrderedDict"),
            _ => tclass(9, "dict"),
        },
        V::Cell(_) => ext_class("cell"),
        V::Set(_) => tclass(10, "set"),
        V::FSet(_) => tclass(11, "frozenset"),
        V::None => tclass(12, "NoneType"),
        // a collections.abc class (or a subclass of one) is made by abc.ABCMeta, a subclass of type
        V::Class(k) if k.meta.borrow().is_some() => k.meta.borrow().clone().unwrap(),
        V::Class(k) if k.abc.get() => abc_meta_class(),
        V::Class(_) => tclass(13, "type"),
        V::Func(f) if f.kind.get() == 3 => ext_class("builtin_function_or_method"),
        V::BMeth(_) => ext_class("builtin_function_or_method"),
        V::Bound(_) => ext_class("method"),
        V::Func(_) => tclass(14, "function"),
        V::Range(_) => tclass(15, "range"),
        V::Deque(_) => tclass(16, "deque"),
        V::View(x) if x.1 == 3 => tclass(17, "mappingproxy"),
        V::View(x) if x.1 <= 2 && matches!(&x.0, V::Dict(d) if d.borrow().kind == 3) => ext_class(["odict_keys", "odict_values", "odict_items"][x.1 as usize]),
        V::View(x) => ext_class(match x.1 {
            0 => "dict_keys",
            1 => "dict_values",
            _ => "dict_items",
        }),
        V::Slice(_) => ext_class("slice"),
        V::Ellipsis => ext_class("ellipsis"),
        V::NotImpl => ext_class("NotImplementedType"),
        V::Ext(e) if e.tname() == "range" => tclass(15, "range"),
        V::Ext(e) if e.tname() == "typing.TypeVar" => ext_class_mod("typing", "TypeVar"),
        V::Ext(e) if e.any().is::<SigV>() => sig_class(),
        V::Ext(e) if e.any().is::<TeeDataV>() => teedata_class(),
        V::Ext(e) => ext_class(e.tname()),
        V::Iter(it) => return iter_type_of(it),
        V::NTF(_) => ext_class("collections._tuplegetter"),
        _ => tclass(18, "object"),
    })
}
fn iter_type_unsupported() -> E {
    exc("NotImplementedError", "brevi: type() of this iterator is not supported".into())
}
fn iter_type_of(it: &Rc<IterCell>) -> Result<Rc<Class>, E> {
    match &it.kind {
        IterClass::Class(c) => Ok(c.clone()),
        IterClass::Kind(k) => it_class(*k),
    }
}
pub fn it_class(k: ItKind) -> Result<Rc<Class>, E> {
    let (module, name) = match k {
        ItKind::ListIter => ("builtins", "list_iterator"),
        ItKind::TupleIter => ("builtins", "tuple_iterator"),
        ItKind::StrAsciiIter => ("builtins", "str_ascii_iterator"),
        ItKind::StrIter => ("builtins", "str_iterator"),
        ItKind::RangeIter => ("builtins", "range_iterator"),
        ItKind::DictKeys => ("builtins", "dict_keyiterator"),
        ItKind::DictValues => ("builtins", "dict_valueiterator"),
        ItKind::DictItems => ("builtins", "dict_itemiterator"),
        ItKind::SetIter => ("builtins", "set_iterator"),
        ItKind::ListReverseIter => ("builtins", "list_reverseiterator"),
        ItKind::Reversed => ("builtins", "reversed"),
        ItKind::DictReverseKeys => ("builtins", "dict_reversekeyiterator"),
        ItKind::OdictIter => ("builtins", "odict_iterator"),
        ItKind::DictReverseValues => ("builtins", "dict_reversevalueiterator"),
        ItKind::DictReverseItems => ("builtins", "dict_reverseitemiterator"),
        ItKind::Map => ("builtins", "map"),
        ItKind::Filter => ("builtins", "filter"),
        ItKind::Zip => ("builtins", "zip"),
        ItKind::Enumerate => ("builtins", "enumerate"),
        ItKind::Generator => ("builtins", "generator"),
        ItKind::CallableIterator => ("builtins", "callable_iterator"),
        ItKind::BytesIter => ("builtins", "bytes_iterator"),
        ItKind::DequeIter => ("collections", "_deque_iterator"),
        ItKind::Iterator => ("builtins", "iterator"),
        ItKind::ItCount => ("itertools", "count"),
        ItKind::ItChain => ("itertools", "chain"),
        ItKind::ItIslice => ("itertools", "islice"),
        ItKind::ItRepeat => ("itertools", "repeat"),
        ItKind::ItCycle => ("itertools", "cycle"),
        ItKind::ItAccumulate => ("itertools", "accumulate"),
        ItKind::ItZipLongest => ("itertools", "zip_longest"),
        ItKind::ItStarmap => ("itertools", "starmap"),
        ItKind::ItTakewhile => ("itertools", "takewhile"),
        ItKind::ItDropwhile => ("itertools", "dropwhile"),
        ItKind::ItPairwise => ("itertools", "pairwise"),
        ItKind::ItCompress => ("itertools", "compress"),
        ItKind::ItFilterfalse => ("itertools", "filterfalse"),
        ItKind::ItGroupby => ("itertools", "groupby"),
        ItKind::ItGrouper => ("itertools", "_grouper"),
        ItKind::ItProduct => ("itertools", "product"),
        ItKind::ItPermutations => ("itertools", "permutations"),
        ItKind::ItCombinations => ("itertools", "combinations"),
        ItKind::ItCombinationsWithReplacement => ("itertools", "combinations_with_replacement"),
        ItKind::ItTee => ("itertools", "_tee"),
        ItKind::ItBatched => ("itertools", "batched"),
        ItKind::Unknown => return Err(iter_type_unsupported()),
    };
    ITER_CLASSES.with(|c| {
        let i = k as u8 as usize;
        if let Some(k) = &c.borrow()[i] {
            return Ok(k.clone());
        }
        let k = Class::new(name, module, vec![bclass("object")], BT::Other);
        c.borrow_mut()[i] = Some(k.clone());
        if iter_kind_subclassable(k_kind(k.name, k.module)) {
            iter_base_attrs(&k);
        }
        Ok(k)
    })
}
fn k_kind(name: &str, module: &str) -> Option<ItKind> {
    match (module, name) {
        ("builtins", "zip") => Some(ItKind::Zip),
        ("builtins", "map") => Some(ItKind::Map),
        ("builtins", "filter") => Some(ItKind::Filter),
        ("builtins", "enumerate") => Some(ItKind::Enumerate),
        ("builtins", "reversed") => Some(ItKind::Reversed),
        ("itertools", n) => itertools_kind(n),
        _ => None,
    }
}
// The iterator types a class may be built on (CPython's Py_TPFLAGS_BASETYPE): zip, map, filter, enumerate,
// reversed and every public itertools type. The others (list_iterator, generator, itertools._tee, ...) are
// refused as bases, with CPython's message.
fn iter_kind_subclassable(k: Option<ItKind>) -> bool {
    k.is_some()
}
pub fn iter_base_of(c: &Rc<Class>) -> Option<Rc<Class>> {
    c.mro.borrow().iter().find(|k| is_iter_class(k)).cloned()
}
// CPython's tp_name of one of Brevi's iterator classes, as "type '...' is not an acceptable base type" prints it
pub fn iter_tp_name(k: &Class) -> String {
    match k.module {
        "builtins" => k.name.to_string(),
        "collections" => format!("_collections.{}", k.name),
        m => format!("{m}.{}", k.name),
    }
}
pub fn iter_base_check(bases: &[Rc<Class>]) -> Result<(), E> {
    for b in bases {
        if is_iter_class(b) && !iter_kind_subclassable(k_kind(b.name, b.module)) {
            return type_err(format!("type '{}' is not an acceptable base type", iter_tp_name(b)));
        }
    }
    Ok(())
}
// The base's __new__(cls, *args, **kw) for a class built on an iterator type: the iterator the base makes
// from the arguments, wrapped in an instance of cls. The types whose C constructor takes no keyword arguments
// refuse them only when cls keeps the base's __init__ (CPython's `type->tp_init == base->tp_init` test), and
// otherwise ignore them; the others parse them as the base does. reversed() of a sequence with __reversed__
// returns that result itself, not an instance of cls, as CPython's does.
fn iter_base_new(base: &Rc<Class>, mut a: Args) -> R {
    let Some(V::Class(cls)) = a.p.first().cloned() else {
        return type_err(format!("{}.__new__(): not enough arguments", base.name));
    };
    a.p.remove(0);
    if !cls.is_sub(base) {
        return type_err(format!("{}.__new__({}): {} is not a subtype of {}", iter_tp_name(base), cls.name, cls.name, iter_tp_name(base)));
    }
    if Rc::ptr_eq(&cls, base) {
        return match iter_class_call(base, a) {
            Some(r) => r,
            None => type_err(format!("cannot create '{}' instances", iter_tp_name(base))),
        };
    }
    let no_kw = matches!(
        (base.module, base.name),
        ("builtins", "map" | "filter") | ("itertools", "chain" | "islice" | "cycle" | "dropwhile" | "takewhile" | "starmap" | "filterfalse" | "pairwise")
    );
    if no_kw && !a.k.is_empty() {
        let own_init = cls.lookup("__init__").is_some_and(|f| !matches!(&f, V::Func(x) if x.kind.get() == 3 && x.name.get() == "__init__"));
        if !own_init {
            return type_err(format!("{}() takes no keyword arguments", base.name));
        }
        a.k.clear();
    }
    let inner = match iter_class_call(base, a) {
        Some(r) => r?,
        None => return type_err(format!("cannot create '{}' instances", iter_tp_name(base))),
    };
    match type_of(&inner) {
        Ok(t) if Rc::ptr_eq(&t, base) => {}
        _ => return Ok(inner),
    }
    let o = Obj::new(cls.clone());
    o.set_inner(Some(inner));
    Ok(V::Obj(o))
}
// The iterator an instance of a class built on an iterator type wraps (or the iterator itself)
fn iter_self(v: &V, base: &Class, what: &str) -> R {
    match v {
        V::Iter(_) => Ok(v.clone()),
        V::Obj(_) => match inner_of(v) {
            Some(i @ V::Iter(_)) => Ok(i),
            _ => type_err(format!("descriptor '{what}' for '{}' objects doesn't apply to a '{}' object", iter_tp_name(base), tn(v))),
        },
        _ => type_err(format!("descriptor '{what}' for '{}' objects doesn't apply to a '{}' object", iter_tp_name(base), tn(v))),
    }
}
// What a class built on an iterator type inherits: __new__, __iter__ and __next__ (stepping the wrapped
// iterator, so iter(x) is x), the __repr__ of count and repeat (with the subclass's name, as CPython's
// _PyType_Name), __length_hint__ of repeat and reversed, chain.from_iterable making the subclass, and
// __reduce__/__setstate__, which are refused for a subclass instance (CPython rebuilds it from its class and
// the base's arguments, which Brevi does not model).
fn iter_base_attrs(k: &Rc<Class>) {
    let w = Rc::downgrade(k);
    let base = move || w.upgrade().expect("iterator class");
    let b = base.clone();
    k.set_exact("__new__", builtin("__new__", move |a: Args| iter_base_new(&b(), a)));
    let b = base.clone();
    k.set_exact("__iter__", builtin("__iter__", move |a: Args| {
        a.want("__iter__", 1, 1)?;
        iter_self(&a.p[0], &b(), "__iter__")?;
        Ok(a.p[0].clone())
    }));
    let b = base.clone();
    k.set_exact("__next__", builtin("__next__", move |a: Args| {
        a.want("__next__", 1, 1)?;
        let it = iter_self(&a.p[0], &b(), "__next__")?;
        bi_next(Args::p(vec![it]))
    }));
    if matches!(k.name, "count" | "repeat") && k.module == "itertools" {
        let b = base.clone();
        k.set_exact("__repr__", builtin("__repr__", move |a: Args| {
            a.want("__repr__", 1, 1)?;
            let it = iter_self(&a.p[0], &b(), "__repr__")?;
            let r = repr(&it)?;
            Ok(match &a.p[0] {
                V::Obj(o) => st(format!("{}{}", o.cls.name, &r[b().name.len()..])),
                _ => st(r),
            })
        }));
    }
    if (k.module, k.name) == ("itertools", "repeat") || (k.module, k.name) == ("builtins", "reversed") {
        let b = base.clone();
        k.set_exact("__length_hint__", builtin("__length_hint__", move |a: Args| {
            a.want("__length_hint__", 1, 1)?;
            let it = iter_self(&a.p[0], &b(), "__length_hint__")?;
            call_method(&it, "__length_hint__", Args::p(vec![]))
        }));
    }
    let b = base.clone();
    k.set_exact("__reduce__", builtin("__reduce__", move |a: Args| {
        a.want("__reduce__", 1, 1)?;
        let it = iter_self(&a.p[0], &b(), "__reduce__")?;
        if let V::Obj(o) = &a.p[0] {
            return Err(unsupported(&format!("pickling or copying an instance of {}, a subclass of {}", o.cls.name, iter_tp_name(&b()))));
        }
        iter_reduce_v(&it)
    }));
    let b = base.clone();
    k.set_exact("__setstate__", builtin("__setstate__", move |a: Args| {
        a.want("__setstate__", 2, 2)?;
        let it = iter_self(&a.p[0], &b(), "__setstate__")?;
        if let V::Obj(o) = &a.p[0] {
            return Err(unsupported(&format!("__setstate__ of an instance of {}, a subclass of {}", o.cls.name, iter_tp_name(&b()))));
        }
        iter_setstate_v(&it, &a.p[1])
    }));
    if (k.module, k.name) == ("itertools", "chain") {
        k.set_exact("from_iterable", chain_from_iterable_for(k));
    }
    // the methods are the class's own and read as builtin methods (not bound to an instance's class)
    for n in ["__new__", "__iter__", "__next__", "__repr__", "__length_hint__", "__reduce__", "__setstate__"] {
        if let Some(V::Func(f)) = k.own(n) {
            f.qualname.set(sym(&format!("{}.{n}", k.name)));
        }
    }
}
// chain.from_iterable for class k (chain or a subclass of it): a classmethod in CPython, so C.from_iterable(x)
// makes a C without calling its __init__.
pub fn chain_from_iterable_for(k: &Rc<Class>) -> V {
    let w = Rc::downgrade(k);
    let f = builtin("from_iterable", move |a: Args| {
        let src = get_iter(&a1(&a, "from_iterable")?)?;
        let inner = chain_new(src);
        let k = w.upgrade().expect("chain class");
        if k.module == "itertools" {
            return Ok(inner);
        }
        let o = Obj::new(k);
        o.set_inner(Some(inner));
        Ok(V::Obj(o))
    });
    if let V::Func(ff) = &f {
        ff.attrs.borrow_mut().push(("__bv_nobind__", V::Bool(true)));
        ff.attrs.borrow_mut().push(("__bv_chain_fi__", V::Bool(true)));
    }
    f
}
pub fn is_chain_fi(f: &V) -> bool {
    matches!(f, V::Func(x) if x.attrs.borrow().iter().any(|(k, _)| *k == "__bv_chain_fi__"))
}
thread_local! {
    // type_of's builtin classes, filled on first use (init_builtin_classes empties it)
    pub static TYPE_CACHE: RefCell<[Option<Rc<Class>>; 19]> = RefCell::new(Default::default());
    // the classes of the iterator kinds (init_builtin_classes empties this too)
    pub static ITER_CLASSES: RefCell<[Option<Rc<Class>>; ItKind::Unknown as usize + 1]> = RefCell::new(std::array::from_fn(|_| None));
}
#[inline]
fn tclass(i: usize, n: &'static str) -> Rc<Class> {
    TYPE_CACHE.with(|c| {
        if let Some(k) = &c.borrow()[i] {
            return k.clone();
        }
        let k = bclass(n);
        c.borrow_mut()[i] = Some(k.clone());
        k
    })
}
thread_local! {
    pub static EXT_CLASSES: RefCell<HashMap<&'static str, Rc<Class>>> = RefCell::new(HashMap::new());
}
pub fn ext_class(n: &'static str) -> Rc<Class> {
    if let Some(c) = EXT_CLASSES.with(|m| m.borrow().get(n).cloned()) {
        return c;
    }
    // datetime.datetime is a subclass of datetime.date, as in CPython (isinstance, issubclass, __mro__)
    // and odict_keys, odict_values and odict_items subclass the dict views
    let base = match n {
        "datetime.datetime" => ext_class("datetime.date"),
        "odict_keys" => ext_class("dict_keys"),
        "odict_values" => ext_class("dict_values"),
        "odict_items" => ext_class("dict_items"),
        _ => bclass("object"),
    };
    // bytes is the library class a user class may subclass: its instances wrap a bytes value
    let c = Class::new(n, "builtins", vec![base], match n {
        "bytes" => BT::Bytes,
        "bytearray" => BT::ByteArray,
        _ => BT::Other,
    });
    EXT_CLASSES.with(|m| m.borrow_mut().insert(n, c.clone()));
    c
}
// Classes of iterator types a bridged CPython value reports (list_iterator for a proxy list, say).
// Keyed by module and name so `type(a) is type(b)` shares one class per CPython type.
thread_local! {
    pub static EXT_ITER_CLASSES: RefCell<HashMap<(Sym, Sym), Rc<Class>>> = RefCell::new(HashMap::new());
}
// A class type() gives for an iterator (one of ITER_CLASSES, or a bridged value's iterator type).
pub fn is_iter_class(k: &Rc<Class>) -> bool {
    ITER_CLASSES.with(|c| c.borrow().iter().flatten().any(|x| Rc::ptr_eq(x, k)))
        || EXT_ITER_CLASSES.with(|m| m.borrow().values().any(|x| Rc::ptr_eq(x, k)))
}
pub fn ext_class_mod(module: &str, name: &str) -> Rc<Class> {
    EXT_ITER_CLASSES.with(|m| {
        let key = (sym(module), sym(name));
        let mut m = m.borrow_mut();
        m.entry(key).or_insert_with(|| Class::new(key.1, key.0, vec![bclass("object")], BT::Other)).clone()
    })
}
pub fn bi_type(a: Args) -> R {
    if a.p.len() == 3 {
        let name = sym(&to_str(&a.p[0])?);
        let bases = to_vec(&a.p[1])?;
        let attrs: Vec<(Sym, V)> = dict_items_of(&a.p[2])?.into_iter().map(|(k, v)| (sym(&to_str(&k).unwrap_or_default()), v)).collect();
        return make_class(name, "__main__", bases, attrs);
    }
    Ok(V::Class(type_of(&a1(&a, "type")?)?))
}
pub fn bi_issubclass(a: Args) -> R {
    a.want("issubclass", 2, 2)?;
    let sub = match &a.p[0] {
        V::Class(c) => Some(c.clone()),
        V::Ext(e) if e.is_py() => None,
        _ => return type_err("issubclass() arg 1 must be a class".into()),
    };
    let check = |k: &V| -> Result<bool, E> {
        match k {
            V::Class(k) => {
                // a library class (array.array) against a collections.abc ABC: CPython answers
                if let Some(r) = abc_bridge(k, &a.p[0], true) {
                    return r;
                }
                let Some(sub) = &sub else {
                    return type_err("issubclass() arg 1 must be a class".into());
                };
                if k.abc.get() {
                    abc_subclasscheck(k, sub)
                } else {
                    Ok(sub.is_sub(k))
                }
            }
            V::Ext(e) => match e.subclasscheck(&V::Class(sub.clone().ok_or_else(|| exc("TypeError", "issubclass() arg 1 must be a class".into()))?)) {
                Some(r) => r,
                None => type_err("issubclass() arg 2 must be a class, a tuple of classes, or a union".into()),
            },
            _ => type_err("issubclass() arg 2 must be a class, a tuple of classes, or a union".into()),
        }
    };
    match &a.p[1] {
        V::Tuple(t) => {
            for k in t.iter() {
                if check(k)? {
                    return Ok(V::Bool(true));
                }
            }
            Ok(V::Bool(false))
        }
        k => Ok(V::Bool(check(k)?)),
    }
}
pub fn bi_hasattr(a: Args) -> R {
    a.want("hasattr", 2, 2)?;
    Ok(V::Bool(hasattr(&a.p[0], &to_str(&a.p[1])?)?))
}
pub fn bi_getattr(a: Args) -> R {
    a.want("getattr", 2, 3)?;
    let n = to_str(&a.p[1])?;
    // getattr(f, "name", default) on a def without that attribute (jinja2's filters, pydantic's validators):
    // the default, without making and catching an AttributeError. Dunder names keep the general path.
    if a.p.len() == 3 && !n.starts_with("__") {
        if let V::Func(f) = &a.p[0] {
            let found = func_user_attr(f, &n)?;
            return Ok(found.unwrap_or_else(|| a.p[2].clone()));
        }
    }
    match getattr(&a.p[0], &n) {
        Ok(v) => Ok(v),
        Err(E(e)) if is_exc(&e, "AttributeError") && a.p.len() == 3 => Ok(a.p[2].clone()),
        Err(e) => Err(e),
    }
}
pub fn bi_setattr(a: Args) -> R {
    a.want("setattr", 3, 3)?;
    setattr(&a.p[0], sym(&to_str(&a.p[1])?), a.p[2].clone())?;
    Ok(V::None)
}
pub fn bi_delattr(a: Args) -> R {
    a.want("delattr", 2, 2)?;
    delattr(&a.p[0], &to_str(&a.p[1])?)?;
    Ok(V::None)
}
pub fn bi_callable(a: Args) -> R {
    let x = a1(&a, "callable")?;
    Ok(V::Bool(match &x {
        V::Func(_) | V::Bound(_) | V::BMeth(_) | V::Class(_) => true,
        V::Obj(o) => o.cls.lookup("__call__").is_some(),
        V::Ext(e) => e.any().downcast_ref::<ProxyV>().is_some_and(|p| p.callable),
        _ => false,
    }))
}
pub fn bi_ord(a: Args) -> R {
    match &a1(&a, "ord")? {
        V::Str(x) if x.nchars() == 1 => Ok(V::Int(x.cps()[0] as i64)),
        V::Str(x) => type_err(format!("ord() expected a character, but string of length {} found", x.nchars())),
        x => type_err(format!("ord() expected string of length 1, but {} found", tn(x))),
    }
}
pub fn bi_chr(a: Args) -> R {
    let a = enum_nums(a);
    chr_of(&a1(&a, "chr")?)
}
// chr() takes the value `__index__` gives; a huge int is still just out of range, not an index error.
fn chr_of(x: &V) -> R {
    match x {
        V::Big(b) => {
            let n = b.mag.first().copied().unwrap_or(0);
            if b.neg || b.mag.len() > 1 {
                return val_err("chr() arg not in range(0x110000)".into());
            }
            chr_code(n as i64)
        }
        V::Obj(o) if o.cls.lookup("__index__").is_some() => {
            chr_of(&call(&o.cls.lookup("__index__").unwrap(), Args::p(vec![x.clone()]))?)
        }
        // an int subclass (IntEnum)
        V::Obj(_) if matches!(inner_of(x), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => chr_of(&inner_of(x).unwrap()),
        _ => {
            chr_code(idx_of(x)?)
        }
    }
}
// chr() of an int that fits: out of range is CPython's ValueError; a lone surrogate (U+D800..U+DFFF) is a str of
// code points (S::from_cps).
fn chr_code(i: i64) -> R {
    if !(0..=0x10FFFF).contains(&i) {
        return val_err("chr() arg not in range(0x110000)".into());
    }
    match char::from_u32(i as u32) {
        Some(c) => Ok(ch1(c)),
        None => Ok(st_cps(vec![i as u32])),
    }
}
fn radix(a: Args, f: &str, r: u32, p: &str) -> R {
    let x = a1(&a, f)?;
    let b = big_of(&x).ok_or_else(|| exc("TypeError", format!("'{}' object cannot be interpreted as an integer", tn(&x))))?;
    let body = Big { neg: false, mag: b.mag.clone() }.to_radix(r);
    Ok(st(format!("{}{}{}", if b.neg { "-" } else { "" }, p, body)))
}
pub fn bi_hex(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "hex", 16, "0x")
}
pub fn bi_oct(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "oct", 8, "0o")
}
pub fn bi_bin(a: Args) -> R {
    let a = enum_nums(a);
    radix(a, "bin", 2, "0b")
}
pub fn bi_round(a: Args) -> R {
    let mut a = enum_nums(a);
    let nd = a.kw("ndigits");
    a.want("round", 1, 2)?;
    let x = a.p[0].clone();
    let nd = a.p.get(1).cloned().or(nd).filter(|n| !matches!(n, V::None));
    if let V::Obj(o) = &x {
        if let Some(f) = o.cls.lookup("__round__") {
            let mut args = vec![x.clone()];
            if let Some(n) = nd {
                args.push(n);
            }
            return call(&f, Args::p(args));
        }
    }
    // round() looks __round__ up on the type, and a proxy's type has none
    if proxy_deref(&x).is_some() {
        return type_err(format!("type {} doesn't define __round__ method", proxy_tn(&x)));
    }
    if let V::Ext(e) = &x {
        let mut args = Args::p(nd.into_iter().collect());
        if let Some(r) = e.method(&x, "__round__", &mut args) {
            return r;
        }
        return type_err(format!("type {} doesn't define __round__ method", proxy_tn(&x)));
    }
    match (&x, nd) {
        (V::Int(_) | V::Big(_) | V::Bool(_), None) => to_int(&x),
        (V::Int(i), Some(n)) => {
            let n = idx_of(&n)?;
            if n >= 0 {
                return Ok(V::Int(*i));
            }
            if n < -18 {
                return Ok(V::Int(0));
            }
            let p = 10i64.pow((-n) as u32);
            let q = i.div_euclid(p);
            let r = i.rem_euclid(p);
            let q = if r * 2 > p || (r * 2 == p && q % 2 != 0) { q + 1 } else { q };
            Ok(V::Int(q * p))
        }
        (V::Float(f), None) => {
            if f.is_nan() {
                return val_err("cannot convert float NaN to integer".into());
            }
            if f.is_infinite() {
                return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
            }
            to_int(&V::Float(f.round_ties_even()))
        }
        (V::Float(f), Some(n)) => {
            let n = idx_of(&n)?;
            if !f.is_finite() {
                // round(nan, n) is a new NaN object in CPython, not the one passed in.
                return Ok(V::Float(fresh_float(*f)));
            }
            if n >= 0 {
                if n > 300 {
                    return Ok(V::Float(*f));
                }
                let t = format!("{:.*}", n as usize, f);
                Ok(V::Float(t.parse::<f64>().unwrap()))
            } else {
                let p = 10f64.powi((-n) as i32);
                let y = (f / p).round_ties_even() * p;
                Ok(V::Float(y))
            }
        }
        _ => type_err(format!("type {} doesn't define __round__ method", proxy_tn(&x))),
    }
}
pub fn bi_divmod(a: Args) -> R {
    let a = enum_nums(a);
    a.want("divmod", 2, 2)?;
    if cplx_of(&a.p[0]).is_some() || cplx_of(&a.p[1]).is_some() {
        return Err(op_err("divmod()", &a.p[0], &a.p[1]));
    }
    let q = binop(&a.p[0], &a.p[1], "//")?;
    let r = binop(&a.p[0], &a.p[1], "%")?;
    Ok(tup(vec![q, r]))
}
// pow(a, -1, m): the modular inverse of a modulo m, found by the extended Euclid algorithm (the coefficient
// for a, with the same sign as m once reduced). None when no inverse exists (gcd(a, m) != 1).
fn big_invmod(b: &Big, m: &Big) -> Option<Big> {
    if m.mag.len() == 1 && m.mag[0] == 1 {
        return Some(Big::from_i64(0));
    }
    let (mut old_r, mut r) = (b.divmod(m).1, m.clone());
    let (mut old_s, mut s) = (Big::from_i64(1), Big::from_i64(0));
    while !r.is_zero() {
        let (q, rem) = old_r.divmod(&r);
        let ns = old_s.sub(&q.mul(&s));
        (old_r, r) = (r, rem);
        (old_s, s) = (s, ns);
    }
    let neg = old_r.neg;
    if neg {
        old_r = old_r.neg();
    }
    if old_r.mag.len() != 1 || old_r.mag[0] != 1 {
        return None;
    }
    if neg {
        old_s = old_s.neg();
    }
    Some(old_s.divmod(m).1)
}

pub fn bi_pow(a: Args) -> R {
    let a = enum_nums(a);
    a.want("pow", 2, 3)?;
    if a.p.len() == 3 && !matches!(a.p[2], V::None) {
        if let Some(r) = complex_pow3(&a.p[0], &a.p[1], &a.p[2]) {
            return r;
        }
        let (b, e, m) = (big_of(&a.p[0]), big_of(&a.p[1]), big_of(&a.p[2]));
        let (mut b, mut e, m) = match (b, e, m) {
            (Some(b), Some(e), Some(m)) => (b, e, m),
            _ => return type_err("pow() 3rd argument not allowed unless all arguments are integers".into()),
        };
        if m.is_zero() {
            return val_err("pow() 3rd argument cannot be 0".into());
        }
        if e.neg {
            b = big_invmod(&b, &m).ok_or_else(|| exc("ValueError", "base is not invertible for the given modulus".into()))?;
            e = e.neg();
        }
        let mut r = Big::from_i64(1).divmod(&m).1;
        b = b.divmod(&m).1;
        while !e.is_zero() {
            if e.mag[0] & 1 == 1 {
                r = r.mul(&b).divmod(&m).1;
            }
            e = e.shr(1);
            b = b.mul(&b).divmod(&m).1;
        }
        return Ok(r.norm());
    }
    binop(&a.p[0], &a.p[1], "**")
}
pub fn bi_hash(a: Args) -> R {
    Ok(V::Int(py_hash(&a1(&a, "hash")?)?))
}
// id(x), kept in step with `is` (rt_ops::is): two live values have the same id exactly when `x is y`.
// The ids live in four classes of integers that never meet:
// - heap values: their allocation, a multiple of 8;
// - ints (unboxed): 32 * i + 1, odd (a value past 2**58 gets an id past 64 bits, still odd);
// - floats (unboxed): 8 * bits + 4, which is 4 modulo 8;
// - singletons and values CPython keeps once (None, bools, Ellipsis, NotImplemented, the empty tuple, the
//   empty str and each 1-char ASCII str): 2 modulo 4.
// Ints and floats are plain values here, so their identity is their value (each value is one object, which
// the language allows for immutable types). A NaN made by arithmetic has no identity payload (`x is x` is
// false for it), so no id can agree with `is`: id() refuses it rather than give a wrong answer.
pub fn bi_id(a: Args) -> R {
    let x = a1(&a, "id")?;
    let p = |q: *const u8| V::Int(q as i64);
    Ok(match &x {
        V::Obj(o) => p(Rc::as_ptr(o) as *const u8),
        V::List(o) => p(Rc::as_ptr(o) as *const u8),
        V::Dict(o) => p(Rc::as_ptr(o) as *const u8),
        V::Str(o) if !o.is_lone() && o.text().len() <= 1 => V::Int(o.text().bytes().next().map_or(26, |b| 30 + 4 * b as i64)),
        V::Str(o) => p(Rc::as_ptr(o) as *const u8),
        V::Tuple(o) if o.is_empty() => V::Int(22),
        V::Tuple(o) => p(Tup::as_ptr(o)),
        V::Int(i) => Big::from_i128(*i as i128 * 32 + 1).norm(),
        V::Float(f) if f.is_nan() && !same_nan(*f, *f) => {
            return Err(exc("NotImplementedError", "brevi: id() of a NaN made by arithmetic is not supported (it has no identity of its own); use math.isnan()".into()))
        }
        V::Float(f) => Big::from_i128(f.to_bits() as i128 * 8 + 4).norm(),
        V::Set(o) => p(Rc::as_ptr(o) as *const u8),
        V::FSet(o) => p(Rc::as_ptr(o) as *const u8),
        V::Big(o) => p(Rc::as_ptr(o) as *const u8),
        V::Cell(o) => p(Rc::as_ptr(o) as *const u8),
        V::Func(o) => p(Rc::as_ptr(o) as *const u8),
        V::Class(o) => p(Rc::as_ptr(o) as *const u8),
        V::Bound(o) => p(Rc::as_ptr(o) as *const u8),
        V::BMeth(o) => p(Rc::as_ptr(o) as *const u8),
        V::Iter(o) => p(Rc::as_ptr(o) as *const u8),
        V::Range(o) => p(Rc::as_ptr(o) as *const u8),
        V::Slice(o) => p(Rc::as_ptr(o) as *const u8),
        V::Deque(o) => p(Rc::as_ptr(o) as *const u8),
        V::View(o) => p(Rc::as_ptr(o) as *const u8),
        V::Prop(o) => p(Rc::as_ptr(o) as *const u8),
        V::NTF(o) => p(Rc::as_ptr(o) as *const u8),
        V::Ext(o) => p(ExtRc::as_ptr(o)),
        V::Module(m) => p(*m as *const ModName as *const u8),
        V::None => V::Int(2),
        V::Bool(false) => V::Int(6),
        V::Bool(true) => V::Int(10),
        V::Ellipsis => V::Int(14),
        V::NotImpl => V::Int(18),
        V::Unbound => return Err(exc("NameError", "id() of an unbound name".into())),
    })
}
pub fn bi_format(a: Args) -> R {
    a.want("format", 1, 2)?;
    let spec = match a.p.get(1) {
        Some(x) => to_str(x)?,
        None => String::new(),
    };
    fmt_v(&a.p[0], &spec)
}
pub fn bi_vars(a: Args) -> R {
    let x = a1(&a, "vars")?;
    if let V::Obj(o) = &x {
        if !o.cls.has_dict() {
            return type_err("vars() argument must have __dict__ attribute".into());
        }
    }
    getattr(&x, "__dict__")
}
pub fn bi_exit(a: Args) -> R {
    let code = a.p.first().cloned().unwrap_or(V::None);
    let c = exc_class("SystemExit");
    let o = Obj::new(c);
    o.set("args", tup(vec![code.clone()]));
    o.set("code", code);
    Err(E(V::Obj(o)))
}
pub fn bi_open(_a: Args) -> R {
    Err(unsupported("open()"))
}
pub fn bi_staticmethod(a: Args) -> R {
    let f = a1(&a, "staticmethod")?;
    Ok(match &f {
        V::Func(x) => {
            let g = rewrap(x, 1);
            g
        }
        V::Obj(_) => rewrap_obj(&f, 1),
        _ => f,
    })
}
pub fn bi_classmethod(a: Args) -> R {
    let f = a1(&a, "classmethod")?;
    Ok(match &f {
        V::Func(x) => rewrap(x, 2),
        V::Obj(_) => rewrap_obj(&f, 2),
        // classmethod(GenericAlias) and other callable types: a classmethod that calls it with the class first
        V::Class(_) | V::Ext(_) => {
            let g = rewrap_obj(&f, 2);
            if let V::Func(gf) = &g {
                gf.attrs.borrow_mut().push(("__bv_repr__", st(format!("<classmethod({})>", repr(&f)?))));
            }
            g
        }
        _ => f,
    })
}
// staticmethod/classmethod of a callable instance (a functools.partial, an object with __call__): read from
// the class it is the instance itself (its own __get__ never runs) or a method bound to the class.
fn rewrap_obj(f: &V, kind: u8) -> V {
    let inner = f.clone();
    let g = func("", move |a: Args| call(&inner, a));
    if let V::Func(gf) = &g {
        gf.kind.set(kind);
        let mut at = gf.attrs.borrow_mut();
        at.push(("__bv_wrapped__", f.clone()));
        at.push(("__func__", f.clone()));
    }
    g
}
fn rewrap(x: &Rc<Func>, kind: u8) -> V {
    let inner = V::Func(x.clone());
    let wrapped = inner.clone();
    let g = func(x.name.get(), move |a: Args| call(&inner, a));
    if let V::Func(gf) = &g {
        gf.kind.set(kind);
        gf.fq.set(x.fq.get());
        // a @staticmethod/@classmethod keeps the def's dotted __qualname__ (T.s, outer.<locals>.C.c)
        gf.qualname.set(x.qualname.get());
        // the wrapped function's attributes (__doc__, __bv_np__) read through, as through CPython's __func__
        *gf.attrs.borrow_mut() = x.attrs.borrow().clone();
        // __get__ returns the wrapped function (staticmethod) or a bound method of it (classmethod)
        gf.attrs.borrow_mut().push(("__bv_wrapped__", wrapped));
        // staticmethod(f).__func__ and classmethod(f).__func__ are f
        let mut at = gf.attrs.borrow_mut();
        at.retain(|(k, _)| *k != "__func__");
        at.push(("__func__", V::Func(x.clone())));
    }
    g
}
pub fn bi_property(mut a: Args) -> R {
    let getter = a.p.first().cloned().or(a.kw("fget")).unwrap_or(V::None);
    let setter = a.p.get(1).cloned().or(a.kw("fset")).unwrap_or(V::None);
    Ok(V::Prop(Rc::new((getter, setter))))
}
pub fn bi_object(_a: Args) -> R {
    Ok(V::Obj(Obj::new(bclass("object"))))
}
pub fn bi_globals(_a: Args) -> R {
    Err(unsupported("globals()"))
}
pub fn bi_slice(a: Args) -> R {
    Ok(match a.p.len() {
        1 => slice(V::None, a.p[0].clone(), V::None),
        2 => slice(a.p[0].clone(), a.p[1].clone(), V::None),
        _ => slice(a.p[0].clone(), a.p[1].clone(), a.p[2].clone()),
    })
}

pub fn builtin_fn(name: &str) -> Option<fn(Args) -> R> {
    Some(match name {
        "print" => bi_print,
        "input" => bi_input,
        "len" => bi_len,
        "repr" => bi_repr,
        "ascii" => bi_ascii,
        "abs" => bi_abs,
        "min" => bi_min,
        "max" => bi_max,
        "sum" => bi_sum,
        "sorted" => bi_sorted,
        "reversed" => bi_reversed,
        "enumerate" => bi_enumerate,
        "zip" => bi_zip,
        "map" => bi_map,
        "filter" => bi_filter,
        "any" => bi_any,
        "all" => bi_all,
        "iter" => bi_iter,
        "next" => bi_next,
        "isinstance" => bi_isinstance,
        "issubclass" => bi_issubclass,
        "hasattr" => bi_hasattr,
        "getattr" => bi_getattr,
        "setattr" => bi_setattr,
        "delattr" => bi_delattr,
        "callable" => bi_callable,
        "ord" => bi_ord,
        "chr" => bi_chr,
        "hex" => bi_hex,
        "oct" => bi_oct,
        "bin" => bi_bin,
        "round" => bi_round,
        "divmod" => bi_divmod,
        "pow" => bi_pow,
        "hash" => bi_hash,
        "id" => bi_id,
        "format" => bi_format,
        "vars" => bi_vars,
        "dir" => bi_dir,
        "exit" | "quit" => bi_exit,
        "open" => bi_open,
        "staticmethod" => bi_staticmethod,
        "classmethod" => bi_classmethod,
        "property" => bi_property,
        "globals" | "locals" => bi_globals,
        "aiter" => bi_aiter,
        "anext" => bi_anext,
        "__brevi_aenter" => bi___brevi_aenter,
        "__brevi_aexit" => bi___brevi_aexit,
        "__brevi_awith" => bi___brevi_awith,
        "__brevi_afor_await" => bi___brevi_afor_await,
        "__brevi_eg_split" => bi___brevi_eg_split,
        "__brevi_eg_reraise" => bi___brevi_eg_reraise,
        _ => return None,
    })
}
// Builtin names that are types (callable classes).
pub fn builtin_type_name(name: &str) -> bool {
    matches!(name, "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "slice")
}
thread_local! {
    // builtin classes found by builtin_value, by the address of the name asked for (a hit is checked against
    // the class's own name, so a reused address cannot give a wrong class); emptied by init_builtin_classes
    pub static BV_CACHE: RefCell<[(usize, Option<Rc<Class>>); 16]> = RefCell::new(Default::default());
}
pub fn builtin_value(name: &str) -> Option<V> {
    let addr = name.as_ptr() as usize;
    let slot = (addr >> 3 ^ addr >> 8 ^ name.len()) & 15;
    let hit = BV_CACHE.with(|c| match &c.borrow()[slot] {
        (a, Some(k)) if *a == addr && same_name(k.name, name) => Some(k.clone()),
        _ => None,
    });
    if let Some(k) = hit {
        return Some(V::Class(k));
    }
    if builtin_type_name(name) && name != "slice" && name != "bytes" {
        let k = bclass(name);
        if same_name(k.name, name) {
            BV_CACHE.with(|c| c.borrow_mut()[slot] = (addr, Some(k.clone())));
        }
        return Some(V::Class(k));
    }
    if name == "bytes" {
        return Some(V::Class(ext_class("bytes")));
    }
    if name == "bytearray" {
        return Some(V::Class(ext_class("bytearray")));
    }
    if name == "memoryview" {
        return Some(V::Class(ext_class("memoryview")));
    }
    if name == "complex" {
        return Some(V::Class(ext_class("complex")));
    }
    if builtin_type_name(name) && name != "slice" {
        return Some(V::Class(bclass(name)));
    }
    if name == "slice" {
        return Some(builtin("slice", bi_slice));
    }
    // the builtin iterator types are classes, as in CPython (type(zip()) is zip, subclassing zip)
    let kind = match name {
        "zip" => Some(ItKind::Zip),
        "map" => Some(ItKind::Map),
        "filter" => Some(ItKind::Filter),
        "enumerate" => Some(ItKind::Enumerate),
        "reversed" => Some(ItKind::Reversed),
        _ => None,
    };
    if let Some(k) = kind {
        return it_class(k).ok().map(V::Class);
    }
    if let Some(f) = builtin_fn(name) {
        let n: Sym = sym(name);
        let v = builtin(n, f);
        if let V::Func(x) = &v {
            x.attrs.borrow_mut().push(("__bv_nobind__", V::Bool(true)));
        }
        return Some(v);
    }
    match name {
        "None" => Some(V::None),
        "True" => Some(V::Bool(true)),
        "False" => Some(V::Bool(false)),
        "NotImplemented" => Some(V::NotImpl),
        "Ellipsis" => Some(V::Ellipsis),
        _ => BUILTIN_CLASSES.with(|b| b.borrow().get(name).cloned()).map(V::Class),
    }
}
// int(x), list(x), dict(...) and so on when called through the class value.
pub fn builtin_type_call(c: &Rc<Class>, a: &mut Args) -> Option<R> {
    if c.name == "bytes" && c.module == "builtins" {
        return Some(bi_bytes(Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) }));
    }
    if c.name == "bytearray" && c.module == "builtins" {
        return Some(bi_bytearray(Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) }));
    }
    if c.name == "complex" && c.module == "builtins" {
        return Some(bi_complex(Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) }));
    }
    if c.name == "memoryview" && c.module == "builtins" {
        return Some(bi_memoryview(Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) }));
    }
    // Other builtin classes (the exceptions) keep their arguments for the caller.
    if !matches!(c.name, "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "defaultdict" | "Counter" | "OrderedDict" | "deque") {
        return None;
    }
    let a = Args { p: std::mem::take(&mut a.p), k: std::mem::take(&mut a.k) };
    Some(match c.name {
        "int" => bi_int(a),
        "float" => bi_float(a),
        "str" => bi_str(a),
        "bool" => bi_bool(a),
        "list" => bi_list(a),
        "tuple" => bi_tuple(a),
        "dict" => bi_dict(a),
        "set" => bi_set(a),
        "frozenset" => bi_frozenset(a),
        "object" => bi_object(a),
        "type" => bi_type(a),
        "range" => bi_range(a),
        "defaultdict" => col_defaultdict(a),
        "Counter" => col_counter(a),
        "OrderedDict" => odict_new(a),
        "deque" => col_deque(a),
        _ => return None,
    })
}
// cls.fromkeys(iterable, value=None), a classmethod of dict (`base`) or OrderedDict: dict's takes its arguments by
// position only, OrderedDict's by keyword too. The class itself builds its own value; a subclass is called with
// no arguments and each key set through its __setitem__, as CPython's dict_fromkeys does.
pub fn dict_fromkeys(cls: Rc<Class>, base: Rc<Class>) -> V {
    builtin("fromkeys", move |mut a: Args| {
        let od = base.name == "OrderedDict";
        if od {
            if let Some(x) = a.kw("iterable") {
                a.p.insert(0, x);
            }
            if let Some(x) = a.kw("value") {
                a.p.push(x);
            }
            a.no_kw("OrderedDict.fromkeys")?;
            if a.p.is_empty() {
                return type_err("OrderedDict.fromkeys() missing required argument 'iterable' (pos 1)".into());
            }
            if a.p.len() > 2 {
                return type_err(format!("OrderedDict.fromkeys() takes at most 2 arguments ({} given)", a.p.len()));
            }
        } else {
            if !a.k.is_empty() {
                return type_err("dict.fromkeys() takes no keyword arguments".into());
            }
            a.want("fromkeys", 1, 2)?;
        }
        let v = a.p.get(1).cloned().unwrap_or(V::None);
        if cls.module == "builtins" || cls.module == "collections" {
            let mut d = Dict::new();
            if od {
                d.kind = 3;
            }
            // a set's or plain dict's keys keep their stored hashes (CPython's dict_fromkeys)
            let known = match &a.p[0] {
                V::Set(_) | V::FSet(_) => Some(set_of(&a.p[0]).unwrap().keys_h()),
                V::Dict(x) if x.borrow().kind == 0 => Some(x.borrow().keys_h()),
                _ => None,
            };
            match known {
                Some(ks) => {
                    for (k, h) in ks {
                        d.set_h(k, v.clone(), h)?;
                    }
                }
                None => {
                    for k in to_vec(&a.p[0])? {
                        d.set(k, v.clone())?;
                    }
                }
            }
            return Ok(V::Dict(Rc::new(RefCell::new(d))));
        }
        let keys = to_vec(&a.p[0])?;
        let me = call(&V::Class(cls.clone()), Args::p(vec![]))?;
        for k in keys {
            setitem(&me, &k, v.clone())?;
        }
        Ok(me)
    })
}
pub fn builtin_class_attr(c: &Rc<Class>, name: &str) -> Option<V> {
    // object's attributes are made once, so each read gives the same value (object.__hash__ is object.__hash__,
    // as CPython's type dict holds one slot wrapper)
    thread_local! {
        static OBJ_ATTRS: RefCell<Vec<(Sym, V)>> = const { RefCell::new(vec![]) };
    }
    let obj = c.module == "builtins" && c.name == "object";
    if obj {
        if let Some(v) = OBJ_ATTRS.with(|m| m.borrow().iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone())) {
            return Some(v);
        }
    }
    let v = builtin_class_attr1(c, name)?;
    if obj {
        OBJ_ATTRS.with(|m| m.borrow_mut().push((sym(name), v.clone())));
    }
    Some(v)
}
fn builtin_class_attr1(c: &Rc<Class>, name: &str) -> Option<V> {
    let v = builtin_class_attr_raw(c, name)?;
    if let V::Func(f) = &v {
        if f.kind.get() == 3 {
            let fq = if c.module == "builtins" {
                format!("{}.{}", c.name, name)
            } else {
                format!("{}.{}.{}", c.module, c.name, name)
            };
            f.fq.set(sym(&fq));
        }
    }
    Some(v)
}
fn builtin_class_attr_raw(c: &Rc<Class>, name: &str) -> Option<V> {
    // dict.fromkeys, str.maketrans, int.from_bytes ... and unbound methods (str.lower)
    match (c.name, name) {
        ("bytes", "fromhex") => Some(builtin("fromhex", bytes_fromhex)),
        ("bytearray", "fromhex") => Some(builtin("fromhex", bytearray_fromhex)),
        // bytes.maketrans and bytearray.maketrans are static methods (both return bytes): class calls do not bind
        // their first argument as self
        ("bytes" | "bytearray", "maketrans") => Some(builtin("maketrans", bytes_maketrans)),
        ("int", "from_bytes") => Some(builtin("from_bytes", |a: Args| int_from_bytes(&V::None, a))),
        // str.maketrans is a static method: class calls do not bind their first argument as self
        ("str", "maketrans") => Some(builtin("maketrans", |a: Args| str_maketrans(&a))),
        // an instance of a plain class, its __init__ not run
        ("object", "__new__") => Some(builtin("__new__", |a: Args| match a.p.first() {
            Some(V::Class(c)) if matches!(c.base_bt(), BT::Object | BT::Other) && c.module != "typing" && !is_enum(c) => Ok(V::Obj(Obj::new(c.clone()))),
            Some(V::Class(c)) => Err(unsupported(&format!("object.__new__({})", c.name))),
            _ => type_err("object.__new__(X): X is not a type object".into()),
        })),
        // object.__init__ is a no-op that accepts any object but needs the instance argument
        ("object", "__init__") => Some(builtin("__init__", |a: Args| {
            if a.p.is_empty() {
                return type_err("descriptor '__init__' of 'object' object needs an argument".into());
            }
            Ok(V::None)
        })),
        ("object", "__init_subclass__") => Some(builtin("__init_subclass__", |_a: Args| Ok(V::None))),
        // a class method: C.__subclasshook__(sub) is NotImplemented for any class that does not define its own
        // (ABCMeta's issubclass calls it and then goes on to the registry and subclasses)
        ("object", "__subclasshook__") => Some(builtin("__subclasshook__", |_a: Args| Ok(V::NotImpl))),
        // the lookup without the class's __getattr__ fallback
        ("object", "__getattribute__") => Some(builtin("__getattribute__", |a: Args| {
            let (Some(o), Some(V::Str(n))) = (a.p.first(), a.p.get(1)) else {
                return type_err("object.__getattribute__(o, name): name must be a string".into());
            };
            match o {
                V::Obj(ob) => {
                    if n.text() == "__dict__" && ob.cls.lookup(&n.text()).is_none() && ob.cls.has_dict() {
                        return ob.dict();
                    }
                    if let Some(v) = ob.get_attr(&n.text())? {
                        return Ok(v);
                    }
                    if ob.cls.lookup(&n.text()).is_some() || n.text() == "__class__" || n.text() == "__dict__" {
                        return getattr(o, &n.text());
                    }
                    Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", ob.cls.name, n.text())))
                }
                _ => getattr(o, &n.text()),
            }
        })),
        // Counter.fromkeys is defined only to refuse: initializing counts to a constant has no clear meaning.
        ("Counter", "fromkeys") => Some(builtin("fromkeys", |_a: Args| {
            Err(exc("NotImplementedError", "Counter.fromkeys() is undefined.  Use Counter(iterable) instead.".into()))
        })),
        // a classmethod bound to its class: a class attribute holding it never binds (`ctor = dict.fromkeys`)
        ("dict", "fromkeys") | ("OrderedDict", "fromkeys") => Some({
            let f = dict_fromkeys(c.clone(), c.clone());
            if let V::Func(ff) = &f {
                ff.attrs.borrow_mut().push(("__bv_nobind__", V::Bool(true)));
            }
            f
        }),
        // only defaultdict has __missing__ (dict subclasses test `hasattr(dict, "__missing__")`)
        _ if name == "__missing__" && c.name != "defaultdict" => None,
        _ => {
            let n = sym(name);
            let cname = c.name;
            let v = builtin(n, move |mut a: Args| {
                if a.p.is_empty() {
                    return type_err(format!("descriptor '{n}' needs an argument"));
                }
                let me = a.p.remove(0);
                odict_bypass_refused(cname, &n.to_string(), &me)?;
                // Base.method(self, ...) on an instance of a subclass: the base's own method, not an override
                if let V::Obj(o) = &me {
                    let base = {
                        let mro = o.cls.mro.borrow();
                        match mro.iter().position(|k| k.name == cname && (k.module == "builtins" || k.module == "collections")) {
                            // object.__setattr__(obj, name, v) with nothing from object on in the MRO defining it:
                            // super_attr would hand back the plain instance store, so do that store directly
                            Some(i) if i > 0 && n == "__setattr__" && mro[i..].iter().all(|k| k.own(n).is_none()) => {
                                drop(mro);
                                return object_setattr(&me, &a.p);
                            }
                            Some(i) if i > 0 => Some(mro[i - 1].clone()),
                            _ => None,
                        }
                    };
                    if let Some(b) = base {
                        let f = super_attr(&V::Class(b), &me, n)?;
                        return call(&f, a);
                    }
                }
                call_method(&me, n, a)
            });
            if let V::Func(f) = &v {
                // CPython reprs tuple's slot wrappers with the type ("<slot wrapper '__getitem__' of 'tuple' objects>")
                if cname == "tuple" && matches!(n, "__getitem__" | "__contains__" | "__iter__" | "__len__" | "__add__" | "__mul__" | "__rmul__" | "__hash__" | "__repr__") {
                    f.attrs.borrow_mut().push(("__bv_repr__", st(format!("<slot wrapper '{n}' of 'tuple' objects>"))));
                }
            }
            Some(v)
        }
    }
}

// ---------- collections ----------

pub fn col_defaultdict(mut a: Args) -> R {
    let f = if a.p.is_empty() { V::None } else { a.p.remove(0) };
    let d = bi_dict(a)?;
    if let V::Dict(x) = &d {
        let mut x = x.borrow_mut();
        x.kind = 1;
        x.dflt = if matches!(f, V::None) { None } else { Some(f) };
    }
    Ok(d)
}
pub fn col_counter(mut a: Args) -> R {
    // CPython's Counter.__init__(self, iterable=None, /, **kwds): the positional argument is iterable only.
    if a.p.len() > 1 {
        return type_err(format!("Counter.__init__() takes from 1 to 2 positional arguments but {} were given", a.p.len() + 1));
    }
    let mut d = Dict::new();
    d.kind = 2;
    let v = V::Dict(Rc::new(RefCell::new(d)));
    counter_update(&v, a, true)?;
    Ok(v)
}
fn counter_dict(c: &V) -> Option<Rc<RefCell<Dict>>> {
    match c {
        V::Dict(d) if d.borrow().kind == 2 => Some(d.clone()),
        _ => None,
    }
}
// `dict_items_of`, but a mapping is told apart from an iterable of keys: a dict subclass's `items()` are its
// (key, count) pairs, while iterating it yields its keys.
fn is_counter_mapping(v: &V) -> bool {
    match v {
        V::Dict(_) => true,
        V::View(x) => x.1 == 3,
        V::Obj(o) => o.cls.lookup("keys").is_some(),
        _ => false,
    }
}
// CPython's Counter.update/subtract: the positional iterable (None = none) first, then the keywords as if they
// were a mapping. An empty counter updating from a mapping copies the counts instead of adding to a zero, so
// `Counter(iterable=None)` keeps the None value instead of computing 0 + None.
fn counter_update(c: &V, mut a: Args, add: bool) -> R {
    let Some(d) = counter_dict(c) else {
        return Ok(V::None);
    };
    a.want(if add { "Counter.update" } else { "Counter.subtract" }, 0, 1)?;
    if let Some(src) = a.p.first().cloned() {
        if !matches!(src, V::None) {
            if is_counter_mapping(&src) {
                counter_update_pairs(&d, dict_items_of(&src)?, add)?;
            } else {
                for k in to_vec(&src)? {
                    counter_update_pair(&d, k, V::Int(1), add, false)?;
                }
            }
        }
    }
    let kw = a.k.drain(..).map(|(n, v)| (s(n), v)).collect();
    counter_update_pairs(&d, kw, add)?;
    Ok(V::None)
}
fn counter_update_pairs(d: &Rc<RefCell<Dict>>, pairs: Vec<(V, V)>, add: bool) -> R {
    // CPython's empty fast path: super().update(iterable) copies the values as they are.
    if add && d.borrow().n == 0 {
        for (k, v) in pairs {
            Dict::set_cell(d, k, v)?;
        }
        return Ok(V::None);
    }
    for (k, v) in pairs {
        counter_update_pair(d, k, v, add, true)?;
    }
    Ok(V::None)
}
// add: count + self_get(elem, 0); subtract: self_get(elem, 0) - count. `mapping` only orders the operands.
fn counter_update_pair(d: &Rc<RefCell<Dict>>, k: V, v: V, add: bool, mapping: bool) -> R {
    let cur = Dict::get_cell(d, &k)?.unwrap_or(V::Int(0));
    let n = if add {
        if mapping {
            binop(&v, &cur, "+")?
        } else {
            binop(&cur, &v, "+")?
        }
    } else {
        binop(&cur, &v, "-")?
    };
    Dict::set_cell(d, k, n)?;
    Ok(V::None)
}
// A Counter subclass instance's __init__, update and subtract, as CPython's Python Counter runs them: through the
// instance's own update, get, __setitem__ and truth, so a subclass that overrides them is called (the native
// paths above work on the wrapped dict and see no override).
pub fn counter_inner(me: &V) -> Option<Rc<RefCell<Dict>>> {
    match me {
        V::Obj(o) => match o.inner() {
            Some(V::Dict(d)) if d.borrow().kind == 2 => Some(d),
            _ => None,
        },
        _ => None,
    }
}
pub fn counter_obj_init(me: &V, a: Args) -> R {
    // Counter.__init__(self, iterable=None, /, **kwds): super().__init__(); self.update(iterable, **kwds)
    if a.p.len() > 1 {
        return type_err(format!("Counter.__init__() takes from 1 to 2 positional arguments but {} were given", a.p.len() + 1));
    }
    if let V::Obj(o) = me {
        let mut d = Dict::new();
        d.kind = 2;
        o.set_inner(Some(V::Dict(Rc::new(RefCell::new(d)))));
    }
    call_method(me, "update", a)?;
    Ok(V::None)
}
pub fn counter_obj_method(me: &V, name: &str, mut a: Args) -> R {
    let add = name == "update";
    let Some(d) = counter_inner(me) else {
        return Ok(V::None);
    };
    a.want(if add { "Counter.update" } else { "Counter.subtract" }, 0, 1)?;
    if let Some(src) = a.p.first().cloned() {
        if !matches!(src, V::None) {
            if is_counter_mapping(&src) {
                if add && !truth(me)? {
                    // super().update(iterable): dict.update stores into the dict itself
                    counter_update_pairs(&d, dict_items_of(&src)?, true)?;
                } else {
                    let get = getattr(me, "get")?;
                    for (k, c) in dict_items_of(&src)? {
                        let cur = call(&get, Args::p(vec![k.clone(), V::Int(0)]))?;
                        let n = if add { binop(&c, &cur, "+")? } else { binop(&cur, &c, "-")? };
                        setitem(me, &k, n)?;
                    }
                }
            } else if add {
                module_call("collections", "_count_elements", Args::p(vec![me.clone(), src]))?;
            } else {
                let get = getattr(me, "get")?;
                let mut it = iter(&src)?;
                while let Some(k) = it.next()? {
                    let cur = call(&get, Args::p(vec![k.clone(), V::Int(0)]))?;
                    setitem(me, &k, binop(&cur, &V::Int(1), "-")?)?;
                }
            }
        }
    }
    if !a.k.is_empty() {
        let mut kw = Dict::new();
        for (n, v) in a.k.drain(..) {
            kw.set(s(n), v)?;
        }
        call_method(me, name, Args::p(vec![V::Dict(Rc::new(RefCell::new(kw)))]))?;
    }
    Ok(V::None)
}
// ---------- methods of builtin types ----------

pub fn has_method(o: &V, name: &str) -> bool {
    let names: &[&str] = match o {
        V::Str(_) => &["split", "rsplit", "strip", "lstrip", "rstrip", "join", "replace", "find", "rfind", "index", "rindex", "count", "startswith", "endswith", "upper", "lower", "title", "capitalize", "swapcase", "isdigit", "isalpha", "isalnum", "isspace", "isupper", "islower", "isnumeric", "isdecimal", "isidentifier", "istitle", "isprintable", "isascii", "ljust", "rjust", "center", "zfill", "partition", "rpartition", "splitlines", "format", "casefold", "removeprefix", "removesuffix", "expandtabs", "encode", "translate", "maketrans", "format_map"],
        V::List(_) => &["append", "extend", "insert", "pop", "remove", "index", "count", "sort", "reverse", "copy", "clear"],
        V::Tuple(_) => &["count", "index"],
        V::Dict(_) => &["get", "setdefault", "pop", "popitem", "keys", "values", "items", "update", "copy", "clear", "most_common", "elements", "subtract", "total", "move_to_end", "fromkeys"],
        V::FSet(_) => &["union", "intersection", "difference", "symmetric_difference", "issubset", "issuperset", "isdisjoint", "copy"],
        V::Set(_) => &["add", "remove", "discard", "pop", "union", "intersection", "difference", "symmetric_difference", "issubset", "issuperset", "isdisjoint", "update", "intersection_update", "difference_update", "symmetric_difference_update", "copy", "clear"],
        V::Deque(_) => &["append", "appendleft", "pop", "popleft", "extend", "extendleft", "rotate", "clear", "copy", "count", "index", "remove", "reverse", "insert"],
        V::Int(_) | V::Big(_) | V::Bool(_) => &["bit_length", "to_bytes", "from_bytes", "conjugate", "bit_count", "as_integer_ratio", "is_integer"],
        V::Float(_) => &["is_integer", "as_integer_ratio", "hex", "conjugate"],
        V::Iter(_) => &["__next__", "__iter__", "send", "throw", "close"],
        // bytes, bytearray and memoryview have exactly CPython's attributes (b''.append is an AttributeError)
        V::Ext(x) if matches!(x.tname(), "bytes" | "bytearray" | "memoryview") && (x.any().is::<BytesV>() || x.any().is::<ByteArrayV>() || x.any().is::<MemV>()) => {
            return cpy_builtin_has(x.tname(), name)
        }
        V::Ext(_) => return true,
        _ => &[],
    };
    // Dunder methods (`__len__`, `__add__`, ...) are there; dunder data that only functions, classes and modules
    // carry is not: `hasattr("s", "__name__")` is False, as in CPython (argparse tests it on every value).
    const NOT_ON_VALUES: &[&str] = &["__name__", "__qualname__", "__dict__", "__module__", "__wrapped__", "__self__", "__func__", "__code__", "__defaults__", "__kwdefaults__", "__annotations__", "__file__", "__all__", "__mro__", "__bases__", "__members__", "__dataclass_fields__", "__slots__", "__origin__", "__args__", "__version__", "__path__", "__closure__", "__globals__", "__match_args__"];
    // Protocol methods numbers and None do not have, and the ones containers do not (hasattr(3, "__iter__") is
    // False: requests and httpx test values that way).
    const NOT_ON_ANY: &[&str] = &["__next__", "__call__", "__enter__", "__exit__", "__await__", "__aiter__", "__anext__", "__aenter__", "__aexit__", "__fspath__", "__missing__", "read", "__get__", "__set__", "__html__", "__html_format__", "__conform__"];
    const NOT_ON_SCALARS: &[&str] = &["__iter__", "__len__", "__getitem__", "__contains__", "__setitem__", "__delitem__", "__reversed__"];
    if names.contains(&name) {
        return true;
    }
    if name.starts_with("__") || name == "read" {
        let scalar = matches!(o, V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_) | V::None);
        // (iterators and generators have __next__ and __iter__, listed above, and none of the others)
        let container = matches!(o, V::Str(_) | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) | V::FSet(_) | V::Deque(_) | V::Iter(_));
        if (scalar || container) && NOT_ON_ANY.contains(&name) {
            return false;
        }
        // sqlite3's adaptation protocol: no library value has it
        if name == "__conform__" && matches!(o, V::Ext(_)) {
            return false;
        }
        if scalar && NOT_ON_SCALARS.contains(&name) {
            return false;
        }
        if matches!(o, V::Set(_) | V::FSet(_)) && matches!(name, "__getitem__" | "__setitem__" | "__reversed__") {
            return false;
        }
    }
    names.contains(&name) || (name.starts_with("__") && !NOT_ON_VALUES.contains(&name))
}
fn bad_method(o: &V, name: &str) -> R {
    Err(exc("AttributeError", format!("'{}' object has no attribute '{}'", tn(o), name)))
}
fn arg_str(v: &V, f: &str) -> Result<Rc<S>, E> {
    match v {
        V::Str(x) => Ok(x.clone()),
        x => type_err(format!("{f} arg must be None or str, not {}", tn(x))),
    }
}
fn opt_idx(a: &Args, i: usize) -> Result<Option<i64>, E> {
    match a.p.get(i) {
        None | Some(V::None) => Ok(None),
        // `xs.index(v, -4 * sys.maxsize)`: past 64 bits, clamped as CPython clamps it to the ends.
        Some(V::Big(b)) => Ok(Some(if b.neg { i64::MIN / 4 } else { i64::MAX / 4 })),
        Some(x) => Ok(Some(idx_of(x)?)),
    }
}
// char positions <-> byte offsets
fn char_range(st: &S, a: Option<i64>, b: Option<i64>) -> (usize, usize) {
    let n = st.nchars() as i64;
    let fix = |x: i64| -> i64 {
        if x < 0 {
            (x + n).max(0)
        } else {
            x.min(n)
        }
    };
    let s0 = fix(a.unwrap_or(0)) as usize;
    let e0 = fix(b.unwrap_or(n)) as usize;
    (s0, e0)
}
fn byte_at(st: &S, ci: usize) -> usize {
    if st.ascii() {
        ci.min(st.text().len())
    } else {
        st.text().char_indices().nth(ci).map(|(b, _)| b).unwrap_or(st.text().len())
    }
}
fn char_at_byte(st: &S, b: usize) -> usize {
    if st.ascii() {
        b
    } else {
        st.text()[..b].chars().count()
    }
}
fn py_split_ws(t: &str, maxsplit: i64) -> Vec<V> {
    let mut out = vec![];
    let mut rest = t.trim_start_matches(|c: char| c.is_whitespace());
    let mut n = 0;
    while !rest.is_empty() {
        if maxsplit >= 0 && n >= maxsplit {
            out.push(s(rest));
            return out;
        }
        match rest.find(|c: char| c.is_whitespace()) {
            Some(i) => {
                out.push(s(&rest[..i]));
                rest = rest[i..].trim_start_matches(|c: char| c.is_whitespace());
                n += 1;
            }
            None => {
                out.push(s(rest));
                rest = "";
            }
        }
    }
    out
}
fn py_rsplit_ws(t: &str, maxsplit: i64) -> Vec<V> {
    let mut out = vec![];
    let mut rest = t.trim_end_matches(|c: char| c.is_whitespace());
    let mut n = 0;
    while !rest.is_empty() {
        if maxsplit >= 0 && n >= maxsplit {
            out.push(s(rest));
            break;
        }
        match rest.rfind(|c: char| c.is_whitespace()) {
            Some(i) => {
                let c = rest[i..].chars().next().unwrap();
                out.push(s(&rest[i + c.len_utf8()..]));
                rest = rest[..i].trim_end_matches(|c: char| c.is_whitespace());
                n += 1;
            }
            None => {
                out.push(s(rest));
                rest = "";
            }
        }
    }
    out.reverse();
    out
}
fn is_linebreak(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
}
fn py_title(t: &str) -> String {
    let mut out = String::new();
    let mut prev_cased = false;
    for c in t.chars() {
        if c.is_alphabetic() {
            if prev_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            prev_cased = true;
        } else {
            out.push(c);
            prev_cased = false;
        }
    }
    out
}
fn strip_set(a: &Args, f: &str) -> Result<Option<Vec<char>>, E> {
    match a.p.first() {
        None | Some(V::None) => Ok(None),
        Some(x) => Ok(Some(arg_str(x, f)?.text().chars().collect())),
    }
}
// A str argument with a lone surrogate, or a tuple holding one (startswith's prefixes).
fn arg_lone(x: &V) -> bool {
    match x {
        V::Str(y) => y.is_lone(),
        V::Tuple(t) => t.iter().any(|y| matches!(y, V::Str(z) if z.is_lone())),
        _ => false,
    }
}
// The str methods whose answer for a str with a lone surrogate is the answer for the same text with each
// surrogate swapped for an unused private-use character (Swap's rule): the methods only compare characters,
// count them, and ask the properties Cs and Co share. Their errors never quote the text.
const STR_SWAP_OK: &[&str] = &[
    "split", "rsplit", "strip", "lstrip", "rstrip", "replace", "find", "rfind", "index", "rindex", "count", "startswith", "endswith", "upper", "lower", "casefold", "title", "capitalize", "swapcase", "isdigit", "isdecimal", "isnumeric", "isalpha", "isalnum", "isspace", "isupper", "islower", "istitle", "isascii", "isprintable", "isidentifier", "ljust", "rjust", "center", "zfill", "partition", "rpartition", "splitlines", "removeprefix", "removesuffix", "expandtabs",
];
// str_method for a call where the str or a str argument holds a lone surrogate: through Swap for the methods it is
// sound for, code points for join and format; every other method stops (text()).
fn str_method_lone(me: &Rc<S>, o: &V, name: &str, a: Args) -> R {
    if !STR_SWAP_OK.contains(&name) {
        return match name {
            "join" | "format" | "format_map" => str_method_cps(me, name, a),
            _ => lone_stop(std::panic::Location::caller()),
        };
    }
    let mut strs: Vec<Rc<S>> = vec![me.clone()];
    for x in a.p.iter().chain(a.k.iter().map(|(_, v)| v)) {
        match x {
            V::Str(y) => strs.push(y.clone()),
            V::Tuple(t) => strs.extend(t.iter().filter_map(|y| if let V::Str(z) = y { Some(z.clone()) } else { None })),
            _ => {}
        }
    }
    let refs: Vec<&S> = strs.iter().map(|x| &**x).collect();
    let Some(sw) = Swap::plan(&refs, &[]) else { lone_stop(std::panic::Location::caller()) };
    // swapped value -> the caller's object, so that answers which are an input itself stay that object
    let mut orig: Vec<(Rc<S>, V)> = vec![];
    let mut to = |x: &V, orig: &mut Vec<(Rc<S>, V)>| -> V {
        match x {
            V::Str(y) => {
                let z = Rc::new(sw.to(y));
                orig.push((z.clone(), x.clone()));
                V::Str(z)
            }
            V::Tuple(t) if t.iter().any(|y| matches!(y, V::Str(_))) => tup(t
                .iter()
                .map(|y| match y {
                    V::Str(z) => V::Str(Rc::new(sw.to(z))),
                    _ => y.clone(),
                })
                .collect()),
            _ => x.clone(),
        }
    };
    let me2 = Rc::new(sw.to(me));
    orig.push((me2.clone(), o.clone()));
    let a2 = Args { p: a.p.iter().map(|x| to(x, &mut orig)).collect(), k: a.k.iter().map(|(n, x)| (*n, to(x, &mut orig))).collect() };
    let back = |x: V| -> V {
        match &x {
            V::Str(y) => match orig.iter().find(|(z, _)| Rc::ptr_eq(z, y)) {
                Some((_, v)) => v.clone(),
                None => V::Str(Rc::new(sw.back(y))),
            },
            _ => x,
        }
    };
    Ok(match str_method(&me2, &V::Str(me2.clone()), name, a2)? {
        V::List(l) => list(l.borrow().iter().cloned().map(back).collect()),
        V::Tuple(t) => tup(t.iter().cloned().map(back).collect()),
        r => back(r),
    })
}
// join, format and format_map, over code points: the text they copy may hold lone surrogates.
fn str_method_cps(me: &Rc<S>, name: &str, a: Args) -> R {
    match name {
        "join" => str_join(me, &a1(&a, "join")?),
        "format" => Ok(V::Str(Rc::new(str_format_s(me, &a, None)?))),
        _ => {
            let m = a1(&a, "format_map")?;
            Ok(V::Str(Rc::new(str_format_s(me, &Args { p: vec![], k: vec![] }, Some(&m))?)))
        }
    }
}
fn str_join(me: &S, it: &V) -> R {
    let items = to_vec(it)?;
    let mut out = Txt::new();
    for (i, x) in items.iter().enumerate() {
        if i > 0 {
            out.push_s(me);
        }
        match x {
            V::Str(y) => out.push_s(y),
            // a library str (numpy's np.str_)
            V::Ext(e) if matches!(e.native(), Some(Ok(V::Str(_)))) => {
                if let Some(Ok(V::Str(y))) = e.native() {
                    out.push_s(&y);
                }
            }
            // a str subclass (StrEnum)
            V::Obj(_) if matches!(inner_of(x), Some(V::Str(_))) => {
                if let Some(V::Str(y)) = inner_of(x) {
                    out.push_s(&y);
                }
            }
            y => return type_err(format!("sequence item {i}: expected str instance, {} found", tn(y))),
        }
    }
    Ok(txt_v(out))
}
pub fn str_method(me: &Rc<S>, o: &V, name: &str, mut a: Args) -> R {
    // str subclasses (StrEnum members) as arguments: their str
    for x in a.p.iter_mut().chain(a.k.iter_mut().map(|(_, v)| v)) {
        if let V::Obj(_) = x {
            if let Some(i @ V::Str(_)) = inner_of(x) {
                *x = i;
            }
        }
    }
    // encode reads code points itself (str_encode), so a str with lone surrogates goes there as it is
    if name == "encode" {
        let enc = a.kw("encoding").or_else(|| a.p.first().cloned());
        let errs = a.kw("errors").or_else(|| a.p.get(1).cloned());
        a.no_kw("encode")?;
        return str_encode(me, enc.as_ref(), errs.as_ref());
    }
    if me.is_lone() || a.p.iter().chain(a.k.iter().map(|(_, v)| v)).any(arg_lone) {
        return str_method_lone(me, o, name, a);
    }
    let t = me.text();
    Ok(match name {
        "split" | "rsplit" => {
            let sep = a.kw("sep").or_else(|| a.p.first().cloned()).unwrap_or(V::None);
            let maxsplit = match a.kw("maxsplit").or_else(|| a.p.get(1).cloned()) {
                Some(m) => idx_of(&m)?,
                None => -1,
            };
            match sep {
                V::None => list(if name == "split" { py_split_ws(t, maxsplit) } else { py_rsplit_ws(t, maxsplit) }),
                V::Str(sep) => {
                    if sep.text().is_empty() {
                        return val_err("empty separator".into());
                    }
                    let parts: Vec<V> = if maxsplit < 0 {
                        t.split(sep.text()).map(s).collect()
                    } else if name == "split" {
                        t.splitn(maxsplit as usize + 1, sep.text()).map(s).collect()
                    } else {
                        let mut v: Vec<V> = t.rsplitn(maxsplit as usize + 1, sep.text()).map(s).collect();
                        v.reverse();
                        v
                    };
                    list(parts)
                }
                x => return type_err(format!("must be str or None, not {}", tn(&x))),
            }
        }
        "strip" | "lstrip" | "rstrip" => {
            let cs = strip_set(&a, name)?;
            let f = |c: char| match &cs {
                Some(v) => v.contains(&c),
                None => c.is_whitespace(),
            };
            let r = match name {
                "strip" => t.trim_matches(f),
                "lstrip" => t.trim_start_matches(f),
                _ => t.trim_end_matches(f),
            };
            if r.len() == t.len() {
                return Ok(o.clone());
            }
            s(r)
        }
        "join" => return str_join(me, &a1(&a, "join")?),
        "replace" => {
            a.want("replace", 2, 3)?;
            let (old, new) = (arg_str(&a.p[0], "replace")?, arg_str(&a.p[1], "replace")?);
            let count = opt_idx(&a, 2)?.unwrap_or(-1);
            // nothing to replace: the str itself, as CPython returns it
            if count == 0 || (old.text().is_empty() && new.text().is_empty()) || (!old.text().is_empty() && !t.contains(old.text())) {
                return Ok(o.clone());
            }
            if count < 0 {
                if old.text().is_empty() {
                    let mut out = new.text().to_string();
                    for c in t.chars() {
                        out.push(c);
                        out.push_str(&new.text());
                    }
                    st(out)
                } else {
                    st(t.replace(old.text(), &new.text()))
                }
            } else {
                st(t.replacen(old.text(), &new.text(), count as usize))
            }
        }
        "find" | "rfind" | "index" | "rindex" | "count" => {
            let sub = arg_str(a.p.first().ok_or_else(|| exc("TypeError", format!("{name}() takes at least 1 argument (0 given)")))?, name)?;
            let (cs, ce) = char_range(me, opt_idx(&a, 1)?, opt_idx(&a, 2)?);
            if cs > ce {
                return Ok(V::Int(if name == "count" { 0 } else if name.ends_with("index") { return val_err("substring not found".into()) } else { -1 }));
            }
            let (bs, be) = (byte_at(me, cs), byte_at(me, ce));
            let hay = &t[bs..be];
            if name == "count" {
                if sub.text().is_empty() {
                    return Ok(V::Int((ce - cs + 1) as i64));
                }
                return Ok(V::Int(hay.matches(sub.text()).count() as i64));
            }
            let r = if name.starts_with('r') { hay.rfind(sub.text()) } else { hay.find(sub.text()) };
            match r {
                Some(b) => V::Int(char_at_byte(me, bs + b) as i64),
                None => {
                    if name.ends_with("index") {
                        return val_err("substring not found".into());
                    }
                    V::Int(-1)
                }
            }
        }
        "startswith" | "endswith" => {
            let (cs, ce) = char_range(me, opt_idx(&a, 1)?, opt_idx(&a, 2)?);
            let hay = if cs <= ce { &t[byte_at(me, cs)..byte_at(me, ce)] } else { "" };
            let pats: Vec<V> = match a.p.first() {
                Some(V::Tuple(x)) => x.to_vec(),
                Some(x) => vec![x.clone()],
                None => return type_err(format!("{name}() takes at least 1 argument (0 given)")),
            };
            let mut r = false;
            for p in pats {
                let p = match &p {
                    V::Str(p) => p.clone(),
                    x => return type_err(format!("{name} first arg must be str or a tuple of str, not {}", tn(x))),
                };
                if cs > ce {
                    continue;
                }
                if if name == "startswith" { hay.starts_with(p.text()) } else { hay.ends_with(p.text()) } {
                    r = true;
                    break;
                }
            }
            V::Bool(r)
        }
        "upper" => st(t.to_uppercase()),
        "lower" => st(t.to_lowercase()),
        "casefold" => st(py_casefold(t)),
        "title" => st(py_title(t)),
        "capitalize" => {
            let mut cs = t.chars();
            match cs.next() {
                Some(c) => st(c.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase()),
                None => s(""),
            }
        }
        "swapcase" => st(t.chars().map(|c| if c.is_uppercase() { c.to_lowercase().collect::<String>() } else { c.to_uppercase().collect() }).collect()),
        "isdigit" => V::Bool(py_str_isdigit(t)),
        "isdecimal" => V::Bool(py_str_isdecimal(t)),
        "isnumeric" => V::Bool(py_str_isnumeric(t)),
        "isalpha" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_alphabetic())),
        "isalnum" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_alphanumeric())),
        "isspace" => V::Bool(!t.is_empty() && t.chars().all(|c| c.is_whitespace())),
        "isupper" => V::Bool(t.chars().any(|c| c.is_uppercase()) && !t.chars().any(|c| c.is_lowercase())),
        "islower" => V::Bool(t.chars().any(|c| c.is_lowercase()) && !t.chars().any(|c| c.is_uppercase())),
        "istitle" => V::Bool(t.chars().any(|c| c.is_alphabetic()) && py_title(t) == t),
        "isascii" => V::Bool(t.is_ascii()),
        "isprintable" => V::Bool(t.chars().all(|c| if c.is_ascii() { (' '..='~').contains(&c) } else { !tab_has(PY_NOPRINT, c) })),
        "isidentifier" => V::Bool(py_isidentifier(t)),
        "ljust" | "rjust" | "center" => {
            a.want(name, 1, 2)?;
            let w = idx_of(&a.p[0])?.max(0) as usize;
            let fill = match a.p.get(1) {
                Some(V::Str(f)) if f.nchars() == 1 => f.text().chars().next().unwrap(),
                Some(_) => return type_err("The fill character must be exactly one character long".into()),
                None => ' ',
            };
            let n = me.nchars();
            if n >= w {
                return Ok(o.clone());
            }
            let k = w - n;
            let f = |k: usize| std::iter::repeat(fill).take(k).collect::<String>();
            st(match name {
                "ljust" => format!("{t}{}", f(k)),
                "rjust" => format!("{}{t}", f(k)),
                _ => {
                    let left = k / 2 + (k & w & 1);
                    format!("{}{t}{}", f(left), f(k - left))
                }
            })
        }
        "zfill" => {
            let w = idx_of(&a1(&a, "zfill")?)?.max(0) as usize;
            let n = me.nchars();
            if n >= w {
                return Ok(o.clone());
            }
            let (sign, body) = if t.starts_with('-') || t.starts_with('+') { (&t[..1], &t[1..]) } else { ("", t) };
            st(format!("{sign}{}{body}", "0".repeat(w - n)))
        }
        "partition" | "rpartition" => {
            let sep = arg_str(&a1(&a, name)?, name)?;
            if sep.text().is_empty() {
                return val_err("empty separator".into());
            }
            let r = if name == "partition" { t.find(sep.text()) } else { t.rfind(sep.text()) };
            match r {
                Some(i) => tup(vec![s(&t[..i]), V::Str(sep.clone()), s(&t[i + sep.text().len()..])]),
                None => {
                    if name == "partition" {
                        tup(vec![o.clone(), s(""), s("")])
                    } else {
                        tup(vec![s(""), s(""), o.clone()])
                    }
                }
            }
        }
        "splitlines" => {
            let keep = a.kw("keepends").or_else(|| a.p.first().cloned()).map(|k| truth(&k)).transpose()?.unwrap_or(false);
            let mut out = vec![];
            let cs: Vec<char> = t.chars().collect();
            let mut cur = String::new();
            let mut i = 0;
            while i < cs.len() {
                let c = cs[i];
                if is_linebreak(c) {
                    let mut brk = c.to_string();
                    if c == '\r' && i + 1 < cs.len() && cs[i + 1] == '\n' {
                        brk.push('\n');
                        i += 1;
                    }
                    if keep {
                        cur.push_str(&brk);
                    }
                    out.push(st(std::mem::take(&mut cur)));
                } else {
                    cur.push(c);
                }
                i += 1;
            }
            if !cur.is_empty() {
                out.push(st(cur));
            }
            list(out)
        }
        "format" | "format_map" => return str_method_cps(me, name, a),
        "removeprefix" => {
            let p = arg_str(&a1(&a, name)?, name)?;
            match t.strip_prefix(p.text()) {
                Some(r) => s(r),
                None => o.clone(),
            }
        }
        "removesuffix" => {
            let p = arg_str(&a1(&a, name)?, name)?;
            match t.strip_suffix(p.text()) {
                Some(r) if !p.text().is_empty() => s(r),
                _ => o.clone(),
            }
        }
        "expandtabs" => {
            let ts = opt_idx(&a, 0)?.unwrap_or(8).max(0) as usize;
            let mut out = String::new();
            let mut col = 0;
            for c in t.chars() {
                if c == '\t' {
                    if ts > 0 {
                        let k = ts - col % ts;
                        out.push_str(&" ".repeat(k));
                        col += k;
                    }
                } else if c == '\n' || c == '\r' {
                    out.push(c);
                    col = 0;
                } else {
                    out.push(c);
                    col += 1;
                }
            }
            st(out)
        }
        "translate" => {
            let table = a1(&a, "translate")?;
            let mut out = String::new();
            for c in t.chars() {
                let r = match &table {
                    V::Dict(d) => d.borrow().get(&V::Int(c as i64))?,
                    _ => match getitem(&table, &V::Int(c as i64)) {
                        Ok(v) => Some(v),
                        Err(E(e)) if is_exc(&e, "LookupError") => None,
                        Err(e) => return Err(e),
                    },
                };
                match r {
                    None => out.push(c),
                    Some(V::None) => {}
                    Some(V::Int(i)) => out.push(char::from_u32(i as u32).unwrap_or('?')),
                    Some(V::Str(x)) => out.push_str(&x.text()),
                    Some(x) => return type_err(format!("character mapping must return integer, None or str")),
                }
            }
            st(out)
        }
        "maketrans" => str_maketrans(&a)?,
        "__len__" => V::Int(me.nchars() as i64),
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__lt__" => V::Bool(lt(o, &a1(&a, name)?)?),
        "__hash__" => V::Int(py_hash(o)?),
        "__str__" => o.clone(),
        "__repr__" => st(repr(o)?),
        "__add__" => add(o, &a1(&a, name)?)?,
        "__mul__" => binop(o, &a1(&a, name)?, "*")?,
        "__mod__" => binop(o, &a1(&a, name)?, "%")?,
        "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?)?,
        _ => return bad_method(o, name),
    })
}
fn str_maketrans(a: &Args) -> R {
    if !a.k.is_empty() {
        return type_err("str.maketrans() takes no keyword arguments".into());
    }
    let as_str = |v: &V| match v {
        V::Str(x) => Some(x.clone()),
        V::Obj(o) => o.inner().and_then(|i| match i {
            V::Str(x) => Some(x),
            _ => None,
        }),
        _ => None,
    };
    let mut d = Dict::new();
    match a.p.len() {
        0 => type_err("maketrans expected at least 1 argument, got 0".into()),
        1 => {
            if !matches!(&a.p[0], V::Dict(_)) && !matches!(&a.p[0], V::Obj(o) if matches!(o.inner(), Some(V::Dict(_)))) {
                return type_err("if you give only one argument to maketrans it must be a dict".into());
            }
            for (k, v) in dict_items_of(&a.p[0])? {
                let k = match as_str(&k) {
                    Some(x) => {
                        let mut cs = x.text().chars();
                        match cs.next() {
                            Some(c) if cs.next().is_none() => V::Int(c as i64),
                            _ => return val_err("string keys in translate table must be of length 1".into()),
                        }
                    }
                    None => k.clone(),
                };
                d.set(k, v)?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        2 | 3 => {
            let x = match as_str(&a.p[0]) {
                Some(x) => x,
                None => return type_err("first maketrans argument must be a string if there is a second argument".into()),
            };
            let y = match as_str(&a.p[1]) {
                Some(y) => y,
                None => return type_err(format!("maketrans() argument 2 must be str, not {}", tn(&a.p[1]))),
            };
            if x.nchars() != y.nchars() {
                return val_err("the first two maketrans arguments must have equal length".into());
            }
            for (p, q) in x.text().chars().zip(y.text().chars()) {
                d.set(V::Int(p as i64), V::Int(q as i64))?;
            }
            if let Some(z) = a.p.get(2) {
                let z = match as_str(z) {
                    Some(z) => z,
                    None => return type_err(format!("maketrans() argument 3 must be str, not {}", tn(z))),
                };
                for p in z.text().chars() {
                    d.set(V::Int(p as i64), V::None)?;
                }
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        n => type_err(format!("maketrans expected at most 3 arguments, got {n}")),
    }
}
// "{} {name} {0:>5}".format(...); `mapping` is str.format_map's mapping, resolved lazily with [key].
pub fn str_format_s(t: &S, a: &Args, mapping: Option<&V>) -> Result<S, E> {
    Ok(str_format_auto(t, a, mapping, Some(0), 2)?.0)
}
// `auto` mirrors CPython's auto_arg_index: None once a manual field was seen, Some(next) while
// automatic numbering is in effect. It is threaded through nested format specs, which continue
// the same numbering (and can switch modes), and updated state comes back to the outer string.
// `depth` mirrors CPython's recursion budget: every field consumes one level for its spec, even
// an empty one, so two nested specs are as deep as str.format allows.
fn str_format_auto(ts: &S, a: &Args, mapping: Option<&V>, mut auto: Option<usize>, depth: usize) -> Result<(S, Option<usize>), E> {
    // a lone surrogate in the template is U+FFFD here; literal text is copied from the code points, and a field
    // holding one stops
    let orig = ts.lone();
    let t = ts.text_sub('\u{fffd}');
    let c: Vec<char> = t.chars().collect();
    let mut out = Txt::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '{' {
            if i + 1 < c.len() && c[i + 1] == '{' {
                out.push('{');
                i += 2;
                continue;
            }
            // find matching }
            let mut bdepth = 1;
            let mut j = i + 1;
            while j < c.len() {
                if c[j] == '{' {
                    bdepth += 1;
                } else if c[j] == '}' {
                    bdepth -= 1;
                    if bdepth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if j >= c.len() {
                return val_err("Single '{' encountered in format string".into());
            }
            if orig.is_some_and(|o| o[i + 1..j].iter().any(|&x| is_surrogate(x))) {
                lone_stop(std::panic::Location::caller());
            }
            let field: String = c[i + 1..j].iter().collect();
            i = j + 1;
            if depth == 0 {
                return val_err("Max string recursion exceeded".into());
            }
            let (head, spec) = match field.find(':') {
                Some(p) => (field[..p].to_string(), field[p + 1..].to_string()),
                None => (field.clone(), String::new()),
            };
            let (head, conv) = match head.find('!') {
                Some(p) => (head[..p].to_string(), Some(head[p + 1..].to_string())),
                None => (head, None),
            };
            // first part: index or name, then .attr / [key]
            let name_end = head.find(|ch: char| ch == '.' || ch == '[').unwrap_or(head.len());
            let first = &head[..name_end];
            let mut v = if first.is_empty() {
                if mapping.is_some() {
                    return val_err("Format string contains positional fields".into());
                }
                let k = auto.ok_or_else(|| exc("ValueError", "cannot switch from manual field specification to automatic field numbering".into()))?;
                auto = Some(k + 1);
                a.p.get(k).cloned().ok_or_else(|| exc("IndexError", format!("Replacement index {k} out of range for positional args tuple")))?
            } else if let Ok(k) = first.parse::<usize>() {
                if mapping.is_some() {
                    return val_err("Format string contains positional fields".into());
                }
                if let Some(n) = auto {
                    if n > 0 {
                        return val_err("cannot switch from automatic field numbering to manual field specification".into());
                    }
                }
                auto = None;
                a.p.get(k).cloned().ok_or_else(|| exc("IndexError", format!("Replacement index {k} out of range for positional args tuple")))?
            } else if let Some(m) = mapping {
                getitem(m, &s(first))?
            } else {
                a.k.iter().find(|(n, _)| *n == first).map(|(_, v)| v.clone()).ok_or_else(|| exc_v("KeyError", s(first)))?
            };
            let mut rest = &head[name_end..];
            while !rest.is_empty() {
                if let Some(r) = rest.strip_prefix('.') {
                    let e = r.find(|ch: char| ch == '.' || ch == '[').unwrap_or(r.len());
                    v = getattr(&v, &r[..e])?;
                    rest = &r[e..];
                } else if let Some(r) = rest.strip_prefix('[') {
                    let e = r.find(']').unwrap_or(r.len());
                    let key = &r[..e];
                    let k = match key.parse::<i64>() {
                        Ok(n) => V::Int(n),
                        Err(_) => s(key),
                    };
                    v = getitem(&v, &k)?;
                    rest = &r[(e + 1).min(r.len())..];
                } else {
                    break;
                }
            }
            let v = match conv.as_deref() {
                Some("r") => st(repr(&v)?),
                Some("s") => str_v(&v)?,
                Some("a") => bi_ascii(Args::p(vec![v]))?,
                _ => v,
            };
            // nested fields in the spec continue the same auto numbering
            let spec = if spec.contains('{') {
                let (s, next) = str_format_auto(&S::new(spec), a, mapping, auto, depth - 1)?;
                auto = next;
                s.into_text()
            } else {
                spec
            };
            out.push_s(&fmt_s(&v, &spec)?);
        } else if c[i] == '}' {
            if i + 1 < c.len() && c[i + 1] == '}' {
                out.push('}');
                i += 2;
                continue;
            }
            return val_err("Single '}' encountered in format string".into());
        } else {
            match orig {
                Some(o) => out.push_cp(o[i]),
                None => out.push(c[i]),
            }
            i += 1;
        }
    }
    Ok((out.finish(), auto))
}
// `xs.clear(None)`: CPython's TypeError for a method that takes no arguments.
fn no_args(ty: &str, name: &str, a: &Args) -> Result<(), E> {
    if !a.k.is_empty() {
        return type_err(format!("{ty}.{name}() takes no keyword arguments"));
    }
    if !a.p.is_empty() {
        return type_err(format!("{ty}.{name}() takes no arguments ({} given)", a.p.len()));
    }
    Ok(())
}

pub fn list_method(l: &Rc<RefCell<Vec<V>>>, o: &V, name: &str, mut a: Args) -> R {
    if matches!(name, "reverse" | "copy" | "clear" | "__len__") {
        no_args("list", name, &a)?;
    }
    Ok(match name {
        "append" => {
            let x = a1(&a, "append")?;
            l.borrow_mut().push(x);
            V::None
        }
        // `a.__init__([4, 5, 6])` called on an existing list replaces its items
        "__init__" => {
            a.want("__init__", 0, 1)?;
            let v = match a.p.first() { Some(x) => to_vec(x)?, None => vec![] };
            *l.borrow_mut() = v;
            V::None
        }
        "extend" => {
            let x = a1(&a, "extend")?;
            let v = to_vec(&x)?;
            l.borrow_mut().extend(v);
            V::None
        }
        "insert" => {
            a.want("insert", 2, 2)?;
            let i = idx_of(&a.p[0])?;
            let mut l = l.borrow_mut();
            let n = l.len() as i64;
            let j = if i < 0 { (i + n).max(0) } else { i.min(n) } as usize;
            l.insert(j, a.p[1].clone());
            V::None
        }
        "pop" => {
            if a.p.len() > 1 || !a.k.is_empty() {
                return type_err(format!("pop expected at most 1 argument, got {}", a.p.len() + a.k.len()));
            }
            let mut l = l.borrow_mut();
            if l.is_empty() {
                return Err(exc("IndexError", "pop from empty list".into()));
            }
            match a.p.first() {
                None => l.pop().unwrap(),
                Some(i) => {
                    let i = idx_of(i)?;
                    let n = l.len() as i64;
                    let j = if i < 0 { i + n } else { i };
                    if j < 0 || j >= n {
                        return Err(exc("IndexError", "pop index out of range".into()));
                    }
                    l.remove(j as usize)
                }
            }
        }
        "remove" => {
            let x = a1(&a, "remove")?;
            let v = l.borrow().clone();
            for (i, y) in v.iter().enumerate() {
                if is(y, &x) || eq(y, &x)? {
                    l.borrow_mut().remove(i);
                    return Ok(V::None);
                }
            }
            return val_err("list.remove(x): x not in list".into());
        }
        "index" => {
            let x = a.p.first().cloned().ok_or_else(|| exc("TypeError", "index expected at least 1 argument, got 0".into()))?;
            let v = l.borrow().clone();
            let n = v.len() as i64;
            let fix = |i: i64| if i < 0 { (i + n).max(0) } else { i.min(n) };
            let st0 = fix(opt_idx(&a, 1)?.unwrap_or(0)) as usize;
            let e0 = fix(opt_idx(&a, 2)?.unwrap_or(n)) as usize;
            for i in st0..e0.max(st0) {
                if is(&v[i], &x) || eq(&v[i], &x)? {
                    return Ok(V::Int(i as i64));
                }
            }
            return val_err(format!("{} is not in list", repr(&x)?));
        }
        "count" => {
            let x = a1(&a, "count")?;
            let v = l.borrow().clone();
            let mut k = 0;
            for y in v.iter() {
                if is(y, &x) || eq(y, &x)? {
                    k += 1;
                }
            }
            V::Int(k)
        }
        "sort" => {
            let key = a.kw("key");
            let rev = a.kw("reverse").map(|r| truth(&r)).transpose()?.unwrap_or(false);
            a.no_kw("sort")?;
            if !a.p.is_empty() {
                return type_err("sort() takes no positional arguments".into());
            }
            let mut v = std::mem::take(&mut *l.borrow_mut());
            let r = sort_values(&mut v, key.as_ref(), rev);
            let changed = !l.borrow().is_empty();
            *l.borrow_mut() = v;
            r?;
            if changed {
                return val_err("list modified during sort".into());
            }
            V::None
        }
        "reverse" => {
            l.borrow_mut().reverse();
            V::None
        }
        "copy" => list(l.borrow().clone()),
        "clear" => {
            // The items go last first, as CPython's list_clear frees them, and after the list is released.
            let old = std::mem::take(&mut *l.borrow_mut());
            for x in old.into_iter().rev() {
                drop(x);
            }
            V::None
        }
        "__len__" => V::Int(l.borrow().len() as i64),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__setitem__" => {
            a.want(name, 2, 2)?;
            setitem(o, &a.p[0], a.p[1].clone())?;
            V::None
        }
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__iter__" => iter_v(o)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__add__" => add(o, &a1(&a, name)?)?,
        "__iadd__" => iop(o, &a1(&a, name)?, "+")?,
        "__delitem__" => {
            delitem(o, &a1(&a, name)?)?;
            V::None
        }
        "__repr__" | "__str__" => st(repr(o)?),
        _ => return bad_method(o, name),
    })
}
pub fn dict_method(d: &Rc<RefCell<Dict>>, o: &V, name: &str, mut a: Args) -> R {
    if matches!(name, "keys" | "values" | "items" | "copy" | "clear" | "__len__") {
        no_args("dict", name, &a)?;
    }
    Ok(match name {
        "get" => {
            a.want("get", 1, 2)?;
            let r = Dict::get_cell(d, &a.p[0])?;
            r.unwrap_or_else(|| a.p.get(1).cloned().unwrap_or(V::None))
        }
        "setdefault" => {
            a.want("setdefault", 1, 2)?;
            let v = a.p.get(1).cloned().unwrap_or(V::None);
            let k = a.p[0].clone();
            Dict::set_default_cell(d, k, v)?
        }
        "pop" => {
            a.want("pop", 1, 2)?;
            let r = Dict::del_cell(d, &a.p[0])?;
            match r {
                Some(v) => v,
                None => match a.p.get(1) {
                    Some(x) => x.clone(),
                    None => return Err(exc_v("KeyError", a.p[0].clone())),
                },
            }
        }
        "popitem" => {
            if d.borrow().kind != 3 {
                no_args("dict", name, &a)?;
            }
            let last = a.kw("last").map(|x| truth(&x)).transpose()?.unwrap_or(true);
            let k = {
                let d = d.borrow();
                let ks = d.ents.iter().flatten().map(|(k, _)| k.clone());
                if last {
                    ks.last()
                } else {
                    ks.into_iter().next()
                }
            };
            match k {
                Some(k) => {
                    let v = Dict::del_cell(d, &k)?.unwrap();
                    tup(vec![k, v])
                }
                None => return Err(exc("KeyError", "popitem(): dictionary is empty".into())),
            }
        }
        "keys" => V::View(Rc::new((o.clone(), 0))),
        "values" => V::View(Rc::new((o.clone(), 1))),
        "items" => V::View(Rc::new((o.clone(), 2))),
        "update" => {
            if d.borrow().kind == 2 {
                return counter_update(o, a, true);
            }
            if d.borrow().kind == 3 {
                odict_update(o, a)?;
                return Ok(V::None);
            }
            if a.p.len() > 1 {
                return type_err(format!("update expected at most 1 argument, got {}", a.p.len()));
            }
            if let Some(x) = a.p.first() {
                let other = if let V::Dict(y) = x { Some((y.clone(), y.borrow().n)) } else { None };
                for (k, v) in dict_items_of(x)? {
                    Dict::set_cell(d, k, v)?;
                    if let Some((y, n0)) = &other {
                        if y.try_borrow().map_or(false, |y| y.n != *n0) {
                            return Err(exc("RuntimeError", "dict mutated during update".into()));
                        }
                    }
                }
            }
            for (k, v) in a.k.drain(..) {
                Dict::set_cell(d, s(k), v)?;
            }
            V::None
        }
        "subtract" => return counter_update(o, a, false),
        "__init__" => {
            if d.borrow().kind == 2 {
                return counter_update(o, a, true);
            }
            if d.borrow().kind == 3 {
                return odict_init(o, a);
            }
            return bad_method(o, name);
        }
        "__reduce__" if d.borrow().kind == 3 => {
            no_args("OrderedDict", name, &a)?;
            return odict_reduce(o);
        }
        "__or__" | "__ror__" if d.borrow().kind == 3 => {
            let x = a1(&a, name)?;
            let (l, r) = if name == "__or__" { (o.clone(), x) } else { (x, o.clone()) };
            return odict_or(&l, &r).unwrap_or(Ok(V::NotImpl));
        }
        "__ior__" if d.borrow().kind == 3 => {
            odict_update_arg(o, &a1(&a, name)?)?;
            o.clone()
        }
        "__or__" | "__ror__" => {
            let x = a1(&a, name)?;
            let V::Dict(y) = &x else { return Ok(V::NotImpl) };
            let (first, second) = if name == "__or__" { (d, y) } else { (y, d) };
            let mut r = first.borrow().copy();
            let items = second.borrow().items();
            for (k, v) in items {
                r.set(k, v)?;
            }
            V::Dict(Rc::new(RefCell::new(r)))
        }
        "__ior__" => {
            let x = a1(&a, name)?;
            for (k, v) in dict_items_of(&x)? {
                Dict::set_cell(d, k, v)?;
            }
            o.clone()
        }
        "copy" if d.borrow().kind == 3 => return odict_copy(d),
        "copy" => V::Dict(Rc::new(RefCell::new(d.borrow().copy()))),
        "clear" => {
            d.borrow_mut().clear();
            V::None
        }
        "most_common" => {
            let mut items = d.borrow().items();
            sort_by_key_desc_count(&mut items)?;
            let n = match a.p.first() {
                Some(V::None) | None => items.len(),
                Some(x) => idx_of(x)?.max(0) as usize,
            };
            list(items.into_iter().take(n).map(|(k, v)| tup(vec![k, v])).collect())
        }
        "elements" => {
            let mut out = vec![];
            for (k, v) in d.borrow().items() {
                let n = idx_of(&v).unwrap_or(0);
                for _ in 0..n.max(0) {
                    out.push(k.clone());
                }
            }
            vec_iter(ItKind::ItChain, out)
        }
        "total" => {
            let mut t = V::Int(0);
            for v in d.borrow().values() {
                t = add(&t, &v)?;
            }
            t
        }
        "move_to_end" => {
            let last = a.kw("last").or_else(|| a.p.get(1).cloned()).map(|x| truth(&x)).transpose()?.unwrap_or(true);
            let k = a.p.first().cloned().ok_or_else(|| exc("TypeError", "move_to_end() missing key".into()))?;
            if !Dict::move_to_end_cell(d, &k, last)? {
                return Err(exc_v("KeyError", k));
            }
            V::None
        }
        "__len__" => V::Int(d.borrow().n as i64),
        "__getitem__" => getitem(o, &a1(&a, name)?)?,
        "__setitem__" => {
            a.want(name, 2, 2)?;
            setitem(o, &a.p[0], a.p[1].clone())?;
            V::None
        }
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__delitem__" => {
            // Counter.__delitem__ does not raise for missing keys (dict.__delitem__ does).
            if d.borrow().kind == 2 {
                let k = a1(&a, name)?;
                if Dict::contains_cell(d, &k)? {
                    Dict::del_cell(d, &k)?;
                }
                V::None
            } else {
                delitem(o, &a1(&a, name)?)?;
                V::None
            }
        }
        "__iter__" => iter_v(o)?,
        "__eq__" => V::Bool(eq(o, &a1(&a, name)?)?),
        "__repr__" | "__str__" => st(repr(o)?),
        _ => return bad_method(o, name),
    })
}
pub fn set_method(o: &V, name: &str, a: Args) -> R {
    if matches!(name, "copy" | "clear" | "pop") {
        no_args(if matches!(o, V::FSet(_)) { "frozenset" } else { "set" }, name, &a)?;
    }
    // A copy only for the methods that read the whole set (set_of copies; add must not).
    let me = || set_of(o).unwrap();
    let others = |a: &Args| -> Result<Vec<PSet>, E> {
        let mut v = vec![];
        for x in &a.p {
            v.push(pset_of_iterable(x)?);
        }
        Ok(v)
    };
    let wrap = |p: PSet| -> V {
        if matches!(o, V::FSet(_)) {
            V::FSet(Rc::new(p))
        } else {
            V::Set(Rc::new(RefCell::new(p)))
        }
    };
    let mutable = |o: &V| -> Result<Rc<RefCell<PSet>>, E> {
        match o {
            V::Set(x) => Ok(x.clone()),
            _ => Err(exc("AttributeError", format!("'frozenset' object has no attribute '{name}'"))),
        }
    };
    Ok(match name {
        // set.__init__(iterable=()): empties the set, then adds the iterable's items
        "__init__" if matches!(o, V::Set(_)) => {
            if !a.k.is_empty() {
                return type_err("set() takes no keyword arguments".into());
            }
            if a.p.len() > 1 {
                return type_err(format!("set expected at most 1 argument, got {}", a.p.len()));
            }
            let m = mutable(o)?;
            let items = match a.p.first() {
                Some(x) => Some(pset_of_iterable(x)?),
                None => None,
            };
            PSet::cell_clear(&m);
            if let Some(p) = items {
                PSet::cell_merge(&m, &p)?;
            }
            V::None
        }
        // No borrow of the set is held while a key's __eq__ runs (it may change this set).
        "add" => {
            let x = a1(&a, "add")?;
            PSet::cell_add(&*mutable(o)?, x)?;
            V::None
        }
        "remove" => {
            let x = a1(&a, "remove")?;
            if !PSet::cell_discard(&*mutable(o)?, &x)? {
                return Err(exc_v("KeyError", x));
            }
            V::None
        }
        "discard" => {
            let x = a1(&a, "discard")?;
            PSet::cell_discard(&*mutable(o)?, &x)?;
            V::None
        }
        "pop" => match PSet::cell_pop(&*mutable(o)?) {
            Some(v) => v,
            None => return Err(exc("KeyError", "pop from an empty set".into())),
        },
        "clear" => {
            PSet::cell_clear(&*mutable(o)?);
            V::None
        }
        "copy" => wrap(me().copy()),
        "union" | "intersection" | "difference" | "symmetric_difference" => {
            let op = match name {
                "union" => "|",
                "intersection" => "&",
                "difference" => "-",
                _ => "^",
            };
            let mut r = me().copy();
            for x in others(&a)? {
                r = set_op(&r, &x, op)?;
            }
            wrap(r)
        }
        "update" | "intersection_update" | "difference_update" | "symmetric_difference_update" => {
            let m = mutable(o)?;
            let op = match name {
                "update" => "|",
                "intersection_update" => "&",
                "difference_update" => "-",
                _ => "^",
            };
            // No borrow of m is held while a key's __eq__ runs: it may change m.
            if op == "|" {
                for x in &a.p {
                    match x {
                        // CPython's set_merge: one resize up front, then a table copy or
                        // insertion with the other set's own hashes.
                        V::Set(_) | V::FSet(_) => {
                            let p = set_of(x).unwrap();
                            PSet::cell_merge(&m, &p)?;
                        }
                        // set_update_dict: the dict's stored hashes
                        V::Dict(d) if d.borrow().kind == 0 => {
                            let ks = d.borrow().keys_h();
                            PSet::cell_update_dict(&m, ks)?;
                        }
                        _ => {
                            for k in to_vec(x)? {
                                PSet::cell_add(&m, k)?;
                            }
                        }
                    }
                }
            } else {
                for x in others(&a)? {
                    match op {
                        "-" => {
                            for (k, h) in x.keys_h() {
                                PSet::cell_discard_h(&m, &k, h)?;
                            }
                        }
                        _ => {
                            let cur = m.borrow().clone();
                            let r = set_op(&cur, &x, op)?;
                            PSet::cell_replace(&m, r);
                        }
                    }
                }
            }
            V::None
        }
        "issubset" | "issuperset" | "isdisjoint" => {
            let x = others(&a)?.pop().unwrap_or_else(PSet::new);
            let me = me();
            V::Bool(match name {
                "issubset" => pset_issubset(&me, &x)?,
                "issuperset" => pset_issubset(&x, &me)?,
                _ => {
                    let mut hit = false;
                    for (k, h) in me.keys_h() {
                        if x.find(&k, h)?.is_some() {
                            hit = true;
                            break;
                        }
                    }
                    !hit
                }
            })
        }
        "__len__" => V::Int(match o {
            V::Set(x) => x.borrow().used,
            V::FSet(x) => x.used,
            _ => 0,
        } as i64),
        "__contains__" => V::Bool(contains(o, &a1(&a, name)?)?),
        "__iter__" => iter_v(o)?,
        _ => return bad_method(o, name),
    })
}
// ---------- int.to_bytes / int.from_bytes ----------

// The length argument of to_bytes, converted as CPython's PyLong_AsSsize_t does.
fn to_bytes_len(v: &V) -> Result<i64, E> {
    match v {
        V::Int(i) => Ok(*i),
        V::Bool(b) => Ok(*b as i64),
        V::Big(_) => Err(exc("OverflowError", "Python int too large to convert to C ssize_t".into())),
        V::Obj(o) if o.cls.lookup("__index__").is_some() => {
            let f = o.cls.lookup("__index__").unwrap();
            to_bytes_len(&call(&f, Args::p(vec![v.clone()]))?)
        }
        V::Obj(_) if matches!(inner_of(v), Some(V::Int(_) | V::Bool(_) | V::Big(_))) => to_bytes_len(&inner_of(v).unwrap()),
        V::Ext(x) if is_np(v) => match x.method(v, "__index__", &mut Args::p(vec![])) {
            Some(r) => to_bytes_len(&r?),
            None => type_err(format!("'{}' object cannot be interpreted as an integer", tn(v))),
        },
        x => type_err(format!("'{}' object cannot be interpreted as an integer", tn(x))),
    }
}
// True means little-endian, as _PyLong_ByteOrder parses.
fn byteorder_arg(f: &str, v: &V) -> Result<bool, E> {
    match v {
        V::Str(x) => match x.text() {
            "big" => Ok(false),
            "little" => Ok(true),
            _ => val_err("byteorder must be either 'little' or 'big'".into()),
        },
        x => type_err(format!("{f}() argument 'byteorder' must be str, not {}", tn(x))),
    }
}
pub fn int_to_bytes(o: &V, mut a: Args) -> R {
    let f = "to_bytes";
    // to_bytes(length=1, byteorder='big', *, signed=False)
    if a.p.len() + a.k.len() > 3 {
        return type_err(format!("{f}() takes at most 3 arguments ({} given)", a.p.len() + a.k.len()));
    }
    if a.p.len() > 2 {
        return type_err(format!("{f}() takes at most 2 positional arguments ({} given)", a.p.len()));
    }
    let length = match (a.p.get(0).cloned(), a.kw("length")) {
        (Some(_), Some(_)) => return type_err(format!("argument for {f}() given by name ('length') and position (1)")),
        (Some(p), None) => Some(p),
        (None, k) => k,
    };
    let byteorder = match (a.p.get(1).cloned(), a.kw("byteorder")) {
        (Some(_), Some(_)) => return type_err(format!("argument for {f}() given by name ('byteorder') and position (2)")),
        (Some(p), None) => Some(p),
        (None, k) => k,
    };
    let signed = match a.kw("signed") {
        Some(x) => truth(&x)?,
        None => false,
    };
    a.no_kw(f)?;
    let n = to_bytes_len(&length.unwrap_or(V::Int(1)))?;
    let little = byteorder_arg(f, &byteorder.unwrap_or(s("big")))?;
    if n < 0 {
        return val_err("length argument must be non-negative".into());
    }
    let n = n as usize;
    let b = big_of(o).unwrap();
    if !signed && b.neg {
        return Err(exc("OverflowError", "can't convert negative int to unsigned".into()));
    }
    // The smallest byte count that holds the value (0 fits in 0 bytes only unsigned, or signed).
    let (mag, neg) = if b.neg { (b.neg().sub(&Big::from_i64(1)), true) } else { (b.clone(), false) };
    let min = if b.is_zero() {
        0
    } else if neg {
        // -128 fits in one byte (0x80), -129 does not: the top magnitude bit doubles as the sign.
        (mag.bits() + 8) / 8
    } else if signed {
        (b.bits() + 8) / 8
    } else {
        (b.bits() + 7) / 8
    };
    if n < min {
        return Err(exc("OverflowError", "int too big to convert".into()));
    }
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| exc0("MemoryError"))?;
    out.resize(n, 0);
    // Two's complement: for a negative x write |x|-1 and invert every byte.
    for (i, byte) in out.iter_mut().enumerate() {
        let j = if little { i } else { n - 1 - i };
        let w = mag.mag.get(j / 4).copied().unwrap_or(0);
        *byte = (w >> (8 * (j % 4))) as u8;
        if neg {
            *byte = !*byte;
        }
    }
    Ok(bytesv(out))
}
pub fn int_from_bytes(o: &V, mut a: Args) -> R {
    let f = "from_bytes";
    // from_bytes(bytes, byteorder='big', *, signed=False)
    if a.p.len() + a.k.len() > 3 {
        return type_err(format!("{f}() takes at most 3 arguments ({} given)", a.p.len() + a.k.len()));
    }
    if a.p.len() > 2 {
        return type_err(format!("{f}() takes at most 2 positional arguments ({} given)", a.p.len()));
    }
    let bytes_v = match (a.p.get(0).cloned(), a.kw("bytes")) {
        (Some(_), Some(_)) => return type_err(format!("argument for {f}() given by name ('bytes') and position (1)")),
        (Some(p), None) => Some(p),
        (None, k) => k,
    };
    let Some(bytes_v) = bytes_v else {
        return type_err(format!("{f}() missing required argument 'bytes' (pos 1)"));
    };
    let byteorder = match (a.p.get(1).cloned(), a.kw("byteorder")) {
        (Some(_), Some(_)) => return type_err(format!("argument for {f}() given by name ('byteorder') and position (2)")),
        (Some(p), None) => Some(p),
        (None, k) => k,
    };
    let signed = match a.kw("signed") {
        Some(x) => truth(&x)?,
        None => false,
    };
    a.no_kw(f)?;
    let little = byteorder_arg(f, &byteorder.unwrap_or(s("big")))?;
    let b = bytes_of(&bytes_v).ok_or_else(|| exc("TypeError", format!("cannot convert '{}' object to bytes", tn(&bytes_v))))?;
    let mut words = vec![0u32; (b.len() + 3) / 4];
    if little {
        for (i, &c) in b.iter().enumerate() {
            words[i / 4] |= (c as u32) << (8 * (i % 4));
        }
    } else {
        for (i, &c) in b.iter().enumerate() {
            let j = b.len() - 1 - i;
            words[j / 4] |= (c as u32) << (8 * (j % 4));
        }
    }
    let mut x = Big { neg: false, mag: mtrim(words) };
    if signed && !b.is_empty() {
        let top = if little { b[b.len() - 1] } else { b[0] };
        if top & 0x80 != 0 {
            x = x.sub(&Big::from_i64(1).shl(8 * b.len()));
        }
    }
    // from_bytes is a classmethod: on a bool it returns a bool, as CPython does.
    if let V::Bool(_) = o {
        return Ok(V::Bool(!x.is_zero()));
    }
    Ok(x.norm())
}

pub fn builtin_method(o: &V, name: &str, a: Args) -> R {
    match o {
        V::Str(x) => str_method(x, o, name, a),
        V::List(l) => list_method(l, o, name, a),
        V::Dict(d) => dict_method(d, o, name, a),
        V::Set(_) | V::FSet(_) => set_method(o, name, a),
        V::Deque(d) => deque_method(d, o, name, a),
        V::Tuple(t) => match name {
            "count" => {
                let x = a1(&a, name)?;
                let mut k = 0;
                for y in t.iter() {
                    if is(y, &x) || eq(y, &x)? {
                        k += 1;
                    }
                }
                Ok(V::Int(k))
            }
            "index" => {
                let x = a.p.first().cloned().ok_or_else(|| exc("TypeError", "index expected at least 1 argument, got 0".into()))?;
                for (i, y) in t.iter().enumerate() {
                    if is(y, &x) || eq(y, &x)? {
                        return Ok(V::Int(i as i64));
                    }
                }
                val_err("tuple.index(x): x not in tuple".into())
            }
            "__len__" => Ok(V::Int(t.len() as i64)),
            "__getitem__" => getitem(o, &a1(&a, name)?),
            "__contains__" => Ok(V::Bool(contains(o, &a1(&a, name)?)?)),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            _ => bad_method(o, name),
        },
        V::Int(_) | V::Big(_) | V::Bool(_) => match name {
            "bit_length" => Ok(V::Int(big_of(o).unwrap().bits() as i64)),
            "bit_count" => Ok(V::Int(big_of(o).unwrap().mag.iter().map(|w| w.count_ones() as i64).sum())),
            "to_bytes" => int_to_bytes(o, a),
            "from_bytes" => int_from_bytes(o, a),
            "conjugate" | "__int__" | "__index__" | "__trunc__" | "__floor__" | "__ceil__" => to_int(o),
            "is_integer" => Ok(V::Bool(true)),
            "as_integer_ratio" => Ok(tup(vec![to_int(o)?, V::Int(1)])),
            "__add__" | "__radd__" => add(o, &a1(&a, name)?),
            "__sub__" => sub(o, &a1(&a, name)?),
            "__mul__" | "__rmul__" => binop(o, &a1(&a, name)?, "*"),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__str__" | "__repr__" => str_v(o),
            "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?),
            "__neg__" => neg(o),
            "__abs__" => bi_abs(Args::p(vec![o.clone()])),
            "__float__" => Ok(V::Float(to_float(o)?)),
            "__bool__" => Ok(V::Bool(truth(o)?)),
            _ => bad_method(o, name),
        },
        V::Float(f) => match name {
            "is_integer" => Ok(V::Bool(f.is_finite() && f.fract() == 0.0)),
            "conjugate" | "__float__" => Ok(o.clone()),
            "__int__" | "__trunc__" => to_int(o),
            "__floor__" => to_int(&V::Float(f.floor())),
            "__ceil__" => to_int(&V::Float(f.ceil())),
            "__round__" => bi_round(Args { p: [vec![o.clone()], a.p].concat(), k: vec![] }),
            "as_integer_ratio" => {
                if !f.is_finite() {
                    return val_err("cannot convert Infinity to integer ratio".into());
                }
                let (m, e) = frexp(*f);
                let mut num = m;
                let mut e = e;
                for _ in 0..300 {
                    if num == num.floor() {
                        break;
                    }
                    num *= 2.0;
                    e -= 1;
                }
                let mut n = Big::from_f64(num);
                let mut d = Big::from_i64(1);
                if e > 0 {
                    n = n.shl(e as usize);
                } else {
                    d = d.shl((-e) as usize);
                }
                // reduce
                let g = big_gcd(&n, &d);
                Ok(tup(vec![n.divmod(&g).0.norm(), d.divmod(&g).0.norm()]))
            }
            "hex" => Err(unsupported("float.hex()")),
            "__add__" | "__radd__" => add(o, &a1(&a, name)?),
            "__eq__" => Ok(V::Bool(eq(o, &a1(&a, name)?)?)),
            "__lt__" => Ok(V::Bool(lt(o, &a1(&a, name)?)?)),
            "__hash__" => Ok(V::Int(py_hash(o)?)),
            "__str__" | "__repr__" => str_v(o),
            "__format__" => fmt_v(o, &to_str(&a1(&a, name)?)?),
            "__neg__" => neg(o),
            "__abs__" => Ok(V::Float(fresh_float(f.abs()))),
            "__bool__" => Ok(V::Bool(*f != 0.0)),
            _ => bad_method(o, name),
        },
        V::Iter(it) => match name {
            "__next__" => bi_next(Args::p(vec![o.clone()])),
            "__iter__" => Ok(o.clone()),
            "__reduce__" | "__reduce_ex__" => iter_reduce_v(o),
            "__setstate__" => iter_setstate_v(o, &a1(&a, name)?),
            // CPython's setiter_len: the entries left, or 0 once the set's size changed
            "__length_hint__" if matches!(&*it.it.borrow(), It::Set(..)) || matches!(it.kind, IterClass::Kind(ItKind::SetIter)) => {
                no_args("set_iterator", name, &a)?;
                Ok(V::Int(match &*it.it.borrow() {
                    It::Set(s, i, n0) => {
                        let s = s.borrow();
                        if *n0 >= 0 && s.used as i64 == *n0 {
                            s.t[(*i).min(s.t.len())..].iter().filter(|x| matches!(x, Slot::Full(..))).count() as i64
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }))
            }
            "send" => {
                let c = match &*it.it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return bad_method(o, name),
                };
                let r = c.borrow_mut().resume(a1(&a, name)?);
                match r? {
                    Some(v) => Ok(v),
                    None => Err(stop_with(V::None)),
                }
            }
            // throw(exc) or throw(type[, value[, tb]]): raised at the yield it is suspended at.
            "throw" => {
                let c = match &*it.it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return bad_method(o, name),
                };
                if a.p.is_empty() || a.p.len() > 3 || !a.k.is_empty() {
                    return type_err(format!("throw expected at least 1 argument, got {}", a.p.len()));
                }
                let e = match (&a.p[0], a.p.get(1)) {
                    (V::Class(c), Some(v)) if !matches!(v, V::None) => match make_exc(v.clone()) {
                        Ok(e) if isinstance(&e.0, &V::Class(c.clone()))? => e,
                        _ => E(instantiate(c, Args::p(vec![v.clone()]))?),
                    },
                    (x, _) => make_exc(x.clone())?,
                };
                let r = c.borrow_mut().throw(e);
                match r? {
                    Some(v) => Ok(v),
                    None => Err(stop_with(V::None)),
                }
            }
            "close" => {
                let c = match &*it.it.borrow() {
                    It::Coro(c) => c.clone(),
                    _ => return Ok(V::None),
                };
                let r = c.borrow_mut().close_py();
                r
            }
            _ => bad_method(o, name),
        },
        V::View(v) => match name {
            "__len__" => Ok(V::Int(len(o)? as i64)),
            "__iter__" => iter_v(o),
            "isdisjoint" if v.1 == 0 || v.1 == 2 => {
                let other = to_vec(&a1(&a, name)?)?;
                for x in other {
                    if contains(o, &x)? {
                        return Ok(V::Bool(false));
                    }
                }
                Ok(V::Bool(true))
            }
            "mapping" if v.1 != 3 => {
                let m = V::View(Rc::new((v.0.clone(), 3)));
                call(&m, a)
            }
            "get" | "copy" | "__contains__" | "__getitem__" if v.1 == 3 => call_method(&v.0, name, a),
            "keys" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 0)))),
            "values" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 1)))),
            "items" if v.1 == 3 => Ok(V::View(Rc::new((v.0.clone(), 2)))),
            _ => bad_method(o, name),
        },
        V::Ext(x) => {
            let mut a = a;
            match x.method(o, name, &mut a) {
                Some(r) => r,
                None => bad_method(o, name),
            }
        }
        V::None if name == "__eq__" => Ok(V::Bool(matches!(a.p.first(), Some(V::None)))),
        V::Range(_) => match name {
            "count" => Ok(V::Int(if contains(o, &a1(&a, name)?)? { 1 } else { 0 })),
            "index" => {
                let x = a1(&a, name)?;
                if let (V::Range(r), V::Int(i)) = (o, &x) {
                    if contains(o, &x)? {
                        return Ok(V::Int((i - r.0) / r.2));
                    }
                }
                val_err(format!("{} is not in range", repr(&x)?))
            }
            _ => bad_method(o, name),
        },
        _ => bad_method(o, name),
    }
}
pub fn big_gcd(a: &Big, b: &Big) -> Big {
    let mut x = Big { neg: false, mag: a.mag.clone() };
    let mut y = Big { neg: false, mag: b.mag.clone() };
    while !y.is_zero() {
        let r = x.divmod(&y).1;
        x = y;
        y = r;
    }
    x
}
