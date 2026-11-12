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
    #[cfg(unix)]
    extern "C" {
        fn dlopen(f: *const c_char, flag: c_int) -> P;
        fn dlsym(h: P, s: *const c_char) -> P;
    }
    #[cfg(windows)]
    extern "system" {
        fn LoadLibraryExW(f: *const u16, h: P, flags: u32) -> P;
        fn GetProcAddress(h: P, s: *const c_char) -> Option<unsafe extern "system" fn() -> isize>;
    }
    // One way to load a shared library and look a symbol up, on Unix and on Windows.
    #[cfg(unix)]
    pub unsafe fn ax_open(f: *const c_char, flag: c_int) -> P {
        dlopen(f, flag)
    }
    #[cfg(windows)]
    pub unsafe fn ax_open(f: *const c_char, _flag: c_int) -> P {
        // Only an absolute path, and its dependencies only from its own folder or System32
        // (LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32): never the program's folder, the
        // working folder or PATH, where a planted vcruntime140.dll or python3.dll could sit.
        let s = std::ffi::CStr::from_ptr(f).to_string_lossy();
        if !std::path::Path::new(s.as_ref()).is_absolute() {
            return std::ptr::null_mut();
        }
        let w: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        LoadLibraryExW(w.as_ptr(), std::ptr::null_mut(), 0x100 | 0x800)
    }
    #[cfg(unix)]
    pub unsafe fn ax_sym(h: P, s: *const c_char) -> P {
        dlsym(h, s)
    }
    #[cfg(windows)]
    pub unsafe fn ax_sym(h: P, s: *const c_char) -> P {
        GetProcAddress(h, s).map_or(std::ptr::null_mut(), |f| f as *mut c_void as P)
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
        pub ComplexFrom: unsafe extern "C" fn(f64, f64) -> P,
        pub ComplexReal: unsafe extern "C" fn(P) -> f64,
        pub ComplexImag: unsafe extern "C" fn(P) -> f64,
        pub UniFrom: unsafe extern "C" fn(*const c_char, isize) -> P,
        pub Intern: unsafe extern "C" fn(*mut P),
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
        pub IsSubclass: unsafe extern "C" fn(P, P) -> c_int,
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
        pub TComplex: P,
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
fn ax_default_python() -> String {
    if cfg!(windows) { "python".into() } else { "python3".into() }
}

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
    let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| ax_default_python());
    let out = std::process::Command::new(&exe).args(["-c", AX_PYB_QUERY]).output().map_err(|e| format!("cannot start {exe}: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    parse(&s).filter(|c| !c.0.is_empty()).ok_or_else(|| format!("{exe} has no shared library (libpython) to load"))
}

// The CPython a standalone build carries (appended by pybundle.py): unpacked once into a cache folder
// named by its hash, then loaded from there. None for a binary without one.
fn ax_pyb_bundle() -> Result<Option<(String, String, Vec<String>)>, String> {
    // A standalone bundle is built on Linux only for now; a Windows/macOS binary has none to find.
    if !cfg!(unix) {
        return Ok(None);
    }
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
            unsafe { ax_pyffi::ax_open(c.as_ptr(), 2 | 0x100) }.is_null()
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
    #[cfg(unix)]
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
        #[cfg(unix)]
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
        #[cfg(not(unix))]
        let _ = mode;
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

pub type AxPybHold = Option<(Option<&'static [&'static str]>, Option<&'static [&'static str]>, bool, Option<&'static [&'static str]>)>;

// Under --sandbox Brevi checks every file its own code opens, but not what CPython's code (or a C extension)
// opens, so before CPython starts the kernel holds the whole process to the grants: Linux Landlock, which
// needs no privileges and cannot be undone. Reads stay open to CPython's own files (its home, sys.path, shared
// libraries) and the few system files the C library and TLS read; nothing else outside the granted folders can
// be opened, made, renamed or removed. Without Landlock (another OS, an older kernel) a caller who named
// folders is refused rather than given every folder; with no folders named, CPython keeps the full trust its
// grant always meant.
fn ax_pyb_hold(h: (Option<&[&str]>, Option<&[&str]>, bool, Option<&[&str]>), lib: &str, home: &str, path: &[String]) -> Result<(), String> {
    let (read, write, scoped, local) = h;
    let refuse = |why: String| -> Result<(), String> {
        if scoped {
            Err(format!("brevi: --sandbox: CPython is held to --allow-read / --allow-write folders by the kernel (Linux Landlock), and {why}; grant read and write to every folder, or run on Linux 5.13 or later"))
        } else {
            Ok(())
        }
    };
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (read, write, local, lib, home, path);
        return refuse("this system has no Landlock".into());
    }
    #[cfg(target_os = "linux")]
    {
        use std::ffi::{c_int, c_long, c_void};
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::io::AsRawFd;
        extern "C" {
            fn syscall(n: c_long, ...) -> c_long;
            fn prctl(op: c_int, ...) -> c_int;
        }
        // Landlock's syscall numbers are the same on every Linux architecture.
        const CREATE: c_long = 444;
        const ADD_RULE: c_long = 445;
        const RESTRICT: c_long = 446;
        let abi = unsafe { syscall(CREATE, std::ptr::null::<c_void>(), 0usize, 1u32) };
        if abi < 1 {
            return refuse("this kernel has no Landlock (it needs Linux 5.13 or later, with Landlock on)".into());
        }
        // Rights on files and folders: read file, read dir; write file, remove dir/file, make char/dir/reg/
        // sock/fifo/block/sym; refer (ABI 2: moving across folders); truncate (ABI 3). Execute is left alone.
        const RF: u64 = 1 << 2;
        const RD: u64 = 1 << 3;
        let mut wr: u64 = (1 << 1) | (1 << 4) | (1 << 5) | (0x7f << 6);
        if abi >= 2 {
            wr |= 1 << 13;
        }
        if abi >= 3 {
            wr |= 1 << 14;
        }
        let file_wr: u64 = (1 << 1) | if abi >= 3 { 1 << 14 } else { 0 };
        let handled = RF | RD | wr;
        let attr: u64 = handled;
        let fd = unsafe { syscall(CREATE, &attr as *const u64, 8usize, 0u32) };
        if fd < 0 {
            return refuse(format!("Landlock refused a ruleset ({})", std::io::Error::last_os_error()));
        }
        #[repr(C, packed)]
        struct Beneath {
            allowed: u64,
            parent: i32,
        }
        let add = |p: &str, rights: u64| -> Result<(), String> {
            let Ok(f) = std::fs::OpenOptions::new().read(true).custom_flags(0o10000000).open(p) else { return Ok(()) };
            let dir = f.metadata().map(|m| m.is_dir()).unwrap_or(false);
            let rights = if dir { rights } else { rights & (RF | file_wr) };
            if rights == 0 {
                return Ok(());
            }
            let b = Beneath { allowed: rights, parent: f.as_raw_fd() };
            if unsafe { syscall(ADD_RULE, fd as c_int, 1 as c_int, &b as *const Beneath, 0u32) } != 0 {
                return Err(format!("brevi: --sandbox: Landlock refused a rule for {p} ({})", std::io::Error::last_os_error()));
            }
            Ok(())
        };
        let all = |rights: u64| add("/", rights);
        match read {
            None => all(RF | RD)?,
            Some(rs) => {
                for r in rs {
                    add(r, RF | RD)?;
                }
                // CPython's own files, and what the C library, DNS and TLS read.
                let libdir = std::path::Path::new(lib).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                let exe = std::env::current_exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
                let own = [home, &libdir, &exe];
                let sys = ["/usr/lib", "/usr/lib64", "/lib", "/lib64", "/usr/local/lib", "/usr/share/zoneinfo", "/usr/share/ca-certificates", "/etc/ssl", "/etc/pki", "/etc/ca-certificates", "/etc/localtime", "/etc/ld.so.cache", "/etc/resolv.conf", "/etc/hosts", "/etc/nsswitch.conf", "/etc/gai.conf", "/etc/services", "/etc/protocols", "/dev/null", "/dev/zero", "/dev/urandom", "/dev/random", "/sys/devices/system/cpu", "/proc/self", "/proc/cpuinfo", "/proc/meminfo"];
                // sys.path minus the program's own folder and the working folder (and any folder above it):
                // those hold the caller's files, which only --allow-read grants.
                let canon = |p: &str| std::fs::canonicalize(p).ok();
                let cwd = std::env::current_dir().ok().and_then(|d| std::fs::canonicalize(d).ok());
                let mine = canon(AX_PYB_FILE).and_then(|f| f.parent().map(|d| d.to_path_buf()));
                let lib_path = path.iter().map(String::as_str).filter(|p| match canon(p) {
                    None => false,
                    Some(c) => Some(&c) != mine.as_ref() && !cwd.as_ref().is_some_and(|w| w.starts_with(&c)),
                });
                // The libraries granted to CPython that sit in the program's folder or the working folder
                // (`--allow-py mylib` for mylib.py or mylib/), or every one there under --allow native.
                let mut mods: Vec<String> = Vec::new();
                for d in mine.iter().chain(cwd.iter()) {
                    let Ok(rd) = std::fs::read_dir(d) else { continue };
                    for e in rd.flatten() {
                        let p = e.path();
                        let (name, ok) = match p.extension().and_then(|x| x.to_str()) {
                            Some("py") => (p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_string(), p.is_file()),
                            None if p.join("__init__.py").is_file() => (p.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string(), true),
                            _ => (String::new(), false),
                        };
                        if ok && local.map_or(true, |l| l.contains(&name.as_str())) && Some(&p) != canon(AX_PYB_FILE).as_ref() {
                            mods.push(p.to_string_lossy().to_string());
                        }
                    }
                }
                // With such a library there, CPython's import finder must list that folder: names only (of that
                // folder and those below it), never what the files hold.
                if !mods.is_empty() {
                    for d in mine.iter().chain(cwd.iter()) {
                        add(&d.to_string_lossy(), RD)?;
                    }
                }
                for p in own.iter().copied().chain(lib_path).chain(mods.iter().map(String::as_str)).chain(sys) {
                    if !p.is_empty() {
                        let real = std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string());
                        add(&real, RF | RD)?;
                    }
                }
            }
        }
        match write {
            None => all(wr)?,
            Some(ws) => {
                for w in ws {
                    add(w, wr)?;
                }
                add("/dev/null", file_wr)?;
            }
        }
        // Python would write __pycache__ files it may no longer make.
        std::env::set_var("PYTHONDONTWRITEBYTECODE", "1");
        extern "C" {
            fn close(fd: c_int) -> c_int;
        }
        let held = unsafe { prctl(38, 1u64, 0u64, 0u64, 0u64) } == 0 && unsafe { syscall(RESTRICT, fd as c_int, 0u32) } == 0;
        unsafe { close(fd as c_int) };
        if !held {
            return Err(format!("brevi: --sandbox: Landlock did not hold the program ({})", std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

// Every kernel hold under --sandbox (no network, the file grants), taken once, before CPython starts.
static AX_PYB_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn ax_pyb_holds(lib: &str, home: &str, path: &[String]) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if (AX_PYB_NET == 0 && AX_PYB_HOLD.is_none()) || AX_PYB_HELD.load(Ordering::SeqCst) {
        return Ok(());
    }
    ax_pyb_one_thread()?;
    if AX_PYB_NET != 0 {
        ax_pyb_nonet(AX_PYB_NET)?;
    }
    if let Some(h) = AX_PYB_HOLD {
        ax_pyb_hold(h, lib, home, path)?;
    }
    AX_PYB_HELD.store(true, Ordering::SeqCst);
    Ok(())
}

// Called first thing in main() by a dynamic build that may start CPython: the holds are taken while main is
// still the only thread (the dynamic runtime runs the program on a thread of its own), so they hold both.
// Without a CPython to find, nothing is held yet: loading CPython fails later anyway.
#[allow(dead_code)]
fn ax_pyb_early() {
    if AX_PYB_NET == 0 && AX_PYB_HOLD.is_none() {
        return;
    }
    if let Ok((lib, home, path)) = ax_pyb_conf() {
        if let Err(e) = ax_pyb_holds(&lib, &home, &path) {
            eprintln!("{{\"e\":\"runtime\",\"msg\":{e:?},\"fix\":\"change what the message names, or run without --sandbox\"}}");
            std::process::exit(1)
        }
    }
}

// The kernel holds (Landlock's restrict_self, and seccomp) are taken by the thread that starts CPython, so it
// must be the program's only thread: another thread, left unheld, shares the memory CPython's native code can
// write. Brevi's own runtime starts no thread before this point; a program that did is refused.
fn ax_pyb_one_thread() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let n = std::fs::read_dir("/proc/self/task").map(|d| d.count()).map_err(|e| format!("brevi: --sandbox: cannot count the program's threads before CPython starts ({e})"))?;
        if n != 1 {
            return Err(format!("brevi: --sandbox: CPython must start before the program starts threads ({n} are running), so the kernel can hold all of them"));
        }
    }
    Ok(())
}

// Under --sandbox without a network grant, CPython (and the C extensions and processes it starts) must reach no
// network: before CPython starts, a seccomp filter makes socket() and io_uring_setup() fail with EACCES (a
// PermissionError in Python) for the whole program and everything it starts, and it cannot be lifted.
// socketpair() stays (asyncio's and multiprocessing's own pipes reach nothing outside). A Unix socket by path
// (a Docker or database daemon) needs socket() too, so it is held as well. Calls of another architecture's
// table (x32, or 32-bit int 0x80 on x86-64) fail for every system call. With hosts named (--allow-net HOST)
// the kernel cannot hold CPython to them, so that is refused on every OS. On Linux a kernel that refuses the
// filter means the program is refused; other systems have no such filter and CPython keeps the network there.
fn ax_pyb_nonet(mode: u8) -> Result<(), String> {
    if mode == 2 {
        return Err("brevi: --sandbox: CPython (--allow-py or --allow native) cannot be held to the hosts named with --allow-net: the kernel can hold it to no network, or allow it every host; grant --allow net, or leave out --allow-net".into());
    }
    // Elsewhere (macOS, Windows) CPython keeps the network, as THREAT_MODEL.md says: --allow-py and --allow
    // native are full trust there.
    #[cfg(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        return Ok(());
    }
    #[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        use std::ffi::{c_int, c_long};
        extern "C" {
            fn syscall(n: c_long, ...) -> c_long;
            fn prctl(op: c_int, ...) -> c_int;
        }
        #[repr(C)]
        struct Insn {
            code: u16,
            jt: u8,
            jf: u8,
            k: u32,
        }
        #[repr(C)]
        struct Prog {
            len: u16,
            filter: *const Insn,
        }
        const LD_W_ABS: u16 = 0x20;
        const JEQ_K: u16 = 0x15;
        const JGE_K: u16 = 0x35;
        const RET_K: u16 = 0x06;
        const ALLOW: u32 = 0x7fff_0000;
        const EACCES: u32 = 0x0005_0000 | 13;
        // AUDIT_ARCH_*, and the numbers of socket, io_uring_setup and seccomp
        #[cfg(target_arch = "x86_64")]
        const T: (u32, u32, u32, c_long) = (0xc000_003e, 41, 425, 317);
        #[cfg(target_arch = "aarch64")]
        const T: (u32, u32, u32, c_long) = (0xc000_00b7, 198, 425, 277);
        let (arch, socket, uring, seccomp) = T;
        let i = |code: u16, jt: u8, jf: u8, k: u32| Insn { code, jt, jf, k };
        let f = [
            i(LD_W_ABS, 0, 0, 4),     // arch
            i(JEQ_K, 1, 0, arch),     // ours: go on
            i(RET_K, 0, 0, EACCES),   // another table: nothing
            i(LD_W_ABS, 0, 0, 0),     // nr
            i(JGE_K, 3, 0, 0x4000_0000), // x32 numbers: nothing
            i(JEQ_K, 2, 0, socket),
            i(JEQ_K, 1, 0, uring),
            i(RET_K, 0, 0, ALLOW),
            i(RET_K, 0, 0, EACCES),
        ];
        let prog = Prog { len: f.len() as u16, filter: f.as_ptr() };
        // PR_SET_NO_NEW_PRIVS, then seccomp(SECCOMP_SET_MODE_FILTER, SECCOMP_FILTER_FLAG_TSYNC): every thread.
        let held = unsafe { prctl(38, 1u64, 0u64, 0u64, 0u64) } == 0 && unsafe { syscall(seccomp, 1 as c_long, 1 as c_long, &prog as *const Prog) } == 0;
        if !held {
            return Err(format!("brevi: --sandbox: CPython without a network grant is held to no network by the kernel (seccomp), which refused ({}); grant --allow net to run it here", std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

fn ax_pyb_load() -> Result<ax_pyffi::Api, String> {
    use std::ffi::CString;
    let (lib, home, path) = ax_pyb_conf()?;
    let c = CString::new(lib.clone()).unwrap();
    // RTLD_NOW | RTLD_GLOBAL: extension modules (numpy's) find the interpreter's symbols.
    let h = unsafe { ax_pyffi::ax_open(c.as_ptr(), 2 | 0x100) };
    if h.is_null() {
        return Err(format!("cannot load {lib}"));
    }
    let sym = |n: &str| -> Result<ax_pyffi::P, String> {
        let c = CString::new(n).unwrap();
        let p = unsafe { ax_pyffi::ax_sym(h, c.as_ptr()) };
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
        ComplexFrom: f!("PyComplex_FromDoubles"),
        ComplexReal: f!("PyComplex_RealAsDouble"),
        ComplexImag: f!("PyComplex_ImagAsDouble"),
        UniFrom: f!("PyUnicode_FromStringAndSize"),
        Intern: f!("PyUnicode_InternInPlace"),
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
        IsSubclass: f!("PyObject_IsSubclass"),
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
        TComplex: sym("PyComplex_Type")?,
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
    // python3 under the C or POSIX locale reads and writes text as UTF-8 (PEP 538/540); an embedded Python
    // does not unless told, and decodes files as ASCII (#211).
    if std::env::var_os("PYTHONUTF8").is_none() {
        let ctype = ["LC_ALL", "LC_CTYPE", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())).unwrap_or_default();
        if matches!(ctype.as_str(), "" | "C" | "POSIX") {
            // Py_InitializeEx reads the legacy flag, not PYTHONUTF8.
            if let Ok(p) = sym("Py_UTF8Mode") {
                unsafe { *(p as *mut std::ffi::c_int) = 1 };
            }
        }
    }
    ax_pyb_holds(&lib, &home, &path)?;
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
