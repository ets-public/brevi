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
    pub bind: fn(i64, i64, &DbV) -> i64,
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

fn db_bytes(v: &V) -> Vec<u8> {
    match v {
        V::Str(x) => x.s.as_bytes().to_vec(),
        _ => bytes_of(v).unwrap_or_default(),
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

pub fn brevi_sqlite_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    let o = sql_ops()?;
    let i = |k: usize| db_i(&arg(k));
    match n {
        "load" => match (o.load)() {
            Ok(v) => Ok(st(v)),
            Err(m) => Err(exc("ImportError", m)),
        },
        "open" => {
            let (rc, db) = (o.open)(&net_s(&arg(0)), i(1));
            Ok(tup(vec![V::Int(rc), V::Int(db)]))
        }
        "close" => Ok(V::Int((o.close)(i(0)))),
        "errmsg" => Ok(st((o.errmsg)(i(0)))),
        "errcode" => Ok(V::Int((o.errcode)(i(0)))),
        "errstr" => Ok(st((o.errstr)(i(0)))),
        "prepare" => {
            // the offset comes back in characters of the str
            let sql = net_s(&arg(1));
            let (rc, stmt, used) = (o.prepare)(i(0), sql.as_bytes());
            let chars = sql.get(..used.max(0) as usize).map_or(sql.chars().count(), |p| p.chars().count());
            Ok(tup(vec![V::Int(rc), V::Int(stmt), V::Int(chars as i64)]))
        }
        "finalize" => Ok(V::Int((o.finalize)(i(0)))),
        "reset" => Ok(V::Int((o.reset)(i(0)))),
        "clear" => Ok(V::Int((o.clear)(i(0)))),
        "step" => Ok(V::Int((o.step)(i(0)))),
        // bind(stmt, index, value, kind): kind 0 null, 1 int, 2 float, 3 text, 4 blob
        "bind" => {
            let v = arg(2);
            let d = match i(3) {
                1 => DbV::Int(db_i(&v)),
                2 => DbV::Float(match v {
                    V::Float(f) => f,
                    V::Int(x) => x as f64,
                    _ => 0.0,
                }),
                3 => DbV::Text(db_bytes(&v)),
                4 => DbV::Blob(db_bytes(&v)),
                _ => DbV::Null,
            };
            Ok(V::Int((o.bind)(i(0), i(1), &d)))
        }
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
            Ok(tup(vec![list(out), V::Int(bad)]))
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
            Ok(tup(vec![V::Int(k), dbv_py(d, 0).unwrap_or(V::None)]))
        }
        // run(stmt, params): reset, bind a tuple or list of None / int / float / str / bytes, step; the step's
        // result code, or -1 when the parameters need Python (a dict, other types, a count that is off)
        "run" => {
            let stv = i(0);
            let ps = match arg(1) {
                V::Tuple(t) => t.to_vec(),
                V::List(l) => l.borrow().clone(),
                _ => return Ok(V::Int(-1)),
            };
            if ps.len() as i64 != (o.nparams)(stv) {
                return Ok(V::Int(-1));
            }
            let mut ds = Vec::with_capacity(ps.len());
            for v in &ps {
                ds.push(match v {
                    V::None => DbV::Null,
                    V::Int(x) => DbV::Int(*x),
                    V::Float(f) => DbV::Float(*f),
                    V::Str(x) => DbV::Text(x.s.as_bytes().to_vec()),
                    V::Ext(x) if x.any().is::<BytesV>() => DbV::Blob(bytes_of(v).unwrap_or_default()),
                    _ => return Ok(V::Int(-1)),
                });
            }
            (o.reset)(stv);
            for (k, d) in ds.iter().enumerate() {
                if (o.bind)(stv, k as i64 + 1, d) != 0 {
                    return Ok(V::Int(-1));
                }
            }
            Ok(V::Int((o.step)(stv)))
        }
        // fetch(stmt, text): the current row (as row() gives it) and the result code of the next step
        "fetch" => {
            let st_ = i(0);
            let nc = (o.ncols)(st_);
            let mut out = Vec::with_capacity(nc as usize);
            let mut bad = -1;
            for c in 0..nc {
                match dbv_py((o.col)(st_, c), i(1)) {
                    Ok(v) => out.push(v),
                    Err(b) => {
                        if bad < 0 {
                            bad = c;
                        }
                        out.push(bytesv(b));
                    }
                }
            }
            if bad >= 0 {
                return Ok(tup(vec![tup(out), V::Int(bad), V::Int(0)]));
            }
            let rc = (o.step)(st_);
            Ok(tup(vec![tup(out), V::Int(-1), V::Int(rc)]))
        }
        // fetch_all(stmt, text, max): rows as fetch gives them, until the statement is done, a step fails, a text
        // is not UTF-8 (that row is left unread) or max rows (< 0: no limit); (rows, bad column, last result code)
        "fetch_all" => {
            let st_ = i(0);
            let text = i(1);
            let max = i(2);
            let nc = (o.ncols)(st_);
            let mut rows = vec![];
            loop {
                if max >= 0 && rows.len() as i64 >= max {
                    return Ok(tup(vec![list(rows), V::Int(-1), V::Int(100)]));
                }
                let mut out = Vec::with_capacity(nc as usize);
                for c in 0..nc {
                    match dbv_py((o.col)(st_, c), text) {
                        Ok(v) => out.push(v),
                        Err(_) => return Ok(tup(vec![list(rows), V::Int(c), V::Int(100)])),
                    }
                }
                rows.push(tup(out));
                let rc = (o.step)(st_);
                if rc != 100 {
                    return Ok(tup(vec![list(rows), V::Int(-1), V::Int(rc)]));
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
        V::Str(s) => DuckV::Str(s.s.clone()),
        V::List(l) => DuckV::List(l.borrow().iter().map(py_duck).collect::<Result<_, _>>()?),
        V::Tuple(t) => DuckV::List(t.iter().map(py_duck).collect::<Result<_, _>>()?),
        V::Dict(d) => {
            let mut kv = vec![];
            for (k, x) in d.borrow().items() {
                match &k {
                    V::Str(s) => kv.push((s.s.clone(), py_duck(&x)?)),
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
