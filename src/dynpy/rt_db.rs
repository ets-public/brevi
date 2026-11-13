// ---------- _brevi_sqlite: the SQLite engine for Brevi's sqlite3 ----------
// The runtime itself loads no library (caps test dynamic_runtime_reaches_nothing): every SQLite call goes
// through SQL, which only a program that imports Brevi's sqlite3 gets (src/dynpy/sqlglue.rs is pasted into it
// and sets SQL first thing in main). The glue loads the system's SQLite (libsqlite3), the engine CPython's
// sqlite3 module runs on, and under --sandbox puts a file layer (VFS) in front of it that opens files only as
// the read and write grants allow. What is here turns values to and from Python's.

pub enum DbV {
    Null,
    Int(i64),
    Float(f64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

pub struct SqlOps {
    // the library's version ("3.45.1"), or why it could not be loaded
    pub load: fn() -> Result<String, String>,
    // sqlite3_open_v2: (rc, db); the db is set even when rc is an error, for its message
    pub open: fn(&str, i64) -> (i64, i64),
    pub close: fn(i64) -> i64,
    pub errmsg: fn(i64) -> String,
    pub errcode: fn(i64) -> i64,
    pub errstr: fn(i64) -> String,
    // (rc, stmt (0: nothing to run), bytes of the SQL used)
    pub prepare: fn(i64, &[u8]) -> (i64, i64, i64),
    pub finalize: fn(i64) -> i64,
    pub reset: fn(i64) -> i64,
    pub clear: fn(i64) -> i64,
    pub step: fn(i64) -> i64,
    // bind a value already converted by Python: None / int / float / str / bytes
    pub bind: fn(i64, i64, &V) -> i64,
    pub nparams: fn(i64) -> i64,
    pub param_name: fn(i64, i64) -> Option<String>,
    pub ncols: fn(i64) -> i64,
    pub col_name: fn(i64, i64) -> Vec<u8>,
    pub col_decl: fn(i64, i64) -> Option<String>,
    pub col: fn(i64, i64) -> DbV,
    // 0 changes, 1 total changes, 2 last insert rowid, 3 autocommit
    pub db_int: fn(i64, i64) -> i64,
    // 0 readonly, 1 data count
    pub stmt_int: fn(i64, i64) -> i64,
    pub busy_timeout: fn(i64, i64) -> i64,
    pub interrupt: fn(i64),
    pub limit: fn(i64, i64, i64) -> i64,
    pub complete: fn(&[u8]) -> bool,
}

pub static SQL: std::sync::OnceLock<SqlOps> = std::sync::OnceLock::new();

fn sql_ops() -> Result<&'static SqlOps, E> {
    SQL.get().ok_or_else(|| exc("OSError", "brevi: this program was built without SQLite".into()))
}

fn db_i(v: &V) -> i64 {
    match v {
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        _ => 0,
    }
}

// a _Statement's column count (set at prepare time), for a hold to carry into fetch_next
fn stmt_nc(v: &V) -> i64 {
    match v {
        V::Obj(o) => db_i(&o.get("nc").unwrap_or(V::None)),
        _ => 0,
    }
}

fn dbv_py(d: DbV, text: i64) -> Result<V, Vec<u8>> {
    Ok(match d {
        DbV::Null => V::None,
        DbV::Int(i) => V::Int(i),
        DbV::Float(f) => V::Float(f),
        DbV::Blob(b) => bytesv(b),
        DbV::Text(t) if text == 1 => match String::from_utf8(t) {
            Ok(x) => st(x),
            Err(e) => return Err(e.into_bytes()),
        },
        DbV::Text(t) => bytesv(t),
    })
}

// run(stmt, params, db, begin, row): reset, bind a tuple or list of None / int / float / str / bytes and
// step, all in one call. begin is the implicit "BEGIN ..." to run first when the connection is in autocommit
// (CPython's legacy isolation_level); row 1 also reads the first row (text columns as bytes, with the column
// kinds alongside) so a point query needs no fetch call. Returns the result code, the column count, the
// connection's changes and last insert rowid, the first row and the column kinds
// (5 null, 1 int, 2 float, 3 text, 4 blob). The column names and declared types are not returned: Python's
// _Statement captures them once at prepare time. The result code is -1 when the parameters need Python (a
// dict, other types, a count that is off).

// None: ok (the caller steps on). Some(rc): the implicit BEGIN could not be reset or stepped. The BEGIN
// statement itself is prepared once by Python's Connection (as CPython keeps it) and reset here before reuse.
fn fast_begin(o: &SqlOps, db: i64, begin_st: i64) -> Option<i64> {
    if begin_st == 0 || (o.db_int)(db, 3) == 0 {
        return None;
    }
    let rc = (o.reset)(begin_st);
    if rc != 0 {
        return Some(rc);
    }
    let rc = (o.step)(begin_st);
    if rc != 100 && rc != 101 {
        return Some(rc);
    }
    None
}

fn fast_param(v: &V) -> bool {
    matches!(v, V::None | V::Int(_) | V::Float(_) | V::Str(_))
        || matches!(v, V::Ext(x) if x.any().is::<BytesV>())
}

// None: the parameters need Python. Some(rc): the statement was reset, bound and stepped.
fn fast_step(o: &SqlOps, stmt: i64, ps: &[V]) -> Option<i64> {
    if ps.len() as i64 != (o.nparams)(stmt) || ps.iter().any(|v| !fast_param(v)) {
        return None;
    }
    (o.reset)(stmt);
    for (k, v) in ps.iter().enumerate() {
        if (o.bind)(stmt, k as i64 + 1, v) != 0 {
            return None;
        }
    }
    Some((o.step)(stmt))
}

#[allow(clippy::type_complexity)]
fn run_fast(o: &SqlOps, stmt: i64, db: i64, ps: &[V], row: i64)
    -> (i64, i64, i64, i64, Vec<V>, Vec<V>) {
    let none = || (-1, 0, 0, 0, vec![], vec![]);
    let rc = match fast_step(o, stmt, ps) {
        Some(rc) => rc,
        None => return none(),
    };
    let nc = if rc == 100 || rc == 101 { (o.ncols)(stmt) } else { 0 };
    let mut first = vec![];
    let mut kinds = vec![];
    if row == 1 && rc == 100 {
        first = Vec::with_capacity(nc as usize);
        kinds = Vec::with_capacity(nc as usize);
        for k in 0..nc {
            let d = (o.col)(stmt, k);
            kinds.push(V::Int(match d {
                DbV::Null => 5,
                DbV::Int(_) => 1,
                DbV::Float(_) => 2,
                DbV::Text(_) => 3,
                DbV::Blob(_) => 4,
            }));
            first.push(dbv_py(d, 0).unwrap_or(V::None));
        }
    }
    (rc, nc, (o.db_int)(db, 0), (o.db_int)(db, 2), first, kinds)
}

// dml(stmt, params, db, begin): run() minus the columns, for DML: (result code, changes, last insert rowid).
// begin is the connection's prepared "BEGIN ..." statement (0: none). The result code is -1 when the
// parameters need Python.
fn dml_fast(o: &SqlOps, stmt: i64, db: i64, ps: &[V], begin_st: i64) -> (i64, i64, i64) {
    if let Some(rc) = fast_begin(o, db, begin_st) {
        return (rc, 0, 0);
    }
    let rc = match fast_step(o, stmt, ps) {
        Some(rc) => rc,
        None => return (-1, 0, 0),
    };
    let out = (rc, (o.db_int)(db, 0), (o.db_int)(db, 2));
    out
}

// A cursor's held statement as a native value (Cursor._hold). When the cursor lets the statement go, this value's
// Drop resets it (cached) or finalizes it, and clears its in_use flag, so a Cursor needs no Python __del__ and a
// discarded cursor costs none of the finalizer queue (rt_core's FIN_PENDING) or a Python-level call. The stmt is
// the Python _Statement object, kept only so in_use can be cleared; the cache holds it too, so no cycle.
struct StmtHold {
    st: Cell<i64>,
    cached: bool,
    stmt: V,
    // the statement's column count (constant for a prepared statement), so fetch_next need not ask the engine
    nc: i64,
}

thread_local! {
    // live holds by database: Connection.close invalidates them before sqlite3_close_v2, so a hold dropped
    // after the close does not reset or finalize an already-finalized statement
    static HOLDS: RefCell<HashMap<i64, Vec<std::rc::Weak<Box<dyn Ext>>>>> = RefCell::new(HashMap::new());
}

impl Drop for StmtHold {
    fn drop(&mut self) {
        let st = self.st.replace(0);
        if st != 0 {
            if let Some(o) = SQL.get() {
                let _ = if self.cached { (o.reset)(st) } else { (o.finalize)(st) };
            }
        }
        if let V::Obj(o) = &self.stmt {
            o.set(sym("in_use"), V::Bool(false));
        }
    }
}

impl Ext for StmtHold {
    fn tname(&self) -> &'static str {
        "sqlite3.Statement"
    }
    fn repr(&self) -> String {
        "<sqlite3.Statement>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}

// a cursor's held statement, registered so Connection.close can invalidate it before sqlite3_close_v2
fn make_hold(st: i64, cached: bool, stmt: V, db: i64) -> V {
    let nc = stmt_nc(&stmt);
    let h = ExtRc::new(StmtHold { st: Cell::new(st), cached, stmt, nc });
    HOLDS.with(|hs| {
        let mut hs = hs.borrow_mut();
        let v = hs.entry(db).or_default();
        v.retain(|w| w.strong_count() > 0);
        v.push(ExtRc::downgrade(&h));
    });
    V::Ext(h)
}

// runhold_fast: fast_step plus the native hold, for the point-query hot path (one engine call runs and holds)
fn runhold_fast(o: &SqlOps, st: i64, db: i64, ps: &[V], stmt: &V, cached: bool) -> (i64, i64, Option<V>) {
    let rc = match fast_step(o, st, ps) {
        Some(rc) => rc,
        None => return (-1, 0, None),
    };
    let lastrowid = (o.db_int)(db, 2);
    if rc != 100 {
        return (rc, lastrowid, None);
    }
    (rc, lastrowid, Some(make_hold(st, cached, stmt.clone(), db)))
}

// a statement the native execute path is done with (its in_use was never set): _Statement.release without
// the Python call
fn release_stmt(o: &SqlOps, so: &Obj, st: i64, cached: bool) {
    if cached {
        let _ = (o.reset)(st);
    } else {
        so.set(sym("st"), V::Int(0));
        let _ = (o.finalize)(st);
    }
    so.set(sym("in_use"), V::Bool(false));
}

// the current row's columns as a tuple, TEXT as str (text 1) or bytes; bad: a text column that was not UTF-8
// (its bytes are in the tuple). Built straight into the tuple, no intermediate Vec. nc: the statement's column
// count, kept by Python's _Statement from prepare time (a prepared statement's count never changes).
fn row_tup(o: &SqlOps, st_: i64, nc: usize, text: i64) -> (Tup, i64) {
    let mut bad: i64 = -1;
    let t = Tup::build(nc, |d| unsafe {
        for c in 0..nc {
            let v = match dbv_py((o.col)(st_, c as i64), text) {
                Ok(v) => v,
                Err(b) => {
                    if bad < 0 {
                        bad = c as i64;
                    }
                    bytesv(b)
                }
            };
            std::ptr::write(d.add(c), v);
        }
    });
    (t, bad)
}

// Cursor.__next__ / fetchone as native functions: the per-row hot path. The Python originals (_next_py and
// _fetchone_py, set on Cursor by Brevi's sqlite3) own every other case, so all slow paths (converters, a
// non-str text_factory, a closed cursor or connection) and their exact error messages stay Python's. next 1
// is __next__ (StopIteration comes from the Python original); next 0 is fetchone.
fn cursor_py_fetch(me: &V, py: &'static str) -> R {
    if let V::Obj(co) = me {
        if let Some(f) = co.cls.lookup(py) {
            return call1(&f, me.clone());
        }
    }
    type_err(format!("'{}' object is not an iterator", tn(me)))
}

fn apply_row_factory(me: &V, row: V) -> R {
    if let V::Obj(co) = me {
        match co.get("row_factory") {
            None | Some(V::None) => return Ok(row),
            Some(f) => return call(&f, Args::p(vec![me.clone(), row])),
        }
    }
    Ok(row)
}

fn cursor_fetch_native(me: V, py: &'static str) -> R {
    let o = sql_ops()?;
    let V::Obj(co) = &me else {
        return type_err(format!("'{}' object is not an iterator", tn(&me)));
    };
    let Some(V::Obj(so)) = co.get("_stmt") else { return cursor_py_fetch(&me, py); };
    if db_i(&so.get("st").unwrap_or(V::None)) == 0 {
        return cursor_py_fetch(&me, py);
    }
    // the plain fast case only: no converters and text_factory is still the str class. Both are checked per
    // fetch (not cached at execute time), so a change between execute and fetch uses the new setting, as
    // CPython's fetch does.
    if !matches!(co.get("_conv"), Some(V::None) | None) {
        return cursor_py_fetch(&me, py);
    }
    let tf = match co.get("connection") {
        Some(V::Obj(cn)) => cn.get("text_factory").unwrap_or(V::None),
        _ => return cursor_py_fetch(&me, py),
    };
    if !is(&tf, &V::Class(bclass("str"))) {
        return cursor_py_fetch(&me, py);
    }
    let hold = co.get("_hold");
    let Some(V::Ext(he)) = &hold else { return cursor_py_fetch(&me, py); };
    let Some(h) = he.any().downcast_ref::<StmtHold>() else { return cursor_py_fetch(&me, py); };
    let st_ = h.st.get();
    if st_ == 0 {
        return cursor_py_fetch(&me, py);
    }
    let (row, bad) = row_tup(o, st_, h.nc.max(0) as usize, 1);
    if bad >= 0 {
        // the statement stays on the bad row; Python's fetch re-reads it and raises the exact message
        return cursor_py_fetch(&me, py);
    }
    let rc = (o.step)(st_);
    if rc == 100 {
        return apply_row_factory(&me, V::Tuple(row));
    }
    if rc == 101 {
        if let V::Obj(so2) = &h.stmt {
            if matches!(so2.get("is_dml"), Some(V::Bool(true))) {
                if let Some(V::Obj(cn)) = co.get("connection") {
                    let db = db_i(&cn.get("_db").unwrap_or(V::None));
                    co.set(sym("rowcount"), V::Int((o.db_int)(db, 0)));
                }
            }
        }
        co.set(sym("_stmt"), V::None);
        co.set(sym("_hold"), V::None);
        return apply_row_factory(&me, V::Tuple(row));
    }
    // a step error: CPython's fetch clears the cursor and raises the connection's error
    co.set(sym("_stmt"), V::None);
    co.set(sym("_hold"), V::None);
    match co.cls.lookup("_step_error") {
        Some(f) => call1(&f, me.clone()),
        None => type_err(format!("'{}' object is not an iterator", tn(&me))),
    }
}

thread_local! {
    static CURSOR_NEXT: RefCell<Option<V>> = const { RefCell::new(None) };
    static CURSOR_FETCHONE: RefCell<Option<V>> = const { RefCell::new(None) };
}

fn cursor_next_func() -> V {
    CURSOR_NEXT.with(|c| {
        if let Some(v) = &*c.borrow() {
            return v.clone();
        }
        let f = func("__next__", |a: Args| {
            if a.p.len() == 1 && a.k.is_empty() {
                return cursor_fetch_native(a.p[0].clone(), "_next_py");
            }
            // wrong arguments: Python's original __next__ raises CPython's exact TypeError
            if let Some(V::Obj(co)) = a.p.first() {
                if let Some(f) = co.cls.lookup("_next_py") {
                    return call(&f, a);
                }
            }
            type_err(format!("'{}' object is not an iterator", tn(a.p.first().map_or(&V::None, |x| x))))
        });
        let f = with_fast(f, FastFn::F1(|me| cursor_fetch_native(me, "_next_py")));
        *c.borrow_mut() = Some(f.clone());
        f
    })
}

fn cursor_fetchone_func() -> V {
    CURSOR_FETCHONE.with(|c| {
        if let Some(v) = &*c.borrow() {
            return v.clone();
        }
        let f = func("fetchone", |a: Args| {
            if a.p.len() == 1 && a.k.is_empty() {
                return cursor_fetch_native(a.p[0].clone(), "_fetchone_py");
            }
            // wrong arguments: Python's original fetchone raises CPython's exact TypeError
            if let Some(V::Obj(co)) = a.p.first() {
                if let Some(f) = co.cls.lookup("_fetchone_py") {
                    return call(&f, a);
                }
            }
            type_err(format!("'{}' object is not an iterator", tn(a.p.first().map_or(&V::None, |x| x))))
        });
        let f = with_fast(f, FastFn::F1(|me| cursor_fetch_native(me, "_fetchone_py")));
        *c.borrow_mut() = Some(f.clone());
        f
    })
}

// Connection.execute as a native function: the repeated-statement hot paths (inserts and point queries).
// Every other case runs Python's original execute (_execute_py), so all slow paths (adapters, converters,
// dict parameters, subclasses) and their exact errors stay Python's.
fn conn_execute_py(me: &V, sql: V, params: V) -> R {
    if let V::Obj(co) = me {
        if let Some(f) = co.cls.lookup("_execute_py") {
            return call(&f, Args::p(vec![me.clone(), sql, params]));
        }
    }
    type_err(format!("'{}' object is not callable", tn(me)))
}

fn conn_db_error(me: &V) -> R {
    if let V::Obj(co) = me {
        if let Some(f) = co.cls.lookup("_raise_db_error") {
            return call1(&f, me.clone());
        }
    }
    type_err(format!("'{}' object is not callable", tn(me)))
}

fn cursor_new_for(con: &V) -> V {
    let cls = SQLITE_CURSOR_CLS.with(|c| c.borrow().clone());
    let Some(cls) = cls else { return V::None };
    let co = Obj::new(cls);
    co.set(sym("connection"), con.clone());
    if let V::Obj(cn) = con {
        if let Some(rf) = cn.get("row_factory") {
            if !matches!(rf, V::None) {
                co.set(sym("row_factory"), rf);
            }
        }
    }
    V::Obj(co)
}

fn conn_execute(me: V, sql: V, params: V) -> R {
    let o = sql_ops()?;
    let V::Obj(co) = &me else { return conn_execute_py(&me, sql, params); };
    if !matches!(co.get("_exact"), Some(V::Bool(true))) {
        return conn_execute_py(&me, sql, params);
    }
    let db = db_i(&co.get("_db").unwrap_or(V::None));
    if db == 0 || !matches!(sql, V::Str(_)) {
        return conn_execute_py(&me, sql, params);
    }
    if ADAPTED.with(|a| a.get()) {
        return conn_execute_py(&me, sql, params);
    }
    let st = co.get("_last_stmt");
    let Some(st) = st else { return conn_execute_py(&me, sql, params); };
    let V::Obj(so) = &st else { return conn_execute_py(&me, sql, params); };
    let st_id = db_i(&so.get("st").unwrap_or(V::None));
    if st_id == 0 || matches!(so.get("in_use"), Some(V::Bool(true))) {
        return conn_execute_py(&me, sql, params);
    }
    let last_sql = co.get("_last_sql");
    if !matches!(&last_sql, Some(ls) if is(ls, &sql)) {
        return conn_execute_py(&me, sql, params);
    }
    let cached = matches!(so.get("cached"), Some(V::Bool(true)));
    let cur = cursor_new_for(&me);
    let stmt_nc = db_i(&so.get("nc").unwrap_or(V::None));
    let detect = db_i(&co.get("_detect").unwrap_or(V::None)) != 0;
    // converters (detect_types) on a statement with columns, a SELECT or a DML ... RETURNING: Python's path
    if detect && stmt_nc > 0 {
        return conn_execute_py(&me, sql, params);
    }
    if matches!(so.get("is_dml"), Some(V::Bool(true))) {
        let begin = db_i(&co.get("_begin_st").unwrap_or(V::None));
        let (rc, changes, lastrowid) = match &params {
            V::Tuple(t) => dml_fast(o, st_id, db, t.as_slice(), begin),
            V::List(l) => dml_fast(o, st_id, db, &l.borrow(), begin),
            _ => return conn_execute_py(&me, sql, params),
        };
        if rc == -1 {
            return conn_execute_py(&me, sql, params);
        }
        // ... RETURNING: CPython gives the cursor a description
        if (rc == 100 || rc == 101) && stmt_nc > 0 {
            if let V::Obj(cco) = &cur {
                cco.set(sym("_stmt_meta"), st.clone());
            }
        }
        if rc == 101 {
            if let V::Obj(cco) = &cur {
                cco.set(sym("lastrowid"), V::Int(lastrowid));
                cco.set(sym("rowcount"), V::Int(changes));
            }
            if !cached {
                so.set(sym("st"), V::Int(0));
                let _ = (o.finalize)(st_id);
            }
            return Ok(cur);
        }
        if rc == 100 {
            so.set(sym("in_use"), V::Bool(true));
            if let V::Obj(cco) = &cur {
                cco.set(sym("lastrowid"), V::Int(lastrowid));
                cco.set(sym("rowcount"), V::Int(0));
                cco.set(sym("_stmt"), st.clone());
                cco.set(sym("_hold"), make_hold(st_id, cached, st.clone(), db));
            }
            return Ok(cur);
        }
        release_stmt(o, &so, st_id, cached);
        return conn_db_error(&me);
    }
    if detect {
        return conn_execute_py(&me, sql, params);
    }
    let (rc, lastrowid, hold) = match &params {
        V::Tuple(t) => runhold_fast(o, st_id, db, t.as_slice(), &st, cached),
        V::List(l) => runhold_fast(o, st_id, db, &l.borrow(), &st, cached),
        _ => return conn_execute_py(&me, sql, params),
    };
    if rc == -1 {
        return conn_execute_py(&me, sql, params);
    }
    if rc != 100 && rc != 101 {
        release_stmt(o, &so, st_id, cached);
        return conn_db_error(&me);
    }
    if let V::Obj(cco) = &cur {
        cco.set(sym("lastrowid"), V::Int(lastrowid));
        if db_i(&so.get("nc").unwrap_or(V::None)) > 0 {
            cco.set(sym("_stmt_meta"), st.clone());
        }
        if rc == 101 {
            if !cached {
                so.set(sym("st"), V::Int(0));
                let _ = (o.finalize)(st_id);
            }
        } else {
            so.set(sym("in_use"), V::Bool(true));
            cco.set(sym("_stmt"), st.clone());
            cco.set(sym("_hold"), hold.unwrap_or(V::None));
        }
    }
    Ok(cur)
}

thread_local! {
    static SQLITE_CURSOR_CLS: RefCell<Option<Rc<Class>>> = const { RefCell::new(None) };
    static ADAPTED: Cell<bool> = const { Cell::new(false) };
    static CONN_EXECUTE: RefCell<Option<V>> = const { RefCell::new(None) };
}

fn conn_execute_func() -> V {
    CONN_EXECUTE.with(|c| {
        if let Some(v) = &*c.borrow() {
            return v.clone();
        }
        let f = func("execute", |a: Args| match a.p.len() {
            2 if a.k.is_empty() => conn_execute(a.p[0].clone(), a.p[1].clone(), tup(vec![])),
            3 if a.k.is_empty() => conn_execute(a.p[0].clone(), a.p[1].clone(), a.p[2].clone()),
            _ => {
                if let Some(V::Obj(co)) = a.p.first() {
                    if let Some(f) = co.cls.lookup("_execute_py") {
                        return call(&f, a);
                    }
                }
                type_err(format!("'{}' object is not callable", tn(a.p.first().map_or(&V::None, |x| x))))
            }
        });
        let f = with_fast(f, FastFn::F3(|me, sql, params| conn_execute(me, sql, params)));
        *c.borrow_mut() = Some(f.clone());
        f
    })
}

pub fn brevi_sqlite_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    let o = sql_ops()?;
    let i = |k: usize| db_i(&arg(k));
    match n {
        "cursor_new" => {
            // Connection.execute's hot path makes its cursor natively (CPython makes one in C too); the
            // class's Python __init__ is still what a direct sqlite3.Cursor(...) call runs.
            let V::Class(c) = &arg(0) else { return type_err("cursor_new() argument 1 must be a class".into()) };
            let co = Obj::new(c.clone());
            let con = arg(1);
            co.set(sym("connection"), con.clone());
            if let V::Obj(cn) = &con {
                if let Some(rf) = cn.get("row_factory") {
                    if !matches!(rf, V::None) {
                        co.set(sym("row_factory"), rf);
                    }
                }
            }
            Ok(V::Obj(co))
        }
        "set_cursor_class" => {
            if let V::Class(c) = &arg(0) {
                SQLITE_CURSOR_CLS.with(|s| *s.borrow_mut() = Some(c.clone()));
            }
            Ok(V::None)
        }
        "set_adapted" => {
            ADAPTED.with(|a| a.set(true));
            Ok(V::None)
        }
        "load" => match (o.load)() {
            Ok(v) => Ok(st(v)),
            Err(m) => Err(exc("ImportError", m)),
        },
        "open" => {
            let (rc, db) = (o.open)(&net_s(&arg(0)), i(1));
            Ok(tupa([V::Int(rc), V::Int(db)]))
        }
        "close" => {
            let db = i(0);
            HOLDS.with(|hs| {
                if let Some(v) = hs.borrow_mut().remove(&db) {
                    for w in v {
                        if let Some(r) = w.upgrade() {
                            if let Some(h) = (*r).any().downcast_ref::<StmtHold>() {
                                h.st.set(0);
                            }
                        }
                    }
                }
            });
            Ok(V::Int((o.close)(db)))
        }
        "errmsg" => Ok(st((o.errmsg)(i(0)))),
        "errcode" => Ok(V::Int((o.errcode)(i(0)))),
        "errstr" => Ok(st((o.errstr)(i(0)))),
        "prepare" => {
            // the offset comes back in characters of the str
            let sql = net_s(&arg(1));
            let (rc, stmt, used) = (o.prepare)(i(0), sql.as_bytes());
            let chars = sql.get(..used.max(0) as usize).map_or(sql.chars().count(), |p| p.chars().count());
            Ok(tupa([V::Int(rc), V::Int(stmt), V::Int(chars as i64)]))
        }
        "finalize" => Ok(V::Int((o.finalize)(i(0)))),
        "reset" => Ok(V::Int((o.reset)(i(0)))),
        "clear" => Ok(V::Int((o.clear)(i(0)))),
        "step" => Ok(V::Int((o.step)(i(0)))),
        // bind(stmt, index, value, kind): the value is already what _bind_one turned it into
        "bind" => Ok(V::Int((o.bind)(i(0), i(1), &arg(2)))),
        "nparams" => Ok(V::Int((o.nparams)(i(0)))),
        "param_name" => Ok((o.param_name)(i(0), i(1)).map_or(V::None, st)),
        "ncols" => Ok(V::Int((o.ncols)(i(0)))),
        "col_name" => Ok(st(String::from_utf8_lossy(&(o.col_name)(i(0), i(1))).into_owned())),
        "col_decl" => Ok((o.col_decl)(i(0), i(1)).map_or(V::None, st)),
        // row(stmt, text): the row's values, with TEXT as str (text 1) or bytes; (values, -1), or (values, k)
        // when column k's text is not UTF-8 (its bytes are in values[k])
        "row" => {
            let st_ = i(0);
            let text = i(1);
            let nc = (o.ncols)(st_);
            let mut out = Vec::with_capacity(nc as usize);
            let mut bad = -1;
            for c in 0..nc {
                match dbv_py((o.col)(st_, c), text) {
                    Ok(v) => out.push(v),
                    Err(b) => {
                        if bad < 0 {
                            bad = c;
                        }
                        out.push(bytesv(b));
                    }
                }
            }
            Ok(tupa([list(out), V::Int(bad)]))
        }
        // col(stmt, k): (type, value) with TEXT as bytes, for converters and text factories
        "col" => {
            let d = (o.col)(i(0), i(1));
            let k = match &d {
                DbV::Null => 5,
                DbV::Int(_) => 1,
                DbV::Float(_) => 2,
                DbV::Text(_) => 3,
                DbV::Blob(_) => 4,
            };
            Ok(tupa([V::Int(k), dbv_py(d, 0).unwrap_or(V::None)]))
        }
        "run" => {
            let stv = i(0);
            let db = i(2);
            let text = i(3);
            let (rc, nc, changes, lastrowid, row, kinds) = match &arg(1) {
                V::Tuple(t) => run_fast(o, stv, db, t.as_slice(), text),
                V::List(l) => run_fast(o, stv, db, &l.borrow(), text),
                _ => (-1, 0, 0, 0, vec![], vec![]),
            };
            Ok(tupa([
                V::Int(rc),
                V::Int(nc),
                V::Int(changes),
                V::Int(lastrowid),
                tup(row),
                tup(kinds),
            ]))
        }
        // runhold(stmt, params, db, statement, cached): run() plus the native hold for the cursor, in one
        // dispatch (the point query path runs and holds with one engine call). The hold is None when the
        // parameters need Python, which then binds and holds the old way.
        "runhold" => {
            let stv = i(0);
            let db = i(2);
            let cached = i(4) != 0;
            let (rc, lastrowid, hold) = match &arg(1) {
                V::Tuple(t) => runhold_fast(o, stv, db, t.as_slice(), &arg(3), cached),
                V::List(l) => runhold_fast(o, stv, db, &l.borrow(), &arg(3), cached),
                _ => (-1, 0, None),
            };
            Ok(tupa([V::Int(rc), V::Int(lastrowid), hold.unwrap_or(V::None)]))
        }
        // dml(stmt, params, db, begin): run() minus the columns, for DML
        "dml" => {
            let stv = i(0);
            let db = i(2);
            let begin = i(3);
            let (rc, changes, lastrowid) = match &arg(1) {
                V::Tuple(t) => dml_fast(o, stv, db, t.as_slice(), begin),
                V::List(l) => dml_fast(o, stv, db, &l.borrow(), begin),
                _ => (-1, 0, 0),
            };
            Ok(tupa([V::Int(rc), V::Int(changes), V::Int(lastrowid)]))
        }
        // fetch(stmt, nc, text): the current row (as row() gives it) and the result code of the next step
        "fetch" => {
            let st_ = i(0);
            let nc = i(1).max(0) as usize;
            let text = i(2);
            let (row, bad) = row_tup(o, st_, nc, text);
            if bad >= 0 {
                return Ok(tupa([V::Tuple(row), V::Int(bad), V::Int(0)]));
            }
            let rc = (o.step)(st_);
            Ok(tupa([V::Tuple(row), V::Int(-1), V::Int(rc)]))
        }
        // fetch_all(stmt, nc, text, max): rows as fetch gives them, until the statement is done, a step fails, a
        // text is not UTF-8 (that row is left unread) or max rows (< 0: no limit);
        // (rows, bad column, last result code)
        "fetch_all" => {
            let st_ = i(0);
            let nc = i(1).max(0) as usize;
            let text = i(2);
            let max = i(3);
            let mut rows = vec![];
            loop {
                if max >= 0 && rows.len() as i64 >= max {
                    return Ok(tupa([list(rows), V::Int(-1), V::Int(100)]));
                }
                let (row, bad) = row_tup(o, st_, nc, text);
                if bad >= 0 {
                    return Ok(tupa([list(rows), V::Int(bad), V::Int(100)]));
                }
                rows.push(V::Tuple(row));
                let rc = (o.step)(st_);
                if rc != 100 {
                    return Ok(tupa([list(rows), V::Int(-1), V::Int(rc)]));
                }
            }
        }
        "db_int" => Ok(V::Int((o.db_int)(i(0), i(1)))),
        "stmt_int" => Ok(V::Int((o.stmt_int)(i(0), i(1)))),
        "busy_timeout" => Ok(V::Int((o.busy_timeout)(i(0), i(1)))),
        "interrupt" => {
            (o.interrupt)(i(0));
            Ok(V::None)
        }
        "limit" => Ok(V::Int((o.limit)(i(0), i(1), i(2)))),
        // hold(stmt, cached, statement, db): the cursor takes this statement; see StmtHold
        "hold" => {
            let st_ = i(0);
            let db = i(3);
            Ok(make_hold(st_, i(1) != 0, arg(2), db))
        }
        // fetch_next(cursor, hold): the hot path of __next__ and fetchone in one dispatch. The hold carries the
        // statement and its column count; the row (TEXT as str) is read, then the statement is stepped. Done
        // clears the cursor's _stmt/_hold (the hold's Drop releases the statement) and returns None; a step
        // error returns [1, code, message] and a text column that was not UTF-8 returns [2, column name, bytes],
        // both for Python to build the exact exception (they leave the statement where it is).
        "fetch_next" => {
            let cur = arg(0);
            let V::Obj(co) = &cur else { return Ok(V::None) };
            let V::Ext(he) = &arg(1) else {
                co.set(sym("_stmt"), V::None);
                return Ok(V::None);
            };
            let Some(h) = he.any().downcast_ref::<StmtHold>() else {
                co.set(sym("_stmt"), V::None);
                return Ok(V::None);
            };
            let st_ = h.st.get();
            if st_ == 0 {
                // the connection was closed under this cursor; Python's next look at stmt.st raises _check
                return Ok(V::None);
            }
            let (row, bad) = row_tup(o, st_, h.nc.max(0) as usize, 1);
            if bad >= 0 {
                let name = String::from_utf8_lossy(&(o.col_name)(st_, bad)).into_owned();
                let bytes = match row.get(bad as usize) {
                    Some(V::Ext(x)) => x.any().downcast_ref::<BytesV>().map_or_else(Vec::new, |b| b.0.clone()),
                    _ => vec![],
                };
                return Ok(list(vec![V::Int(2), st(name), bytesv(bytes)]));
            }
            let rc = (o.step)(st_);
            if rc == 100 {
                return Ok(V::Tuple(row));
            }
            if rc == 101 {
                if let V::Obj(so) = &h.stmt {
                    let dml = matches!(so.get("is_dml"), Some(V::Bool(true)));
                    if dml {
                        let db = match co.get("connection") {
                            Some(V::Obj(cn)) => db_i(&cn.get("_db").unwrap_or(V::None)),
                            _ => 0,
                        };
                        co.set(sym("rowcount"), V::Int((o.db_int)(db, 0)));
                    }
                }
                co.set(sym("_stmt"), V::None);
                co.set(sym("_hold"), V::None);
                // the row just read is still the last row; the next call sees _stmt cleared
                return Ok(V::Tuple(row));
            }
            let db = match co.get("connection") {
                Some(V::Obj(cn)) => db_i(&cn.get("_db").unwrap_or(V::None)),
                _ => 0,
            };
            Ok(list(vec![V::Int(1), V::Int((o.errcode)(db)), st((o.errmsg)(db))]))
        }
        "complete" => Ok(V::Bool((o.complete)(net_s(&arg(0)).as_bytes()))),
        _ => Err(unsupported(&format!("_brevi_sqlite.{n}"))),
    }
}

// ---------- _brevi_duckdb: the DuckDB engine for Brevi's duckdb ----------
// As with SQLite, the glue (src/dynpy/duckglue.rs, pasted into a program that imports Brevi's duckdb) loads the
// engine and sets DUCK; what is here turns DuckDB's values to Python's and back.

pub enum DuckV {
    Null,
    Bool(bool),
    Int(i64),
    // a HUGEINT / UHUGEINT / UBIGINT past i64, in decimal
    Big(String),
    Float(f64),
    Str(String),
    Blob(Vec<u8>),
    // days since 1970-01-01
    Date(i64),
    // microseconds since midnight
    Time(i64),
    // microseconds since 1970-01-01 00:00:00
    Ts(i64),
    // months, days, microseconds
    Interval(i64, i64, i64),
    // a DECIMAL's text ("12.345")
    Dec(String),
    List(Vec<DuckV>),
    // an ARRAY: a tuple in Python
    Arr(Vec<DuckV>),
    Struct(Vec<(String, DuckV)>),
    Map(Vec<(DuckV, DuckV)>),
    // a value of a type Brevi's duckdb does not turn into Python (its DuckDB type name)
    Bad(String),
}

pub struct DuckCol {
    pub name: String,
    // DuckDB's name of the type ("INTEGER", "DECIMAL(10,3)", "STRUCT(a INTEGER)")
    pub ty: String,
    // the box renderer's name of it ("int32", "decimal(10,3)")
    pub render: String,
    // numbers are right-aligned in the box
    pub right: bool,
}

pub struct DuckOps {
    pub load: fn() -> Result<String, String>,
    // open(path, config) -> (database, connection)
    pub open: fn(&str, &[(String, String)]) -> Result<(i64, i64), String>,
    pub connect: fn(i64) -> Result<i64, String>,
    pub disconnect: fn(i64),
    pub close: fn(i64),
    // query(connection, sql, positional, named): a result
    // (given: values were passed at all, None being none)
    pub query: fn(i64, &str, &[DuckV], &[(String, DuckV)], bool) -> Result<i64, String>,
    // executemany of one statement, prepared once: (the last result, rows changed in all); None for a script
    pub many: fn(i64, &str, &[(Vec<DuckV>, Vec<(String, DuckV)>, bool)]) -> Result<Option<(i64, i64)>, String>,
    // the columns and statement type (DUCKDB_STATEMENT_TYPE_*) of the last statement, without running it
    pub describe: fn(i64, &str) -> Result<(Vec<DuckCol>, i64), String>,
    pub cols: fn(i64) -> Vec<DuckCol>,
    // (return type: 1 changed rows, 2 nothing, 3 query result; rows changed; statement type)
    pub info: fn(i64) -> (i64, i64, i64),
    // up to max rows (< 0: all) from where the last fetch stopped
    pub fetch: fn(i64, i64) -> Result<Vec<Vec<DuckV>>, String>,
    pub free: fn(i64),
    pub interrupt: fn(i64),
    // the terminal's width when stdout is one, else 0
    pub term: fn() -> i64,
}

pub static DUCK: std::sync::OnceLock<DuckOps> = std::sync::OnceLock::new();

fn duck_ops() -> Result<&'static DuckOps, E> {
    DUCK.get().ok_or_else(|| exc("OSError", "brevi: this program was built without DuckDB".into()))
}

fn duck_err(m: String) -> E {
    // the copy turns "Catalog Error: ..." into its exception class
    exc("RuntimeError", m)
}

fn duck_py(d: DuckV) -> R {
    Ok(match d {
        DuckV::Null => V::None,
        DuckV::Bool(b) => V::Bool(b),
        DuckV::Int(i) => V::Int(i),
        DuckV::Big(s) => parse_int(&s, 10).unwrap_or(V::None),
        DuckV::Float(f) => V::Float(f),
        DuckV::Str(s) => st(s),
        DuckV::Blob(b) => bytesv(b),
        DuckV::Date(d) => dtv(DT { ord: ymd_to_ord(1970, 1, 1) + d, us: 0, has_time: false, tz: None, fold: false }),
        DuckV::Time(us) => tmv(us),
        DuckV::Ts(us) => {
            let day = 86_400_000_000i64;
            let (d, r) = (us.div_euclid(day), us.rem_euclid(day));
            let ord = ymd_to_ord(1970, 1, 1) + d;
            if !(1..=3652059).contains(&ord) {
                return Err(exc("ValueError", format!("year is out of range")));
            }
            dtv(DT { ord, us: r, has_time: true, tz: None, fold: false })
        }
        DuckV::Interval(m, d, us) => tdv(TD { us: ((m * 30 + d) as i128) * 86_400_000_000 + us as i128 }),
        DuckV::Dec(s) => decimal_new(Args::p(vec![st(s)]))?,
        DuckV::List(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(duck_py(x)?);
            }
            list(out)
        }
        DuckV::Arr(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(duck_py(x)?);
            }
            tup(out)
        }
        DuckV::Struct(kv) => {
            let mut d = Dict::new();
            for (k, v) in kv {
                d.set(st(k), duck_py(v)?)?;
            }
            V::Dict(Rc::new(RefCell::new(d)))
        }
        DuckV::Map(kv) => {
            let mut d = Dict::new();
            for (k, v) in kv {
                d.set(duck_py(k)?, duck_py(v)?)?;
            }
            V::Dict(Rc::new(RefCell::new(d)))
        }
        DuckV::Bad(t) => return Err(exc("NotImplementedError", format!("brevi: Brevi's duckdb cannot turn a {t} value into Python"))),
    })
}

fn py_duck(v: &V) -> Result<DuckV, E> {
    Ok(match v {
        V::None => DuckV::Null,
        V::Bool(b) => DuckV::Bool(*b),
        V::Int(i) => DuckV::Int(*i),
        V::Big(_) => DuckV::Big(repr(v)?),
        V::Float(f) => DuckV::Float(*f),
        V::Str(s) => DuckV::Str(s.text().to_string()),
        V::List(l) => DuckV::List(l.borrow().iter().map(py_duck).collect::<Result<_, _>>()?),
        V::Tuple(t) => DuckV::List(t.iter().map(py_duck).collect::<Result<_, _>>()?),
        V::Dict(d) => {
            let mut kv = vec![];
            for (k, x) in d.borrow().items() {
                match &k {
                    V::Str(s) => kv.push((s.text().to_string(), py_duck(&x)?)),
                    _ => return Err(exc("NotImplementedError", "brevi: Brevi's duckdb binds dicts with str keys only".into())),
                }
            }
            DuckV::Struct(kv)
        }
        V::Ext(x) => {
            if let Some(b) = x.any().downcast_ref::<BytesV>() {
                DuckV::Blob(b.0.clone())
            } else if let Some(d) = x.any().downcast_ref::<DTV>() {
                if d.0.tz.is_some() {
                    return Err(exc("NotImplementedError", "brevi: Brevi's duckdb does not bind aware datetimes".into()));
                }
                let days = d.0.ord - ymd_to_ord(1970, 1, 1);
                if d.0.has_time {
                    DuckV::Ts(days * 86_400_000_000 + d.0.us)
                } else {
                    DuckV::Date(days)
                }
            } else if let Some(t) = x.any().downcast_ref::<TMV>() {
                DuckV::Time(t.0)
            } else if let Some(t) = x.any().downcast_ref::<TDV>() {
                let (d, s, us) = t.0.parts();
                DuckV::Interval(0, d, s * 1_000_000 + us)
            } else if x.any().is::<DecV>() {
                DuckV::Dec(x.str_())
            } else {
                return Err(exc("NotImplementedError", format!("brevi: Brevi's duckdb cannot bind a {} value", x.tname())));
            }
        }
        _ => return Err(exc("NotImplementedError", format!("brevi: Brevi's duckdb cannot bind a {} value", tn(v)))),
    })
}

fn duck_cols(cs: Vec<DuckCol>) -> V {
    list(cs.into_iter().map(|c| tup(vec![st(c.name), st(c.ty), st(c.render), V::Bool(c.right)])).collect())
}

pub fn brevi_duckdb_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    let o = duck_ops()?;
    let i = |k: usize| db_i(&arg(k));
    match n {
        "load" => (o.load)().map(st).map_err(|m| exc("ImportError", m)),
        // open(path, [(key, value), ...])
        "open" => {
            let mut cfg = vec![];
            for p in to_vec(&arg(1))? {
                let kv = to_vec(&p)?;
                cfg.push((net_s(&kv[0]), net_s(&kv[1])));
            }
            let (db, con) = (o.open)(&net_s(&arg(0)), &cfg).map_err(duck_err)?;
            Ok(tup(vec![V::Int(db), V::Int(con)]))
        }
        "connect" => Ok(V::Int((o.connect)(i(0)).map_err(duck_err)?)),
        "disconnect" => {
            (o.disconnect)(i(0));
            Ok(V::None)
        }
        "close" => {
            (o.close)(i(0));
            Ok(V::None)
        }
        // query(con, sql, params): params a list / tuple (positional), a dict (named) or None
        "query" => {
            let (mut pos, mut named) = (vec![], vec![]);
            let given = !matches!(arg(2), V::None);
            match arg(2) {
                V::None => {}
                V::Dict(d) => {
                    for (k, x) in d.borrow().items() {
                        named.push((net_s(&k), py_duck(&x)?));
                    }
                }
                p => {
                    for x in to_vec(&p)? {
                        pos.push(py_duck(&x)?);
                    }
                }
            }
            Ok(V::Int((o.query)(i(0), &net_s(&arg(1)), &pos, &named, given).map_err(duck_err)?))
        }
        // many(con, sql, [params, ...]): (result, rows changed) or None
        "many" => {
            let mut sets = vec![];
            for p in to_vec(&arg(2))? {
                let (mut pos, mut named) = (vec![], vec![]);
                let given = !matches!(p, V::None);
                match &p {
                    V::None => {}
                    V::Dict(d) => {
                        for (k, x) in d.borrow().items() {
                            named.push((net_s(&k), py_duck(&x)?));
                        }
                    }
                    p => {
                        for x in to_vec(p)? {
                            pos.push(py_duck(&x)?);
                        }
                    }
                }
                sets.push((pos, named, given));
            }
            match (o.many)(i(0), &net_s(&arg(1)), &sets).map_err(duck_err)? {
                Some((r, n)) => Ok(tup(vec![V::Int(r), V::Int(n)])),
                None => Ok(V::None),
            }
        }
        "describe" => {
            let (cs, kind) = (o.describe)(i(0), &net_s(&arg(1))).map_err(duck_err)?;
            Ok(tup(vec![duck_cols(cs), V::Int(kind)]))
        }
        "cols" => Ok(duck_cols((o.cols)(i(0)))),
        "info" => {
            let (r, c, k) = (o.info)(i(0));
            Ok(tup(vec![V::Int(r), V::Int(c), V::Int(k)]))
        }
        "fetch" => {
            let rows = (o.fetch)(i(0), i(1)).map_err(duck_err)?;
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                let mut t = Vec::with_capacity(r.len());
                for x in r {
                    t.push(duck_py(x)?);
                }
                out.push(tup(t));
            }
            Ok(list(out))
        }
        "free" => {
            (o.free)(i(0));
            Ok(V::None)
        }
        "interrupt" => {
            (o.interrupt)(i(0));
            Ok(V::None)
        }
        "term" => Ok(V::Int((o.term)())),
        _ => Err(unsupported(&format!("_brevi_duckdb.{n}"))),
    }
}
