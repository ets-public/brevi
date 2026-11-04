// ------------------------------------------------------------------------------------------- loading CPython
// Shared by both bridges: pyrt.rs (typed builds) and dynpy/pyglue.rs (dynamic builds) are pasted after this.
// It finds libpython (AX_PYB_CONF, or a standalone build's own copy), loads it and starts the interpreter.
// Pasted into programs that use Python libraries Brevi does not build itself (see pybridge.rs). CPython is
// loaded into the program (dlopen of libpython) the first time a library is used; library values are `AxPy`
// handles, and every operation on one runs in CPython, so results are CPython's own. The program's other
// values stay native and are converted when they are passed to a library.
#[allow(non_snake_case, dead_code)]
mod ax_pyffi {
    use std::ffi::{c_char, c_int, c_long, c_void};
    pub type P = *mut c_void;
    extern "C" {
        pub fn dlopen(f: *const c_char, flag: c_int) -> P;
        pub fn dlsym(h: P, s: *const c_char) -> P;
        pub fn dlerror() -> *const c_char;
    }
    pub struct Api {
        pub IncRef: unsafe extern "C" fn(P),
        pub DecRef: unsafe extern "C" fn(P),
        pub Hash: unsafe extern "C" fn(P) -> isize,
        pub Import: unsafe extern "C" fn(*const c_char) -> P,
        pub GetAttr: unsafe extern "C" fn(P, *const c_char) -> P,
        pub SetAttr: unsafe extern "C" fn(P, *const c_char, P) -> c_int,
        pub GetItem: unsafe extern "C" fn(P, P) -> P,
        pub SetItem: unsafe extern "C" fn(P, P, P) -> c_int,
        pub DelItem: unsafe extern "C" fn(P, P) -> c_int,
        pub Call: unsafe extern "C" fn(P, P, P) -> P,
        pub Str: unsafe extern "C" fn(P) -> P,
        pub Repr: unsafe extern "C" fn(P) -> P,
        pub IsTrue: unsafe extern "C" fn(P) -> c_int,
        pub GetIter: unsafe extern "C" fn(P) -> P,
        pub IterNext: unsafe extern "C" fn(P) -> P,
        pub Format: unsafe extern "C" fn(P, P) -> P,
        pub LongFrom: unsafe extern "C" fn(i64) -> P,
        pub LongAs: unsafe extern "C" fn(P, *mut c_int) -> i64,
        pub FloatFrom: unsafe extern "C" fn(f64) -> P,
        pub FloatAs: unsafe extern "C" fn(P) -> f64,
        pub UniFrom: unsafe extern "C" fn(*const c_char, isize) -> P,
        pub UniAs: unsafe extern "C" fn(P, *mut isize) -> *const c_char,
        pub TupleNew: unsafe extern "C" fn(isize) -> P,
        pub TupleSet: unsafe extern "C" fn(P, isize, P) -> c_int,
        pub ListNew: unsafe extern "C" fn(isize) -> P,
        pub ListSet: unsafe extern "C" fn(P, isize, P) -> c_int,
        pub DictNew: unsafe extern "C" fn() -> P,
        pub DictSet: unsafe extern "C" fn(P, P, P) -> c_int,
        pub BoolFrom: unsafe extern "C" fn(c_long) -> P,
        pub SliceNew: unsafe extern "C" fn(P, P, P) -> P,
        pub ErrOccurred: unsafe extern "C" fn() -> P,
        pub ErrFetch: unsafe extern "C" fn(*mut P, *mut P, *mut P),
        pub ErrNormalize: unsafe extern "C" fn(*mut P, *mut P, *mut P),
        pub ErrClear: unsafe extern "C" fn(),
        pub Init: unsafe extern "C" fn(c_int),
        pub SaveThread: unsafe extern "C" fn() -> P,
        pub GilEnsure: unsafe extern "C" fn() -> c_int,
        pub GilRelease: unsafe extern "C" fn(c_int),
        pub RunSimple: unsafe extern "C" fn(*const c_char) -> c_int,
        pub CapsuleNew: unsafe extern "C" fn(P, *const c_char, Option<unsafe extern "C" fn(P)>) -> P,
        pub CapsuleGet: unsafe extern "C" fn(P, *const c_char) -> P,
        pub CFunctionNew: unsafe extern "C" fn(*mut MethodDef, P, P) -> P,
        pub TupleSize: unsafe extern "C" fn(P) -> isize,
        pub TupleGet: unsafe extern "C" fn(P, isize) -> P,
        pub ErrSetString: unsafe extern "C" fn(P, *const c_char),
        pub RuntimeError: P,
        pub None: P,
        // For the dynamic builds' bridge (dynpy/pyglue.rs).
        pub RichCompareBool: unsafe extern "C" fn(P, P, c_int) -> c_int,
        pub IsInstance: unsafe extern "C" fn(P, P) -> c_int,
        pub Type: unsafe extern "C" fn(P) -> P,
        pub Size: unsafe extern "C" fn(P) -> isize,
        pub LongFromStr: unsafe extern "C" fn(*const c_char, *mut *mut c_char, c_int) -> P,
        pub BytesFrom: unsafe extern "C" fn(*const c_char, isize) -> P,
        pub BytesAs: unsafe extern "C" fn(P, *mut *mut c_char, *mut isize) -> c_int,
        pub SetNew: unsafe extern "C" fn(P) -> P,
        pub SetAdd: unsafe extern "C" fn(P, P) -> c_int,
        pub FrozenSetNew: unsafe extern "C" fn(P) -> P,
        pub DictNext: unsafe extern "C" fn(P, *mut isize, *mut P, *mut P) -> c_int,
        pub DictSize: unsafe extern "C" fn(P) -> isize,
        pub ListSize: unsafe extern "C" fn(P) -> isize,
        pub ListGet: unsafe extern "C" fn(P, isize) -> P,
        pub ListAppend: unsafe extern "C" fn(P, P) -> c_int,
        pub NumberFloat: unsafe extern "C" fn(P) -> P,
        pub ErrSetObject: unsafe extern "C" fn(P, P),
        pub CFunctionSelf: unsafe extern "C" fn(P) -> P,
        pub CapsuleValid: unsafe extern "C" fn(P, *const c_char) -> c_int,
        pub TBool: P,
        pub TLong: P,
        pub TFloat: P,
        pub TUni: P,
        pub TBytes: P,
        pub TList: P,
        pub TTuple: P,
        pub TDict: P,
        pub TSet: P,
        pub TFrozenSet: P,
        pub TCFunction: P,
    }
    #[repr(C)]
    pub struct MethodDef {
        pub name: *const c_char,
        pub meth: unsafe extern "C" fn(P, P) -> P,
        pub flags: c_int,
        pub doc: *const c_char,
    }
    unsafe impl Sync for MethodDef {}
    unsafe impl Sync for Api {}
    unsafe impl Send for Api {}
}


// libpython's path, PYTHONHOME and sys.path, found when the program was built (AX_PYB_CONF); asked of
// python3 again if that library is gone.
fn ax_pyb_conf() -> Result<(String, String, Vec<String>), String> {
    let parse = |s: &str| -> Option<(String, String, Vec<String>)> {
        let mut it = s.split('\u{1}');
        let lib = it.next()?.to_string();
        let home = it.next()?.to_string();
        Some((lib, home, it.map(String::from).collect()))
    };
    // A standalone build (`brevi build --standalone`) carries its own CPython.
    if let Some(c) = ax_pyb_bundle()? {
        return Ok(c);
    }
    if let Some(c) = parse(AX_PYB_CONF).filter(|c| std::path::Path::new(&c.0).is_file()) {
        return Ok(c);
    }
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let out = std::process::Command::new(&exe).args(["-c", AX_PYB_QUERY]).output().map_err(|e| format!("cannot start {exe}: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    parse(&s).filter(|c| !c.0.is_empty()).ok_or_else(|| format!("{exe} has no shared library (libpython) to load"))
}

// The CPython a standalone build carries (appended by pybundle.py): unpacked once into a cache folder
// named by its hash, then loaded from there. None for a binary without one.
fn ax_pyb_bundle() -> Result<Option<(String, String, Vec<String>)>, String> {
    use std::io::{Read, Seek, SeekFrom};
    // /proc may be missing (a bare container): argv[0] then names the program.
    let exe = std::env::current_exe().ok().filter(|p| p.is_file()).or_else(|| std::env::args_os().next().map(std::path::PathBuf::from)).unwrap_or_default();
    let Ok(mut f) = std::fs::File::open(&exe) else { return Ok(None) };
    let size = f.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    if size < 32 {
        return Ok(None);
    }
    let mut t = [0u8; 32];
    f.seek(SeekFrom::End(-32)).and_then(|_| f.read_exact(&mut t)).map_err(|e| e.to_string())?;
    if &t[24..] != b"BRVPYB01" {
        return Ok(None);
    }
    let len = u64::from_le_bytes(t[..8].try_into().unwrap());
    let tag = String::from_utf8_lossy(&t[8..24]).to_string();
    let base = match std::env::var_os("BREVI_PY_CACHE") {
        Some(d) => std::path::PathBuf::from(d),
        None => match (std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME")) {
            (Some(x), _) if !x.is_empty() => std::path::PathBuf::from(x).join("brevi").join("py"),
            (_, Some(h)) if !h.is_empty() => std::path::PathBuf::from(h).join(".cache").join("brevi").join("py"),
            _ => std::env::temp_dir().join("brevi-py"),
        },
    };
    let root = base.join(&tag);
    if !root.join(".brevi-ok").is_file() {
        std::fs::create_dir_all(&base).map_err(|e| format!("cannot create {}: {e}", base.display()))?;
        let tmp = base.join(format!(".{tag}.{}", std::process::id()));
        std::fs::remove_dir_all(&tmp).ok();
        f.seek(SeekFrom::Start(size - 32 - len)).map_err(|e| e.to_string())?;
        let r = ax_pyb_unpack(&mut std::io::BufReader::with_capacity(1 << 20, f.take(len)), &tmp);
        if let Err(e) = r {
            std::fs::remove_dir_all(&tmp).ok();
            return Err(format!("cannot unpack the bundled Python into {}: {e}", base.display()));
        }
        std::fs::write(tmp.join(".brevi-ok"), "").map_err(|e| e.to_string())?;
        // Another run of the program may have unpacked it first.
        if std::fs::rename(&tmp, &root).is_err() {
            std::fs::remove_dir_all(&tmp).ok();
            if !root.join(".brevi-ok").is_file() {
                return Err(format!("cannot unpack the bundled Python into {}", root.display()));
            }
        }
    }
    let at = |p: &str| root.join(p).to_string_lossy().to_string();
    let conf = std::fs::read_to_string(root.join(".brevi-conf")).map_err(|e| e.to_string())?;
    let mut lines = conf.lines();
    let lib = at(lines.next().unwrap_or(""));
    let path: Vec<String> = lines.map(at).collect();
    // Shared libraries libpython and the extension modules need, loaded first so they are found by name;
    // repeated while some still wait on others.
    let mut pre: Vec<String> = std::fs::read_to_string(root.join(".brevi-preload")).unwrap_or_default().lines().map(at).collect();
    while !pre.is_empty() {
        let n = pre.len();
        pre.retain(|p| {
            let c = std::ffi::CString::new(p.as_str()).unwrap();
            unsafe { ax_pyffi::dlopen(c.as_ptr(), 2 | 0x100) }.is_null()
        });
        if pre.len() == n {
            break;
        }
    }
    std::env::set_var("PYTHONNOUSERSITE", "1");
    std::env::remove_var("PYTHONPATH");
    std::env::remove_var("PYTHONSTARTUP");
    Ok(Some((lib, root.to_string_lossy().to_string(), path)))
}

fn ax_pyb_unpack(r: &mut impl std::io::Read, dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "damaged bundle");
    let mut head = [0u8; 15];
    loop {
        match r.read_exact(&mut head) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        }
        let kind = head[0];
        let mode = u32::from_le_bytes(head[1..5].try_into().unwrap());
        let mtime = u64::from_le_bytes(head[5..13].try_into().unwrap());
        let plen = u16::from_le_bytes(head[13..15].try_into().unwrap()) as usize;
        let mut p = vec![0u8; plen + 8];
        r.read_exact(&mut p)?;
        let raw = u32::from_le_bytes(p[plen..plen + 4].try_into().unwrap()) as usize;
        let dlen = u32::from_le_bytes(p[plen + 4..].try_into().unwrap()) as usize;
        let name = String::from_utf8(p[..plen].to_vec()).map_err(|_| bad())?;
        if name.starts_with('/') || name.split('/').any(|c| c == "..") {
            return Err(bad());
        }
        let mut data = vec![0u8; dlen];
        r.read_exact(&mut data)?;
        let data = if kind == 0 { ax_inflate(&data, raw).ok_or_else(bad)? } else { data };
        let out = dir.join(&name);
        std::fs::create_dir_all(out.parent().unwrap())?;
        std::fs::write(&out, &data)?;
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
        // Source times kept, so Python's compiled-module cache (written on first import) stays valid.
        if mtime > 0 {
            if let Ok(fh) = std::fs::File::options().write(true).open(&out) {
                fh.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime)).ok();
            }
        }
    }
}

// Raw DEFLATE (RFC 1951) decoding, after zlib's puff.c.
struct AxInf<'a> {
    d: &'a [u8],
    p: usize,
    b: u64,
    n: u32,
}
struct AxHuff {
    count: [u16; 16],
    sym: Vec<u16>,
    // Codes of up to 9 bits by their next 9 input bits: symbol << 4 | length (0 = longer code).
    fast: Vec<u32>,
}
impl AxHuff {
    fn new(lens: &[u8]) -> AxHuff {
        let mut count = [0u16; 16];
        for &l in lens {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..15 {
            offs[i + 1] = offs[i] + count[i];
        }
        let mut sym = vec![0u16; lens.len()];
        for (s, &l) in lens.iter().enumerate() {
            if l != 0 {
                sym[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        let mut fast = vec![0u32; 512];
        let (mut code, mut idx) = (0u32, 0usize);
        for len in 1..16u32 {
            for _ in 0..count[len as usize] {
                if len <= 9 {
                    let rev = code.reverse_bits() >> (32 - len);
                    for k in 0..(1u32 << (9 - len)) {
                        fast[(rev | (k << len)) as usize] = (sym[idx] as u32) << 4 | len;
                    }
                }
                code += 1;
                idx += 1;
            }
            code <<= 1;
        }
        AxHuff { count, sym, fast }
    }
}
impl<'a> AxInf<'a> {
    #[inline]
    fn bits(&mut self, k: u32) -> Option<u32> {
        while self.n < k {
            let x = *self.d.get(self.p)?;
            self.p += 1;
            self.b |= (x as u64) << self.n;
            self.n += 8;
        }
        let v = (self.b & ((1u64 << k) - 1)) as u32;
        self.b >>= k;
        self.n -= k;
        Some(v)
    }
    #[inline]
    fn decode(&mut self, h: &AxHuff) -> Option<u16> {
        while self.n <= 56 && self.p < self.d.len() {
            self.b |= (self.d[self.p] as u64) << self.n;
            self.p += 1;
            self.n += 8;
        }
        let e = h.fast[(self.b & 511) as usize];
        if e != 0 && (e & 15) <= self.n {
            self.b >>= e & 15;
            self.n -= e & 15;
            return Some((e >> 4) as u16);
        }
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= self.bits(1)? as i32;
            let count = h.count[len] as i32;
            if code - count < first {
                return h.sym.get((index + code - first) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

fn ax_inflate(d: &[u8], size: usize) -> Option<Vec<u8>> {
    const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
    const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
    const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
    const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let mut s = AxInf { d, p: 0, b: 0, n: 0 };
    let mut out: Vec<u8> = Vec::with_capacity(size);
    loop {
        let last = s.bits(1)?;
        match s.bits(2)? {
            0 => {
                // Stored: drop to a byte boundary, giving back whole bytes already read.
                let k = s.n % 8;
                s.b >>= k;
                s.n -= k;
                s.p -= (s.n / 8) as usize;
                s.b = 0;
                s.n = 0;
                let len = u16::from_le_bytes([*d.get(s.p)?, *d.get(s.p + 1)?]) as usize;
                s.p += 4;
                out.extend_from_slice(d.get(s.p..s.p + len)?);
                s.p += len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [8u8; 288];
                    l[144..256].fill(9);
                    l[256..280].fill(7);
                    (AxHuff::new(&l), AxHuff::new(&[5u8; 30]))
                } else {
                    let nlen = s.bits(5)? as usize + 257;
                    let ndist = s.bits(5)? as usize + 1;
                    let ncode = s.bits(4)? as usize + 4;
                    let mut cl = [0u8; 19];
                    for &o in &ORDER[..ncode] {
                        cl[o] = s.bits(3)? as u8;
                    }
                    let ch = AxHuff::new(&cl);
                    let mut lens = vec![0u8; nlen + ndist];
                    let mut i = 0;
                    while i < nlen + ndist {
                        let sym = s.decode(&ch)?;
                        let (v, rep) = match sym {
                            0..=15 => (sym as u8, 1),
                            16 => (*lens.get(i.checked_sub(1)?)?, 3 + s.bits(2)? as usize),
                            17 => (0, 3 + s.bits(3)? as usize),
                            _ => (0, 11 + s.bits(7)? as usize),
                        };
                        for _ in 0..rep {
                            *lens.get_mut(i)? = v;
                            i += 1;
                        }
                    }
                    (AxHuff::new(&lens[..nlen]), AxHuff::new(&lens[nlen..]))
                };
                loop {
                    let sym = s.decode(&lit)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let i = sym - 257;
                        let len = *LBASE.get(i)? as usize + s.bits(*LEXT.get(i)? as u32)? as usize;
                        let j = s.decode(&dist)? as usize;
                        let back = *DBASE.get(j)? as usize + s.bits(*DEXT.get(j)? as u32)? as usize;
                        let from = out.len().checked_sub(back)?;
                        for k in 0..len {
                            let c = out[from + k];
                            out.push(c);
                        }
                    }
                }
            }
            _ => return None,
        }
        if last == 1 {
            break;
        }
    }
    (out.len() == size).then_some(out)
}

fn ax_pyb_load() -> Result<ax_pyffi::Api, String> {
    use std::ffi::CString;
    let (lib, home, path) = ax_pyb_conf()?;
    let c = CString::new(lib.clone()).unwrap();
    // RTLD_NOW | RTLD_GLOBAL: extension modules (numpy's) find the interpreter's symbols.
    let h = unsafe { ax_pyffi::dlopen(c.as_ptr(), 2 | 0x100) };
    if h.is_null() {
        return Err(format!("cannot load {lib}"));
    }
    let sym = |n: &str| -> Result<ax_pyffi::P, String> {
        let c = CString::new(n).unwrap();
        let p = unsafe { ax_pyffi::dlsym(h, c.as_ptr()) };
        if p.is_null() { Err(format!("{lib} has no {n}")) } else { Ok(p) }
    };
    macro_rules! f {
        ($n:expr) => {
            unsafe { std::mem::transmute(sym($n)?) }
        };
    }
    let api = ax_pyffi::Api {
        IncRef: f!("Py_IncRef"),
        DecRef: f!("Py_DecRef"),
        Import: f!("PyImport_ImportModule"),
        GetAttr: f!("PyObject_GetAttrString"),
        SetAttr: f!("PyObject_SetAttrString"),
        GetItem: f!("PyObject_GetItem"),
        SetItem: f!("PyObject_SetItem"),
        DelItem: f!("PyObject_DelItem"),
        Call: f!("PyObject_Call"),
        Str: f!("PyObject_Str"),
        Repr: f!("PyObject_Repr"),
        Hash: f!("PyObject_Hash"),
        IsTrue: f!("PyObject_IsTrue"),
        GetIter: f!("PyObject_GetIter"),
        IterNext: f!("PyIter_Next"),
        Format: f!("PyObject_Format"),
        LongFrom: f!("PyLong_FromLongLong"),
        LongAs: f!("PyLong_AsLongLongAndOverflow"),
        FloatFrom: f!("PyFloat_FromDouble"),
        FloatAs: f!("PyFloat_AsDouble"),
        UniFrom: f!("PyUnicode_FromStringAndSize"),
        UniAs: f!("PyUnicode_AsUTF8AndSize"),
        TupleNew: f!("PyTuple_New"),
        TupleSet: f!("PyTuple_SetItem"),
        ListNew: f!("PyList_New"),
        ListSet: f!("PyList_SetItem"),
        DictNew: f!("PyDict_New"),
        DictSet: f!("PyDict_SetItem"),
        BoolFrom: f!("PyBool_FromLong"),
        SliceNew: f!("PySlice_New"),
        ErrOccurred: f!("PyErr_Occurred"),
        ErrFetch: f!("PyErr_Fetch"),
        ErrNormalize: f!("PyErr_NormalizeException"),
        ErrClear: f!("PyErr_Clear"),
        Init: f!("Py_InitializeEx"),
        SaveThread: f!("PyEval_SaveThread"),
        GilEnsure: f!("PyGILState_Ensure"),
        GilRelease: f!("PyGILState_Release"),
        RunSimple: f!("PyRun_SimpleString"),
        CapsuleNew: f!("PyCapsule_New"),
        CapsuleGet: f!("PyCapsule_GetPointer"),
        CFunctionNew: f!("PyCFunction_NewEx"),
        TupleSize: f!("PyTuple_Size"),
        TupleGet: f!("PyTuple_GetItem"),
        ErrSetString: f!("PyErr_SetString"),
        RuntimeError: unsafe { *(sym("PyExc_RuntimeError")? as *const ax_pyffi::P) },
        None: sym("_Py_NoneStruct")?,
        RichCompareBool: f!("PyObject_RichCompareBool"),
        IsInstance: f!("PyObject_IsInstance"),
        Type: f!("PyObject_Type"),
        Size: f!("PyObject_Size"),
        LongFromStr: f!("PyLong_FromString"),
        BytesFrom: f!("PyBytes_FromStringAndSize"),
        BytesAs: f!("PyBytes_AsStringAndSize"),
        SetNew: f!("PySet_New"),
        SetAdd: f!("PySet_Add"),
        FrozenSetNew: f!("PyFrozenSet_New"),
        DictNext: f!("PyDict_Next"),
        DictSize: f!("PyDict_Size"),
        ListSize: f!("PyList_Size"),
        ListGet: f!("PyList_GetItem"),
        ListAppend: f!("PyList_Append"),
        NumberFloat: f!("PyNumber_Float"),
        ErrSetObject: f!("PyErr_SetObject"),
        CFunctionSelf: f!("PyCFunction_GetSelf"),
        CapsuleValid: f!("PyCapsule_IsValid"),
        TBool: sym("PyBool_Type")?,
        TLong: sym("PyLong_Type")?,
        TFloat: sym("PyFloat_Type")?,
        TUni: sym("PyUnicode_Type")?,
        TBytes: sym("PyBytes_Type")?,
        TList: sym("PyList_Type")?,
        TTuple: sym("PyTuple_Type")?,
        TDict: sym("PyDict_Type")?,
        TSet: sym("PySet_Type")?,
        TFrozenSet: sym("PyFrozenSet_Type")?,
        TCFunction: sym("PyCFunction_Type")?,
    };
    if !home.is_empty() {
        std::env::set_var("PYTHONHOME", &home);
    }
    // Python's output goes straight out, so it keeps its place among the program's own lines.
    std::env::set_var("PYTHONUNBUFFERED", "1");
    unsafe { (api.Init)(0) };
    // sys.argv as `python3 file.py args...` gives it (argparse reads it), sys.path as python3 has it.
    let lit = |s: &str| format!("{s:?}").replace("\\u{", "\\U000").replace('}', "");
    let mut argv: Vec<String> = vec![lit(AX_PYB_FILE)];
    argv.extend(std::env::args().skip(1).map(|a| lit(&a)));
    let mut code = String::from("import sys\n");
    code.push_str(&format!("sys.argv = [{}]\n", argv.join(", ")));
    if !path.is_empty() {
        let ps: Vec<String> = path.iter().map(|p| lit(p)).collect();
        code.push_str(&format!("sys.path[:] = [{}]\n", ps.join(", ")));
    }
    code.push_str(AX_PYB_HELPERS);
    let c = CString::new(code).unwrap();
    if unsafe { (api.RunSimple)(c.as_ptr()) } != 0 {
        return Err("Python did not start".into());
    }
    // Other threads may now take the interpreter lock (each operation takes it, see AxGil).
    unsafe { (api.SaveThread)() };
    Ok(api)
}

const AX_PYB_HELPERS: &str = r#"
import builtins as _b, operator as _o, types as _t
_m = _t.ModuleType('_brevi')
def _exc(e):
    return ','.join(c.__name__ for c in type(e).__mro__ if c is not object) + '\x01' + str(e)
def _unpack(v, n):
    xs = list(v) if not isinstance(v, (list, tuple)) else v
    if len(xs) != n:
        a, *_r = [None] * n
        raise ValueError(f"too many values to unpack (expected {n})" if len(xs) > n else f"not enough values to unpack (expected {n}, got {len(xs)})")
    return tuple(xs)
_m.exc = _exc
_m.unpack = _unpack
_m.isnot = lambda a, b: a is not b
_m.notin = lambda a, b: a not in b
_m.contains = lambda a, b: a in b
_m.floordiv = _o.floordiv
sys.modules['_brevi'] = _m
"#;
