
// ---------- str() and repr() ----------

// The shortest round-trip digits as `{:e}` gives them, but a tie between two shortest candidates goes to the
// exact value rounded half-to-even, as in Python's repr (Rust rounds such ties up: -254.62014770507812).
pub fn short_e(x: f64) -> String {
    let s = format!("{:e}", x);
    let n = s.split_once('e').unwrap().0.bytes().filter(|c| c.is_ascii_digit()).count();
    // (two shortest candidates fit only when 15+ digits are needed)
    if n >= 15 {
        let t = format!("{:.*e}", n - 1, x);
        if t != s && t.parse::<f64>() == Ok(x) {
            return t;
        }
    }
    s
}
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let e = short_e(x);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if (-4..16).contains(&exp) {
        if exp < 0 {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        } else {
            let ip = exp as usize + 1;
            if digits.len() <= ip {
                out.push_str(&digits);
                for _ in 0..(ip - digits.len()) {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                out.push_str(&digits[..ip]);
                out.push('.');
                out.push_str(&digits[ip..]);
            }
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exp.abs()));
    }
    out
}
// repr() of a str, lone surrogates included (each is \udXXX: CPython's isprintable() rejects it).
pub fn repr_s(s: &S) -> String {
    let Some(v) = s.lone() else { return str_repr(s.text()) };
    let has = |c: char| v.contains(&(c as u32));
    let q = if has('\'') && !has('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(v.len() + 2);
    out.push(q);
    for &u in v {
        match char::from_u32(u) {
            Some(c) => repr_char(&mut out, c, q),
            None => out.push_str(&format!("\\u{u:04x}")),
        }
    }
    out.push(q);
    out
}
pub fn str_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        repr_char(&mut out, c, q);
    }
    out.push(q);
    out
}
fn repr_char(out: &mut String, c: char, q: char) {
    match c {
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if c == q => {
            out.push('\\');
            out.push(c);
        }
        c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
        // what str.isprintable() rejects: \xNN, \uNNNN or \UNNNNNNNN, as CPython's repr
        c if !c.is_ascii() && tab_has(PY_NOPRINT, c) => {
            let u = c as u32;
            out.push_str(&if u < 0x100 { format!("\\x{u:02x}") } else if u < 0x10000 { format!("\\u{u:04x}") } else { format!("\\U{u:08x}") })
        }
        c => out.push(c),
    }
}
thread_local! {
    static REPR_DEPTH: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}
fn guard(p: usize) -> bool {
    REPR_DEPTH.with(|d| {
        let mut d = d.borrow_mut();
        if d.contains(&p) {
            return false;
        }
        d.push(p);
        true
    })
}
fn unguard() {
    REPR_DEPTH.with(|d| {
        d.borrow_mut().pop();
    });
}
// The subclass a plain OSError(errno, ...) stands for (PermissionError and so on); used by the constructor
// factory, since in CPython 3.13 OSError(errno, strerror, ...) makes the subclass itself. The tables follow
// CPython's errnomodule.c on each target, with that platform's errno.h values.
pub fn ose_errno_name(myerrno: i64) -> &'static str {
    #[cfg(target_os = "macos")]
    const TABLE: &[(i64, &str)] = &[
        (1, "PermissionError"), (2, "FileNotFoundError"), (3, "ProcessLookupError"), (4, "InterruptedError"),
        (10, "ChildProcessError"), (13, "PermissionError"), (17, "FileExistsError"), (20, "NotADirectoryError"),
        (21, "IsADirectoryError"), (32, "BrokenPipeError"), (35, "BlockingIOError"), (36, "BlockingIOError"),
        (37, "BlockingIOError"), (53, "ConnectionAbortedError"), (54, "ConnectionResetError"),
        (58, "BrokenPipeError"), (60, "TimeoutError"), (61, "ConnectionRefusedError"),
    ];
    #[cfg(target_os = "windows")]
    const TABLE: &[(i64, &str)] = &[
        (1, "PermissionError"), (2, "FileNotFoundError"), (3, "ProcessLookupError"), (4, "InterruptedError"),
        (10, "ChildProcessError"), (11, "BlockingIOError"), (13, "PermissionError"), (17, "FileExistsError"),
        (20, "NotADirectoryError"), (21, "IsADirectoryError"), (32, "BrokenPipeError"), (103, "BlockingIOError"),
        (106, "ConnectionAbortedError"), (107, "ConnectionRefusedError"), (108, "ConnectionResetError"),
        (112, "BlockingIOError"), (138, "TimeoutError"), (140, "BlockingIOError"),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    const TABLE: &[(i64, &str)] = &[
        (1, "PermissionError"), (2, "FileNotFoundError"), (3, "ProcessLookupError"), (4, "InterruptedError"),
        (10, "ChildProcessError"), (11, "BlockingIOError"), (13, "PermissionError"), (17, "FileExistsError"),
        (20, "NotADirectoryError"), (21, "IsADirectoryError"), (32, "BrokenPipeError"),
        (103, "ConnectionAbortedError"), (104, "ConnectionResetError"), (108, "BrokenPipeError"),
        (110, "TimeoutError"), (111, "ConnectionRefusedError"), (114, "BlockingIOError"),
        (115, "BlockingIOError"),
    ];
    TABLE.iter().find(|(e, _)| *e == myerrno).map_or("OSError", |(_, n)| *n)
}
// OSError.__str__: the stored fields (filename first, as CPython), or BaseException's own fallback when
// strerror (or errno) was never given or deleted.
pub fn ose_str(o: &Rc<Obj>, args: &V) -> Result<String, E> {
    let errno = o.get("errno");
    let errno_v = errno.clone().unwrap_or(V::None);
    let strerror = o.get("strerror");
    let filename = o.get("filename");
    let filename2 = o.get("filename2");
    if let Some(f) = filename {
        let mut s = format!("[Errno {}] {}: {}", to_str(&errno_v)?, to_str(strerror.as_ref().unwrap_or(&V::None))?, repr(&f)?);
        if let Some(f2) = filename2 {
            s.push_str(&format!(" -> {}", repr(&f2)?));
        }
        return Ok(s);
    }
    if errno.is_some() {
        if let Some(se) = strerror {
            return Ok(format!("[Errno {}] {}", to_str(&errno_v)?, to_str(&se)?));
        }
    }
    Ok(match args {
        V::Tuple(t) if t.is_empty() => String::new(),
        V::Tuple(t) if t.len() == 1 => to_str(&t[0])?,
        _ => repr(args)?,
    })
}
pub fn repr(v: &V) -> Result<String, E> {
    if let V::Ext(_) = v {
        if let Some(r) = sig_str(v, true) {
            return r;
        }
    }
    Ok(match v {
        V::None => "None".into(),
        V::Bool(b) => if *b { "True" } else { "False" }.into(),
        V::Int(i) => i.to_string(),
        V::Big(b) => {
            let s = b.to_string();
            let lim = int_max_digits();
            if lim > 0 && s.len() > lim + b.neg as usize {
                return val_err(format!("Exceeds the limit ({lim} digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit"));
            }
            s
        }
        V::Float(f) => float_repr(*f),
        V::Str(s) => repr_s(s),
        V::List(l) => {
            let p = Rc::as_ptr(l) as usize;
            if !guard(p) {
                return Ok("[...]".into());
            }
            // item by item from the live list: an item's __repr__ may shorten it (CPython prints what is left)
            let mut parts = vec![];
            let mut i = 0;
            let r = loop {
                let x = {
                    let b = l.borrow();
                    if i >= b.len() {
                        break Ok(());
                    }
                    b[i].clone()
                };
                match repr(&x) {
                    Ok(t) => parts.push(t),
                    Err(e) => break Err(e),
                }
                i += 1;
            };
            unguard();
            r?;
            format!("[{}]", parts.join(", "))
        }
        V::Tuple(t) => {
            if t.len() == 1 {
                format!("({},)", repr(&t[0])?)
            } else {
                format!("({})", join_repr(t)?)
            }
        }
        V::Dict(d) => {
            if d.borrow().kind == 2 {
                return counter_repr(d, "Counter");
            }
            let p = Rc::as_ptr(d) as usize;
            if !guard(p) {
                return Ok("{...}".into());
            }
            let (items, kind, dflt) = {
                let d = d.borrow();
                (d.items(), d.kind, d.dflt.clone())
            };
            let mut parts = vec![];
            for (k, x) in &items {
                match (|| -> Result<String, E> { Ok(format!("{}: {}", repr(k)?, repr(x)?)) })() {
                    Ok(s) => parts.push(s),
                    Err(e) => {
                        unguard();
                        return Err(e);
                    }
                }
            }
            unguard();
            let body = format!("{{{}}}", parts.join(", "));
            match kind {
                1 => format!("defaultdict({}, {})", match &dflt {
                    Some(f) => repr(f)?,
                    None => "None".into(),
                }, body),
                2 => {
                    // Counter repr goes through counter_repr above.
                    unreachable!()
                }
                3 => {
                    if items.is_empty() {
                        "OrderedDict()".into()
                    } else {
                        format!("OrderedDict({body})")
                    }
                }
                _ => body,
            }
        }
        V::Set(s) => {
            let ks = s.borrow().keys();
            if ks.is_empty() {
                "set()".into()
            } else {
                format!("{{{}}}", join_repr(&ks)?)
            }
        }
        V::FSet(s) => {
            let ks = s.keys();
            if ks.is_empty() {
                "frozenset()".into()
            } else {
                format!("frozenset({{{}}})", join_repr(&ks)?)
            }
        }
        V::Func(f) => {
            if let Some(V::Str(r)) = f.attrs.borrow().iter().find(|(k, _)| *k == "__bv_repr__").map(|(_, v)| v.clone()) {
                return Ok(r.text().to_string());
            }
            if f.kind.get() == 3 {
                format!("<built-in function {}>", f.name.get())
            } else {
                format!("<function {} at 0x{:x}>", f.qualname.get(), Rc::as_ptr(f) as *const u8 as usize)
            }
        }
        V::Class(c) if meta_has(c, "__repr__") => return meta_str(c, v, "__repr__"),
        V::Class(c) => {
            if is_enum(c) {
                format!("<{} '{}'>", if c.lookup("_flag_mask_").is_some() { "flag" } else { "enum" }, c.name)
            } else if c.module == "builtins" {
                format!("<class '{}'>", c.name)
            } else {
                format!("<class '{}.{}'>", c.module, c.qualname.get())
            }
        }
        V::Obj(o) => {
            // A builtin container subclass without its own __repr__ reprs as the subclass: a Counter subclass
            // prints `MyCounter({...})`, not `Counter({...})`.
            if o.cls.own("__repr__").is_none() {
                if let Some(V::Dict(d)) = inner_of(v) {
                    if d.borrow().kind == 2 {
                        return counter_repr(&d, o.cls.name);
                    }
                }
            }
            if let Some(f) = o.cls.lookup("__repr__") {
                // bound as an attribute lookup would bind it: a @staticmethod __repr__ gets no self
                let r = match &f {
                    V::Func(fun) if fun.kind.get() == 0 => call(&f, Args::p(vec![v.clone()]))?,
                    _ => call(&getattr(v, "__repr__")?, Args::p(vec![]))?,
                };
                return match r {
                    V::Str(s) => Ok(s.text().to_string()),
                    x => type_err(format!("__repr__ returned non-string (type {})", tn(&x))),
                };
            }
            if o.cls.base_bt() == BT::Exc {
                let args = o.get("args").unwrap_or(tup(vec![]));
                let a = match &args {
                    V::Tuple(t) if t.len() == 1 => format!("({})", repr(&t[0])?),
                    _ => repr(&args)?,
                };
                return Ok(format!("{}{}", o.cls.name, a));
            }
            if let Some(i) = inner_of(v) {
                if let V::Dict(d) = &i {
                    if d.borrow().kind == 2 {
                        return counter_repr(d, o.cls.name);
                    }
                }
                // a deque subclass's repr names the instance's type, as CPython's deque_repr does: D([...])
                if let V::Deque(_) = &i {
                    let r = repr(&i)?;
                    return Ok(match r.strip_prefix("deque") {
                        Some(rest) => format!("{}{}", o.cls.name, rest),
                        None => r,
                    });
                }
                // bytearray's repr names the instance's type, as CPython's does: BA(b'..')
                if o.cls.base_bt() == BT::ByteArray {
                    if let Some(b) = bytes_of(&i) {
                        return Ok(format!("{}({})", o.cls.name, bytes_repr_q(&b, true)));
                    }
                }
                return repr(&i);
            }
            // object.__repr__ leaves out the module for builtins (`<object object at 0x...>`)
            if o.cls.module == "builtins" {
                format!("<{} object at 0x{:x}>", o.cls.qualname.get(), Rc::as_ptr(o) as *const u8 as usize)
            } else {
                format!("<{}.{} object at 0x{:x}>", o.cls.module, o.cls.qualname.get(), Rc::as_ptr(o) as *const u8 as usize)
            }
        }
        V::Bound(b) => {
            let fname = match &b.1 {
                V::Func(f) => f.qualname.get(),
                _ => "?",
            };
            format!("<bound method {} of {}>", fname, repr(&b.0)?)
        }
        V::BMeth(b) => format!("<built-in method {} of {} object>", b.1, tn(&b.0)),
        V::Iter(x) => {
            // itertools.repeat keeps its element and remaining count as its state; copy them out before repr,
            // because repr'ing the element may run user code.
            if let IterClass::Kind(ItKind::ItRepeat) = &x.kind {
                if let Some((e, n)) = x.it.try_borrow().ok().and_then(|it| match &*it {
                    It::Repeat(e, n) => Some((e.clone(), *n)),
                    _ => None,
                }) {
                    return Ok(match n {
                        Some(n) => format!("repeat({}, {})", repr(&e)?, n),
                        None => format!("repeat({})", repr(&e)?),
                    });
                }
            }
            // an iterator with CPython's own repr (itertools.count)
            let red = x.it.try_borrow().ok().and_then(|it| match &*it {
                It::Red(r) => Some(r.clone()),
                _ => None,
            });
            if let Some(r) = red {
                if let Some(t) = r.repr() {
                    return t;
                }
            }
            // a generator (or genexpr) reprs with its dotted __qualname__, as in CPython; the cells are held
            // while the generator body runs, so read them only when free (the running generator's name is
            // remembered elsewhere for that case)
            let name = x.it.try_borrow().ok().and_then(|it| match &*it {
                It::Coro(c) => c.try_borrow().ok().map(|c| c.name),
                It::GenExp(_, n, _) => Some(*n),
                _ => None,
            })
            .or_else(running_gen_name);
            match name {
                Some(n) => format!("<generator object {} at 0x{:x}>", n, Rc::as_ptr(x) as *const u8 as usize),
                None => "<iterator object>".into(),
            }
        }
        V::Range(r) => {
            if r.2 == 1 {
                format!("range({}, {})", r.0, r.1)
            } else {
                format!("range({}, {}, {})", r.0, r.1, r.2)
            }
        }
        V::Slice(s) => format!("slice({}, {}, {})", repr(&s.0)?, repr(&s.1)?, repr(&s.2)?),
        V::Deque(d) => {
            // a deque holding itself shows it as [...], as CPython's deque_repr (Py_ReprEnter)
            let p = Rc::as_ptr(d) as *const u8 as usize;
            if !guard(p) {
                return Ok("[...]".into());
            }
            let v: Vec<V> = d.borrow().v.iter().cloned().collect();
            let items = join_repr(&v);
            unguard();
            let items = items?;
            match d.borrow().max {
                Some(m) => format!("deque([{}], maxlen={})", items, m),
                None => format!("deque([{}])", items),
            }
        }
        V::Module(m) => format!("<module '{}'>", m),
        V::View(x) => {
            // kind 3 is a mappingproxy: the dict's own repr in `mappingproxy({...})`.
            if x.1 == 3 {
                if let V::Dict(d) = &x.0 {
                    let p = Rc::as_ptr(d) as usize;
                    if !guard(p) {
                        return Ok("mappingproxy({...})".into());
                    }
                    let mut parts = vec![];
                    // Copy the items out first: a user __repr__ may change the dict.
                    let items = d.borrow().items();
                    let r = (|| -> Result<(), E> {
                        for (k, v) in items {
                            parts.push(format!("{}: {}", repr(&k)?, repr(&v)?));
                        }
                        Ok(())
                    })();
                    unguard();
                    r?;
                    return Ok(format!("mappingproxy({{{}}})", parts.join(", ")));
                }
                return Ok("mappingproxy({})".into());
            }
            let name = ["dict_keys", "dict_values", "dict_items"][x.1 as usize];
            format!("{}([{}])", name, join_repr(&to_vec(v)?)?)
        }
        V::Prop(_) => "<property object>".into(),
        V::NTF(n) => {
            let d = n.doc.borrow().clone();
            match d {
                V::Unbound => format!("_tuplegetter({}, <NULL>)", n.idx),
                d => format!("_tuplegetter({}, {})", n.idx, repr(&d)?),
            }
        }
        V::Cell(c) => {
            let p = Rc::as_ptr(c) as *const u8 as usize;
            // Copy the contents out before repr'ing them: a cell's value may change while its repr runs.
            match &*c.borrow() {
                V::Unbound => return Ok(format!("<cell at {p:#x}: empty>")),
                v => {
                    let v = v.clone();
                    let q = boxed_addr(&v).unwrap_or(0);
                    format!("<cell at {p:#x}: {} object at {q:#x}>", tn(&v))
                }
            }
        }
        V::Ext(x) => x.repr(),
        V::NotImpl => "NotImplemented".into(),
        V::Ellipsis => "Ellipsis".into(),
        V::Unbound => "<unbound>".into(),
    })
}
// CPython's Counter repr: counts sorted highest first, ties in insertion order; non-sortable counts fall back
// to insertion order. `name` is the class name (a Counter subclass reprs as the subclass).
fn counter_repr(d: &Rc<RefCell<Dict>>, name: &str) -> Result<String, E> {
    let p = Rc::as_ptr(d) as usize;
    if !guard(p) {
        return Ok("{...}".into());
    }
    let r = (|| -> Result<String, E> {
        let items = d.borrow().items();
        let mut sorted = items.clone();
        let use_sorted = match sort_by_key_desc_count(&mut sorted) {
            Ok(()) => true,
            Err(e) if is_exc(&e.0, "TypeError") => false,
            Err(e) => return Err(e),
        };
        let shown = if use_sorted { sorted } else { items };
        if shown.is_empty() {
            return Ok(format!("{name}()"));
        }
        let mut ps = vec![];
        for (k, x) in &shown {
            ps.push(format!("{}: {}", repr(k)?, repr(x)?));
        }
        Ok(format!("{name}({{{}}})", ps.join(", ")))
    })();
    unguard();
    r
}
fn sort_by_key_desc_count(items: &mut Vec<(V, V)>) -> Result<(), E> {
    // stable sort by count, highest first (Counter.most_common order)
    let mut keyed: Vec<(V, (V, V))> = items.drain(..).map(|p| (p.1.clone(), p)).collect();
    let mut err = None;
    merge_sort(&mut keyed, &mut |a: &(V, (V, V)), b: &(V, (V, V))| match lt(&b.0, &a.0) {
        Ok(x) => x,
        Err(e) => {
            err.get_or_insert(e);
            false
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    items.extend(keyed.into_iter().map(|x| x.1));
    Ok(())
}
fn join_repr(xs: &[V]) -> Result<String, E> {
    let mut s = String::new();
    for (i, x) in xs.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&repr(x)?);
    }
    Ok(s)
}
pub fn to_str(v: &V) -> Result<String, E> {
    if let Some(r) = sig_str(v, false) {
        return r;
    }
    if let Some(__p) = proxy_deref(v) {
        return to_str(&__p?);
    }
    Ok(match v {
        V::Class(c) if meta_has(c, "__str__") || meta_has(c, "__repr__") => return meta_str(c, v, if meta_has(c, "__str__") { "__str__" } else { "__repr__" }),
        V::Str(s) => s.text().to_string(),
        V::Float(f) => float_repr(*f),
        V::Int(i) => i.to_string(),
        V::Obj(o) => {
            // Counter subclasses str as their own name; an inherited dict __str__ would print `Counter(...)`.
            // A subclass that defines __repr__ but not __str__ strs through its __repr__, as CPython's
            // __str__ slot falls back to __repr__.
            if o.cls.own("__str__").is_none() && o.cls.own("__repr__").is_none() {
                if let Some(V::Dict(d)) = inner_of(v) {
                    if d.borrow().kind == 2 {
                        return counter_repr(&d, o.cls.name);
                    }
                }
            }
            // deque has no __str__ of its own: object.__str__ is the instance's repr (its class's __repr__, or
            // deque's, which names the subclass)
            if matches!(inner_of(v), Some(V::Deque(_))) && o.cls.lookup("__str__").is_none() {
                return repr(v);
            }
            if let Some(f) = o.cls.lookup("__str__") {
                return match call(&f, Args::p(vec![v.clone()]))? {
                    V::Str(s) => Ok(s.text().to_string()),
                    x => type_err(format!("__str__ returned non-string (type {})", tn(&x))),
                };
            }
            if o.cls.base_bt() == BT::Exc {
                let args = o.get("args").unwrap_or(tup(vec![]));
                if o.cls.is_sub(&ose_class()) {
                    return ose_str(o, &args);
                }
                // UnicodeDecodeError(encoding, object, start, end, reason)
                if let V::Tuple(t) = &args {
                    if t.len() == 5 && o.cls.is_sub(&exc_class("UnicodeDecodeError")) {
                        if let (V::Str(e), Some(b), V::Int(a), V::Int(z), V::Str(r)) = (&t[0], bytes_of(&t[1]), &t[2], &t[3], &t[4]) {
                            return Ok(if *z == *a + 1 && (*a as usize) < b.len() {
                                format!("'{}' codec can't decode byte 0x{:02x} in position {a}: {}", e.text(), b[*a as usize], r.text())
                            } else {
                                format!("'{}' codec can't decode bytes in position {a}-{}: {}", e.text(), z - 1, r.text())
                            });
                        }
                    }
                    if t.len() == 5 && o.cls.is_sub(&exc_class("UnicodeEncodeError")) {
                        if let (V::Str(e), V::Str(ob), V::Int(a), V::Int(z), V::Str(r)) = (&t[0], &t[1], &t[2], &t[3], &t[4]) {
                            return Ok(uenc_msg(e.text(), &ob.cps(), *a, *z, r.text()));
                        }
                    }
                }
                return Ok(match &args {
                    V::Tuple(t) if t.is_empty() => String::new(),
                    V::Tuple(t) if t.len() == 1 => {
                        if o.cls.is_sub(&exc_class("KeyError")) {
                            repr(&t[0])?
                        } else {
                            to_str(&t[0])?
                        }
                    }
                    _ => repr(&args)?,
                });
            }
            // str defines its own __str__, so a str subclass's __repr__ never stands in for it (int and float
            // define only __repr__, so theirs does)
            if let Some(i @ V::Str(_)) = inner_of(v) {
                return to_str(&i);
            }
            // bytearray has no __str__ of its own: object's gives the repr, which names the subclass
            if o.cls.lookup("__repr__").is_some() || o.cls.base_bt() == BT::ByteArray {
                return repr(v);
            }
            if let Some(i) = inner_of(v) {
                return to_str(&i);
            }
            repr(v)?
        }
        V::View(x) if x.1 == 3 => repr(&x.0)?,
        V::Ext(x) => x.str_(),
        _ => repr(v)?,
    })
}
pub fn str_v(v: &V) -> R {
    if let V::Str(_) = v {
        return Ok(v.clone());
    }
    Ok(st(to_str(v)?))
}

// ---------- format specs ----------

struct Spec {
    fill: char,
    align: Option<char>,
    sign: char,
    alt: bool,
    zero: bool,
    width: usize,
    group: Option<char>,
    prec: Option<usize>,
    ty: Option<char>,
}
fn parse_spec(spec: &str, v: Option<&V>) -> Result<Spec, E> {
    let c: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut sp = Spec { fill: ' ', align: None, sign: '-', alt: false, zero: false, width: 0, group: None, prec: None, ty: None };
    if c.len() >= 2 && matches!(c[1], '<' | '>' | '^' | '=') {
        sp.fill = c[0];
        sp.align = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && matches!(c[0], '<' | '>' | '^' | '=') {
        sp.align = Some(c[0]);
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        sp.sign = c[i];
        i += 1;
    }
    if i < c.len() && c[i] == 'z' {
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        sp.alt = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        sp.zero = true;
        i += 1;
    }
    let st = i;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i > st {
        sp.width = c[st..i].iter().collect::<String>().parse().unwrap_or(0);
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        sp.group = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == st {
            return val_err("Format specifier missing precision".into());
        }
        sp.prec = Some(c[st..i].iter().collect::<String>().parse().unwrap_or(0));
    }
    if i < c.len() {
        sp.ty = Some(c[i]);
        i += 1;
    }
    if i < c.len() {
        return val_err(match v {
            Some(v) => format!("Invalid format specifier '{spec}' for object of type '{}'", tn(v)),
            None => format!("Invalid format specifier '{spec}'"),
        });
    }
    Ok(sp)
}
fn group_digits(int_part: &str, sep: char, every: usize) -> String {
    let n = int_part.len();
    let mut out = String::new();
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (n - i) % every == 0 {
            out.push(sep);
        }
        out.push(ch);
    }
    out
}
fn pad(body: String, sign: &str, sp: &Spec, default_right: bool) -> String {
    let len = body.chars().count() + sign.chars().count();
    if len >= sp.width {
        return format!("{sign}{body}");
    }
    let n = sp.width - len;
    let (fill, align) = if sp.zero && sp.align.is_none() { ('0', '=') } else { (sp.fill, sp.align.unwrap_or(if default_right { '>' } else { '<' })) };
    let f = |k: usize| std::iter::repeat(fill).take(k).collect::<String>();
    match align {
        '<' => format!("{sign}{body}{}", f(n)),
        '>' => format!("{}{sign}{body}", f(n)),
        '^' => format!("{}{sign}{body}{}", f(n / 2), f(n - n / 2)),
        _ => format!("{sign}{}{body}", f(n)),
    }
}
// Fixed/exponent digits for a float with Python's rules.
fn fmt_float(x: f64, ty: char, prec: Option<usize>, alt: bool) -> String {
    if x.is_nan() {
        return if ty.is_ascii_uppercase() { "NAN".into() } else { "nan".into() };
    }
    if x.is_infinite() {
        return if ty.is_ascii_uppercase() { "INF".into() } else { "inf".into() };
    }
    match ty {
        'f' | 'F' => {
            let p = prec.unwrap_or(6);
            let mut s = format!("{:.*}", p, x.abs());
            if alt && p == 0 {
                s.push('.');
            }
            s
        }
        'e' | 'E' => {
            let p = prec.unwrap_or(6);
            let s = format!("{:.*e}", p, x.abs());
            let (m, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let mut m = m.to_string();
            if alt && p == 0 {
                m.push('.');
            }
            let r = format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs());
            if ty == 'E' {
                r.to_uppercase()
            } else {
                r
            }
        }
        '%' => {
            let p = prec.unwrap_or(6);
            format!("{:.*}%", p, x.abs() * 100.0)
        }
        'g' | 'G' | 'n' => {
            let p = prec.unwrap_or(6).max(1);
            if x == 0.0 {
                return if alt { format!("{:.*}", p - 1, 0.0) } else { "0".into() };
            }
            let s = format!("{:.*e}", p - 1, x.abs());
            let (_, e) = s.split_once('e').unwrap();
            let e: i32 = e.parse().unwrap();
            let r = if e >= -4 && e < p as i32 {
                let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                if !alt && s.contains('.') {
                    s = s.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                s
            } else {
                let (m, _) = s.split_once('e').unwrap();
                let mut m = m.to_string();
                if !alt && m.contains('.') {
                    m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
            };
            if ty == 'G' {
                r.to_uppercase()
            } else {
                r
            }
        }
        _ => {
            // no type: repr-like, or 'g'-like with at least one decimal when a precision is given
            match prec {
                None => float_repr(x.abs()),
                Some(p) => {
                    let p = p.max(1);
                    let s = format!("{:.*e}", p - 1, x.abs());
                    let (_, e) = s.split_once('e').unwrap();
                    let e: i32 = e.parse().unwrap();
                    if e >= -4 && e < p as i32 {
                        let mut s = format!("{:.*}", (p as i32 - 1 - e).max(0) as usize, x.abs());
                        if s.contains('.') {
                            s = s.trim_end_matches('0').to_string();
                            if s.ends_with('.') {
                                s.push('0');
                            }
                        } else {
                            s.push_str(".0");
                        }
                        s
                    } else {
                        let (m, _) = s.split_once('e').unwrap();
                        let mut m = m.to_string();
                        if m.contains('.') {
                            m = m.trim_end_matches('0').trim_end_matches('.').to_string();
                        }
                        format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
                    }
                }
            }
        }
    }
}
fn sign_str(neg: bool, sp: &Spec) -> &'static str {
    if neg {
        "-"
    } else {
        match sp.sign {
            '+' => "+",
            ' ' => " ",
            _ => "",
        }
    }
}
fn group_num(body: &str, sp: &Spec) -> String {
    match sp.group {
        None => body.to_string(),
        Some(g) => {
            let (ip, rest) = match body.find(|c: char| !c.is_ascii_digit()) {
                Some(i) => (&body[..i], &body[i..]),
                None => (body, ""),
            };
            format!("{}{}", group_digits(ip, g, 3), rest)
        }
    }
}
// Where a format spec's 'z' option is, if it has one: after an optional fill-and-align (or align) and sign.
fn z_option_at(spec: &str) -> Option<usize> {
    if !spec.contains('z') {
        return None;
    }
    let c: Vec<(usize, char)> = spec.char_indices().collect();
    let mut i = 0;
    if c.len() >= 2 && matches!(c[1].1, '<' | '>' | '^' | '=') {
        i = 2;
    } else if !c.is_empty() && matches!(c[0].1, '<' | '>' | '^' | '=') {
        i = 1;
    }
    if i < c.len() && matches!(c[i].1, '+' | '-' | ' ') {
        i += 1;
    }
    (i < c.len() && c[i].1 == 'z').then(|| c[i].0)
}
pub fn format_spec(v: &V, spec: &str) -> Result<String, E> {
    match v {
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__format__") {
                return to_str(&call(&f, Args::p(vec![v.clone(), s(spec)]))?);
            }
            // Enum.__format__ formats str(member); IntEnum, StrEnum and IntFlag format the value
            if is_enum(&o.cls) {
                return match inner_of(v) {
                    Some(i) if is_repr_enum(&o.cls) => format_spec(&i, spec),
                    _ => format_spec(&st(to_str(v)?), spec),
                };
            }
            if spec.is_empty() {
                return to_str(v);
            }
            if let Some(i) = inner_of(v) {
                return format_spec(&i, spec);
            }
            return type_err(format!("unsupported format string passed to {}.__format__", o.cls.name));
        }
        V::Ext(x) => {
            if let Some(r) = x.fmt(spec) {
                return to_str(&r?);
            }
            if spec.is_empty() {
                return to_str(v);
            }
        }
        _ => {}
    }
    // the 'z' option (negative zero coercion): after the fill, align and sign, as CPython parses it
    if let Some(k) = z_option_at(spec) {
        let mut rest = spec.to_string();
        rest.remove(k);
        let ty = rest.chars().last().filter(|c| c.is_ascii_alphabetic() || *c == '%');
        let floaty = matches!(ty, Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%' | 'n'));
        match v {
            V::Float(x) => {
                let r = format_spec(v, &rest)?;
                if x.is_sign_negative() && !x.is_nan() && r == format_spec(&V::Float(-0.0), &rest)? {
                    return format_spec(&V::Float(0.0), &rest);
                }
                return Ok(r);
            }
            V::Int(_) | V::Big(_) | V::Bool(_) if floaty && ty != Some('n') => return format_spec(v, &rest),
            V::Int(_) | V::Big(_) | V::Bool(_) => return val_err("Negative zero coercion (z) not allowed in integer format specifier".into()),
            V::Str(_) => return val_err("Negative zero coercion (z) not allowed in string format specifier".into()),
            _ => {}
        }
    }
    if spec.is_empty() {
        return to_str(v);
    }
    let sp = parse_spec(spec, Some(v))?;
    match v {
        V::Str(x) => {
            if let Some(t) = sp.ty {
                if t != 's' {
                    return val_err(format!("Unknown format code '{t}' for object of type 'str'"));
                }
            }
            if sp.sign != '-' {
                return val_err("Sign not allowed in string format specifier".into());
            }
            if sp.align == Some('=') {
                return val_err("'=' alignment not allowed in string format specifier".into());
            }
            let body: String = match sp.prec {
                Some(p) => x.text().chars().take(p).collect(),
                None => x.text().to_string(),
            };
            Ok(pad(body, "", &sp, false))
        }
        V::Int(_) | V::Big(_) | V::Bool(_) if !matches!(sp.ty, Some('e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%')) => {
            if let V::Bool(b) = v {
                if sp.ty.is_none() {
                    return Ok(pad(if *b { "True" } else { "False" }.into(), "", &sp, false));
                }
            }
            let b = big_of(v).unwrap();
            let neg = b.neg;
            let mag = b.neg().clone();
            let a = if neg { mag } else { b.clone() };
            if sp.prec.is_some() {
                return val_err("Precision not allowed in integer format specifier".into());
            }
            let (body, prefix) = match sp.ty {
                None | Some('d') | Some('n') => (a.to_string(), ""),
                Some('x') => (a.to_radix(16), "0x"),
                Some('X') => (a.to_radix(16).to_uppercase(), "0X"),
                Some('o') => (a.to_radix(8), "0o"),
                Some('b') => (a.to_radix(2), "0b"),
                Some('c') => (char::from_u32(a.to_i64().unwrap_or(0) as u32).unwrap_or('?').to_string(), ""),
                Some('s') => return val_err("Unknown format code 's' for object of type 'int'".into()),
                Some(t) => return val_err(format!("Unknown format code '{t}' for object of type 'int'")),
            };
            let body = match sp.group {
                Some(g) => group_digits(&body, g, if matches!(sp.ty, Some('x' | 'X' | 'o' | 'b')) { 4 } else { 3 }),
                None => body,
            };
            let sign = format!("{}{}", sign_str(neg, &sp), if sp.alt { prefix } else { "" });
            if sp.zero && sp.align.is_none() && sp.group.is_some() && body.len() + sign.len() < sp.width {
                // zero padding goes through the grouping
                let digits: String = body.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                let g = sp.group.unwrap();
                let mut d = digits.clone();
                loop {
                    let t = group_digits(&d, g, 3);
                    if t.len() + sign.len() >= sp.width {
                        return Ok(format!("{sign}{t}"));
                    }
                    d.insert(0, '0');
                }
            }
            Ok(pad(body, &sign, &sp, true))
        }
        V::Float(_) | V::Int(_) | V::Big(_) | V::Bool(_) => {
            let x = num_f(v).unwrap();
            if let Some(t) = sp.ty {
                if !matches!(t, 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | 'n' | '%') {
                    return val_err(format!("Unknown format code '{t}' for object of type 'float'"));
                }
            }
            let neg = x.is_sign_negative() && !(x == 0.0 && false) && !x.is_nan();
            let neg = neg && !(x == 0.0 && matches!(v, V::Int(_)));
            let mut body = fmt_float(x, sp.ty.unwrap_or('\0'), sp.prec, sp.alt);
            body = group_num(&body, &sp);
            let sign = sign_str(neg, &sp);
            if sp.zero && sp.align.is_none() && sp.group.is_some() {
                let mut b = body.clone();
                while b.len() + sign.len() < sp.width {
                    let ip: String = b.split(|c: char| c == '.' || c == 'e' || c == '%').next().unwrap().chars().filter(|c| c.is_ascii_digit()).collect();
                    let rest = &b[b.find(|c: char| c == '.' || c == 'e' || c == '%').unwrap_or(b.len())..];
                    let t = group_digits(&format!("0{ip}"), sp.group.unwrap(), 3);
                    b = format!("{t}{rest}");
                }
                return Ok(format!("{sign}{b}"));
            }
            Ok(pad(body, sign, &sp, true))
        }
        V::None | V::List(_) | V::Tuple(_) | V::Dict(_) | V::Set(_) => type_err(format!("unsupported format string passed to {}.__format__", tn(v))),
        _ => {
            let s = to_str(v)?;
            Ok(pad(s, "", &sp, false))
        }
    }
}
pub fn fmt_v(v: &V, spec: &str) -> R {
    Ok(V::Str(Rc::new(fmt_s(v, spec)?)))
}

// The one-conversion, no-flags cases ("%d" % n, "%s" % name, "%f" % x): the general parser below walks the
// format as a Vec<char> and allocates several strings per conversion, so these run through a straight scan.
// Anything it does not recognise falls through to the general (exact) parser.
fn percent_simple(f: &str, args: &V) -> Option<Result<String, E>> {
    if !f.is_ascii() {
        return None;
    }
    let b = f.as_bytes();
    let mut conv: Option<usize> = None;
    for (i, &c) in b.iter().enumerate() {
        if c == b'%' {
            if i + 1 >= b.len() || b[i + 1] == b'%' || conv.is_some() {
                return None;
            }
            conv = Some(i);
        }
    }
    let p = conv?;
    let ty = b[p + 1] as char;
    if !matches!(ty, 'd' | 'i' | 'u' | 's' | 'f') {
        return None;
    }
    let v: &V = match args {
        V::Tuple(t) => {
            let t = t.as_slice();
            if t.len() != 1 {
                return None;
            }
            &t[0]
        }
        V::Dict(_) => return None,
        x => x,
    };
    let piece = match ty {
        'd' | 'i' | 'u' => match v {
            V::Int(i) => format_spec(&V::Int(*i), "d").ok()?,
            V::Bool(x) => format_spec(&V::Int(*x as i64), "d").ok()?,
            _ => return None,
        },
        's' => match v {
            V::Str(s) if s.is_lone() => return None,
            V::Str(s) => s.text().to_string(),
            _ => return None,
        },
        'f' => match v {
            V::Float(x) => format_spec(&V::Float(*x), ".6f").ok()?,
            _ => return None,
        },
        _ => unreachable!(),
    };
    let mut out = String::with_capacity(b.len() - 2 + piece.len());
    out.push_str(&f[..p]);
    out.push_str(&piece);
    out.push_str(&f[p + 2..]);
    Some(Ok(out))
}

// '%5.2f' % x
pub fn percent_format(fs: &S, args: &V) -> Result<S, E> {
    if !fs.is_lone() {
        if let Some(r) = percent_simple(fs.text(), args) {
            return r.map(S::new);
        }
    }
    // the template as chars, with each lone surrogate as U+FFFD (it can only be literal text: a surrogate in a
    // key or a conversion stops), and its code points to copy literal text from
    let orig = fs.lone();
    let sur = |i: usize| orig.is_some_and(|o| i < o.len() && is_surrogate(o[i]));
    let f = fs.text_sub('\u{fffd}');
    let f: &str = &f;
    let (items, dict): (Vec<V>, Option<V>) = match args {
        V::Tuple(t) => (t.to_vec(), None),
        V::Dict(_) => (vec![args.clone()], Some(args.clone())),
        x => (vec![x.clone()], None),
    };
    let c: Vec<char> = f.chars().collect();
    let mut out = Txt::new();
    let mut i = 0;
    let mut k = 0;
    let next = |k: &mut usize| -> Result<V, E> {
        let v = items.get(*k).cloned().ok_or_else(|| exc("TypeError", "not enough arguments for format string".into()))?;
        *k += 1;
        Ok(v)
    };
    while i < c.len() {
        if c[i] != '%' {
            match orig {
                Some(o) => out.push_cp(o[i]),
                None => out.push(c[i]),
            }
            i += 1;
            continue;
        }
        i += 1;
        if i >= c.len() {
            return val_err("incomplete format".into());
        }
        if c[i] == '%' {
            out.push('%');
            i += 1;
            continue;
        }
        let mut key = None;
        if c[i] == '(' {
            let st = i + 1;
            while i < c.len() && c[i] != ')' {
                i += 1;
            }
            if (st..i.min(c.len())).any(sur) {
                lone_stop(std::panic::Location::caller());
            }
            key = Some(c[st..i].iter().collect::<String>());
            i += 1;
        }
        let mut flags = String::new();
        while i < c.len() && matches!(c[i], '-' | '+' | ' ' | '#' | '0') {
            flags.push(c[i]);
            i += 1;
        }
        let mut width = String::new();
        if i < c.len() && c[i] == '*' {
            width = to_str(&next(&mut k)?)?;
            i += 1;
        }
        while i < c.len() && c[i].is_ascii_digit() {
            width.push(c[i]);
            i += 1;
        }
        let mut prec = None;
        if i < c.len() && c[i] == '.' {
            i += 1;
            let mut p = String::new();
            if i < c.len() && c[i] == '*' {
                p = to_str(&next(&mut k)?)?;
                i += 1;
            }
            while i < c.len() && c[i].is_ascii_digit() {
                p.push(c[i]);
                i += 1;
            }
            prec = Some(p.parse::<usize>().unwrap_or(0));
        }
        while i < c.len() && matches!(c[i], 'l' | 'h' | 'L') {
            i += 1;
        }
        if i >= c.len() {
            return val_err("incomplete format".into());
        }
        if sur(i) {
            lone_stop(std::panic::Location::caller());
        }
        let ty = c[i];
        i += 1;
        let v = match &key {
            Some(kk) => getitem(dict.as_ref().ok_or_else(|| exc("TypeError", "format requires a mapping".into()))?, &s(kk))?,
            None => next(&mut k)?,
        };
        let left = flags.contains('-');
        let mut spec = String::new();
        if left {
            spec.push('<');
        } else if !matches!(ty, 's' | 'r' | 'a' | 'c') {
            // numbers right aligned by default
        } else {
            spec.push('>');
        }
        if flags.contains('+') {
            spec.push('+');
        } else if flags.contains(' ') {
            spec.push(' ');
        }
        if flags.contains('#') {
            spec.push('#');
        }
        if flags.contains('0') && !left && !matches!(ty, 's' | 'r' | 'a' | 'c') {
            spec.push('0');
        }
        spec.push_str(&width);
        let piece = match ty {
            's' if matches!(&v, V::Str(x) if x.is_lone()) => {
                let V::Str(x) = &v else { unreachable!() };
                let x = match prec {
                    Some(p) => V::Str(Rc::new(S::from_cps(x.cps().into_iter().take(p).collect()))),
                    None => v.clone(),
                };
                out.push_s(&fmt_s(&x, &spec)?);
                continue;
            }
            'c' if matches!(&v, V::Int(i) if (0xD800..=0xDFFF).contains(i)) || matches!(&v, V::Str(x) if x.is_lone()) => {
                let x = match &v {
                    V::Int(i) => st_cps(vec![*i as u32]),
                    _ if nchars_of(&v) == 1 => v.clone(),
                    _ => return type_err("%c requires int or char".into()),
                };
                out.push_s(&fmt_s(&x, &spec)?);
                continue;
            }
            's' | 'r' | 'a' => {
                let mut t = if ty == 's' { to_str(&v)? } else { repr(&v)? };
                if let Some(p) = prec {
                    t = t.chars().take(p).collect();
                }
                format_spec(&st(t), &spec)?
            }
            'd' | 'i' | 'u' => {
                let n = match &v {
                    V::Float(x) => {
                        if !x.is_finite() {
                            return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
                        }
                        Big::from_f64(*x).norm()
                    }
                    V::Int(_) | V::Big(_) | V::Bool(_) => v.clone(),
                    V::Ext(_) if cplx_of(&v).is_some() => return type_err(format!("%{ty} format: a real number is required, not complex")),
                    V::Ext(_) => to_int(&v)?,
                    // an int subclass (IntEnum): its int
                    V::Obj(_) if matches!(inner_of(&v), Some(V::Int(_) | V::Big(_) | V::Bool(_))) => inner_of(&v).unwrap(),
                    _ => return type_err(format!("%{ty} format: a real number is required, not {}", tn(&v))),
                };
                let n = if let V::Bool(b) = n { V::Int(b as i64) } else { n };
                let mut t = format_spec(&n, &format!("{spec}d"))?;
                if let Some(p) = prec {
                    // precision = minimum digits
                    let neg = t.trim_start().starts_with('-');
                    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
                    if digits.len() < p {
                        let d = format!("{}{}", "0".repeat(p - digits.len()), digits);
                        t = format_spec(&st(format!("{}{}", if neg { "-" } else { "" }, d)), &format!("{}{}", if left { "<" } else { ">" }, width))?;
                    }
                }
                t
            }
            'x' | 'X' | 'o' => {
                let n = match &v {
                    V::Int(_) | V::Big(_) | V::Bool(_) => v.clone(),
                    _ => return type_err(format!("%{ty} format: an integer is required, not {}", tn(&v))),
                };
                let mut sp2 = spec.clone();
                let alt = sp2.contains('#');
                sp2 = sp2.replace('#', "");
                let t = format_spec(&n, &format!("{}{}{ty}", if alt { "#" } else { "" }, sp2))?;
                t
            }
            'e' | 'E' | 'f' | 'F' | 'g' | 'G' => {
                let x = match &v {
                    V::Str(_) | V::None | V::List(_) | V::Tuple(_) | V::Dict(_) => return type_err(format!("must be real number, not {}", tn(&v))),
                    _ if cplx_of(&v).is_some() => return type_err("must be real number, not complex".into()),
                    _ => V::Float(to_float(&v)?),
                };
                format_spec(&x, &format!("{spec}.{}{ty}", prec.unwrap_or(6)))?
            }
            'c' => {
                let t = match &v {
                    V::Int(i) => match u32::try_from(*i).ok().and_then(char::from_u32) {
                        Some(c) => c.to_string(),
                        None => return Err(exc("OverflowError", "%c arg not in range(0x110000)".into())),
                    },
                    V::Str(x) if x.nchars() == 1 => x.text().to_string(),
                    V::Bool(b) => (*b as u8 as char).to_string(),
                    V::Big(_) => return Err(exc("OverflowError", "%c arg not in range(0x110000)".into())),
                    _ => return type_err("%c requires int or char".into()),
                };
                format_spec(&st(t), &spec)?
            }
            _ => return val_err(format!("unsupported format character '{ty}' (0x{:x}) at index {}", ty as u32, i - 1)),
        };
        out.push_str(&piece);
    }
    if k < items.len() && dict.is_none() {
        return type_err("not all arguments converted during string formatting".into());
    }
    Ok(out.finish())
}
fn nchars_of(v: &V) -> usize {
    if let V::Str(x) = v {
        x.nchars()
    } else {
        0
    }
}
// format(v, spec) as a str: a str with a lone surrogate goes through Swap (a str's format only cuts and pads).
pub fn fmt_s(v: &V, spec: &str) -> Result<S, E> {
    if let V::Str(x) = v {
        if x.is_lone() {
            let Some(sw) = Swap::plan(&[x], &[spec]) else { lone_stop(std::panic::Location::caller()) };
            let r = format_spec(&V::Str(Rc::new(sw.to(x))), spec)?;
            return Ok(sw.back_str(&r));
        }
    }
    Ok(S::new(format_spec(v, spec)?))
}

// ---------- numbers from values ----------

pub fn to_float(v: &V) -> Result<f64, E> {
    if let Some(__p) = proxy_deref(v) {
        return to_float(&__p?);
    }
    match v {
        V::Float(f) => Ok(*f),
        V::Int(i) => Ok(*i as f64),
        V::Bool(b) => Ok(*b as i64 as f64),
        V::Big(b) => {
            let f = b.to_f64();
            if f.is_infinite() {
                return Err(exc("OverflowError", "int too large to convert to float".into()));
            }
            Ok(f)
        }
        V::Str(s) => parse_float(&s.text()).ok_or_else(|| exc("ValueError", format!("could not convert string to float: {}", str_repr(&s.text())))),
        V::Obj(o) => match o.cls.lookup("__float__") {
            Some(f) => to_float(&call(&f, Args::p(vec![v.clone()]))?),
            // a bytes or bytearray subclass instance parses as its bytes do
            None if matches!(o.cls.base_bt(), BT::Bytes | BT::ByteArray) => {
                let b = o.inner().and_then(|i| bytes_of(&i)).unwrap_or_default();
                parse_float(&String::from_utf8_lossy(&b)).ok_or_else(|| exc("ValueError", format!("could not convert string to float: {}", repr(v).unwrap_or_default())))
            }
            None => type_err(format!("float() argument must be a string or a real number, not '{}'", tn(v))),
        },
        // a memoryview counts when it gives a simple buffer (live and contiguous), as in CPython
        V::Ext(x) if x.any().is::<BytesV>() || x.any().is::<ByteArrayV>() || (x.any().is::<MemV>() && bytes_of(v).is_some()) => {
            let b = bytes_of(v).unwrap_or_default();
            // (the message shows the object's repr, as CPython's %R does: bytearray(b'x'))
            parse_float(&String::from_utf8_lossy(&b)).ok_or_else(|| exc("ValueError", format!("could not convert string to float: {}", repr(v).unwrap_or_default())))
        }
        V::Ext(x) if x.any().is::<MemV>() => type_err("float() argument must be a string or a real number, not 'memoryview'".into()),
        V::Ext(x) => x.to_f64().ok_or_else(|| exc("TypeError", format!("float() argument must be a string or a real number, not '{}'", tn(v)))),
        _ => type_err(format!("float() argument must be a string or a real number, not '{}'", tn(v))),
    }
}
pub fn parse_float(t: &str) -> Option<f64> {
    let t = t.trim();
    let l = t.to_ascii_lowercase();
    let body = l.trim_start_matches(['+', '-']);
    let neg = l.starts_with('-');
    let r = match body {
        "inf" | "infinity" => Some(f64::INFINITY),
        "nan" => Some(nan_obj()),
        _ => {
            if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | '+' | '-' | '_')) || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
                return None;
            }
            if body.starts_with('e') || body.starts_with('.') && body.len() == 1 {
                return None;
            }
            let b = body.replace('_', "");
            b.parse::<f64>().ok()
        }
    }?;
    Some(if neg { -r } else { r })
}
// sys.set_int_max_str_digits: how many decimal digits int(str) reads and str(int) writes (0: no limit).
static INT_MAX_DIGITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(4300);
pub fn int_max_digits() -> usize {
    INT_MAX_DIGITS.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_int_max_digits(v: &V) -> Result<(), E> {
    let n = idx_of(v)?;
    if n != 0 && n < 640 {
        return val_err("maxdigits must be 0 or larger than 640".into());
    }
    INT_MAX_DIGITS.store(n as usize, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}
// int(text, base) past the limit: CPython counts the digits (not the sign, spaces or underscores), and only
// for a base that is not a power of two.
pub fn int_text_limit(t: &str, base: u32, r: Option<V>) -> Result<Option<V>, E> {
    let lim = int_max_digits();
    if lim == 0 || !matches!(r, Some(V::Big(_))) {
        return Ok(r);
    }
    let lower = t.trim().trim_start_matches(['+', '-']).to_ascii_lowercase();
    let b = match base {
        0 if lower.starts_with("0x") => 16,
        0 if lower.starts_with("0o") => 8,
        0 if lower.starts_with("0b") => 2,
        0 => 10,
        b => b,
    };
    if b.is_power_of_two() {
        return Ok(r);
    }
    let n = lower.chars().filter(|c| *c != '_').count();
    if n > lim {
        return val_err(format!("Exceeds the limit ({lim} digits) for integer string conversion: value has {n} digits; use sys.set_int_max_str_digits() to increase the limit"));
    }
    Ok(r)
}
pub fn parse_int(t: &str, base: u32) -> Option<V> {
    let t = t.trim();
    let (neg, mut body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let mut base = base;
    let lower = body.to_ascii_lowercase();
    if base == 16 && lower.starts_with("0x") || base == 8 && lower.starts_with("0o") || base == 2 && lower.starts_with("0b") {
        body = &body[2..];
    } else if base == 0 {
        if lower.starts_with("0x") {
            base = 16;
            body = &body[2..];
        } else if lower.starts_with("0o") {
            base = 8;
            body = &body[2..];
        } else if lower.starts_with("0b") {
            base = 2;
            body = &body[2..];
        } else {
            base = 10;
            if body.len() > 1 && body.starts_with('0') && body.chars().any(|c| c != '0' && c != '_') {
                return None;
            }
        }
    }
    if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
        return None;
    }
    let digits: String = body.chars().filter(|&c| c != '_').collect();
    if !digits.chars().all(|c| c.is_digit(base)) {
        // allow non-ascii digits for base 10
        if base == 10 {
            if let Some(d) = digits.chars().map(|c| uni_decimal(c).map(|x| std::char::from_digit(x, 10).unwrap())).collect::<Option<String>>() {
                return parse_int(&format!("{}{}", if neg { "-" } else { "" }, d), 10);
            }
        }
        return None;
    }
    if digits.len() < 18 {
        if let Ok(n) = i64::from_str_radix(&digits, base) {
            return Some(V::Int(if neg { -n } else { n }));
        }
    }
    let b = Big::parse(&digits, base)?;
    Some(if neg { b.neg() } else { b }.norm())
}
pub fn to_int(v: &V) -> R {
    if let Some(__p) = proxy_deref(v) {
        return to_int(&__p?);
    }
    match v {
        V::Int(_) | V::Big(_) => Ok(v.clone()),
        V::Bool(b) => Ok(V::Int(*b as i64)),
        V::Float(f) => {
            if f.is_nan() {
                return val_err("cannot convert float NaN to integer".into());
            }
            if f.is_infinite() {
                return Err(exc("OverflowError", "cannot convert float infinity to integer".into()));
            }
            let t = f.trunc();
            if t.abs() < 9.2e18 {
                Ok(V::Int(t as i64))
            } else {
                Ok(Big::from_f64(t).norm())
            }
        }
        // a lone surrogate reads as CPython's '?' (_PyUnicode_TransformDecimalAndSpaceToASCII): never a digit
        V::Str(s) => {
            let t = s.text_sub('?');
            int_text_limit(&t, 10, parse_int(&t, 10))?.ok_or_else(|| exc("ValueError", format!("invalid literal for int() with base 10: {}", repr_s(s))))
        }
        V::Obj(o) => {
            if let Some(f) = o.cls.lookup("__int__") {
                return int_returned(call(&f, Args::p(vec![v.clone()]))?, "__int__");
            }
            // a subclass of int, float or str (IntEnum, StrEnum): int's own __int__ wins over a user __index__
            if let Some(i) = inner_of(v).filter(|i| matches!(i, V::Int(_) | V::Big(_) | V::Bool(_) | V::Float(_) | V::Str(_)) || is_bytes_v(i) || bytearray_of(i).is_some()) {
                return to_int(&i);
            }
            if let Some(f) = o.cls.lookup("__index__") {
                return int_returned(call(&f, Args::p(vec![v.clone()]))?, "__index__");
            }
            if let Some(f) = o.cls.lookup("__trunc__") {
                // CPython 3.13: int() still delegates to __trunc__, with a DeprecationWarning
                re_warn("DeprecationWarning", "The delegation of int() to __trunc__ is deprecated.".into())?;
                let r = call(&f, Args::p(vec![v.clone()]))?;
                return match &r {
                    V::Int(_) | V::Big(_) => Ok(r),
                    V::Bool(b) => Ok(V::Int(*b as i64)),
                    _ => {
                        if let Some(i) = inner_of(&r).filter(|i| matches!(i, V::Int(_) | V::Big(_) | V::Bool(_))) {
                            return to_int(&i);
                        }
                        match &r {
                            V::Obj(ro) if ro.cls.lookup("__index__").is_some() => {
                                let g = ro.cls.lookup("__index__").unwrap();
                                match call(&g, Args::p(vec![r.clone()])).and_then(|x| int_returned(x, "__index__")) {
                                    Ok(x) => Ok(x),
                                    Err(e) if is_exc(&e.0, "TypeError") => type_err(format!("__trunc__ returned non-Integral (type {})", tn(&r))),
                                    Err(e) => Err(e),
                                }
                            }
                            _ => type_err(format!("__trunc__ returned non-Integral (type {})", tn(&r))),
                        }
                    }
                };
            }
            type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v)))
        }
        // int(b"42"), as CPython reads ASCII digits from bytes
        V::Ext(x) if x.any().is::<BytesV>() || x.any().is::<ByteArrayV>() || (x.any().is::<MemV>() && bytes_of(v).is_some()) => bytes_int(&bytes_of(v).unwrap_or_default(), 10),
        V::Ext(x) if x.any().is::<MemV>() => type_err("int() argument must be a string, a bytes-like object or a real number, not 'memoryview'".into()),
        V::Ext(x) => match x.method(v, "__int__", &mut Args::p(vec![])) {
            Some(r) => r,
            None => type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v))),
        },
        _ => type_err(format!("int() argument must be a string, a bytes-like object or a real number, not '{}'", tn(v))),
    }
}

// int(b"42", base): CPython reads the bytes as ASCII text (PyLong_FromString on the raw buffer), so a byte past
// 0x7f (a UTF-8 digit or space included) is never part of a number; the digit limit applies as for str.
pub fn bytes_int(b: &[u8], base: u32) -> R {
    let bad = || exc("ValueError", format!("invalid literal for int() with base {base}: {}", bytes_repr(b)));
    if !b.is_ascii() {
        return Err(bad());
    }
    let t = String::from_utf8_lossy(b);
    int_text_limit(&t, base, parse_int(&t, base))?.ok_or_else(bad)
}

// What int() makes of `r`, the result of a user __int__ or __index__ (CPython's PyNumber_Long and
// PyNumber_Index): an int, or an instance of a strict subclass of int (bool included) as a plain int with
// CPython's DeprecationWarning; anything else is a TypeError.
fn int_returned(r: V, name: &str) -> R {
    match &r {
        V::Int(_) | V::Big(_) => Ok(r),
        _ => {
            let i = match &r {
                V::Bool(b) => Some(V::Int(*b as i64)),
                _ => inner_of(&r).filter(|i| matches!(i, V::Int(_) | V::Big(_) | V::Bool(_))),
            };
            match i {
                Some(i) => {
                    re_warn("DeprecationWarning", format!("{name} returned non-int (type {}).  The ability to return an instance of a strict subclass of int is deprecated, and may be removed in a future version of Python.", tn(&r)))?;
                    to_int(&i)
                }
                None => type_err(format!("{name} returned non-int (type {})", tn(&r))),
            }
        }
    }
}

// Stable merge sort using only a "less than" test (Python's sort only calls <).
pub fn merge_sort<T: Clone>(v: &mut Vec<T>, lt: &mut dyn FnMut(&T, &T) -> bool) {
    let n = v.len();
    if n < 2 {
        return;
    }
    // insertion sort for small runs
    if n <= 16 {
        for i in 1..n {
            let mut j = i;
            while j > 0 && lt(&v[j], &v[j - 1]) {
                v.swap(j, j - 1);
                j -= 1;
            }
        }
        return;
    }
    let mut right = v.split_off(n / 2);
    merge_sort(v, lt);
    merge_sort(&mut right, lt);
    let left = std::mem::take(v);
    let mut out = Vec::with_capacity(n);
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if lt(&right[j], &left[i]) {
            out.push(right[j].clone());
            j += 1;
        } else {
            out.push(left[i].clone());
            i += 1;
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    *v = out;
}
// list.sort(key=..., reverse=...)
pub fn sort_values(v: &mut Vec<V>, key: Option<&V>, reverse: bool) -> Result<(), E> {
    let mut keyed: Vec<(V, V)> = match key {
        Some(k) if !matches!(k, V::None) => {
            let mut out = Vec::with_capacity(v.len());
            for x in v.iter() {
                out.push((call(k, Args::p(vec![x.clone()]))?, x.clone()));
            }
            out
        }
        _ => v.iter().map(|x| (x.clone(), x.clone())).collect(),
    };
    if reverse {
        keyed.reverse();
    }
    let mut err: Option<E> = None;
    let all_int = keyed.iter().all(|(k, _)| matches!(k, V::Int(_)));
    let all_str = keyed.iter().all(|(k, _)| matches!(k, V::Str(_)));
    if all_int {
        keyed.sort_by(|a, b| if let (V::Int(x), V::Int(y)) = (&a.0, &b.0) { x.cmp(y) } else { std::cmp::Ordering::Equal });
    } else if all_str {
        keyed.sort_by(|a, b| if let (V::Str(x), V::Str(y)) = (&a.0, &b.0) { x.order(y) } else { std::cmp::Ordering::Equal });
    } else {
        merge_sort(&mut keyed, &mut |a: &(V, V), b: &(V, V)| {
            if err.is_some() {
                return false;
            }
            match lt(&a.0, &b.0) {
                Ok(r) => r,
                Err(e) => {
                    err = Some(e);
                    false
                }
            }
        });
    }
    if let Some(e) = err {
        return Err(e);
    }
    if reverse {
        keyed.reverse();
    }
    *v = keyed.into_iter().map(|x| x.1).collect();
    Ok(())
}

// The address shown in a cell's repr for a heap value. Ints, floats and other unboxed values have none;
// their cells print 0, which is close enough (cell reprs are not compared by conformance).
fn boxed_addr(v: &V) -> Option<usize> {
    Some(match v {
        V::List(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Tuple(x) => Tup::as_ptr(x) as usize,
        V::Dict(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Cell(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Set(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::FSet(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Func(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Class(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Obj(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Iter(x) => Rc::as_ptr(x) as *const u8 as usize,
        V::Deque(x) => Rc::as_ptr(x) as *const u8 as usize,
        _ => return None,
    })
}

/// The value of a Unicode decimal digit (category Nd, Unicode 15.1): `int("१२३")` is 123, as in CPython.
fn uni_decimal(c: char) -> Option<u32> {
    const ZEROS: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66, 0xde6, 0xe50, 0xed0, 0xf20,
        0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900,
        0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10, 0x104a0, 0x10d30, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0,
        0x11650, 0x116c0, 0x11730, 0x118e0, 0x11950, 0x11c50, 0x11d50, 0x11da0, 0x11f50, 0x16a60, 0x16ac0, 0x16b50, 0x1d7ce, 0x1d7d8,
        0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e950, 0x1fbf0,
    ];
    let u = c as u32;
    ZEROS.iter().find(|&&z| u >= z && u < z + 10).map(|z| u - z)
}

// The non-ASCII characters str.isprintable() rejects, which repr escapes (from CPython 3.13.13, unicodedata 15.1.0)
const PY_NOPRINT: &[(u32, u32)] = &[(0x80,0xa0),(0xad,0xad),(0x378,0x379),(0x380,0x383),(0x38b,0x38b),(0x38d,0x38d),(0x3a2,0x3a2),(0x530,0x530),(0x557,0x558),(0x58b,0x58c),(0x590,0x590),(0x5c8,0x5cf),(0x5eb,0x5ee),(0x5f5,0x605),(0x61c,0x61c),(0x6dd,0x6dd),(0x70e,0x70f),(0x74b,0x74c),(0x7b2,0x7bf),(0x7fb,0x7fc),(0x82e,0x82f),(0x83f,0x83f),(0x85c,0x85d),(0x85f,0x85f),(0x86b,0x86f),(0x88f,0x897),(0x8e2,0x8e2),(0x984,0x984),(0x98d,0x98e),(0x991,0x992),(0x9a9,0x9a9),(0x9b1,0x9b1),(0x9b3,0x9b5),(0x9ba,0x9bb),(0x9c5,0x9c6),(0x9c9,0x9ca),(0x9cf,0x9d6),(0x9d8,0x9db),(0x9de,0x9de),(0x9e4,0x9e5),(0x9ff,0xa00),(0xa04,0xa04),(0xa0b,0xa0e),(0xa11,0xa12),(0xa29,0xa29),(0xa31,0xa31),(0xa34,0xa34),(0xa37,0xa37),(0xa3a,0xa3b),(0xa3d,0xa3d),(0xa43,0xa46),(0xa49,0xa4a),(0xa4e,0xa50),(0xa52,0xa58),(0xa5d,0xa5d),(0xa5f,0xa65),(0xa77,0xa80),(0xa84,0xa84),(0xa8e,0xa8e),(0xa92,0xa92),(0xaa9,0xaa9),(0xab1,0xab1),(0xab4,0xab4),(0xaba,0xabb),(0xac6,0xac6),(0xaca,0xaca),(0xace,0xacf),(0xad1,0xadf),(0xae4,0xae5),(0xaf2,0xaf8),(0xb00,0xb00),(0xb04,0xb04),(0xb0d,0xb0e),(0xb11,0xb12),(0xb29,0xb29),(0xb31,0xb31),(0xb34,0xb34),(0xb3a,0xb3b),(0xb45,0xb46),(0xb49,0xb4a),(0xb4e,0xb54),(0xb58,0xb5b),(0xb5e,0xb5e),(0xb64,0xb65),(0xb78,0xb81),(0xb84,0xb84),(0xb8b,0xb8d),(0xb91,0xb91),(0xb96,0xb98),(0xb9b,0xb9b),(0xb9d,0xb9d),(0xba0,0xba2),(0xba5,0xba7),(0xbab,0xbad),(0xbba,0xbbd),(0xbc3,0xbc5),(0xbc9,0xbc9),(0xbce,0xbcf),(0xbd1,0xbd6),(0xbd8,0xbe5),(0xbfb,0xbff),(0xc0d,0xc0d),(0xc11,0xc11),(0xc29,0xc29),(0xc3a,0xc3b),(0xc45,0xc45),(0xc49,0xc49),(0xc4e,0xc54),(0xc57,0xc57),(0xc5b,0xc5c),(0xc5e,0xc5f),(0xc64,0xc65),(0xc70,0xc76),(0xc8d,0xc8d),(0xc91,0xc91),(0xca9,0xca9),(0xcb4,0xcb4),(0xcba,0xcbb),(0xcc5,0xcc5),(0xcc9,0xcc9),(0xcce,0xcd4),(0xcd7,0xcdc),(0xcdf,0xcdf),(0xce4,0xce5),(0xcf0,0xcf0),(0xcf4,0xcff),(0xd0d,0xd0d),(0xd11,0xd11),(0xd45,0xd45),(0xd49,0xd49),(0xd50,0xd53),(0xd64,0xd65),(0xd80,0xd80),(0xd84,0xd84),(0xd97,0xd99),(0xdb2,0xdb2),(0xdbc,0xdbc),(0xdbe,0xdbf),(0xdc7,0xdc9),(0xdcb,0xdce),(0xdd5,0xdd5),(0xdd7,0xdd7),(0xde0,0xde5),(0xdf0,0xdf1),(0xdf5,0xe00),(0xe3b,0xe3e),(0xe5c,0xe80),(0xe83,0xe83),(0xe85,0xe85),(0xe8b,0xe8b),(0xea4,0xea4),(0xea6,0xea6),(0xebe,0xebf),(0xec5,0xec5),(0xec7,0xec7),(0xecf,0xecf),(0xeda,0xedb),(0xee0,0xeff),(0xf48,0xf48),(0xf6d,0xf70),(0xf98,0xf98),(0xfbd,0xfbd),(0xfcd,0xfcd),(0xfdb,0xfff),(0x10c6,0x10c6),(0x10c8,0x10cc),(0x10ce,0x10cf),(0x1249,0x1249),(0x124e,0x124f),(0x1257,0x1257),(0x1259,0x1259),(0x125e,0x125f),(0x1289,0x1289),(0x128e,0x128f),(0x12b1,0x12b1),(0x12b6,0x12b7),(0x12bf,0x12bf),(0x12c1,0x12c1),(0x12c6,0x12c7),(0x12d7,0x12d7),(0x1311,0x1311),(0x1316,0x1317),(0x135b,0x135c),(0x137d,0x137f),(0x139a,0x139f),(0x13f6,0x13f7),(0x13fe,0x13ff),(0x1680,0x1680),(0x169d,0x169f),(0x16f9,0x16ff),(0x1716,0x171e),(0x1737,0x173f),(0x1754,0x175f),(0x176d,0x176d),(0x1771,0x1771),(0x1774,0x177f),(0x17de,0x17df),(0x17ea,0x17ef),(0x17fa,0x17ff),(0x180e,0x180e),(0x181a,0x181f),(0x1879,0x187f),(0x18ab,0x18af),(0x18f6,0x18ff),(0x191f,0x191f),(0x192c,0x192f),(0x193c,0x193f),(0x1941,0x1943),(0x196e,0x196f),(0x1975,0x197f),(0x19ac,0x19af),(0x19ca,0x19cf),(0x19db,0x19dd),(0x1a1c,0x1a1d),(0x1a5f,0x1a5f),(0x1a7d,0x1a7e),(0x1a8a,0x1a8f),(0x1a9a,0x1a9f),(0x1aae,0x1aaf),(0x1acf,0x1aff),(0x1b4d,0x1b4f),(0x1b7f,0x1b7f),(0x1bf4,0x1bfb),(0x1c38,0x1c3a),(0x1c4a,0x1c4c),(0x1c89,0x1c8f),(0x1cbb,0x1cbc),(0x1cc8,0x1ccf),(0x1cfb,0x1cff),(0x1f16,0x1f17),(0x1f1e,0x1f1f),(0x1f46,0x1f47),(0x1f4e,0x1f4f),(0x1f58,0x1f58),(0x1f5a,0x1f5a),(0x1f5c,0x1f5c),(0x1f5e,0x1f5e),(0x1f7e,0x1f7f),(0x1fb5,0x1fb5),(0x1fc5,0x1fc5),(0x1fd4,0x1fd5),(0x1fdc,0x1fdc),(0x1ff0,0x1ff1),(0x1ff5,0x1ff5),(0x1fff,0x200f),(0x2028,0x202f),(0x205f,0x206f),(0x2072,0x2073),(0x208f,0x208f),(0x209d,0x209f),(0x20c1,0x20cf),(0x20f1,0x20ff),(0x218c,0x218f),(0x2427,0x243f),(0x244b,0x245f),(0x2b74,0x2b75),(0x2b96,0x2b96),(0x2cf4,0x2cf8),(0x2d26,0x2d26),(0x2d28,0x2d2c),(0x2d2e,0x2d2f),(0x2d68,0x2d6e),(0x2d71,0x2d7e),(0x2d97,0x2d9f),(0x2da7,0x2da7),(0x2daf,0x2daf),(0x2db7,0x2db7),(0x2dbf,0x2dbf),(0x2dc7,0x2dc7),(0x2dcf,0x2dcf),(0x2dd7,0x2dd7),(0x2ddf,0x2ddf),(0x2e5e,0x2e7f),(0x2e9a,0x2e9a),(0x2ef4,0x2eff),(0x2fd6,0x2fef),(0x3000,0x3000),(0x3040,0x3040),(0x3097,0x3098),(0x3100,0x3104),(0x3130,0x3130),(0x318f,0x318f),(0x31e4,0x31ee),(0x321f,0x321f),(0xa48d,0xa48f),(0xa4c7,0xa4cf),(0xa62c,0xa63f),(0xa6f8,0xa6ff),(0xa7cb,0xa7cf),(0xa7d2,0xa7d2),(0xa7d4,0xa7d4),(0xa7da,0xa7f1),(0xa82d,0xa82f),(0xa83a,0xa83f),(0xa878,0xa87f),(0xa8c6,0xa8cd),(0xa8da,0xa8df),(0xa954,0xa95e),(0xa97d,0xa97f),(0xa9ce,0xa9ce),(0xa9da,0xa9dd),(0xa9ff,0xa9ff),(0xaa37,0xaa3f),(0xaa4e,0xaa4f),(0xaa5a,0xaa5b),(0xaac3,0xaada),(0xaaf7,0xab00),(0xab07,0xab08),(0xab0f,0xab10),(0xab17,0xab1f),(0xab27,0xab27),(0xab2f,0xab2f),(0xab6c,0xab6f),(0xabee,0xabef),(0xabfa,0xabff),(0xd7a4,0xd7af),(0xd7c7,0xd7ca),(0xd7fc,0xf8ff),(0xfa6e,0xfa6f),(0xfada,0xfaff),(0xfb07,0xfb12),(0xfb18,0xfb1c),(0xfb37,0xfb37),(0xfb3d,0xfb3d),(0xfb3f,0xfb3f),(0xfb42,0xfb42),(0xfb45,0xfb45),(0xfbc3,0xfbd2),(0xfd90,0xfd91),(0xfdc8,0xfdce),(0xfdd0,0xfdef),(0xfe1a,0xfe1f),(0xfe53,0xfe53),(0xfe67,0xfe67),(0xfe6c,0xfe6f),(0xfe75,0xfe75),(0xfefd,0xff00),(0xffbf,0xffc1),(0xffc8,0xffc9),(0xffd0,0xffd1),(0xffd8,0xffd9),(0xffdd,0xffdf),(0xffe7,0xffe7),(0xffef,0xfffb),(0xfffe,0xffff),(0x1000c,0x1000c),(0x10027,0x10027),(0x1003b,0x1003b),(0x1003e,0x1003e),(0x1004e,0x1004f),(0x1005e,0x1007f),(0x100fb,0x100ff),(0x10103,0x10106),(0x10134,0x10136),(0x1018f,0x1018f),(0x1019d,0x1019f),(0x101a1,0x101cf),(0x101fe,0x1027f),(0x1029d,0x1029f),(0x102d1,0x102df),(0x102fc,0x102ff),(0x10324,0x1032c),(0x1034b,0x1034f),(0x1037b,0x1037f),(0x1039e,0x1039e),(0x103c4,0x103c7),(0x103d6,0x103ff),(0x1049e,0x1049f),(0x104aa,0x104af),(0x104d4,0x104d7),(0x104fc,0x104ff),(0x10528,0x1052f),(0x10564,0x1056e),(0x1057b,0x1057b),(0x1058b,0x1058b),(0x10593,0x10593),(0x10596,0x10596),(0x105a2,0x105a2),(0x105b2,0x105b2),(0x105ba,0x105ba),(0x105bd,0x105ff),(0x10737,0x1073f),(0x10756,0x1075f),(0x10768,0x1077f),(0x10786,0x10786),(0x107b1,0x107b1),(0x107bb,0x107ff),(0x10806,0x10807),(0x10809,0x10809),(0x10836,0x10836),(0x10839,0x1083b),(0x1083d,0x1083e),(0x10856,0x10856),(0x1089f,0x108a6),(0x108b0,0x108df),(0x108f3,0x108f3),(0x108f6,0x108fa),(0x1091c,0x1091e),(0x1093a,0x1093e),(0x10940,0x1097f),(0x109b8,0x109bb),(0x109d0,0x109d1),(0x10a04,0x10a04),(0x10a07,0x10a0b),(0x10a14,0x10a14),(0x10a18,0x10a18),(0x10a36,0x10a37),(0x10a3b,0x10a3e),(0x10a49,0x10a4f),(0x10a59,0x10a5f),(0x10aa0,0x10abf),(0x10ae7,0x10aea),(0x10af7,0x10aff),(0x10b36,0x10b38),(0x10b56,0x10b57),(0x10b73,0x10b77),(0x10b92,0x10b98),(0x10b9d,0x10ba8),(0x10bb0,0x10bff),(0x10c49,0x10c7f),(0x10cb3,0x10cbf),(0x10cf3,0x10cf9),(0x10d28,0x10d2f),(0x10d3a,0x10e5f),(0x10e7f,0x10e7f),(0x10eaa,0x10eaa),(0x10eae,0x10eaf),(0x10eb2,0x10efc),(0x10f28,0x10f2f),(0x10f5a,0x10f6f),(0x10f8a,0x10faf),(0x10fcc,0x10fdf),(0x10ff7,0x10fff),(0x1104e,0x11051),(0x11076,0x1107e),(0x110bd,0x110bd),(0x110c3,0x110cf),(0x110e9,0x110ef),(0x110fa,0x110ff),(0x11135,0x11135),(0x11148,0x1114f),(0x11177,0x1117f),(0x111e0,0x111e0),(0x111f5,0x111ff),(0x11212,0x11212),(0x11242,0x1127f),(0x11287,0x11287),(0x11289,0x11289),(0x1128e,0x1128e),(0x1129e,0x1129e),(0x112aa,0x112af),(0x112eb,0x112ef),(0x112fa,0x112ff),(0x11304,0x11304),(0x1130d,0x1130e),(0x11311,0x11312),(0x11329,0x11329),(0x11331,0x11331),(0x11334,0x11334),(0x1133a,0x1133a),(0x11345,0x11346),(0x11349,0x1134a),(0x1134e,0x1134f),(0x11351,0x11356),(0x11358,0x1135c),(0x11364,0x11365),(0x1136d,0x1136f),(0x11375,0x113ff),(0x1145c,0x1145c),(0x11462,0x1147f),(0x114c8,0x114cf),(0x114da,0x1157f),(0x115b6,0x115b7),(0x115de,0x115ff),(0x11645,0x1164f),(0x1165a,0x1165f),(0x1166d,0x1167f),(0x116ba,0x116bf),(0x116ca,0x116ff),(0x1171b,0x1171c),(0x1172c,0x1172f),(0x11747,0x117ff),(0x1183c,0x1189f),(0x118f3,0x118fe),(0x11907,0x11908),(0x1190a,0x1190b),(0x11914,0x11914),(0x11917,0x11917),(0x11936,0x11936),(0x11939,0x1193a),(0x11947,0x1194f),(0x1195a,0x1199f),(0x119a8,0x119a9),(0x119d8,0x119d9),(0x119e5,0x119ff),(0x11a48,0x11a4f),(0x11aa3,0x11aaf),(0x11af9,0x11aff),(0x11b0a,0x11bff),(0x11c09,0x11c09),(0x11c37,0x11c37),(0x11c46,0x11c4f),(0x11c6d,0x11c6f),(0x11c90,0x11c91),(0x11ca8,0x11ca8),(0x11cb7,0x11cff),(0x11d07,0x11d07),(0x11d0a,0x11d0a),(0x11d37,0x11d39),(0x11d3b,0x11d3b),(0x11d3e,0x11d3e),(0x11d48,0x11d4f),(0x11d5a,0x11d5f),(0x11d66,0x11d66),(0x11d69,0x11d69),(0x11d8f,0x11d8f),(0x11d92,0x11d92),(0x11d99,0x11d9f),(0x11daa,0x11edf),(0x11ef9,0x11eff),(0x11f11,0x11f11),(0x11f3b,0x11f3d),(0x11f5a,0x11faf),(0x11fb1,0x11fbf),(0x11ff2,0x11ffe),(0x1239a,0x123ff),(0x1246f,0x1246f),(0x12475,0x1247f),(0x12544,0x12f8f),(0x12ff3,0x12fff),(0x13430,0x1343f),(0x13456,0x143ff),(0x14647,0x167ff),(0x16a39,0x16a3f),(0x16a5f,0x16a5f),(0x16a6a,0x16a6d),(0x16abf,0x16abf),(0x16aca,0x16acf),(0x16aee,0x16aef),(0x16af6,0x16aff),(0x16b46,0x16b4f),(0x16b5a,0x16b5a),(0x16b62,0x16b62),(0x16b78,0x16b7c),(0x16b90,0x16e3f),(0x16e9b,0x16eff),(0x16f4b,0x16f4e),(0x16f88,0x16f8e),(0x16fa0,0x16fdf),(0x16fe5,0x16fef),(0x16ff2,0x16fff),(0x187f8,0x187ff),(0x18cd6,0x18cff),(0x18d09,0x1afef),(0x1aff4,0x1aff4),(0x1affc,0x1affc),(0x1afff,0x1afff),(0x1b123,0x1b131),(0x1b133,0x1b14f),(0x1b153,0x1b154),(0x1b156,0x1b163),(0x1b168,0x1b16f),(0x1b2fc,0x1bbff),(0x1bc6b,0x1bc6f),(0x1bc7d,0x1bc7f),(0x1bc89,0x1bc8f),(0x1bc9a,0x1bc9b),(0x1bca0,0x1ceff),(0x1cf2e,0x1cf2f),(0x1cf47,0x1cf4f),(0x1cfc4,0x1cfff),(0x1d0f6,0x1d0ff),(0x1d127,0x1d128),(0x1d173,0x1d17a),(0x1d1eb,0x1d1ff),(0x1d246,0x1d2bf),(0x1d2d4,0x1d2df),(0x1d2f4,0x1d2ff),(0x1d357,0x1d35f),(0x1d379,0x1d3ff),(0x1d455,0x1d455),(0x1d49d,0x1d49d),(0x1d4a0,0x1d4a1),(0x1d4a3,0x1d4a4),(0x1d4a7,0x1d4a8),(0x1d4ad,0x1d4ad),(0x1d4ba,0x1d4ba),(0x1d4bc,0x1d4bc),(0x1d4c4,0x1d4c4),(0x1d506,0x1d506),(0x1d50b,0x1d50c),(0x1d515,0x1d515),(0x1d51d,0x1d51d),(0x1d53a,0x1d53a),(0x1d53f,0x1d53f),(0x1d545,0x1d545),(0x1d547,0x1d549),(0x1d551,0x1d551),(0x1d6a6,0x1d6a7),(0x1d7cc,0x1d7cd),(0x1da8c,0x1da9a),(0x1daa0,0x1daa0),(0x1dab0,0x1deff),(0x1df1f,0x1df24),(0x1df2b,0x1dfff),(0x1e007,0x1e007),(0x1e019,0x1e01a),(0x1e022,0x1e022),(0x1e025,0x1e025),(0x1e02b,0x1e02f),(0x1e06e,0x1e08e),(0x1e090,0x1e0ff),(0x1e12d,0x1e12f),(0x1e13e,0x1e13f),(0x1e14a,0x1e14d),(0x1e150,0x1e28f),(0x1e2af,0x1e2bf),(0x1e2fa,0x1e2fe),(0x1e300,0x1e4cf),(0x1e4fa,0x1e7df),(0x1e7e7,0x1e7e7),(0x1e7ec,0x1e7ec),(0x1e7ef,0x1e7ef),(0x1e7ff,0x1e7ff),(0x1e8c5,0x1e8c6),(0x1e8d7,0x1e8ff),(0x1e94c,0x1e94f),(0x1e95a,0x1e95d),(0x1e960,0x1ec70),(0x1ecb5,0x1ed00),(0x1ed3e,0x1edff),(0x1ee04,0x1ee04),(0x1ee20,0x1ee20),(0x1ee23,0x1ee23),(0x1ee25,0x1ee26),(0x1ee28,0x1ee28),(0x1ee33,0x1ee33),(0x1ee38,0x1ee38),(0x1ee3a,0x1ee3a),(0x1ee3c,0x1ee41),(0x1ee43,0x1ee46),(0x1ee48,0x1ee48),(0x1ee4a,0x1ee4a),(0x1ee4c,0x1ee4c),(0x1ee50,0x1ee50),(0x1ee53,0x1ee53),(0x1ee55,0x1ee56),(0x1ee58,0x1ee58),(0x1ee5a,0x1ee5a),(0x1ee5c,0x1ee5c),(0x1ee5e,0x1ee5e),(0x1ee60,0x1ee60),(0x1ee63,0x1ee63),(0x1ee65,0x1ee66),(0x1ee6b,0x1ee6b),(0x1ee73,0x1ee73),(0x1ee78,0x1ee78),(0x1ee7d,0x1ee7d),(0x1ee7f,0x1ee7f),(0x1ee8a,0x1ee8a),(0x1ee9c,0x1eea0),(0x1eea4,0x1eea4),(0x1eeaa,0x1eeaa),(0x1eebc,0x1eeef),(0x1eef2,0x1efff),(0x1f02c,0x1f02f),(0x1f094,0x1f09f),(0x1f0af,0x1f0b0),(0x1f0c0,0x1f0c0),(0x1f0d0,0x1f0d0),(0x1f0f6,0x1f0ff),(0x1f1ae,0x1f1e5),(0x1f203,0x1f20f),(0x1f23c,0x1f23f),(0x1f249,0x1f24f),(0x1f252,0x1f25f),(0x1f266,0x1f2ff),(0x1f6d8,0x1f6db),(0x1f6ed,0x1f6ef),(0x1f6fd,0x1f6ff),(0x1f777,0x1f77a),(0x1f7da,0x1f7df),(0x1f7ec,0x1f7ef),(0x1f7f1,0x1f7ff),(0x1f80c,0x1f80f),(0x1f848,0x1f84f),(0x1f85a,0x1f85f),(0x1f888,0x1f88f),(0x1f8ae,0x1f8af),(0x1f8b2,0x1f8ff),(0x1fa54,0x1fa5f),(0x1fa6e,0x1fa6f),(0x1fa7d,0x1fa7f),(0x1fa89,0x1fa8f),(0x1fabe,0x1fabe),(0x1fac6,0x1facd),(0x1fadc,0x1fadf),(0x1fae9,0x1faef),(0x1faf9,0x1faff),(0x1fb93,0x1fb93),(0x1fbcb,0x1fbef),(0x1fbfa,0x1ffff),(0x2a6e0,0x2a6ff),(0x2b73a,0x2b73f),(0x2b81e,0x2b81f),(0x2cea2,0x2ceaf),(0x2ebe1,0x2ebef),(0x2ee5e,0x2f7ff),(0x2fa1e,0x2ffff),(0x3134b,0x3134f),(0x323b0,0xe00ff),(0xe01f0,0x10ffff)];
