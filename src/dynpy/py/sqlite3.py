# Brevi's sqlite3: CPython 3.13's sqlite3 module (Modules/_sqlite, Lib/sqlite3/dbapi2.py and dump.py) written
# in Python over the system's SQLite engine (_brevi_sqlite, src/dynpy/rt_db.rs and sqlglue.rs). What it lacks
# (user functions, authorizers, backup, blobs, serialize, extensions) is in caps.rs SQLITE3_LACKS: a program using
# it runs CPython's sqlite3.

# sqlite3's constants (CPython 3.13)
SQLITE_ABORT = 4
SQLITE_AUTH = 23
SQLITE_BUSY = 5
SQLITE_CANTOPEN = 14
SQLITE_CONSTRAINT = 19
SQLITE_CORRUPT = 11
SQLITE_DONE = 101
SQLITE_EMPTY = 16
SQLITE_ERROR = 1
SQLITE_FORMAT = 24
SQLITE_FULL = 13
SQLITE_INTERNAL = 2
SQLITE_INTERRUPT = 9
SQLITE_IOERR = 10
SQLITE_LOCKED = 6
SQLITE_MISMATCH = 20
SQLITE_MISUSE = 21
SQLITE_NOLFS = 22
SQLITE_NOMEM = 7
SQLITE_NOTADB = 26
SQLITE_NOTFOUND = 12
SQLITE_OK = 0
SQLITE_PERM = 3
SQLITE_PROTOCOL = 15
SQLITE_RANGE = 25
SQLITE_READONLY = 8
SQLITE_ROW = 100
SQLITE_SCHEMA = 17
SQLITE_TOOBIG = 18
SQLITE_NOTICE = 27
SQLITE_WARNING = 28
SQLITE_ABORT_ROLLBACK = 516
SQLITE_BUSY_RECOVERY = 261
SQLITE_CANTOPEN_FULLPATH = 782
SQLITE_CANTOPEN_ISDIR = 526
SQLITE_CANTOPEN_NOTEMPDIR = 270
SQLITE_CORRUPT_VTAB = 267
SQLITE_IOERR_ACCESS = 3338
SQLITE_IOERR_BLOCKED = 2826
SQLITE_IOERR_CHECKRESERVEDLOCK = 3594
SQLITE_IOERR_CLOSE = 4106
SQLITE_IOERR_DELETE = 2570
SQLITE_IOERR_DELETE_NOENT = 5898
SQLITE_IOERR_DIR_CLOSE = 4362
SQLITE_IOERR_DIR_FSYNC = 1290
SQLITE_IOERR_FSTAT = 1802
SQLITE_IOERR_FSYNC = 1034
SQLITE_IOERR_LOCK = 3850
SQLITE_IOERR_NOMEM = 3082
SQLITE_IOERR_RDLOCK = 2314
SQLITE_IOERR_READ = 266
SQLITE_IOERR_SEEK = 5642
SQLITE_IOERR_SHMLOCK = 5130
SQLITE_IOERR_SHMMAP = 5386
SQLITE_IOERR_SHMOPEN = 4618
SQLITE_IOERR_SHMSIZE = 4874
SQLITE_IOERR_SHORT_READ = 522
SQLITE_IOERR_TRUNCATE = 1546
SQLITE_IOERR_UNLOCK = 2058
SQLITE_IOERR_WRITE = 778
SQLITE_LOCKED_SHAREDCACHE = 262
SQLITE_READONLY_CANTLOCK = 520
SQLITE_READONLY_RECOVERY = 264
SQLITE_CONSTRAINT_CHECK = 275
SQLITE_CONSTRAINT_COMMITHOOK = 531
SQLITE_CONSTRAINT_FOREIGNKEY = 787
SQLITE_CONSTRAINT_FUNCTION = 1043
SQLITE_CONSTRAINT_NOTNULL = 1299
SQLITE_CONSTRAINT_PRIMARYKEY = 1555
SQLITE_CONSTRAINT_TRIGGER = 1811
SQLITE_CONSTRAINT_UNIQUE = 2067
SQLITE_CONSTRAINT_VTAB = 2323
SQLITE_READONLY_ROLLBACK = 776
SQLITE_IOERR_MMAP = 6154
SQLITE_NOTICE_RECOVER_ROLLBACK = 539
SQLITE_NOTICE_RECOVER_WAL = 283
SQLITE_BUSY_SNAPSHOT = 517
SQLITE_IOERR_GETTEMPPATH = 6410
SQLITE_WARNING_AUTOINDEX = 284
SQLITE_CANTOPEN_CONVPATH = 1038
SQLITE_IOERR_CONVPATH = 6666
SQLITE_CONSTRAINT_ROWID = 2579
SQLITE_READONLY_DBMOVED = 1032
SQLITE_AUTH_USER = 279
SQLITE_IOERR_VNODE = 6922
SQLITE_IOERR_AUTH = 7178
SQLITE_OK_LOAD_PERMANENTLY = 256
SQLITE_IOERR_BEGIN_ATOMIC = 7434
SQLITE_IOERR_COMMIT_ATOMIC = 7690
SQLITE_IOERR_ROLLBACK_ATOMIC = 7946
SQLITE_ERROR_MISSING_COLLSEQ = 257
SQLITE_ERROR_RETRY = 513
SQLITE_READONLY_CANTINIT = 1288
SQLITE_READONLY_DIRECTORY = 1544
SQLITE_CORRUPT_SEQUENCE = 523
SQLITE_LOCKED_VTAB = 518
SQLITE_CANTOPEN_DIRTYWAL = 1294
SQLITE_ERROR_SNAPSHOT = 769
SQLITE_CANTOPEN_SYMLINK = 1550
SQLITE_CONSTRAINT_PINNED = 2835
SQLITE_OK_SYMLINK = 512
SQLITE_BUSY_TIMEOUT = 773
SQLITE_CORRUPT_INDEX = 779
SQLITE_IOERR_DATA = 8202
SQLITE_IOERR_CORRUPTFS = 8458
SQLITE_DENY = 1
SQLITE_IGNORE = 2
SQLITE_CREATE_INDEX = 1
SQLITE_CREATE_TABLE = 2
SQLITE_CREATE_TEMP_INDEX = 3
SQLITE_CREATE_TEMP_TABLE = 4
SQLITE_CREATE_TEMP_TRIGGER = 5
SQLITE_CREATE_TEMP_VIEW = 6
SQLITE_CREATE_TRIGGER = 7
SQLITE_CREATE_VIEW = 8
SQLITE_DELETE = 9
SQLITE_DROP_INDEX = 10
SQLITE_DROP_TABLE = 11
SQLITE_DROP_TEMP_INDEX = 12
SQLITE_DROP_TEMP_TABLE = 13
SQLITE_DROP_TEMP_TRIGGER = 14
SQLITE_DROP_TEMP_VIEW = 15
SQLITE_DROP_TRIGGER = 16
SQLITE_DROP_VIEW = 17
SQLITE_INSERT = 18
SQLITE_PRAGMA = 19
SQLITE_READ = 20
SQLITE_SELECT = 21
SQLITE_TRANSACTION = 22
SQLITE_UPDATE = 23
SQLITE_ATTACH = 24
SQLITE_DETACH = 25
SQLITE_ALTER_TABLE = 26
SQLITE_REINDEX = 27
SQLITE_ANALYZE = 28
SQLITE_CREATE_VTABLE = 29
SQLITE_DROP_VTABLE = 30
SQLITE_FUNCTION = 31
SQLITE_SAVEPOINT = 32
SQLITE_RECURSIVE = 33
SQLITE_LIMIT_LENGTH = 0
SQLITE_LIMIT_SQL_LENGTH = 1
SQLITE_LIMIT_COLUMN = 2
SQLITE_LIMIT_EXPR_DEPTH = 3
SQLITE_LIMIT_COMPOUND_SELECT = 4
SQLITE_LIMIT_VDBE_OP = 5
SQLITE_LIMIT_FUNCTION_ARG = 6
SQLITE_LIMIT_ATTACHED = 7
SQLITE_LIMIT_LIKE_PATTERN_LENGTH = 8
SQLITE_LIMIT_VARIABLE_NUMBER = 9
SQLITE_LIMIT_TRIGGER_DEPTH = 10
SQLITE_LIMIT_WORKER_THREADS = 11
SQLITE_DBCONFIG_ENABLE_FKEY = 1002
SQLITE_DBCONFIG_ENABLE_TRIGGER = 1003
SQLITE_DBCONFIG_ENABLE_FTS3_TOKENIZER = 1004
SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION = 1005
SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE = 1006
SQLITE_DBCONFIG_ENABLE_QPSG = 1007
SQLITE_DBCONFIG_TRIGGER_EQP = 1008
SQLITE_DBCONFIG_RESET_DATABASE = 1009
SQLITE_DBCONFIG_DEFENSIVE = 1010
SQLITE_DBCONFIG_WRITABLE_SCHEMA = 1011
SQLITE_DBCONFIG_DQS_DDL = 1014
SQLITE_DBCONFIG_DQS_DML = 1013
SQLITE_DBCONFIG_LEGACY_ALTER_TABLE = 1012
SQLITE_DBCONFIG_ENABLE_VIEW = 1015
SQLITE_DBCONFIG_LEGACY_FILE_FORMAT = 1016
SQLITE_DBCONFIG_TRUSTED_SCHEMA = 1017

_ERRNAMES = {
    4: 'SQLITE_ABORT',
    23: 'SQLITE_AUTH',
    5: 'SQLITE_BUSY',
    14: 'SQLITE_CANTOPEN',
    19: 'SQLITE_CONSTRAINT',
    11: 'SQLITE_CORRUPT',
    101: 'SQLITE_DONE',
    16: 'SQLITE_EMPTY',
    1: 'SQLITE_ERROR',
    24: 'SQLITE_FORMAT',
    13: 'SQLITE_FULL',
    2: 'SQLITE_INTERNAL',
    9: 'SQLITE_INTERRUPT',
    10: 'SQLITE_IOERR',
    6: 'SQLITE_LOCKED',
    20: 'SQLITE_MISMATCH',
    21: 'SQLITE_MISUSE',
    22: 'SQLITE_NOLFS',
    7: 'SQLITE_NOMEM',
    26: 'SQLITE_NOTADB',
    12: 'SQLITE_NOTFOUND',
    0: 'SQLITE_OK',
    3: 'SQLITE_PERM',
    15: 'SQLITE_PROTOCOL',
    25: 'SQLITE_RANGE',
    8: 'SQLITE_READONLY',
    100: 'SQLITE_ROW',
    17: 'SQLITE_SCHEMA',
    18: 'SQLITE_TOOBIG',
    27: 'SQLITE_NOTICE',
    28: 'SQLITE_WARNING',
    516: 'SQLITE_ABORT_ROLLBACK',
    261: 'SQLITE_BUSY_RECOVERY',
    782: 'SQLITE_CANTOPEN_FULLPATH',
    526: 'SQLITE_CANTOPEN_ISDIR',
    270: 'SQLITE_CANTOPEN_NOTEMPDIR',
    267: 'SQLITE_CORRUPT_VTAB',
    3338: 'SQLITE_IOERR_ACCESS',
    2826: 'SQLITE_IOERR_BLOCKED',
    3594: 'SQLITE_IOERR_CHECKRESERVEDLOCK',
    4106: 'SQLITE_IOERR_CLOSE',
    2570: 'SQLITE_IOERR_DELETE',
    5898: 'SQLITE_IOERR_DELETE_NOENT',
    4362: 'SQLITE_IOERR_DIR_CLOSE',
    1290: 'SQLITE_IOERR_DIR_FSYNC',
    1802: 'SQLITE_IOERR_FSTAT',
    1034: 'SQLITE_IOERR_FSYNC',
    3850: 'SQLITE_IOERR_LOCK',
    3082: 'SQLITE_IOERR_NOMEM',
    2314: 'SQLITE_IOERR_RDLOCK',
    266: 'SQLITE_IOERR_READ',
    5642: 'SQLITE_IOERR_SEEK',
    5130: 'SQLITE_IOERR_SHMLOCK',
    5386: 'SQLITE_IOERR_SHMMAP',
    4618: 'SQLITE_IOERR_SHMOPEN',
    4874: 'SQLITE_IOERR_SHMSIZE',
    522: 'SQLITE_IOERR_SHORT_READ',
    1546: 'SQLITE_IOERR_TRUNCATE',
    2058: 'SQLITE_IOERR_UNLOCK',
    778: 'SQLITE_IOERR_WRITE',
    262: 'SQLITE_LOCKED_SHAREDCACHE',
    520: 'SQLITE_READONLY_CANTLOCK',
    264: 'SQLITE_READONLY_RECOVERY',
    275: 'SQLITE_CONSTRAINT_CHECK',
    531: 'SQLITE_CONSTRAINT_COMMITHOOK',
    787: 'SQLITE_CONSTRAINT_FOREIGNKEY',
    1043: 'SQLITE_CONSTRAINT_FUNCTION',
    1299: 'SQLITE_CONSTRAINT_NOTNULL',
    1555: 'SQLITE_CONSTRAINT_PRIMARYKEY',
    1811: 'SQLITE_CONSTRAINT_TRIGGER',
    2067: 'SQLITE_CONSTRAINT_UNIQUE',
    2323: 'SQLITE_CONSTRAINT_VTAB',
    776: 'SQLITE_READONLY_ROLLBACK',
    6154: 'SQLITE_IOERR_MMAP',
    539: 'SQLITE_NOTICE_RECOVER_ROLLBACK',
    283: 'SQLITE_NOTICE_RECOVER_WAL',
    517: 'SQLITE_BUSY_SNAPSHOT',
    6410: 'SQLITE_IOERR_GETTEMPPATH',
    284: 'SQLITE_WARNING_AUTOINDEX',
    1038: 'SQLITE_CANTOPEN_CONVPATH',
    6666: 'SQLITE_IOERR_CONVPATH',
    2579: 'SQLITE_CONSTRAINT_ROWID',
    1032: 'SQLITE_READONLY_DBMOVED',
    279: 'SQLITE_AUTH_USER',
    6922: 'SQLITE_IOERR_VNODE',
    7178: 'SQLITE_IOERR_AUTH',
    256: 'SQLITE_OK_LOAD_PERMANENTLY',
    7434: 'SQLITE_IOERR_BEGIN_ATOMIC',
    7690: 'SQLITE_IOERR_COMMIT_ATOMIC',
    7946: 'SQLITE_IOERR_ROLLBACK_ATOMIC',
    257: 'SQLITE_ERROR_MISSING_COLLSEQ',
    513: 'SQLITE_ERROR_RETRY',
    1288: 'SQLITE_READONLY_CANTINIT',
    1544: 'SQLITE_READONLY_DIRECTORY',
    523: 'SQLITE_CORRUPT_SEQUENCE',
    518: 'SQLITE_LOCKED_VTAB',
    1294: 'SQLITE_CANTOPEN_DIRTYWAL',
    769: 'SQLITE_ERROR_SNAPSHOT',
    1550: 'SQLITE_CANTOPEN_SYMLINK',
    2835: 'SQLITE_CONSTRAINT_PINNED',
    512: 'SQLITE_OK_SYMLINK',
    773: 'SQLITE_BUSY_TIMEOUT',
    779: 'SQLITE_CORRUPT_INDEX',
    8202: 'SQLITE_IOERR_DATA',
    8458: 'SQLITE_IOERR_CORRUPTFS',
}

import datetime
import _brevi_sqlite as _s

sqlite_version = _s.load()
sqlite_version_info = tuple(int(x) for x in sqlite_version.split("."))
apilevel = "2.0"
paramstyle = "qmark"
threadsafety = 3
PARSE_DECLTYPES = 1
PARSE_COLNAMES = 2
LEGACY_TRANSACTION_CONTROL = -1

Date = datetime.date
Time = datetime.time
Timestamp = datetime.datetime
Binary = bytes


class Warning(Exception):
    pass


class Error(Exception):
    sqlite_errorcode = None
    sqlite_errorname = None


class InterfaceError(Error):
    pass


class DatabaseError(Error):
    pass


class InternalError(DatabaseError):
    pass


class OperationalError(DatabaseError):
    pass


class ProgrammingError(DatabaseError):
    pass


class IntegrityError(DatabaseError):
    pass


class DataError(DatabaseError):
    pass


class NotSupportedError(DatabaseError):
    pass


class PrepareProtocol:
    pass


adapters = {}
converters = {}
# an adapter for int, float or str: those values are adapted too (CPython's BaseTypeAdapted)
_base_adapted = False


def register_adapter(type, adapter):
    global _base_adapted
    if type is int or type is float or type is str:
        _base_adapted = True
    adapters[(type, PrepareProtocol)] = adapter


def register_converter(typename, converter):
    if not isinstance(typename, str):
        raise TypeError("register_converter() argument 1 must be str, not " + type(typename).__name__)
    converters[typename.upper()] = converter


def adapt(obj, proto=PrepareProtocol, alt=None):
    a = adapters.get((type(obj), proto))
    if a is not None:
        return a(obj)
    if hasattr(obj, "__conform__"):
        r = obj.__conform__(proto)
        if r is not None:
            return r
    if alt is not None:
        return alt
    raise ProgrammingError("can't adapt")


def complete_statement(statement):
    if not isinstance(statement, str):
        raise TypeError("complete_statement() argument 'statement' must be str, not " + type(statement).__name__)
    return _s.complete(statement)


def enable_callback_tracebacks(enable):
    pass


# the exception for an SQLite result code, with sqlite_errorcode / sqlite_errorname as CPython sets them
def _error(code, msg):
    primary = code & 0xFF
    if primary in (SQLITE_INTERNAL, SQLITE_NOTFOUND):
        cls = InternalError
    elif primary == SQLITE_NOMEM:
        return MemoryError()
    elif primary in (SQLITE_ERROR, SQLITE_PERM, SQLITE_ABORT, SQLITE_BUSY, SQLITE_LOCKED, SQLITE_READONLY, SQLITE_INTERRUPT,
                     SQLITE_IOERR, SQLITE_FULL, SQLITE_CANTOPEN, SQLITE_PROTOCOL, SQLITE_EMPTY, SQLITE_SCHEMA):
        cls = OperationalError
    elif primary == SQLITE_CORRUPT:
        cls = DatabaseError
    elif primary == SQLITE_TOOBIG:
        cls = DataError
    elif primary in (SQLITE_CONSTRAINT, SQLITE_MISMATCH):
        cls = IntegrityError
    elif primary in (SQLITE_MISUSE, SQLITE_RANGE):
        cls = InterfaceError
    else:
        cls = DatabaseError
    e = cls(msg)
    e.sqlite_errorcode = code
    e.sqlite_errorname = _ERRNAMES.get(code, "unknown")
    return e


def _db_error(db):
    return _error(_s.errcode(db), _s.errmsg(db))


_DML = ("insert", "update", "delete", "replace")


# the SQL after leading white space and comments, as CPython's statement.c skips them
def _lstrip_sql(sql):
    i = 0
    n = len(sql)
    while i < n:
        c = sql[i]
        if c in " \t\f\n\r":
            i += 1
        elif c == "-" and sql[i + 1:i + 2] == "-":
            j = sql.find("\n", i)
            if j < 0:
                return ""
            i = j + 1
        elif c == "/" and sql[i + 1:i + 2] == "*":
            j = sql.find("*/", i + 2)
            if j < 0:
                return ""
            i = j + 2
        else:
            return sql[i:]
    return ""


def _is_dml(sql):
    s = _lstrip_sql(sql)[:7].lower()
    for k in _DML:
        if s.startswith(k):
            return True
    return False


class _Statement:
    def __init__(self, conn, sql):
        rc, st, used = _s.prepare(conn._db, sql)
        if rc != SQLITE_OK:
            raise _db_error(conn._db)
        self.st = st
        self.is_dml = _is_dml(sql)
        self.cached = False
        self.in_use = False
        if st and _lstrip_sql(sql[used:]):
            _s.finalize(st)
            self.st = 0
            raise ProgrammingError("You can only execute one statement at a time.")

    def reset(self):
        if self.st:
            _s.reset(self.st)

    # done with by a cursor: back to the connection's cache (reset), or gone
    def release(self):
        self.in_use = False
        if self.cached:
            if self.st:
                _s.reset(self.st)
        else:
            self.finalize()

    def finalize(self):
        if self.st:
            _s.finalize(self.st)
            self.st = 0


def _bind_one(st, i, v):
    t = type(v)
    if _base_adapted or (t is not int and t is not float and t is not str and t is not bytes and v is not None):
        a = adapters.get((t, PrepareProtocol))
        if a is not None:
            v = a(v)
        elif hasattr(v, "__conform__"):
            r = v.__conform__(PrepareProtocol)
            if r is not None:
                v = r
    if v is None:
        rc = _s.bind(st, i, None, 0)
    elif isinstance(v, int):
        if v > 9223372036854775807 or v < -9223372036854775808:
            raise OverflowError("Python int too large to convert to SQLite INTEGER")
        rc = _s.bind(st, i, int(v), 1)
    elif isinstance(v, float):
        rc = _s.bind(st, i, float(v), 2)
    elif isinstance(v, str):
        rc = _s.bind(st, i, str(v), 3)
    elif isinstance(v, bytes):
        rc = _s.bind(st, i, v, 4)
    else:
        raise ProgrammingError("Error binding parameter " + str(i) + ": type '" + type(v).__name__ + "' is not supported")
    if rc != SQLITE_OK:
        raise InterfaceError("Error binding parameter " + str(i) + " - probably unsupported type.")


def _bind(st, params):
    n = _s.nparams(st)
    if isinstance(params, (tuple, list)) or (not isinstance(params, dict) and isinstance(params, str)):
        if len(params) != n:
            raise ProgrammingError("Incorrect number of bindings supplied. The current statement uses " + str(n) + ", and there are " + str(len(params)) + " supplied.")
        for i in range(n):
            _bind_one(st, i + 1, params[i])
    elif isinstance(params, dict):
        for i in range(1, n + 1):
            name = _s.param_name(st, i)
            if name is None:
                raise ProgrammingError("Binding " + str(i) + " has no name, but you supplied a dictionary (which has only names).")
            key = name[1:]
            if type(params) is dict:
                if key not in params:
                    raise ProgrammingError("You did not supply a value for binding parameter :" + key + ".")
                v = params[key]
            else:
                try:
                    v = params[key]
                except KeyError:
                    raise ProgrammingError("You did not supply a value for binding parameter :" + key + ".")
            _bind_one(st, i, v)
    elif hasattr(params, "__len__") and hasattr(params, "__getitem__"):
        if len(params) != n:
            raise ProgrammingError("Incorrect number of bindings supplied. The current statement uses " + str(n) + ", and there are " + str(len(params)) + " supplied.")
        for i in range(n):
            _bind_one(st, i + 1, params[i])
    else:
        raise ProgrammingError("parameters are of unsupported type")


def _decltype_converter(decl):
    if decl is None:
        return None
    k = 0
    while k < len(decl) and decl[k] != " " and decl[k] != "(":
        k += 1
    return converters.get(decl[:k].upper())


def _colname_type(name):
    a = name.find("[")
    if a < 0:
        return None
    b = name.find("]", a + 1)
    if b < 0:
        return None
    return name[a + 1:b]


def _strip_colname(name):
    a = name.find("[")
    if a < 0:
        return name
    if a > 0 and name[a - 1] == " ":
        a -= 1
    return name[:a]


class Cursor:
    def __init__(self, connection):
        if not isinstance(connection, Connection):
            raise TypeError("Cursor() argument 1 must be sqlite3.Connection, not " + type(connection).__name__)
        self.connection = connection
        self.arraysize = 1
        self.description = None
        self.lastrowid = None
        self.rowcount = -1
        self.row_factory = connection.row_factory
        self._stmt = None
        self._closed = False
        self._conv = None

    def _check(self):
        if self._closed:
            raise ProgrammingError("Cannot operate on a closed cursor.")
        self.connection._check()

    def _query(self, multiple, sql, params):
        self._check()
        if self._stmt is not None:
            self._stmt.release()
            self._stmt = None
        con = self.connection
        stmt = con._statement(sql)
        if multiple and not stmt.is_dml:
            stmt.release()
            raise ProgrammingError("executemany() can only execute DML statements.")
        if con._autocommit == LEGACY_TRANSACTION_CONTROL and con._isolation is not None and stmt.is_dml and _s.db_int(con._db, 3):
            con._exec("BEGIN " + con._isolation)
        self.description = None
        self.rowcount = 0 if stmt.is_dml else -1
        self._conv = None
        if not stmt.st:
            return self
        plist = params if multiple else [params]
        try:
            for p in plist:
                rc = -1 if _base_adapted else _s.run(stmt.st, p)
                if rc == -1:
                    _s.reset(stmt.st)
                    _bind(stmt.st, p)
                    rc = _s.step(stmt.st)
                if rc != SQLITE_ROW and rc != SQLITE_DONE:
                    _s.reset(stmt.st)
                    raise _db_error(con._db)
                nc = _s.ncols(stmt.st)
                if self.description is None and nc > 0:
                    cols = []
                    conv = []
                    any_conv = False
                    for k in range(nc):
                        name = _s.col_name(stmt.st, k)
                        c = None
                        if con._detect & PARSE_COLNAMES:
                            t = _colname_type(name)
                            if t is not None:
                                c = converters.get(t.upper())
                            name = _strip_colname(name)
                        if c is None and con._detect & PARSE_DECLTYPES:
                            c = _decltype_converter(_s.col_decl(stmt.st, k))
                        if c is not None:
                            any_conv = True
                        conv.append(c)
                        cols.append((name, None, None, None, None, None, None))
                    self.description = tuple(cols)
                    self._conv = conv if any_conv else None
                if rc == SQLITE_DONE:
                    if stmt.is_dml:
                        self.rowcount += _s.db_int(con._db, 0)
                    _s.reset(stmt.st)
                elif multiple:
                    raise ProgrammingError("executemany() can only execute DML statements.")
                if not multiple:
                    self.lastrowid = _s.db_int(con._db, 2)
                if rc == SQLITE_ROW:
                    self._stmt = stmt
                    stmt = None
        finally:
            if stmt is not None:
                stmt.release()
        return self

    def execute(self, sql, parameters=()):
        if not isinstance(sql, str):
            raise TypeError("execute() argument 1 must be str, not " + type(sql).__name__)
        return self._query(False, sql, parameters)

    def executemany(self, sql, seq_of_parameters):
        if not isinstance(sql, str):
            raise TypeError("executemany() argument 1 must be str, not " + type(sql).__name__)
        return self._query(True, sql, seq_of_parameters)

    def executescript(self, sql_script):
        if not isinstance(sql_script, str):
            raise ValueError("script argument must be unicode.")
        self._check()
        con = self.connection
        if con._autocommit == LEGACY_TRANSACTION_CONTROL and not _s.db_int(con._db, 3):
            con._exec("COMMIT")
        sql = sql_script
        while True:
            rc, st, used = _s.prepare(con._db, sql)
            if rc != SQLITE_OK:
                raise _db_error(con._db)
            if st:
                rc = _s.step(st)
                while rc == SQLITE_ROW:
                    rc = _s.step(st)
                if rc != SQLITE_DONE:
                    _s.finalize(st)
                    raise _db_error(con._db)
                _s.finalize(st)
            sql = sql[used:]
            if not _lstrip_sql(sql) or used == 0:
                break
        return self

    def _row(self):
        st = self._stmt.st
        con = self.connection
        tf = con.text_factory
        if self._conv is not None:
            vals = []
            for k in range(len(self._conv)):
                c = self._conv[k]
                kind, v = _s.col(st, k)
                if c is not None:
                    vals.append(None if kind == 5 else c(v if kind != 1 and kind != 2 else str(v).encode()))
                elif kind == 3:
                    vals.append(self._text(v, tf, k))
                else:
                    vals.append(v)
            return tuple(vals)
        if tf is str:
            vals, bad = _s.row(st, 1)
            if bad >= 0:
                name = _s.col_name(st, bad)
                raise OperationalError("Could not decode to UTF-8 column '" + name + "' with text '" + vals[bad].decode("utf-8", "replace") + "'")
            return tuple(vals)
        vals, bad = _s.row(st, 0)
        if tf is bytes:
            return tuple(vals)
        nc = len(vals)
        out = []
        for k in range(nc):
            kind, v = _s.col(st, k)
            out.append(self._text(v, tf, k) if kind == 3 else vals[k])
        return tuple(out)

    def _text(self, b, tf, k):
        if tf is str:
            try:
                return b.decode("utf-8")
            except UnicodeDecodeError:
                raise OperationalError("Could not decode to UTF-8 column '" + _s.col_name(self._stmt.st, k) + "' with text '" + b.decode("utf-8", "replace") + "'")
        if tf is bytes:
            return b
        return tf(b)

    def __iter__(self):
        return self

    def __next__(self):
        if self._closed or not self.connection._db:
            self._check()
        stmt = self._stmt
        if stmt is None:
            raise StopIteration
        if self._conv is None and self.connection.text_factory is str:
            row, bad, rc = _s.fetch(stmt.st, 1)
            if bad >= 0:
                raise OperationalError("Could not decode to UTF-8 column '" + _s.col_name(stmt.st, bad) + "' with text '" + row[bad].decode("utf-8", "replace") + "'")
        else:
            row = self._row()
            rc = _s.step(stmt.st)
        if rc == SQLITE_DONE:
            if stmt.is_dml:
                self.rowcount = _s.db_int(self.connection._db, 0)
            stmt.release()
            self._stmt = None
        elif rc != SQLITE_ROW:
            e = _db_error(self.connection._db)
            stmt.release()
            self._stmt = None
            raise e
        if self.row_factory is not None:
            return self.row_factory(self, row)
        return row

    def fetchone(self):
        try:
            return self.__next__()
        except StopIteration:
            return None

    # rows read by the engine in one go, when nothing of Python's is called per row
    def _fast_rows(self, max):
        stmt = self._stmt
        if stmt is None or self._closed or not self.connection._db or self._conv is not None or self.connection.text_factory is not str:
            return None
        rows, bad, rc = _s.fetch_all(stmt.st, 1, max)
        if rc == SQLITE_DONE:
            if stmt.is_dml:
                self.rowcount = _s.db_int(self.connection._db, 0)
            stmt.release()
            self._stmt = None
        elif rc != SQLITE_ROW:
            e = _db_error(self.connection._db)
            stmt.release()
            self._stmt = None
            raise e
        elif bad >= 0:
            self.__next__()
        rf = self.row_factory
        if rf is not None:
            return [rf(self, r) for r in rows]
        return rows

    def fetchmany(self, size=None):
        if size is None:
            size = self.arraysize
        rows = self._fast_rows(size)
        if rows is not None:
            return rows
        out = []
        while len(out) < size:
            try:
                out.append(self.__next__())
            except StopIteration:
                break
        return out

    def fetchall(self):
        rows = self._fast_rows(-1)
        if rows is not None:
            return rows
        out = []
        while True:
            try:
                out.append(self.__next__())
            except StopIteration:
                break
        return out

    def close(self):
        if not self.connection._closed_flag() and self._stmt is not None:
            self._stmt.release()
        self._stmt = None
        self._closed = True

    def setinputsizes(self, sizes):
        pass

    def setoutputsize(self, size, column=None):
        pass


class Row:
    def __init__(self, cursor, data):
        if not isinstance(cursor, Cursor):
            raise TypeError("Row() argument 1 must be sqlite3.Cursor, not " + type(cursor).__name__)
        if not isinstance(data, tuple):
            raise TypeError("Row() argument 2 must be tuple, not " + type(data).__name__)
        self._d = cursor.description
        self._v = data

    def keys(self):
        if self._d is None:
            return []
        return [c[0] for c in self._d]

    def __getitem__(self, k):
        if isinstance(k, str):
            if self._d is not None:
                kl = k.lower()
                for i in range(len(self._d)):
                    if self._d[i][0].lower() == kl:
                        return self._v[i]
            raise IndexError("No item with that key")
        return self._v[k]

    def __len__(self):
        return len(self._v)

    def __iter__(self):
        return iter(self._v)

    def __eq__(self, other):
        if not isinstance(other, Row):
            return NotImplemented
        return self._d == other._d and self._v == other._v

    def __ne__(self, other):
        if not isinstance(other, Row):
            return NotImplemented
        return not (self._d == other._d and self._v == other._v)

    def __hash__(self):
        return hash(self._d) ^ hash(self._v)


def _quote_name(name):
    return '"{0}"'.format(name.replace('"', '""'))


def _escape_single_quotes(value):
    return value.replace("'", "''")


def _quote_value(value):
    return "'{0}'".format(_escape_single_quotes(value))


# Lib/sqlite3/dump.py
def _iterdump(connection, filter=None):
    writeable_schema = False
    cu = connection.cursor()
    cu.row_factory = None
    violations = cu.execute("PRAGMA foreign_key_check").fetchall()
    if violations:
        yield ('PRAGMA foreign_keys=OFF;')
    yield ('BEGIN TRANSACTION;')
    if filter:
        filter_name_clause = 'AND "name" LIKE ?'
        params = [filter]
    else:
        filter_name_clause = ""
        params = []
    q = """
        SELECT "name", "type", "sql"
        FROM "sqlite_master"
            WHERE "sql" NOT NULL AND
            "type" == 'table'
            """ + filter_name_clause + """
            ORDER BY "name"
        """
    schema_res = cu.execute(q, params)
    sqlite_sequence = []
    for table_name, type_, sql in schema_res.fetchall():
        if table_name == 'sqlite_sequence':
            rows = cu.execute('SELECT * FROM "sqlite_sequence";')
            sqlite_sequence = ['DELETE FROM "sqlite_sequence"']
            for tn, seq_value in rows.fetchall():
                sqlite_sequence.append('INSERT INTO "sqlite_sequence" VALUES(' + _quote_value(tn) + ',' + str(seq_value) + ')')
            continue
        elif table_name == 'sqlite_stat1':
            yield ('ANALYZE "sqlite_master";')
        elif table_name.startswith('sqlite_'):
            continue
        elif sql.startswith('CREATE VIRTUAL TABLE'):
            if not writeable_schema:
                writeable_schema = True
                yield ('PRAGMA writable_schema=ON;')
            yield ("INSERT INTO sqlite_master(type,name,tbl_name,rootpage,sql)"
                   "VALUES('table',{0},{0},0,{1});".format(_quote_value(table_name), _quote_value(sql)))
        else:
            yield ('{0};'.format(sql))
        table_name_ident = _quote_name(table_name)
        res = cu.execute('PRAGMA table_info(' + table_name_ident + ')')
        column_names = [str(table_info[1]) for table_info in res.fetchall()]
        q = "SELECT 'INSERT INTO {0} VALUES('{1}')' FROM {2};".format(
            _escape_single_quotes(table_name_ident),
            "','".join("||quote({0})||".format(_quote_name(col)) for col in column_names),
            table_name_ident,
        )
        query_res = cu.execute(q)
        for row in query_res:
            yield ("{0};".format(row[0]))
    q = """
        SELECT "name", "type", "sql"
        FROM "sqlite_master"
            WHERE "sql" NOT NULL AND
            "type" IN ('index', 'trigger', 'view')
            """ + filter_name_clause + """
        """
    schema_res = cu.execute(q, params)
    for name, type_, sql in schema_res.fetchall():
        yield ('{0};'.format(sql))
    if writeable_schema:
        yield ('PRAGMA writable_schema=OFF;')
    for row in sqlite_sequence:
        yield ('{0};'.format(row))
    yield ('COMMIT;')


_LEVELS = ("", "DEFERRED", "IMMEDIATE", "EXCLUSIVE")


def _lacks(what):
    raise NotImplementedError("brevi: sqlite3 " + what + " is not in Brevi's sqlite3")


class Connection:
    def __init__(self, database, timeout=5.0, detect_types=0, isolation_level="", check_same_thread=True,
                 factory=None, cached_statements=128, uri=False, autocommit=LEGACY_TRANSACTION_CONTROL):
        self._db = 0
        if hasattr(database, "__fspath__"):
            database = database.__fspath__()
        if isinstance(database, bytes):
            database = database.decode("utf-8")
        if not isinstance(database, str):
            raise TypeError("expected str, bytes or " + "os" + ".PathLike object, not " + type(database).__name__)
        if autocommit is not LEGACY_TRANSACTION_CONTROL and autocommit is not True and autocommit is not False:
            if autocommit == LEGACY_TRANSACTION_CONTROL and type(autocommit) is int:
                pass
            else:
                raise ValueError("autocommit must be True, False, or sqlite3.LEGACY_TRANSACTION_CONTROL")
        flags = 0x2 | 0x4
        if uri:
            flags |= 0x40
        rc, db = _s.open(database, flags)
        if rc != SQLITE_OK:
            e = _error(_s.errcode(db), _s.errmsg(db)) if db else _error(rc, _s.errstr(rc))
            if db:
                _s.close(db)
            raise e
        self._db = db
        self._cache = {}
        self._cache_size = cached_statements
        self._detect = detect_types
        self._autocommit = autocommit
        self._isolation = ""
        self.row_factory = None
        self.text_factory = str
        _s.busy_timeout(db, int(timeout * 1000))
        self.isolation_level = isolation_level
        if autocommit is False:
            self._exec("BEGIN")

    # a prepared statement for the SQL: the cached one when no cursor is using it (CPython's LRU of 128)
    def _statement(self, sql):
        st = self._cache.get(sql)
        if st is not None and not st.in_use:
            del self._cache[sql]
            self._cache[sql] = st
            st.in_use = True
            return st
        st = _Statement(self, sql)
        st.in_use = True
        if st.st and self._cache_size > 0 and sql not in self._cache:
            st.cached = True
            self._cache[sql] = st
            if len(self._cache) > self._cache_size:
                old = next(iter(self._cache))
                o = self._cache.pop(old)
                o.cached = False
                if not o.in_use:
                    o.finalize()
        return st

    def _closed_flag(self):
        return not self._db

    def _check(self):
        if not self._db:
            raise ProgrammingError("Cannot operate on a closed database.")

    def _exec(self, sql):
        rc, st, used = _s.prepare(self._db, sql)
        if rc != SQLITE_OK:
            raise _db_error(self._db)
        rc = _s.step(st)
        _s.finalize(st)
        if rc != SQLITE_DONE and rc != SQLITE_ROW:
            raise _db_error(self._db)

    @property
    def isolation_level(self):
        return self._isolation

    @isolation_level.setter
    def isolation_level(self, value):
        if value is None:
            if self._db and not _s.db_int(self._db, 3):
                self._exec("COMMIT")
            self._isolation = None
            return
        if not isinstance(value, str):
            raise TypeError("isolation_level must be str or None")
        if value.upper() not in _LEVELS:
            raise ValueError("isolation_level string must be '', 'DEFERRED', 'IMMEDIATE', or 'EXCLUSIVE'")
        self._isolation = value.upper()

    @property
    def autocommit(self):
        self._check()
        return self._autocommit

    @autocommit.setter
    def autocommit(self, value):
        self._check()
        if value is not True and value is not False and value != LEGACY_TRANSACTION_CONTROL:
            raise ValueError("autocommit must be True, False, or sqlite3.LEGACY_TRANSACTION_CONTROL")
        if value is True and not _s.db_int(self._db, 3):
            self._exec("COMMIT")
        elif value is False and _s.db_int(self._db, 3):
            self._exec("BEGIN")
        self._autocommit = value

    @property
    def in_transaction(self):
        self._check()
        return not _s.db_int(self._db, 3)

    @property
    def total_changes(self):
        self._check()
        return _s.db_int(self._db, 1)

    def cursor(self, factory=Cursor):
        self._check()
        c = factory(self)
        if not isinstance(c, Cursor):
            raise TypeError("factory must return a cursor, not " + type(c).__name__)
        if self.row_factory is not None:
            c.row_factory = self.row_factory
        return c

    def execute(self, sql, parameters=()):
        return self.cursor().execute(sql, parameters)

    def executemany(self, sql, parameters):
        return self.cursor().executemany(sql, parameters)

    def executescript(self, sql_script):
        return self.cursor().executescript(sql_script)

    def commit(self):
        self._check()
        if self._autocommit == LEGACY_TRANSACTION_CONTROL:
            if not _s.db_int(self._db, 3):
                self._exec("COMMIT")
        elif self._autocommit is False:
            self._exec("COMMIT")
            self._exec("BEGIN")

    def rollback(self):
        self._check()
        if self._autocommit == LEGACY_TRANSACTION_CONTROL:
            if not _s.db_int(self._db, 3):
                self._exec("ROLLBACK")
        elif self._autocommit is False:
            self._exec("ROLLBACK")
            self._exec("BEGIN")

    def close(self):
        if not self._db:
            return
        if self._autocommit is False and not _s.db_int(self._db, 3):
            self._exec("ROLLBACK")
        for st in self._cache.values():
            st.cached = False
            st.finalize()
        self._cache = {}
        _s.close(self._db)
        self._db = 0

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, tb):
        if exc_type is None:
            try:
                self.commit()
            except Exception:
                self.rollback()
                raise
        else:
            self.rollback()
        return False

    def iterdump(self, filter=None):
        self._check()
        return _iterdump(self, filter=filter)

    def interrupt(self):
        self._check()
        _s.interrupt(self._db)

    def getlimit(self, category):
        self._check()
        if category < 0 or category > 11:
            raise ProgrammingError("'category' is out of bounds")
        return _s.limit(self._db, category, -1)

    def setlimit(self, category, limit):
        self._check()
        if category < 0 or category > 11:
            raise ProgrammingError("'category' is out of bounds")
        return _s.limit(self._db, category, limit)

    def create_function(self, *a, **k):
        _lacks("create_function")

    def create_aggregate(self, *a, **k):
        _lacks("create_aggregate")

    def create_window_function(self, *a, **k):
        _lacks("create_window_function")

    def create_collation(self, *a, **k):
        _lacks("create_collation")

    def set_authorizer(self, *a, **k):
        _lacks("set_authorizer")

    def set_progress_handler(self, *a, **k):
        _lacks("set_progress_handler")

    def set_trace_callback(self, *a, **k):
        _lacks("set_trace_callback")

    def enable_load_extension(self, *a, **k):
        _lacks("enable_load_extension")

    def load_extension(self, *a, **k):
        _lacks("load_extension")

    def backup(self, *a, **k):
        _lacks("backup")

    def blobopen(self, *a, **k):
        _lacks("blobopen")

    def serialize(self, *a, **k):
        _lacks("serialize")

    def deserialize(self, *a, **k):
        _lacks("deserialize")

    def getconfig(self, *a, **k):
        _lacks("getconfig")

    def setconfig(self, *a, **k):
        _lacks("setconfig")


Connection.Warning = Warning
Connection.Error = Error
Connection.InterfaceError = InterfaceError
Connection.DatabaseError = DatabaseError
Connection.InternalError = InternalError
Connection.OperationalError = OperationalError
Connection.ProgrammingError = ProgrammingError
Connection.IntegrityError = IntegrityError
Connection.DataError = DataError
Connection.NotSupportedError = NotSupportedError


def connect(database, timeout=5.0, detect_types=0, isolation_level="", check_same_thread=True, factory=Connection,
            cached_statements=128, uri=False, autocommit=LEGACY_TRANSACTION_CONTROL):
    return factory(database, timeout, detect_types, isolation_level, check_same_thread, None, cached_statements, uri,
                   autocommit=autocommit)


# Lib/sqlite3/dbapi2.py: the default adapters and converters
def _adapt_date(val):
    return val.isoformat()


def _adapt_datetime(val):
    return val.isoformat(" ")


def _convert_date(val):
    return datetime.date(*map(int, val.split(b"-")))


def _convert_timestamp(val):
    datepart, timepart = val.split(b" ")
    year, month, day = map(int, datepart.split(b"-"))
    timepart_full = timepart.split(b".")
    hours, minutes, seconds = map(int, timepart_full[0].split(b":"))
    if len(timepart_full) == 2:
        microseconds = int('{:0<6.6}'.format(timepart_full[1].decode()))
    else:
        microseconds = 0
    return datetime.datetime(year, month, day, hours, minutes, seconds, microseconds)


register_adapter(datetime.date, _adapt_date)
register_adapter(datetime.datetime, _adapt_datetime)
register_converter("date", _convert_date)
register_converter("timestamp", _convert_timestamp)
