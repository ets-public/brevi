// ---------- SQLite, for Brevi's sqlite3 (pasted into the program; see _brevi_sqlite in rt_db.rs) ----------
// The engine is the system's SQLite (libsqlite3, or $BREVI_SQLITE_LIB), loaded the first time the program
// imports sqlite3, as CPython's _sqlite3 module links it. Under --sandbox a file layer (a VFS in front of the
// system's) is SQLite's only way to files: without the read grant it opens no file, without the write grant
// it opens files read-only and deletes none; every other VFS (`?vfs=unix` in a URI) is unregistered. Extensions
// (native code) never load: the program never enables them (FS_SANDBOX / FS_READ / FS_WRITE: set by the compiler).
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod brevi_sqlglue {
    use super::*;
    use brevi_dyn::{DbV, SqlOps};
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::sync::OnceLock;

    type P = *mut c_void;

    #[cfg(unix)]
    extern "C" {
        fn dlopen(f: *const c_char, flag: c_int) -> P;
        fn dlsym(h: P, s: *const c_char) -> P;
    }
    #[cfg(windows)]
    unsafe extern "system" {
        fn LoadLibraryA(f: *const c_char) -> P;
        fn LoadLibraryExA(f: *const c_char, file: P, flags: u32) -> P;
        fn GetProcAddress(h: P, s: *const c_char) -> *mut c_void;
    }
    #[cfg(unix)]
    unsafe fn ax_dlopen(f: *const c_char, flag: c_int) -> P {
        dlopen(f, flag)
    }
    #[cfg(windows)]
    unsafe fn ax_dlopen(f: *const c_char, _flag: c_int) -> P {
        // winsqlite3.dll is Windows' own copy in System32: LOAD_LIBRARY_SEARCH_SYSTEM32 (0x800) keeps a
        // sqlite3.dll in the program's folder or CWD from being picked up instead. Any other name is an
        // explicit absolute path (Python's DLL, or BREVI_SQLITE_LIB).
        let name = CStr::from_ptr(f).to_string_lossy();
        if name.eq_ignore_ascii_case("winsqlite3.dll") {
            LoadLibraryExA(f, std::ptr::null_mut(), 0x800)
        } else {
            LoadLibraryA(f)
        }
    }
    #[cfg(unix)]
    unsafe fn ax_dlsym(h: P, s: *const c_char) -> P {
        dlsym(h, s)
    }
    #[cfg(windows)]
    unsafe fn ax_dlsym(h: P, s: *const c_char) -> P {
        GetProcAddress(h, s)
    }

    struct Lib {
        libversion: unsafe extern "C" fn() -> *const c_char,
        open_v2: unsafe extern "C" fn(*const c_char, *mut P, c_int, *const c_char) -> c_int,
        close_v2: unsafe extern "C" fn(P) -> c_int,
        errmsg: unsafe extern "C" fn(P) -> *const c_char,
        extended_errcode: unsafe extern "C" fn(P) -> c_int,
        errstr: unsafe extern "C" fn(c_int) -> *const c_char,
        prepare_v2: unsafe extern "C" fn(P, *const c_char, c_int, *mut P, *mut *const c_char) -> c_int,
        finalize: unsafe extern "C" fn(P) -> c_int,
        reset: unsafe extern "C" fn(P) -> c_int,
        clear_bindings: unsafe extern "C" fn(P) -> c_int,
        step: unsafe extern "C" fn(P) -> c_int,
        bind_null: unsafe extern "C" fn(P, c_int) -> c_int,
        bind_int64: unsafe extern "C" fn(P, c_int, i64) -> c_int,
        bind_double: unsafe extern "C" fn(P, c_int, f64) -> c_int,
        bind_text: unsafe extern "C" fn(P, c_int, *const c_char, c_int, isize) -> c_int,
        bind_blob: unsafe extern "C" fn(P, c_int, *const c_void, c_int, isize) -> c_int,
        bind_parameter_count: unsafe extern "C" fn(P) -> c_int,
        bind_parameter_name: unsafe extern "C" fn(P, c_int) -> *const c_char,
        column_count: unsafe extern "C" fn(P) -> c_int,
        column_name: unsafe extern "C" fn(P, c_int) -> *const c_char,
        column_decltype: unsafe extern "C" fn(P, c_int) -> *const c_char,
        column_type: unsafe extern "C" fn(P, c_int) -> c_int,
        column_int64: unsafe extern "C" fn(P, c_int) -> i64,
        column_double: unsafe extern "C" fn(P, c_int) -> f64,
        column_text: unsafe extern "C" fn(P, c_int) -> *const u8,
        column_blob: unsafe extern "C" fn(P, c_int) -> *const u8,
        column_bytes: unsafe extern "C" fn(P, c_int) -> c_int,
        data_count: unsafe extern "C" fn(P) -> c_int,
        changes: unsafe extern "C" fn(P) -> c_int,
        total_changes: unsafe extern "C" fn(P) -> c_int,
        last_insert_rowid: unsafe extern "C" fn(P) -> i64,
        get_autocommit: unsafe extern "C" fn(P) -> c_int,
        stmt_readonly: unsafe extern "C" fn(P) -> c_int,
        busy_timeout: unsafe extern "C" fn(P, c_int) -> c_int,
        interrupt: unsafe extern "C" fn(P),
        limit: unsafe extern "C" fn(P, c_int, c_int) -> c_int,
        complete: unsafe extern "C" fn(*const c_char) -> c_int,
        vfs_find: unsafe extern "C" fn(*const c_char) -> *mut Vfs,
        vfs_register: unsafe extern "C" fn(*mut Vfs, c_int) -> c_int,
        vfs_unregister: unsafe extern "C" fn(*mut Vfs) -> c_int,
    }
    unsafe impl Send for Lib {}
    unsafe impl Sync for Lib {}

    // sqlite3_vfs, version 1 to 3 (the fields SQLite reads depend on iVersion, copied from the system's VFS)
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Vfs {
        iVersion: c_int,
        szOsFile: c_int,
        mxPathname: c_int,
        pNext: *mut Vfs,
        zName: *const c_char,
        pAppData: P,
        xOpen: Option<unsafe extern "C" fn(*mut Vfs, *const c_char, P, c_int, *mut c_int) -> c_int>,
        xDelete: Option<unsafe extern "C" fn(*mut Vfs, *const c_char, c_int) -> c_int>,
        xAccess: Option<unsafe extern "C" fn(*mut Vfs, *const c_char, c_int, *mut c_int) -> c_int>,
        xFullPathname: P,
        xDlOpen: Option<unsafe extern "C" fn(*mut Vfs, *const c_char) -> P>,
        xDlError: P,
        xDlSym: P,
        xDlClose: P,
        xRandomness: P,
        xSleep: P,
        xCurrentTime: P,
        xGetLastError: P,
        xCurrentTimeInt64: P,
        xSetSystemCall: P,
        xGetSystemCall: P,
        xNextSystemCall: P,
    }

    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    static mut REAL: *mut Vfs = std::ptr::null_mut();

    const SQLITE_OK: c_int = 0;
    const SQLITE_CANTOPEN: c_int = 14;
    const SQLITE_IOERR_DELETE: c_int = 10 | (10 << 8);
    const OPEN_READONLY: c_int = 0x1;
    const OPEN_READWRITE: c_int = 0x2;
    const OPEN_CREATE: c_int = 0x4;
    const OPEN_DELETEONCLOSE: c_int = 0x8;
    const OPEN_MAIN_DB: c_int = 0x100;

    // The sandbox's file layer: a named file opens only with the read grant, and only read-only without the
    // write grant (a journal, which must be written, does not open). SQLite's own unnamed temporary files
    // (sorting, temp tables) are not the caller's files and open as usual.
    unsafe extern "C" fn sb_open(_v: *mut Vfs, name: *const c_char, f: P, flags: c_int, out: *mut c_int) -> c_int {
        let real = REAL;
        let mut flags = flags;
        if !name.is_null() && flags & OPEN_CREATE != 0 {
            brevi_new_file(&std::ffi::CStr::from_ptr(name).to_string_lossy());
        }
        if !name.is_null() && flags & OPEN_DELETEONCLOSE == 0 {
            if !FS_READ || !in_scope(name, false) {
                return SQLITE_CANTOPEN;
            }
            if (!FS_WRITE || !in_scope(name, true)) && flags & OPEN_READWRITE != 0 {
                if flags & OPEN_MAIN_DB == 0 {
                    return SQLITE_CANTOPEN;
                }
                flags = (flags & !(OPEN_READWRITE | OPEN_CREATE)) | OPEN_READONLY;
            }
        }
        ((*real).xOpen.unwrap())(real, name, f, flags, out)
    }
    // Under --allow-read / --allow-write DIR: the file SQLite names (a full path) is inside those folders.
    unsafe fn in_scope(name: *const c_char, write: bool) -> bool {
        let n = std::ffi::CStr::from_ptr(name).to_string_lossy();
        brevi_fs_path(&n, write).is_ok()
    }
    unsafe extern "C" fn sb_delete(_v: *mut Vfs, name: *const c_char, sync: c_int) -> c_int {
        if !FS_WRITE || (!name.is_null() && !in_scope(name, true)) {
            return SQLITE_IOERR_DELETE;
        }
        let real = REAL;
        ((*real).xDelete.unwrap())(real, name, sync)
    }
    unsafe extern "C" fn sb_access(_v: *mut Vfs, name: *const c_char, flags: c_int, out: *mut c_int) -> c_int {
        if !FS_READ || (!name.is_null() && !in_scope(name, false)) {
            *out = 0;
            return SQLITE_OK;
        }
        let real = REAL;
        ((*real).xAccess.unwrap())(real, name, flags, out)
    }
    unsafe extern "C" fn sb_dlopen(_v: *mut Vfs, _f: *const c_char) -> P {
        std::ptr::null_mut()
    }

    fn lib() -> Result<&'static Lib, String> {
        LIB.get_or_init(|| unsafe { load_lib() }).as_ref().map_err(|e| e.clone())
    }

    unsafe fn load_lib() -> Result<Lib, String> {
        let mut names: Vec<String> = vec![];
        if !FS_SANDBOX {
            if let Ok(p) = std::env::var("BREVI_SQLITE_LIB") {
                names.push(p);
            }
        }
        if cfg!(target_os = "macos") {
            names.push("/usr/lib/libsqlite3.dylib".into());
            names.push("libsqlite3.dylib".into());
        } else if cfg!(windows) {
            // Python's SQLite DLL (same build as CPython's _sqlite3) when the program was built with it,
            // then Windows' own winsqlite3.dll (System32). No bare sqlite3.dll: the loader search path
            // would let a DLL in the program's folder or CWD run.
            if let Some(p) = SQLITE_DLL {
                names.push(p.to_string());
            }
            names.push("winsqlite3.dll".into());
        } else {
            names.push("libsqlite3.so.0".into());
            names.push("libsqlite3.so".into());
        }
        let mut h: P = std::ptr::null_mut();
        for n in &names {
            let c = CString::new(n.as_str()).unwrap();
            h = ax_dlopen(c.as_ptr(), 2 /* RTLD_NOW */);
            if !h.is_null() {
                break;
            }
        }
        if h.is_null() {
            return Err(format!("brevi: sqlite3 needs the SQLite library ({}); install it or set BREVI_SQLITE_LIB", names.last().unwrap()));
        }
        macro_rules! sym {
            ($n:expr) => {{
                let c = CString::new($n).unwrap();
                let p = ax_dlsym(h, c.as_ptr());
                if p.is_null() {
                    return Err(format!("brevi: the SQLite library lacks {}", $n));
                }
                std::mem::transmute(p)
            }};
        }
        let l = Lib {
            libversion: sym!("sqlite3_libversion"),
            open_v2: sym!("sqlite3_open_v2"),
            close_v2: sym!("sqlite3_close_v2"),
            errmsg: sym!("sqlite3_errmsg"),
            extended_errcode: sym!("sqlite3_extended_errcode"),
            errstr: sym!("sqlite3_errstr"),
            prepare_v2: sym!("sqlite3_prepare_v2"),
            finalize: sym!("sqlite3_finalize"),
            reset: sym!("sqlite3_reset"),
            clear_bindings: sym!("sqlite3_clear_bindings"),
            step: sym!("sqlite3_step"),
            bind_null: sym!("sqlite3_bind_null"),
            bind_int64: sym!("sqlite3_bind_int64"),
            bind_double: sym!("sqlite3_bind_double"),
            bind_text: sym!("sqlite3_bind_text"),
            bind_blob: sym!("sqlite3_bind_blob"),
            bind_parameter_count: sym!("sqlite3_bind_parameter_count"),
            bind_parameter_name: sym!("sqlite3_bind_parameter_name"),
            column_count: sym!("sqlite3_column_count"),
            column_name: sym!("sqlite3_column_name"),
            column_decltype: sym!("sqlite3_column_decltype"),
            column_type: sym!("sqlite3_column_type"),
            column_int64: sym!("sqlite3_column_int64"),
            column_double: sym!("sqlite3_column_double"),
            column_text: sym!("sqlite3_column_text"),
            column_blob: sym!("sqlite3_column_blob"),
            column_bytes: sym!("sqlite3_column_bytes"),
            data_count: sym!("sqlite3_data_count"),
            changes: sym!("sqlite3_changes"),
            total_changes: sym!("sqlite3_total_changes"),
            last_insert_rowid: sym!("sqlite3_last_insert_rowid"),
            get_autocommit: sym!("sqlite3_get_autocommit"),
            stmt_readonly: sym!("sqlite3_stmt_readonly"),
            busy_timeout: sym!("sqlite3_busy_timeout"),
            interrupt: sym!("sqlite3_interrupt"),
            limit: sym!("sqlite3_limit"),
            complete: sym!("sqlite3_complete"),
            vfs_find: sym!("sqlite3_vfs_find"),
            vfs_register: sym!("sqlite3_vfs_register"),
            vfs_unregister: sym!("sqlite3_vfs_unregister"),
        };
        // also under --limit files=N, which the file layer counts
        if FS_SANDBOX && (!(FS_READ && FS_WRITE && FS_READ_IN.is_none() && FS_WRITE_IN.is_none()) || std::env::var_os("BREVI_LIMIT_FILES").is_some()) {
            let real = (l.vfs_find)(std::ptr::null());
            if real.is_null() {
                return Err("brevi: SQLite has no file layer".into());
            }
            REAL = real;
            let mut sb = *real;
            sb.pNext = std::ptr::null_mut();
            sb.zName = c"brevi-sandbox".as_ptr();
            sb.xOpen = Some(sb_open);
            sb.xDelete = Some(sb_delete);
            sb.xAccess = Some(sb_access);
            sb.xDlOpen = Some(sb_dlopen);
            let sb = Box::into_raw(Box::new(sb));
            if (l.vfs_register)(sb, 1) != SQLITE_OK {
                return Err("brevi: could not set up SQLite's sandbox".into());
            }
            // the only VFS left is the sandbox's
            let mut others = vec![];
            let mut v = (*sb).pNext;
            while !v.is_null() {
                others.push(v);
                v = (*v).pNext;
            }
            for v in others {
                (l.vfs_unregister)(v);
            }
        }
        Ok(l)
    }
    fn l() -> &'static Lib {
        lib().expect("SQLite loaded")
    }
    fn p(h: i64) -> P {
        h as usize as P
    }
    unsafe fn cstr(c: *const c_char) -> Option<String> {
        (!c.is_null()).then(|| CStr::from_ptr(c).to_string_lossy().into_owned())
    }

    fn load() -> Result<String, String> {
        let l = lib()?;
        Ok(unsafe { cstr((l.libversion)()) }.unwrap_or_default())
    }
    fn open(path: &str, flags: i64) -> (i64, i64) {
        let c = match CString::new(path) {
            Ok(c) => c,
            Err(_) => return (-1, 0),
        };
        let mut db: P = std::ptr::null_mut();
        let rc = unsafe { (l().open_v2)(c.as_ptr(), &mut db, flags as c_int, std::ptr::null()) };
        (rc as i64, db as usize as i64)
    }
    fn close(db: i64) -> i64 {
        unsafe { (l().close_v2)(p(db)) as i64 }
    }
    fn errmsg(db: i64) -> String {
        unsafe { cstr((l().errmsg)(p(db))) }.unwrap_or_default()
    }
    fn errcode(db: i64) -> i64 {
        unsafe { (l().extended_errcode)(p(db)) as i64 }
    }
    fn errstr(rc: i64) -> String {
        unsafe { cstr((l().errstr)(rc as c_int)) }.unwrap_or_default()
    }
    fn prepare(db: i64, sql: &[u8]) -> (i64, i64, i64) {
        let mut st: P = std::ptr::null_mut();
        let mut tail: *const c_char = std::ptr::null();
        let base = sql.as_ptr() as *const c_char;
        let rc = unsafe { (l().prepare_v2)(p(db), base, sql.len() as c_int, &mut st, &mut tail) };
        let used = if tail.is_null() { sql.len() as i64 } else { unsafe { tail.offset_from(base) as i64 } };
        (rc as i64, st as usize as i64, used)
    }
    fn finalize(st: i64) -> i64 {
        unsafe { (l().finalize)(p(st)) as i64 }
    }
    fn reset(st: i64) -> i64 {
        unsafe { (l().reset)(p(st)) as i64 }
    }
    fn clear(st: i64) -> i64 {
        unsafe { (l().clear_bindings)(p(st)) as i64 }
    }
    fn step(st: i64) -> i64 {
        unsafe { (l().step)(p(st)) as i64 }
    }
    fn bind(st: i64, k: i64, v: &DbV) -> i64 {
        let l = l();
        let (s, k) = (p(st), k as c_int);
        unsafe {
            (match v {
                DbV::Null => (l.bind_null)(s, k),
                DbV::Int(i) => (l.bind_int64)(s, k, *i),
                DbV::Float(f) => (l.bind_double)(s, k, *f),
                DbV::Text(t) => (l.bind_text)(s, k, t.as_ptr() as *const c_char, t.len() as c_int, -1),
                DbV::Blob(b) => (l.bind_blob)(s, k, b.as_ptr() as *const c_void, b.len() as c_int, -1),
            }) as i64
        }
    }
    fn nparams(st: i64) -> i64 {
        unsafe { (l().bind_parameter_count)(p(st)) as i64 }
    }
    fn param_name(st: i64, k: i64) -> Option<String> {
        unsafe { cstr((l().bind_parameter_name)(p(st), k as c_int)) }
    }
    fn ncols(st: i64) -> i64 {
        unsafe { (l().column_count)(p(st)) as i64 }
    }
    fn col_name(st: i64, k: i64) -> Vec<u8> {
        unsafe {
            let c = (l().column_name)(p(st), k as c_int);
            if c.is_null() {
                vec![]
            } else {
                CStr::from_ptr(c).to_bytes().to_vec()
            }
        }
    }
    fn col_decl(st: i64, k: i64) -> Option<String> {
        unsafe { cstr((l().column_decltype)(p(st), k as c_int)) }
    }
    fn col(st: i64, k: i64) -> DbV {
        let l = l();
        let (s, k) = (p(st), k as c_int);
        unsafe {
            match (l.column_type)(s, k) {
                1 => DbV::Int((l.column_int64)(s, k)),
                2 => DbV::Float((l.column_double)(s, k)),
                3 => {
                    let t = (l.column_text)(s, k);
                    let n = (l.column_bytes)(s, k) as usize;
                    DbV::Text(if t.is_null() { vec![] } else { std::slice::from_raw_parts(t, n).to_vec() })
                }
                4 => {
                    let b = (l.column_blob)(s, k);
                    let n = (l.column_bytes)(s, k) as usize;
                    DbV::Blob(if b.is_null() { vec![] } else { std::slice::from_raw_parts(b, n).to_vec() })
                }
                _ => DbV::Null,
            }
        }
    }
    fn db_int(db: i64, what: i64) -> i64 {
        let l = l();
        unsafe {
            match what {
                0 => (l.changes)(p(db)) as i64,
                1 => (l.total_changes)(p(db)) as i64,
                2 => (l.last_insert_rowid)(p(db)),
                _ => (l.get_autocommit)(p(db)) as i64,
            }
        }
    }
    fn stmt_int(st: i64, what: i64) -> i64 {
        let l = l();
        unsafe {
            match what {
                0 => (l.stmt_readonly)(p(st)) as i64,
                _ => (l.data_count)(p(st)) as i64,
            }
        }
    }
    fn busy_timeout(db: i64, ms: i64) -> i64 {
        unsafe { (l().busy_timeout)(p(db), ms as c_int) as i64 }
    }
    fn interrupt(db: i64) {
        unsafe { (l().interrupt)(p(db)) }
    }
    fn limit(db: i64, id: i64, v: i64) -> i64 {
        unsafe { (l().limit)(p(db), id as c_int, v as c_int) as i64 }
    }
    fn complete(sql: &[u8]) -> bool {
        let Ok(c) = CString::new(sql) else { return false };
        unsafe { (l().complete)(c.as_ptr()) != 0 }
    }

    pub fn ops() -> SqlOps {
        SqlOps {
            load,
            open,
            close,
            errmsg,
            errcode,
            errstr,
            prepare,
            finalize,
            reset,
            clear,
            step,
            bind,
            nparams,
            param_name,
            ncols,
            col_name,
            col_decl,
            col,
            db_int,
            stmt_int,
            busy_timeout,
            interrupt,
            limit,
            complete,
        }
    }
}
