// ---------- ONNX Runtime, for Brevi's onnxruntime (pasted into the program; see _brevi_ort in rt_ort.rs) ----------
// The engine is ONNX Runtime's own, through its C API (OrtGetApiBase): libonnxruntime, $BREVI_ORT_LIB (outside
// the sandbox), or the library inside the onnxruntime Python package found when the program was built (ORT_LIBS).
// It needs no CPython. Under --sandbox a model file (and its external data) opens only with the read grant;
// optimized_model_filepath, profiling and session settings naming files need the write grant too.
// FS_SANDBOX / FS_READ / FS_WRITE are set by the compiler.
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod brevi_ortglue {
    use super::*;
    use brevi_dyn::{OrtArg, OrtMeta, OrtOps, OrtOpts, OrtOut, OrtSess, OrtTensor};
    use std::collections::HashMap;
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::{Mutex, OnceLock};

    type P = *mut c_void;
    type St = P;

    // libonnxruntime is dlopen'd on Unix; on Windows the same library (onnxruntime.dll) is LoadLibraryA'd, and
    // GetLastError names what a failed load wanted.
    #[cfg(unix)]
    extern "C" {
        fn dlopen(f: *const c_char, flag: i32) -> P;
        fn dlsym(h: P, s: *const c_char) -> P;
        fn dlerror() -> *const c_char;
    }
    #[cfg(windows)]
    unsafe extern "system" {
        fn LoadLibraryA(f: *const c_char) -> P;
        fn GetProcAddress(h: P, s: *const c_char) -> P;
        fn GetLastError() -> u32;
        fn FreeLibrary(h: P) -> i32;
    }
    #[cfg(unix)]
    unsafe fn ax_dlopen(f: *const c_char, flag: i32) -> P {
        dlopen(f, flag)
    }
    #[cfg(windows)]
    unsafe fn ax_dlopen(f: *const c_char, _flag: i32) -> P {
        LoadLibraryA(f)
    }
    #[cfg(unix)]
    unsafe fn ax_freelib(_h: P) {}
    #[cfg(windows)]
    unsafe fn ax_freelib(h: P) {
        FreeLibrary(h);
    }
    #[cfg(unix)]
    unsafe fn ax_dlsym(h: P, s: *const c_char) -> P {
        dlsym(h, s)
    }
    #[cfg(windows)]
    unsafe fn ax_dlsym(h: P, s: *const c_char) -> P {
        GetProcAddress(h, s)
    }
    #[cfg(unix)]
    unsafe fn ax_dlerror() -> String {
        let e = dlerror();
        if e.is_null() {
            String::new()
        } else {
            CStr::from_ptr(e).to_string_lossy().into_owned()
        }
    }
    #[cfg(windows)]
    unsafe fn ax_dlerror() -> String {
        let c = GetLastError();
        format!("{} (Win32 error {c})", std::io::Error::from_raw_os_error(c as i32))
    }

    #[repr(C)]
    struct ApiBase {
        get_api: unsafe extern "C" fn(u32) -> *const P,
        version: unsafe extern "C" fn() -> *const c_char,
    }

    struct Api {
        t: *const P,
        version: String,
        minor: u32,
        env: P,
        mem: P,
        alloc: P,
    }
    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    // the C API's functions by their place in OrtApi (onnxruntime_c_api.h)
    macro_rules! f {
        ($api:expr, $i:expr, $t:ty) => {
            std::mem::transmute::<P, $t>(*$api.t.add($i))
        };
    }

    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    static SESS: Mutex<Vec<(i64, usize)>> = Mutex::new(Vec::new());
    static NEXT: Mutex<i64> = Mutex::new(1);

    fn cstr(p: *const c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }

    // ONNX Runtime's ORTCHAR_T: char on Unix, wchar_t (UTF-16) on Windows. Model paths, optimized-model paths and
    // profiling prefixes reach the C API in that spelling; names, tags and config keys are char* everywhere.
    #[cfg(not(windows))]
    fn ortchars(s: &str) -> Option<CString> {
        CString::new(s).ok()
    }
    #[cfg(windows)]
    fn ortchars(s: &str) -> Option<Vec<u16>> {
        if s.contains('\0') {
            None
        } else {
            Some(s.encode_utf16().chain(std::iter::once(0)).collect())
        }
    }
    #[cfg(not(windows))]
    fn ortchars_ptr(c: &CString) -> *const c_char {
        c.as_ptr()
    }
    #[cfg(windows)]
    fn ortchars_ptr(c: &[u16]) -> *const c_char {
        c.as_ptr() as *const c_char
    }

    fn libs() -> Vec<String> {
        let mut v = vec![];
        if !FS_SANDBOX {
            if let Ok(p) = std::env::var("BREVI_ORT_LIB") {
                v.push(p);
            }
        }
        for l in ORT_LIBS {
            v.push(l.to_string());
        }
        #[cfg(unix)]
        for l in ["libonnxruntime.so", "libonnxruntime.so.1", "libonnxruntime.dylib"] {
            v.push(l.to_string());
        }
        // Windows has no bare-name fallback: an onnxruntime.dll in the program's folder or CWD must not
        // run. ORT_LIBS holds the absolute path from the onnxruntime package found at build time;
        // BREVI_ORT_LIB is the explicit override.
        v
    }

    fn api() -> Result<&'static Api, String> {
        API.get_or_init(|| unsafe {
            let mut why = String::new();
            for l in libs() {
                let c = CString::new(l.clone()).unwrap();
                let h = ax_dlopen(c.as_ptr(), 2);
                if h.is_null() {
                    why = ax_dlerror();
                    continue;
                }
                let g = ax_dlsym(h, c"OrtGetApiBase".as_ptr());
                if g.is_null() {
                    // a DLL that is not ONNX Runtime's own (an old wheel's onnxruntime_providers_shared.dll):
                    // unload it and keep looking
                    ax_freelib(h);
                    why = format!("{l} has no OrtGetApiBase");
                    continue;
                }
                let get: unsafe extern "C" fn() -> *const ApiBase = std::mem::transmute(g);
                let base = &*get();
                let version = cstr((base.version)());
                let minor: u32 = version.split('.').nth(1).and_then(|m| m.parse().ok()).unwrap_or(0);
                if minor < 16 {
                    // loaded, but too old to serve this program: keep looking for a newer library
                    ax_freelib(h);
                    why = format!("ONNX Runtime {version} is older than 1.16");
                    continue;
                }
                let t = (base.get_api)(minor.min(29));
                if t.is_null() {
                    ax_freelib(h);
                    why = format!("ONNX Runtime {version} gave no C API");
                    continue;
                }
                let mut a = Api { t, version, minor, env: std::ptr::null_mut(), mem: std::ptr::null_mut(), alloc: std::ptr::null_mut() };
                // CreateEnv(warning, "Default"): onnxruntime's Python binding logs at warning level too
                let mut env: P = std::ptr::null_mut();
                let st = f!(a, 3, unsafe extern "C" fn(i32, *const c_char, *mut P) -> St)(2, c"Default".as_ptr(), &mut env);
                if let Err((_, m)) = check(&a, st) {
                    return Err(m);
                }
                a.env = env;
                let mut mem: P = std::ptr::null_mut();
                check(&a, f!(a, 69, unsafe extern "C" fn(i32, i32, *mut P) -> St)(1, 0, &mut mem)).map_err(|e| e.1)?;
                a.mem = mem;
                let mut al: P = std::ptr::null_mut();
                check(&a, f!(a, 78, unsafe extern "C" fn(*mut P) -> St)(&mut al)).map_err(|e| e.1)?;
                a.alloc = al;
                return Ok(a);
            }
            Err(format!("brevi: Brevi's onnxruntime needs ONNX Runtime (the onnxruntime package, or libonnxruntime): {why}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
    }

    // an OrtStatus: Ok, or (code, message), released
    unsafe fn check(a: &Api, s: St) -> Result<(), (i64, String)> {
        if s.is_null() {
            return Ok(());
        }
        let code = f!(a, 1, unsafe extern "C" fn(St) -> i32)(s);
        let msg = cstr(f!(a, 2, unsafe extern "C" fn(St) -> *const c_char)(s));
        f!(a, 93, unsafe extern "C" fn(St))(s);
        Err((code as i64, msg))
    }

    unsafe fn take_str(a: &Api, p: *mut c_char) -> String {
        let s = cstr(p);
        if !p.is_null() {
            f!(a, 76, unsafe extern "C" fn(P, P) -> St)(a.alloc, p as P);
        }
        s
    }

    fn load() -> Result<String, String> {
        api().map(|a| a.version.clone())
    }

    fn providers() -> Vec<String> {
        let Ok(a) = api() else { return vec![] };
        unsafe {
            let mut p: *mut *mut c_char = std::ptr::null_mut();
            let mut n: i32 = 0;
            if check(a, f!(a, 125, unsafe extern "C" fn(*mut *mut *mut c_char, *mut i32) -> St)(&mut p, &mut n)).is_err() {
                return vec![];
            }
            let v = (0..n as usize).map(|i| cstr(*p.add(i))).collect();
            f!(a, 126, unsafe extern "C" fn(*mut *mut c_char, i32) -> St)(p, n);
            v
        }
    }

    fn build_info() -> String {
        let Ok(a) = api() else { return String::new() };
        if a.minor < 15 {
            return String::new();
        }
        unsafe { cstr(f!(a, 254, unsafe extern "C" fn() -> *const c_char)()) }
    }

    fn elem_name(e: i32) -> &'static str {
        match e {
            1 => "float",
            2 => "uint8",
            3 => "int8",
            4 => "uint16",
            5 => "int16",
            6 => "int32",
            7 => "int64",
            8 => "string",
            9 => "bool",
            10 => "float16",
            11 => "double",
            12 => "uint32",
            13 => "uint64",
            14 => "complex64",
            15 => "complex128",
            16 => "bfloat16",
            17 => "float8e4m3fn",
            18 => "float8e4m3fnuz",
            19 => "float8e5m2",
            20 => "float8e5m2fnuz",
            21 => "uint4",
            22 => "int4",
            23 => "float4e2m1",
            _ => "undefined",
        }
    }

    // the type's name as onnx writes it, and a tensor's shape
    unsafe fn type_of(a: &Api, ti: P) -> (String, Option<Vec<(Option<i64>, String)>>) {
        let mut kind: i32 = 0;
        if check(a, f!(a, 56, unsafe extern "C" fn(P, *mut i32) -> St)(ti, &mut kind)).is_err() {
            return ("unknown".into(), None);
        }
        match kind {
            // tensor, sparse tensor
            1 | 5 => {
                let mut info: P = std::ptr::null_mut();
                if check(a, f!(a, 55, unsafe extern "C" fn(P, *mut P) -> St)(ti, &mut info)).is_err() || info.is_null() {
                    return ("tensor(undefined)".into(), None);
                }
                let mut e: i32 = 0;
                let _ = check(a, f!(a, 60, unsafe extern "C" fn(P, *mut i32) -> St)(info, &mut e));
                let mut n: usize = 0;
                let _ = check(a, f!(a, 61, unsafe extern "C" fn(P, *mut usize) -> St)(info, &mut n));
                let mut dims = vec![0i64; n];
                let _ = check(a, f!(a, 62, unsafe extern "C" fn(P, *mut i64, usize) -> St)(info, dims.as_mut_ptr(), n));
                let mut syms: Vec<*const c_char> = vec![std::ptr::null(); n];
                let _ = check(a, f!(a, 63, unsafe extern "C" fn(P, *mut *const c_char, usize) -> St)(info, syms.as_mut_ptr(), n));
                let shape = dims.iter().zip(&syms).map(|(&d, &s)| (if d >= 0 { Some(d) } else { None }, cstr(s))).collect();
                let w = if kind == 1 { "tensor" } else { "sparse_tensor" };
                (format!("{w}({})", elem_name(e)), Some(shape))
            }
            2 => {
                let mut si: P = std::ptr::null_mut();
                let _ = check(a, f!(a, 104, unsafe extern "C" fn(P, *mut P) -> St)(ti, &mut si));
                let mut el: P = std::ptr::null_mut();
                let _ = check(a, f!(a, 107, unsafe extern "C" fn(P, *mut P) -> St)(si, &mut el));
                let inner = if el.is_null() { "unknown".to_string() } else { type_of(a, el).0 };
                if !el.is_null() {
                    f!(a, 98, unsafe extern "C" fn(P))(el);
                }
                (format!("seq({inner})"), Some(vec![]))
            }
            3 => {
                let mut mi: P = std::ptr::null_mut();
                let _ = check(a, f!(a, 103, unsafe extern "C" fn(P, *mut P) -> St)(ti, &mut mi));
                let mut k: i32 = 0;
                let _ = check(a, f!(a, 105, unsafe extern "C" fn(P, *mut i32) -> St)(mi, &mut k));
                let mut vt: P = std::ptr::null_mut();
                let _ = check(a, f!(a, 106, unsafe extern "C" fn(P, *mut P) -> St)(mi, &mut vt));
                let inner = if vt.is_null() { "unknown".to_string() } else { type_of(a, vt).0 };
                if !vt.is_null() {
                    f!(a, 98, unsafe extern "C" fn(P))(vt);
                }
                (format!("map({},{inner})", elem_name(k)), Some(vec![]))
            }
            4 => ("opaque".into(), Some(vec![])),
            6 => ("optional".into(), Some(vec![])),
            _ => ("unknown".into(), None),
        }
    }

    // inputs (30, 33, 36), outputs (31, 34, 37) or overridable initializers (32, 35, 38)
    unsafe fn args(a: &Api, s: P, count: usize, info: usize, name: usize) -> Vec<OrtArg> {
        let mut n: usize = 0;
        if check(a, f!(a, count, unsafe extern "C" fn(P, *mut usize) -> St)(s, &mut n)).is_err() {
            return vec![];
        }
        let mut out = vec![];
        for i in 0..n {
            let mut nm: *mut c_char = std::ptr::null_mut();
            let _ = check(a, f!(a, name, unsafe extern "C" fn(P, usize, P, *mut *mut c_char) -> St)(s, i, a.alloc, &mut nm));
            let mut ti: P = std::ptr::null_mut();
            let _ = check(a, f!(a, info, unsafe extern "C" fn(P, usize, *mut P) -> St)(s, i, &mut ti));
            let (ty, shape) = if ti.is_null() { ("unknown".to_string(), None) } else { type_of(a, ti) };
            if !ti.is_null() {
                f!(a, 98, unsafe extern "C" fn(P))(ti);
            }
            out.push(OrtArg { name: take_str(a, nm), ty, shape });
        }
        out
    }

    unsafe fn meta(a: &Api, s: P) -> OrtMeta {
        let mut m: P = std::ptr::null_mut();
        let mut out = OrtMeta { producer: String::new(), graph: String::new(), domain: String::new(), description: String::new(), graph_description: String::new(), version: 0, custom: vec![] };
        if check(a, f!(a, 111, unsafe extern "C" fn(P, *mut P) -> St)(s, &mut m)).is_err() {
            return out;
        }
        let get = |i: usize| -> String {
            let mut p: *mut c_char = std::ptr::null_mut();
            let _ = check(a, f!(a, i, unsafe extern "C" fn(P, P, *mut *mut c_char) -> St)(m, a.alloc, &mut p));
            take_str(a, p)
        };
        out.producer = get(112);
        out.graph = get(113);
        out.domain = get(114);
        out.description = get(115);
        out.graph_description = get(158);
        let _ = check(a, f!(a, 117, unsafe extern "C" fn(P, *mut i64) -> St)(m, &mut out.version));
        let mut keys: *mut *mut c_char = std::ptr::null_mut();
        let mut nk: i64 = 0;
        if check(a, f!(a, 123, unsafe extern "C" fn(P, P, *mut *mut *mut c_char, *mut i64) -> St)(m, a.alloc, &mut keys, &mut nk)).is_ok() && !keys.is_null() {
            for i in 0..nk as usize {
                let kp = *keys.add(i);
                let mut vp: *mut c_char = std::ptr::null_mut();
                let _ = check(a, f!(a, 116, unsafe extern "C" fn(P, P, *const c_char, *mut *mut c_char) -> St)(m, a.alloc, kp, &mut vp));
                let k = take_str(a, kp);
                out.custom.push((k, take_str(a, vp)));
            }
            f!(a, 76, unsafe extern "C" fn(P, P) -> St)(a.alloc, keys as P);
        }
        f!(a, 118, unsafe extern "C" fn(P))(m);
        out
    }

    // ---- the protobuf of an ONNX model, read only to find tensors whose data is in files ----
    fn pb_varint(b: &[u8], i: &mut usize) -> Option<u64> {
        let mut v = 0u64;
        let mut sh = 0;
        loop {
            let c = *b.get(*i)?;
            *i += 1;
            v |= ((c & 0x7f) as u64) << sh;
            if c & 0x80 == 0 {
                return Some(v);
            }
            sh += 7;
            if sh > 63 {
                return None;
            }
        }
    }
    // the fields of a message: (number, wire type, varint value, bytes of a length-delimited field)
    fn pb_fields(b: &[u8]) -> Option<Vec<(u64, u64, u64, &[u8])>> {
        let mut i = 0;
        let mut out = vec![];
        while i < b.len() {
            let key = pb_varint(b, &mut i)?;
            let (f, w) = (key >> 3, key & 7);
            match w {
                0 => out.push((f, w, pb_varint(b, &mut i)?, &b[0..0])),
                1 => i += 8,
                5 => i += 4,
                2 => {
                    let n = pb_varint(b, &mut i)? as usize;
                    let s = b.get(i..i.checked_add(n)?)?;
                    i += n;
                    out.push((f, w, 0, s));
                }
                _ => return None,
            }
        }
        Some(out)
    }
    // kind 0 model, 1 graph, 2 node, 3 attribute, 4 tensor, 5 sparse tensor, 6 function. Bytes this cannot read
    // count as external data, but for a model in ORT format (a flatbuffer, which has none).
    fn pb_external(b: &[u8], kind: u8) -> bool {
        pb_ext(b, kind, 0)
    }
    fn pb_ext(b: &[u8], kind: u8, depth: u32) -> bool {
        if depth > 64 {
            return true;
        }
        let Some(fs) = pb_fields(b) else { return !(kind == 0 && b.get(4..8) == Some(b"ORTM")) };
        for (f, w, v, s) in fs {
            let sub = |k: u8| w == 2 && pb_ext(s, k, depth + 1);
            let hit = match (kind, f) {
                (0, 7) => sub(1),
                (0, 25) => sub(6),
                (1, 1) => sub(2),
                (1, 5) => sub(4),
                (1, 15) => sub(5),
                (2, 5) => sub(3),
                (3, 5) | (3, 10) => sub(4),
                (3, 6) | (3, 11) => sub(1),
                (3, 22) | (3, 23) => sub(5),
                (4, 13) => true,
                (4, 14) => w == 0 && v == 1,
                (5, 1) | (5, 2) => sub(4),
                (6, 7) => sub(2),
                _ => false,
            };
            if hit {
                return true;
            }
        }
        false
    }

    fn refuse(cap: &str, what: &str) -> (i64, String) {
        (-1, format!("brevi: --sandbox: {what} needs allow {cap}"))
    }

    fn session(src: Result<&str, &[u8]>, o: &OrtOpts) -> Result<OrtSess, (i64, String)> {
        let a = api().map_err(|m| (-2, m))?;
        // held to folders (--allow-read / --allow-write DIR)
        let scoped = FS_READ_IN.is_some() || FS_WRITE_IN.is_some();
        let held = |p: &str, write: bool| brevi_fs_path(p, write).map_err(|m| (-1, m));
        if FS_SANDBOX {
            if let Ok(p) = src {
                if !FS_READ {
                    return Err(refuse("read", &format!("the model file '{p}'")));
                }
                held(p, false)?;
                // ONNX Runtime reads external data files itself, next to the model: not held to the folders
                if FS_READ_IN.is_some() && std::fs::read(brevi_fs_path(p, false).unwrap_or_default()).is_ok_and(|b| pb_external(&b, 0)) {
                    return Err(refuse("read of every file (--allow read)", "a model whose weights are in external data files"));
                }
            }
            if !FS_WRITE && !o.optimized_path.is_empty() {
                return Err(refuse("write", "optimized_model_filepath"));
            }
            if !o.optimized_path.is_empty() {
                held(&o.optimized_path, true)?;
                brevi_new_file(&o.optimized_path);
            }
            if !FS_WRITE && o.profiling {
                return Err(refuse("write", "enable_profiling"));
            }
            if o.profiling && FS_WRITE_IN.is_some() {
                return Err(refuse("write of every file (--allow write)", "enable_profiling"));
            }
            for (k, _) in &o.config {
                let lk = k.to_lowercase();
                if (!FS_WRITE || !FS_READ || scoped) && ["file", "path", "folder", "dir"].iter().any(|w| lk.contains(w)) {
                    return Err(refuse("read and write of every file", &format!("the session setting '{k}'")));
                }
            }
            // a model in bytes still reads its external data from files
            if let Err(b) = src {
                if !FS_READ && pb_external(b, 0) {
                    return Err(refuse("read", "a model whose weights are in external data files"));
                }
            }
        }
        unsafe {
            let mut so: P = std::ptr::null_mut();
            check(a, f!(a, 10, unsafe extern "C" fn(*mut P) -> St)(&mut so))?;
            let done = |so: P| f!(a, 100, unsafe extern "C" fn(P))(so);
            let r = (|| -> Result<P, (i64, String)> {
                check(a, f!(a, 23, unsafe extern "C" fn(P, i32) -> St)(so, o.graph_opt as i32))?;
                check(a, f!(a, 24, unsafe extern "C" fn(P, i32) -> St)(so, o.intra as i32))?;
                check(a, f!(a, 25, unsafe extern "C" fn(P, i32) -> St)(so, o.inter as i32))?;
                check(a, f!(a, 13, unsafe extern "C" fn(P, i32) -> St)(so, o.exec_mode as i32))?;
                if o.log_severity >= 0 {
                    check(a, f!(a, 22, unsafe extern "C" fn(P, i32) -> St)(so, o.log_severity as i32))?;
                }
                check(a, f!(a, 21, unsafe extern "C" fn(P, i32) -> St)(so, o.log_verbosity as i32))?;
                if !o.logid.is_empty() {
                    let c = CString::new(o.logid.clone()).unwrap_or_default();
                    check(a, f!(a, 20, unsafe extern "C" fn(P, *const c_char) -> St)(so, c.as_ptr()))?;
                }
                check(a, f!(a, if o.mem_pattern { 16 } else { 17 }, unsafe extern "C" fn(P) -> St)(so))?;
                check(a, f!(a, if o.cpu_arena { 18 } else { 19 }, unsafe extern "C" fn(P) -> St)(so))?;
                if !o.optimized_path.is_empty() {
                    let c = ortchars(&o.optimized_path).ok_or((2, "embedded null byte".to_string()))?;
                    check(a, f!(a, 11, unsafe extern "C" fn(P, *const c_char) -> St)(so, ortchars_ptr(&c)))?;
                }
                if o.profiling {
                    let c = ortchars(&o.profile_prefix).ok_or((2, "embedded null byte".to_string()))?;
                    check(a, f!(a, 14, unsafe extern "C" fn(P, *const c_char) -> St)(so, ortchars_ptr(&c)))?;
                }
                if o.deterministic {
                    // SetDeterministicCompute (ORT 1.17)
                    if a.minor >= 17 {
                        check(a, f!(a, 273, unsafe extern "C" fn(P, bool) -> St)(so, true))?;
                    }
                }
                for (k, v) in &o.config {
                    let (k, v) = (CString::new(k.clone()).unwrap_or_default(), CString::new(v.clone()).unwrap_or_default());
                    check(a, f!(a, 130, unsafe extern "C" fn(P, *const c_char, *const c_char) -> St)(so, k.as_ptr(), v.as_ptr()))?;
                }
                let mut s: P = std::ptr::null_mut();
                match src {
                    Ok(p) => {
                        let c = ortchars(&p).ok_or((2, "embedded null byte".to_string()))?;
                        check(a, f!(a, 7, unsafe extern "C" fn(P, *const c_char, P, *mut P) -> St)(a.env, ortchars_ptr(&c), so, &mut s))?;
                    }
                    Err(b) => {
                        check(a, f!(a, 8, unsafe extern "C" fn(P, *const c_void, usize, P, *mut P) -> St)(a.env, b.as_ptr() as *const c_void, b.len(), so, &mut s))?;
                    }
                }
                Ok(s)
            })();
            done(so);
            let s = r?;
            let inputs = args(a, s, 30, 33, 36);
            let outputs = args(a, s, 31, 34, 37);
            let overridable = args(a, s, 32, 35, 38);
            let meta = meta(a, s);
            let mut n = NEXT.lock().unwrap();
            let id = *n;
            *n += 1;
            SESS.lock().unwrap().push((id, s as usize));
            Ok(OrtSess { id, inputs, outputs, overridable, meta })
        }
    }

    fn sess(id: i64) -> Option<P> {
        SESS.lock().unwrap().iter().find(|(i, _)| *i == id).map(|(_, p)| *p as P)
    }

    fn elem_size(e: i32) -> usize {
        match e {
            2 | 3 | 9 => 1,
            4 | 5 | 10 | 16 => 2,
            1 | 6 | 12 => 4,
            7 | 11 | 13 | 14 => 8,
            15 => 16,
            17..=20 => 1,
            _ => 0,
        }
    }

    unsafe fn tensor_out(a: &Api, v: P) -> Result<OrtTensor, (i64, String)> {
        let mut info: P = std::ptr::null_mut();
        check(a, f!(a, 65, unsafe extern "C" fn(P, *mut P) -> St)(v, &mut info))?;
        let mut e: i32 = 0;
        let _ = check(a, f!(a, 60, unsafe extern "C" fn(P, *mut i32) -> St)(info, &mut e));
        let mut n: usize = 0;
        let _ = check(a, f!(a, 61, unsafe extern "C" fn(P, *mut usize) -> St)(info, &mut n));
        let mut dims = vec![0i64; n];
        let _ = check(a, f!(a, 62, unsafe extern "C" fn(P, *mut i64, usize) -> St)(info, dims.as_mut_ptr(), n));
        let mut count: usize = 0;
        let _ = check(a, f!(a, 64, unsafe extern "C" fn(P, *mut usize) -> St)(info, &mut count));
        f!(a, 99, unsafe extern "C" fn(P))(info);
        let mut t = OrtTensor { elem: e, shape: dims, data: vec![], strs: vec![] };
        if e == 8 {
            for i in 0..count {
                let mut len: usize = 0;
                check(a, f!(a, 127, unsafe extern "C" fn(P, usize, *mut usize) -> St)(v, i, &mut len))?;
                let mut buf = vec![0u8; len];
                check(a, f!(a, 128, unsafe extern "C" fn(P, usize, usize, *mut c_void) -> St)(v, len, i, buf.as_mut_ptr() as *mut c_void))?;
                t.strs.push(String::from_utf8_lossy(&buf).into_owned());
            }
        } else {
            let size = elem_size(e) * count;
            if size > 0 {
                let mut p: P = std::ptr::null_mut();
                check(a, f!(a, 51, unsafe extern "C" fn(P, *mut P) -> St)(v, &mut p))?;
                t.data = std::slice::from_raw_parts(p as *const u8, size).to_vec();
            }
        }
        Ok(t)
    }

    unsafe fn value_out(a: &Api, v: P) -> Result<OrtOut, (i64, String)> {
        let mut kind: i32 = 0;
        check(a, f!(a, 67, unsafe extern "C" fn(P, *mut i32) -> St)(v, &mut kind))?;
        let part = |i: i32| -> Result<P, (i64, String)> {
            let mut o: P = std::ptr::null_mut();
            check(a, f!(a, 80, unsafe extern "C" fn(P, i32, P, *mut P) -> St)(v, i, a.alloc, &mut o))?;
            Ok(o)
        };
        let release = |o: P| f!(a, 96, unsafe extern "C" fn(P))(o);
        match kind {
            1 => Ok(OrtOut::Tensor(tensor_out(a, v)?)),
            2 => {
                let mut n: usize = 0;
                check(a, f!(a, 81, unsafe extern "C" fn(P, *mut usize) -> St)(v, &mut n))?;
                let mut out = vec![];
                for i in 0..n {
                    let o = part(i as i32)?;
                    let r = value_out(a, o);
                    release(o);
                    out.push(r?);
                }
                Ok(OrtOut::Seq(out))
            }
            3 => {
                let k = part(0)?;
                let kt = tensor_out(a, k);
                release(k);
                let x = part(1)?;
                let xt = tensor_out(a, x);
                release(x);
                Ok(OrtOut::Map(kt?, xt?))
            }
            4 => Ok(OrtOut::Bad("opaque".into())),
            5 => Ok(OrtOut::Bad("sparse tensor".into())),
            6 => Ok(OrtOut::Bad("optional".into())),
            _ => Ok(OrtOut::Bad("unknown".into())),
        }
    }

    fn run(id: i64, names: &[String], ins: &[(String, OrtTensor)], ro: (i64, i64, &str, bool)) -> Result<Vec<OrtOut>, (i64, String)> {
        let a = api().map_err(|m| (-2, m))?;
        let Some(s) = sess(id) else { return Err((1, "the session is closed".into())) };
        unsafe {
            let mut run_opts: P = std::ptr::null_mut();
            check(a, f!(a, 39, unsafe extern "C" fn(*mut P) -> St)(&mut run_opts))?;
            let mut vals: Vec<P> = vec![];
            let free_all = |vals: &[P], run_opts: P| {
                for &v in vals {
                    if !v.is_null() {
                        f!(a, 96, unsafe extern "C" fn(P))(v);
                    }
                }
                f!(a, 97, unsafe extern "C" fn(P))(run_opts);
            };
            let r = (|| -> Result<Vec<OrtOut>, (i64, String)> {
                if ro.0 >= 0 {
                    check(a, f!(a, 41, unsafe extern "C" fn(P, i32) -> St)(run_opts, ro.0 as i32))?;
                }
                check(a, f!(a, 40, unsafe extern "C" fn(P, i32) -> St)(run_opts, ro.1 as i32))?;
                if !ro.2.is_empty() {
                    let c = CString::new(ro.2).unwrap_or_default();
                    check(a, f!(a, 42, unsafe extern "C" fn(P, *const c_char) -> St)(run_opts, c.as_ptr()))?;
                }
                if ro.3 {
                    check(a, f!(a, 46, unsafe extern "C" fn(P) -> St)(run_opts))?;
                }
                let in_names: Vec<CString> = ins.iter().map(|(n, _)| CString::new(n.clone()).unwrap_or_default()).collect();
                for (_, t) in ins {
                    let mut v: P = std::ptr::null_mut();
                    if t.elem == 8 {
                        check(a, f!(a, 48, unsafe extern "C" fn(P, *const i64, usize, i32, *mut P) -> St)(a.alloc, t.shape.as_ptr(), t.shape.len(), 8, &mut v))?;
                        vals.push(v);
                        let cs: Vec<CString> = t.strs.iter().map(|x| CString::new(x.clone()).unwrap_or_default()).collect();
                        let ps: Vec<*const c_char> = cs.iter().map(|c| c.as_ptr()).collect();
                        check(a, f!(a, 52, unsafe extern "C" fn(P, *const *const c_char, usize) -> St)(v, ps.as_ptr(), ps.len()))?;
                    } else {
                        let ptr = if t.data.is_empty() { std::ptr::NonNull::<u64>::dangling().as_ptr() as P } else { t.data.as_ptr() as P };
                        check(
                            a,
                            f!(a, 49, unsafe extern "C" fn(P, P, usize, *const i64, usize, i32, *mut P) -> St)(a.mem, ptr, t.data.len(), t.shape.as_ptr(), t.shape.len(), t.elem, &mut v),
                        )?;
                        vals.push(v);
                    }
                }
                let out_names: Vec<CString> = names.iter().map(|n| CString::new(n.clone()).unwrap_or_default()).collect();
                let inp: Vec<*const c_char> = in_names.iter().map(|c| c.as_ptr()).collect();
                let outp: Vec<*const c_char> = out_names.iter().map(|c| c.as_ptr()).collect();
                let mut outs: Vec<P> = vec![std::ptr::null_mut(); names.len()];
                check(
                    a,
                    f!(a, 9, unsafe extern "C" fn(P, P, *const *const c_char, *const P, usize, *const *const c_char, usize, *mut P) -> St)(
                        s,
                        run_opts,
                        inp.as_ptr(),
                        vals.as_ptr(),
                        vals.len(),
                        outp.as_ptr(),
                        outp.len(),
                        outs.as_mut_ptr(),
                    ),
                )?;
                let mut res = vec![];
                let mut err = None;
                for &o in &outs {
                    if err.is_none() {
                        match value_out(a, o) {
                            Ok(x) => res.push(x),
                            Err(e) => err = Some(e),
                        }
                    }
                    if !o.is_null() {
                        f!(a, 96, unsafe extern "C" fn(P))(o);
                    }
                }
                match err {
                    Some(e) => Err(e),
                    None => Ok(res),
                }
            })();
            free_all(&vals, run_opts);
            r
        }
    }

    fn free(id: i64) {
        let Ok(a) = api() else { return };
        let mut v = SESS.lock().unwrap();
        if let Some(k) = v.iter().position(|(i, _)| *i == id) {
            let (_, p) = v.remove(k);
            unsafe { f!(a, 95, unsafe extern "C" fn(P))(p as P) };
        }
    }

    pub fn ops() -> OrtOps {
        OrtOps { load, providers, build_info, session, run, free }
    }
}
