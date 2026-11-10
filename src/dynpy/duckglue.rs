// ---------- DuckDB, for Brevi's duckdb (pasted into the program; see _brevi_duckdb in rt_db.rs) ----------
// The engine is DuckDB's own: libduckdb, $BREVI_DUCKDB_LIB, or the one inside the duckdb Python package found
// when the program was built (DUCK_LIBS), whose C API it exports. That library refers to CPython's symbols, so
// CPython's library (DUCK_PRELOAD) is loaded first for them, never started: no Python runs. Under --sandbox
// DuckDB loads and installs no extensions, its configuration is locked, SQL reaches no file without both the
// read and write grants (enable_external_access), and a database file opens only with the read grant (read-only
// without the write grant). FS_SANDBOX / FS_READ / FS_WRITE are set by the compiler.
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod brevi_duckglue {
    use super::*;
    use brevi_dyn::{DuckCol, DuckOps, DuckV};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::OnceLock;

    type P = *mut c_void;

    // libduckdb is dlopen'd on Unix; on Windows the same library (libduckdb.dll, or the DuckDB library inside
    // the duckdb Python package) is LoadLibraryA'd, and GetLastError names what a failed load wanted.
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
        let m = std::io::Error::from_raw_os_error(c as i32).to_string();
        let m = m.split(" (os error").next().unwrap_or_default().to_string();
        format!("{m} (Win32 error {c})")
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Res {
        a: u64,
        b: u64,
        c: u64,
        d: P,
        e: P,
        internal: P,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Huge {
        lower: u64,
        upper: i64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Dec {
        width: u8,
        scale: u8,
        value: Huge,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Interval {
        months: i32,
        days: i32,
        micros: i64,
    }

    struct Lib {
        library_version: unsafe extern "C" fn() -> *const c_char,
        create_config: unsafe extern "C" fn(*mut P) -> i32,
        set_config: unsafe extern "C" fn(P, *const c_char, *const c_char) -> i32,
        destroy_config: unsafe extern "C" fn(*mut P),
        open_ext: unsafe extern "C" fn(*const c_char, *mut P, P, *mut *mut c_char) -> i32,
        close: unsafe extern "C" fn(*mut P),
        connect: unsafe extern "C" fn(P, *mut P) -> i32,
        disconnect: unsafe extern "C" fn(*mut P),
        interrupt: unsafe extern "C" fn(P),
        free: unsafe extern "C" fn(P),
        query: unsafe extern "C" fn(P, *const c_char, *mut Res) -> i32,
        extract_statements: unsafe extern "C" fn(P, *const c_char, *mut P) -> u64,
        extract_statements_error: unsafe extern "C" fn(P) -> *const c_char,
        prepare_extracted_statement: unsafe extern "C" fn(P, P, u64, *mut P) -> i32,
        destroy_extracted: unsafe extern "C" fn(*mut P),
        prepare_error: unsafe extern "C" fn(P) -> *const c_char,
        prepare: unsafe extern "C" fn(P, *const c_char, *mut P) -> i32,
        clear_bindings: unsafe extern "C" fn(P) -> i32,
        destroy_prepare: unsafe extern "C" fn(*mut P),
        nparams: unsafe extern "C" fn(P) -> u64,
        bind_value: unsafe extern "C" fn(P, u64, P) -> i32,
        bind_parameter_index: unsafe extern "C" fn(P, *mut u64, *const c_char) -> i32,
        parameter_name: unsafe extern "C" fn(P, u64) -> *const c_char,
        execute_prepared: unsafe extern "C" fn(P, *mut Res) -> i32,
        prepared_statement_type: unsafe extern "C" fn(P) -> i32,
        prepared_statement_column_count: unsafe extern "C" fn(P) -> u64,
        prepared_statement_column_name: unsafe extern "C" fn(P, u64) -> *const c_char,
        prepared_statement_column_logical_type: unsafe extern "C" fn(P, u64) -> P,
        result_error: unsafe extern "C" fn(*mut Res) -> *const c_char,
        destroy_result: unsafe extern "C" fn(*mut Res),
        column_count: unsafe extern "C" fn(*mut Res) -> u64,
        column_name: unsafe extern "C" fn(*mut Res, u64) -> *const c_char,
        column_logical_type: unsafe extern "C" fn(*mut Res, u64) -> P,
        rows_changed: unsafe extern "C" fn(*mut Res) -> u64,
        result_return_type: unsafe extern "C" fn(Res) -> i32,
        result_statement_type: unsafe extern "C" fn(Res) -> i32,
        fetch_chunk: unsafe extern "C" fn(Res) -> P,
        data_chunk_get_size: unsafe extern "C" fn(P) -> u64,
        data_chunk_get_vector: unsafe extern "C" fn(P, u64) -> P,
        destroy_data_chunk: unsafe extern "C" fn(*mut P),
        vector_get_data: unsafe extern "C" fn(P) -> P,
        vector_get_validity: unsafe extern "C" fn(P) -> *mut u64,
        list_vector_get_child: unsafe extern "C" fn(P) -> P,
        struct_vector_get_child: unsafe extern "C" fn(P, u64) -> P,
        array_vector_get_child: unsafe extern "C" fn(P) -> P,
        get_type_id: unsafe extern "C" fn(P) -> i32,
        destroy_logical_type: unsafe extern "C" fn(*mut P),
        decimal_width: unsafe extern "C" fn(P) -> u8,
        decimal_scale: unsafe extern "C" fn(P) -> u8,
        decimal_internal_type: unsafe extern "C" fn(P) -> i32,
        enum_internal_type: unsafe extern "C" fn(P) -> i32,
        enum_dictionary_size: unsafe extern "C" fn(P) -> u32,
        enum_dictionary_value: unsafe extern "C" fn(P, u64) -> *mut c_char,
        list_type_child_type: unsafe extern "C" fn(P) -> P,
        array_type_child_type: unsafe extern "C" fn(P) -> P,
        array_type_array_size: unsafe extern "C" fn(P) -> u64,
        map_type_key_type: unsafe extern "C" fn(P) -> P,
        map_type_value_type: unsafe extern "C" fn(P) -> P,
        struct_type_child_count: unsafe extern "C" fn(P) -> u64,
        struct_type_child_name: unsafe extern "C" fn(P, u64) -> *mut c_char,
        struct_type_child_type: unsafe extern "C" fn(P, u64) -> P,
        logical_type_get_alias: unsafe extern "C" fn(P) -> *mut c_char,
        create_null_value: unsafe extern "C" fn() -> P,
        create_bool: unsafe extern "C" fn(bool) -> P,
        create_int64: unsafe extern "C" fn(i64) -> P,
        create_hugeint: unsafe extern "C" fn(Huge) -> P,
        create_uhugeint: unsafe extern "C" fn(Huge) -> P,
        create_double: unsafe extern "C" fn(f64) -> P,
        create_varchar_length: unsafe extern "C" fn(*const c_char, u64) -> P,
        create_blob: unsafe extern "C" fn(*const u8, u64) -> P,
        create_date: unsafe extern "C" fn(i32) -> P,
        create_time: unsafe extern "C" fn(i64) -> P,
        create_timestamp: unsafe extern "C" fn(i64) -> P,
        create_interval: unsafe extern "C" fn(Interval) -> P,
        create_decimal: unsafe extern "C" fn(Dec) -> P,
        create_list_value: unsafe extern "C" fn(P, *mut P, u64) -> P,
        create_struct_value: unsafe extern "C" fn(P, *mut P) -> P,
        create_struct_type: unsafe extern "C" fn(*mut P, *const *const c_char, u64) -> P,
        create_logical_type: unsafe extern "C" fn(i32) -> P,
        get_value_type: unsafe extern "C" fn(P) -> P,
        destroy_value: unsafe extern "C" fn(*mut P),
    }
    unsafe impl Send for Lib {}
    unsafe impl Sync for Lib {}

    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();

    struct Out {
        res: Res,
        // the chunk being read and the row in it
        chunk: P,
        row: u64,
        done: bool,
    }
    thread_local! {
        static RESULTS: RefCell<HashMap<i64, Box<Out>>> = RefCell::new(HashMap::new());
        static NEXT: std::cell::Cell<i64> = std::cell::Cell::new(1);
    }

    fn lib() -> Result<&'static Lib, String> {
        LIB.get_or_init(|| unsafe { load_lib() }).as_ref().map_err(|e| e.clone())
    }

    unsafe fn load_lib() -> Result<Lib, String> {
        let mut names: Vec<(String, &str)> = vec![];
        if !FS_SANDBOX {
            if let Ok(p) = std::env::var("BREVI_DUCKDB_LIB") {
                names.push((p, ""));
            }
        }
        #[cfg(unix)]
        names.push((if cfg!(target_os = "macos") { "libduckdb.dylib" } else { "libduckdb.so" }.to_string(), ""));
        // Windows has no bare-name fallback: a duckdb.dll in the program's folder or CWD must not run.
        // DUCK_LIBS holds the absolute path from the duckdb package found at build time; BREVI_DUCKDB_LIB
        // is the explicit override.
        for l in DUCK_LIBS {
            names.push((l.to_string(), DUCK_PRELOAD));
        }
        let mut h: P = std::ptr::null_mut();
        let mut why = String::new();
        for (n, pre) in &names {
            if !pre.is_empty() {
                let c = CString::new(*pre).unwrap();
                // RTLD_NOW | RTLD_GLOBAL: CPython's symbols, for the duckdb package's library
                ax_dlopen(c.as_ptr(), 2 | 0x100);
            }
            let c = CString::new(n.as_str()).unwrap();
            h = ax_dlopen(c.as_ptr(), 2);
            if !h.is_null() {
                break;
            }
            // a Windows load failure is worth telling even for the bare names: it names the missing DLL
            #[cfg(windows)]
            {
                why = ax_dlerror();
            }
            #[cfg(not(windows))]
            if !pre.is_empty() {
                why = ax_dlerror();
            }
        }
        if h.is_null() {
            return Err(format!("brevi: duckdb needs DuckDB's library (libduckdb, or the duckdb Python package: pip install duckdb){}", if why.is_empty() { String::new() } else { format!(": {why}") }));
        }
        macro_rules! sym {
            ($n:expr) => {{
                let c = CString::new(concat!("duckdb_", $n)).unwrap();
                let p = ax_dlsym(h, c.as_ptr());
                if p.is_null() {
                    return Err(format!("brevi: the DuckDB library lacks duckdb_{}", $n));
                }
                std::mem::transmute(p)
            }};
        }
        Ok(Lib {
            library_version: sym!("library_version"),
            create_config: sym!("create_config"),
            set_config: sym!("set_config"),
            destroy_config: sym!("destroy_config"),
            open_ext: sym!("open_ext"),
            close: sym!("close"),
            connect: sym!("connect"),
            disconnect: sym!("disconnect"),
            interrupt: sym!("interrupt"),
            free: sym!("free"),
            query: sym!("query"),
            extract_statements: sym!("extract_statements"),
            extract_statements_error: sym!("extract_statements_error"),
            prepare_extracted_statement: sym!("prepare_extracted_statement"),
            destroy_extracted: sym!("destroy_extracted"),
            prepare_error: sym!("prepare_error"),
            prepare: sym!("prepare"),
            clear_bindings: sym!("clear_bindings"),
            destroy_prepare: sym!("destroy_prepare"),
            nparams: sym!("nparams"),
            bind_value: sym!("bind_value"),
            bind_parameter_index: sym!("bind_parameter_index"),
            parameter_name: sym!("parameter_name"),
            execute_prepared: sym!("execute_prepared"),
            prepared_statement_type: sym!("prepared_statement_type"),
            prepared_statement_column_count: sym!("prepared_statement_column_count"),
            prepared_statement_column_name: sym!("prepared_statement_column_name"),
            prepared_statement_column_logical_type: sym!("prepared_statement_column_logical_type"),
            result_error: sym!("result_error"),
            destroy_result: sym!("destroy_result"),
            column_count: sym!("column_count"),
            column_name: sym!("column_name"),
            column_logical_type: sym!("column_logical_type"),
            rows_changed: sym!("rows_changed"),
            result_return_type: sym!("result_return_type"),
            result_statement_type: sym!("result_statement_type"),
            fetch_chunk: sym!("fetch_chunk"),
            data_chunk_get_size: sym!("data_chunk_get_size"),
            data_chunk_get_vector: sym!("data_chunk_get_vector"),
            destroy_data_chunk: sym!("destroy_data_chunk"),
            vector_get_data: sym!("vector_get_data"),
            vector_get_validity: sym!("vector_get_validity"),
            list_vector_get_child: sym!("list_vector_get_child"),
            struct_vector_get_child: sym!("struct_vector_get_child"),
            array_vector_get_child: sym!("array_vector_get_child"),
            get_type_id: sym!("get_type_id"),
            destroy_logical_type: sym!("destroy_logical_type"),
            decimal_width: sym!("decimal_width"),
            decimal_scale: sym!("decimal_scale"),
            decimal_internal_type: sym!("decimal_internal_type"),
            enum_internal_type: sym!("enum_internal_type"),
            enum_dictionary_size: sym!("enum_dictionary_size"),
            enum_dictionary_value: sym!("enum_dictionary_value"),
            list_type_child_type: sym!("list_type_child_type"),
            array_type_child_type: sym!("array_type_child_type"),
            array_type_array_size: sym!("array_type_array_size"),
            map_type_key_type: sym!("map_type_key_type"),
            map_type_value_type: sym!("map_type_value_type"),
            struct_type_child_count: sym!("struct_type_child_count"),
            struct_type_child_name: sym!("struct_type_child_name"),
            struct_type_child_type: sym!("struct_type_child_type"),
            logical_type_get_alias: sym!("logical_type_get_alias"),
            create_null_value: sym!("create_null_value"),
            create_bool: sym!("create_bool"),
            create_int64: sym!("create_int64"),
            create_hugeint: sym!("create_hugeint"),
            create_uhugeint: sym!("create_uhugeint"),
            create_double: sym!("create_double"),
            create_varchar_length: sym!("create_varchar_length"),
            create_blob: sym!("create_blob"),
            create_date: sym!("create_date"),
            create_time: sym!("create_time"),
            create_timestamp: sym!("create_timestamp"),
            create_interval: sym!("create_interval"),
            create_decimal: sym!("create_decimal"),
            create_list_value: sym!("create_list_value"),
            create_struct_value: sym!("create_struct_value"),
            create_struct_type: sym!("create_struct_type"),
            create_logical_type: sym!("create_logical_type"),
            get_value_type: sym!("get_value_type"),
            destroy_value: sym!("destroy_value"),
        })
    }

    fn l() -> &'static Lib {
        lib().expect("DuckDB loaded")
    }
    fn p(h: i64) -> P {
        h as usize as P
    }
    unsafe fn cs(c: *const c_char) -> String {
        if c.is_null() {
            String::new()
        } else {
            CStr::from_ptr(c).to_string_lossy().into_owned()
        }
    }
    unsafe fn cs_free(c: *mut c_char) -> String {
        let s = cs(c);
        if !c.is_null() {
            (l().free)(c as P);
        }
        s
    }

    fn load() -> Result<String, String> {
        let l = lib()?;
        let v = unsafe { cs((l.library_version)()) };
        Ok(v.trim_start_matches('v').to_string())
    }

    fn open(path: &str, cfg: &[(String, String)]) -> Result<(i64, i64), String> {
        let l = l();
        let memory = path.is_empty() || path == ":memory:" || path.starts_with(":memory:");
        if FS_SANDBOX && !memory && !FS_READ {
            return Err(format!("Permission Error: brevi: --sandbox: '{path}' needs allow read"));
        }
        // --allow-read / --allow-write DIR: the database file is inside them (opened read-only when only the
        // read grant holds it), and SQL reaches files only in folders granted both.
        // --limit files=N: DuckDB's SQL could create files uncounted, so it reaches none; a new database counts
        // --limit written: DuckDB writes its files itself, where Brevi cannot count the bytes, so it writes none:
        // a database file opens read-only, SQL reaches no file, and nothing spills to a temporary folder
        let written = brevi_written_held();
        let counted = std::env::var_os("BREVI_LIMIT_FILES").is_some() || written;
        if counted && !memory {
            brevi_new_file(path);
        }
        let scoped = FS_READ_IN.is_some() || FS_WRITE_IN.is_some() || counted;
        let ro_scope = FS_SANDBOX && !memory && FS_WRITE && brevi_fs_path(path, true).is_err();
        if FS_SANDBOX && !memory {
            brevi_fs_path(path, false).map_err(|m| format!("Permission Error: {m}"))?;
        }
        let sql_dirs: Vec<String> = match (FS_READ_IN, FS_WRITE_IN) {
            (None, None) => vec![],
            _ if counted => vec![],
            (r, w) => r.into_iter().flatten().chain(w.into_iter().flatten()).filter(|d| brevi_fs_path(d, false).is_ok() && brevi_fs_path(d, true).is_ok()).map(|d| d.to_string()).collect(),
        };
        unsafe {
            let mut c: P = std::ptr::null_mut();
            (l.create_config)(&mut c);
            let mut all: Vec<(String, String)> = cfg.to_vec();
            if FS_SANDBOX {
                if !memory && (!FS_WRITE || ro_scope || written) {
                    all.push(("access_mode".into(), "READ_ONLY".into()));
                }
                if written {
                    all.push(("temp_directory".into(), "".into()));
                }
                for (k, v) in [("autoinstall_known_extensions", "false"), ("autoload_known_extensions", "false"), ("allow_community_extensions", "false"), ("allow_unsigned_extensions", "false")] {
                    all.push((k.into(), v.into()));
                }
                if !(FS_READ && FS_WRITE) || written {
                    all.push(("enable_external_access".into(), "false".into()));
                }
                // held to folders: allowed_directories can only be set once the database runs (below)
                if !scoped {
                    all.push(("lock_configuration".into(), "true".into()));
                }
            }
            for (k, v) in &all {
                let (kc, vc) = (CString::new(k.as_str()).unwrap_or_default(), CString::new(v.as_str()).unwrap_or_default());
                if (l.set_config)(c, kc.as_ptr(), vc.as_ptr()) != 0 {
                    (l.destroy_config)(&mut c);
                    return Err(format!("Invalid Input Error: The following options were not recognized: {k}"));
                }
            }
            let pc = CString::new(path).map_err(|_| "Invalid Input Error: the path holds a NUL".to_string())?;
            let mut db: P = std::ptr::null_mut();
            let mut err: *mut c_char = std::ptr::null_mut();
            let rc = (l.open_ext)(if memory { std::ptr::null() } else { pc.as_ptr() }, &mut db, c, &mut err);
            (l.destroy_config)(&mut c);
            if rc != 0 {
                return Err(cs_free(err));
            }
            let mut con: P = std::ptr::null_mut();
            if (l.connect)(db, &mut con) != 0 {
                return Err("Connection Error: could not connect".into());
            }
            if FS_SANDBOX && scoped {
                let q = |d: &String, end: &str| format!("'{}{end}'", d.replace('\'', "''"));
                let dirs: Vec<String> = sql_dirs.iter().filter(|d| std::path::Path::new(d.as_str()).is_dir()).map(|d| q(d, "/")).collect();
                let files: Vec<String> = sql_dirs.iter().filter(|d| !std::path::Path::new(d.as_str()).is_dir()).map(|d| q(d, "")).collect();
                let mut sets = vec![];
                if !dirs.is_empty() {
                    sets.push(format!("SET allowed_directories = [{}]", dirs.join(", ")));
                }
                if !files.is_empty() {
                    sets.push(format!("SET allowed_paths = [{}]", files.join(", ")));
                }
                sets.push("SET enable_external_access = false".into());
                sets.push("SET lock_configuration = true".into());
                for q in sets {
                    let c = CString::new(q).unwrap_or_default();
                    let mut r = Res { a: 0, b: 0, c: 0, d: std::ptr::null_mut(), e: std::ptr::null_mut(), internal: std::ptr::null_mut() };
                    let bad = (l.query)(con, c.as_ptr(), &mut r) != 0;
                    let e = if bad { cs((l.result_error)(&mut r)) } else { String::new() };
                    (l.destroy_result)(&mut r);
                    if bad {
                        // never run unheld
                        let mut cc = con;
                        (l.disconnect)(&mut cc);
                        let mut dd = db;
                        (l.close)(&mut dd);
                        return Err(format!("brevi: --sandbox: could not hold DuckDB to the granted folders ({e})"));
                    }
                }
            }
            Ok((db as usize as i64, con as usize as i64))
        }
    }
    fn connect(db: i64) -> Result<i64, String> {
        let mut con: P = std::ptr::null_mut();
        if unsafe { (l().connect)(p(db), &mut con) } != 0 {
            return Err("Connection Error: could not connect".into());
        }
        Ok(con as usize as i64)
    }
    fn disconnect(con: i64) {
        let mut c = p(con);
        unsafe { (l().disconnect)(&mut c) }
    }
    fn close(db: i64) {
        let mut d = p(db);
        unsafe { (l().close)(&mut d) }
    }
    fn interrupt(con: i64) {
        unsafe { (l().interrupt)(p(con)) }
    }

    // ---- values to DuckDB ----
    unsafe fn value(v: &DuckV) -> Result<P, String> {
        let l = l();
        Ok(match v {
            DuckV::Null => (l.create_null_value)(),
            DuckV::Bool(b) => (l.create_bool)(*b),
            DuckV::Int(i) => (l.create_int64)(*i),
            DuckV::Big(s) => {
                if let Ok(x) = s.parse::<i128>() {
                    (l.create_hugeint)(Huge { lower: x as u64, upper: (x >> 64) as i64 })
                } else if let Ok(x) = s.parse::<u128>() {
                    (l.create_uhugeint)(Huge { lower: x as u64, upper: (x >> 64) as u64 as i64 })
                } else {
                    // as the duckdb package does past 128 bits: a DOUBLE
                    (l.create_double)(s.parse::<f64>().unwrap_or(f64::INFINITY))
                }
            }
            DuckV::Float(f) => (l.create_double)(*f),
            DuckV::Str(s) => (l.create_varchar_length)(s.as_ptr() as *const c_char, s.len() as u64),
            DuckV::Blob(b) => (l.create_blob)(b.as_ptr(), b.len() as u64),
            DuckV::Date(d) => (l.create_date)(*d as i32),
            DuckV::Time(t) => (l.create_time)(*t),
            DuckV::Ts(t) => (l.create_timestamp)(*t),
            DuckV::Interval(m, d, us) => (l.create_interval)(Interval { months: *m as i32, days: *d as i32, micros: *us }),
            DuckV::Dec(s) => {
                let neg = s.starts_with('-');
                let body = s.trim_start_matches(['-', '+']);
                let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
                let digits = format!("{ip}{fp}").trim_start_matches('0').to_string();
                let scale = fp.len();
                let width = digits.len().max(scale).max(1);
                match digits.parse::<i128>().ok().filter(|_| width <= 38 && !s.contains(['e', 'E', 'n', 'N', 'i', 'I'])) {
                    Some(x) => {
                        let x = if neg { -x } else { x };
                        (l.create_decimal)(Dec { width: width as u8, scale: scale as u8, value: Huge { lower: x as u64, upper: (x >> 64) as i64 } })
                    }
                    None => (l.create_double)(s.parse::<f64>().unwrap_or(f64::NAN)),
                }
            }
            DuckV::List(xs) | DuckV::Arr(xs) => {
                let mut vals: Vec<P> = vec![];
                for x in xs {
                    vals.push(value(x)?);
                }
                // the child type of the first value that is not NULL (INTEGER for an empty list, as the package)
                let child = xs.iter().position(|x| !matches!(x, DuckV::Null)).map(|k| (l.get_value_type)(vals[k]));
                let mut own = std::ptr::null_mut();
                let ty = match child {
                    Some(t) => t,
                    None => {
                        own = (l.create_logical_type)(4);
                        own
                    }
                };
                let out = (l.create_list_value)(ty, vals.as_mut_ptr(), vals.len() as u64);
                if !own.is_null() {
                    (l.destroy_logical_type)(&mut own);
                }
                for mut v in vals {
                    (l.destroy_value)(&mut v);
                }
                if out.is_null() {
                    return Err("Invalid Input Error: list values of different types".into());
                }
                out
            }
            DuckV::Struct(kv) => {
                let mut vals: Vec<P> = vec![];
                let mut types: Vec<P> = vec![];
                let names: Vec<CString> = kv.iter().map(|(k, _)| CString::new(k.as_str()).unwrap_or_default()).collect();
                for (_, x) in kv {
                    let v = value(x)?;
                    types.push((l.get_value_type)(v));
                    vals.push(v);
                }
                let np: Vec<*const c_char> = names.iter().map(|c| c.as_ptr()).collect();
                let mut st = (l.create_struct_type)(types.as_mut_ptr(), np.as_ptr(), kv.len() as u64);
                let out = (l.create_struct_value)(st, vals.as_mut_ptr());
                (l.destroy_logical_type)(&mut st);
                for mut v in vals {
                    (l.destroy_value)(&mut v);
                }
                out
            }
            _ => return Err("Invalid Input Error: brevi: cannot bind this value".into()),
        })
    }

    // As DuckDB checks values given to a prepared statement: by identifier ("1", "2", ... for positional ones).
    unsafe fn bind_all(st: P, pos: &[DuckV], named: &[(String, DuckV)], given: bool) -> Result<(), String> {
        let l = l();
        let n = (l.nparams)(st);
        let vals: Vec<(String, &DuckV)> = if named.is_empty() { pos.iter().enumerate().map(|(k, v)| ((k + 1).to_string(), v)).collect() } else { named.iter().map(|(k, v)| (k.to_lowercase(), v)).collect() };
        if n == 0 {
            if given && !vals.is_empty() {
                return Err(format!("Binder Error: Parameter/argument count mismatch for prepared statement. Expected 0, got {}", vals.len()));
            }
            return Ok(());
        }
        let names: Vec<String> = (1..=n).map(|k| cs_free((l.parameter_name)(st, k) as *mut c_char).to_lowercase()).collect();
        let mut excess: Vec<&String> = vals.iter().map(|(k, _)| k).filter(|k| !names.contains(k)).collect();
        if !excess.is_empty() {
            excess.sort_by(|a, b| (a.parse::<u64>().ok(), a.as_str()).cmp(&(b.parse::<u64>().ok(), b.as_str())));
            excess.dedup();
            return Err(format!("Invalid Input Error: Parameter argument/count mismatch, identifiers of the excess parameters: {}", excess.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        let mut missing: Vec<&String> = vec![];
        for nm in &names {
            if !vals.iter().any(|(k, _)| k == nm) && !missing.contains(&nm) {
                missing.push(nm);
            }
        }
        if !missing.is_empty() {
            return Err(format!("Invalid Input Error: Values were not provided for the following parameters: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        for (k, v) in &vals {
            let kc = CString::new(k.as_str()).unwrap_or_default();
            let mut idx = 0u64;
            if (l.bind_parameter_index)(st, &mut idx, kc.as_ptr()) != 0 {
                continue;
            }
            let mut val = value(v)?;
            let rc = (l.bind_value)(st, idx, val);
            (l.destroy_value)(&mut val);
            if rc != 0 {
                return Err(cs((l.prepare_error)(st)));
            }
        }
        Ok(())
    }

    // The statements of the SQL, each prepared (and bound) and run; the last one's result.
    fn query(con: i64, sql: &str, pos: &[DuckV], named: &[(String, DuckV)], given: bool) -> Result<i64, String> {
        let l = l();
        let c = CString::new(sql).map_err(|_| "Invalid Input Error: the query holds a NUL".to_string())?;
        unsafe {
            let mut ex: P = std::ptr::null_mut();
            let n = (l.extract_statements)(p(con), c.as_ptr(), &mut ex);
            (l.destroy_extracted)(&mut ex);
            if !given && n == 1 {
                // one statement and no values: it runs as written (a prepared statement names itself EXECUTE in
                // errors such as a read-only database's)
                let mut r = Res { a: 0, b: 0, c: 0, d: std::ptr::null_mut(), e: std::ptr::null_mut(), internal: std::ptr::null_mut() };
                if (l.query)(p(con), c.as_ptr(), &mut r) != 0 {
                    let e = cs((l.result_error)(&mut r));
                    (l.destroy_result)(&mut r);
                    return Err(e);
                }
                if (l.column_count)(&mut r) == 0 && (l.result_statement_type)(r) == 0 {
                    (l.destroy_result)(&mut r);
                    return Err("brevi: no statement".into());
                }
                let id = NEXT.with(|n| {
                    let v = n.get();
                    n.set(v + 1);
                    v
                });
                RESULTS.with(|m| m.borrow_mut().insert(id, Box::new(Out { res: r, chunk: std::ptr::null_mut(), row: 0, done: false })));
                return Ok(id);
            }
            let mut ex: P = std::ptr::null_mut();
            let n = (l.extract_statements)(p(con), c.as_ptr(), &mut ex);
            if n == 0 {
                let e = cs((l.extract_statements_error)(ex));
                (l.destroy_extracted)(&mut ex);
                // no statement at all: the duckdb package's execute answers None
                return Err(if e.is_empty() { "brevi: no statement".into() } else { e });
            }
            let mut last: Option<Res> = None;
            for k in 0..n {
                let mut st: P = std::ptr::null_mut();
                if (l.prepare_extracted_statement)(p(con), ex, k, &mut st) != 0 {
                    let e = cs((l.prepare_error)(st));
                    (l.destroy_prepare)(&mut st);
                    (l.destroy_extracted)(&mut ex);
                    if let Some(mut r) = last {
                        (l.destroy_result)(&mut r);
                    }
                    return Err(e);
                }
                let is_last = k + 1 == n;
                let early = !is_last && (l.nparams)(st) > 0 && given;
                if early || is_last || (l.nparams)(st) > 0 {
                    let r = if early { Err("Not implemented Error: Prepared parameters are only supported for the last statement, please split your query up into separate 'execute' calls if you want to use prepared parameters".to_string()) } else { bind_all(st, if is_last { pos } else { &[] }, if is_last { named } else { &[] }, is_last && given) };
                    if let Err(e) = r {
                        (l.destroy_prepare)(&mut st);
                        (l.destroy_extracted)(&mut ex);
                        if let Some(mut r) = last {
                            (l.destroy_result)(&mut r);
                        }
                        return Err(e);
                    }
                }
                let mut r = Res { a: 0, b: 0, c: 0, d: std::ptr::null_mut(), e: std::ptr::null_mut(), internal: std::ptr::null_mut() };
                let rc = (l.execute_prepared)(st, &mut r);
                (l.destroy_prepare)(&mut st);
                if let Some(mut old) = last.take() {
                    (l.destroy_result)(&mut old);
                }
                if rc != 0 {
                    let e = cs((l.result_error)(&mut r));
                    (l.destroy_result)(&mut r);
                    (l.destroy_extracted)(&mut ex);
                    return Err(e);
                }
                last = Some(r);
            }
            (l.destroy_extracted)(&mut ex);
            let id = NEXT.with(|n| {
                let v = n.get();
                n.set(v + 1);
                v
            });
            RESULTS.with(|m| m.borrow_mut().insert(id, Box::new(Out { res: last.unwrap(), chunk: std::ptr::null_mut(), row: 0, done: false })));
            Ok(id)
        }
    }

    fn keep(r: Res) -> i64 {
        let id = NEXT.with(|n| {
            let v = n.get();
            n.set(v + 1);
            v
        });
        RESULTS.with(|m| m.borrow_mut().insert(id, Box::new(Out { res: r, chunk: std::ptr::null_mut(), row: 0, done: false })));
        id
    }

    // executemany of one statement: prepared once, run with each set of values; the last result and the rows
    // changed in all (-1 when none changed rows). None when the SQL is not one statement (the copy loops instead).
    fn many(con: i64, sql: &str, sets: &[(Vec<DuckV>, Vec<(String, DuckV)>, bool)]) -> Result<Option<(i64, i64)>, String> {
        let l = l();
        let c = CString::new(sql).map_err(|_| "Invalid Input Error: the query holds a NUL".to_string())?;
        unsafe {
            let mut ex: P = std::ptr::null_mut();
            let n = (l.extract_statements)(p(con), c.as_ptr(), &mut ex);
            (l.destroy_extracted)(&mut ex);
            if n != 1 || sets.is_empty() {
                return Ok(None);
            }
            let mut st: P = std::ptr::null_mut();
            if (l.prepare)(p(con), c.as_ptr(), &mut st) != 0 {
                let e = cs((l.prepare_error)(st));
                (l.destroy_prepare)(&mut st);
                return Err(e);
            }
            let mut last: Option<Res> = None;
            let mut total: i64 = -1;
            for (pos, named, given) in sets {
                (l.clear_bindings)(st);
                if let Err(e) = bind_all(st, pos, named, *given) {
                    (l.destroy_prepare)(&mut st);
                    if let Some(mut r) = last {
                        (l.destroy_result)(&mut r);
                    }
                    return Err(e);
                }
                let mut r = Res { a: 0, b: 0, c: 0, d: std::ptr::null_mut(), e: std::ptr::null_mut(), internal: std::ptr::null_mut() };
                let rc = (l.execute_prepared)(st, &mut r);
                if let Some(mut old) = last.take() {
                    (l.destroy_result)(&mut old);
                }
                if rc != 0 {
                    let e = cs((l.result_error)(&mut r));
                    (l.destroy_result)(&mut r);
                    (l.destroy_prepare)(&mut st);
                    return Err(e);
                }
                if (l.result_return_type)(r) == 1 {
                    let k = (l.rows_changed)(&mut r) as i64;
                    total = if total < 0 { k } else { total + k };
                }
                last = Some(r);
            }
            (l.destroy_prepare)(&mut st);
            Ok(Some((keep(last.unwrap()), total)))
        }
    }

    // ---- types ----
    fn ident(n: &str) -> String {
        let plain = !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if plain {
            n.to_string()
        } else {
            format!("\"{}\"", n.replace('"', "\"\""))
        }
    }
    // DuckDB's LogicalType::ToString, and the box renderer's RenderType
    unsafe fn type_name(t: P, render: bool) -> String {
        let l = l();
        let alias = cs_free((l.logical_type_get_alias)(t));
        if !alias.is_empty() {
            return if render { alias.to_lowercase() } else { alias.to_uppercase() };
        }
        let id = (l.get_type_id)(t);
        if render {
            let r = match id {
                2 => "int8",
                3 => "int16",
                4 => "int32",
                5 => "int64",
                16 => "int128",
                6 => "uint8",
                7 => "uint16",
                8 => "uint32",
                9 => "uint64",
                32 => "uint128",
                _ => "",
            };
            if !r.is_empty() {
                return r.into();
            }
            if id == 24 {
                let mut c = (l.list_type_child_type)(t);
                let s = type_name(c, true);
                (l.destroy_logical_type)(&mut c);
                return s + "[]";
            }
            return lower_keep_quotes(&type_name(t, false));
        }
        let simple = match id {
            1 => "BOOLEAN",
            2 => "TINYINT",
            3 => "SMALLINT",
            4 => "INTEGER",
            5 => "BIGINT",
            6 => "UTINYINT",
            7 => "USMALLINT",
            8 => "UINTEGER",
            9 => "UBIGINT",
            10 => "FLOAT",
            11 => "DOUBLE",
            12 => "TIMESTAMP",
            13 => "DATE",
            14 => "TIME",
            15 => "INTERVAL",
            16 => "HUGEINT",
            32 => "UHUGEINT",
            17 => "VARCHAR",
            18 => "BLOB",
            20 => "TIMESTAMP_S",
            21 => "TIMESTAMP_MS",
            22 => "TIMESTAMP_NS",
            27 => "UUID",
            29 => "BIT",
            30 => "TIME WITH TIME ZONE",
            31 => "TIMESTAMP WITH TIME ZONE",
            35 => "BIGNUM",
            36 => "\"NULL\"",
            39 => "TIME_NS",
            _ => "",
        };
        if !simple.is_empty() {
            return simple.into();
        }
        match id {
            19 => format!("DECIMAL({},{})", (l.decimal_width)(t), (l.decimal_scale)(t)),
            24 => {
                let mut c = (l.list_type_child_type)(t);
                let s = type_name(c, false);
                (l.destroy_logical_type)(&mut c);
                s + "[]"
            }
            33 => {
                let mut c = (l.array_type_child_type)(t);
                let s = type_name(c, false);
                (l.destroy_logical_type)(&mut c);
                format!("{s}[{}]", (l.array_type_array_size)(t))
            }
            25 => {
                let n = (l.struct_type_child_count)(t);
                let mut parts = vec![];
                for k in 0..n {
                    let name = cs_free((l.struct_type_child_name)(t, k));
                    let mut c = (l.struct_type_child_type)(t, k);
                    parts.push(format!("{} {}", ident(&name), type_name(c, false)));
                    (l.destroy_logical_type)(&mut c);
                }
                format!("STRUCT({})", parts.join(", "))
            }
            26 => {
                let (mut k, mut v) = ((l.map_type_key_type)(t), (l.map_type_value_type)(t));
                let s = format!("MAP({}, {})", type_name(k, false), type_name(v, false));
                (l.destroy_logical_type)(&mut k);
                (l.destroy_logical_type)(&mut v);
                s
            }
            23 => {
                let n = (l.enum_dictionary_size)(t);
                let vals: Vec<String> = (0..n as u64).map(|k| format!("'{}'", cs_free((l.enum_dictionary_value)(t, k)).replace('\'', "''"))).collect();
                format!("ENUM({})", vals.join(", "))
            }
            _ => format!("TYPE{id}"),
        }
    }
    fn lower_keep_quotes(s: &str) -> String {
        let mut out = String::new();
        let mut q = false;
        let cs: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            if c == '\'' {
                out.push(c);
                if q && cs.get(i + 1) == Some(&'\'') {
                    out.push('\'');
                    i += 2;
                    continue;
                }
                q = !q;
            } else if q {
                out.push(c);
            } else {
                out.push(c.to_ascii_lowercase());
            }
            i += 1;
        }
        out
    }
    unsafe fn col_of(name: String, t: P) -> DuckCol {
        let id = (l().get_type_id)(t);
        DuckCol { name, ty: type_name(t, false), render: type_name(t, true), right: matches!(id, 2..=11 | 16 | 19 | 32) }
    }

    fn describe(con: i64, sql: &str) -> Result<(Vec<DuckCol>, i64), String> {
        let l = l();
        let c = CString::new(sql).map_err(|_| "Invalid Input Error: the query holds a NUL".to_string())?;
        unsafe {
            let mut ex: P = std::ptr::null_mut();
            let n = (l.extract_statements)(p(con), c.as_ptr(), &mut ex);
            if n == 0 {
                let e = cs((l.extract_statements_error)(ex));
                (l.destroy_extracted)(&mut ex);
                return Err(if e.is_empty() { "Invalid Input Error: No statements to execute.".into() } else { e });
            }
            let mut st: P = std::ptr::null_mut();
            let rc = (l.prepare_extracted_statement)(p(con), ex, n - 1, &mut st);
            (l.destroy_extracted)(&mut ex);
            if rc != 0 {
                let e = cs((l.prepare_error)(st));
                (l.destroy_prepare)(&mut st);
                return Err(e);
            }
            let kind = if n > 1 { 27 } else { (l.prepared_statement_type)(st) as i64 };
            let nc = (l.prepared_statement_column_count)(st);
            let mut out = vec![];
            for k in 0..nc {
                let name = cs((l.prepared_statement_column_name)(st, k));
                let mut t = (l.prepared_statement_column_logical_type)(st, k);
                out.push(col_of(name, t));
                (l.destroy_logical_type)(&mut t);
            }
            (l.destroy_prepare)(&mut st);
            Ok((out, kind))
        }
    }

    fn with_out<T>(id: i64, f: impl FnOnce(&mut Out) -> T) -> Option<T> {
        RESULTS.with(|m| m.borrow_mut().get_mut(&id).map(|o| f(o)))
    }

    fn cols(id: i64) -> Vec<DuckCol> {
        let l = l();
        with_out(id, |o| unsafe {
            let n = (l.column_count)(&mut o.res);
            (0..n)
                .map(|k| {
                    let name = cs((l.column_name)(&mut o.res, k));
                    let mut t = (l.column_logical_type)(&mut o.res, k);
                    let c = col_of(name, t);
                    (l.destroy_logical_type)(&mut t);
                    c
                })
                .collect()
        })
        .unwrap_or_default()
    }

    fn info(id: i64) -> (i64, i64, i64) {
        let l = l();
        with_out(id, |o| unsafe { ((l.result_return_type)(o.res) as i64, (l.rows_changed)(&mut o.res) as i64, (l.result_statement_type)(o.res) as i64) }).unwrap_or((0, 0, 0))
    }

    fn free(id: i64) {
        let l = l();
        if let Some(mut o) = RESULTS.with(|m| m.borrow_mut().remove(&id)) {
            unsafe {
                if !o.chunk.is_null() {
                    (l.destroy_data_chunk)(&mut o.chunk);
                }
                (l.destroy_result)(&mut o.res);
            }
        }
    }

    // ---- values from DuckDB ----
    unsafe fn valid(vec: P, row: u64) -> bool {
        let v = (l().vector_get_validity)(vec);
        v.is_null() || (*v.add((row / 64) as usize) >> (row % 64)) & 1 == 1
    }
    unsafe fn huge(data: P, row: u64) -> i128 {
        let h = *(data as *const Huge).add(row as usize);
        ((h.upper as i128) << 64) | h.lower as i128
    }
    unsafe fn uhuge(data: P, row: u64) -> u128 {
        let h = *(data as *const Huge).add(row as usize);
        ((h.upper as u64 as u128) << 64) | h.lower as u128
    }
    fn int_v(x: i128) -> DuckV {
        if x >= i64::MIN as i128 && x <= i64::MAX as i128 {
            DuckV::Int(x as i64)
        } else {
            DuckV::Big(x.to_string())
        }
    }
    fn dec_text(x: i128, scale: u8) -> String {
        let neg = x < 0;
        let mut d = x.unsigned_abs().to_string();
        let s = scale as usize;
        if s > 0 {
            while d.len() <= s {
                d.insert(0, '0');
            }
            d.insert(d.len() - s, '.');
        }
        if neg {
            d.insert(0, '-');
        }
        d
    }
    unsafe fn read(t: P, vec: P, row: u64) -> DuckV {
        let l = l();
        if !valid(vec, row) {
            return DuckV::Null;
        }
        let data = (l.vector_get_data)(vec);
        let r = row as usize;
        match (l.get_type_id)(t) {
            1 => DuckV::Bool(*(data as *const u8).add(r) != 0),
            2 => DuckV::Int(*(data as *const i8).add(r) as i64),
            3 => DuckV::Int(*(data as *const i16).add(r) as i64),
            4 => DuckV::Int(*(data as *const i32).add(r) as i64),
            5 => DuckV::Int(*(data as *const i64).add(r)),
            6 => DuckV::Int(*(data as *const u8).add(r) as i64),
            7 => DuckV::Int(*(data as *const u16).add(r) as i64),
            8 => DuckV::Int(*(data as *const u32).add(r) as i64),
            9 => int_v(*(data as *const u64).add(r) as i128),
            10 => DuckV::Float(*(data as *const f32).add(r) as f64),
            11 => DuckV::Float(*(data as *const f64).add(r)),
            16 => int_v(huge(data, row)),
            32 => {
                let x = uhuge(data, row);
                if x <= i64::MAX as u128 {
                    DuckV::Int(x as i64)
                } else {
                    DuckV::Big(x.to_string())
                }
            }
            17 | 18 => {
                let base = (data as *const u8).add(r * 16);
                let len = *(base as *const u32) as usize;
                let bytes = if len <= 12 { std::slice::from_raw_parts(base.add(4), len).to_vec() } else { std::slice::from_raw_parts(*(base.add(8) as *const *const u8), len).to_vec() };
                if (l.get_type_id)(t) == 18 {
                    DuckV::Blob(bytes)
                } else {
                    DuckV::Str(String::from_utf8_lossy(&bytes).into_owned())
                }
            }
            13 => {
                let d = *(data as *const i32).add(r);
                if d == i32::MAX || d == -i32::MAX {
                    DuckV::Bad("DATE infinity".into())
                } else {
                    DuckV::Date(d as i64)
                }
            }
            14 => DuckV::Time(*(data as *const i64).add(r)),
            12 => {
                let x = *(data as *const i64).add(r);
                if x == i64::MAX || x == -i64::MAX {
                    DuckV::Bad("TIMESTAMP infinity".into())
                } else {
                    DuckV::Ts(x)
                }
            }
            20 => DuckV::Ts(*(data as *const i64).add(r) * 1_000_000),
            21 => DuckV::Ts(*(data as *const i64).add(r) * 1_000),
            22 => DuckV::Ts((*(data as *const i64).add(r)).div_euclid(1_000)),
            15 => {
                let iv = *(data as *const Interval).add(r);
                DuckV::Interval(iv.months as i64, iv.days as i64, iv.micros)
            }
            19 => {
                let scale = (l.decimal_scale)(t);
                let x: i128 = match (l.decimal_internal_type)(t) {
                    3 => *(data as *const i16).add(r) as i128,
                    4 => *(data as *const i32).add(r) as i128,
                    5 => *(data as *const i64).add(r) as i128,
                    _ => huge(data, row),
                };
                DuckV::Dec(dec_text(x, scale))
            }
            23 => {
                let k: u64 = match (l.enum_internal_type)(t) {
                    6 => *(data as *const u8).add(r) as u64,
                    7 => *(data as *const u16).add(r) as u64,
                    _ => *(data as *const u32).add(r) as u64,
                };
                DuckV::Str(cs_free((l.enum_dictionary_value)(t, k)))
            }
            24 | 26 => {
                let e = *(data as *const [u64; 2]).add(r);
                let child = (l.list_vector_get_child)(vec);
                let mut ct = (l.list_type_child_type)(t);
                let mut out = vec![];
                for k in e[0]..e[0] + e[1] {
                    out.push(read(ct, child, k));
                }
                (l.destroy_logical_type)(&mut ct);
                if (l.get_type_id)(t) == 26 {
                    DuckV::Map(
                        out.into_iter()
                            .map(|kv| match kv {
                                DuckV::Struct(mut f) if f.len() == 2 => {
                                    let v = f.pop().unwrap().1;
                                    (f.pop().unwrap().1, v)
                                }
                                other => (other, DuckV::Null),
                            })
                            .collect(),
                    )
                } else {
                    DuckV::List(out)
                }
            }
            33 => {
                let n = (l.array_type_array_size)(t);
                let child = (l.array_vector_get_child)(vec);
                let mut ct = (l.array_type_child_type)(t);
                let out = (row * n..row * n + n).map(|k| read(ct, child, k)).collect();
                (l.destroy_logical_type)(&mut ct);
                DuckV::Arr(out)
            }
            25 => {
                let n = (l.struct_type_child_count)(t);
                let mut out = vec![];
                for k in 0..n {
                    let name = cs_free((l.struct_type_child_name)(t, k));
                    let mut ct = (l.struct_type_child_type)(t, k);
                    out.push((name, read(ct, (l.struct_vector_get_child)(vec, k), row)));
                    (l.destroy_logical_type)(&mut ct);
                }
                DuckV::Struct(out)
            }
            36 => DuckV::Null,
            _ => DuckV::Bad(type_name(t, false)),
        }
    }

    fn fetch(id: i64, max: i64) -> Result<Vec<Vec<DuckV>>, String> {
        let l = l();
        let mut rows = vec![];
        with_out(id, |o| unsafe {
            let nc = (l.column_count)(&mut o.res);
            let mut types: Vec<P> = (0..nc).map(|k| (l.column_logical_type)(&mut o.res, k)).collect();
            while !o.done && (max < 0 || (rows.len() as i64) < max) {
                if o.chunk.is_null() {
                    o.chunk = (l.fetch_chunk)(o.res);
                    o.row = 0;
                    if o.chunk.is_null() {
                        o.done = true;
                        break;
                    }
                }
                let size = (l.data_chunk_get_size)(o.chunk);
                if o.row >= size {
                    (l.destroy_data_chunk)(&mut o.chunk);
                    o.chunk = std::ptr::null_mut();
                    continue;
                }
                let vecs: Vec<P> = (0..nc).map(|k| (l.data_chunk_get_vector)(o.chunk, k)).collect();
                while o.row < size && (max < 0 || (rows.len() as i64) < max) {
                    rows.push((0..nc as usize).map(|k| read(types[k], vecs[k], o.row)).collect());
                    o.row += 1;
                }
            }
            for t in types.iter_mut() {
                (l.destroy_logical_type)(t);
            }
        });
        Ok(rows)
    }

    #[cfg(unix)]
    extern "C" {
        fn isatty(fd: i32) -> i32;
        fn ioctl(fd: i32, req: u64, ...) -> i32;
    }
    #[cfg(unix)]
    fn term() -> i64 {
        unsafe {
            if isatty(1) == 0 {
                return 0;
            }
            let mut ws = [0u16; 4];
            let req: u64 = if cfg!(target_os = "macos") { 0x40087468 } else { 0x5413 };
            if ioctl(1, req, ws.as_mut_ptr()) == 0 && ws[1] > 0 {
                ws[1] as i64
            } else {
                80
            }
        }
    }
    #[cfg(windows)]
    #[repr(C)]
    struct WinCoord {
        x: i16,
        y: i16,
    }
    #[cfg(windows)]
    #[repr(C)]
    struct WinRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }
    #[cfg(windows)]
    #[repr(C)]
    struct WinConsoleInfo {
        size: WinCoord,
        cursor: WinCoord,
        attributes: u16,
        window: WinRect,
        maximum_size: WinCoord,
    }
    #[cfg(windows)]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> P;
        fn GetConsoleMode(h: P, mode: *mut u32) -> i32;
        fn GetConsoleScreenBufferInfo(h: P, info: *mut WinConsoleInfo) -> i32;
    }
    // isatty(1) and the terminal width on Windows: the console window's columns, 80 if that is unknown, 0 when
    // stdout is no console (a pipe or a file, as isatty says on Unix)
    #[cfg(windows)]
    fn term() -> i64 {
        unsafe {
            let h = GetStdHandle(0xffff_fff5); // STD_OUTPUT_HANDLE
            let mut mode = 0u32;
            if h.is_null() || GetConsoleMode(h, &mut mode) == 0 {
                return 0;
            }
            let mut info: WinConsoleInfo = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(h, &mut info) != 0 && info.window.right > info.window.left {
                (info.window.right - info.window.left + 1) as i64
            } else {
                80
            }
        }
    }

    pub fn ops() -> DuckOps {
        DuckOps { load, open, connect, disconnect, close, query, many, describe, cols, info, fetch, free, interrupt, term }
    }
}
