# Brevi's duckdb: the duckdb 2.0 package's Python API over DuckDB's own engine (its C API, through _brevi_duckdb).
# Connections (execute, fetch*, description, rowcount, executemany, transactions, cursors, the unfinished-result
# rule), the default connection and its module functions, relations (lazy SQL built a step at a time: filter,
# project, order, limit, aggregate, joins, set operations, the aggregate shortcuts), DuckDB's exceptions, types and
# its table printer (box_renderer.cpp, ported). Values come from the engine as Python values: ints, floats,
# Decimal, str, bytes, date / time / datetime / timedelta, lists, tuples (ARRAY), dicts (STRUCT and MAP).
# Not here (caps.rs DUCKDB_LACKS sends a program using them to CPython's duckdb): pandas / numpy / Arrow / polars
# interop, Python UDFs, extensions, filesystems, expressions (duckdb.col and the like), file readers and writers of
# the API (SQL's read_csv etc. work), sql_query, explain, describe, materialize and the rest of the relation API.
import _brevi_duckdb as _d

__version__ = "2.0.0"
apilevel = "2.0"
paramstyle = "qmark"
threadsafety = 1


class Warning(Exception):
    pass


class Error(Exception):
    pass


class DatabaseError(Error):
    pass


class DataError(DatabaseError):
    pass


class IntegrityError(DatabaseError):
    pass


class InternalError(DatabaseError):
    pass


class NotSupportedError(DatabaseError):
    pass


class OperationalError(DatabaseError):
    pass


class ProgrammingError(DatabaseError):
    pass


class DependencyException(DatabaseError):
    pass


class FatalException(DatabaseError):
    pass


class InterruptException(DatabaseError):
    pass


class PermissionException(DatabaseError):
    pass


class SequenceException(DatabaseError):
    pass


class BinderException(ProgrammingError):
    pass


class CatalogException(ProgrammingError):
    pass


class InvalidInputException(ProgrammingError):
    pass


class InvalidTypeException(ProgrammingError):
    pass


class ParserException(ProgrammingError):
    pass


class SyntaxException(ProgrammingError):
    pass


class ConnectionException(OperationalError):
    pass


class IOException(OperationalError):
    pass


class OutOfMemoryException(OperationalError):
    pass


class ResourceInUseException(OperationalError):
    pass


class SerializationException(OperationalError):
    pass


class TransactionException(OperationalError):
    pass


class HTTPException(IOException):
    pass


class ConstraintException(IntegrityError):
    pass


class ConversionException(DataError):
    pass


class OutOfRangeException(DataError):
    pass


class TypeMismatchException(DataError):
    pass


class InternalException(InternalError):
    pass


class NotImplementedException(NotSupportedError):
    pass


# The engine's error text starts with its kind ("Catalog Error: ..."), as the duckdb package raises it.
_KINDS = {
    "Catalog": CatalogException,
    "Parser": ParserException,
    "Binder": BinderException,
    "Conversion": ConversionException,
    "Out of Range": OutOfRangeException,
    "Constraint": ConstraintException,
    "Invalid Input": InvalidInputException,
    "TransactionContext": TransactionException,
    "IO": IOException,
    "Permission": PermissionException,
    "Connection": ConnectionException,
    "Not implemented": NotImplementedException,
    "Invalid type": InvalidTypeException,
    "Mismatch Type": TypeMismatchException,
    "Serialization": SerializationException,
    "INTERRUPT": InterruptException,
    "Dependency": DependencyException,
    "Sequence": SequenceException,
    "Out of Memory": OutOfMemoryException,
    "FATAL": FatalException,
    "INTERNAL": InternalException,
    "Syntax": SyntaxException,
    "HTTP": HTTPException,
    "Resource In Use": ResourceInUseException,
    "Invalid Configuration": InvalidInputException,
    "Missing Extension": IOException,
    "Extension Autoloading": IOException,
}


def _exc(message):
    k = message.find(" Error: ")
    cls = _KINDS.get(message[:k], Error) if k > 0 else Error
    return cls(message)


def _bare(message):
    # an error of a relation's own SQL, without the "LINE 1:" context the duckdb package does not show
    k = message.find("\n\nLINE ")
    return message[:k] if k >= 0 else message


_UNFINISHED = (
    "Resource In Use Error: connection has an unfinished result from execute(); call complete() or abort(), "
    "or fetch until a fetch returns no rows, before other work on this connection, or use a separate connection "
    "from cursor()"
)


class DuckDBPyType:
    def __init__(self, name):
        self._name = name

    def __repr__(self):
        return self._name

    def __str__(self):
        return self._name

    def __eq__(self, other):
        if isinstance(other, DuckDBPyType):
            return self._name == other._name
        if isinstance(other, str):
            return self._name == other.upper() or self._name.lower() == other.lower()
        return False

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._name)

    @property
    def id(self):
        n = self._name
        if n.endswith("]"):
            return "list" if n.endswith("[]") else "array"
        k = n.find("(")
        if k > 0:
            n = n[:k]
        return n.lower()


# ---------------------------------------------------------------- the statements of a script


def _statements(text):
    """The statements of a script, as DuckDB's parser splits them (on ';' outside quotes and comments)."""
    out = []
    start = 0
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == "'" or c == '"':
            j = i + 1
            while j < n:
                if text[j] == c:
                    if j + 1 < n and text[j + 1] == c:
                        j += 2
                        continue
                    break
                j += 1
            i = j + 1
        elif c == "-" and text.startswith("--", i):
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
        elif c == "/" and text.startswith("/*", i):
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
        elif c == "$":
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] == "_"):
                j += 1
            if j < n and text[j] == "$" and not text[i + 1 : j][:1].isdigit():
                tag = text[i : j + 1]
                k = text.find(tag, j + 1)
                i = n if k < 0 else k + len(tag)
            else:
                i = j
        elif c == ";":
            s = text[start:i].strip()
            if s:
                out.append(s)
            start = i + 1
            i += 1
        else:
            i += 1
    s = text[start:].strip()
    if s:
        out.append(s)
    return out


def _check_params(parameters):
    if parameters is not None and not isinstance(parameters, (list, tuple, dict)):
        raise InvalidInputException("Invalid Input Error: Prepared parameters can only be passed as a list or a dictionary")


def _query(h, sql, params):
    try:
        return _d.query(h, sql, params)
    except RuntimeError as e:
        m = str(e)
        if m == "brevi: no statement":
            return None
        raise _exc(m)


def _describe(h, sql):
    try:
        return _d.describe(h, sql)
    except RuntimeError as e:
        m = str(e)
        if m == "brevi: no statement":
            m = "Invalid Input Error: No statements to execute."
        raise _exc(m)


def _fetch(r, n):
    try:
        return _d.fetch(r, n)
    except RuntimeError as e:
        raise _exc(str(e))


def _all_rows(h, sql, params):
    r = _query(h, sql, params)
    if r is None:
        return [], []
    try:
        return _d.cols(r), _fetch(r, -1)
    finally:
        _d.free(r)


# ---------------------------------------------------------------- connections


class _Database:
    def __init__(self, handle, key):
        self.handle = handle
        self.key = key
        self.users = 0


_DATABASES = {}


def _version_loaded():
    try:
        return _d.load()
    except ImportError as e:
        raise ImportError(str(e))


def connect(database=":memory:", read_only=False, config=None):
    _version_loaded()
    if not isinstance(database, str):
        database = str(database)
    path = database
    memory = path == "" or path.startswith(":memory:")
    settings = []
    if read_only:
        settings.append(("access_mode", "READ_ONLY"))
    if config:
        for k in config:
            v = config[k]
            if isinstance(v, bool):
                v = "true" if v else "false"
            settings.append((str(k), str(v)))
    key = None if path in ("", ":memory:") else path
    db = _DATABASES.get(key) if key is not None else None
    if db is None:
        try:
            handle, con = _d.open(path, settings)
        except RuntimeError as e:
            raise _exc(str(e))
        db = _Database(handle, key)
        if key is not None:
            _DATABASES[key] = db
    else:
        try:
            con = _d.connect(db.handle)
        except RuntimeError as e:
            raise _exc(str(e))
    return DuckDBPyConnection(db, con)


class DuckDBPyConnection:
    def __init__(self, db, handle):
        self._db = db
        self._h = handle
        db.users += 1
        self._r = None
        self._busy = False
        self._empty = False
        self._rows = -1
        self._closed = False

    # -- state
    def _check(self):
        if self._closed:
            raise ConnectionException("Connection Error: Connection already closed!")

    def _check_rel(self):
        if self._closed:
            raise ConnectionException("Connection Error: Connection has already been closed")
        if self._busy:
            raise ResourceInUseException(_UNFINISHED)

    def _drop(self):
        if self._r is not None:
            _d.free(self._r)
            self._r = None
        self._busy = False
        self._empty = False

    def _keep(self, r):
        self._r = r
        kind, changed, _ = _d.info(r)
        self._busy = kind == 3
        self._empty = False
        return changed if kind == 1 else -1

    # -- running
    def execute(self, query, parameters=None):
        self._check()
        _check_params(parameters)
        self._drop()
        self._rows = -1
        r = _query(self._h, query, parameters)
        if r is None:
            return None
        self._rows = self._keep(r)
        return self

    def executemany(self, query, parameters=None):
        self._check()
        self._drop()
        self._rows = -1
        if parameters is None:
            parameters = [None]
        sets = list(parameters)
        for p in sets:
            _check_params(p)
        try:
            done = _d.many(self._h, query, sets)
        except RuntimeError as e:
            raise _exc(str(e))
        if done is not None:
            self._keep(done[0])
            self._rows = done[1]
            return self
        total = -1
        for p in sets:
            _check_params(p)
            self._drop()
            r = _query(self._h, query, p)
            if r is None:
                continue
            n = self._keep(r)
            if n >= 0:
                total = n if total < 0 else total + n
        self._rows = total
        return self

    def _result(self):
        self._check()
        if self._r is None:
            raise InvalidInputException("Invalid Input Error: No open result set")
        return self._r

    def fetchone(self):
        r = self._result()
        if self._empty:
            return None
        rows = _fetch(r, 1)
        if not rows:
            self._busy = False
            return None
        return rows[0]

    def fetchmany(self, size=1):
        r = self._result()
        if self._empty:
            return []
        rows = _fetch(r, size)
        if len(rows) < size:
            self._busy = False
        return rows

    def fetchall(self):
        r = self._result()
        if self._empty:
            return []
        rows = _fetch(r, -1)
        self._busy = False
        return rows

    def complete(self):
        self._check()
        self._busy = False
        if self._r is not None:
            self._empty = True
        return None

    def abort(self):
        self._check()
        self._busy = False
        if self._r is not None:
            self._empty = True
        return None

    @property
    def description(self):
        if self._closed or self._r is None:
            return None
        return [(c[0], DuckDBPyType(c[1]), None, None, None, None, None) for c in _d.cols(self._r)]

    @property
    def rowcount(self):
        return self._rows

    # -- transactions
    def begin(self):
        self.execute("BEGIN TRANSACTION")
        return self

    def _end(self, sql):
        # as the duckdb package: nothing to end outside a transaction
        try:
            self.execute(sql)
        except Exception as e:
            if not isinstance(e, TransactionException) or "no transaction is active" not in str(e):
                raise e
        return self

    def commit(self):
        return self._end("COMMIT")

    def rollback(self):
        return self._end("ROLLBACK")

    def checkpoint(self):
        self.execute("CHECKPOINT")
        return self

    def interrupt(self):
        self._check()
        _d.interrupt(self._h)

    # -- connections
    def cursor(self):
        self._check()
        try:
            h = _d.connect(self._db.handle)
        except RuntimeError as e:
            raise _exc(str(e))
        return DuckDBPyConnection(self._db, h)

    def duplicate(self):
        return self.cursor()

    def close(self):
        if self._closed:
            return None
        self._drop()
        _d.disconnect(self._h)
        self._closed = True
        db = self._db
        db.users -= 1
        if db.users == 0:
            _d.close(db.handle)
            if db.key is not None and _DATABASES.get(db.key) is db:
                del _DATABASES[db.key]
        return None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
        return False

    # -- relations
    def sql(self, query, alias="", params=None):
        self._check_rel()
        if params is not None:
            _check_params(params)
        parts = _statements(query)
        if not parts:
            return None
        for s in parts[:-1]:
            r = _query(self._h, s, None)
            if r is not None:
                _d.free(r)
        last = parts[-1]
        try:
            cols, kind = _d.describe(self._h, last)
        except RuntimeError as e:
            m = str(e)
            if m.startswith("Parser Error"):
                raise _exc(m)
            word = last.lstrip("( \t\n").split(None, 1)[0].lower() if last.strip("( \t\n") else ""
            if word in ("select", "with", "from", "values", "table"):
                # a query binds lazily, and its errors show no LINE context
                raise _exc(_bare(m))
            kind = 0
        if kind == 1:
            rel = DuckDBPyRelation(self, last, alias or _unnamed(), params, None, None)
            rel._base = True
            rel._bind(True)
            return rel
        r = _query(self._h, last, params)
        if r is None:
            return None
        try:
            cols = _d.cols(r)
            if not cols:
                return None
            rows = _fetch(r, -1)
        finally:
            _d.free(r)
        return DuckDBPyRelation._hold(self, cols, rows, alias or _unnamed())

    def query(self, query, alias="", params=None):
        return self.sql(query, alias, params)

    def from_query(self, query, alias="", params=None):
        return self.sql(query, alias, params)

    def table(self, table_name):
        self._check_rel()
        return DuckDBPyRelation._table(self, table_name)

    def view(self, view_name):
        return self.table(view_name)

    def values(self, *args):
        self._check_rel()
        if len(args) == 1 and isinstance(args[0], list) and args[0] and isinstance(args[0][0], (list, tuple)):
            rows = args[0]
        elif len(args) == 1 and isinstance(args[0], (list, tuple)):
            rows = [args[0]]
        else:
            rows = [args]
        params = []
        groups = []
        for row in rows:
            groups.append("(" + ", ".join("?" for _ in row) + ")")
            params.extend(row)
        return DuckDBPyRelation._new(self, "SELECT * FROM (VALUES " + ", ".join(groups) + ")", _unnamed(), params)

    def get_table_names(self, query, qualified=False):
        raise NotImplementedError("brevi: Brevi's duckdb has no get_table_names")


_COUNTER = [0]


def _unnamed():
    _COUNTER[0] += 1
    return "unnamed_relation_" + format(0x5D3A9C1E7B20F4A6 + _COUNTER[0] * 0x9E3779B97F4A7C15 & 0xFFFFFFFFFFFFFFFF, "016x")


# ---------------------------------------------------------------- relations


def _plain(name):
    if name and (name[0].isalpha() or name[0] == "_") and all(c.isalnum() or c == "_" for c in name) and name.isascii():
        return name
    return '"' + name.replace('"', '""') + '"'


def _split_list(text):
    """Comma-separated items, not splitting inside parentheses, brackets or quotes."""
    out = []
    depth = 0
    cur = ""
    q = ""
    for c in text:
        if q:
            cur += c
            if c == q:
                q = ""
        elif c == "'" or c == '"':
            q = c
            cur += c
        elif c in "([{":
            depth += 1
            cur += c
        elif c in ")]}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += c
    out.append(cur)
    return [x for x in out if x.strip()] if any(x.strip() for x in out) else []


_JOINS = {
    "inner": ("INNER", True),
    "left": ("LEFT", True),
    "right": ("RIGHT", True),
    "outer": ("FULL OUTER", True),
    "full": ("FULL OUTER", True),
    "semi": ("SEMI", True),
    "anti": ("ANTI", True),
    "cross": ("CROSS", False),
    "positional": ("POSITIONAL", False),
    "natural": ("NATURAL", False),
    "asof": ("ASOF", True),
}

_REMOVED = {
    "fetchone": "rel.limit(1).fetchall()",
    "fetchmany": "rel.limit(n).fetchall()",
    "fetch_df_chunk": "rel.to_arrow_reader() to read in batches, or rel.limit(n).df()",
    "execute": "rel.materialize() to run it once and keep its rows, or a method such as fetchall()",
}


def _removed(name):
    return AttributeError(
        "Relations no longer have " + name + "(): a relation keeps no result between calls, so every call runs it again. Use " + _REMOVED[name] + " instead."
    )


class DuckDBPyRelation:
    def __init__(self, con, sql, alias, params, cols, held):
        self._con = con
        self._sql = sql
        self._alias = alias
        self._params = params
        self._cols = cols
        self._held = held
        # the text the user wrote (a query, a table): wrapped, it starts on line 3, where the duckdb package's
        # rendering puts it, so an error's "LINE 3:" context is the same
        self._base = False

    @staticmethod
    def _new(con, sql, alias, params):
        rel = DuckDBPyRelation(con, sql, alias, params, None, None)
        rel._base = True
        rel._bind(False)
        return rel

    @staticmethod
    def _hold(con, cols, rows, alias):
        return DuckDBPyRelation(con, None, alias, None, cols, rows)

    @staticmethod
    def _table(con, name):
        rel = DuckDBPyRelation(con, "SELECT * FROM " + name, name.split(".")[-1].strip('"'), None, None, None)
        rel._base = True
        rel._bind(True)
        return rel

    def _bind(self, bare):
        try:
            if self._params is None:
                cols, _ = _d.describe(self._con._h, self._sql)
            else:
                r = _d.query(self._con._h, "SELECT * FROM (" + self._sql + ") LIMIT 0", self._params)
                cols = _d.cols(r)
                _d.free(r)
        except RuntimeError as e:
            m = str(e)
            raise _exc(_bare(m) if bare and not m.startswith("Parser Error") else m)
        self._cols = cols

    def _syntax(self, text):
        # a step's own text is parsed alone first, so a syntax error shows that text, as the duckdb package's do
        try:
            _d.describe(self._con._h, text)
        except RuntimeError as e:
            m = str(e)
            if m.startswith("Parser Error"):
                raise _exc(m)

    # -- building
    def _from(self):
        if self._held is not None:
            raise NotImplementedError("brevi: Brevi's duckdb cannot build on the result of a statement that is not a SELECT")
        return "(" + self._wrapped() + ") AS " + _plain(self._alias)

    def _wrapped(self):
        return "\n\n" + self._sql + "\n" if self._base else self._sql

    def _derive(self, sql, params=None, alias=None, keep=False):
        rel = DuckDBPyRelation(self._con, sql, alias or self._alias, self._params if params is None else params, None, None)
        rel._bind(True)
        if keep and len(rel._cols) == len(self._cols):
            # a step that passes the rows through keeps their names, which a subquery would deduplicate (id_1)
            rel._cols = [(self._cols[k][0],) + tuple(rel._cols[k][1:]) for k in range(len(rel._cols))]
        return rel

    def filter(self, filter_expr):
        self._syntax("SELECT " + filter_expr)
        return self._derive("SELECT * FROM " + self._from() + " WHERE (" + filter_expr + ")", keep=True)

    def project(self, *args, groups=""):
        items = ", ".join(args)
        self._syntax("SELECT " + items)
        if groups:
            self._syntax("SELECT 42 GROUP BY " + groups)
            return self._derive("SELECT " + items + " FROM " + self._from() + " GROUP BY " + groups)
        return self._derive("SELECT " + items + " FROM " + self._from())

    def select(self, *args, groups=""):
        return self.project(*args, groups=groups)

    def order(self, order_expr):
        self._syntax("SELECT 1 ORDER BY " + order_expr)
        return self._derive("SELECT * FROM " + self._from() + " ORDER BY " + order_expr, keep=True)

    def sort(self, *args):
        return self.order(", ".join(args))

    def limit(self, n, offset=0):
        text = "SELECT * FROM " + self._from() + " LIMIT " + str(int(n))
        if offset:
            text += " OFFSET " + str(int(offset))
        return self._derive(text, keep=True)

    def head(self, n=5):
        return self.limit(n)

    def offset(self, n):
        return self._derive("SELECT * FROM " + self._from() + " OFFSET " + str(int(n)), keep=True)

    def distinct(self, on=None):
        if on is not None:
            raise NotImplementedError("brevi: Brevi's duckdb has no distinct(on=...)")
        return self._derive("SELECT DISTINCT * FROM " + self._from(), keep=True)

    def unique(self, unique_aggr):
        self._syntax("SELECT " + unique_aggr)
        return self._derive("SELECT DISTINCT " + unique_aggr + " FROM " + self._from())

    def aggregate(self, aggr_expr, group_expr=""):
        if isinstance(aggr_expr, (list, tuple)):
            aggr_expr = ", ".join(aggr_expr)
        self._syntax("SELECT " + aggr_expr)
        if group_expr:
            self._syntax("SELECT 42 GROUP BY " + group_expr)
        return self._derive("SELECT " + aggr_expr + " FROM " + self._from() + " GROUP BY " + (group_expr or "ALL"))

    def _shortcut(self, fname, columns, parameter="", groups="", window_spec="", projected_columns=""):
        if groups and window_spec:
            raise InvalidInputException("Invalid Input Error: Either groups or window must be set (can't be both at the same time)")
        if window_spec:
            raise NotImplementedError("brevi: Brevi's duckdb has no window_spec")
        pieces = _split_list(columns)
        if fname == "count" and not pieces:
            pieces = ["*"]
        text = projected_columns + ", " if projected_columns else ""
        if not pieces and parameter:
            return self.aggregate(text + fname + "(" + parameter + ") ", groups)
        out = []
        for p in pieces:
            t = p.strip()
            arg = t if not parameter else t + "," + parameter
            out.append(fname + "(" + arg + ") ")
        return self.aggregate(text + ",".join(out), groups)

    def any_value(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("any_value", expression, "", groups, window_spec, projected_columns)

    def arg_max(self, arg_column, value_column, groups="", window_spec="", projected_columns=""):
        return self._shortcut("arg_max", arg_column, value_column, groups, window_spec, projected_columns)

    def arg_min(self, arg_column, value_column, groups="", window_spec="", projected_columns=""):
        return self._shortcut("arg_min", arg_column, value_column, groups, window_spec, projected_columns)

    def avg(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("avg", expression, "", groups, window_spec, projected_columns)

    def mean(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("avg", expression, "", groups, window_spec, projected_columns)

    def bit_and(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("bit_and", expression, "", groups, window_spec, projected_columns)

    def bit_or(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("bit_or", expression, "", groups, window_spec, projected_columns)

    def bit_xor(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("bit_xor", expression, "", groups, window_spec, projected_columns)

    def bool_and(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("bool_and", expression, "", groups, window_spec, projected_columns)

    def bool_or(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("bool_or", expression, "", groups, window_spec, projected_columns)

    def count(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("count", expression, "", groups, window_spec, projected_columns)

    def first(self, expression, groups="", projected_columns=""):
        return self._shortcut("first", expression, "", groups, "", projected_columns)

    def last(self, expression, groups="", projected_columns=""):
        return self._shortcut("last", expression, "", groups, "", projected_columns)

    def list(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("list", expression, "", groups, window_spec, projected_columns)

    def max(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("max", expression, "", groups, window_spec, projected_columns)

    def min(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("min", expression, "", groups, window_spec, projected_columns)

    def median(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("median", expression, "", groups, window_spec, projected_columns)

    def mode(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("mode", expression, "", groups, window_spec, projected_columns)

    def product(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("product", expression, "", groups, window_spec, projected_columns)

    def std(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("stddev_samp", expression, "", groups, window_spec, projected_columns)

    def stddev(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("stddev_samp", expression, "", groups, window_spec, projected_columns)

    def stddev_samp(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("stddev_samp", expression, "", groups, window_spec, projected_columns)

    def stddev_pop(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("stddev_pop", expression, "", groups, window_spec, projected_columns)

    def var(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("var_samp", expression, "", groups, window_spec, projected_columns)

    def variance(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("var_samp", expression, "", groups, window_spec, projected_columns)

    def var_samp(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("var_samp", expression, "", groups, window_spec, projected_columns)

    def var_pop(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("var_pop", expression, "", groups, window_spec, projected_columns)

    def string_agg(self, expression, sep=",", groups="", window_spec="", projected_columns=""):
        return self._shortcut("string_agg", expression, "'" + sep.replace("'", "''") + "'", groups, window_spec, projected_columns)

    def sum(self, expression, groups="", window_spec="", projected_columns=""):
        return self._shortcut("sum", expression, "", groups, window_spec, projected_columns)

    def _other(self, other):
        if not isinstance(other, DuckDBPyRelation):
            raise InvalidInputException("Invalid Input Error: No relation provided")
        return other

    def _both(self, other):
        a = self._params
        b = other._params
        if a is None and b is None:
            return None
        if isinstance(a, dict) or isinstance(b, dict):
            out = {}
            for x in (a, b):
                if x:
                    out.update(x)
            return out
        return list(a or []) + list(b or [])

    def join(self, other_rel, condition=None, how="inner", suffix=None):
        if other_rel is None:
            raise InvalidInputException("Invalid Input Error: No relation provided for join")
        other = self._other(other_rel)
        if suffix is not None:
            raise NotImplementedError("brevi: Brevi's duckdb has no join(suffix=...)")
        kind = str(how).strip().lower()
        if kind not in _JOINS:
            listed = ", ".join("'" + k + "'" for k in _JOINS)
            raise InvalidInputException("Invalid Input Error: Unsupported join type " + str(how) + ", try one of: " + listed)
        if self._alias.lower() == other._alias.lower():
            raise InvalidInputException(
                "Invalid Input Error: Both relations have the same alias, please change the alias of one or both relations using 'rel = rel.set_alias(<new alias>)'"
            )
        keyword, needs = _JOINS[kind]
        on = None
        using = None
        if condition is not None:
            if isinstance(condition, (list, tuple)):
                using = [str(c) for c in condition]
            else:
                items = [x.strip() for x in _split_list(condition)]
                if items and all(_plain(x) == x or (x.startswith('"') and x.endswith('"')) for x in items) and not any(" " in x for x in items):
                    using = items
                else:
                    on = condition
        if needs and on is None and not using:
            raise InvalidInputException("Invalid Input Error: Join type '" + kind + "' needs a condition")
        if not needs and (on is not None or using):
            raise InvalidInputException("Invalid Input Error: Join type '" + kind + "' takes no condition")
        text = "SELECT * FROM " + self._from() + " " + keyword + " JOIN " + other._from()
        if on is not None:
            text += " ON (" + on + ")"
        elif using:
            text += " USING (" + ", ".join(using) + ")"
        return self._derive(text, self._both(other))

    def cross(self, other_rel):
        other = self._other(other_rel)
        return self._derive("SELECT * FROM " + self._from() + " CROSS JOIN " + other._from(), self._both(other))

    def _setop(self, keyword, other):
        other = self._other(other)
        return self._derive("(SELECT * FROM " + self._from() + ") " + keyword + " (SELECT * FROM " + other._from() + ")", self._both(other))

    def union(self, union_rel):
        return self._setop("UNION ALL", union_rel)

    def except_(self, other_rel):
        return self._setop("EXCEPT ALL", other_rel)

    def intersect(self, other_rel):
        return self._setop("INTERSECT ALL", other_rel)

    def set_alias(self, alias):
        rel = DuckDBPyRelation(self._con, self._sql, alias, self._params, self._cols, self._held)
        rel._base = self._base
        return rel

    def query(self, virtual_table_name, sql_query):
        self._con._check_rel()
        text = "WITH " + _plain(virtual_table_name) + " AS (" + self._sql + ") " + sql_query
        cols, kind = _describe(self._con._h, text)
        if kind == 1:
            return DuckDBPyRelation._new(self._con, text, _unnamed(), self._params)
        cols, rows = _all_rows(self._con._h, text, self._params)
        return DuckDBPyRelation._hold(self._con, cols, rows, _unnamed()) if cols else None

    # -- writing
    def _write(self, text):
        self._con._check_rel()
        r = _query(self._con._h, text, self._params)
        if r is not None:
            _d.free(r)

    def create(self, table_name):
        self._write("CREATE TABLE " + table_name + " AS " + self._sql)

    def to_table(self, table_name):
        self.create(table_name)

    def create_view(self, view_name, replace=True):
        self._write(("CREATE OR REPLACE VIEW " if replace else "CREATE VIEW ") + view_name + " AS " + self._sql)
        return self

    def to_view(self, view_name, replace=True):
        return self.create_view(view_name, replace)

    def insert_into(self, table_name):
        self._write("INSERT INTO " + table_name + " " + self._sql)

    def insert(self, values):
        self._con._check_rel()
        row = list(values)
        r = _query(self._con._h, "INSERT INTO " + self._alias + " VALUES (" + ", ".join("?" for _ in row) + ")", row)
        if r is not None:
            _d.free(r)

    # -- reading
    def _rows(self, sql=None):
        self._con._check_rel()
        if self._held is not None:
            return self._held
        cols, rows = _all_rows(self._con._h, self._sql if sql is None else sql, self._params)
        return rows

    def fetchall(self):
        return self._rows()

    def iter_rows(self, batch_size=1024):
        for row in self._rows():
            yield row

    def to_dicts(self):
        names = self.columns
        return [dict(zip(names, row)) for row in self._rows()]

    def fetchone(self):
        raise _removed("fetchone")

    def fetchmany(self, size=1):
        raise _removed("fetchmany")

    def execute(self, *args):
        raise _removed("execute")

    def close(self):
        return None

    def __len__(self):
        if self._held is not None:
            return len(self._held)
        rows = self._rows("SELECT count(*) FROM (" + self._wrapped() + ")")
        return rows[0][0]

    @property
    def shape(self):
        return (len(self), len(self._cols))

    @property
    def columns(self):
        return [c[0] for c in self._cols]

    @property
    def types(self):
        return [DuckDBPyType(c[1]) for c in self._cols]

    @property
    def dtypes(self):
        return self.types

    @property
    def description(self):
        return [(c[0], DuckDBPyType(c[1]), None, None, None, None, None) for c in self._cols]

    @property
    def alias(self):
        return self._alias

    @property
    def type(self):
        if self._sql is not None and self._sql.startswith("SELECT * FROM ") and "(" not in self._sql:
            return "TABLE_RELATION"
        return "QUERY_RELATION"

    # -- printing
    def _box(self, max_width=None, max_rows=None, max_col_width=None, null_value=None, render_mode=None):
        self._con._check_rel()
        if render_mode is not None:
            raise NotImplementedError("brevi: Brevi's duckdb has no render_mode")
        cols = self._cols
        if self._held is not None:
            rows = [[None if v is None else _text(v) for v in row] for row in self._held[:_BOX_LIMIT]]
        elif not cols:
            rows = []
        else:
            sql = "SELECT COLUMNS(*)::VARCHAR FROM (" + self._wrapped() + ") LIMIT " + str(_BOX_LIMIT)
            rows = [list(r) for r in _all_rows(self._con._h, sql, self._params)[1]]
        return _render(cols, rows, max_width, max_rows, max_col_width, null_value)

    def __str__(self):
        return self._box()

    def __repr__(self):
        return self._box()

    def show(self, max_width=None, max_rows=None, max_col_width=None, null_value=None, render_mode=None):
        print(self._box(max_width, max_rows, max_col_width, null_value, render_mode))


# ---------------------------------------------------------------- values as DuckDB writes them (held results)


def _text(v):
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, float):
        if v != v:
            return "nan"
        if v == float("inf"):
            return "inf"
        if v == float("-inf"):
            return "-inf"
        return repr(v)
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join("NULL" if x is None else _text(x) for x in v) + "]"
    if isinstance(v, dict):
        return "{" + ", ".join("'" + str(k) + "': " + ("NULL" if x is None else _text(x)) for k, x in v.items()) + "}"
    return str(v)


# ---------------------------------------------------------------- DuckDB's box renderer (box_renderer.cpp)

_BOX_LIMIT = 10000

_WIDE = (
    (0x1100, 0x115F), (0x231A, 0x231B), (0x2329, 0x232A), (0x23E9, 0x23EC), (0x23F0, 0x23F0), (0x23F3, 0x23F3),
    (0x25FD, 0x25FE), (0x2614, 0x2615), (0x2648, 0x2653), (0x267F, 0x267F), (0x2693, 0x2693), (0x26A1, 0x26A1),
    (0x26AA, 0x26AB), (0x26BD, 0x26BE), (0x26C4, 0x26C5), (0x26CE, 0x26CE), (0x26D4, 0x26D4), (0x26EA, 0x26EA),
    (0x26F2, 0x26F3), (0x26F5, 0x26F5), (0x26FA, 0x26FA), (0x26FD, 0x26FD), (0x2705, 0x2705), (0x270A, 0x270B),
    (0x2728, 0x2728), (0x274C, 0x274C), (0x274E, 0x274E), (0x2753, 0x2755), (0x2757, 0x2757), (0x2795, 0x2797),
    (0x27B0, 0x27B0), (0x27BF, 0x27BF), (0x2B1B, 0x2B1C), (0x2B50, 0x2B50), (0x2B55, 0x2B55), (0x2E80, 0x303E),
    (0x3041, 0x33FF), (0x3400, 0x4DBF), (0x4E00, 0x9FFF), (0xA000, 0xA4CF), (0xA960, 0xA97F), (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF), (0xFE10, 0xFE19), (0xFE30, 0xFE6F), (0xFF00, 0xFF60), (0xFFE0, 0xFFE6), (0x16FE0, 0x16FE4),
    (0x17000, 0x18AFF), (0x1B000, 0x1B2FF), (0x1F004, 0x1F004), (0x1F0CF, 0x1F0CF), (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A), (0x1F200, 0x1F251), (0x1F300, 0x1F64F), (0x1F680, 0x1F6FF), (0x1F7E0, 0x1F7EB),
    (0x1F90C, 0x1F9FF), (0x1FA70, 0x1FAFF), (0x20000, 0x3FFFD),
)
_ZERO = (
    (0x0300, 0x036F), (0x0483, 0x0489), (0x0591, 0x05BD), (0x0610, 0x061A), (0x064B, 0x065F), (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A), (0x200B, 0x200F), (0x20D0, 0x20FF), (0xFE00, 0xFE0F), (0xFE20, 0xFE2F), (0x1F3FB, 0x1F3FF),
    (0xE0100, 0xE01EF),
)


def _cw(c):
    o = ord(c)
    if o < 0x300:
        return 1
    for a, b in _ZERO:
        if a <= o <= b:
            return 0
    for a, b in _WIDE:
        if a <= o <= b:
            return 2
    return 1


def _w(s):
    if s.isascii():
        return len(s)
    n = 0
    for c in s:
        n += _cw(c)
    return n


_ESC = {7: "a", 8: "b", 9: "t", 10: "n", 11: "v", 12: "f", 13: "r", 27: "e"}


def _escape(s):
    if s.isprintable() or all(ord(c) >= 32 for c in s):
        return s
    out = []
    for c in s:
        o = ord(c)
        if o < 32:
            out.append("\\" + _ESC.get(o, str(o)))
        else:
            out.append(c)
    return "".join(out)


def _truncate(value, width, pos, cur):
    """(text, pos, cur): as much of value from pos as fits in width (cur already used), stopping at a newline."""
    start = pos
    n = len(value)
    while pos < n:
        c = value[pos]
        if c == "\n":
            return value[start:pos], pos + 1, cur
        k = _cw(c)
        if cur + k > width:
            break
        cur += k
        pos += 1
    return value[start:pos], pos, cur


class _Val:
    def __init__(self, text, align, width=None):
        self.text = text
        self.align = align
        self.width = _w(text) if width is None else width


_LEFT = 0
_MIDDLE = 1
_RIGHT = 2


def _value(out, text, cw, align, rw=None, vertical="│"):
    if rw is None:
        rw = _w(text)
    if rw > cw:
        small, pos, cur = _truncate(text, cw, 0, 1)
        text = small + "…"
        rw = cur
    pad = cw - rw + 2
    if align == _LEFT:
        lp = 1
        rp = pad - 1
    elif align == _MIDDLE:
        lp = pad // 2
        rp = pad - lp
    else:
        lp = pad - 1
        rp = 1
    out.append(vertical)
    out.append(" " * lp)
    out.append(text)
    out.append(" " * rp)


def _render(cols, rows, max_width=None, max_rows=None, max_col_width=None, null_value=None, limit=_BOX_LIMIT):
    names = [c[0] for c in cols]
    rtypes = [c[2] for c in cols]
    aligns = [_RIGHT if c[3] else _LEFT for c in cols]
    ncols = len(cols)
    mw = max_width or 0
    if mw == 0:
        t = _d.term()
        mw = t if t > 0 else 120
    mw = max(80, mw)
    max_rows = 20 if max_rows is None else max_rows
    mcw = 20 if max_col_width is None else max_col_width
    null = "NULL" if null_value is None else null_value

    row_count = len(rows)
    to_render = min(row_count, max_rows)
    if row_count <= max_rows + 3:
        to_render = row_count
    if to_render == row_count:
        top = row_count
        bottom = 0
    else:
        top = to_render // 2 + (1 if to_render % 2 else 0)
        bottom = to_render - top
    # the footer
    col_count_str = str(ncols) + " column" + ("s" if ncols > 1 else "")
    row_count_str = str(row_count) + " rows"
    limited = limit > 0 and row_count == limit
    if limited:
        row_count_str = "? rows"
    shown = ""
    hidden_rows = top + bottom < row_count
    if hidden_rows:
        if limited:
            shown += ">" + str(limit - 1) + " rows, "
        shown += str(top + bottom) + " shown"
    must_show = limited or hidden_rows or row_count == 0
    footer_len = max(len(row_count_str), len(shown) + 2, 2) + 4

    # the values, as text with their widths
    def cells(rs):
        out = []
        for r in rs:
            line = []
            for c in range(ncols):
                v = r[c]
                if v is None:
                    line.append(_Val(null, aligns[c]))
                else:
                    t = v if aligns[c] == _RIGHT else _escape(v)
                    line.append(_Val(t, aligns[c]))
            out.append(line)
        return out

    collections = [cells(rows[:top]), cells(rows[row_count - bottom :] if bottom else [])]

    # the widths
    header = []
    types = []
    widths = []
    for c in range(ncols):
        name = _escape(names[c])
        header.append(_Val(name, _MIDDLE))
        types.append(_Val(rtypes[c], _MIDDLE))
        widths.append(max(_w(name), _w(rtypes[c])))
    for coll in collections:
        for line in coll:
            for c in range(ncols):
                if line[c].width > widths[c]:
                    widths[c] = line[c].width
    total = 1
    for w in widths:
        total += w + 3
    min_width = footer_len if must_show else 0
    if total < min_width:
        widths[0] += min_width - total
        total = min_width
    pruned = set()
    shortened = False
    if total > mw:
        original = list(widths)
        amounts = []
        total_amount = 0
        for w in widths:
            if w <= mcw:
                amounts.append(0)
                continue
            amounts.append(w - mcw)
            total_amount += w - mcw
        required = total - mw
        if total_amount >= required:
            by_amount = {}
            for c in range(len(amounts)):
                by_amount.setdefault(amounts[c], []).append(c)
            actual = [0] * len(amounts)
            while required > 0:
                keys = sorted(by_amount)
                largest = keys[-1]
                group = by_amount[largest]
                second = keys[-2] if len(keys) > 1 else 0
                step = largest - second
                potential = step * len(group)
                if potential >= required:
                    per = required // len(group)
                    for c in group:
                        actual[c] += per
                        required -= per
                    i = len(group)
                    while i > 0 and required > 0:
                        actual[group[i - 1]] += 1
                        required -= 1
                        i -= 1
                    break
                for c in group:
                    actual[c] += step
                by_amount[second].extend(group)
                del by_amount[largest]
                required -= potential
            for c in range(len(actual)):
                if actual[c] == 0:
                    continue
                widths[c] -= actual[c]
                total -= actual[c]
                shortened = True
        else:
            for c in range(len(widths)):
                if widths[c] <= mcw:
                    continue
                total -= widths[c] - mcw
                widths[c] = mcw
                shortened = True
        if total > mw:
            total += 3 + 1
            offset = 0
            while total > mw:
                c = ncols // 2 + offset
                total -= widths[c] + 3
                pruned.add(c)
                offset = -offset - 1 if offset >= 0 else -offset
            space = mw - total
            for c in range(len(widths)):
                if space <= 0:
                    break
                if c in pruned or widths[c] >= original[c]:
                    continue
                inc = min(space, original[c] - widths[c])
                widths[c] += inc
                space -= inc
                total += inc
    hidden_cols = False
    if pruned:
        hidden_cols = True
        col_count_str += " (" + str(ncols - len(pruned)) + " shown)"
    colmap = []
    new_widths = []
    split_added = False
    for c in range(ncols):
        if c not in pruned:
            colmap.append(c)
            new_widths.append(widths[c])
        elif not split_added:
            colmap.append(None)
            new_widths.append(1)
            split_added = True
    widths = new_widths
    header = [header[c] for c in colmap if c is not None]
    types = [types[c] for c in colmap if c is not None]
    rendered = top + bottom
    expand = False
    per_row = 1
    if shortened and rendered > 0 and rendered + 5 < max_rows and not pruned:
        per_row = max(1, 0 if max_rows <= 5 else (max_rows - 5) // rendered)
        if per_row > 1:
            expand = True
    bounds = []
    for c in range(len(widths)):
        bounds.append(widths[c] + 2 if c == 0 else bounds[c - 1] + widths[c] + 3)

    out = []

    def layout(fill, cross, left, right):
        out.append(left)
        k = 0
        bi = 0
        while k < total - 2:
            if bi < len(bounds) and k == bounds[bi]:
                out.append(cross)
                bi += 1
            else:
                out.append(fill)
            k += 1
        out.append(right)
        out.append("\n")

    def render_row(line):
        if line is None:
            layout("─", "┼", "├", "┤")
            return
        vi = 0
        for ci in range(len(widths)):
            if colmap[ci] is None:
                _value(out, "…", widths[ci], _MIDDLE, 1)
            else:
                v = line[vi]
                vi += 1
                _value(out, v.text, widths[ci], v.align, v.width)
        out.append("│\n")

    def expand_row(line, dest, first):
        extra = []
        for c in range(len(line)):
            v = line[c]
            if v.width <= widths[c]:
                continue
            full = v.text
            text, pos, cur = _truncate(full, widths[c], 0, 0)
            v.text = text
            v.width = cur
            row_i = 0
            while pos < len(full):
                if row_i >= len(extra):
                    if len(extra) >= per_row + 1:
                        break
                    extra.append([_Val("", x.align, 0) for x in line])
                can_add = row_i + 1 < len(extra) or len(extra) < per_row
                er = extra[row_i]
                row_i += 1
                if can_add:
                    text, pos, cur = _truncate(full, widths[c], pos, 0)
                else:
                    text = full[pos:]
                    cur = _w(text)
                    pos = len(full)
                er[c] = _Val(text, er[c].align, cur)
        if not first:
            dest.append(None)
        dest.append(line)
        dest.extend(extra)

    layout("─", "┬", "┌", "┐")
    head_rows = []
    for line in (header, types):
        if expand:
            expand_row(line, head_rows, True)
        else:
            head_rows.append(line)
    for line in head_rows:
        render_row(line)
    if row_count > 0:
        render_row(None)

    first = True
    last = None
    divider = False
    first_coll = True
    for coll in collections:
        if not coll:
            continue
        if not first_coll:
            divider = True
        first_coll = False
        lines = []
        for c in range(len(coll)):
            line = [coll[c][k] for k in colmap if k is not None]
            if divider:
                prev = last[-1]
                dots = []
                for k in range(len(prev)):
                    pv = prev[k]
                    align = pv.align
                    if align == _MIDDLE:
                        s = "·"
                    else:
                        tl = min(widths[k], _w(pv.text))
                        bl = min(widths[k], _w(line[k].text))
                        dl = min(tl, bl)
                        if tl == 0:
                            dl = bl
                        elif bl == 0:
                            dl = tl
                        if dl > 1:
                            padding = dl - 1
                            if align == _LEFT:
                                lp = padding // 2
                                rp = padding - lp
                            else:
                                rp = padding // 2
                                lp = padding - rp
                            s = " " * lp + "·" + " " * rp
                        else:
                            if dl == 0:
                                align = _MIDDLE
                            s = "·"
                    dots.append(_Val(s, align))
                for _ in range(3):
                    render_row(dots)
                divider = False
            if expand:
                expand_row(line, lines, first)
            else:
                lines.append(line)
            first = False
        for line in lines:
            render_row(line)
        last = [x for x in lines if x is not None]

    # the footer
    min_len = len(row_count_str) + len(col_count_str) + 6
    rows_and_cols = total >= min_len and ((hidden_cols and row_count > 0) or (row_count >= 10 and ncols > 1))
    rows_only = total >= footer_len and (row_count == 0 or row_count >= 10)
    layout("─", "┴", "└", "┘")
    if rows_only or rows_and_cols:
        padding = total - len(row_count_str) - 4
        if rows_and_cols:
            padding -= len(col_count_str)
        extra = ""
        shown_size = len(shown) + 3
        if hidden_rows and padding >= shown_size:
            extra = " (" + shown + ")"
            padding -= shown_size
            shown = ""
        out.append("  ")
        if rows_and_cols:
            out.append(row_count_str)
            out.append(extra)
            out.append(" " * padding)
            out.append(col_count_str)
        else:
            lp = padding // 2
            out.append(" " * lp)
            out.append(row_count_str)
            out.append(extra)
            out.append(" " * (padding - lp))
        out.append("\n")
        if shown:
            _value(out, "(" + shown + ")", total - 4, _LEFT if rows_and_cols else _MIDDLE, None, " ")
            out.append("\n")
    return "".join(out)


# ---------------------------------------------------------------- the default connection

_DEFAULT = [None]


def default_connection():
    if _DEFAULT[0] is None or _DEFAULT[0]._closed:
        _DEFAULT[0] = connect(":memory:")
    return _DEFAULT[0]


def set_default_connection(connection):
    _DEFAULT[0] = connection


def execute(query, parameters=None, connection=None):
    return (connection or default_connection()).execute(query, parameters)


def executemany(query, parameters=None, connection=None):
    return (connection or default_connection()).executemany(query, parameters)


def sql(query, alias="", params=None, connection=None):
    return (connection or default_connection()).sql(query, alias, params)


def query(query, alias="", params=None, connection=None):
    return (connection or default_connection()).sql(query, alias, params)


def from_query(query, alias="", params=None, connection=None):
    return (connection or default_connection()).sql(query, alias, params)


def table(table_name, connection=None):
    return (connection or default_connection()).table(table_name)


def view(view_name, connection=None):
    return (connection or default_connection()).view(view_name)


def values(*args, connection=None):
    return (connection or default_connection()).values(*args)


def fetchone(connection=None):
    return (connection or default_connection()).fetchone()


def fetchmany(size=1, connection=None):
    return (connection or default_connection()).fetchmany(size)


def fetchall(connection=None):
    return (connection or default_connection()).fetchall()


def description(connection=None):
    return (connection or default_connection()).description


def rowcount(connection=None):
    return (connection or default_connection()).rowcount


def cursor(connection=None):
    return (connection or default_connection()).cursor()


def begin(connection=None):
    return (connection or default_connection()).begin()


def commit(connection=None):
    return (connection or default_connection()).commit()


def rollback(connection=None):
    return (connection or default_connection()).rollback()


def checkpoint(connection=None):
    return (connection or default_connection()).checkpoint()


def complete(connection=None):
    return (connection or default_connection()).complete()


def abort(connection=None):
    return (connection or default_connection()).abort()


def interrupt(connection=None):
    return (connection or default_connection()).interrupt()


def close(connection=None):
    return (connection or default_connection()).close()


def duplicate(connection=None):
    return (connection or default_connection()).cursor()
