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
                V::Tuple(t) => t.as_ref().clone(),
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
