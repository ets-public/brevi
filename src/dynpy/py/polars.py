# Brevi's polars (after polars 2.1.0), in plain Python: DataFrame, Series, LazyFrame, expressions (pl.col, pl.lit,
# pl.when, ...), group_by / join / sort / pivot, the str and list namespaces, CSV / JSON / NDJSON in and out, and
# the printed tables (comfy-table's layout, polars' float rules). Values are kept per column in Python lists
# (None is null). What it lacks (temporal and struct dtypes, parquet / IPC / databases, SQL, numpy / pandas
# interop, plugins) sends the program to CPython's polars (src/caps.rs POLARS_LACKS), or is refused under
# --sandbox. Files are read and written through _brevi_fs, which the read / write grants hold.

import math
import json as _json
import re as _re
import io as _io

__version__ = "2.1.0"

# polars names some functions as builtins (pl.len, pl.sum, pl.all, ...): the module uses the builtins by these names
_len = len
_sum = sum
_min = min
_max = max
_all = all
_format = format

_I64_MIN = -(1 << 63)
_I64_MAX = (1 << 63) - 1


# ---------- exceptions ----------


class PolarsError(Exception):
    pass


class ColumnNotFoundError(PolarsError):
    pass


class ComputeError(PolarsError):
    pass


class DuplicateError(PolarsError):
    pass


class InvalidOperationError(PolarsError):
    pass


class NoDataError(PolarsError):
    pass


class OutOfBoundsError(PolarsError):
    pass


class SchemaError(PolarsError):
    pass


class SchemaFieldNotFoundError(PolarsError):
    pass


class ShapeError(PolarsError):
    pass


class SQLInterfaceError(PolarsError):
    pass


class PolarsWarning(Warning):
    pass


class _Exceptions:
    pass


exceptions = _Exceptions()
for _n, _c in (("PolarsError", PolarsError), ("ColumnNotFoundError", ColumnNotFoundError), ("ComputeError", ComputeError),
               ("DuplicateError", DuplicateError), ("InvalidOperationError", InvalidOperationError), ("NoDataError", NoDataError),
               ("OutOfBoundsError", OutOfBoundsError), ("SchemaError", SchemaError), ("SchemaFieldNotFoundError", SchemaFieldNotFoundError),
               ("ShapeError", ShapeError), ("PolarsWarning", PolarsWarning)):
    setattr(exceptions, _n, _c)


# ---------- data types ----------


class DataType:
    def __init__(self, name, short, kind, bits=0, inner=None):
        self._name = name
        self._short = short
        self._kind = kind  # "int", "uint", "float", "str", "bool", "null", "list", "obj"
        self._bits = bits
        self.inner = inner

    def __call__(self):
        return self

    def __repr__(self):
        if self._kind == "list":
            return "List(" + repr(self.inner) + ")"
        return self._name

    def __str__(self):
        return self.__repr__()

    def __eq__(self, other):
        if isinstance(other, DataType):
            if self._kind == "list" and other._kind == "list":
                return self.inner is None or other.inner is None or self.inner == other.inner
            return self._name == other._name
        if other is list:
            return self._kind == "list"
        if other is int:
            return self._name == "Int64"
        if other is float:
            return self._name == "Float64"
        if other is str:
            return self._name == "String"
        if other is bool:
            return self._name == "Boolean"
        return False

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._name)

    def is_numeric(self):
        return self._kind in ("int", "uint", "float")

    def is_integer(self):
        return self._kind in ("int", "uint")

    def is_signed_integer(self):
        return self._kind == "int"

    def is_unsigned_integer(self):
        return self._kind == "uint"

    def is_float(self):
        return self._kind == "float"

    def is_nested(self):
        return self._kind == "list"

    def is_temporal(self):
        return False

    def is_decimal(self):
        return False

    def is_(self, other):
        return self == other

    def base_type(self):
        if self._kind == "list":
            return List
        return self

    def to_python(self):
        return {"int": int, "uint": int, "float": float, "str": str, "bool": bool, "list": list}.get(self._kind, type(None))


class _ListType(DataType):
    def __init__(self, inner=None):
        DataType.__init__(self, "List", "list", "list", 0, _dt(inner) if inner is not None else None)

    def __call__(self, inner=None):
        return _ListType(inner)


Int8 = DataType("Int8", "i8", "int", 8)
Int16 = DataType("Int16", "i16", "int", 16)
Int32 = DataType("Int32", "i32", "int", 32)
Int64 = DataType("Int64", "i64", "int", 64)
UInt8 = DataType("UInt8", "u8", "uint", 8)
UInt16 = DataType("UInt16", "u16", "uint", 16)
UInt32 = DataType("UInt32", "u32", "uint", 32)
UInt64 = DataType("UInt64", "u64", "uint", 64)
Float32 = DataType("Float32", "f32", "float", 32)
Float64 = DataType("Float64", "f64", "float", 64)
String = DataType("String", "str", "str")
Utf8 = String
Boolean = DataType("Boolean", "bool", "bool")
Null = DataType("Null", "null", "null")
Object = DataType("Object", "object", "obj")
List = _ListType()

_INTS = [Int8, Int16, Int32, Int64]
_UINTS = [UInt8, UInt16, UInt32, UInt64]


def _dt(d):
    # a dtype, from a DataType or a Python type
    if isinstance(d, DataType):
        return d
    if d is int:
        return Int64
    if d is float:
        return Float64
    if d is str:
        return String
    if d is bool:
        return Boolean
    if d is None or d is type(None):
        return Null
    if d is list:
        return _ListType(None)
    raise TypeError("cannot parse input of type " + repr(type(d).__name__) + " into Polars data type: " + repr(d))


def _short(d):
    if d._kind == "list":
        return "list[" + (_short(d.inner) if d.inner is not None else "null") + "]"
    return d._short


def _int_range(d):
    if d._kind == "int":
        return -(1 << (d._bits - 1)), (1 << (d._bits - 1)) - 1
    return 0, (1 << d._bits) - 1


def _wrap(v, d):
    # integer arithmetic wraps, as Rust's wrapping ops in polars' kernels
    lo, hi = _int_range(d)
    if lo <= v <= hi:
        return v
    m = 1 << d._bits
    v = v % m
    if d._kind == "int" and v > hi:
        v -= m
    return v


_F32_MAX = 3.4028234663852886e38


def _f32(x):
    # round a float to the nearest float32 (ties to even), as Rust's `as f32`
    if x is None or x != x or x == 0.0 or x in (math.inf, -math.inf):
        return x
    m, e = math.frexp(x)
    if e - 1 < -126:
        q = math.ldexp(x, 149)
        r = round(q)
        return math.ldexp(r, -149)
    r = round(m * 16777216.0)
    y = math.ldexp(r, e - 24)
    if abs(y) > _F32_MAX:
        return math.copysign(math.inf, x)
    return y


def _f32_repr(x):
    # the shortest decimal that rounds back to the same float32 (Rust's Display / Python repr of a float32 value)
    if x != x or x in (math.inf, -math.inf) or x == 0.0:
        return repr(x)
    for p in range(1, 18):
        s = "%.*g" % (p, x)
        if _f32(float(s)) == x:
            return repr(float(s))
    return repr(x)


def _supertype(a, b):
    if a == b:
        return a
    if a._kind == "null":
        return b
    if b._kind == "null":
        return a
    ka, kb = a._kind, b._kind
    if ka == "list" and kb == "list":
        if a.inner is None:
            return b
        if b.inner is None:
            return a
        return _ListType(_supertype(a.inner, b.inner))
    if ka == "bool" and kb in ("int", "uint", "float"):
        return b
    if kb == "bool" and ka in ("int", "uint", "float"):
        return a
    if ka in ("int", "uint") and kb in ("int", "uint"):
        if ka == kb:
            return a if a._bits >= b._bits else b
        s, u = (a, b) if ka == "int" else (b, a)
        if s._bits > u._bits:
            return s
        if u._bits == 64:
            return Float64
        return _INTS[[8, 16, 32, 64].index(u._bits) + 1]
    if ka == "float" and kb == "float":
        return Float64
    if ka == "float" and kb in ("int", "uint"):
        return a if (a._bits == 64 or b._bits <= 16) else Float64
    if kb == "float" and ka in ("int", "uint"):
        return b if (b._bits == 64 or a._bits <= 16) else Float64
    if ka == "str" or kb == "str":
        return String
    raise SchemaError("failed to determine supertype of " + _short(a) + " and " + _short(b))


def _py_dtype(v):
    # the dtype polars infers for one Python value
    if v is None:
        return Null
    if v is True or v is False:
        return Boolean
    if isinstance(v, int):
        return Int64
    if isinstance(v, float):
        return Float64
    if isinstance(v, str):
        return String
    if isinstance(v, (list, tuple)):
        inner = Null
        for x in v:
            inner = _supertype(inner, _py_dtype(x))
        return _ListType(inner)
    if isinstance(v, Series):
        return _ListType(v.dtype)
    raise TypeError("unexpected value while building Series of type Object; found value of type " + repr(type(v).__name__) + ": " + repr(v))


def _infer(vals):
    d = Null
    for v in vals:
        if v is None:
            continue
        t = _py_dtype(v)
        if d._kind == "null":
            d = t
        elif t != d:
            if d._kind == "str" or t._kind == "str":
                raise TypeError("unexpected value while building Series of type " + repr(d) + "; found value of type " + repr(t) + ": " + _repr_value(v))
            d = _supertype(d, t)
    return d


class _Int128NameT:
    def __repr__(self):
        return "Int128"

    _kind = "int128"


_Int128Name = _Int128NameT()


def _repr_value(v):
    if isinstance(v, str):
        return '"' + v + '"'
    return repr(v)


def _cast_value(v, d, strict, name=""):
    # one value to dtype d: (ok, value)
    if v is None:
        return True, None
    k = d._kind
    if k in ("int", "uint"):
        if v is True or v is False:
            return True, int(v)
        if isinstance(v, int):
            lo, hi = _int_range(d)
            if lo <= v <= hi:
                return True, v
            return False, None
        if isinstance(v, float):
            if v != v or v in (math.inf, -math.inf):
                return False, None
            iv = int(v)
            lo, hi = _int_range(d)
            if lo <= iv <= hi:
                return True, iv
            return False, None
        if isinstance(v, str):
            t = v
            if _re.fullmatch(r"[+-]?[0-9]+", t):
                iv = int(t)
                lo, hi = _int_range(d)
                if lo <= iv <= hi:
                    return True, iv
            return False, None
        return False, None
    if k == "float":
        if v is True or v is False:
            r = 1.0 if v else 0.0
        elif isinstance(v, (int, float)):
            try:
                r = float(v)
            except OverflowError:
                r = math.inf if v > 0 else -math.inf
        elif isinstance(v, str):
            r = _parse_float(v)
            if r is None:
                return False, None
        else:
            return False, None
        return True, (_f32(r) if d._bits == 32 else r)
    if k == "str":
        if isinstance(v, str):
            return True, v
        if v is True or v is False:
            return True, "true" if v else "false"
        if isinstance(v, int):
            return True, str(v)
        if isinstance(v, float):
            return True, _float_str(v, False)
        if isinstance(v, list):
            return False, None
        return True, str(v)
    if k == "bool":
        if v is True or v is False:
            return True, v
        if isinstance(v, (int, float)):
            return True, v != 0
        if isinstance(v, str):
            if v == "true":
                return True, True
            if v == "false":
                return True, False
            return False, None
        return False, None
    if k == "null":
        return True, None
    if k == "list":
        if isinstance(v, Series):
            v = v.to_list()
        if not isinstance(v, (list, tuple)):
            return False, None
        out = []
        for x in v:
            ok, y = _cast_value(x, d.inner if d.inner is not None else _py_dtype(x), strict)
            if not ok:
                return False, None
            out.append(y)
        return True, out
    return True, v


_FLOAT_RE = _re.compile(r"[+-]?(?:(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?|inf|infinity|nan|NaN|Inf|Infinity)")


def _parse_float(t):
    t = t.strip() if False else t
    if not _FLOAT_RE.fullmatch(t):
        return None
    return float(t)


def _float_str(v, f32):
    # a float as polars' String cast writes it (Rust Display, shortest round trip)
    if v != v:
        return "NaN"
    if v == math.inf:
        return "inf"
    if v == -math.inf:
        return "-inf"
    s = _rdisp(v, f32)
    if "." not in s:
        s = s + ".0"
    return s


# ---------- Rust float formatting ----------


def _digits(v, f32=False):
    # (negative, digit string, decimal exponent) of the shortest round-trip repr: value = 0.DIGITS * 10**exp
    r = _f32_repr(v) if f32 else repr(v)
    neg = r.startswith("-")
    if neg:
        r = r[1:]
    e = 0
    if "e" in r:
        r, es = r.split("e")
        e = int(es)
    if "." in r:
        a, b = r.split(".")
    else:
        a, b = r, ""
    if b == "0":
        b = ""
    ds = (a + b).lstrip("0")
    lead = _len(a + b) - _len((a + b).lstrip("0"))
    point = _len(a) + e - lead
    ds = ds.rstrip("0")
    if ds == "":
        return neg, "0", 1
    return neg, ds, point


def _rdisp(v, f32=False):
    # Rust's Display for f64 / f32: shortest round-trip digits, never in exponent form
    if v != v:
        return "NaN"
    if v in (math.inf, -math.inf):
        return "inf" if v > 0 else "-inf"
    neg, ds, p = _digits(v, f32)
    sign = "-" if neg else ""
    if ds == "0":
        return sign + "0"
    if p <= 0:
        return sign + "0." + "0" * (-p) + ds
    if p >= _len(ds):
        return sign + ds + "0" * (p - _len(ds))
    return sign + ds[:p] + "." + ds[p:]


def _rexp(v, prec=None, f32=False):
    # Rust's {:e} / {:.Ne}: 1e6, 1.5e10, 1.0000e-7
    if v != v:
        return "NaN"
    if v in (math.inf, -math.inf):
        return "inf" if v > 0 else "-inf"
    if prec is None:
        neg, ds, p = _digits(v, f32)
        sign = "-" if neg else ""
        if ds == "0":
            return sign + "0e0"
        m = ds[0] + ("." + ds[1:] if _len(ds) > 1 else "")
        return sign + m + "e" + str(p - 1)
    s = "%.*e" % (prec, v)
    m, e = s.split("e")
    return m + "e" + str(int(e))


def _rfix(v, prec):
    # Rust's {:.N}
    if v != v:
        return "NaN"
    if v in (math.inf, -math.inf):
        return "inf" if v > 0 else "-inf"
    return "%.*f" % (prec, v)


def _fmt_float(v, f32=False):
    # polars-core fmt.rs fmt_float, with the default (mixed) float format
    prec = _CFG["float_precision"]
    if prec is not None:
        s = _rfix(v, prec)
        if _len(s) > 19:
            return _rexp(v, prec, f32)
        return _group(s)
    if _CFG["float_fmt"] == "full":
        return _group(_rdisp(v, f32))
    fract_zero = v == v and v not in (math.inf, -math.inf) and v == math.floor(v)
    if fract_zero and abs(v) < 999999.0:
        return _group(_rfix(v, 1))
    d = _rdisp(v, f32)
    if _len(d) > 9:
        a = abs(v)
        if (not (0.000001 <= a <= 999999.0) or a > 999999.0) and _CFG["thousands"] == "":
            return _group(_rexp(v, 4, f32))
        s = _rfix(v, 6)
        if s.endswith("0"):
            s = s.rstrip("0")
            if s.endswith("."):
                s = s + "0"
        return _group(s)
    if fract_zero:
        return _group(_rexp(v, None, f32))
    return _group(d)


def _group(s):
    sep = _CFG["thousands"]
    dec = _CFG["decimal"]
    if _len(s) <= 1 or (sep == "" and dec == "."):
        return s
    if "e" in s or "inf" in s or "NaN" in s:
        return s
    i = s.find(".")
    ip, fp = (s, None) if i < 0 else (s[:i], s[i + 1:])
    return _group_int(ip, sep) + ("" if fp is None else dec + fp)


def _group_int(s, sep):
    if sep == "" or _len(s) <= 1:
        return s
    sign = ""
    if s[0] in "+-":
        sign, s = s[0], s[1:]
    parts = []
    while _len(s) > 3:
        parts.insert(0, s[-3:])
        s = s[:-3]
    parts.insert(0, s)
    return sign + sep.join(parts)


def _fmt_int(v):
    return _group_int(str(v), _CFG["thousands"])


def _any_str(v, d, quote):
    # AnyValue Display: a value inside a list or a Series (strings quoted), or a table cell (quote=False)
    if v is None:
        return "null"
    k = d._kind
    if k == "str":
        return '"' + v + '"' if quote else v
    if k == "bool":
        return "true" if v else "false"
    if k in ("int", "uint"):
        return _fmt_int(v)
    if k == "float":
        return _fmt_float(v, d._bits == 32)
    if k == "list":
        return _fmt_list(v, d.inner if d.inner is not None else Null)
    if k == "null":
        return "null"
    return str(v)


def _fmt_list(vals, inner):
    if _len(vals) == 0:
        return "[]"
    lim = _CFG["list_len"]
    ell = _ellipsis()
    if lim == 0:
        return "[" + ell + "]"
    if lim >= _len(vals):
        return "[" + ", ".join(_any_str(x, inner, True) for x in vals) + "]"
    out = []
    for i in range(lim - 1):
        out.append(_any_str(vals[i], inner, True) + ", ")
    return "[" + "".join(out) + ell + " " + _any_str(vals[-1], inner, True) + "]"


# ---------- Config ----------


_CFG_DEFAULT = {
    "rows": 10, "cols": 8, "str_len": 30, "list_len": 3, "width": None, "float_precision": None, "float_fmt": "mixed",
    "thousands": "", "decimal": ".", "formatting": "UTF8_FULL_CONDENSED", "hide_dtypes": False, "hide_names": False,
    "hide_sep": False, "inline_dtype": False, "hide_shape": False, "shape_below": False, "align": None, "num_align": None,
    "round_corners": False, "auto_structify": False,
}
_CFG = dict(_CFG_DEFAULT)


def _lim(n, default):
    if n is None:
        return default
    if n < 0:
        return 1 << 62
    return n


class Config:
    def __init__(self, restore_defaults=False, apply_on_context_enter=False, **options):
        self._saved = None
        self._opts = options
        self._restore = restore_defaults
        if not apply_on_context_enter:
            self._apply()

    def _apply(self):
        self._saved = dict(_CFG)
        if self._restore:
            Config.restore_defaults()
        for k, v in self._opts.items():
            getattr(Config, k if k.startswith("set_") else "set_" + k)(v)

    def __enter__(self):
        if self._saved is None:
            self._apply()
        return self

    def __exit__(self, *a):
        _CFG.clear()
        _CFG.update(self._saved)
        return False

    @staticmethod
    def restore_defaults():
        _CFG.clear()
        _CFG.update(_CFG_DEFAULT)
        return Config

    @staticmethod
    def set_tbl_rows(n=None):
        _CFG["rows"] = _lim(n, 10)
        return Config

    @staticmethod
    def set_tbl_cols(n=None):
        _CFG["cols"] = _lim(n, 8)
        return Config

    @staticmethod
    def set_fmt_str_lengths(n=None):
        if n is not None and n <= 0:
            raise ValueError("invalid `n` value: " + str(n) + "; must be a positive integer")
        _CFG["str_len"] = _lim(n, 30)
        return Config

    @staticmethod
    def set_fmt_table_cell_list_len(n=None):
        _CFG["list_len"] = _lim(n, 3)
        return Config

    @staticmethod
    def set_tbl_width_chars(width=None):
        _CFG["width"] = None if width is None else (65535 if width < 0 else width)
        return Config

    @staticmethod
    def set_float_precision(precision=None):
        _CFG["float_precision"] = precision
        return Config

    @staticmethod
    def set_fmt_float(fmt="mixed"):
        _CFG["float_fmt"] = "mixed" if fmt is None else fmt
        return Config

    @staticmethod
    def set_thousands_separator(separator=None):
        if separator is True:
            separator = ","
        _CFG["thousands"] = separator or ""
        return Config

    @staticmethod
    def set_decimal_separator(separator=None):
        _CFG["decimal"] = separator or "."
        return Config

    @staticmethod
    def set_tbl_formatting(format=None, rounded_corners=False):
        _CFG["formatting"] = format or "UTF8_FULL_CONDENSED"
        _CFG["round_corners"] = rounded_corners
        return Config

    @staticmethod
    def set_tbl_hide_column_data_types(active=True):
        _CFG["hide_dtypes"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_hide_column_names(active=True):
        _CFG["hide_names"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_hide_dtype_separator(active=True):
        _CFG["hide_sep"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_column_data_type_inline(active=True):
        _CFG["inline_dtype"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_hide_dataframe_shape(active=True):
        _CFG["hide_shape"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_dataframe_shape_below(active=True):
        _CFG["shape_below"] = bool(active)
        return Config

    @staticmethod
    def set_tbl_cell_alignment(format=None):
        _CFG["align"] = format
        return Config

    @staticmethod
    def set_tbl_cell_numeric_alignment(format=None):
        _CFG["num_align"] = format
        return Config


def _ellipsis():
    return "..." if _CFG["formatting"].startswith("ASCII") else "…"


# ---------- table drawing (comfy-table 7.2 with polars' settings) ----------

_WIDE = ((0x1100, 0x115F), (0x2E80, 0x303E), (0x3041, 0x33FF), (0x3400, 0x4DBF), (0x4E00, 0x9FFF), (0xA000, 0xA4CF),
         (0xAC00, 0xD7A3), (0xF900, 0xFAFF), (0xFE30, 0xFE4F), (0xFF00, 0xFF60), (0xFFE0, 0xFFE6), (0x1F300, 0x1F64F),
         (0x1F900, 0x1F9FF), (0x20000, 0x2FFFD), (0x30000, 0x3FFFD))
_ZERO = ((0x0300, 0x036F), (0x0483, 0x0489), (0x0591, 0x05BD), (0x0610, 0x061A), (0x064B, 0x065F), (0x200B, 0x200F),
         (0x20D0, 0x20FF), (0xFE00, 0xFE0F), (0xFE20, 0xFE2F))


def _cw(c):
    o = ord(c)
    if o < 0x300:
        return 0 if o < 32 or 0x7F <= o < 0xA0 else 1
    for a, b in _ZERO:
        if a <= o <= b:
            return 0
    for a, b in _WIDE:
        if a <= o <= b:
            return 2
    return 1


def _width(s):
    # unicode-width of a line
    if s.isascii():
        n = 0
        for c in s:
            if " " <= c <= "~":
                n += 1
        return n
    return _sum(_cw(c) for c in s)


def _est_width(s):
    # polars' estimate_string_width
    nc = _len(s)
    nb = _len(s.encode("utf-8"))
    if nb == nc:
        return nc
    return _min(nc * 2, math.ceil(nc * (nb / nc)))


def _trunc(s, n, ell):
    if _len(s) <= n:
        return s
    return s[:n] + ell


def _preset(name):
    # comfy-table's presets: one character per TableComponent (a space: not drawn)
    P = {
        "ASCII_FULL": "||--+==+|-+||++++++",
        "ASCII_FULL_CONDENSED": "||--+==+|    ++++++",
        "ASCII_NO_BORDERS": "     == |-+        ",
        "ASCII_BORDERS_ONLY": "||--+==+   ||--++++",
        "ASCII_BORDERS_ONLY_CONDENSED": "||--+==+     --++++",
        "ASCII_HORIZONTAL_ONLY": "  -- ==  --  --    ",
        "ASCII_MARKDOWN": "||  |-|||           ",
        "MARKDOWN": "||  |-|||           ",
        "UTF8_FULL": "││──╞═╪╡┆╌┼├┤┬┴┌┐└┘",
        "UTF8_FULL_CONDENSED": "││──╞═╪╡┆    ┬┴┌┐└┘",
        "UTF8_NO_BORDERS": "     ═╪ ┆╌┼        ",
        "UTF8_BORDERS_ONLY": "││──╞══╡     ──┌┐└┘",
        "UTF8_HORIZONTAL_ONLY": "  ── ══  ──  ──    ",
        "NOTHING": "                   ",
    }
    s = P.get(name, P["UTF8_FULL_CONDENSED"])
    return [(None if c == " " else c) for c in s[:19]]


# TableComponent indices
_LB, _RB, _TB, _BB, _LHI, _HL, _MHI, _RHI, _VL, _HZ, _MI, _LBI, _RBI, _TBI, _BBI, _TLC, _TRC, _BLC, _BRC = range(19)


def _split_line(line, width):
    # comfy-table content_split::split_line with ' ' as the delimiter
    lines = []
    elements = line.split(" ")
    elements.reverse()
    cur = ""
    while elements:
        nxt = elements.pop()
        cl = _width(cur)
        nl = _width(nxt)
        added = nl + cl + (1 if cur != "" else 0)
        rem = width - cl
        if cur != "":
            rem = _max(0, rem - 1)
        if added <= width:
            if cur != "":
                cur += " "
            cur += nxt
            if _width(cur) > _max(0, width - 2):
                lines.append(cur)
                cur = ""
            continue
        if cur != "" and rem <= 2:
            elements.append(nxt)
            lines.append(cur)
            cur = ""
            continue
        if nl > width:
            new_line = cur == ""
            if not new_line:
                cur += " "
            head, w = "", 0
            i = 0
            while i < _len(nxt) and w + _cw(nxt[i]) <= rem:
                w += _cw(nxt[i])
                head += nxt[i]
                i += 1
            tail = nxt[i:]
            if new_line and head == "":
                head, tail = tail[:1], tail[1:]
            cur += head
            elements.append(tail)
            lines.append(cur)
            cur = ""
            continue
        lines.append(cur)
        cur = nxt
        if _width(cur) > _max(0, width - 2):
            lines.append(cur)
            cur = ""
    if cur != "":
        lines.append(cur)
    return lines


def _arrange(header, rows, constraints, table_width, borders):
    # comfy-table arrangement: content width of each column. constraints: (kind, lo, hi) with kind "abs" or "bounds".
    n = _len(constraints)
    cells = [[c.split("\n") for c in r] for r in ([header] + rows if header is not None else rows)]
    maxw = [0] * n
    for r in cells:
        for i in range(n):
            w = 0
            if i < _len(r):
                for ln in r[i]:
                    w = _max(w, _width(ln))
            w = _max(1, w)
            maxw[i] = _max(maxw[i], w)
    info = {}
    pad = 2
    for i, (kind, lo, hi) in enumerate(constraints):
        if kind == "abs":
            info[i] = _max(1, lo - pad)
        mn = lo
        if maxw[i] + pad <= mn:
            info[i] = _max(1, mn - pad)
    if table_width is None:
        for i in range(n):
            if i not in info:
                w = maxw[i]
                kind, lo, hi = constraints[i]
                if kind == "bounds":
                    if w + pad > hi:
                        w = _max(1, hi - pad)
                info[i] = w
        return [info[i] for i in range(n)], cells
    nb = (1 if borders[0] else 0) + (1 if borders[1] else 0) + (_max(0, n - 1) if borders[2] else 0)
    remaining = _max(0, table_width - nb)
    for i in range(n):
        if i not in info:
            remaining = _max(0, remaining - pad)
    for i in info:
        remaining = _max(0, remaining - (info[i] + pad))
    rcols = n - _len(info)
    if rcols > 0:
        avg = remaining // rcols
        again = True
        while again:
            again = False
            for i in range(n):
                if i in info:
                    continue
                kind, lo, hi = constraints[i]
                if avg >= lo:
                    continue
                w = _max(1, lo - pad)
                info[i] = w
                remaining = _max(0, remaining - w)
                rcols -= 1
                if rcols == 0:
                    break
                avg = remaining // rcols
                again = True
    found = True
    while found:
        found = False
        if rcols == 0:
            break
        avg = remaining // rcols
        if avg == 0:
            break
        for i in range(n):
            if i in info:
                continue
            kind, lo, hi = constraints[i]
            if kind == "bounds":
                if hi <= avg + pad and maxw[i] + pad >= hi:
                    w = _max(1, hi - pad)
                    info[i] = w
                    remaining = _max(0, remaining - w)
                    rcols -= 1
                    if rcols == 0:
                        break
                    avg = remaining // rcols
                    found = True
                    continue
            if maxw[i] <= avg:
                info[i] = maxw[i]
                remaining = _max(0, remaining - maxw[i])
                rcols -= 1
                if rcols == 0:
                    break
                avg = remaining // rcols
                found = True
    if rcols > 0:
        found = True
        avg = remaining // rcols
        while found:
            found = False
            for i in range(n):
                if i in info:
                    continue
                longest = 0
                for r in cells:
                    if i >= _len(r):
                        continue
                    for ln in r[i]:
                        if _width(ln) > avg:
                            for part in _split_line(ln, _max(1, avg)):
                                longest = _max(longest, _len(part.encode("utf-8")))
                        else:
                            longest = _max(longest, _len(ln.encode("utf-8")))
                if avg - longest >= 3 if avg >= longest else False:
                    info[i] = _max(1, longest)
                    remaining = _max(0, remaining - longest)
                    rcols -= 1
                    if rcols == 0:
                        break
                    avg = remaining // rcols
                    found = True
    if rcols > 0:
        if remaining < rcols:
            remaining = rcols
        avg = remaining // rcols
        excess = remaining - avg * rcols
        for i in range(n):
            if i in info:
                continue
            if excess > 0:
                excess -= 1
                info[i] = _max(1, avg + 1)
            else:
                info[i] = _max(1, avg)
    return [info[i] for i in range(n)], cells


def _render_table(header, rows, constraints, aligns, table_width):
    # comfy-table's Display (utils/formatting/borders.rs) with the chosen preset
    st = _preset(_CFG["formatting"])
    if _CFG["round_corners"] and not _CFG["formatting"].startswith("ASCII"):
        st[_TLC], st[_TRC], st[_BLC], st[_BRC] = "╭", "╮", "╰", "╯"

    def has(*ks):
        for k in ks:
            if st[k] is not None:
                return True
        return False

    def g(k):
        return st[k] if st[k] is not None else " "

    left = has(_TLC, _LB, _LBI, _LHI, _BLC)
    right = has(_TRC, _RB, _RBI, _RHI, _BRC)
    vert = has(_TBI, _MHI, _VL, _MI, _BBI)
    widths, cells = _arrange(header, rows, constraints, table_width, (left, right, vert))
    n = _len(widths)
    out = []

    def border(lc, fill, mid, rc, always_mid):
        s = g(lc) if left else ""
        parts = [g(fill) * (w + 2) for w in widths]
        s += (g(mid) if (always_mid or vert) else "").join(parts)
        if right:
            s += g(rc)
        return s

    def fmt_row(r):
        lines_per = []
        for i in range(n):
            cl = r[i] if i < _len(r) else [" " * widths[i]]
            ls = []
            for ln in cl:
                if _width(ln) > widths[i]:
                    ls.extend(_split_line(ln, widths[i]))
                else:
                    ls.append(ln)
            al = aligns[i] if aligns is not None else "left"
            al_ls = []
            for ln in ls:
                rem = _max(0, widths[i] - _width(ln))
                if al == "right":
                    ln = " " * rem + ln
                elif al == "center":
                    lp = (rem + 1) // 2
                    ln = " " * lp + ln + " " * (rem - lp)
                else:
                    ln = ln + " " * rem
                al_ls.append(" " + ln + " ")
            lines_per.append(al_ls)
        h = 0
        for x in lines_per:
            h = _max(h, _len(x))
        res = []
        for j in range(h):
            s = g(_LB) if left else ""
            for i in range(n):
                s += lines_per[i][j] if j < _len(lines_per[i]) else " " * (widths[i] + 2)
                if i == n - 1:
                    if right:
                        s += g(_RB)
                elif vert:
                    s += g(_VL)
            res.append(s)
        return res

    if has(_TLC, _TB, _TBI, _TRC):
        out.append(border(_TLC, _TB, _TBI, _TRC, True))
    body = cells[1:] if header is not None else cells
    if header is not None:
        out.extend(fmt_row(cells[0]))
        if has(_LHI, _HL, _MHI, _RHI):
            out.append(border(_LHI, _HL, _MHI, _RHI, False))
    for k, r in enumerate(body):
        out.extend(fmt_row(r))
        if k < _len(body) - 1 and has(_LBI, _HZ, _MI, _RBI):
            out.append(border(_LBI, _HZ, _MI, _RBI, False))
    if has(_BLC, _BB, _BBI, _BRC):
        out.append(border(_BLC, _BB, _BBI, _BRC, True))
    return "\n".join(out)


def _shape_str(h, w):
    return "(" + _group_int(str(h), "_") + ", " + _group_int(str(w), "_") + ")"


def _frame_str(df):
    cols = df._cols
    height = df.height
    width = _len(cols)
    ell = _ellipsis()
    max_cols = _CFG["cols"]
    max_rows = _CFG["rows"]
    str_trunc = _CFG["str_len"]
    padding = 2
    if width > max_cols:
        n_first, n_last = (max_cols + 1) // 2, max_cols // 2
    else:
        n_first, n_last = width, 0
    reduce_cols = n_first + n_last < width
    shown = list(range(n_first)) + ([None] if reduce_cols else []) + list(range(width - n_last, width))
    names = []
    name_lens = []
    for i in shown:
        if i is None:
            names.append(ell)
            name_lens.append(_len(ell))
            continue
        s = cols[i]
        nm = _trunc(s.name, str_trunc, ell)
        nl = _est_width(nm)
        cname = "" if _CFG["hide_names"] else nm
        if _CFG["hide_dtypes"]:
            cd = ""
        elif _CFG["inline_dtype"] or _CFG["hide_names"]:
            cd = _short(s.dtype)
        else:
            cd = "\n" + _short(s.dtype)
        dl = _len(cd.lstrip())
        sep = "\n---"
        if _CFG["hide_sep"] or _CFG["hide_names"] or _CFG["hide_dtypes"]:
            sep = ""
        if _CFG["inline_dtype"] and not _CFG["hide_dtypes"]:
            txt = cname + " (" + cd + ")"
            dl = _len(txt)
        else:
            txt = cname + sep + cd
        sl = _max(nl, dl)
        if sl < _est_width(sep.strip()):
            sl = _est_width(sep.strip())
        names.append(txt)
        name_lens.append(sl + padding)
    max_elem = [0] * _len(shown)

    def row_strings(r):
        out = []
        for j, i in enumerate(shown):
            if i is None:
                out.append(ell)
                max_elem[j] = _len(ell) + padding
                continue
            s = cols[i]
            v = _any_str(s._v[r], s.dtype, False)
            v = _trunc(v, str_trunc, ell)
            l = _est_width(v) + padding
            if max_elem[j] < l:
                max_elem[j] = l
            out.append(v)
        return out

    rows = []
    if max_rows > 0:
        if height > max_rows:
            half, rest = max_rows // 2, max_rows % 2
            for r in range(half + rest):
                rows.append(row_strings(r))
            rows.append([ell] * _len(rows[0]))
            for r in range(height - half, height):
                rows.append(row_strings(r))
        elif width > 0:
            for r in range(height):
                rows.append(row_strings(r))
    elif height > 0:
        rows.append([ell] * width)
    min_w = _max(5, 3 + padding)
    cons = []
    for j in range(_len(shown)):
        mx = _min(str_trunc + _len(ell) + padding, _max(name_lens[j], max_elem[j]))
        truncated = max_rows > 0 and height > max_rows
        if mx <= min_w and not truncated:
            cons.append(("abs", mx, mx))
        elif mx <= min_w:
            cons.append(("bounds", mx, min_w))
        else:
            cons.append(("bounds", min_w, mx))
    header = None if (_CFG["hide_names"] and _CFG["hide_dtypes"]) else names
    if header is None:
        cons = [(k, lo, hi) for (k, lo, hi) in cons]
    tw = _CFG["width"]
    if tw is None:
        tw = 100
    aligns = None
    if _CFG["align"] is not None or _CFG["num_align"] is not None:
        sp = _CFG["align"] or "DEFAULT"
        npre = _CFG["num_align"] or sp
        aligns = []
        for i in shown:
            p = sp
            if i is not None and cols[i].dtype.is_numeric():
                p = npre
            aligns.append({"RIGHT": "right", "LEFT": "left", "CENTER": "center"}.get(p, "left"))
    tbl = _render_table(header, rows, cons, aligns, tw)
    if _CFG["hide_shape"]:
        return tbl
    shp = _shape_str(height, width)
    if _CFG["shape_below"]:
        return tbl + "\nshape: " + shp
    return "shape: " + shp + "\n" + tbl


def _series_str(s):
    n = _len(s._v)
    out = ["shape: (" + _group_int(str(n), "_") + ",)\nSeries: '" + s.name + "' [" + _short(s.dtype) + "]\n[\n"]
    ell = _ellipsis()
    trunc = s.dtype._kind == "str"
    tl = _CFG["str_len"]

    def one(v):
        t = _any_str(v, s.dtype, True)
        if trunc and v is not None:
            if _len(v) > tl:
                return "\t\"" + v[:tl] + ell + "\n"
        return "\t" + t + "\n"

    lim = _CFG["rows"]
    if n > lim:
        half, rest = lim // 2, lim % 2
        for i in range(half + rest):
            out.append(one(s._v[i]))
        out.append("\t" + ell + "\n")
        for i in range(n - half, n):
            out.append(one(s._v[i]))
    else:
        for v in s._v:
            out.append(one(v))
    out.append("]")
    return "".join(out)


# ---------- numeric kernels (polars-compute) ----------


def _fsum(xs):
    # polars-compute float_sum: pairwise over blocks of 128 (16 lanes each), the remainder added first-to-last
    n = _len(xs)
    rem = n % 128
    rest = 0.0
    for i in range(rem):
        rest = rest + xs[i]
    if n == rem:
        return 0.0 + rest
    return _pairwise(xs, rem, n) + rest


def _pairwise(xs, a, b):
    if b - a == 128:
        v = [0.0] * 16
        for c in range(a, b, 16):
            for j in range(16):
                v[j] = v[j] + xs[c + j]
        w = 16
        while w > 4:
            h = w // 2
            for j in range(h):
                v[j] = v[j] + v[h + j]
            w = h
        return (v[0] + v[2]) + (v[1] + v[3])
    blocks = (b - a) // 128
    mid = a + (blocks // 2) * 128
    return _pairwise(xs, a, mid) + _pairwise(xs, mid, b)


def _var_state(xs):
    # polars-compute moment.rs VarState over chunks of 128: (weight, mean, dp)
    w, mean, dp = 0.0, 0.0, 0.0
    for c in range(0, _len(xs), 128):
        ch = xs[c:c + 128]
        cw = float(_len(ch))
        s = -0.0
        for x in ch:
            s = s + x
        cm = s / cw
        d = -0.0
        for x in ch:
            d = d + (x - cm) * (x - cm)
        if w == 0.0:
            w, mean, dp = cw, cm, d
            continue
        nw = w + cw
        frac = cw / nw
        delta = cm - mean
        nm = mean + delta * frac
        dp = dp + (d + cw * (cm - nm) * delta)
        w, mean = nw, nm
    return w, mean, dp


def _var(xs, ddof):
    w, mean, dp = _var_state(xs)
    if w <= ddof:
        return None
    v = dp / (w - ddof)
    return 0.0 if v < 0.0 else v


def _nonnull(vals):
    return [v for v in vals if v is not None]


def _floats(vals):
    return [float(v) for v in vals if v is not None]


def _quantile(vals, q, interp):
    xs = sorted(_floats(vals), key=_nan_key)
    n = _len(xs)
    if n == 0:
        return None
    if not (0.0 <= q <= 1.0):
        raise ComputeError("quantile should be between 0.0 and 1.0")
    pos = q * (n - 1)
    if interp == "nearest":
        i = int(_round_half_away(pos))
        return xs[_min(i, n - 1)]
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if interp == "lower":
        return xs[lo]
    if interp == "higher":
        return xs[hi]
    if interp == "midpoint":
        if lo == hi:
            return xs[lo]
        return (xs[lo] + xs[hi]) / 2.0
    if interp == "equiprobable":
        i = int(math.ceil(n * q)) - 1
        return xs[_max(0, _min(i, n - 1))]
    if lo == hi:
        return xs[lo]
    f = pos - lo
    return xs[lo] + (xs[hi] - xs[lo]) * f


def _powi(a, b):
    # Rust's f64::powi (compiler-rt __powidf2: square and multiply)
    r = 1.0
    while True:
        if b & 1:
            r *= a
        b //= 2
        if b == 0:
            return r
        a *= a


def _round_half_away(x):
    # Rust's f64::round: half away from zero, exact (floor(x + 0.5) is wrong for 0.49999999999999994)
    a = _abs(x)
    if a >= 4503599627370496.0:
        return x
    r = math.floor(a)
    if a - r >= 0.5:
        r += 1
    return math.copysign(float(r), x)


def _nan_key(x):
    return (1, 0.0) if x != x else (0, x)


# ---------- Series ----------

# Series(_RAW) / DataFrame(_RAW): an empty object for the module to fill in (dynamic builds have no __new__)
_RAW = object()


def _new(name, vals, dtype, lit=None):
    s = Series(_RAW)
    s.name = name
    s._v = vals
    s.dtype = dtype
    s._lit = lit
    return s


def _from_values(name, values, dtype=None, strict=True):
    if isinstance(values, Series):
        vals = list(values._v)
        src = values.dtype
    else:
        vals = []
        for v in values:
            if isinstance(v, Series):
                v = v.to_list()
            elif isinstance(v, tuple):
                v = list(v)
            vals.append(v)
        src = None
    if dtype is None:
        if src is not None:
            return _new(name, vals, src)
        # as polars: the first value's dtype; an int column takes bools, a float column ints and bools
        first = next((v for v in vals if v is not None), None)
        fd = _py_dtype(first)
        if fd._kind in ("int", "float", "bool", "str"):
            for v in vals:
                if v is None:
                    continue
                vd = _py_dtype(v)
                if isinstance(v, int) and not (v is True or v is False) and not (_I64_MIN <= v <= _I64_MAX):
                    vd = _Int128Name
                ok = vd == fd or (fd._kind == "int" and vd._kind == "bool") or (fd._kind == "float" and vd._kind in ("int", "bool"))
                if not ok or vd is _Int128Name:
                    raise TypeError("unexpected value while building Series of type " + repr(fd) + "; found value of type " + repr(vd) + ": " + _repr_value(v)
                                    + "\n\nThis error occurred with the following context stack:\n\t[1] while constructing Series '" + name + "'\n\n\nHint: Try setting `strict=False` to allow passing data with mixed types.")
            d = fd
        else:
            d = _infer(vals)
        out = []
        for v in vals:
            ok, y = _cast_value(v, d, True)
            if not ok:
                raise TypeError("unexpected value while building Series of type " + repr(d) + "; found value of type " + repr(type(v).__name__) + ": " + _repr_value(v))
            out.append(y)
        return _new(name, out, d)
    d = _dt(dtype)
    out = []
    for v in vals:
        ok, y = _cast_value(v, d, strict)
        if not ok:
            if strict:
                raise TypeError("unexpected value while building Series of type " + repr(d) + "; found value of type " + repr(type(v).__name__) + ": " + _repr_value(v) + "\n\nHint: Try setting `strict=False` to allow passing data with mixed types.")
            y = None
        out.append(y)
    return _new(name, out, d)


class Series:
    def __init__(self, name=None, values=None, dtype=None, strict=True, nan_to_null=False):
        if name is _RAW:
            return
        if name is not None and not isinstance(name, str):
            if values is not None and isinstance(values, (DataType, type)):
                dtype = values
            values, name = name, ""
        if name is None:
            name = ""
        if values is None:
            values = []
        if isinstance(values, range):
            values = list(values)
        s = _from_values(name, values, dtype, strict)
        if nan_to_null and s.dtype._kind == "float":
            s._v = [None if (v is not None and v != v) else v for v in s._v]
        self.name = s.name
        self._v = s._v
        self.dtype = s.dtype
        self._lit = None

    # -- basics --
    def __repr__(self):
        return _series_str(self)

    def __str__(self):
        return _series_str(self)

    def __len__(self):
        return _len(self._v)

    def len(self):
        return _len(self._v)

    @property
    def shape(self):
        return (_len(self._v),)

    def __iter__(self):
        return iter(list(self._v))

    def __bool__(self):
        raise TypeError("the truth value of a Series is ambiguous\n\nHint: use '&' or '|' to chain Series boolean results together, not and/or; to check if a Series contains any values, use `is_empty()`.")

    def __contains__(self, item):
        return item in self._v

    def __getitem__(self, k):
        if isinstance(k, bool):
            raise TypeError("cannot use a boolean to index a Series")
        if isinstance(k, int):
            n = _len(self._v)
            i = k + n if k < 0 else k
            if i < 0 or i >= n:
                raise IndexError("index " + str(k) + " is out of bounds for sequence of length " + str(n))
            return self._v[i]
        if isinstance(k, slice):
            return _new(self.name, self._v[k], self.dtype)
        if isinstance(k, Series):
            k = k._v
        if isinstance(k, (list, tuple, range)):
            k = list(k)
            if _len(k) > 0 and (k[0] is True or k[0] is False):
                return _new(self.name, [v for v, m in zip(self._v, k) if m], self.dtype)
            n = _len(self._v)
            out = []
            for i in k:
                j = i + n if i < 0 else i
                if j < 0 or j >= n:
                    raise OutOfBoundsError("gather indices are out of bounds")
                out.append(self._v[j])
            return _new(self.name, out, self.dtype)
        raise TypeError("cannot select elements using key of type " + repr(type(k).__name__) + ": " + repr(k))

    def to_list(self):
        return [(list(v) if isinstance(v, list) else v) for v in self._v]

    def to_frame(self, name=None):
        return DataFrame._of([_new(self.name if name is None else name, list(self._v), self.dtype)])

    def alias(self, name):
        return _new(name, list(self._v), self.dtype)

    def rename(self, name):
        return self.alias(name)

    def clone(self):
        return _new(self.name, list(self._v), self.dtype)

    def is_empty(self):
        return _len(self._v) == 0

    def null_count(self):
        return _sum(1 for v in self._v if v is None)

    def has_nulls(self):
        return any(v is None for v in self._v)

    def item(self, index=None):
        if index is None:
            if _len(self._v) != 1:
                raise ValueError("can only call '.item()' if the Series is of length 1, or an explicit index is provided (Series is of length " + str(_len(self._v)) + ")")
            return self._v[0]
        return self[index]

    def head(self, n=10):
        if n < 0:
            n = _max(0, _len(self._v) + n)
        return _new(self.name, self._v[:n], self.dtype)

    def tail(self, n=10):
        if n < 0:
            n = _max(0, _len(self._v) + n)
        return _new(self.name, self._v[_len(self._v) - n:] if n else [], self.dtype)

    def limit(self, n=10):
        return self.head(n)

    def slice(self, offset, length=None):
        n = _len(self._v)
        o = offset + n if offset < 0 else offset
        o = _max(0, _min(o, n))
        e = n if length is None else _min(n, o + length)
        return _new(self.name, self._v[o:e], self.dtype)

    def append(self, other):
        if other.dtype != self.dtype and other.dtype._kind != "null" and self.dtype._kind != "null":
            raise SchemaError("type " + repr(other.dtype) + " is incompatible with expected type " + repr(self.dtype))
        self._v = self._v + list(other._v)
        if self.dtype._kind == "null":
            self.dtype = other.dtype
        return self

    def extend(self, other):
        return self.append(other)

    def equals(self, other, check_dtypes=False, check_names=False, null_equal=True):
        if check_names and self.name != other.name:
            return False
        if check_dtypes and self.dtype != other.dtype:
            return False
        if _len(self._v) != _len(other._v):
            return False
        for a, b in zip(self._v, other._v):
            if a is None or b is None:
                if not (a is None and b is None and null_equal):
                    return False
            elif a != b and not (a != a and b != b):
                return False
        return True

    def filter(self, predicate):
        if isinstance(predicate, Expr):
            return self._x(lambda e: e.filter(predicate))
        m = predicate._v if isinstance(predicate, Series) else list(predicate)
        return _new(self.name, [v for v, k in zip(self._v, m) if k], self.dtype)

    def _x(self, f):
        # evaluate an expression over this Series (as column self.name of a one-column frame)
        fr = _Frame.of([self])
        e = f(col(self.name))
        r = _eval(e, fr)
        return _new(r.name, r._v, r.dtype)

    def _agg(self, f):
        r = self._x(f)
        return r._v[0] if _len(r._v) else None

    # -- aggregations returning Python values --
    def sum(self):
        return self._agg(lambda e: e.sum())

    def mean(self):
        return self._agg(lambda e: e.mean())

    def median(self):
        return self._agg(lambda e: e.median())

    def min(self):
        return self._agg(lambda e: e.min())

    def max(self):
        return self._agg(lambda e: e.max())

    def std(self, ddof=1):
        return self._agg(lambda e: e.std(ddof))

    def var(self, ddof=1):
        return self._agg(lambda e: e.var(ddof))

    def product(self):
        return self._agg(lambda e: e.product())

    def quantile(self, quantile, interpolation="nearest"):
        return self._agg(lambda e: e.quantile(quantile, interpolation))

    def count(self):
        return self._agg(lambda e: e.count())

    def n_unique(self):
        return self._agg(lambda e: e.n_unique())

    def first(self):
        return self._v[0] if self._v else None

    def last(self):
        return self._v[-1] if self._v else None

    def arg_min(self):
        return self._agg(lambda e: e.arg_min())

    def arg_max(self):
        return self._agg(lambda e: e.arg_max())

    def any(self, ignore_nulls=True):
        return self._agg(lambda e: e.any(ignore_nulls))

    def all(self, ignore_nulls=True):
        return self._agg(lambda e: e.all(ignore_nulls))

    def nan_max(self):
        return self._agg(lambda e: e.nan_max())

    def nan_min(self):
        return self._agg(lambda e: e.nan_min())

    def implode(self):
        return _new(self.name, [list(self._v)], _ListType(self.dtype))

    def value_counts(self, sort=False, parallel=False, name=None, normalize=False):
        return self.to_frame().select(col(self.name).value_counts(sort=sort, name=name, normalize=normalize)).unnest(self.name)

    def describe(self, percentiles=(0.25, 0.5, 0.75), interpolation="nearest"):
        d = self.to_frame().describe(percentiles=percentiles, interpolation=interpolation)
        return d.rename({self.name: "value"}) if self.name != "value" else d

    def to_dummies(self, separator="_", drop_first=False):
        return self.to_frame().to_dummies(separator=separator, drop_first=drop_first)

    def map_elements(self, function, return_dtype=None, skip_nulls=True):
        return self._x(lambda e: e.map_elements(function, return_dtype=return_dtype, skip_nulls=skip_nulls))

    def apply(self, function, return_dtype=None, skip_nulls=True):
        return self.map_elements(function, return_dtype, skip_nulls)

    def sort(self, descending=False, nulls_last=False, multithreaded=True, in_place=False):
        r = self._x(lambda e: e.sort(descending=descending, nulls_last=nulls_last))
        if in_place:
            self._v = r._v
            return self
        return r

    def cast(self, dtype, strict=True, wrap_numerical=False):
        return self._x(lambda e: e.cast(dtype, strict=strict, wrap_numerical=wrap_numerical))

    def to_numpy(self):
        raise NotImplementedError("brevi: Series.to_numpy() is not in Brevi's polars")

    def __array__(self):
        raise NotImplementedError("brevi: numpy arrays from a polars Series are not in Brevi's polars")

    @property
    def str(self):
        return _SeriesNS(self, "str")

    @property
    def list(self):
        return _SeriesNS(self, "list")

    @property
    def arr(self):
        return _SeriesNS(self, "list")

    # -- operators: through expressions, as polars' own Series methods --
    def _bin(self, other, op, rev=False):
        if isinstance(other, Expr):
            return NotImplemented
        if isinstance(other, Series):
            o = other
        elif isinstance(other, (list, tuple)):
            raise TypeError("cannot do arithmetic with Series of dtype: " + repr(self.dtype) + " and argument of type: 'list'")
        else:
            if other is None and op in ("==", "!=", "<", ">", "<=", ">="):
                return _new(self.name, [None] * _len(self._v), Boolean)
            o = _lit_series(other)
            if self.dtype._kind == "bool" and op in ("+", "-", "*", "/", "//", "%") and (other is True or other is False):
                raise TypeError("cannot do arithmetic with Series of dtype: Boolean and argument of type: 'bool'")
        a, b = (o, self) if rev else (self, o)
        r = _binop(op, a, b)
        return _new(self.name, r._v, r.dtype)

    def __add__(self, o):
        return self._bin(o, "+")

    def __radd__(self, o):
        return self._bin(o, "+", True)

    def __sub__(self, o):
        return self._bin(o, "-")

    def __rsub__(self, o):
        return self._bin(o, "-", True)

    def __mul__(self, o):
        return self._bin(o, "*")

    def __rmul__(self, o):
        return self._bin(o, "*", True)

    def __truediv__(self, o):
        return self._bin(o, "/")

    def __rtruediv__(self, o):
        return self._bin(o, "/", True)

    def __floordiv__(self, o):
        return self._bin(o, "//")

    def __rfloordiv__(self, o):
        return self._bin(o, "//", True)

    def __mod__(self, o):
        return self._bin(o, "%")

    def __rmod__(self, o):
        return self._bin(o, "%", True)

    def __pow__(self, o):
        return self._bin(o, "**")

    def __rpow__(self, o):
        return self._bin(o, "**", True)

    def __and__(self, o):
        return self._bin(o, "&")

    def __rand__(self, o):
        return self._bin(o, "&", True)

    def __or__(self, o):
        return self._bin(o, "|")

    def __ror__(self, o):
        return self._bin(o, "|", True)

    def __xor__(self, o):
        return self._bin(o, "^")

    def __rxor__(self, o):
        return self._bin(o, "^", True)

    def __eq__(self, o):
        return self._bin(o, "==")

    def __ne__(self, o):
        return self._bin(o, "!=")

    def __lt__(self, o):
        return self._bin(o, "<")

    def __le__(self, o):
        return self._bin(o, "<=")

    def __gt__(self, o):
        return self._bin(o, ">")

    def __ge__(self, o):
        return self._bin(o, ">=")

    def __neg__(self):
        return self._x(lambda e: -e)

    def __pos__(self):
        return self

    def __abs__(self):
        return self._x(lambda e: e.abs())

    def __invert__(self):
        return self._x(lambda e: ~e)

    def __hash__(self):
        return id(self)

    def eq(self, o):
        return self.__eq__(o)

    def ne(self, o):
        return self.__ne__(o)

    def lt(self, o):
        return self.__lt__(o)

    def le(self, o):
        return self.__le__(o)

    def gt(self, o):
        return self.__gt__(o)

    def ge(self, o):
        return self.__ge__(o)

    def add(self, o):
        return self.__add__(o)

    def sub(self, o):
        return self.__sub__(o)

    def mul(self, o):
        return self.__mul__(o)

    def truediv(self, o):
        return self.__truediv__(o)

    def floordiv(self, o):
        return self.__floordiv__(o)

    def mod(self, o):
        return self.__mod__(o)

    def pow(self, o):
        return self.__pow__(o)

    def eq_missing(self, o):
        return self._x(lambda e: e.eq_missing(o))

    def ne_missing(self, o):
        return self._x(lambda e: e.ne_missing(o))


class _SeriesNS:
    def __init__(self, s, ns):
        self._s = s
        self._ns = ns

    def __getattr__(self, name):
        s = self._s
        ns = self._ns

        def call(*a, **k):
            r = s._x(lambda e: getattr(getattr(e, ns), name)(*a, **k))
            if name in _NS_SCALAR.get(ns, ()) and name != "join":
                return r._v[0] if r._v else None
            return r
        return call


# Series methods that are the expression's, evaluated over the Series
def _series_method(name):
    def m(self, *a, **k):
        return self._x(lambda e: getattr(e, name)(*a, **k))
    return m


for _m in ("abs", "sqrt", "cbrt", "exp", "log", "log10", "log1p", "sin", "cos", "tan", "arcsin", "arccos", "arctan", "sinh", "cosh",
           "tanh", "floor", "ceil", "round", "round_sig_figs", "sign", "clip", "cum_sum", "cum_prod", "cum_min", "cum_max", "cum_count",
           "diff", "pct_change", "shift", "fill_null", "fill_nan", "is_null", "is_not_null", "is_nan", "is_not_nan", "is_finite",
           "is_infinite", "is_in", "is_between", "is_duplicated", "is_unique", "is_first_distinct", "is_last_distinct", "unique",
           "unique_counts", "drop_nulls", "drop_nans", "reverse", "rank", "arg_sort", "arg_unique", "mode", "top_k", "bottom_k",
           "explode", "interpolate", "forward_fill", "backward_fill", "gather", "gather_every", "replace", "replace_strict",
           "not_", "rolling_mean", "rolling_sum", "rolling_min", "rolling_max", "repeat_by", "hash"):
    if not hasattr(Series, _m):
        setattr(Series, _m, _series_method(_m))


def _series_scalar(name):
    # Series.skew() and the like give the value, not a Series
    def m(self, *a, **k):
        return self._x(lambda e: getattr(e, name)(*a, **k))._v[0]
    return m


for _m in ("dot", "entropy", "kurtosis", "skew", "approx_n_unique"):
    if not hasattr(Series, _m):
        setattr(Series, _m, _series_scalar(_m))


def _is_sorted(self, descending=False, nulls_last=False):
    v = self._v
    nn = [x for x in v if x is not None]
    nulls = _len(v) - _len(nn)
    if nulls:
        lead = v[:nulls] == [None] * nulls
        trail = v[_len(v) - nulls:] == [None] * nulls
        if not (trail if nulls_last else lead):
            return False
    key = _nan_key if self.dtype._kind == "float" else (lambda x: x)
    for i in range(1, _len(nn)):
        a, b = key(nn[i - 1]), key(nn[i])
        if (a < b) if descending else (a > b):
            return False
    return True


Series.is_sorted = _is_sorted


# ---------- expressions ----------


class Expr:
    __slots__ = ("_op", "_a")

    def __init__(self, op, *args):
        self._op = op
        self._a = args

    def __repr__(self):
        return "<Expr ['" + _expr_str(self) + "'] at 0x" + ("%X" % (id(self) & 0xFFFFFFFFFFFF)) + ">"

    def __str__(self):
        return _expr_str(self)

    def __bool__(self):
        raise TypeError("the truth value of an Expr is ambiguous\n\nYou probably got here by using a Python standard library function instead of the native expressions API.\nHere are some things you might want to try:\n- instead of `pl.col('a') and pl.col('b')`, use `pl.col('a') & pl.col('b')`\n- instead of `pl.col('a') in [y, z]`, use `pl.col('a').is_in([y, z])`\n- instead of `max(pl.col('a'), pl.col('b'))`, use `pl.max_horizontal(pl.col('a'), pl.col('b'))`\n")

    def __hash__(self):
        return id(self)

    def _f(self, name, *args, **kw):
        return Expr("fn", name, self, args, kw)

    # -- operators --
    def __add__(self, o):
        return Expr("bin", "+", self, _x(o, True))

    def __radd__(self, o):
        return Expr("bin", "+", _x(o, True), self)

    def __sub__(self, o):
        return Expr("bin", "-", self, _x(o))

    def __rsub__(self, o):
        return Expr("bin", "-", _x(o), self)

    def __mul__(self, o):
        return Expr("bin", "*", self, _x(o))

    def __rmul__(self, o):
        return Expr("bin", "*", _x(o), self)

    def __truediv__(self, o):
        return Expr("bin", "/", self, _x(o))

    def __rtruediv__(self, o):
        return Expr("bin", "/", _x(o), self)

    def __floordiv__(self, o):
        return Expr("bin", "//", self, _x(o))

    def __rfloordiv__(self, o):
        return Expr("bin", "//", _x(o), self)

    def __mod__(self, o):
        return Expr("bin", "%", self, _x(o))

    def __rmod__(self, o):
        return Expr("bin", "%", _x(o), self)

    def __pow__(self, o):
        return Expr("bin", "**", self, _x(o))

    def __rpow__(self, o):
        return Expr("bin", "**", _x(o), self)

    def __and__(self, o):
        return Expr("bin", "&", self, _x(o))

    def __rand__(self, o):
        return Expr("bin", "&", _x(o), self)

    def __or__(self, o):
        return Expr("bin", "|", self, _x(o))

    def __ror__(self, o):
        return Expr("bin", "|", _x(o), self)

    def __xor__(self, o):
        return Expr("bin", "^", self, _x(o))

    def __rxor__(self, o):
        return Expr("bin", "^", _x(o), self)

    def __eq__(self, o):
        return Expr("bin", "==", self, _x(o, True))

    def __ne__(self, o):
        return Expr("bin", "!=", self, _x(o, True))

    def __lt__(self, o):
        return Expr("bin", "<", self, _x(o, True))

    def __le__(self, o):
        return Expr("bin", "<=", self, _x(o, True))

    def __gt__(self, o):
        return Expr("bin", ">", self, _x(o, True))

    def __ge__(self, o):
        return Expr("bin", ">=", self, _x(o, True))

    def __neg__(self):
        return Expr("bin", "-", Expr("lit", 0, None), self) if False else self._f("neg")

    def __pos__(self):
        return self

    def __abs__(self):
        return self._f("abs")

    def __invert__(self):
        return self._f("not")

    def add(self, o):
        return self.__add__(o)

    def sub(self, o):
        return self.__sub__(o)

    def mul(self, o):
        return self.__mul__(o)

    def truediv(self, o):
        return self.__truediv__(o)

    def floordiv(self, o):
        return self.__floordiv__(o)

    def mod(self, o):
        return self.__mod__(o)

    def pow(self, o):
        return self.__pow__(o)

    def eq(self, o):
        return self.__eq__(o)

    def ne(self, o):
        return self.__ne__(o)

    def lt(self, o):
        return self.__lt__(o)

    def le(self, o):
        return self.__le__(o)

    def gt(self, o):
        return self.__gt__(o)

    def ge(self, o):
        return self.__ge__(o)

    def and_(self, *o):
        r = self
        for x in o:
            r = r & x
        return r

    def or_(self, *o):
        r = self
        for x in o:
            r = r | x
        return r

    def xor(self, o):
        return self ^ o

    def not_(self):
        return self._f("not")

    def neg(self):
        return self._f("neg")

    def eq_missing(self, o):
        return Expr("bin", "eqm", self, _x(o, True))

    def ne_missing(self, o):
        return Expr("bin", "nem", self, _x(o, True))

    # -- naming --
    def alias(self, name):
        return Expr("alias", self, name)

    @property
    def name(self):
        return _NameNS(self)

    # -- elementwise --
    def cast(self, dtype, strict=True, wrap_numerical=False):
        return Expr("cast", self, _dt(dtype), strict, wrap_numerical)

    def abs(self):
        return self._f("abs")

    def sqrt(self):
        return self._f("math", "sqrt")

    def cbrt(self):
        return self._f("math", "cbrt")

    def exp(self):
        return self._f("math", "exp")

    def log(self, base=math.e):
        return self._f("log", base)

    def log10(self):
        return self._f("log", 10.0)

    def log1p(self):
        return self._f("math", "log1p")

    def sin(self):
        return self._f("math", "sin")

    def cos(self):
        return self._f("math", "cos")

    def tan(self):
        return self._f("math", "tan")

    def arcsin(self):
        return self._f("math", "asin")

    def arccos(self):
        return self._f("math", "acos")

    def arctan(self):
        return self._f("math", "atan")

    def sinh(self):
        return self._f("math", "sinh")

    def cosh(self):
        return self._f("math", "cosh")

    def tanh(self):
        return self._f("math", "tanh")

    def degrees(self):
        return self._f("math", "degrees")

    def radians(self):
        return self._f("math", "radians")

    def floor(self):
        return self._f("floor")

    def ceil(self):
        return self._f("ceil")

    def round(self, decimals=0, mode="half_to_even"):
        return self._f("round", decimals, mode)

    def round_sig_figs(self, digits):
        return self._f("round_sig", digits)

    def sign(self):
        return self._f("sign")

    def clip(self, lower_bound=None, upper_bound=None):
        return self._f("clip", _xo(lower_bound), _xo(upper_bound))

    def clip_min(self, lower_bound):
        return self.clip(lower_bound, None)

    def clip_max(self, upper_bound):
        return self.clip(None, upper_bound)

    def is_null(self):
        return self._f("is_null")

    def is_not_null(self):
        return self._f("is_not_null")

    def is_nan(self):
        return self._f("is_nan")

    def is_not_nan(self):
        return self._f("is_not_nan")

    def is_finite(self):
        return self._f("is_finite")

    def is_infinite(self):
        return self._f("is_infinite")

    def fill_null(self, value=None, strategy=None, limit=None):
        if value is None and strategy is None:
            raise ValueError("must specify either a fill `value` or `strategy`")
        if strategy is not None:
            return self._f("fill_null_strategy", strategy, limit)
        return self._f("fill_null", _x(value, True))

    def fill_nan(self, value):
        return self._f("fill_nan", _x(value, True))

    def forward_fill(self, limit=None):
        return self._f("fill_null_strategy", "forward", limit)

    def backward_fill(self, limit=None):
        return self._f("fill_null_strategy", "backward", limit)

    def is_in(self, other, nulls_equal=False):
        if isinstance(other, (list, tuple, set, frozenset, range)):
            other = Expr("lit", Series("", list(other)), None)
        elif isinstance(other, Series):
            other = Expr("lit", other, None)
        return self._f("is_in", other, nulls_equal)

    def is_between(self, lower_bound, upper_bound, closed="both"):
        lo, hi = _x(lower_bound, True), _x(upper_bound, True)
        if closed == "both":
            return (self >= lo) & (self <= hi)
        if closed == "left":
            return (self >= lo) & (self < hi)
        if closed == "right":
            return (self > lo) & (self <= hi)
        return (self > lo) & (self < hi)

    def replace(self, old, new=None, default=None, return_dtype=None):
        if isinstance(old, dict):
            mapping = old
        elif isinstance(old, (list, tuple, Series)):
            olds = list(old)
            news = list(new) if isinstance(new, (list, tuple, Series)) else [new] * _len(olds)
            mapping = dict(zip(olds, news))
        else:
            mapping = {old: new}
        return self._f("replace", mapping, False, default, return_dtype)

    def replace_strict(self, old, new=None, default=None, return_dtype=None):
        if isinstance(old, dict):
            mapping = old
        elif isinstance(old, (list, tuple, Series)):
            olds = list(old)
            news = list(new) if isinstance(new, (list, tuple, Series)) else [new] * _len(olds)
            mapping = dict(zip(olds, news))
        else:
            mapping = {old: new}
        return self._f("replace", mapping, True, default, return_dtype)

    def map_elements(self, function, return_dtype=None, skip_nulls=True, pass_name=False, strategy="thread_local", returns_scalar=False):
        return self._f("map_elements", function, return_dtype, skip_nulls)

    def map_batches(self, function, return_dtype=None, agg_list=False, is_elementwise=False, returns_scalar=False):
        return self._f("map_batches", function, return_dtype, returns_scalar)

    def pipe(self, function, *args, **kwargs):
        return function(self, *args, **kwargs)

    def hash(self, seed=0, seed_1=None, seed_2=None, seed_3=None):
        raise NotImplementedError("brevi: Expr.hash is not in Brevi's polars")

    # -- window / ordering --
    def shift(self, n=1, fill_value=None):
        return self._f("shift", n, fill_value)

    def diff(self, n=1, null_behavior="ignore"):
        return self._f("diff", n)

    def pct_change(self, n=1):
        return self._f("pct_change", n)

    def cum_sum(self, reverse=False):
        return self._f("cum", "sum", reverse)

    def cumsum(self, reverse=False):
        return self.cum_sum(reverse)

    def cum_prod(self, reverse=False):
        return self._f("cum", "prod", reverse)

    def cum_min(self, reverse=False):
        return self._f("cum", "min", reverse)

    def cum_max(self, reverse=False):
        return self._f("cum", "max", reverse)

    def cum_count(self, reverse=False):
        return self._f("cum", "count", reverse)

    def rank(self, method="average", descending=False, seed=None):
        return self._f("rank", method, descending)

    def sort(self, descending=False, nulls_last=False):
        return self._f("sort", descending, nulls_last)

    def sort_by(self, by, *more_by, descending=False, nulls_last=False, multithreaded=True, maintain_order=False):
        bys = _flat_exprs([by] + list(more_by))
        return Expr("sort_by", self, bys, descending, nulls_last)

    def arg_sort(self, descending=False, nulls_last=False):
        return self._f("arg_sort", descending, nulls_last)

    def reverse(self):
        return self._f("reverse")

    def unique(self, maintain_order=False):
        return self._f("unique")

    def unique_counts(self):
        return self._f("unique_counts")

    def arg_unique(self):
        return self._f("arg_unique")

    def is_duplicated(self):
        return self._f("is_duplicated")

    def is_unique(self):
        return self._f("is_unique")

    def is_first_distinct(self):
        return self._f("is_first_distinct")

    def is_last_distinct(self):
        return self._f("is_last_distinct")

    def drop_nulls(self):
        return self._f("drop_nulls")

    def drop_nans(self):
        return self._f("drop_nans")

    def head(self, n=10):
        return self._f("head", n)

    def tail(self, n=10):
        return self._f("tail", n)

    def limit(self, n=10):
        return self.head(n)

    def slice(self, offset, length=None):
        return self._f("slice", offset, length)

    def gather(self, indices):
        return self._f("gather", _x(indices if not isinstance(indices, (list, tuple)) else Series("", list(indices)), True))

    def get(self, index):
        return self._f("get", _x(index, True))

    def gather_every(self, n, offset=0):
        return self._f("gather_every", n, offset)

    def top_k(self, k=5):
        return self._f("top_k", k, True)

    def bottom_k(self, k=5):
        return self._f("top_k", k, False)

    def mode(self):
        return self._f("mode")

    def exclude(self, columns, *more_columns):
        drop = (list(columns) if isinstance(columns, (list, tuple)) else [columns]) + list(more_columns)
        if self._op == "cols":
            return Expr("cols", ("minus", (self._a[0], drop)))
        if self._op == "col":
            return Expr("cols", ("minus", (("names", [self._a[0]]), drop)))
        raise InvalidOperationError("exclude is only allowed on column selections")

    def explode(self, *, empty_as_null=False, keep_nulls=True):
        return self._f("explode", bool(empty_as_null), keep_nulls)

    def flatten(self):
        return self._f("explode", False, True)

    def interpolate(self, method="linear"):
        return self._f("interpolate", method)

    def filter(self, *predicates, **constraints):
        p = _all_pred(predicates, constraints)
        return Expr("filter", self, p)

    def where(self, predicate):
        return self.filter(predicate)

    def over(self, partition_by=None, *more_exprs, order_by=None, mapping_strategy="group_to_rows"):
        keys = _flat_exprs(([partition_by] if partition_by is not None else []) + list(more_exprs))
        ob = _flat_exprs([order_by]) if order_by is not None else None
        return Expr("over", self, keys, ob, mapping_strategy)

    def rolling_mean(self, window_size, weights=None, min_samples=None, center=False):
        return self._f("rolling", "mean", window_size, min_samples, center)

    def rolling_sum(self, window_size, weights=None, min_samples=None, center=False):
        return self._f("rolling", "sum", window_size, min_samples, center)

    def rolling_min(self, window_size, weights=None, min_samples=None, center=False):
        return self._f("rolling", "min", window_size, min_samples, center)

    def rolling_max(self, window_size, weights=None, min_samples=None, center=False):
        return self._f("rolling", "max", window_size, min_samples, center)

    def repeat_by(self, by):
        return self._f("repeat_by", _x(by, True))

    # -- aggregations --
    def sum(self):
        return self._f("sum")

    def mean(self):
        return self._f("mean")

    def median(self):
        return self._f("median")

    def min(self):
        return self._f("min")

    def max(self):
        return self._f("max")

    def nan_min(self):
        return self._f("nan_min")

    def nan_max(self):
        return self._f("nan_max")

    def std(self, ddof=1):
        return self._f("std", ddof)

    def var(self, ddof=1):
        return self._f("var", ddof)

    def product(self):
        return self._f("product")

    def quantile(self, quantile, interpolation="nearest"):
        return self._f("quantile", quantile, interpolation)

    def count(self):
        return self._f("count")

    def len(self):
        return self._f("len")

    def null_count(self):
        return self._f("null_count")

    def n_unique(self):
        return self._f("n_unique")

    def approx_n_unique(self):
        return self._f("n_unique")

    def first(self):
        return self._f("first")

    def last(self):
        return self._f("last")

    def arg_min(self):
        return self._f("arg_min")

    def arg_max(self):
        return self._f("arg_max")

    def any(self, ignore_nulls=True):
        return self._f("any", ignore_nulls)

    def all(self, ignore_nulls=True):
        return self._f("all", ignore_nulls)

    def implode(self):
        return self._f("implode")

    def agg_groups(self):
        raise NotImplementedError("brevi: Expr.agg_groups is not in Brevi's polars")

    def value_counts(self, sort=False, parallel=False, name=None, normalize=False):
        return self._f("value_counts", sort, name, normalize)

    def entropy(self, base=math.e, normalize=True):
        return self._f("entropy", base, normalize)

    def skew(self, bias=True):
        return self._f("skew", bias)

    def kurtosis(self, fisher=True, bias=True):
        return self._f("kurtosis", fisher, bias)

    def dot(self, other):
        return (self * _x(other, True)).sum()

    @property
    def str(self):
        return _StrNS(self)

    @property
    def list(self):
        return _ListNS(self)

    @property
    def arr(self):
        return _ListNS(self)

    @property
    def dt(self):
        raise NotImplementedError("brevi: temporal expressions (.dt) are not in Brevi's polars")


def _x(v, keep_str=False):
    # a value as an expression: Expr as is; str a column name (keep_str: a literal); else a literal
    if isinstance(v, Expr):
        return v
    if isinstance(v, str) and not keep_str:
        return col(v)
    return Expr("lit", v, None)


def _xo(v):
    return None if v is None else _x(v, True)


def _flat_exprs(items):
    out = []
    for it in items:
        if isinstance(it, (list, tuple)):
            out.extend(_flat_exprs(list(it)))
        elif it is None:
            continue
        else:
            out.append(_x(it))
    return out


def _all_pred(predicates, constraints):
    ps = []
    for p in predicates:
        if isinstance(p, (list, tuple)):
            for q in p:
                ps.append(q if isinstance(q, Expr) else _bool_mask_lit(q))
        elif isinstance(p, Expr):
            ps.append(p)
        else:
            ps.append(_bool_mask_lit(p))
    for k, v in constraints.items():
        ps.append(col(k) == v)
    if not ps:
        raise TypeError("at least one predicate or constraint must be provided")
    r = ps[0]
    for q in ps[1:]:
        r = r & q
    return r


def _bool_mask_lit(p):
    if isinstance(p, Series):
        return Expr("lit", p, None)
    if isinstance(p, bool):
        return Expr("lit", p, None)
    if isinstance(p, (list, tuple)):
        return Expr("lit", Series("", list(p)), None)
    raise TypeError("invalid predicate " + repr(p))


class _NameNS:
    def __init__(self, e):
        self._e = e

    def suffix(self, s):
        return Expr("rename", self._e, "suffix", s)

    def prefix(self, s):
        return Expr("rename", self._e, "prefix", s)

    def keep(self):
        return Expr("rename", self._e, "keep", None)

    def map(self, f):
        return Expr("rename", self._e, "map", f)

    def to_lowercase(self):
        return Expr("rename", self._e, "map", lambda n: n.lower())

    def to_uppercase(self):
        return Expr("rename", self._e, "map", lambda n: n.upper())


class _StrNS:
    def __init__(self, e):
        self._e = e

    def _f(self, name, *a):
        return Expr("fn", "str", self._e, (name,) + a, {})

    def len_chars(self):
        return self._f("len_chars")

    def len_bytes(self):
        return self._f("len_bytes")

    def lengths(self):
        return self._f("len_bytes")

    def n_chars(self):
        return self._f("len_chars")

    def to_uppercase(self):
        return self._f("upper")

    def to_lowercase(self):
        return self._f("lower")

    def to_titlecase(self):
        return self._f("title")

    def strip_chars(self, characters=None):
        return self._f("strip", characters)

    def strip_chars_start(self, characters=None):
        return self._f("lstrip", characters)

    def strip_chars_end(self, characters=None):
        return self._f("rstrip", characters)

    def strip(self, characters=None):
        return self.strip_chars(characters)

    def strip_prefix(self, prefix):
        return self._f("strip_prefix", _x(prefix, True))

    def strip_suffix(self, suffix):
        return self._f("strip_suffix", _x(suffix, True))

    def starts_with(self, prefix):
        return self._f("starts_with", _x(prefix, True))

    def ends_with(self, suffix):
        return self._f("ends_with", _x(suffix, True))

    def contains(self, pattern, literal=False, strict=True):
        return self._f("contains", _x(pattern, True), literal)

    def contains_any(self, patterns, ascii_case_insensitive=False):
        return self._f("contains_any", list(patterns), ascii_case_insensitive)

    def count_matches(self, pattern, literal=False):
        return self._f("count_matches", _x(pattern, True), literal)

    def find(self, pattern, literal=False, strict=True):
        return self._f("find", _x(pattern, True), literal)

    def replace(self, pattern, value, literal=False, n=1):
        return self._f("replace", _x(pattern, True), _x(value, True), literal, n)

    def replace_all(self, pattern, value, literal=False):
        return self._f("replace", _x(pattern, True), _x(value, True), literal, -1)

    def replace_many(self, patterns, replace_with=None, ascii_case_insensitive=False):
        if isinstance(patterns, dict):
            pairs = list(patterns.items())
        else:
            rw = replace_with if isinstance(replace_with, (list, tuple)) else [replace_with] * _len(patterns)
            pairs = list(zip(patterns, rw))
        return self._f("replace_many", pairs)

    def slice(self, offset, length=None):
        return self._f("slice", offset, length)

    def head(self, n):
        return self._f("head", n)

    def tail(self, n):
        return self._f("tail", n)

    def split(self, by, inclusive=False):
        return self._f("split", by, inclusive)

    def split_exact(self, by, n, inclusive=False):
        raise NotImplementedError("brevi: str.split_exact (a struct) is not in Brevi's polars")

    def zfill(self, length):
        return self._f("zfill", length)

    def pad_start(self, length, fill_char=" "):
        return self._f("pad_start", length, fill_char)

    def pad_end(self, length, fill_char=" "):
        return self._f("pad_end", length, fill_char)

    def ljust(self, length, fill_char=" "):
        return self.pad_end(length, fill_char)

    def rjust(self, length, fill_char=" "):
        return self.pad_start(length, fill_char)

    def reverse(self):
        return self._f("reverse")

    def extract(self, pattern, group_index=1):
        return self._f("extract", _x(pattern, True), group_index)

    def extract_all(self, pattern):
        return self._f("extract_all", _x(pattern, True))

    def to_integer(self, base=10, dtype=None, strict=True):
        return self._f("to_integer", base, strict)

    def join(self, delimiter="", ignore_nulls=True):
        return self._f("join", delimiter, ignore_nulls)

    def concat(self, delimiter=None, ignore_nulls=True):
        return self._f("join", "-" if delimiter is None else delimiter, ignore_nulls)

    def encode(self, encoding):
        raise NotImplementedError("brevi: str.encode is not in Brevi's polars")

    def json_decode(self, *a, **k):
        raise NotImplementedError("brevi: str.json_decode is not in Brevi's polars")


class _ListNS:
    def __init__(self, e):
        self._e = e

    def _f(self, name, *a):
        return Expr("fn", "list", self._e, (name,) + a, {})

    def len(self):
        return self._f("len")

    def lengths(self):
        return self._f("len")

    def sum(self):
        return self._f("sum")

    def mean(self):
        return self._f("mean")

    def min(self):
        return self._f("min")

    def max(self):
        return self._f("max")

    def median(self):
        return self._f("median")

    def std(self, ddof=1):
        return self._f("std", ddof)

    def var(self, ddof=1):
        return self._f("var", ddof)

    def first(self):
        return self._f("get", 0, True)

    def last(self):
        return self._f("get", -1, True)

    def get(self, index, null_on_oob=False):
        return self._f("get", index, null_on_oob)

    def contains(self, item, nulls_equal=True):
        return self._f("contains", item)

    def join(self, separator, ignore_nulls=True):
        return self._f("join", separator, ignore_nulls)

    def sort(self, descending=False, nulls_last=False):
        return self._f("sort", descending, nulls_last)

    def unique(self, maintain_order=False):
        return self._f("unique")

    def n_unique(self):
        return self._f("n_unique")

    def reverse(self):
        return self._f("reverse")

    def slice(self, offset, length=None):
        return self._f("slice", offset, length)

    def head(self, n=5):
        return self._f("slice", 0, n)

    def tail(self, n=5):
        return self._f("tail", n)

    def arg_min(self):
        return self._f("arg_min")

    def arg_max(self):
        return self._f("arg_max")

    def explode(self, *, empty_as_null=False, keep_nulls=True):
        return Expr("fn", "explode", self._e, (bool(empty_as_null), keep_nulls), {})

    def count_matches(self, element):
        return self._f("count_matches", element)

    def any(self):
        return self._f("any")

    def all(self):
        return self._f("all")

    def concat(self, other):
        return Expr("fn", "list", self._e, ("concat", _x(other, True)), {})

    def eval(self, expr):
        raise NotImplementedError("brevi: list.eval is not in Brevi's polars")


# Expressions that give one value (per group in an aggregation)
_AGG = {"sum", "mean", "median", "min", "max", "nan_min", "nan_max", "std", "var", "product", "quantile", "count", "len", "null_count",
        "n_unique", "first", "last", "arg_min", "arg_max", "any", "all", "implode", "entropy", "skew", "kurtosis", "get"}
# namespace functions that give one value for the whole column
_NS_SCALAR = {"str": ("join",), "list": ()}


def _is_scalar(e):
    op = e._op
    if op == "lit":
        v = e._a[0]
        return not isinstance(v, Series) or _len(v._v) == 1 and False
    if op in ("col", "cols"):
        return False
    if op in ("len", "corr"):
        return True
    if op == "bin":
        return _is_scalar(e._a[1]) and _is_scalar(e._a[2])
    if op in ("alias", "cast", "rename"):
        return _is_scalar(e._a[0])
    if op == "fn":
        name = e._a[0]
        if name in _AGG:
            return True
        if name == "str" and e._a[2][0] == "join":
            return True
        if name in ("sort", "reverse", "unique", "drop_nulls", "drop_nans", "head", "tail", "slice", "gather", "gather_every", "top_k",
                    "mode", "explode", "value_counts", "unique_counts", "arg_unique", "arg_sort", "map_batches", "filter"):
            if name == "map_batches" and e._a[2][2]:
                return True
            return False
        return _is_scalar(e._a[1])
    if op == "when":
        return _all(_is_scalar(c) and _is_scalar(t) for c, t in e._a[0]) and (e._a[1] is None or _is_scalar(e._a[1]))
    if op == "horiz":
        return _all(_is_scalar(x) for x in e._a[1])
    return False


def _out_name(e):
    op = e._op
    if op == "col":
        return e._a[0]
    if op == "lit":
        v = e._a[0]
        if isinstance(v, Series):
            return v.name
        return "literal"
    if op == "len":
        return "len"
    if op == "corr":
        return _out_name(e._a[0])
    if op == "alias":
        return e._a[1]
    if op == "rename":
        base = _out_name(e._a[0]) if e._a[1] != "keep" else _root_name(e._a[0])
        kind, arg = e._a[1], e._a[2]
        if kind == "suffix":
            return _root_name(e._a[0]) + arg
        if kind == "prefix":
            return arg + _root_name(e._a[0])
        if kind == "map":
            return arg(_root_name(e._a[0]))
        return base
    if op == "bin":
        return _out_name(e._a[1])
    if op in ("fn", "cast", "filter", "sort_by", "over"):
        return _out_name(e._a[1] if op == "fn" else e._a[0])
    if op == "when":
        return _out_name(e._a[0][0][1])
    if op == "fold":
        return _out_name(e._a[0]) if e._a[0] is not None else (_out_name(e._a[2][0]) if e._a[2] else "literal")
    if op == "horiz":
        if e._a[0] in ("sum", "min", "max", "mean", "any", "all", "concat_str", "coalesce", "concat_list", "struct"):
            return _out_name(e._a[1][0]) if e._a[1] else "literal"
    if op == "int_range":
        return "literal"
    return "literal"


def _root_name(e):
    op = e._op
    if op == "col":
        return e._a[0]
    if op == "alias":
        return _root_name(e._a[0])
    if op == "lit":
        return _out_name(e)
    if op == "len":
        return "len"
    if op == "bin":
        return _root_name(e._a[1])
    if op == "fn":
        return _root_name(e._a[1])
    if op in ("cast", "filter", "sort_by", "over", "rename"):
        return _root_name(e._a[0])
    if op == "when":
        return _root_name(e._a[0][0][1])
    return _out_name(e)


def _expr_str(e):
    op = e._op
    if op == "col":
        return 'col("' + e._a[0] + '")'
    if op == "cols":
        return "cols(" + repr(e._a[0]) + ")"
    if op == "lit":
        v = e._a[0]
        if isinstance(v, Series):
            return "Series"
        if isinstance(v, str):
            return '"' + v + '"'
        if v is None:
            return "null"
        if v is True or v is False:
            return "true" if v else "false"
        if isinstance(v, int) and not isinstance(v, bool):
            return "dyn int: " + str(v)
        if isinstance(v, float):
            return "dyn float: " + _rdisp(v)
        return str(v)
    if op == "len":
        return "len()"
    if op == "alias":
        return _expr_str(e._a[0]) + '.alias("' + e._a[1] + '")'
    if op == "bin":
        sym = {"==": "==", "!=": "!=", "&": "&", "|": "|", "//": "floor_div", "**": "pow"}.get(e._a[0], e._a[0])
        if e._a[0] == "**":
            return _expr_str(e._a[1]) + ".pow([" + _expr_str(e._a[2]) + "])"
        return "[(" + _expr_str(e._a[1]) + ") " + sym + " (" + _expr_str(e._a[2]) + ")]"
    if op == "fn":
        return _expr_str(e._a[1]) + "." + e._a[0] + "()"
    if op == "cast":
        return _expr_str(e._a[0]) + ".strict_cast(" + repr(e._a[1]) + ")"
    return op + "(...)"


def col(name, *more_names):
    if isinstance(name, DataType) or (isinstance(name, (list, tuple)) and name and isinstance(name[0], DataType)):
        ds = list(name) if isinstance(name, (list, tuple)) else [name]
        for m in more_names:
            ds.append(m)
        return Expr("cols", ("dtype", [_dt(d) for d in ds]))
    if isinstance(name, (list, tuple)):
        names = list(name) + list(more_names)
        return Expr("cols", ("names", names))
    if more_names:
        return Expr("cols", ("names", [name] + list(more_names)))
    if name == "*":
        return Expr("cols", ("all", None))
    if isinstance(name, str) and name.startswith("^") and name.endswith("$"):
        return Expr("cols", ("regex", name))
    if not isinstance(name, str):
        raise TypeError("invalid input for `col`\n\nExpected `str` or `DataType`, got " + repr(type(name).__name__) + ".")
    return Expr("col", name)


class _ColFactory:
    def __call__(self, name, *more):
        return col(name, *more)

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return col(name)


def lit(value, dtype=None, allow_object=False):
    if dtype is not None:
        return Expr("cast", Expr("lit", value, None), _dt(dtype), True, False)
    if isinstance(value, (list, tuple)):
        return Expr("lit", Series("literal", [list(value)]), None)
    return Expr("lit", value, None)


def all(*args, **kwargs):
    if args or kwargs:
        # pl.all(*names) is pl.col(*names).all()
        return col(*args).all()
    return Expr("cols", ("all", None))


def exclude(columns, *more_columns):
    names = (list(columns) if isinstance(columns, (list, tuple)) else [columns]) + list(more_columns)
    return Expr("cols", ("exclude", names))


def len():
    return Expr("len")


def count(*columns):
    if not columns:
        return Expr("len")
    return col(*columns).count()


def first(*columns):
    if not columns:
        return Expr("cols", ("first", None))
    return col(*columns).first()


def last(*columns):
    if not columns:
        return Expr("cols", ("last", None))
    return col(*columns).last()


def nth(*indices):
    return Expr("cols", ("nth", list(indices)))


def sum(*names):
    if _len(names) == 1 and isinstance(names[0], Expr):
        return names[0].sum()
    return col(*names).sum()


def mean(*names):
    return col(*names).mean()


def median(*names):
    return col(*names).median()


def min(*names):
    return col(*names).min()


def max(*names):
    return col(*names).max()


def std(column, ddof=1):
    return col(column).std(ddof)


def var(column, ddof=1):
    return col(column).var(ddof)


def n_unique(*names):
    return col(*names).n_unique()


def quantile(column, quantile, interpolation="nearest"):
    return col(column).quantile(quantile, interpolation)


def implode(*columns):
    return col(*columns).implode()


def cum_sum(*names):
    return col(*names).cum_sum()


def _horiz(kind, exprs, *extra):
    es = _flat_exprs(list(exprs))
    return Expr("horiz", kind, es) if not extra else Expr("horiz", kind, es, *extra)


def corr(a, b, *, method="pearson", propagate_nans=False, eager=False):
    if method not in ("pearson", "spearman"):
        raise ValueError("method must be one of {'pearson', 'spearman'}, got " + repr(method))
    e = Expr("corr", _x(a), _x(b), method, 1)
    return _eager2(e, a, b) if eager else e


def cov(a, b, *, ddof=1, eager=False):
    e = Expr("corr", _x(a), _x(b), "cov", ddof)
    return _eager2(e, a, b) if eager else e


def _eager2(e, a, b):
    return select(e).to_series()


def _corr(e, f):
    x = _eval(e._a[0], f)
    y = _eval(e._a[1], f)
    method, ddof = e._a[2], e._a[3]
    pairs = [(p, q) for p, q in zip(x._v, y._v) if p is not None and q is not None]
    if method == "spearman":
        # ranks of the rows where both are set
        rx = _fn_rank(_new("", [p for p, _ in pairs], x.dtype), f, "average", False)._v
        ry = _fn_rank(_new("", [q for _, q in pairs], y.dtype), f, "average", False)._v
        pairs = list(zip(rx, ry))
    pairs = [(float(p), float(q)) for p, q in pairs]
    # polars-ops moment.rs CovState / PearsonState over chunks of 128
    w = mx = my = dxx = dxy = dyy = 0.0
    for c in range(0, _len(pairs), 128):
        ch = pairs[c:c + 128]
        ow = float(_len(ch))
        sx = -0.0
        sy = -0.0
        for p, q in ch:
            sx = sx + p
            sy = sy + q
        ox, oy = sx / ow, sy / ow
        oxx, oxy, oyy = 0.0, -0.0, 0.0
        for p, q in ch:
            oxx = oxx + (p - ox) * (p - ox)
            oxy = oxy + (p - ox) * (q - oy)
            oyy = oyy + (q - oy) * (q - oy)
        if w == 0.0:
            w, mx, my, dxx, dxy, dyy = ow, ox, oy, oxx, oxy, oyy
            continue
        nw = w + ow
        frac = ow / nw
        dmx, dmy = ox - mx, oy - my
        nmx, nmy = mx + dmx * frac, my + dmy * frac
        dxx = dxx + (oxx + ow * (ox - nmx) * dmx)
        dxy = dxy + (oxy + ow * (ox - nmx) * dmy)
        dyy = dyy + (oyy + ow * (oy - nmy) * dmy)
        w, mx, my = nw, nmx, nmy
    if method == "cov":
        r = None if w <= ddof else dxy / (w - ddof)
    else:
        d = dxx * dyy
        r = dxy / math.sqrt(d) if d > 0.0 else (None if w == 0.0 else math.nan)
    return _new(x.name, [r], Float64)


def fold(acc, function, exprs, *, returns_scalar=False, return_dtype=None):
    # acc = function(acc, x) over the inputs, as Series; the result keeps the dtype of acc (or return_dtype)
    es = _flat_exprs(exprs if isinstance(exprs, (list, tuple)) else [exprs])
    return Expr("fold", _x(acc, True), function, tuple(es), None if return_dtype is None else _dt(return_dtype), False)


def reduce(function, exprs, *, returns_scalar=False, return_dtype=None):
    es = _flat_exprs(exprs if isinstance(exprs, (list, tuple)) else [exprs])
    return Expr("fold", None, function, tuple(es), None if return_dtype is None else _dt(return_dtype), False)


def cum_fold(acc, function, exprs, *, returns_scalar=False, return_dtype=None, include_init=False):
    raise NotImplementedError("brevi: cum_fold (a struct) is not in Brevi's polars")


def sum_horizontal(*exprs, ignore_nulls=True):
    return _horiz("sum", exprs, ignore_nulls)


def max_horizontal(*exprs):
    return _horiz("max", exprs)


def min_horizontal(*exprs):
    return _horiz("min", exprs)


def mean_horizontal(*exprs, ignore_nulls=True):
    return _horiz("mean", exprs, ignore_nulls)


def any_horizontal(*exprs):
    return _horiz("any", exprs)


def all_horizontal(*exprs):
    return _horiz("all", exprs)


def coalesce(exprs, *more_exprs):
    items = (list(exprs) if isinstance(exprs, (list, tuple)) else [exprs]) + list(more_exprs)
    return _horiz("coalesce", items)


def concat_str(exprs, *more_exprs, separator="", ignore_nulls=False):
    items = (list(exprs) if isinstance(exprs, (list, tuple)) else [exprs]) + list(more_exprs)
    return Expr("horiz", "concat_str", _flat_exprs(items), separator, ignore_nulls)


def concat_list(exprs, *more_exprs):
    items = (list(exprs) if isinstance(exprs, (list, tuple)) else [exprs]) + list(more_exprs)
    return _horiz("concat_list", items)


def format(f_string, *args):
    parts = f_string.split("{}")
    if _len(parts) - 1 != _len(args):
        raise ValueError("number of placeholders should equal the number of arguments")
    items = []
    for i, p in enumerate(parts):
        if p:
            items.append(lit(p))
        if i < _len(args):
            items.append(_x(args[i]))
    return concat_str(items)


def int_range(start=0, end=None, step=1, dtype=None, eager=False):
    if end is None:
        start, end = 0, start
    e = Expr("int_range", _x(start, True), _x(end, True), step, _dt(dtype) if dtype is not None else Int64)
    if eager:
        return select(e.alias("literal")).to_series()
    return e


def arange(start=0, end=None, step=1, dtype=None, eager=False):
    return int_range(start, end, step, dtype, eager)


def repeat(value, n, dtype=None, eager=False):
    e = Expr("repeat", _x(value, True), _x(n, True), _dt(dtype) if dtype is not None else None)
    if eager:
        return select(e.alias("repeat")).to_series()
    return e


class _When:
    def __init__(self, conds):
        self._conds = conds

    def then(self, value):
        return _Then(self._conds, _x(value))


class _Then(Expr):
    __slots__ = ("_branches", "_pending")

    def __init__(self, branches, then=None):
        if then is not None:
            br = list(branches[:-1]) + [(branches[-1][0], then)]
        else:
            br = list(branches)
        Expr.__init__(self, "when", br, None)
        self._branches = br

    def when(self, *predicates, **constraints):
        return _ChainWhen(self._branches, _all_pred(predicates, constraints))

    def otherwise(self, value):
        return Expr("when", self._branches, _x(value))


class _ChainWhen:
    def __init__(self, branches, cond):
        self._branches = branches
        self._cond = cond

    def then(self, value):
        return _Then(list(self._branches) + [(self._cond, _x(value))])


def when(*predicates, **constraints):
    return _ChainWhen([], _all_pred(predicates, constraints))




# ---------- evaluation ----------


class _Frame:
    # the columns an expression is evaluated over: a DataFrame, or the rows `idx` of one (a group)
    __slots__ = ("cols", "names", "height", "idx", "cache")

    @staticmethod
    def of(cols, idx=None):
        f = _Frame()
        f.cols = {}
        f.names = []
        for s in cols:
            f.cols[s.name] = s
            f.names.append(s.name)
        f.idx = idx
        f.height = _len(idx) if idx is not None else (_len(cols[0]._v) if cols else 0)
        f.cache = {}
        return f

    def get(self, name):
        s = self.cols.get(name)
        if s is None:
            raise ColumnNotFoundError(_col_missing(name, self.names))
        if self.idx is None:
            return s
        c = self.cache.get(name)
        if c is None:
            v = s._v
            c = _new(name, [v[i] for i in self.idx], s.dtype)
            self.cache[name] = c
        return c

    def dtype(self, name):
        s = self.cols.get(name)
        if s is None:
            raise ColumnNotFoundError(_col_missing(name, self.names))
        return s.dtype


def _col_missing(name, names):
    return 'unable to find column "' + name + '"; valid columns: [' + ", ".join('"' + n + '"' for n in names) + "]"


def _lit_series(v, name="literal"):
    if isinstance(v, Series):
        return _new(v.name, v._v, v.dtype)
    if v is None:
        return _new(name, [None], Null)
    if v is True or v is False:
        return _new(name, [v], Boolean)
    if isinstance(v, int):
        if -(1 << 31) <= v < (1 << 31):
            return _new(name, [v], Int32, "int")
        if _I64_MIN <= v <= _I64_MAX:
            return _new(name, [v], Int64, "int")
        if 0 <= v < (1 << 64):
            return _new(name, [v], UInt64, "int")
        raise OverflowError("int value too large for Polars integer types: " + str(v))
    if isinstance(v, float):
        return _new(name, [v], Float64, "float")
    if isinstance(v, str):
        return _new(name, [v], String, "str")
    if isinstance(v, (list, tuple)):
        s = Series(name, [list(v)])
        return s
    raise TypeError("cannot create expression literal for value of type " + type(v).__name__ + ".\n\nHint: Pass `allow_object=True` to accept any value and create a literal of type Object.")


def _expand(e, names, dtypes):
    # multi-column expressions (pl.col("a", "b"), pl.all(), pl.exclude, dtype selection) as one expression per column
    e = _inline_multi(e, names, dtypes)
    sel = _find_multi(e)
    if sel is None:
        return [e]
    picked = _pick(sel._a[0], names, dtypes)
    out = []
    for n in picked:
        out.extend(_expand(_subst(e, sel, Expr("col", n)), names, dtypes))
    return out


def _inline_multi(e, names, dtypes):
    # the inputs of a horizontal function or a fold are expanded in place: pl.sum_horizontal(pl.all()) is one column
    if not isinstance(e, Expr) or e._op in ("col", "lit", "cols", "len"):
        return e
    args = []
    for i, a in enumerate(e._a):
        if e._op in ("horiz", "fold") and i == (1 if e._op == "horiz" else 2) and isinstance(a, (list, tuple)):
            xs = []
            for b in a:
                xs.extend(_expand(b, names, dtypes))
            args.append(type(a)(xs))
        elif isinstance(a, Expr):
            args.append(_inline_multi(a, names, dtypes))
        else:
            args.append(a)
    return Expr(e._op, *args)


def _pick(spec, names, dtypes):
    kind, arg = spec
    if kind == "minus":
        # pl.all().exclude("a"), pl.col(pl.Int64).exclude(...)
        base, drop = arg
        keep = set(_pick(("exclude", drop), names, dtypes))
        return [x for x in _pick(base, names, dtypes) if x in keep]
    if kind == "names":
        picked = []
        for n in arg:
            if n.startswith("^") and n.endswith("$"):
                rx = _re.compile(n)
                picked.extend(x for x in names if rx.search(x))
            else:
                picked.append(n)
    elif kind == "all":
        picked = list(names)
    elif kind == "exclude":
        ex = set()
        rxs = []
        for n in arg:
            if isinstance(n, DataType):
                ex.update(x for x in names if dtypes[x] == n)
            elif isinstance(n, str) and n.startswith("^") and n.endswith("$"):
                rxs.append(_re.compile(n))
            else:
                ex.add(n)
        picked = [x for x in names if x not in ex and not _any(r.search(x) for r in rxs)]
    elif kind == "dtype":
        picked = [x for x in names if _any(dtypes[x] == d for d in arg)]
    elif kind == "regex":
        rx = _re.compile(arg)
        picked = [x for x in names if rx.search(x)]
    elif kind == "first":
        picked = names[:1]
    elif kind == "last":
        picked = names[-1:]
    elif kind == "nth":
        picked = [names[i] for i in arg]
    elif kind == "selector":
        picked = arg(names, dtypes)
    else:
        picked = []
    return picked


_any = any


def _find_multi(e):
    if not isinstance(e, Expr):
        return None
    if e._op == "cols":
        return e
    for a in e._a:
        if isinstance(a, Expr):
            r = _find_multi(a)
            if r is not None:
                return r
        elif isinstance(a, (list, tuple)):
            for b in a:
                if isinstance(b, Expr):
                    r = _find_multi(b)
                    if r is not None:
                        return r
                elif isinstance(b, tuple):
                    for c in b:
                        if isinstance(c, Expr):
                            r = _find_multi(c)
                            if r is not None:
                                return r
    return None


def _subst(e, old, new):
    if e is old:
        return new
    if not isinstance(e, Expr):
        return e
    args = []
    for a in e._a:
        if isinstance(a, Expr):
            args.append(_subst(a, old, new))
        elif isinstance(a, list):
            args.append([_subst_any(b, old, new) for b in a])
        elif isinstance(a, tuple):
            args.append(tuple(_subst_any(b, old, new) for b in a))
        else:
            args.append(a)
    return Expr(e._op, *args)


def _subst_any(b, old, new):
    if isinstance(b, Expr):
        return _subst(b, old, new)
    if isinstance(b, tuple):
        return tuple(_subst_any(c, old, new) for c in b)
    return b


def _exprs_for(f, items, named=None):
    # the expressions select / with_columns / agg take: strings are columns, lists are flattened, kwargs alias
    out = []
    for it in items:
        if isinstance(it, (list, tuple)):
            out.extend(_exprs_for(f, it))
        elif isinstance(it, str):
            out.append(col(it))
        elif isinstance(it, Expr):
            out.append(it)
        elif isinstance(it, Series):
            out.append(Expr("lit", it, None))
        else:
            out.append(Expr("lit", it, None))
    if named:
        for k, v in named.items():
            out.append(_x(v, True).alias(k))
    dts = {n: f.dtype(n) for n in f.names}
    res = []
    for e in out:
        res.extend(_expand(e, f.names, dts))
    return res


class _StrArith(InvalidOperationError):
    pass


def _operand_dtype(s):
    return {"int": "dyn int", "float": "dyn float"}.get(s._lit, _short(s.dtype))


def _operand(e, s):
    if e._op == "col":
        return "column '" + e._a[0] + "'"
    return "expression `" + _expr_str(e) + "`"


def _eval(e, f):
    op = e._op
    if op == "col":
        return f.get(e._a[0])
    if op == "lit":
        return _lit_series(e._a[0])
    if op == "alias":
        r = _eval(e._a[0], f)
        return _new(e._a[1], r._v, r.dtype, r._lit)
    if op == "rename":
        r = _eval(e._a[0], f)
        return _new(_out_name(e), r._v, r.dtype, r._lit)
    if op == "bin":
        a = _eval(e._a[1], f)
        b = _eval(e._a[2], f)
        try:
            r = _binop(e._a[0], a, b)
        except _StrArith:
            # as polars words it, with the operands
            raise InvalidOperationError("arithmetic on dtypes " + _operand_dtype(a) + " and " + _operand_dtype(b) + " is not allowed (lhs: "
                                        + _operand(e._a[1], a) + ", rhs: " + _operand(e._a[2], b) + "); try an explicit cast first")
        return _new(a.name, r._v, r.dtype, r._lit)
    if op == "fn":
        return _call_fn(e, f)
    if op == "cast":
        r = _eval(e._a[0], f)
        return _cast(r, e._a[1], e._a[2], e._a[3])
    if op == "len":
        return _new("len", [f.height], UInt32)
    if op == "when":
        return _when(e, f)
    if op == "over":
        return _over(e, f)
    if op == "filter":
        r = _eval(e._a[0], f)
        m = _eval(e._a[1], f)
        if m.dtype._kind not in ("bool", "null"):
            raise InvalidOperationError("filter predicate must be of type `Boolean`, got `" + repr(m.dtype) + "`")
        mv = m._v if _len(m._v) == _len(r._v) else m._v * _len(r._v)
        return _new(r.name, [v for v, k in zip(r._v, mv) if k], r.dtype)
    if op == "sort_by":
        r = _eval(e._a[0], f)
        keys = [_broadcast(_eval(k, f), _len(r._v)) for k in e._a[1]]
        order = _sort_idx(keys, e._a[2], e._a[3])
        return _new(r.name, [r._v[i] for i in order], r.dtype)
    if op == "horiz":
        return _horizontal(e, f)
    if op == "fold":
        return _fold(e, f)
    if op == "corr":
        return _corr(e, f)
    if op == "int_range":
        a = _eval(e._a[0], f)._v[0]
        b = _eval(e._a[1], f)._v[0]
        return _new("literal", list(range(a, b, e._a[2])), e._a[3])
    if op == "repeat":
        v = _eval(e._a[0], f)
        n = _eval(e._a[1], f)._v[0]
        r = _new(v.name, v._v * n, v.dtype, None)
        if e._a[2] is not None:
            r = _cast(r, e._a[2], True, False)
        elif r._lit == "int" or v._lit == "int":
            r = _new(r.name, r._v, Int32)
        return r
    if op == "cols":
        raise ComputeError("expression with multiple columns used where one column was expected")
    raise ComputeError("unsupported expression " + op)


def _broadcast(s, n):
    if _len(s._v) == n:
        return s
    if _len(s._v) == 1:
        return _new(s.name, s._v * n, s.dtype, s._lit)
    raise ShapeError("Series length " + str(_len(s._v)) + " doesn't match the DataFrame height of " + str(n))


def _materialize(s):
    # a dynamic literal (pl.lit(1) is i32) as a column
    if s._lit is not None:
        return _new(s.name, s._v, s.dtype)
    return s


# -- binary operators --

_ARITH = ("+", "-", "*", "/", "//", "%", "**")
_CMP = ("==", "!=", "<", "<=", ">", ">=", "eqm", "nem")


def _num_dtype(a, b):
    # result dtype of arithmetic, with polars' dynamic literals taking the other side's type
    da, db = a.dtype, b.dtype
    if a._lit == "int" and b._lit is None and db.is_numeric():
        return db
    if b._lit == "int" and a._lit is None and da.is_numeric():
        return da
    if a._lit == "float" and b._lit is None and db._kind == "float":
        return db
    if b._lit == "float" and a._lit is None and da._kind == "float":
        return da
    if a._lit == "float" and b._lit is None and db.is_integer():
        return Float64
    if b._lit == "float" and a._lit is None and da.is_integer():
        return Float64
    return _supertype(da, db)


def _binop(op, a, b):
    n = _max(_len(a._v), _len(b._v))
    if _len(a._v) != _len(b._v):
        if _len(a._v) == 1:
            a = _new(a.name, a._v * n, a.dtype, a._lit)
        elif _len(b._v) == 1:
            b = _new(b.name, b._v * n, b.dtype, b._lit)
        else:
            raise ShapeError("cannot do a binary operation on columns of different lengths: got " + str(_len(a._v)) + " and " + str(_len(b._v)))
    lit = a._lit if (a._lit is not None and b._lit is not None) else None
    av, bv = a._v, b._v
    da, db = a.dtype, b.dtype
    if op in _CMP:
        return _new(a.name, _compare(op, a, b), Boolean, None)
    if op in ("&", "|", "^"):
        if da._kind in ("bool", "null") and db._kind in ("bool", "null"):
            out = []
            for x, y in zip(av, bv):
                if op == "&":
                    if x is False or y is False:
                        out.append(False)
                    elif x is None or y is None:
                        out.append(None)
                    else:
                        out.append(True)
                elif op == "|":
                    if x is True or y is True:
                        out.append(True)
                    elif x is None or y is None:
                        out.append(None)
                    else:
                        out.append(False)
                else:
                    out.append(None if x is None or y is None else (x != y))
            return _new(a.name, out, Boolean, None)
        if da.is_integer() and db.is_integer():
            d = _num_dtype(a, b)
            out = []
            for x, y in zip(av, bv):
                if x is None or y is None:
                    out.append(None)
                elif op == "&":
                    out.append(_wrap(x & y, d))
                elif op == "|":
                    out.append(_wrap(x | y, d))
                else:
                    out.append(_wrap(x ^ y, d))
            return _new(a.name, out, d, lit)
        if da._kind == "str" or db._kind == "str":
            raise InvalidOperationError("`bit" + {"&": "and", "|": "or", "^": "xor"}[op] + "` operation not supported for dtype `str`")
        raise InvalidOperationError("bitwise operation not supported for " + _short(da) + " and " + _short(db))
    # arithmetic
    if da._kind == "str" or db._kind == "str":
        if op == "+" and da._kind in ("str", "null") and db._kind in ("str", "null"):
            return _new(a.name, [None if x is None or y is None else x + y for x, y in zip(av, bv)], String, lit)
        if op == "/":
            raise InvalidOperationError("division with 'String' datatypes is not allowed")
        if da._kind == "str" and db._kind == "str":
            name = {"-": "sub", "*": "mul", "%": "rem"}.get(op)
            if name:
                raise InvalidOperationError(name + " operation not supported for dtypes `str` and `str`")
        if op == "//" and da._kind == "str":
            raise InvalidOperationError("floor_div operation not supported for dtype `str`")
        if op == "**":
            raise InvalidOperationError("`pow` operation not supported for dtype `str` as " + ("base" if da._kind == "str" else "exponent"))
        raise _StrArith("arithmetic on string and numeric not allowed, try an explicit cast first")
    if da._kind == "list" or db._kind == "list":
        raise InvalidOperationError("arithmetic on lists is not supported in Brevi's polars")
    if da._kind == "bool" and db._kind == "bool" and op in ("+", "-", "*"):
        d = _ListType if False else (UInt32 if op == "+" else Int64)
        if op == "+":
            d = UInt32
    d = _num_dtype(a, b) if not (da._kind == "null" and db._kind == "null") else Null
    if d._kind == "bool":
        d = Int64 if op != "+" else UInt32
    if d._kind == "null":
        return _new(a.name, [None] * n, Null, lit)
    if op == "/":
        if d._kind != "float":
            d = Float64
        f32 = d._bits == 32
        out = []
        for x, y in zip(av, bv):
            if x is None or y is None:
                out.append(None)
                continue
            x, y = float(x), float(y)
            if y == 0.0:
                if x == 0.0 or x != x:
                    r = math.nan
                else:
                    r = math.copysign(math.inf, x) * math.copysign(1.0, y)
            else:
                r = x / y
            out.append(_f32(r) if f32 else r)
        return _new(a.name, out, d, lit if lit != "int" else "float")
    if op == "**":
        bt = db
        if da.is_integer() and bt.is_integer() and not (a._lit is not None and b._lit is None and False):
            d = da if a._lit is None else (Int64 if da._kind != "uint" else da)
            if a._lit == "int" and b._lit is None:
                d = Int32 if a._v and a._v[0] is not None and -(1 << 31) <= a._v[0] < (1 << 31) else Int64
            out = []
            for x, y in zip(av, bv):
                if x is None or y is None:
                    out.append(None)
                elif y < 0:
                    out.append(0 if _abs(x) != 1 else (x if y % 2 else 1))
                else:
                    out.append(_wrap(_ipow(x, y, d), d))
            return _new(a.name, out, d, lit)
        if d._kind != "float":
            d = Float64
        if da._kind == "float" and b._lit == "int":
            d = da
        f32 = d._bits == 32
        out = []
        for x, y in zip(av, bv):
            if x is None or y is None:
                out.append(None)
                continue
            r = _fpow(float(x), float(y))
            out.append(_f32(r) if f32 else r)
        return _new(a.name, out, d, lit)
    out = []
    if d._kind == "float":
        f32 = d._bits == 32
        for x, y in zip(av, bv):
            if x is None or y is None:
                out.append(None)
                continue
            x, y = float(x), float(y)
            if op == "+":
                r = x + y
            elif op == "-":
                r = x - y
            elif op == "*":
                r = x * y
            elif op == "//":
                if y == 0.0:
                    r = math.nan if x == 0.0 or x != x else math.copysign(math.inf, x) * math.copysign(1.0, y)
                else:
                    r = math.floor(x / y) if (x / y) == (x / y) and _abs(x / y) != math.inf else x / y
                    r = float(r)
            else:
                r = _fmod_py(x, y)
            out.append(_f32(r) if f32 else r)
        return _new(a.name, out, d, lit)
    for x, y in zip(av, bv):
        if x is None or y is None:
            out.append(None)
            continue
        if op == "+":
            r = x + y
        elif op == "-":
            r = x - y
        elif op == "*":
            r = x * y
        elif op == "//":
            if y == 0:
                out.append(None)
                continue
            r = x // y
        else:
            if y == 0:
                out.append(None)
                continue
            r = x % y
        out.append(_wrap(int(r), d))
    return _new(a.name, out, d, lit)


_abs = abs


def _ipow(x, y, d):
    if y > 64 and _abs(x) > 1:
        r = 1
        for _ in range(y):
            r = _wrap(r * x, d)
        return r
    return x ** y


def _fpow(x, y):
    try:
        return math.pow(x, y)
    except (OverflowError, ValueError):
        if x < 0 and y != math.floor(y):
            return math.nan
        if x == 0.0 and y < 0:
            return math.inf
        return math.inf if (x > 1 or x < -1) == (y > 0) else 0.0


def _fmod_py(x, y):
    # polars' float % is Python's (floored): x - y * floor(x / y)
    if y == 0.0:
        return math.nan
    if x != x or y != y or x in (math.inf, -math.inf):
        return math.nan
    r = math.fmod(x, y)
    if r != 0.0 and ((r < 0) != (y < 0)):
        r += y
    return r


def _cmp_key(v, kind):
    return v


def _compare(op, a, b):
    da, db = a.dtype, b.dtype
    ka, kb = da._kind, db._kind
    num = ("int", "uint", "float", "bool")
    if ka != kb and ka != "null" and kb != "null" and not (ka in num and kb in num):
        if (ka == "str") != (kb == "str"):
            raise ComputeError("cannot compare string with numeric type (" + _short(db if ka == "str" else da) + ")")
        raise ComputeError("cannot compare " + _short(da) + " with " + _short(db))
    out = []
    missing = op in ("eqm", "nem")
    for x, y in zip(a._v, b._v):
        if x is None or y is None:
            if missing:
                r = (x is None) == (y is None)
                out.append(r if op == "eqm" else not r)
            else:
                out.append(None)
            continue
        if op in ("==", "eqm"):
            r = x == y or (x != x and y != y)
        elif op in ("!=", "nem"):
            r = not (x == y or (x != x and y != y))
        else:
            # NaN is the largest float in polars' total order
            xn = isinstance(x, float) and x != x
            yn = isinstance(y, float) and y != y
            if xn or yn:
                c = 0 if (xn and yn) else (1 if xn else -1)
            else:
                c = -1 if x < y else (1 if x > y else 0)
            if op == "<":
                r = c < 0
            elif op == "<=":
                r = c <= 0
            elif op == ">":
                r = c > 0
            else:
                r = c >= 0
        out.append(r)
    return out


# -- casts --


def _cast(s, d, strict, wrap=False):
    if s.dtype == d and s._lit is None:
        return s
    sk = s.dtype._kind
    out = []
    bad = []
    for v in s._v:
        if v is None:
            out.append(None)
            continue
        if d.is_integer() and isinstance(v, int) and not isinstance(v, bool) and wrap:
            out.append(_wrap(v, d))
            continue
        if sk == "float" and d._kind == "str":
            out.append(_float_str(v, s.dtype._bits == 32))
            continue
        ok, y = _cast_value(v, d, strict)
        if not ok:
            if strict:
                bad.append(v)
            y = None
        out.append(y)
    if bad:
        shown = bad[:10]
        vals = ", ".join(_any_str(x, s.dtype, True) for x in shown)
        raise InvalidOperationError("conversion from `" + _short(s.dtype) + "` to `" + _short(d) + "` failed in column '" + s.name + "' for " + str(_len(bad)) + " out of " + str(_len(s._v)) + " values: [" + vals + "]")
    return _new(s.name, out, d)


# -- when / then / otherwise --


def _when(e, f):
    branches, other = e._a[0], e._a[1]
    conds = [_eval(c, f) for c, _ in branches]
    thens = [_eval(t, f) for _, t in branches]
    oth = _eval(other, f) if other is not None else _new("literal", [None], Null)
    n = 1
    for s in conds + thens + [oth]:
        if _len(s._v) != 1:
            n = _len(s._v)
    if _all(_is_scalar(c) for c, _ in branches) is False and f.height and n == 1:
        n = 1
    d = oth.dtype if (oth._lit is None or _all(t._lit is not None for t in thens)) else None
    ds = [t for t in thens] + [oth]
    nonlit = [s for s in ds if s._lit is None and s.dtype._kind != "null"]
    if nonlit:
        d = nonlit[0].dtype
        for s in nonlit[1:]:
            d = _supertype(d, s.dtype)
        for s in ds:
            if s._lit is not None:
                if s._lit == "float" and d.is_integer():
                    d = Float64
                elif s._lit == "str" and d._kind != "str":
                    d = _supertype(d, String)
    else:
        d = Null
        for s in ds:
            if s.dtype._kind != "null":
                d = s.dtype if d._kind == "null" else _supertype(d, s.dtype)
    conds = [_broadcast(c, n) for c in conds]
    thens = [_broadcast(_cast(t, d, False) if d._kind != "null" else t, n) for t in thens]
    oth = _broadcast(_cast(oth, d, False) if d._kind != "null" else oth, n)
    out = []
    for i in range(n):
        for c, t in zip(conds, thens):
            if c._v[i] is True:
                out.append(t._v[i])
                break
        else:
            out.append(oth._v[i])
    lit = None
    if _all(t._lit is not None for t in thens) and (oth._lit is not None) and _all(_len(c._v) == 1 for c in conds):
        lit = thens[0]._lit
    return _new(_out_name(e), out, d, lit)


# -- window functions --


def _over(e, f):
    inner, keys, order_by, strategy = e._a
    ks = [_broadcast(_eval(k, f), f.height) for k in keys]
    groups = _group_rows(ks, f.height)
    base_idx = f.idx
    out = [None] * f.height
    dtype = None
    per = []
    for g in groups:
        if order_by is not None:
            sub = _Frame.of([f.get(n) for n in f.names], g)
            oks = [_eval(o, sub) for o in order_by]
            perm = _sort_idx(oks, False, False)
            g = [g[i] for i in perm]
        sub = _Frame.of([f.get(n) for n in f.names], g)
        r = _eval(inner, sub)
        dtype = r.dtype if dtype is None or dtype._kind == "null" else dtype
        per.append((g, r))
    if strategy == "join":
        for g, r in per:
            for i in g:
                out[i] = list(r._v)
        return _new(_out_name(inner), out, _ListType(dtype if dtype is not None else Null))
    if strategy == "explode":
        vals = []
        for g, r in per:
            vals.extend(r._v)
        return _new(_out_name(inner), vals, dtype if dtype is not None else Null)
    for g, r in per:
        if _len(r._v) == 1 and (_is_scalar(inner) or _len(g) != 1):
            for i in g:
                out[i] = r._v[0]
        elif _len(r._v) == _len(g):
            for i, v in zip(g, r._v):
                out[i] = v
        else:
            raise ComputeError("the length of the window expression did not match that of the group")
    if dtype is None:
        r = _eval(inner, _Frame.of([_new(n, [], f.dtype(n)) for n in f.names]))
        dtype = r.dtype
    return _new(_out_name(inner), out, dtype)


def _key_of(v):
    if isinstance(v, list):
        return ("l",) + tuple(_key_of(x) for x in v)
    if isinstance(v, float):
        if v != v:
            return ("nan",)
        if v == 0.0:
            return 0.0
    return v


def _group_rows(keys, height):
    # row indices of each distinct key, in order of first appearance
    groups = {}
    order = []
    if _len(keys) == 1:
        kv = keys[0]._v
        for i in range(height):
            k = _key_of(kv[i])
            g = groups.get(k)
            if g is None:
                g = []
                groups[k] = g
                order.append(g)
            g.append(i)
        return order
    cols = [k._v for k in keys]
    for i in range(height):
        k = tuple(_key_of(c[i]) for c in cols)
        g = groups.get(k)
        if g is None:
            g = []
            groups[k] = g
            order.append(g)
        g.append(i)
    return order


# -- sorting --


class _Rev:
    __slots__ = ("v",)

    def __init__(self, v):
        self.v = v

    def __lt__(self, o):
        return o.v < self.v

    def __eq__(self, o):
        return self.v == o.v


def _sort_key_val(v):
    if isinstance(v, float) and v != v:
        return (1, 0.0)
    if isinstance(v, list):
        return (0, tuple(_sort_key_val(x) for x in v))
    return (0, v)


def _sort_idx(keys, descending, nulls_last):
    n = _len(keys[0]._v) if keys else 0
    if not isinstance(descending, (list, tuple)):
        descending = [descending] * _len(keys)
    if not isinstance(nulls_last, (list, tuple)):
        nulls_last = [nulls_last] * _len(keys)
    if _len(descending) != _len(keys):
        raise ValueError("the length of `descending` (" + str(_len(descending)) + ") does not match the length of `by` (" + str(_len(keys)) + ")")
    idx = list(range(n))
    for k in range(_len(keys) - 1, -1, -1):
        vals = keys[k]._v
        desc = descending[k]
        nl = nulls_last[k]
        nulls = [i for i in idx if vals[i] is None]
        rest = [i for i in idx if vals[i] is not None]
        rest = sorted(rest, key=lambda i: _sort_key_val(vals[i]), reverse=desc)
        idx = rest + nulls if nl else nulls + rest
    return idx


# -- functions (Expr._f) --


def _arg(a, f, n=None):
    if isinstance(a, Expr):
        s = _eval(a, f)
        return s
    return a


def _call_fn(e, f):
    name, inner, args, kw = e._a
    s = _eval(inner, f)
    fn = _FNS.get(name)
    if fn is None:
        raise ComputeError("unsupported expression function " + name)
    args = tuple(_arg(a, f) for a in args)
    r = fn(s, f, *args)
    if r._lit is None and s._lit is not None and name not in _AGG and name not in ("value_counts", "unique_counts"):
        r = _new(r.name, r._v, r.dtype, s._lit)
    return r


def _same(s, vals, dtype=None):
    return _new(s.name, vals, s.dtype if dtype is None else dtype)


def _one(s, v, dtype):
    return _new(s.name, [v], dtype)


def _fn_neg(s, f):
    if s.dtype._kind in ("int", "uint"):
        return _same(s, [None if v is None else _wrap(-v, s.dtype) for v in s._v])
    if s.dtype._kind == "float":
        return _same(s, [None if v is None else -v for v in s._v])
    raise InvalidOperationError("`neg` operation not supported for dtype `" + _short(s.dtype) + "`")


def _fn_not(s, f):
    if s.dtype._kind == "bool":
        return _same(s, [None if v is None else not v for v in s._v])
    if s.dtype.is_integer():
        return _same(s, [None if v is None else _wrap(~v, s.dtype) for v in s._v])
    if s.dtype._kind == "null":
        return _same(s, list(s._v), Boolean)
    raise InvalidOperationError("dtype " + repr(s.dtype) + " not supported in 'not' operation")


def _fn_abs(s, f):
    if not s.dtype.is_numeric():
        raise InvalidOperationError("`abs` operation not supported for dtype `" + _short(s.dtype) + "`")
    return _same(s, [None if v is None else (_wrap(_abs(v), s.dtype) if isinstance(v, int) else _abs(v)) for v in s._v])


def _float_out(s):
    return Float32 if s.dtype == Float32 else Float64


def _fn_math(s, f, which):
    if not (s.dtype.is_numeric() or s.dtype._kind == "null"):
        raise InvalidOperationError("`" + which + "` operation not supported for dtype `" + _short(s.dtype) + "`")
    d = _float_out(s)
    fn = {"sqrt": _sqrt, "cbrt": _cbrt, "exp": _exp, "log1p": _log1p, "sin": math.sin, "cos": math.cos, "tan": math.tan,
          "asin": _asin, "acos": _acos, "atan": math.atan, "sinh": _sinh, "cosh": _cosh, "tanh": math.tanh,
          "degrees": math.degrees, "radians": math.radians}[which]
    out = []
    for v in s._v:
        if v is None:
            out.append(None)
            continue
        x = float(v)
        if x != x:
            out.append(x)
            continue
        if x in (math.inf, -math.inf) and which in ("sin", "cos", "tan"):
            out.append(math.nan)
            continue
        r = fn(x)
        out.append(_f32(r) if d._bits == 32 else r)
    return _same(s, out, d)


def _sqrt(x):
    if x < 0:
        return math.nan
    return math.sqrt(x)


def _cbrt(x):
    if x == 0.0 or x != x or x in (math.inf, -math.inf):
        return x
    r = _abs(x) ** (1.0 / 3.0)
    # one Newton step to the correctly rounded cube root (libm's cbrt)
    r = r - (r * r * r - _abs(x)) / (3.0 * r * r)
    return math.copysign(r, x)


def _exp(x):
    try:
        return math.exp(x)
    except OverflowError:
        return math.inf


def _log1p(x):
    if x < -1:
        return math.nan
    if x == -1:
        return -math.inf
    return math.log1p(x)


def _asin(x):
    return math.asin(x) if -1 <= x <= 1 else math.nan


def _acos(x):
    return math.acos(x) if -1 <= x <= 1 else math.nan


def _sinh(x):
    try:
        return math.sinh(x)
    except OverflowError:
        return math.copysign(math.inf, x)


def _cosh(x):
    try:
        return math.cosh(x)
    except OverflowError:
        return math.inf


def _fn_log(s, f, base):
    d = _float_out(s)
    out = []
    for v in s._v:
        if v is None:
            out.append(None)
            continue
        x = float(v)
        if x != x:
            r = x
        elif x < 0:
            r = math.nan
        elif x == 0:
            r = -math.inf
        elif x == math.inf:
            r = math.inf
        elif base == math.e:
            r = math.log(x)
        elif base == 10.0:
            r = math.log10(x)
        elif base == 2.0:
            r = math.log2(x)
        else:
            r = math.log(x) / math.log(base)
        out.append(_f32(r) if d._bits == 32 else r)
    return _same(s, out, d)


def _fn_floor(s, f):
    if s.dtype.is_integer():
        return s
    return _same(s, [None if v is None else (v if v != v or v in (math.inf, -math.inf) else float(math.floor(v))) for v in s._v])


def _fn_ceil(s, f):
    if s.dtype.is_integer():
        return s
    return _same(s, [None if v is None else (v if v != v or v in (math.inf, -math.inf) else float(math.ceil(v))) for v in s._v])


def _fn_round(s, f, decimals, mode):
    if s.dtype.is_integer():
        return s
    if s.dtype._kind != "float":
        raise InvalidOperationError("`round` operation not supported for dtype `" + _short(s.dtype) + "`")
    out = []
    for v in s._v:
        if v is None or v != v or v in (math.inf, -math.inf):
            out.append(v)
            continue
        # as polars: round(v * 10^d) / 10^d, the value itself when that is not finite
        if decimals >= 326:
            out.append(v)
            continue
        if decimals >= 300:
            # 10^d is not a double: 2^d and 5^d apart
            x = v * math.ldexp(1.0, decimals) * _powi(5.0, decimals)
        else:
            m = _powi(10.0, decimals)
            x = v * m if decimals else v
        if x in (math.inf, -math.inf) or x != x:
            out.append(v)
            continue
        r = _round_half_away(x) if mode == "half_away_from_zero" else math.copysign(float(round(x)), x)
        if decimals >= 300:
            r = r / _powi(5.0, decimals) * (1.0 / math.ldexp(1.0, decimals))
        elif decimals:
            r = r / m
        out.append(v if r in (math.inf, -math.inf) or r != r else float(r))
    return _same(s, out)


def _fn_round_sig(s, f, digits):
    out = []
    for v in s._v:
        if v is None or v == 0 or v != v:
            out.append(v)
            continue
        out.append(float("%.*g" % (digits, v)))
    return _same(s, out, s.dtype if s.dtype._kind == "float" else Float64)


def _fn_sign(s, f):
    out = []
    for v in s._v:
        if v is None:
            out.append(None)
        elif isinstance(v, float):
            out.append(v if (v != v or v == 0.0) else (1.0 if v > 0 else -1.0))
        else:
            out.append(1 if v > 0 else (-1 if v < 0 else 0))
    return _same(s, out)


def _fn_clip(s, f, lo, hi):
    n = _len(s._v)
    lv = _broadcast(lo, n)._v if lo is not None else None
    hv = _broadcast(hi, n)._v if hi is not None else None
    out = []
    for i, v in enumerate(s._v):
        if v is None:
            out.append(None)
            continue
        if lv is not None and lv[i] is not None and v < lv[i]:
            v = lv[i]
        if hv is not None and hv[i] is not None and v > hv[i]:
            v = hv[i]
        out.append(float(v) if s.dtype._kind == "float" else v)
    return _same(s, out)


def _fn_is_null(s, f):
    return _same(s, [v is None for v in s._v], Boolean)


def _fn_is_not_null(s, f):
    return _same(s, [v is not None for v in s._v], Boolean)


def _need_float(s, what):
    if s.dtype._kind not in ("float", "int", "uint", "null"):
        raise InvalidOperationError("`" + what + "` operation not supported for dtype `" + _short(s.dtype) + "`")


def _fn_is_nan(s, f):
    _need_float(s, "is_nan")
    return _same(s, [None if v is None else (isinstance(v, float) and v != v) for v in s._v], Boolean)


def _fn_is_not_nan(s, f):
    _need_float(s, "is_not_nan")
    return _same(s, [None if v is None else not (isinstance(v, float) and v != v) for v in s._v], Boolean)


def _fn_is_finite(s, f):
    return _same(s, [None if v is None else (v == v and v not in (math.inf, -math.inf)) for v in s._v], Boolean)


def _fn_is_infinite(s, f):
    return _same(s, [None if v is None else (v in (math.inf, -math.inf)) for v in s._v], Boolean)


def _fn_fill_null(s, f, val):
    n = _len(s._v)
    if s.dtype._kind == "null":
        d = val.dtype if val._lit is None else (Int32 if val._lit == "int" else val.dtype)
    elif val._lit is not None:
        if val._lit == "int" and s.dtype.is_numeric():
            d = s.dtype
        elif val._lit == "float" and s.dtype.is_integer():
            d = Float64
        elif val._lit == "float" and s.dtype._kind == "float":
            d = s.dtype
        else:
            d = _supertype(s.dtype, val.dtype)
    else:
        d = _supertype(s.dtype, val.dtype)
    a = _cast(s, d, False) if s.dtype != d else s
    b = _cast(_broadcast(val, n), d, False)
    return _new(s.name, [x if x is not None else y for x, y in zip(a._v, b._v)], d)


def _fn_fill_null_strategy(s, f, strategy, limit):
    v = list(s._v)
    if strategy in ("forward", "backward"):
        rng = range(_len(v)) if strategy == "forward" else range(_len(v) - 1, -1, -1)
        last = None
        run = 0
        for i in rng:
            if v[i] is None:
                if last is not None and (limit is None or run < limit):
                    v[i] = last
                    run += 1
            else:
                last = v[i]
                run = 0
        return _same(s, v)
    nn = _nonnull(v)
    if strategy == "zero":
        rep = 0.0 if s.dtype._kind == "float" else 0
    elif strategy == "one":
        rep = 1.0 if s.dtype._kind == "float" else 1
    elif strategy == "min":
        rep = _agg_min(nn, s.dtype)
    elif strategy == "max":
        rep = _agg_max(nn, s.dtype)
    elif strategy == "mean":
        rep = _agg_mean(s)
        if s.dtype.is_integer() and rep is not None:
            rep = int(rep)
    else:
        raise ValueError("strategy " + repr(strategy) + " is not supported")
    return _same(s, [rep if x is None else x for x in v])


def _fn_fill_nan(s, f, val):
    n = _len(s._v)
    b = _broadcast(val, n)
    d = s.dtype if (b._lit is not None or b.dtype._kind == "float") else _supertype(s.dtype, b.dtype)
    return _new(s.name, [y if (x is not None and x != x) else x for x, y in zip(s._v, b._v)], d)


def _fn_is_in(s, f, other, nulls_equal):
    if other._lit is not None or _len(other._v) != _len(s._v) or other.dtype._kind != "list":
        vals = other._v
        if other.dtype._kind == "list" and _len(vals) == 1:
            vals = vals[0] or []
        sk = s.dtype._kind
        ok = [x for x in vals if x is not None]
        for x in ok:
            xk = _py_dtype(x)._kind
            if (sk == "str") != (xk == "str") and sk != "null":
                raise InvalidOperationError("'is_in' cannot check for " + ("String" if xk == "str" else repr(_py_dtype(x))) + " values in " + repr(s.dtype) + " data")
        st = set(_key_of(x) for x in ok)
        has_null = _len(ok) != _len(vals)
        out = []
        for v in s._v:
            if v is None:
                out.append(has_null if nulls_equal else None)
            else:
                out.append(_key_of(v) in st)
        return _same(s, out, Boolean)
    out = []
    for v, l in zip(s._v, other._v):
        out.append(None if v is None else (l is not None and v in l))
    return _same(s, out, Boolean)


def _fn_replace(s, f, mapping, strict, default, return_dtype):
    m = {}
    for k, v in mapping.items():
        m[_key_of(k)] = v
    out = []
    for v in s._v:
        k = _key_of(v)
        if k in m:
            out.append(m[k])
        elif strict:
            if default is not None:
                out.append(default)
            elif v is None:
                out.append(None)
            else:
                raise InvalidOperationError("incomplete mapping specified for `replace_strict`\n\nHint: Pass a `default` value to set unmapped values.")
        else:
            out.append(v)
    if return_dtype is not None:
        return _from_values(s.name, out, return_dtype, False)
    if not strict:
        vals = [v for v in mapping.values() if v is not None]
        d = s.dtype
        for v in vals:
            t = _py_dtype(v)
            if t != d:
                if d.is_numeric() and t.is_numeric():
                    d = d if (t._kind != "float" or d._kind == "float") else Float64
                else:
                    d = _supertype(d, t)
        return _from_values(s.name, out, d, False)
    return _from_values(s.name, out, None, False)


def _fn_map_elements(s, f, fn, return_dtype, skip_nulls):
    out = []
    for v in s._v:
        if v is None and skip_nulls:
            out.append(None)
            continue
        out.append(fn(list(v) if isinstance(v, list) else v))
    if return_dtype is not None:
        return _from_values(s.name, out, return_dtype, False)
    return _from_values(s.name, out, None, False)


def _fn_map_batches(s, f, fn, return_dtype, returns_scalar):
    r = fn(_new(s.name, list(s._v), s.dtype))
    if not isinstance(r, Series):
        r = Series(s.name, [r])
    if return_dtype is not None:
        r = _cast(r, _dt(return_dtype), True)
    return _new(s.name, r._v, r.dtype)


def _fn_shift(s, f, n, fill):
    v = s._v
    k = _len(v)
    if isinstance(n, Series):
        n = n._v[0]
    fv = fill._v[0] if isinstance(fill, Series) else fill
    if n >= 0:
        out = [fv] * _min(n, k) + v[:_max(0, k - n)]
    else:
        m = -n
        out = v[m:] + [fv] * _min(m, k)
    return _same(s, out)


def _fn_diff(s, f, n):
    v = s._v
    d = s.dtype
    if d._kind == "uint":
        d = {8: Int16, 16: Int32, 32: Int64, 64: Int64}[d._bits]
    out = []
    for i in range(_len(v)):
        j = i - n
        if j < 0 or j >= _len(v) or v[i] is None or v[j] is None:
            out.append(None)
        else:
            r = v[i] - v[j]
            out.append(_wrap(r, d) if isinstance(r, int) else r)
    return _same(s, out, d)


def _fn_pct_change(s, f, n):
    v = s._v
    out = []
    # nulls are forward filled first
    filled = []
    last = None
    for x in v:
        if x is not None:
            last = x
        filled.append(last)
    for i in range(_len(v)):
        j = i - n
        if j < 0 or filled[i] is None or filled[j] is None:
            out.append(None)
        else:
            a, b = float(filled[i]), float(filled[j])
            if b == 0.0:
                out.append(math.nan if a == 0.0 else math.copysign(math.inf, a) * math.copysign(1.0, b))
            else:
                out.append(a / b - 1.0)
    return _same(s, out, Float64)


def _fn_cum(s, f, kind, reverse):
    v = list(reversed(s._v)) if reverse else s._v
    d = s.dtype
    if kind in ("sum", "prod"):
        if d._kind == "bool":
            d = UInt32
        elif d.is_integer() and d._bits < 64:
            d = Int64 if d._kind == "int" or kind == "prod" else (UInt64 if d._bits == 32 else Int64)
            if s.dtype._kind == "uint" and s.dtype._bits == 32:
                d = UInt32 if kind == "sum" else UInt64
    if kind == "count":
        d = UInt32
    out = []
    acc = None
    cnt = 0
    for x in v:
        if kind == "count":
            if x is not None:
                cnt += 1
            out.append(cnt)
            continue
        if x is None:
            out.append(None)
            continue
        if acc is None:
            acc = int(x) if d.is_integer() else x
        elif kind == "sum":
            acc = acc + x
        elif kind == "prod":
            acc = acc * x
        elif kind == "min":
            if x < acc or (x == x and acc != acc):
                acc = x
        elif kind == "max":
            if x > acc or (x == x and acc != acc):
                acc = x
        if isinstance(acc, int) and d.is_integer():
            acc = _wrap(acc, d)
        out.append(acc)
    if reverse:
        out.reverse()
    return _same(s, out, d)


def _fn_rank(s, f, method, descending):
    v = s._v
    idx = [i for i in range(_len(v)) if v[i] is not None]
    order = sorted(idx, key=lambda i: _sort_key_val(v[i]), reverse=descending)
    ranks = [None] * _len(v)
    if method == "ordinal":
        for r, i in enumerate(order):
            ranks[i] = r + 1
        return _same(s, ranks, UInt32)
    i = 0
    dense = 0
    while i < _len(order):
        j = i
        while j + 1 < _len(order) and _key_of(v[order[j + 1]]) == _key_of(v[order[i]]):
            j += 1
        dense += 1
        for k in range(i, j + 1):
            if method == "average":
                ranks[order[k]] = (i + 1 + j + 1) / 2.0
            elif method == "min":
                ranks[order[k]] = i + 1
            elif method == "max":
                ranks[order[k]] = j + 1
            elif method == "dense":
                ranks[order[k]] = dense
            else:
                ranks[order[k]] = k + 1
        i = j + 1
    return _same(s, ranks, Float64 if method == "average" else UInt32)


def _fn_sort(s, f, descending, nulls_last):
    order = _sort_idx([s], descending, nulls_last)
    return _same(s, [s._v[i] for i in order])


def _fn_arg_sort(s, f, descending, nulls_last):
    return _same(s, _sort_idx([s], descending, nulls_last), UInt32)


def _fn_reverse(s, f):
    return _same(s, list(reversed(s._v)))


def _uniq_idx(v):
    seen = set()
    out = []
    for i, x in enumerate(v):
        k = _key_of(x)
        if k not in seen:
            seen.add(k)
            out.append(i)
    return out


def _fn_unique(s, f):
    return _same(s, [s._v[i] for i in _uniq_idx(s._v)])


def _fn_unique_counts(s, f):
    c = {}
    order = []
    for x in s._v:
        k = _key_of(x)
        if k not in c:
            c[k] = 0
            order.append(k)
        c[k] += 1
    return _same(s, [c[k] for k in order], UInt32)


def _fn_arg_unique(s, f):
    return _same(s, _uniq_idx(s._v), UInt32)


def _counts(v):
    c = {}
    for x in v:
        k = _key_of(x)
        c[k] = c.get(k, 0) + 1
    return c


def _fn_is_duplicated(s, f):
    c = _counts(s._v)
    return _same(s, [c[_key_of(x)] > 1 for x in s._v], Boolean)


def _fn_is_unique(s, f):
    c = _counts(s._v)
    return _same(s, [c[_key_of(x)] == 1 for x in s._v], Boolean)


def _fn_is_first_distinct(s, f):
    seen = set()
    out = []
    for x in s._v:
        k = _key_of(x)
        out.append(k not in seen)
        seen.add(k)
    return _same(s, out, Boolean)


def _fn_is_last_distinct(s, f):
    seen = set()
    out = []
    for x in reversed(s._v):
        k = _key_of(x)
        out.append(k not in seen)
        seen.add(k)
    out.reverse()
    return _same(s, out, Boolean)


def _fn_drop_nulls(s, f):
    return _same(s, [x for x in s._v if x is not None])


def _fn_drop_nans(s, f):
    return _same(s, [x for x in s._v if not (isinstance(x, float) and x != x)])


def _fn_head(s, f, n):
    if n < 0:
        n = _max(0, _len(s._v) + n)
    return _same(s, s._v[:n])


def _fn_tail(s, f, n):
    if n < 0:
        n = _max(0, _len(s._v) + n)
    return _same(s, s._v[_len(s._v) - n:] if n else [])


def _fn_slice(s, f, offset, length):
    if isinstance(offset, Series):
        offset = offset._v[0]
    if isinstance(length, Series):
        length = length._v[0]
    n = _len(s._v)
    o = offset + n if offset < 0 else offset
    o = _max(0, _min(o, n))
    e = n if length is None else _min(n, o + length)
    return _same(s, s._v[o:e])


def _fn_gather(s, f, idx):
    n = _len(s._v)
    out = []
    for i in idx._v:
        if i is None:
            out.append(None)
            continue
        j = i + n if i < 0 else i
        if j < 0 or j >= n:
            raise OutOfBoundsError("gather indices are out of bounds")
        out.append(s._v[j])
    return _same(s, out)


def _fn_get(s, f, idx):
    i = idx._v[0]
    n = _len(s._v)
    j = i + n if i < 0 else i
    if j < 0 or j >= n:
        raise OutOfBoundsError("gather indices are out of bounds")
    return _same(s, [s._v[j]])


def _fn_gather_every(s, f, n, offset):
    return _same(s, s._v[offset::n])


def _fn_top_k(s, f, k, top):
    nn = [x for x in s._v if x is not None]
    vs = sorted(nn, key=_sort_key_val, reverse=top)
    out = vs[:k]
    if _len(out) < k:
        out = out + [None] * _min(k - _len(out), _len(s._v) - _len(nn))
    return _same(s, out)


def _fn_mode(s, f):
    c = {}
    first = {}
    for x in s._v:
        k = _key_of(x)
        c[k] = c.get(k, 0) + 1
        if k not in first:
            first[k] = x
    if not c:
        return _same(s, [])
    m = _max(c.values())
    return _same(s, [first[k] for k in first if c[k] == m])


def _fn_explode(s, f, empty_as_null=False, keep_nulls=True):
    # an empty list gives no row (a null with empty_as_null), a null list a null (none without keep_nulls)
    if s.dtype._kind == "str":
        out = []
        for x in s._v:
            if x is None:
                out.append(None)
            elif x == "":
                out.append(None)
            else:
                out.extend(list(x))
        return _same(s, out)
    if s.dtype._kind != "list":
        raise InvalidOperationError("`explode` operation not supported for dtype `" + _short(s.dtype) + "`")
    out = []
    for x in s._v:
        if x is None:
            if keep_nulls:
                out.append(None)
        elif _len(x) == 0:
            if empty_as_null:
                out.append(None)
        else:
            out.extend(x)
    return _same(s, out, s.dtype.inner if s.dtype.inner is not None else Null)


def _fn_interpolate(s, f, method):
    v = list(s._v)
    known = [i for i, x in enumerate(v) if x is not None]
    out = list(v)
    for a, b in zip(known, known[1:]):
        for i in range(a + 1, b):
            if method == "nearest":
                out[i] = v[a] if (i - a) <= (b - i) else v[b]
            else:
                out[i] = v[a] + (v[b] - v[a]) * (i - a) / (b - a)
    d = s.dtype if (method == "nearest" or s.dtype._kind == "float") else Float64
    if d._kind == "float":
        out = [None if x is None else float(x) for x in out]
    return _same(s, out, d)


def _fn_rolling(s, f, kind, w, min_samples, center):
    v = s._v
    n = _len(v)
    ms = w if min_samples is None else min_samples
    out = []
    d = s.dtype
    if kind in ("mean", "std", "var", "median") or (kind == "sum" and d._kind == "float"):
        d = _float_out(s) if kind != "sum" else d
    for i in range(n):
        if center:
            lo = i - w // 2
            hi = lo + w
        else:
            lo = i - w + 1
            hi = i + 1
        win = [x for x in v[_max(0, lo):_max(0, hi)] if x is not None]
        if _len(win) < ms:
            out.append(None)
            continue
        if kind == "sum":
            out.append(_sum(win) if d.is_integer() else _sum(float(x) for x in win))
        elif kind == "mean":
            out.append(_sum(float(x) for x in win) / _len(win))
        elif kind == "min":
            out.append(_min(win))
        elif kind == "max":
            out.append(_max(win))
    return _same(s, out, d)


def _fn_repeat_by(s, f, by):
    b = _broadcast(by, _len(s._v))._v
    return _same(s, [None if (k is None) else [x] * k for x, k in zip(s._v, b)], _ListType(s.dtype))


# aggregations


def _agg_min(nn, d):
    if not nn:
        return None
    if d._kind == "float":
        xs = [x for x in nn if x == x]
        return _min(xs) if xs else math.nan
    return _min(nn)


def _agg_max(nn, d):
    if not nn:
        return None
    if d._kind == "float":
        xs = [x for x in nn if x == x]
        return _max(xs) if xs else math.nan
    return _max(nn)


def _sum_dtype(d):
    if d._kind == "bool":
        return UInt32
    if d.is_integer() and d._bits < 32:
        return Int64
    if d._kind == "null":
        return Int64
    return d


def _agg_sum(s):
    d = _sum_dtype(s.dtype)
    nn = _nonnull(s._v)
    if d._kind == "float":
        r = _fsum([float(x) for x in nn])
        return (_f32(r) if d._bits == 32 else r), d
    if d._kind in ("int", "uint"):
        return _wrap(_sum(int(x) for x in nn), d), d
    if d._kind == "str":
        raise InvalidOperationError("`sum` operation not supported for dtype `str`")
    raise InvalidOperationError("`sum` operation not supported for dtype `" + _short(s.dtype) + "`")


def _agg_mean(s):
    nn = _nonnull(s._v)
    if not nn:
        return None
    if s.dtype._kind == "bool":
        return _sum(1 for x in nn if x) / _len(nn)
    return _fsum([float(x) for x in nn]) / _len(nn)


def _fn_sum(s, f):
    if s.dtype._kind not in ("int", "uint", "float", "bool", "null"):
        raise InvalidOperationError("`sum` operation not supported for dtype `" + _short(s.dtype) + "`")
    v, d = _agg_sum(s)
    return _one(s, v, d)


def _fn_mean(s, f):
    if s.dtype._kind not in ("int", "uint", "float", "bool", "null"):
        if s.dtype._kind == "str":
            return _one(s, None, String)
        raise InvalidOperationError("`mean` operation not supported for dtype `" + _short(s.dtype) + "`")
    m = _agg_mean(s)
    if s.dtype == Float32:
        return _one(s, None if m is None else _f32(m), Float32)
    return _one(s, m, Float64)


def _fn_median(s, f):
    if s.dtype._kind not in ("int", "uint", "float", "bool", "null"):
        if s.dtype._kind == "str":
            return _one(s, None, String)
        raise InvalidOperationError("`median` operation not supported for dtype `" + _short(s.dtype) + "`")
    m = _quantile(s._v, 0.5, "linear")
    return _one(s, m, Float32 if s.dtype == Float32 else Float64)


def _fn_quantile(s, f, q, interp):
    if isinstance(q, Series):
        q = q._v[0]
    m = _quantile(s._v, q, interp)
    if s.dtype == Float32:
        return _one(s, m, Float32)
    return _one(s, m, Float64)


def _fn_min(s, f):
    nn = _nonnull(s._v)
    if s.dtype._kind == "list":
        raise InvalidOperationError("`min` operation not supported for dtype `" + _short(s.dtype) + "`")
    return _one(s, _agg_min(nn, s.dtype), s.dtype)


def _fn_max(s, f):
    nn = _nonnull(s._v)
    if s.dtype._kind == "list":
        raise InvalidOperationError("`max` operation not supported for dtype `" + _short(s.dtype) + "`")
    return _one(s, _agg_max(nn, s.dtype), s.dtype)


def _fn_nan_min(s, f):
    nn = _nonnull(s._v)
    if _any(x != x for x in nn):
        return _one(s, math.nan, s.dtype)
    return _one(s, _min(nn) if nn else None, s.dtype)


def _fn_nan_max(s, f):
    nn = _nonnull(s._v)
    if _any(x != x for x in nn):
        return _one(s, math.nan, s.dtype)
    return _one(s, _max(nn) if nn else None, s.dtype)


def _fn_std(s, f, ddof):
    if s.dtype._kind not in ("int", "uint", "float", "bool", "null"):
        raise InvalidOperationError("`std` operation not supported for dtype `" + _short(s.dtype) + "`")
    v = _var(_floats(s._v), ddof)
    r = None if v is None else math.sqrt(v)
    return _one(s, r if s.dtype != Float32 or r is None else _f32(r), Float32 if s.dtype == Float32 else Float64)


def _fn_var(s, f, ddof):
    if s.dtype._kind not in ("int", "uint", "float", "bool", "null"):
        raise InvalidOperationError("`var` operation not supported for dtype `" + _short(s.dtype) + "`")
    v = _var(_floats(s._v), ddof)
    return _one(s, v if s.dtype != Float32 or v is None else _f32(v), Float32 if s.dtype == Float32 else Float64)


def _fn_product(s, f):
    d = s.dtype
    if d.is_integer() or d._kind == "bool":
        d = Int64 if not (d._kind == "uint" and d._bits == 64) else UInt64
        r = 1
        for x in s._v:
            if x is not None:
                r = _wrap(r * int(x), d)
        return _one(s, r, d)
    r = 1.0
    for x in s._v:
        if x is not None:
            r = r * x
    return _one(s, r, d)


def _fn_count(s, f):
    return _one(s, _sum(1 for x in s._v if x is not None), UInt32)


def _fn_len(s, f):
    return _one(s, _len(s._v), UInt32)


def _fn_null_count(s, f):
    return _one(s, _sum(1 for x in s._v if x is None), UInt32)


def _fn_n_unique(s, f):
    return _one(s, _len(set(_key_of(x) for x in s._v)), UInt32)


def _fn_first(s, f):
    return _one(s, s._v[0] if s._v else None, s.dtype)


def _fn_last(s, f):
    return _one(s, s._v[-1] if s._v else None, s.dtype)


def _fn_arg_min(s, f):
    best = None
    for i, x in enumerate(s._v):
        if x is None or (isinstance(x, float) and x != x):
            continue
        if best is None or x < s._v[best]:
            best = i
    return _one(s, best, UInt32)


def _fn_arg_max(s, f):
    best = None
    for i, x in enumerate(s._v):
        if x is None or (isinstance(x, float) and x != x):
            continue
        if best is None or x > s._v[best]:
            best = i
    return _one(s, best, UInt32)


def _fn_any(s, f, ignore_nulls):
    if s.dtype._kind not in ("bool", "null"):
        raise InvalidOperationError("expected boolean, got " + _short(s.dtype))
    if _any(x is True for x in s._v):
        return _one(s, True, Boolean)
    if not ignore_nulls and _any(x is None for x in s._v):
        return _one(s, None, Boolean)
    return _one(s, False, Boolean)


def _fn_all(s, f, ignore_nulls):
    if s.dtype._kind not in ("bool", "null"):
        raise InvalidOperationError("expected boolean, got " + _short(s.dtype))
    if _any(x is False for x in s._v):
        return _one(s, False, Boolean)
    if not ignore_nulls and _any(x is None for x in s._v):
        return _one(s, None, Boolean)
    return _one(s, True, Boolean)


def _fn_implode(s, f):
    return _one(s, list(s._v), _ListType(s.dtype))


def _fn_value_counts(s, f, sort, name, normalize):
    c = {}
    first = {}
    order = []
    for x in s._v:
        k = _key_of(x)
        if k not in c:
            c[k] = 0
            first[k] = x
            order.append(k)
        c[k] += 1
    if sort:
        order = sorted(order, key=lambda k: -c[k])
    total = _len(s._v)
    cname = name if name is not None else ("proportion" if normalize else "count")
    counts = [(c[k] / total if normalize else c[k]) for k in order]
    return _new(s.name, [[first[k], n] for k, n in zip(order, counts)], _StructType([(s.name, s.dtype), (cname, Float64 if normalize else UInt32)]))


def _fn_entropy(s, f, base, normalize):
    # as polars: -sum(pk * log_base(pk)), pk = x / sum(x) (0 * log 0 is NaN there too)
    if _len(s._v) == 1:
        return _one(s, 0.0, Float64)
    xs = _floats(s._v)
    if normalize:
        t = _fsum(xs)
        if t != 1.0:
            xs = [x / t for x in xs]
    lb = math.log(base)

    def lg(x):
        if x != x or x < 0:
            return math.nan
        if x == 0:
            return -math.inf
        return math.log(x) / lb

    return _one(s, -_fsum([x * lg(x) for x in xs]), Float64)


def _moments(xs, k4):
    # polars-ops moment.rs SkewState / KurtosisState over chunks of 128: (weight, mean, m2, m3, m4). The sums
    # are in order; polars lets LLVM reorder them, so the last bits can differ on more than a few values.
    w, mean, m2, m3, m4 = 0.0, 0.0, 0.0, 0.0, 0.0
    for c in range(0, _len(xs), 128):
        ch = xs[c:c + 128]
        ow = float(_len(ch))
        s = -0.0
        for x in ch:
            s = s + x
        om = s / ow
        o2, o3, o4 = 0.0, 0.0, 0.0
        for x in ch:
            d = x - om
            d2 = d * d
            o2 = o2 + d2
            o3 = o3 + d * d2
            o4 = o4 + d2 * d2
        if w == 0.0:
            w, mean, m2, m3, m4 = ow, om, o2, o3, o4
            continue
        nw = w + ow
        dm = om - mean
        dmw = dm / nw
        nm = mean + ow * dmw
        wd = w - ow
        swo = w * o2
        ows = ow * m2
        up = ow * (om - nm) * dm
        n2 = m2 + o2 + up
        n3 = m3 + o3 + dmw * (up * wd + 3.0 * (swo - ows))
        n4 = m4 + o4 + dmw * (dmw * (up * (w * wd + ow * ow) + 6.0 * (w * swo + ow * ows)) + 4.0 * (w * o3 - ow * m3))
        w, mean, m2, m3, m4 = nw, nm, n2, n3, n4
    return w, mean, m2, m3, m4


def _fn_skew(s, f, bias):
    w, mean, m2, m3, _ = _moments(_floats(s._v), False)
    if w == 0.0:
        return _one(s, None, Float64)
    m2, m3 = m2 / w, m3 / w
    est = math.nan if m2 <= (2.220446049250313e-16 * mean) ** 2 else m3 / m2 ** 1.5
    if bias:
        return _one(s, est, Float64)
    if w <= 2.0:
        return _one(s, None, Float64)
    return _one(s, math.sqrt(w * (w - 1.0)) / (w - 2.0) * est, Float64)


def _fn_kurtosis(s, f, fisher, bias):
    w, mean, m2, _, m4 = _moments(_floats(s._v), True)
    if w == 0.0:
        return _one(s, None, Float64)
    m4, m2 = m4 / w, m2 / w
    est = math.nan if m2 <= (2.220446049250313e-16 * mean) ** 2 else m4 / (m2 * m2)
    if bias:
        out = est
    else:
        if w <= 3.0:
            return _one(s, None, Float64)
        n = w
        out = (n - 1.0) / (n - 2.0) * ((n + 1.0) / (n - 3.0) * est - 3.0 * ((n - 1.0) / (n - 3.0))) + 3.0
    return _one(s, out - 3.0 if fisher else out, Float64)


class _StructType(DataType):
    # only as the result of value_counts, unnested right away
    def __init__(self, fields):
        DataType.__init__(self, "Struct", "struct", "struct")
        self.fields = fields

    def __repr__(self):
        return "Struct({" + ", ".join(repr(n) + ": " + repr(d) for n, d in self.fields) + "})"


# -- str namespace --


def _rx_repl(v):
    # a Rust regex replacement ($1, ${name}, $$) as a Python one
    out = []
    i = 0
    while i < _len(v):
        c = v[i]
        if c == "\\":
            out.append("\\\\")
            i += 1
            continue
        if c == "$" and i + 1 < _len(v):
            n = v[i + 1]
            if n == "$":
                out.append("$")
                i += 2
                continue
            if n == "{":
                j = v.find("}", i)
                if j > 0:
                    out.append("\\g<" + v[i + 2:j] + ">")
                    i = j + 1
                    continue
            j = i + 1
            while j < _len(v) and (v[j].isalnum() or v[j] == "_"):
                j += 1
            if j > i + 1:
                name = v[i + 1:j]
                out.append("\\g<" + name + ">")
                i = j
                continue
        out.append(c)
        i += 1
    return "".join(out)


_RX_CACHE = {}


def _rx(p):
    r = _RX_CACHE.get(p)
    if r is None:
        try:
            r = _re.compile(p)
        except Exception as e:
            raise ComputeError("invalid regex: " + str(e))
        _RX_CACHE[p] = r
    return r


def _need_str(s, what):
    if s.dtype._kind not in ("str", "null"):
        raise InvalidOperationError("expected String type, got: " + _short(s.dtype))


def _fn_str(s, f, op, *a):
    if op == "join":
        _need_str(s, op)
        sep, ign = a
        vals = [x for x in s._v if x is not None] if ign else s._v
        if not ign and _any(x is None for x in vals):
            return _one(s, None, String)
        return _one(s, sep.join(vals), String)
    _need_str(s, op)
    v = s._v
    n = _len(v)

    def arg(x):
        if isinstance(x, Series):
            return _broadcast(x, n)._v
        return [x] * n

    if op == "len_chars":
        return _same(s, [None if x is None else _len(x) for x in v], UInt32)
    if op == "len_bytes":
        return _same(s, [None if x is None else _len(x.encode("utf-8")) for x in v], UInt32)
    if op == "upper":
        return _same(s, [None if x is None else x.upper() for x in v])
    if op == "lower":
        return _same(s, [None if x is None else x.lower() for x in v])
    if op == "title":
        return _same(s, [None if x is None else _titlecase(x) for x in v])
    if op in ("strip", "lstrip", "rstrip"):
        ch = a[0]
        if isinstance(ch, Series):
            ch = ch._v[0]
        fn = {"strip": str.strip, "lstrip": str.lstrip, "rstrip": str.rstrip}[op]
        return _same(s, [None if x is None else (fn(x) if ch is None else fn(x, ch)) for x in v])
    if op in ("strip_prefix", "strip_suffix"):
        p = arg(a[0])
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
            elif op == "strip_prefix":
                out.append(x[_len(q):] if x.startswith(q) else x)
            else:
                out.append(x[:_len(x) - _len(q)] if (q and x.endswith(q)) else x)
        return _same(s, out)
    if op in ("starts_with", "ends_with"):
        p = arg(a[0])
        return _same(s, [None if (x is None or q is None) else (x.startswith(q) if op == "starts_with" else x.endswith(q)) for x, q in zip(v, p)], Boolean)
    if op == "contains":
        p = arg(a[0])
        literal = a[1]
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
            elif literal:
                out.append(q in x)
            else:
                out.append(_rx(q).search(x) is not None)
        return _same(s, out, Boolean)
    if op == "contains_any":
        pats, ci = a
        out = []
        for x in v:
            if x is None:
                out.append(None)
            elif ci:
                lx = x.lower()
                out.append(_any(p.lower() in lx for p in pats))
            else:
                out.append(_any(p in x for p in pats))
        return _same(s, out, Boolean)
    if op == "count_matches":
        p = arg(a[0])
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
            elif a[1]:
                out.append(x.count(q) if q else _len(x) + 1)
            else:
                out.append(_len(_rx(q).findall(x)))
        return _same(s, out, UInt32)
    if op == "find":
        p = arg(a[0])
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
                continue
            if a[1]:
                i = x.find(q)
                out.append(None if i < 0 else _len(x[:i].encode("utf-8")))
            else:
                m = _rx(q).search(x)
                out.append(None if m is None else _len(x[:m.start()].encode("utf-8")))
        return _same(s, out, UInt32)
    if op == "replace":
        p = arg(a[0])
        r = arg(a[1])
        literal, cnt = a[2], a[3]
        out = []
        for x, q, w in zip(v, p, r):
            if x is None or q is None or w is None:
                out.append(None)
            elif literal:
                out.append(x.replace(q, w) if cnt < 0 else x.replace(q, w, cnt))
            else:
                out.append(_rx(q).sub(_rx_repl(w), x, count=0 if cnt < 0 else cnt))
        return _same(s, out)
    if op == "replace_many":
        out = []
        for x in v:
            if x is None:
                out.append(None)
                continue
            res = []
            i = 0
            while i < _len(x):
                for pat, rep in a[0]:
                    if pat and x.startswith(pat, i):
                        res.append(rep)
                        i += _len(pat)
                        break
                else:
                    res.append(x[i])
                    i += 1
            out.append("".join(res))
        return _same(s, out)
    if op == "slice":
        off, ln = a
        if isinstance(off, Series):
            off = off._v[0]
        if isinstance(ln, Series):
            ln = ln._v[0]
        out = []
        for x in v:
            if x is None:
                out.append(None)
                continue
            m = _len(x)
            o = off + m if off < 0 else off
            o = _max(0, _min(o, m))
            e = m if ln is None else _min(m, o + ln)
            out.append(x[o:e])
        return _same(s, out)
    if op == "head":
        k = a[0]
        return _same(s, [None if x is None else (x[:k] if k >= 0 else x[:_max(0, _len(x) + k)]) for x in v])
    if op == "tail":
        k = a[0]
        return _same(s, [None if x is None else (x[_len(x) - k:] if 0 < k else ("" if k == 0 else x[-k:])) for x in v])
    if op == "split":
        by, inc = a
        out = []
        for x in v:
            if x is None:
                out.append(None)
            elif by == "":
                out.append(list(x))
            elif inc:
                parts = x.split(by)
                out.append([p + by for p in parts[:-1]] + [parts[-1]])
            else:
                out.append(x.split(by))
        return _same(s, out, _ListType(String))
    if op == "zfill":
        k = a[0]
        out = []
        for x in v:
            if x is None:
                out.append(None)
            elif x[:1] in ("-", "+"):
                out.append(x[0] + x[1:].rjust(k - 1, "0"))
            else:
                out.append(x.rjust(k, "0"))
        return _same(s, out)
    if op == "pad_start":
        return _same(s, [None if x is None else x.rjust(a[0], a[1]) for x in v])
    if op == "pad_end":
        return _same(s, [None if x is None else x.ljust(a[0], a[1]) for x in v])
    if op == "reverse":
        return _same(s, [None if x is None else x[::-1] for x in v])
    if op == "extract":
        p = arg(a[0])
        gi = a[1]
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
                continue
            m = _rx(q).search(x)
            out.append(None if m is None else m.group(gi))
        return _same(s, out)
    if op == "extract_all":
        p = arg(a[0])
        out = []
        for x, q in zip(v, p):
            if x is None or q is None:
                out.append(None)
                continue
            out.append([m.group(0) for m in _rx(q).finditer(x)])
        return _same(s, out, _ListType(String))
    if op == "to_integer":
        base, strict = a
        out = []
        bad = []
        for x in v:
            if x is None:
                out.append(None)
                continue
            try:
                r = int(x, base)
                if not (_I64_MIN <= r <= _I64_MAX):
                    raise ValueError()
                out.append(r)
            except ValueError:
                bad.append(x)
                out.append(None)
        if bad and strict:
            raise ComputeError("strict integer parsing failed for " + str(_len(bad)) + " value(s): [" + ", ".join('"' + b + '"' for b in bad[:10]) + "]")
        return _same(s, out, Int64)
    raise ComputeError("unsupported str function " + op)


def _titlecase(x):
    out = []
    prev = False
    for c in x:
        if c.isalnum():
            out.append(c.lower() if prev else c.upper())
            prev = True
        else:
            out.append(c)
            prev = False
    return "".join(out)


# -- list namespace --


def _fn_list(s, f, op, *a):
    if s.dtype._kind != "list":
        if s.dtype._kind == "null":
            pass
        else:
            raise InvalidOperationError("expected List type, got: " + _short(s.dtype))
    inner = s.dtype.inner if (s.dtype._kind == "list" and s.dtype.inner is not None) else Null
    v = s._v

    def sub(x):
        return _new(s.name, x, inner)

    if op == "len":
        return _same(s, [None if x is None else _len(x) for x in v], UInt32)
    if op in ("sum", "mean", "min", "max", "median", "std", "var", "n_unique", "arg_min", "arg_max", "any", "all"):
        out = []
        d = None
        for x in v:
            if x is None:
                out.append(None)
                continue
            r = _FNS[op](sub(x), f, *a) if op not in ("any", "all") else _FNS[op](sub(x), f, True)
            out.append(r._v[0])
            d = r.dtype
        if d is None:
            d = _FNS[op](sub([]), f, *a).dtype if op not in ("any", "all") else Boolean
        if op == "mean" and _all(x is None or _len(x) == 0 for x in v) and False:
            d = Float64
        return _same(s, out, d)
    if op == "get":
        idx, null_oob = a
        if isinstance(idx, Series):
            idx = idx._v[0]
        out = []
        for x in v:
            if x is None:
                out.append(None)
                continue
            j = idx + _len(x) if idx < 0 else idx
            if 0 <= j < _len(x):
                out.append(x[j])
            elif null_oob:
                out.append(None)
            else:
                raise ComputeError("get index is out of bounds")
        return _same(s, out, inner)
    if op == "contains":
        item = a[0]
        return _same(s, [None if x is None else (item in x) for x in v], Boolean)
    if op == "join":
        sep, ign = a
        out = []
        for x in v:
            if x is None:
                out.append(None)
            elif not ign and _any(y is None for y in x):
                out.append(None)
            else:
                out.append(sep.join(y for y in x if y is not None))
        return _same(s, out, String)
    if op == "sort":
        desc, nl = a
        return _same(s, [None if x is None else [x[i] for i in _sort_idx([sub(x)], desc, nl)] for x in v])
    if op == "unique":
        return _same(s, [None if x is None else [x[i] for i in _uniq_idx(x)] for x in v])
    if op == "reverse":
        return _same(s, [None if x is None else list(reversed(x)) for x in v])
    if op == "slice":
        off, ln = a
        out = []
        for x in v:
            if x is None:
                out.append(None)
                continue
            m = _len(x)
            o = off + m if off < 0 else off
            o = _max(0, _min(o, m))
            e = m if ln is None else _min(m, o + ln)
            out.append(x[o:e])
        return _same(s, out)
    if op == "tail":
        k = a[0]
        return _same(s, [None if x is None else (x[_len(x) - k:] if k else []) for x in v])
    if op == "count_matches":
        return _same(s, [None if x is None else _sum(1 for y in x if y == a[0]) for x in v], UInt32)
    if op == "concat":
        o = _broadcast(a[0], _len(v))
        out = []
        for x, y in zip(v, o._v):
            if x is None or y is None:
                out.append(None)
            else:
                out.append(list(x) + (list(y) if isinstance(y, list) else [y]))
        return _same(s, out)
    raise ComputeError("unsupported list function " + op)


# -- horizontal --


def _fold(e, f):
    ss = [_eval(x, f) for x in e._a[2]]
    if e._a[0] is None:
        # reduce: no cast
        if not ss:
            raise ComputeError("cannot return empty fold because the number of output rows is unknown")
        acc, ss = ss[0], ss[1:]
        dtype = e._a[3]
    else:
        acc = _eval(e._a[0], f)
        dtype = e._a[3] or (Int32 if acc._lit == "int" else acc.dtype)
    name = acc.name
    n = _max([_len(acc._v)] + [_len(s._v) for s in ss])
    acc = _broadcast(acc, n)
    for s in ss:
        acc = e._a[1](acc, _broadcast(s, n))
        if not isinstance(acc, Series):
            acc = _from_values(name, [acc] * n if not isinstance(acc, list) else acc)
    if dtype is not None and acc.dtype != dtype:
        acc = _cast(acc, dtype, False, False)
    return _new(name, acc._v, acc.dtype)


def _horizontal(e, f):
    kind = e._a[0]
    ss = [_eval(x, f) for x in e._a[1]]
    if not ss:
        raise ComputeError("cannot return empty fold because the number of output rows is unknown")
    n = _max(_len(s._v) for s in ss)
    ss = [_broadcast(s, n) for s in ss]
    name = ss[0].name
    if kind == "concat_str":
        sep, ign = e._a[2], e._a[3]
        out = []
        for i in range(n):
            parts = []
            null = False
            for s in ss:
                x = s._v[i]
                if x is None:
                    if not ign:
                        null = True
                    continue
                parts.append(_any_str(x, s.dtype, False) if s.dtype._kind != "str" else x)
            out.append(None if null else sep.join(parts))
        return _new(name, out, String)
    if kind == "coalesce":
        d = Null
        for s in ss:
            d = _supertype(d, s.dtype) if s._lit is None or d._kind == "null" else d
        ss = [_cast(s, d, False) for s in ss]
        out = []
        for i in range(n):
            r = None
            for s in ss:
                if s._v[i] is not None:
                    r = s._v[i]
                    break
            out.append(r)
        return _new(name, out, d)
    if kind == "concat_list":
        d = Null
        for s in ss:
            d = _supertype(d, s.dtype.inner if s.dtype._kind == "list" else s.dtype)
        out = []
        for i in range(n):
            row = []
            null = False
            for s in ss:
                x = s._v[i]
                if s.dtype._kind == "list":
                    if x is None:
                        null = True
                        break
                    row.extend(x)
                else:
                    row.append(x)
            out.append(None if null else row)
        return _new(name, out, _ListType(d))
    if kind in ("any", "all"):
        out = []
        for i in range(n):
            vals = [s._v[i] for s in ss]
            if kind == "any":
                out.append(True if True in vals else (None if None in vals else False))
            else:
                out.append(False if False in vals else (None if None in vals else True))
        return _new(name, out, Boolean)
    d = Null
    for s in ss:
        d = _supertype(d, s.dtype if s.dtype._kind != "bool" or kind not in ("sum", "mean") else (UInt32 if _len(ss) else s.dtype))
    if kind == "mean":
        d = Float64 if d != Float32 else Float32
    if kind == "sum" and d._kind == "bool":
        d = UInt32
    ign = e._a[2] if _len(e._a) > 2 else True
    out = []
    for i in range(n):
        vals = [s._v[i] for s in ss]
        nn = [x for x in vals if x is not None]
        if kind == "sum":
            if not ign and _len(nn) != _len(vals):
                out.append(None)
                continue
            if d._kind == "float":
                acc = 0.0
                for x in nn:
                    acc = acc + float(x)
                out.append(_f32(acc) if d._bits == 32 else acc)
            else:
                out.append(_wrap(_sum(int(x) for x in nn), d) if d.is_integer() else (_sum(nn) if nn else 0))
        elif kind == "mean":
            if (not ign and _len(nn) != _len(vals)) or not nn:
                out.append(None)
                continue
            acc = 0.0
            for x in nn:
                acc = acc + float(x)
            out.append(acc / _len(nn))
        elif kind == "max":
            out.append(_agg_max(nn, d) if nn else None)
        elif kind == "min":
            out.append(_agg_min(nn, d) if nn else None)
    if d.is_integer() or d._kind == "float":
        out = [None if x is None else (float(x) if d._kind == "float" else x) for x in out]
    return _new(name, out, d)


_FNS = {
    "neg": _fn_neg, "not": _fn_not, "abs": _fn_abs, "math": _fn_math, "log": _fn_log, "floor": _fn_floor, "ceil": _fn_ceil,
    "round": _fn_round, "round_sig": _fn_round_sig, "sign": _fn_sign, "clip": _fn_clip, "is_null": _fn_is_null,
    "is_not_null": _fn_is_not_null, "is_nan": _fn_is_nan, "is_not_nan": _fn_is_not_nan, "is_finite": _fn_is_finite,
    "is_infinite": _fn_is_infinite, "fill_null": _fn_fill_null, "fill_null_strategy": _fn_fill_null_strategy, "fill_nan": _fn_fill_nan,
    "is_in": _fn_is_in, "replace": _fn_replace, "map_elements": _fn_map_elements, "map_batches": _fn_map_batches, "shift": _fn_shift,
    "diff": _fn_diff, "pct_change": _fn_pct_change, "cum": _fn_cum, "rank": _fn_rank, "sort": _fn_sort, "arg_sort": _fn_arg_sort,
    "reverse": _fn_reverse, "unique": _fn_unique, "unique_counts": _fn_unique_counts, "arg_unique": _fn_arg_unique,
    "is_duplicated": _fn_is_duplicated, "is_unique": _fn_is_unique, "is_first_distinct": _fn_is_first_distinct,
    "is_last_distinct": _fn_is_last_distinct, "drop_nulls": _fn_drop_nulls, "drop_nans": _fn_drop_nans, "head": _fn_head,
    "tail": _fn_tail, "slice": _fn_slice, "gather": _fn_gather, "get": _fn_get, "gather_every": _fn_gather_every, "top_k": _fn_top_k,
    "mode": _fn_mode, "explode": _fn_explode, "interpolate": _fn_interpolate, "rolling": _fn_rolling, "repeat_by": _fn_repeat_by,
    "sum": _fn_sum, "mean": _fn_mean, "median": _fn_median, "quantile": _fn_quantile, "min": _fn_min, "max": _fn_max,
    "nan_min": _fn_nan_min, "nan_max": _fn_nan_max, "std": _fn_std, "var": _fn_var, "product": _fn_product, "count": _fn_count,
    "len": _fn_len, "null_count": _fn_null_count, "n_unique": _fn_n_unique, "first": _fn_first, "last": _fn_last,
    "arg_min": _fn_arg_min, "arg_max": _fn_arg_max, "any": _fn_any, "all": _fn_all, "implode": _fn_implode,
    "value_counts": _fn_value_counts, "entropy": _fn_entropy, "skew": _fn_skew, "kurtosis": _fn_kurtosis, "str": _fn_str,
    "list": _fn_list,
}


# ---------- DataFrame ----------


def _check_unique(names):
    seen = set()
    for n in names:
        if n in seen:
            raise DuplicateError("projections contained duplicate output name '" + n + "'. It's possible that multiple expressions are returning the same default column name. If this is the case, try renaming the columns with `.alias(\"new_name\")` to avoid duplicate column names.")
        seen.add(n)


def _schema_items(schema):
    # [(name, dtype or None)]
    if schema is None:
        return None
    if isinstance(schema, Schema):
        return list(schema.items())
    if isinstance(schema, dict):
        return [(k, None if v is None else _dt(v)) for k, v in schema.items()]
    out = []
    for it in schema:
        if isinstance(it, tuple):
            out.append((it[0], None if it[1] is None else _dt(it[1])))
        else:
            out.append((it, None))
    return out


class DataFrame:
    def __init__(self, data=None, schema=None, schema_overrides=None, strict=True, orient=None, infer_schema_length=100, nan_to_null=False):
        if data is _RAW:
            return
        cols = _frame_cols(data, _schema_items(schema), orient, strict)
        if schema_overrides:
            ov = {k: _dt(v) for k, v in schema_overrides.items()}
            cols = [(_cast(c, ov[c.name], strict) if c.name in ov else c) for c in cols]
        if nan_to_null:
            cols = [(_new(c.name, [None if (isinstance(v, float) and v != v) else v for v in c._v], c.dtype) if c.dtype._kind == "float" else c) for c in cols]
        _check_frame(cols)
        self._cols = cols

    @staticmethod
    def _of(cols):
        df = DataFrame(_RAW)
        df._cols = cols
        return df

    # -- shape and schema --
    @property
    def height(self):
        return _len(self._cols[0]._v) if self._cols else 0

    @property
    def width(self):
        return _len(self._cols)

    @property
    def shape(self):
        return (self.height, self.width)

    def __len__(self):
        return self.height

    @property
    def columns(self):
        return [c.name for c in self._cols]

    @columns.setter
    def columns(self, names):
        if _len(names) != _len(self._cols):
            raise ShapeError("dimensions of columns arg (" + str(_len(names)) + ") must match df dimensions (" + str(_len(self._cols)) + ")")
        _check_unique(names)
        self._cols = [_new(n, c._v, c.dtype) for n, c in zip(names, self._cols)]

    @property
    def dtypes(self):
        return [c.dtype for c in self._cols]

    @property
    def schema(self):
        return Schema([(c.name, c.dtype) for c in self._cols])

    def collect_schema(self):
        return self.schema

    def is_empty(self):
        return self.height == 0

    def estimated_size(self, unit="b"):
        raise NotImplementedError("brevi: DataFrame.estimated_size is not in Brevi's polars")

    def __repr__(self):
        return _frame_str(self)

    def __str__(self):
        return _frame_str(self)

    def __bool__(self):
        raise TypeError("the truth value of a DataFrame is ambiguous\n\nHint: to check if a DataFrame contains any values, use `is_empty()`.")

    def __iter__(self):
        return iter([_new(c.name, c._v, c.dtype) for c in self._cols])

    def __contains__(self, key):
        return key in self.columns

    def __eq__(self, other):
        return self._cmp(other, "==")

    def __ne__(self, other):
        return self._cmp(other, "!=")

    def _cmp(self, other, op):
        if isinstance(other, DataFrame):
            if other.columns != self.columns:
                raise ValueError("DataFrame columns do not match")
            return DataFrame._of([_new(a.name, _compare(op, a, b), Boolean) for a, b in zip(self._cols, other._cols)])
        return DataFrame._of([_new(a.name, _compare(op, a, _broadcast(_lit_series(other), _len(a._v))), Boolean) for a in self._cols])

    def __hash__(self):
        return id(self)

    def _frame(self):
        return _Frame.of(self._cols)

    # -- selection --
    def __getitem__(self, key):
        if isinstance(key, tuple) and _len(key) == 2:
            rows, cols = key
            df = self[cols] if not isinstance(cols, (int, str)) else None
            if isinstance(cols, str):
                s = self.get_column(cols)
                return s[rows]
            if isinstance(cols, int):
                s = self.to_series(cols)
                return s[rows]
            return df[rows]
        if isinstance(key, str):
            return self.get_column(key)
        if isinstance(key, bool):
            raise TypeError("cannot select columns using a bool")
        if isinstance(key, int):
            n = self.height
            i = key + n if key < 0 else key
            if i < 0 or i >= n:
                raise IndexError("index " + str(key) + " is out of bounds for DataFrame of height " + str(n))
            return DataFrame._of([_new(c.name, [c._v[i]], c.dtype) for c in self._cols])
        if isinstance(key, slice):
            if isinstance(key.start, str) or isinstance(key.stop, str):
                names = self.columns
                a = 0 if key.start is None else names.index(key.start)
                b = _len(names) if key.stop is None else names.index(key.stop) + 1
                return DataFrame._of([self._cols[i] for i in range(a, b)])
            return DataFrame._of([_new(c.name, c._v[key], c.dtype) for c in self._cols])
        if isinstance(key, Series):
            key = key._v if key.dtype._kind != "str" else key._v
        if isinstance(key, (list, tuple, range)):
            key = list(key)
            if not key:
                return DataFrame._of([])
            if isinstance(key[0], str):
                return DataFrame._of([self.get_column(k) for k in key])
            if key[0] is True or key[0] is False:
                return self.filter(Series("", key))
            if isinstance(key[0], int):
                n = self.height
                idx = []
                for i in key:
                    j = i + n if i < 0 else i
                    if j < 0 or j >= n:
                        raise OutOfBoundsError("gather indices are out of bounds")
                    idx.append(j)
                return DataFrame._of([_new(c.name, [c._v[i] for i in idx], c.dtype) for c in self._cols])
        if isinstance(key, Expr):
            raise TypeError("cannot use \"Expr\" for indexing a DataFrame; use `select` or `filter`")
        raise TypeError("cannot use " + repr(type(key).__name__) + " for indexing")

    def __setitem__(self, key, value):
        raise TypeError("DataFrame object does not support `Series` assignment by index\n\nUse `DataFrame.with_columns`.")

    def get_column(self, name, default=None):
        for c in self._cols:
            if c.name == name:
                return _new(c.name, c._v, c.dtype)
        if default is not None:
            return default
        raise ColumnNotFoundError(_col_missing(name, self.columns))

    def get_columns(self):
        return [_new(c.name, c._v, c.dtype) for c in self._cols]

    def get_column_index(self, name):
        names = self.columns
        if name not in names:
            raise ColumnNotFoundError(_col_missing(name, names))
        return names.index(name)

    def to_series(self, index=0):
        c = self._cols[index]
        return _new(c.name, c._v, c.dtype)

    def iter_columns(self):
        return iter(self.get_columns())

    def item(self, row=None, column=None):
        if row is None and column is None:
            if self.shape != (1, 1):
                raise ValueError("can only call `.item()` if the dataframe is of shape (1, 1), or if explicit row/col values are provided; frame has shape " + str(self.shape))
            return self._cols[0]._v[0]
        s = self.get_column(column) if isinstance(column, str) else self.to_series(column)
        return s[row]

    def select(self, *exprs, **named_exprs):
        f = self._frame()
        es = _exprs_for(f, exprs, named_exprs)
        return DataFrame._of(_project(f, es, self.height, True))

    def select_seq(self, *exprs, **named_exprs):
        return self.select(*exprs, **named_exprs)

    def with_columns(self, *exprs, **named_exprs):
        f = self._frame()
        es = _exprs_for(f, exprs, named_exprs)
        new = _project(f, es, self.height, False)
        cols = list(self._cols)
        names = [c.name for c in cols]
        for s in new:
            if s.name in names:
                cols[names.index(s.name)] = s
            else:
                cols.append(s)
                names.append(s.name)
        return DataFrame._of(cols)

    def with_columns_seq(self, *exprs, **named_exprs):
        return self.with_columns(*exprs, **named_exprs)

    def filter(self, *predicates, **constraints):
        p = _all_pred(predicates, constraints)
        f = self._frame()
        m = _eval(p, f)
        if m.dtype._kind not in ("bool", "null"):
            raise InvalidOperationError("filter predicate must be of type `Boolean`, got `" + repr(m.dtype) + "`")
        mv = m._v
        if _len(mv) == 1 and self.height != 1:
            mv = mv * self.height
        if _len(mv) != self.height:
            raise ShapeError("filter's length: " + str(_len(mv)) + " differs from that of the series: " + str(self.height))
        idx = [i for i, k in enumerate(mv) if k is True]
        return self._take(idx)

    def remove(self, *predicates, **constraints):
        p = _all_pred(predicates, constraints)
        m = _eval(p, self._frame())._v
        if _len(m) == 1:
            m = m * self.height
        return self._take([i for i, k in enumerate(m) if k is not True])

    def _take(self, idx):
        return DataFrame._of([_new(c.name, [c._v[i] for i in idx], c.dtype) for c in self._cols])

    def head(self, n=5):
        if n < 0:
            n = _max(0, self.height + n)
        return DataFrame._of([_new(c.name, c._v[:n], c.dtype) for c in self._cols])

    def tail(self, n=5):
        if n < 0:
            n = _max(0, self.height + n)
        h = self.height
        return DataFrame._of([_new(c.name, c._v[h - n:] if n else [], c.dtype) for c in self._cols])

    def limit(self, n=5):
        return self.head(n)

    def slice(self, offset, length=None):
        h = self.height
        o = offset + h if offset < 0 else offset
        o = _max(0, _min(o, h))
        e = h if length is None else _min(h, o + length)
        return DataFrame._of([_new(c.name, c._v[o:e], c.dtype) for c in self._cols])

    def gather_every(self, n, offset=0):
        return DataFrame._of([_new(c.name, c._v[offset::n], c.dtype) for c in self._cols])

    def reverse(self):
        return DataFrame._of([_new(c.name, list(reversed(c._v)), c.dtype) for c in self._cols])

    def sort(self, by, *more_by, descending=False, nulls_last=False, multithreaded=True, maintain_order=False):
        f = self._frame()
        keys = _exprs_for(f, [by] + list(more_by))
        ks = [_broadcast(_eval(k, f), self.height) for k in keys]
        if isinstance(descending, (list, tuple)) and _len(descending) != _len(ks):
            raise ValueError("the length of `descending` (" + str(_len(descending)) + ") does not match the length of `by` (" + str(_len(ks)) + ")")
        return self._take(_sort_idx(ks, descending, nulls_last))

    def top_k(self, k, by, reverse=False):
        bys = by if isinstance(by, (list, tuple)) else [by]
        return self.sort(bys, descending=not reverse, nulls_last=True).head(k)

    def bottom_k(self, k, by, reverse=False):
        bys = by if isinstance(by, (list, tuple)) else [by]
        return self.sort(bys, descending=reverse, nulls_last=True).head(k)

    def drop(self, *columns, strict=True):
        names = []
        for c in columns:
            if isinstance(c, (list, tuple)):
                names.extend(c)
            elif isinstance(c, Expr):
                f = self._frame()
                names.extend(_out_name(x) for x in _exprs_for(f, [c]))
            else:
                names.append(c)
        have = self.columns
        if strict:
            for n in names:
                if n not in have:
                    raise ColumnNotFoundError(_col_missing(n, have) if False else 'unable to find column "' + n + '"; valid columns: [' + ", ".join('"' + x + '"' for x in have) + "]")
        return DataFrame._of([c for c in self._cols if c.name not in names])

    def drop_in_place(self, name):
        s = self.get_column(name)
        self._cols = [c for c in self._cols if c.name != name]
        return s

    def rename(self, mapping, strict=True):
        if callable(mapping):
            return DataFrame._of([_new(mapping(c.name), c._v, c.dtype) for c in self._cols])
        have = self.columns
        if strict:
            for k in mapping:
                if k not in have:
                    raise ColumnNotFoundError(_col_missing(k, have))
        cols = [_new(mapping.get(c.name, c.name), c._v, c.dtype) for c in self._cols]
        _check_unique([c.name for c in cols])
        return DataFrame._of(cols)

    def insert_column(self, index, column):
        if index < 0:
            index += _len(self._cols) + 1
        if column.name in self.columns:
            raise DuplicateError("column with name '" + column.name + "' already exists")
        if self._cols and _len(column._v) != self.height:
            raise ShapeError("unable to add a column of length " + str(_len(column._v)) + " to a DataFrame of height " + str(self.height))
        self._cols.insert(index, _new(column.name, column._v, column.dtype))
        return self

    def replace_column(self, index, column):
        self._cols[index] = _new(column.name, column._v, column.dtype)
        return self

    def hstack(self, columns, in_place=False):
        cs = columns._cols if isinstance(columns, DataFrame) else list(columns)
        new = list(self._cols) + [_new(c.name, c._v, c.dtype) for c in cs]
        _check_unique([c.name for c in new])
        _check_frame(new)
        if in_place:
            self._cols = new
            return self
        return DataFrame._of(new)

    def vstack(self, other, in_place=False):
        r = _vconcat([self, other], False)
        if in_place:
            self._cols = r._cols
            return self
        return r

    def extend(self, other):
        self._cols = _vconcat([self, other], False)._cols
        return self

    def clone(self):
        return DataFrame._of([_new(c.name, list(c._v), c.dtype) for c in self._cols])

    def clear(self, n=0):
        return DataFrame._of([_new(c.name, [None] * n, c.dtype) for c in self._cols])

    def cast(self, dtypes, strict=True):
        f = self._frame()
        if isinstance(dtypes, (DataType, type)):
            d = _dt(dtypes)
            return DataFrame._of([_cast(c, d, strict) for c in self._cols])
        m = {}
        for k, v in dtypes.items():
            if isinstance(k, DataType):
                for c in self._cols:
                    if c.dtype == k:
                        m[c.name] = _dt(v)
            elif isinstance(k, Expr):
                for x in _exprs_for(f, [k]):
                    m[_out_name(x)] = _dt(v)
            else:
                m[k] = _dt(v)
        return DataFrame._of([(_cast(c, m[c.name], strict) if c.name in m else c) for c in self._cols])

    def drop_nulls(self, subset=None):
        names = self.columns if subset is None else ([subset] if isinstance(subset, str) else list(subset))
        cs = [self.get_column(n)._v for n in names]
        idx = [i for i in range(self.height) if _all(c[i] is not None for c in cs)]
        return self._take(idx)

    def drop_nans(self, subset=None):
        names = self.columns if subset is None else ([subset] if isinstance(subset, str) else list(subset))
        cs = [self.get_column(n)._v for n in names]
        idx = [i for i in range(self.height) if not _any(isinstance(c[i], float) and c[i] != c[i] for c in cs)]
        return self._take(idx)

    def fill_null(self, value=None, strategy=None, limit=None, matches_supertype=True):
        out = []
        for c in self._cols:
            if strategy is not None:
                out.append(_fn_fill_null_strategy(c, None, strategy, limit))
                continue
            if value is None:
                raise ValueError("must specify either a fill `value` or `strategy`")
            vs = _lit_series(value)
            if vs.dtype.is_numeric() and not c.dtype.is_numeric():
                out.append(c)
                continue
            if vs.dtype._kind == "str" and c.dtype._kind != "str" and c.dtype._kind != "null":
                out.append(c)
                continue
            if vs.dtype._kind == "bool" and c.dtype._kind not in ("bool", "null"):
                out.append(c)
                continue
            out.append(_fn_fill_null(c, None, vs))
        return DataFrame._of(out)

    def fill_nan(self, value):
        out = []
        for c in self._cols:
            if c.dtype._kind == "float":
                out.append(_fn_fill_nan(c, None, _lit_series(value)))
            else:
                out.append(c)
        return DataFrame._of(out)

    def with_row_index(self, name="index", offset=0):
        if name in self.columns:
            raise DuplicateError("column with name '" + name + "' already exists")
        return DataFrame._of([_new(name, list(range(offset, offset + self.height)), UInt32)] + list(self._cols))

    def with_row_count(self, name="row_nr", offset=0):
        return self.with_row_index(name, offset)

    def unique(self, subset=None, keep="any", maintain_order=False):
        names = self.columns if subset is None else ([subset] if isinstance(subset, str) else list(subset))
        f = self._frame()
        ks = [_eval(e, f) for e in _exprs_for(f, names)]
        groups = _group_rows(ks, self.height)
        if keep in ("first", "any"):
            idx = [g[0] for g in groups]
        elif keep == "last":
            idx = sorted(g[-1] for g in groups)
        elif keep == "none":
            idx = [g[0] for g in groups if _len(g) == 1]
        else:
            raise ValueError("`keep` must be one of {'first', 'last', 'any', 'none'}, got " + repr(keep))
        return self._take(idx)

    def n_unique(self, subset=None):
        names = self.columns if subset is None else ([subset] if isinstance(subset, str) else list(subset))
        f = self._frame()
        ks = [_eval(e, f) for e in _exprs_for(f, names)]
        return _len(_group_rows(ks, self.height))

    def is_duplicated(self):
        groups = _group_rows(self._cols, self.height)
        out = [False] * self.height
        for g in groups:
            if _len(g) > 1:
                for i in g:
                    out[i] = True
        return _new("", out, Boolean)

    def is_unique(self):
        d = self.is_duplicated()
        return _new("", [not x for x in d._v], Boolean)

    def null_count(self):
        return DataFrame._of([_new(c.name, [_sum(1 for v in c._v if v is None)], UInt32) for c in self._cols])

    def _colagg(self, name, *a):
        out = []
        for c in self._cols:
            if c.dtype._kind in ("str", "list") and name in ("sum", "mean", "median", "std", "var", "quantile", "product"):
                d = String if (c.dtype._kind == "str" and name not in ("sum", "product")) else c.dtype
                if name == "sum" and c.dtype._kind == "str":
                    out.append(_new(c.name, [None], String))
                    continue
                out.append(_new(c.name, [None], d))
                continue
            out.append(_FNS[name](c, None, *a))
        return DataFrame._of(out)

    def sum(self):
        return self._colagg("sum")

    def mean(self):
        return self._colagg("mean")

    def median(self):
        return self._colagg("median")

    def min(self):
        return self._colagg("min")

    def max(self):
        return self._colagg("max")

    def std(self, ddof=1):
        return self._colagg("std", ddof)

    def var(self, ddof=1):
        return self._colagg("var", ddof)

    def product(self):
        return self._colagg("product")

    def quantile(self, quantile, interpolation="nearest"):
        return self._colagg("quantile", quantile, interpolation)

    def count(self):
        return DataFrame._of([_new(c.name, [_sum(1 for v in c._v if v is not None)], UInt32) for c in self._cols])

    def sum_horizontal(self, ignore_nulls=True):
        return self.select(sum_horizontal(all(), ignore_nulls=ignore_nulls).alias("sum")).to_series()

    def mean_horizontal(self, ignore_nulls=True):
        return self.select(mean_horizontal(all(), ignore_nulls=ignore_nulls).alias("mean")).to_series()

    def max_horizontal(self):
        return self.select(max_horizontal(all()).alias("max")).to_series()

    def min_horizontal(self):
        return self.select(min_horizontal(all()).alias("min")).to_series()

    # -- rows and Python data --
    def rows(self, named=False):
        if named:
            names = self.columns
            return [dict(zip(names, r)) for r in self._rows()]
        return self._rows()

    def _rows(self):
        cs = [c.to_list() for c in self._cols]
        return [tuple(c[i] for c in cs) for i in range(self.height)]

    def row(self, index=None, by_predicate=None, named=False):
        if by_predicate is not None:
            r = self.filter(by_predicate)
            if r.height == 0:
                raise NoDataError("predicate <" + str(by_predicate) + "> returned no rows")
            if r.height > 1:
                raise TooManyRowsReturnedError("predicate <" + str(by_predicate) + "> returned " + str(r.height) + " rows")
            return r.row(0, named=named)
        n = self.height
        i = index + n if index < 0 else index
        if i < 0 or i >= n:
            raise IndexError("index " + str(index) + " is out of bounds")
        t = tuple((list(c._v[i]) if isinstance(c._v[i], list) else c._v[i]) for c in self._cols)
        if named:
            return dict(zip(self.columns, t))
        return t

    def iter_rows(self, named=False, buffer_size=512):
        return iter(self.rows(named=named))

    def iter_slices(self, n_rows=10000):
        return iter([self.slice(i, n_rows) for i in range(0, self.height, n_rows)])

    def to_dicts(self):
        return self.rows(named=True)

    def to_dict(self, as_series=True):
        if as_series:
            return {c.name: _new(c.name, c._v, c.dtype) for c in self._cols}
        return {c.name: c.to_list() for c in self._cols}

    def to_numpy(self, *a, **k):
        raise NotImplementedError("brevi: DataFrame.to_numpy() is not in Brevi's polars")

    def to_pandas(self, *a, **k):
        raise NotImplementedError("brevi: DataFrame.to_pandas() is not in Brevi's polars")

    def equals(self, other, null_equal=True):
        if self.columns != other.columns or self.dtypes != other.dtypes:
            return False
        return _all(a.equals(b, null_equal=null_equal) for a, b in zip(self._cols, other._cols))

    def frame_equal(self, other, null_equal=True):
        return self.equals(other, null_equal)

    def pipe(self, function, *args, **kwargs):
        return function(self, *args, **kwargs)

    def lazy(self):
        return LazyFrame(self)

    # -- grouping --
    def group_by(self, *by, maintain_order=False, **named_by):
        return GroupBy(self, list(by), named_by, maintain_order)

    def groupby(self, *by, maintain_order=False, **named_by):
        return self.group_by(*by, maintain_order=maintain_order, **named_by)

    def partition_by(self, by, *more_by, maintain_order=True, include_key=True, as_dict=False):
        names = ([by] if isinstance(by, str) else list(by)) + list(more_by)
        ks = [self.get_column(n) for n in names]
        groups = _group_rows(ks, self.height)
        parts = []
        keys = []
        for g in groups:
            p = self._take(g)
            if not include_key:
                p = p.drop(names)
            parts.append(p)
            keys.append(tuple(k._v[g[0]] for k in ks))
        if as_dict:
            return dict(zip(keys, parts))
        return parts

    # -- joins --
    def join(self, other, on=None, how="inner", left_on=None, right_on=None, suffix="_right", validate="m:m", nulls_equal=False, coalesce=None, maintain_order=None):
        return _join(self, other, on, how, left_on, right_on, suffix, nulls_equal, coalesce)

    # -- reshaping --
    def explode(self, columns, *more_columns, empty_as_null=False, keep_nulls=True):
        names = ([columns] if isinstance(columns, str) else list(columns)) + list(more_columns)
        lists = [self.get_column(n) for n in names]
        for l in lists:
            if l.dtype._kind != "list":
                raise InvalidOperationError("`explode` operation not supported for dtype `" + _short(l.dtype) + "`")
        rows = []
        for i in range(self.height):
            ln = None
            for l in lists:
                x = l._v[i]
                if x is None:
                    k = 1 if keep_nulls else 0
                elif _len(x) == 0:
                    k = 1 if empty_as_null else 0
                else:
                    k = _len(x)
                if ln is None:
                    ln = k
                elif ln != k:
                    raise ShapeError("exploded columns must have matching element counts")
            rows.append((i, ln))
        out = []
        for c in self._cols:
            if c.name in names:
                vals = []
                for i, ln in rows:
                    x = c._v[i]
                    if ln == 0:
                        continue
                    if x is None or _len(x) == 0:
                        vals.append(None)
                    else:
                        vals.extend(x)
                out.append(_new(c.name, vals, c.dtype.inner if c.dtype.inner is not None else Null))
            else:
                vals = []
                for i, ln in rows:
                    vals.extend([c._v[i]] * ln)
                out.append(_new(c.name, vals, c.dtype))
        return DataFrame._of(out)

    def unpivot(self, on=None, index=None, variable_name=None, value_name=None):
        idx = [] if index is None else ([index] if isinstance(index, str) else list(index))
        if on is None:
            on_cols = [n for n in self.columns if n not in idx]
        else:
            f = self._frame()
            on_cols = [_out_name(e) for e in _exprs_for(f, [on])]
        vn = variable_name or "variable"
        valn = value_name or "value"
        d = Null
        for n in on_cols:
            d = _supertype(d, self.get_column(n).dtype)
        out = [_new(n, [], self.get_column(n).dtype) for n in idx]
        var = []
        val = []
        for n in on_cols:
            c = _cast(self.get_column(n), d, False) if d._kind != "null" else self.get_column(n)
            for j, s in enumerate(idx):
                out[j]._v.extend(self.get_column(s)._v)
            var.extend([n] * self.height)
            val.extend(c._v)
        return DataFrame._of(out + [_new(vn, var, String), _new(valn, val, d)])

    def melt(self, id_vars=None, value_vars=None, variable_name=None, value_name=None):
        return self.unpivot(on=value_vars, index=id_vars, variable_name=variable_name, value_name=value_name)

    def pivot(self, on, index=None, values=None, aggregate_function=None, maintain_order=True, sort_columns=False, separator="_"):
        on_l = [on] if isinstance(on, str) else list(on)
        if aggregate_function == "count":
            raise ValueError("use of `aggregate_function='count'` should be replaced with `aggregate_function='len'`.")
        if index is None and values is None:
            raise ValueError("`pivot` needs either `index or `values` needs to be specified")
        idx = [] if index is None else ([index] if isinstance(index, str) else list(index))
        if values is None:
            vals = [n for n in self.columns if n not in idx and n not in on_l]
        else:
            vals = [values] if isinstance(values, str) else list(values)
        if index is None:
            idx = [n for n in self.columns if n not in vals and n not in on_l]
        ks = [self.get_column(n) for n in idx]
        groups = _group_rows(ks, self.height)
        onk = [self.get_column(n) for n in on_l]
        on_groups = _group_rows(onk, self.height)
        on_vals = []
        for g in on_groups:
            t = tuple(c._v[g[0]] for c in onk)
            on_vals.append(t)
        if sort_columns:
            on_vals = sorted(on_vals, key=lambda t: tuple(_sort_key_val(x) for x in t))
        row_of = {}
        for gi, g in enumerate(groups):
            for i in g:
                row_of[i] = gi
        out = [_new(n, [k._v[g[0]] for g in groups], k.dtype) for n, k in zip(idx, ks)]
        for vname in vals:
            vc = self.get_column(vname)
            cells = {}
            for i in range(self.height):
                t = tuple(_key_of(c._v[i]) for c in onk)
                cells.setdefault((row_of[i], t), []).append(i)
            for t in on_vals:
                kt = tuple(_key_of(x) for x in t)
                label = t[0] if _len(t) == 1 else "{" + ",".join(_any_str(x, onk[j].dtype, True) for j, x in enumerate(t)) + "}"
                label = _any_str(label, String if isinstance(label, str) else _py_dtype(label), False) if not isinstance(label, str) else label
                if _len(vals) > 1:
                    label = vname + separator + label
                col_vals = []
                dtype = vc.dtype
                for gi in range(_len(groups)):
                    rows = cells.get((gi, kt))
                    if rows is None and aggregate_function is None:
                        col_vals.append(None)
                        continue
                    # a missing cell is the aggregate of no values (0 for sum and len, null for the rest)
                    sub = _new(vname, [vc._v[i] for i in (rows or [])], vc.dtype)
                    if aggregate_function is None:
                        if _len(rows) > 1:
                            raise ComputeError("found multiple elements in the same group, please specify an aggregation function")
                        col_vals.append(sub._v[0])
                    else:
                        if isinstance(aggregate_function, Expr):
                            r = _eval(aggregate_function, _Frame.of([_new("", sub._v, sub.dtype)]))
                        else:
                            r = _FNS[{"len": "len", "count": "len"}.get(aggregate_function, aggregate_function)](sub, None, *(() if aggregate_function not in ("std", "var") else (1,)))
                        col_vals.append(r._v[0])
                        dtype = r.dtype
                if aggregate_function is None:
                    dtype = vc.dtype
                out.append(_new(label, col_vals, dtype))
        return DataFrame._of(out)

    def transpose(self, include_header=False, header_name="column", column_names=None):
        d = Null
        for c in self._cols:
            d = _supertype(d, c.dtype)
        rows = [(_cast(c, d, False) if d._kind != "null" else c)._v for c in self._cols]
        names = list(column_names) if column_names is not None else ["column_" + str(i) for i in range(self.height)]
        out = []
        if include_header:
            out.append(_new(header_name, self.columns, String))
        for i in range(self.height):
            out.append(_new(names[i], [r[i] for r in rows], d))
        return DataFrame._of(out)

    def unnest(self, columns, *more_columns, separator=None):
        names = ([columns] if isinstance(columns, str) else list(columns)) + list(more_columns)
        out = []
        for c in self._cols:
            if c.name in names and isinstance(c.dtype, _StructType):
                for j, (fname, fd) in enumerate(c.dtype.fields):
                    out.append(_new(fname, [None if r is None else r[j] for r in c._v], fd))
            else:
                out.append(c)
        return DataFrame._of(out)

    def shift(self, n=1, fill_value=None):
        return DataFrame._of([_fn_shift(c, None, n, fill_value) for c in self._cols])

    def to_dummies(self, columns=None, separator="_", drop_first=False, drop_nulls=False):
        names = self.columns if columns is None else ([columns] if isinstance(columns, str) else list(columns))
        out = []
        for c in self._cols:
            if c.name not in names:
                out.append(c)
                continue
            vals = [c._v[i] for i in _uniq_idx(c._v)]
            vals = sorted(vals, key=lambda v: (v is None, _sort_key_val(v) if v is not None else (0, 0)))
            if drop_first:
                vals = vals[1:]
            for v in vals:
                if v is None and drop_nulls:
                    continue
                label = c.name + separator + ("null" if v is None else _any_str(v, c.dtype, False))
                out.append(_new(label, [1 if _key_of(x) == _key_of(v) else 0 for x in c._v], UInt8))
        return DataFrame._of(out)

    def describe(self, percentiles=(0.25, 0.5, 0.75), interpolation="nearest"):
        return _describe(self, percentiles, interpolation)

    def glimpse(self, max_items_per_column=10, max_colname_length=50, return_type=None):
        return _glimpse(self, max_items_per_column, max_colname_length, return_type)

    def map_rows(self, function, return_dtype=None, inference_size=256):
        res = [function(r) for r in self._rows()]
        if res and isinstance(res[0], tuple):
            cols = list(zip(*res))
            return DataFrame._of([Series("column_" + str(i), list(c)) for i, c in enumerate(cols)])
        return DataFrame._of([Series("map", res, dtype=return_dtype)])

    def sample(self, *a, **k):
        raise NotImplementedError("brevi: DataFrame.sample is not in Brevi's polars")

    # -- writing --
    def write_csv(self, file=None, include_bom=False, include_header=True, separator=",", line_terminator="\n", quote_char='"', batch_size=1024, datetime_format=None, date_format=None, time_format=None, float_scientific=None, float_precision=None, decimal_comma=False, null_value=None, quote_style=None):
        text = _write_csv(self, include_header, separator, line_terminator, quote_char, null_value or "", float_precision, quote_style)
        if include_bom:
            text = "﻿" + text
        return _write_out(file, text)

    def write_json(self, file=None):
        rows = []
        for r in self.rows(named=True):
            rows.append("{" + ",".join(_json.dumps(k) + ":" + _json_val(v) for k, v in r.items()) + "}")
        return _write_out(file, "[" + ",".join(rows) + "]")

    def write_ndjson(self, file=None):
        out = []
        for r in self.rows(named=True):
            out.append("{" + ",".join(_json.dumps(k) + ":" + _json_val(v) for k, v in r.items()) + "}\n")
        return _write_out(file, "".join(out))

    def write_parquet(self, *a, **k):
        raise NotImplementedError("brevi: parquet is not in Brevi's polars")


class TooManyRowsReturnedError(PolarsError):
    pass


def _check_frame(cols):
    if not cols:
        return
    n = _len(cols[0]._v)
    for c in cols:
        if _len(c._v) != n:
            raise ShapeError("could not create a new DataFrame: height of column '" + c.name + "' (" + str(_len(c._v)) + ") does not match height of column '" + cols[0].name + "' (" + str(n) + ")")
    _check_unique_frame([c.name for c in cols])


def _check_unique_frame(names):
    seen = set()
    for n in names:
        if n in seen:
            raise DuplicateError("column with name '" + n + "' has more than one occurrence")
        seen.add(n)


def _project(f, es, height, select):
    out = [_eval(e, f) for e in es]
    names = [s.name for s in out]
    _check_unique(names)
    if not out:
        return []
    n = height if not select else _max(_len(s._v) for s in out)
    if select:
        lens = set(_len(s._v) for s in out)
        if _len(lens) > 1:
            n = _max(lens)
            for s, e in zip(out, es):
                if _len(s._v) not in (1, n):
                    raise ShapeError("Series " + s.name + ", length " + str(_len(s._v)) + " doesn't match the DataFrame height of " + str(n))
        if _all(_len(s._v) == 1 for s in out):
            n = 1
    res = []
    for s, e in zip(out, es):
        if _len(s._v) != n:
            if _len(s._v) == 1:
                s = _new(s.name, s._v * n, s.dtype, s._lit)
            else:
                raise ShapeError("unable to add a column of length " + str(_len(s._v)) + " to a DataFrame of height " + str(n))
        res.append(_materialize(s))
    return res


def _frame_cols(data, schema, orient, strict):
    if data is None:
        data = {} if schema is None else {n: [] for n, _ in schema}
    if isinstance(data, DataFrame):
        cols = [_new(c.name, c._v, c.dtype) for c in data._cols]
    elif isinstance(data, Series):
        cols = [_new(data.name, data._v, data.dtype)]
    elif isinstance(data, dict):
        cols = []
        n = None
        for k, v in data.items():
            if isinstance(v, Series):
                cols.append(_new(k, list(v._v), v.dtype))
                n = _len(v._v)
            elif isinstance(v, (list, tuple, range)):
                cols.append(_from_values(k, list(v), None))
                n = _len(v)
            else:
                cols.append(None)
        if n is None:
            n = 1
        for i, (k, v) in enumerate(data.items()):
            if cols[i] is None:
                cols[i] = _broadcast(_lit_series(v, k), n) if True else None
                cols[i] = _new(k, cols[i]._v, cols[i].dtype if cols[i]._lit != "int" else Int64)
    elif isinstance(data, (list, tuple)):
        rows = list(data)
        if not rows:
            cols = []
        elif isinstance(rows[0], dict):
            return _from_dicts(rows, schema, strict)
        elif isinstance(rows[0], Series):
            cols = [_new(s.name if s.name else "column_" + str(i), s._v, s.dtype) for i, s in enumerate(rows)]
        elif isinstance(rows[0], (list, tuple)):
            if orient is None:
                if isinstance(rows[0], tuple) or (schema is not None and _len(schema) == _len(rows[0]) and _len(schema) != _len(rows)):
                    orient = "row"
                else:
                    orient = "col"
            if orient == "row":
                w = _len(rows[0])
                cols = [_from_values("column_" + str(j), [r[j] for r in rows], None) for j in range(w)]
            else:
                cols = [_from_values("column_" + str(j), list(r), None) for j, r in enumerate(rows)]
        else:
            cols = [_from_values("column_0", rows, None)]
    else:
        raise TypeError("DataFrame constructor called with unsupported type " + repr(type(data).__name__) + " for the `data` parameter")
    if schema is not None:
        if _len(schema) != _len(cols) and cols:
            raise ShapeError("the given column-schema names do not match the data dimensions")
        out = []
        for i, (n, d) in enumerate(schema):
            c = cols[i] if i < _len(cols) else _new(n, [], d or Null)
            if d is not None and c.dtype != d:
                c = _cast(c, d, strict)
            out.append(_new(n, c._v, c.dtype))
        cols = out
    return cols


def _from_dicts(rows, schema, strict=True):
    names = []
    seen = set()
    for r in rows:
        for k in r:
            if k not in seen:
                seen.add(k)
                names.append(k)
    if schema is not None:
        sch = schema
        cols = []
        for n, d in sch:
            vals = [r.get(n) for r in rows]
            cols.append(_from_values(n, vals, d, strict))
        return cols
    return [_from_values(n, [r.get(n) for r in rows], None, strict) for n in names]


def from_dicts(data, schema=None, schema_overrides=None, strict=True, infer_schema_length=100):
    return DataFrame(list(data), schema=schema, schema_overrides=schema_overrides, strict=strict)


def from_dict(data, schema=None, schema_overrides=None, strict=True):
    return DataFrame(data, schema=schema, schema_overrides=schema_overrides, strict=strict)


def from_records(data, schema=None, schema_overrides=None, strict=True, orient=None, infer_schema_length=100):
    return DataFrame(data, schema=schema, schema_overrides=schema_overrides, strict=strict, orient=orient or "row")


class Schema:
    def __init__(self, schema=None):
        self._items = []
        if schema is None:
            return
        if isinstance(schema, dict):
            items = list(schema.items())
        elif isinstance(schema, Schema):
            items = schema.items()
        else:
            items = list(schema)
        for k, v in items:
            self._items.append((k, _dt(v)))

    def items(self):
        return list(self._items)

    def keys(self):
        return [k for k, _ in self._items]

    def values(self):
        return [v for _, v in self._items]

    def names(self):
        return self.keys()

    def dtypes(self):
        return self.values()

    def len(self):
        return _len(self._items)

    def __len__(self):
        return _len(self._items)

    def __iter__(self):
        return iter(self.keys())

    def __contains__(self, k):
        return k in self.keys()

    def __getitem__(self, k):
        for n, d in self._items:
            if n == k:
                return d
        raise KeyError(k)

    def get(self, k, default=None):
        for n, d in self._items:
            if n == k:
                return d
        return default

    def __eq__(self, other):
        if isinstance(other, Schema):
            return self._items == other._items
        if isinstance(other, dict):
            return self._items == [(k, _dt(v)) for k, v in other.items()]
        return False

    def __ne__(self, other):
        return not self.__eq__(other)

    def to_python(self):
        return {k: v.to_python() for k, v in self._items}

    def __repr__(self):
        return "Schema({" + ", ".join(repr(k) + ": " + repr(v) for k, v in self._items) + "})"


# -- group_by --


class GroupBy:
    def __init__(self, df, by, named_by, maintain_order):
        self._df = df
        self._by = by
        self._named = named_by
        self._mo = maintain_order

    def _keys(self):
        f = self._df._frame()
        es = _exprs_for(f, self._by, self._named)
        ks = [_broadcast(_materialize(_eval(e, f)), self._df.height) for e in es]
        _check_unique([k.name for k in ks])
        return f, ks

    def agg(self, *aggs, **named_aggs):
        f, ks = self._keys()
        groups = _group_rows(ks, self._df.height)
        es = _exprs_for(f, aggs, named_aggs)
        keycols = [_new(k.name, [k._v[g[0]] for g in groups], k.dtype) for k in ks]
        out = list(keycols)
        for e in es:
            out.append(_agg_expr(e, f, groups))
        _check_unique([c.name for c in out])
        return DataFrame._of(out)

    def _all(self, name, *a):
        f, ks = self._keys()
        names = [k.name for k in ks]
        rest = [col(n) for n in self._df.columns if n not in names]
        return self.agg([getattr(e, name)(*a) for e in rest])

    def sum(self):
        return self._all("sum")

    def mean(self):
        return self._all("mean")

    def median(self):
        return self._all("median")

    def min(self):
        return self._all("min")

    def max(self):
        return self._all("max")

    def first(self):
        return self._all("first")

    def last(self):
        return self._all("last")

    def n_unique(self):
        return self._all("n_unique")

    def std(self, ddof=1):
        return self._all("std", ddof)

    def var(self, ddof=1):
        return self._all("var", ddof)

    def quantile(self, quantile, interpolation="nearest"):
        return self._all("quantile", quantile, interpolation)

    def count(self):
        return self.agg(Expr("len").alias("count"))

    def len(self, name=None):
        return self.agg(Expr("len").alias(name or "len"))

    def all(self):
        return self._all("implode")

    def head(self, n=5):
        f, ks = self._keys()
        groups = _group_rows(ks, self._df.height)
        idx = []
        for g in groups:
            idx.extend(g[:n])
        return self._reorder(idx, ks)

    def tail(self, n=5):
        f, ks = self._keys()
        groups = _group_rows(ks, self._df.height)
        idx = []
        for g in groups:
            idx.extend(g[_len(g) - n:] if n else [])
        return self._reorder(idx, ks)

    def _reorder(self, idx, ks):
        names = [k.name for k in ks]
        keyc = [_new(k.name, [k._v[i] for i in idx], k.dtype) for k in ks]
        rest = [_new(c.name, [c._v[i] for i in idx], c.dtype) for c in self._df._cols if c.name not in names]
        return DataFrame._of(keyc + rest)

    def map_groups(self, function):
        f, ks = self._keys()
        groups = _group_rows(ks, self._df.height)
        parts = [function(self._df._take(g)) for g in groups]
        return concat(parts)

    def __iter__(self):
        f, ks = self._keys()
        groups = _group_rows(ks, self._df.height)
        out = []
        for g in groups:
            out.append((tuple(k._v[g[0]] for k in ks), self._df._take(g)))
        return iter(out)


def _agg_expr(e, f, groups):
    scalar = _is_scalar(e)
    fast = _fast_agg(e, f, groups)
    if fast is not None:
        return fast
    vals = []
    dtype = None
    name = None
    for g in groups:
        sub = _Frame.of([f.cols[n] for n in f.names], g)
        r = _eval(e, sub)
        name = r.name
        if dtype is None or dtype._kind == "null":
            dtype = r.dtype
        if scalar:
            if _len(r._v) != 1:
                raise ComputeError("expected a single value per group")
            vals.append(r._v[0])
        else:
            vals.append(list(r._v))
    if dtype is None:
        r = _eval(e, _Frame.of([_new(n, [], f.dtype(n)) for n in f.names]))
        dtype = r.dtype
        name = r.name
    if scalar:
        if dtype._kind == "null" and _any(v is not None for v in vals):
            dtype = _infer(vals)
        return _materialize(_new(name, vals, dtype))
    return _new(name, vals, _ListType(dtype))


def _fast_agg(e, f, groups):
    # pl.col(c).<agg>() and pl.len() without building a frame per group
    if e._op == "len":
        return _new("len", [_len(g) for g in groups], UInt32)
    alias = None
    x = e
    if x._op == "alias":
        alias = x._a[1]
        x = x._a[0]
    if x._op == "len":
        return _new(alias or "len", [_len(g) for g in groups], UInt32)
    if x._op != "fn" or x._a[1]._op != "col" or x._a[0] not in ("sum", "mean", "min", "max", "count", "len", "first", "last", "n_unique", "std", "var", "median", "null_count"):
        return None
    name = x._a[0]
    s = f.get(x._a[1]._a[0])
    v = s._v
    fn = _FNS[name]
    out = []
    dtype = None
    for g in groups:
        r = fn(_new(s.name, [v[i] for i in g], s.dtype), None, *x._a[2])
        out.append(r._v[0])
        dtype = r.dtype
    if dtype is None:
        dtype = fn(_new(s.name, [], s.dtype), None, *x._a[2]).dtype
    return _new(alias or s.name, out, dtype)


# -- joins --


def _join(left, right, on, how, left_on, right_on, suffix, nulls_equal, coalesce):
    if how == "cross":
        lr, rr = left.height, right.height
        out = [_new(c.name, [x for x in c._v for _ in range(rr)], c.dtype) for c in left._cols]
        names = set(left.columns)
        for c in right._cols:
            nm = c.name + suffix if c.name in names else c.name
            out.append(_new(nm, c._v * lr, c.dtype))
        return DataFrame._of(out)
    if how == "outer":
        how = "full"
    if how not in ("inner", "left", "right", "full", "semi", "anti"):
        raise ValueError("`how` must be one of {'inner', 'left', 'full', 'semi', 'anti', 'cross'}, got " + repr(how))
    if on is not None:
        lk = [on] if isinstance(on, (str, Expr)) else list(on)
        rk = lk
    else:
        if left_on is None or right_on is None:
            raise ValueError("must specify `on` OR `left_on` and `right_on`")
        lk = [left_on] if isinstance(left_on, (str, Expr)) else list(left_on)
        rk = [right_on] if isinstance(right_on, (str, Expr)) else list(right_on)
    if _len(lk) != _len(rk):
        raise ValueError("the number of columns passed to `left_on` and `right_on` must be the same")
    lf, rf = left._frame(), right._frame()
    lks = [_eval(_x(k), lf) for k in lk]
    rks = [_eval(_x(k), rf) for k in rk]
    for a, b in zip(lks, rks):
        if a.dtype != b.dtype and not (a.dtype.is_numeric() and b.dtype.is_numeric() and a.dtype._kind == b.dtype._kind):
            if a.dtype.is_integer() and b.dtype.is_integer():
                continue
            raise SchemaError("datatypes of join keys don't match - `" + a.name + "`: " + _short(a.dtype) + " on left does not match `" + b.name + "`: " + _short(b.dtype) + " on right")
    index = {}
    for j in range(right.height):
        key = tuple(_key_of(k._v[j]) for k in rks)
        if not nulls_equal and _any(k._v[j] is None for k in rks):
            continue
        index.setdefault(key, []).append(j)

    def lkey(i):
        if not nulls_equal and _any(k._v[i] is None for k in lks):
            return None
        return tuple(_key_of(k._v[i]) for k in lks)

    if how in ("semi", "anti"):
        keep = []
        for i in range(left.height):
            k = lkey(i)
            hit = k is not None and k in index
            if hit == (how == "semi"):
                keep.append(i)
        return left._take(keep)
    pairs = []
    if how == "right":
        # right's rows in order, each with its matches in left's order; left's columns (but the keys) first
        lindex = {}
        for i in range(left.height):
            k = lkey(i)
            if k is not None:
                lindex.setdefault(k, []).append(i)
        for j in range(right.height):
            key = tuple(_key_of(k._v[j]) for k in rks)
            ls = lindex.get(key) if (nulls_equal or not _any(k._v[j] is None for k in rks)) else None
            if ls:
                for i in ls:
                    pairs.append((i, j))
            else:
                pairs.append((None, j))
        co = coalesce if coalesce is not None else True
        lkn = [k.name for k in lks]
        key_by_name = isinstance(lk[0], str) and _all(isinstance(k, str) for k in lk + rk)
        out = []
        for c in left._cols:
            if co and key_by_name and c.name in lkn:
                continue
            out.append(_new(c.name, [c._v[i] if i is not None else None for i, _ in pairs], c.dtype))
        lnames = set(o.name for o in out)
        for c in right._cols:
            nm = c.name + suffix if c.name in lnames else c.name
            if nm in [o.name for o in out]:
                raise DuplicateError("column with name '" + nm + "' already exists\n\nYou might want to try:\n- renaming the column '" + nm + "' before joining\n- setting the suffix")
            out.append(_new(nm, [c._v[j] for _, j in pairs], c.dtype))
        return DataFrame._of(out)
    if how == "full":
        matched_left = set()
        lindex = {}
        for i in range(left.height):
            k = lkey(i)
            if k is not None:
                lindex.setdefault(k, []).append(i)
        for j in range(right.height):
            key = tuple(_key_of(k._v[j]) for k in rks)
            ls = lindex.get(key) if (nulls_equal or not _any(k._v[j] is None for k in rks)) else None
            if ls:
                for i in ls:
                    pairs.append((i, j))
                    matched_left.add(i)
            else:
                pairs.append((None, j))
        for i in range(left.height):
            if i not in matched_left:
                pairs.append((i, None))
    else:
        for i in range(left.height):
            k = lkey(i)
            rs = index.get(k) if k is not None else None
            if rs:
                for j in rs:
                    pairs.append((i, j))
            elif how == "left":
                pairs.append((i, None))
    lkn = [k.name for k in lks]
    rkn = [k.name for k in rks]
    key_by_name = isinstance(lk[0], str) and _all(isinstance(k, str) for k in lk + rk)
    co = coalesce if coalesce is not None else (how != "full")
    out = []
    for c in left._cols:
        if co and c.name in lkn and how == "full":
            ki = lkn.index(c.name)
            rc = rks[ki]
            vals = []
            for i, j in pairs:
                vals.append(c._v[i] if i is not None else rc._v[j])
            out.append(_new(c.name, vals, _supertype(c.dtype, rc.dtype)))
            continue
        out.append(_new(c.name, [c._v[i] if i is not None else None for i, _ in pairs], c.dtype))
    lnames = set(left.columns)
    for c in right._cols:
        if c.name in rkn and co and key_by_name:
            continue
        nm = c.name + suffix if c.name in lnames else c.name
        if nm in [o.name for o in out]:
            raise DuplicateError("column with name '" + nm + "' already exists\n\nYou might want to try:\n- renaming the column '" + nm + "' before joining\n- setting the suffix")
        out.append(_new(nm, [c._v[j] if j is not None else None for _, j in pairs], c.dtype))
    return DataFrame._of(out)


# -- concat --


def _vconcat(frames, relaxed):
    frames = [f for f in frames]
    if not frames:
        raise ValueError("cannot concat empty list")
    first = frames[0]
    names = first.columns
    out = []
    for i, n in enumerate(names):
        d = first._cols[i].dtype
        vals = list(first._cols[i]._v)
        for f in frames[1:]:
            if f.width != first.width:
                raise ShapeError("unable to append to a DataFrame of width " + str(first.width) + " with a DataFrame of width " + str(f.width))
            c = f._cols[i]
            if c.name != n:
                raise ShapeError("unable to vstack, column names don't match: " + repr(n) + " and " + repr(c.name))
            if c.dtype != d:
                if relaxed or d._kind == "null" or c.dtype._kind == "null":
                    d = _supertype(d, c.dtype)
                else:
                    raise SchemaError("type " + repr(c.dtype) + " is incompatible with expected type " + repr(d))
            vals.extend(c._v)
        s = _new(n, vals, d)
        if relaxed:
            s = _cast(_new(n, vals, _infer(vals) if d._kind != "str" else d), d, False) if False else _from_values(n, vals, d, False)
        out.append(s)
    return DataFrame._of(out)


def concat(items, how="vertical", rechunk=False, parallel=True):
    items = list(items)
    if not items:
        raise ValueError("cannot concat empty list")
    first = items[0]
    if isinstance(first, Series):
        d = first.dtype
        vals = []
        for s in items:
            if s.dtype != d and d._kind != "null" and s.dtype._kind != "null":
                raise SchemaError("type " + repr(s.dtype) + " is incompatible with expected type " + repr(d))
            if d._kind == "null":
                d = s.dtype
            vals.extend(s._v)
        return _new(first.name, vals, d)
    if isinstance(first, LazyFrame):
        return LazyFrame(concat([i.collect() for i in items], how=how))
    if how in ("vertical", "vertical_relaxed"):
        return _vconcat(items, how == "vertical_relaxed")
    if how == "horizontal":
        h = _max(f.height for f in items)
        cols = []
        for f in items:
            for c in f._cols:
                cols.append(_new(c.name, c._v + [None] * (h - _len(c._v)), c.dtype))
        _check_unique_frame([c.name for c in cols])
        return DataFrame._of(cols)
    if how in ("diagonal", "diagonal_relaxed"):
        names = []
        dts = {}
        for f in items:
            for c in f._cols:
                if c.name not in dts:
                    names.append(c.name)
                    dts[c.name] = c.dtype
                elif dts[c.name] != c.dtype:
                    if how == "diagonal_relaxed" or dts[c.name]._kind == "null" or c.dtype._kind == "null":
                        dts[c.name] = _supertype(dts[c.name], c.dtype)
                    else:
                        raise SchemaError("type " + repr(c.dtype) + " is incompatible with expected type " + repr(dts[c.name]))
        out = []
        for n in names:
            vals = []
            for f in items:
                if n in f.columns:
                    vals.extend(_cast(f.get_column(n), dts[n], False)._v)
                else:
                    vals.extend([None] * f.height)
            out.append(_new(n, vals, dts[n]))
        return DataFrame._of(out)
    if how == "align":
        raise NotImplementedError("brevi: concat(how='align') is not in Brevi's polars")
    raise ValueError("DataFrame `how` must be one of {'vertical', 'vertical_relaxed', 'diagonal', 'diagonal_relaxed', 'horizontal', 'align'}, got " + repr(how))


def select(*exprs, **named_exprs):
    return DataFrame._of([]).select(*exprs, **named_exprs) if True else None


# -- describe / glimpse --


def _describe(df, percentiles, interpolation):
    if df.width == 0:
        raise TypeError("cannot describe a DataFrame that has no columns")
    ps = [] if percentiles is None else ([percentiles] if isinstance(percentiles, float) else list(percentiles))
    if 0.5 not in ps and percentiles is not None and False:
        ps.append(0.5)
    stats = ["count", "null_count", "mean", "std", "min"] + [_pct_label(p) for p in ps] + ["max"]
    out = [_new("statistic", stats, String)]
    numeric = [c for c in df._cols if c.dtype.is_numeric()]
    any_num = _any(c.dtype.is_numeric() or c.dtype._kind == "bool" for c in df._cols)
    for c in df._cols:
        k = c.dtype._kind
        nn = _nonnull(c._v)
        cnt = float(_len(nn))
        nulls = float(_len(c._v) - _len(nn))
        if c.dtype.is_numeric():
            mean = _agg_mean(c)
            std = _fn_std(c, None, 1)._v[0]
            mn = _agg_min(nn, c.dtype)
            mx = _agg_max(nn, c.dtype)
            qs = [_quantile(c._v, p, interpolation) for p in ps]
            vals = [cnt, nulls, mean, std, None if mn is None else float(mn)] + qs + [None if mx is None else float(mx)]
            out.append(_new(c.name, vals, Float64))
        elif k == "bool":
            mean = _agg_mean(c)
            vals = [cnt, nulls, mean, None, None if not nn else float(_min(nn))] + [None] * _len(ps) + [None if not nn else float(_max(nn))]
            out.append(_new(c.name, vals, Float64))
        else:
            mn = _agg_min(nn, c.dtype) if k == "str" else None
            mx = _agg_max(nn, c.dtype) if k == "str" else None
            vals = [str(int(cnt)), str(int(nulls)), None, None, mn] + [None] * _len(ps) + [mx]
            out.append(_new(c.name, vals, String))
    if any_num and _any(c.dtype._kind not in ("int", "uint", "float", "bool") for c in df._cols):
        pass
    return DataFrame._of(out)


def _pct_label(p):
    v = p * 100
    if v == math.floor(v):
        return str(int(v)) + "%"
    return _rdisp(v) + "%"


def _glimpse(df, max_items, max_name, return_type):
    lines = ["Rows: " + str(df.height), "Columns: " + str(df.width)]
    names = [c.name for c in df._cols]
    w = _max([_len(n) for n in names] + [0])
    tw = _max([_len("<" + _short(c.dtype) + ">") for c in df._cols] + [0])
    for c in df._cols:
        vals = []
        for v in c._v[:max_items]:
            if v is None:
                vals.append("None")
            elif c.dtype._kind == "str":
                vals.append(repr(v) if '"' not in v else repr(v))
                vals[-1] = "'" + v + "'" if "'" not in v else '"' + v + '"'
            elif c.dtype._kind == "float":
                vals.append(repr(v))
            else:
                vals.append(str(v))
        lines.append("$ " + c.name.ljust(w) + " " + ("<" + _short(c.dtype) + ">").ljust(tw) + " " + ", ".join(vals))
    s = "\n".join(lines) + "\n"
    if return_type == "string" or return_type is str:
        return s
    if return_type is None:
        print(s, end="")
        return None
    return s


# ---------- LazyFrame ----------


class LazyFrame:
    # runs each step at once (Brevi's polars has no query planner); collect() returns the result
    def __init__(self, data=None, schema=None, schema_overrides=None, strict=True, orient=None, infer_schema_length=100, nan_to_null=False):
        if isinstance(data, DataFrame):
            self._df = data
        else:
            self._df = DataFrame(data, schema=schema, schema_overrides=schema_overrides, strict=strict, orient=orient, nan_to_null=nan_to_null)

    def collect(self, *a, **k):
        return self._df

    def collect_schema(self):
        return self._df.schema

    @property
    def schema(self):
        return self._df.schema

    @property
    def columns(self):
        return self._df.columns

    @property
    def dtypes(self):
        return self._df.dtypes

    @property
    def width(self):
        return self._df.width

    def lazy(self):
        return self

    def explain(self, *a, **k):
        raise NotImplementedError("brevi: LazyFrame.explain is not in Brevi's polars (it runs each step at once)")

    def show_graph(self, *a, **k):
        raise NotImplementedError("brevi: LazyFrame.show_graph is not in Brevi's polars")

    def fetch(self, n_rows=500):
        return self._df.head(n_rows)

    def __repr__(self):
        return "<LazyFrame at 0x" + ("%X" % (id(self) & 0xFFFFFFFFFFFF)) + ">"

    def group_by(self, *by, maintain_order=False, **named_by):
        return _LazyGroupBy(self._df.group_by(*by, maintain_order=maintain_order, **named_by))

    def groupby(self, *by, maintain_order=False, **named_by):
        return self.group_by(*by, maintain_order=maintain_order, **named_by)

    def join(self, other, *a, **k):
        o = other._df if isinstance(other, LazyFrame) else other
        return LazyFrame(self._df.join(o, *a, **k))

    def sink_csv(self, path, **k):
        self._df.write_csv(path, **k)

    def sink_ndjson(self, path, **k):
        self._df.write_ndjson(path)


class _LazyGroupBy:
    def __init__(self, g):
        self._g = g

    def __getattr__(self, name):
        m = getattr(self._g, name)

        def call(*a, **k):
            return LazyFrame(m(*a, **k))
        return call


def _lazy_method(name):
    def m(self, *a, **k):
        r = getattr(self._df, name)(*a, **k)
        return LazyFrame(r) if isinstance(r, DataFrame) else r
    return m


for _m in ("select", "select_seq", "with_columns", "with_columns_seq", "filter", "remove", "sort", "head", "tail", "limit", "slice", "drop", "rename",
           "unique", "drop_nulls", "drop_nans", "fill_null", "fill_nan", "with_row_index", "with_row_count", "explode", "unpivot", "melt",
           "reverse", "cast", "shift", "sum", "mean", "median", "min", "max", "std", "var", "count", "null_count", "quantile", "top_k",
           "bottom_k", "gather_every", "unnest", "first", "last", "clone", "pipe", "clear"):
    setattr(LazyFrame, _m, _lazy_method(_m))


def _df_first(self):
    return self.head(1)


def _df_last(self):
    return self.tail(1)


DataFrame.first = _df_first
DataFrame.last = _df_last


# ---------- CSV / JSON ----------


def _zmij(v, f32=False):
    # the zmij crate's shortest float text (polars' CSV and JSON writers)
    if v != v:
        return "NaN"
    if v in (math.inf, -math.inf):
        return "inf" if v > 0 else "-inf"
    neg, ds, p = _digits(v, f32)
    sign = "-" if neg else ""
    if ds == "0":
        return sign + "0.0"
    e = p - 1
    lo, hi = (-6, 12) if f32 else (-5, 15)
    if lo <= e <= hi:
        if p <= 0:
            return sign + "0." + "0" * (-p) + ds
        if p >= _len(ds):
            return sign + ds + "0" * (p - _len(ds)) + ".0"
        return sign + ds[:p] + "." + ds[p:]
    m = ds[0] + ("." + ds[1:] if _len(ds) > 1 else "")
    return sign + m + "e" + ("+" if e >= 0 else "-") + str(_abs(e))


def _csv_cell(v, d, sep, quote, null, float_prec, style):
    if v is None:
        return null
    k = d._kind
    if k == "str":
        need = style == "always" or style == "non_numeric" or (style != "never" and (sep in v or quote in v or "\n" in v or "\r" in v or v == ""))
        if v == "" and style not in ("always", "non_numeric"):
            need = True
        if need:
            return quote + v.replace(quote, quote + quote) + quote
        return v
    if k == "bool":
        s = "true" if v else "false"
    elif k == "float":
        s = _zmij(v, d._bits == 32) if float_prec is None else _rfix(v, float_prec)
    elif k in ("int", "uint"):
        s = str(v)
    elif k == "list":
        raise ComputeError("CSV format does not support nested data")
    else:
        s = str(v)
    if style == "always":
        return quote + s + quote
    return s


def _write_csv(df, header, sep, eol, quote, null, float_prec, style):
    out = []
    if header:
        out.append(sep.join(_csv_cell(n, String, sep, quote, "", None, style) if (sep in n or quote in n or "\n" in n or style == "always") else n for n in df.columns) + eol)
    cols = df._cols
    for i in range(df.height):
        out.append(sep.join(_csv_cell(c._v[i], c.dtype, sep, quote, null, float_prec, style) for c in cols) + eol)
    return "".join(out)


def _json_val(v):
    if v is None:
        return "null"
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, float):
        if v != v or v in (math.inf, -math.inf):
            return "null"
        return _zmij(v)
    if isinstance(v, int):
        return str(v)
    if isinstance(v, str):
        return _json.dumps(v, ensure_ascii=False)
    if isinstance(v, list):
        return "[" + ",".join(_json_val(x) for x in v) + "]"
    return _json.dumps(v)


def _path_of(f):
    if isinstance(f, str):
        return f
    if hasattr(f, "__fspath__"):
        return f.__fspath__()
    return None


def _write_out(file, text):
    if file is None:
        return text
    p = _path_of(file)
    if p is not None:
        import _brevi_fs
        _brevi_fs.write(p, text)
        return None
    file.write(text)
    return None


def _read_in(source):
    if isinstance(source, bytes):
        return source.decode("utf-8")
    p = _path_of(source)
    if p is not None:
        import _brevi_fs
        if "://" in p:
            raise NotImplementedError("brevi: reading a URL with polars is not in Brevi's polars")
        try:
            return _brevi_fs.read(p)
        except ValueError as e:
            raise ComputeError(str(e))
    if hasattr(source, "read"):
        t = source.read()
        if isinstance(t, bytes):
            t = t.decode("utf-8")
        return t
    raise TypeError("expected a file path or a file-like object, got " + repr(type(source).__name__))


def _csv_records(text, sep, quote, eol_comment, skip_rows):
    # fields of each record: (value, was_quoted)
    recs = []
    i = 0
    n = _len(text)
    if text.startswith("﻿"):
        i = 1
    row = []
    field = []
    quoted = False
    in_q = False
    started = False
    while i < n:
        c = text[i]
        if in_q:
            if c == quote:
                if i + 1 < n and text[i + 1] == quote:
                    field.append(quote)
                    i += 2
                    continue
                in_q = False
                i += 1
                continue
            field.append(c)
            i += 1
            continue
        if c == quote and quote != "" and not field:
            in_q = True
            quoted = True
            started = True
            i += 1
            continue
        if c == sep:
            row.append(("".join(field), quoted))
            field = []
            quoted = False
            started = True
            i += 1
            continue
        if c == "\n" or c == "\r":
            if c == "\r" and i + 1 < n and text[i + 1] == "\n":
                i += 1
            if started or field or row:
                row.append(("".join(field), quoted))
                recs.append(row)
            row = []
            field = []
            quoted = False
            started = False
            i += 1
            continue
        field.append(c)
        started = True
        i += 1
    if started or field or row:
        row.append(("".join(field), quoted))
        recs.append(row)
    if eol_comment:
        recs = [r for r in recs if not (r and r[0][0].startswith(eol_comment) and not r[0][1])]
    return recs[skip_rows:]


_INT_RE = _re.compile(r"-?(\d+)")
_FLT_RE = _re.compile(r"[-+]?((\d*\.\d+)([eE][-+]?\d+)?|inf|NaN|(\d+)[eE][-+]?\d+|\d+\.)")
_BOOL_RE = _re.compile(r"(?i:true|false)")


def _infer_field(v, quoted):
    if quoted:
        return String
    if _BOOL_RE.fullmatch(v):
        return Boolean
    if _FLT_RE.fullmatch(v):
        return Float64
    if _INT_RE.fullmatch(v):
        if _I64_MIN <= int(v) <= _I64_MAX:
            return Int64
        raise NotImplementedError("brevi: Int128 columns are not in Brevi's polars")
    return String


def _parse_field(v, d):
    k = d._kind
    if k == "str":
        return True, v
    if k == "bool":
        lv = v.lower()
        if lv == "true":
            return True, True
        if lv == "false":
            return True, False
        return False, None
    if k in ("int", "uint"):
        t = v.strip() if False else v
        if _INT_RE.fullmatch(t) or (t[:1] == "+" and _INT_RE.fullmatch(t[1:])):
            iv = int(t)
            lo, hi = _int_range(d)
            if lo <= iv <= hi:
                return True, iv
        return False, None
    if k == "float":
        if _FLT_RE.fullmatch(v) or _INT_RE.fullmatch(v):
            x = float(v) if v not in ("NaN", "-NaN", "+NaN") else math.nan
            return True, (_f32(x) if d._bits == 32 else x)
        return False, None
    if k == "null":
        return True, None
    return False, None


def read_csv(source, has_header=True, columns=None, new_columns=None, separator=",", comment_prefix=None, quote_char='"', skip_rows=0,
             skip_lines=0, schema=None, schema_overrides=None, null_values=None, missing_utf8_is_empty_string=False, ignore_errors=False,
             try_parse_dates=False, n_threads=None, infer_schema=True, infer_schema_length=100, batch_size=8192, n_rows=None, encoding="utf8",
             low_memory=False, rechunk=True, use_pyarrow=False, storage_options=None, skip_rows_after_header=0, row_index_name=None,
             row_index_offset=0, sample_size=1024, eol_char="\n", raise_if_empty=True, truncate_ragged_lines=False, decimal_comma=False,
             glob=True, dtypes=None):
    if try_parse_dates:
        raise NotImplementedError("brevi: read_csv(try_parse_dates=True) (temporal dtypes) is not in Brevi's polars")
    if decimal_comma:
        raise NotImplementedError("brevi: read_csv(decimal_comma=True) is not in Brevi's polars")
    text = _read_in(source)
    if skip_lines:
        text = "\n".join(text.split("\n")[skip_lines:])
    recs = _csv_records(text, separator, quote_char or "", comment_prefix, skip_rows)
    if not recs:
        if raise_if_empty:
            raise NoDataError("empty CSV")
        return DataFrame._of([])
    if has_header:
        hdr = [h for h, _ in recs[0]]
        body = recs[1:]
        names = []
        seen = {}
        for h in hdr:
            if h in seen:
                k = 0
                nm = h + "_duplicated_" + str(k)
                while nm in seen:
                    k += 1
                    nm = h + "_duplicated_" + str(k)
                seen[nm] = 1
                names.append(nm)
            else:
                seen[h] = 1
                names.append(h)
    else:
        body = recs
        names = ["column_" + str(i) for i in range(_len(recs[0]))]
    if skip_rows_after_header:
        body = body[skip_rows_after_header:]
    if n_rows is not None:
        body = body[:n_rows]
    w = _len(names)
    for r in body:
        if _len(r) > w and not truncate_ragged_lines:
            raise ComputeError("found more fields than defined in 'Schema'\n\nConsider setting 'truncate_ragged_lines=True'.")
    nulls = set()
    nullmap = {}
    if null_values is not None:
        if isinstance(null_values, str):
            nulls.add(null_values)
        elif isinstance(null_values, dict):
            nullmap = null_values
        else:
            nulls.update(null_values)
    cols_raw = []
    for j in range(w):
        vals = []
        for r in body:
            if j < _len(r):
                v, q = r[j]
                if (v == "" and not q) or v in nulls or (names[j] in nullmap and v == nullmap[names[j]]):
                    vals.append(None)
                else:
                    vals.append((v, q))
            else:
                vals.append(None)
        cols_raw.append(vals)
    if not has_header:
        names = ["column_" + str(i) for i in range(w)]
    if new_columns:
        names = list(new_columns) + names[_len(new_columns):]
    sch = _schema_items(schema) if schema is not None else None
    ov = {}
    if schema_overrides is not None or dtypes is not None:
        so = schema_overrides if schema_overrides is not None else dtypes
        if isinstance(so, dict):
            ov = {k: _dt(v) for k, v in so.items()}
        else:
            ov = {names[i]: _dt(v) for i, v in enumerate(so)}
    out = []
    for j in range(w):
        name = names[j]
        if sch is not None:
            name, d = sch[j]
        elif name in ov:
            d = ov[name]
        elif not infer_schema:
            d = String
        else:
            cand = []
            lim = _len(cols_raw[j]) if infer_schema_length is None else infer_schema_length
            for x in cols_raw[j][:lim]:
                if x is None:
                    continue
                t = _infer_field(x[0], x[1])
                if t not in cand:
                    cand.append(t)
            if not cand:
                d = String
            elif _len(cand) == 1:
                d = cand[0]
            elif _len(cand) == 2 and Int64 in cand and Float64 in cand:
                d = Float64
            else:
                d = String
        vals = []
        for i, x in enumerate(cols_raw[j]):
            if x is None:
                vals.append(None)
                continue
            ok, y = _parse_field(x[0], d)
            if not ok:
                if ignore_errors:
                    vals.append(None)
                    continue
                raise ComputeError("could not parse `" + x[0] + "` as dtype `" + _short(d) + "` at column '" + name + "' (column number " + str(j + 1) + ")\n\nThe current offset in the file is " + str(i) + " bytes.\n\nYou might want to try:\n- increasing `infer_schema_length` (e.g. `infer_schema_length=10000`),\n- specifying correct dtype with the `schema_overrides` argument\n- setting `ignore_errors` to `True`,\n- adding `" + x[0] + "` to the `null_values` list.\n\nOriginal error: ```remaining bytes non-empty```")
            vals.append(y)
        out.append(_new(name, vals, d))
    if columns is not None:
        if columns and isinstance(columns[0], int):
            out = [out[i] for i in columns]
        else:
            byname = {c.name: c for c in out}
            out = [byname[c] for c in columns]
    df = DataFrame._of(out)
    if row_index_name is not None:
        df = df.with_row_index(row_index_name, row_index_offset)
    return df


def scan_csv(source, **kwargs):
    return LazyFrame(read_csv(source, **kwargs))


def read_json(source, schema=None, schema_overrides=None, infer_schema_length=100):
    data = _json.loads(_read_in(source))
    if isinstance(data, dict):
        data = [data]
    return DataFrame(data, schema=schema, schema_overrides=schema_overrides)


def read_ndjson(source, schema=None, schema_overrides=None, infer_schema_length=100, batch_size=1024, n_rows=None, low_memory=False,
                rechunk=False, row_index_name=None, row_index_offset=0, ignore_errors=False, storage_options=None, retries=2, file_cache_ttl=None,
                include_file_paths=None):
    rows = []
    for line in _read_in(source).split("\n"):
        if line.strip():
            rows.append(_json.loads(line))
    if n_rows is not None:
        rows = rows[:n_rows]
    df = DataFrame(rows, schema=schema, schema_overrides=schema_overrides)
    if row_index_name is not None:
        df = df.with_row_index(row_index_name, row_index_offset)
    return df


def scan_ndjson(source, **kwargs):
    return LazyFrame(read_ndjson(source, **kwargs))


def _no(what):
    def f(*a, **k):
        raise NotImplementedError("brevi: " + what + " is not in Brevi's polars")
    return f


read_parquet = _no("read_parquet")
scan_parquet = _no("scan_parquet")
read_ipc = _no("read_ipc")
scan_ipc = _no("scan_ipc")
read_excel = _no("read_excel")
read_database = _no("read_database")
read_avro = _no("read_avro")
from_pandas = _no("from_pandas")
from_numpy = _no("from_numpy")
from_arrow = _no("from_arrow")
SQLContext = _no("SQLContext")
sql = _no("pl.sql")


def show_versions():
    print("--------Version info---------\nPolars:              " + __version__)


def thread_pool_size():
    return 1

