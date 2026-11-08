# Brevi's own copy of the MCP Python SDK's server side (mcp 2.3.0, the official `mcp` package): MCPServer with
# its tools, resources, prompts and Context, the low-level Server, the stdio transport, and the streamable
# HTTP and SSE transports, written for Brevi so that a server builds into one native binary. What a client
# sees over the wire matches what the SDK sends under CPython: the same JSON-RPC replies with the same keys in
# the same order, the same input and output schemas, the same error texts. Anything a program uses that is
# not here is refused when the program is built (src/caps.rs, src/dynpy/mcpsig.rs), and CPython's mcp runs it.
#
# Pieces: the protocol types are models of Brevi's pydantic (_brevi_mcp_models.py, generated); a result is
# shaped for the wire by the SDK's per-version wire models (_brevi_mcp_types.py, generated tables, see
# _sieve); a tool function's signature comes from the compiler (fn.__brevi_sig__, see src/dynpy/mcpsig.rs)
# since Brevi does not evaluate annotations.
import json
import sys
import asyncio
import base64
import re

import _brevi_stdio
import _brevi_async
import pydantic
from pydantic import BaseModel, ValidationError
import _brevi_mcp_models as T
import _brevi_mcp_types as _W


# ---------------------------------------------------------------- logging, as the SDK's configure_logging sets it
# (logging.basicConfig(level, format="%(message)s") on stderr; rich is not used)

_LEVELS = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40, "CRITICAL": 50}
_log_level = [30]


def _log(level, msg, *args):
    if level < _log_level[0]:
        return
    if args:
        msg = msg % args
    sys.stderr.write(msg + "\n")
    sys.stderr.flush()


def _log_exc(e, msg, *args):
    # logger.exception: the message, then the traceback (its frames are CPython's own files; only the last line,
    # the exception, is the same)
    if 40 < _log_level[0]:
        return
    if args:
        msg = msg % args
    lines = [msg]
    chain = []
    x = e
    while x is not None:
        chain.append(x)
        x = x.__cause__ if x.__cause__ is not None else (getattr(x, "__context__", None) if not getattr(x, "__suppress_context__", False) else None)
    chain.reverse()
    for i, x in enumerate(chain):
        lines.append("Traceback (most recent call last):")
        lines.append(_exc_line(x))
        if i < len(chain) - 1:
            lines.append("")
            if chain[i + 1].__cause__ is x:
                lines.append("The above exception was the direct cause of the following exception:")
            else:
                lines.append("During handling of the above exception, another exception occurred:")
            lines.append("")
    sys.stderr.write("\n".join(lines) + "\n")
    sys.stderr.flush()


def _exc_line(x):
    n = _qualname(type(x))
    s = str(x)
    return n + ": " + s if s else n


def _qualname(c):
    m = getattr(c, "__module__", "builtins")
    if m in ("builtins", "__main__", None):
        return c.__name__
    return m + "." + c.__name__


def configure_logging(level="INFO"):
    if not _cfg_done[0]:
        _cfg_done[0] = True
        _log_level[0] = _LEVELS.get(level, 20)
        _set_root_level(_log_level[0])


_cfg_done = [False]


def _set_root_level(n):
    # (the program's own logging, when it has CPython's, is not configured by Brevi's mcp)
    pass


def _warn_dep(msg):
    # MCPDeprecationWarning, as warnings.warn prints it once per call site (the site is not known here)
    if msg in _warned:
        return
    _warned.add(msg)


_warned = set()


# ---------------------------------------------------------------- errors


INVALID_PARAMS = -32602
METHOD_NOT_FOUND = -32601
INTERNAL_ERROR = -32603
INVALID_REQUEST = -32600
PARSE_ERROR = -32700


class MCPError(Exception):
    def __init__(self, code, message, data=None):
        super().__init__(code, message, data)
        if data is not None:
            self.error = T.ErrorData(code=code, message=message, data=data)
        else:
            self.error = T.ErrorData(code=code, message=message)

    @property
    def code(self):
        return self.error.code

    @property
    def message(self):
        return self.error.message

    @property
    def data(self):
        return self.error.data

    @classmethod
    def from_error_data(cls, error):
        return cls(error.code, error.message, error.data)

    def __str__(self):
        return self.error.message


MCPError.__module__ = "mcp.shared.exceptions"


class MCPDeprecationWarning(UserWarning):
    pass


MCPDeprecationWarning.__module__ = "mcp.shared.exceptions"


class MCPServerError(Exception):
    pass


class ValidationErrorMCP(MCPServerError):
    pass


class ResourceError(MCPServerError):
    pass


class UnexpectedResourceError(ResourceError):
    pass


class ResourceNotFoundError(ResourceError):
    pass


class ToolError(MCPServerError):
    pass


class UnexpectedToolError(ToolError):
    pass


class InvalidSignature(Exception):
    pass


for _c in (MCPServerError, ResourceError, UnexpectedResourceError, ResourceNotFoundError, ToolError, UnexpectedToolError, InvalidSignature):
    _c.__module__ = "mcp.server.mcpserver.exceptions"
ValidationErrorMCP.__name__ = "ValidationError"
ValidationErrorMCP.__module__ = "mcp.server.mcpserver.exceptions"


class InvalidUriTemplate(ValueError):
    def __init__(self, message, *, template, position=None):
        super().__init__(message)
        self.template = template
        self.position = position


InvalidUriTemplate.__module__ = "mcp.shared.uri_template"


# ---------------------------------------------------------------- JSON


def _jdump(v):
    return pydantic._jdump(v)


def _to_json_text(result, indent):
    # pydantic_core.to_json(result, fallback=str, indent=2).decode()
    return pydantic._jdump(_jsonable(result), indent)


def _jsonable(v):
    if v is None or v is True or v is False:
        return v
    t = type(v)
    if t is int or t is str:
        return v
    if t is float:
        if v != v or v in (float("inf"), float("-inf")):
            return None
        return v
    if isinstance(v, BaseModel):
        return v.model_dump(mode="json")
    if isinstance(v, (list, tuple)):
        return [_jsonable(e) for e in v]
    if isinstance(v, (set, frozenset)):
        return [_jsonable(e) for e in v]
    if isinstance(v, dict):
        out = {}
        for k, e in v.items():
            if isinstance(k, str):
                k2 = k
            elif k is None:
                k2 = "None"
            elif k is True:
                k2 = "true"
            elif k is False:
                k2 = "false"
            elif isinstance(k, (int, float)):
                k2 = pydantic._jdump(k)
            else:
                k2 = str(k)
            out[k2] = _jsonable(e)
        return out
    if isinstance(v, bytes):
        return v.decode("utf-8")
    if isinstance(v, int):
        return int(v)
    if isinstance(v, float):
        return float(v)
    if isinstance(v, str):
        return str(v)
    if _is_enum(v):
        return _jsonable(v.value)
    if hasattr(v, "__dataclass_fields__"):
        out = {}
        for k in v.__dataclass_fields__:
            out[k] = _jsonable(getattr(v, k))
        return out
    return str(v)


def _is_enum(v):
    for b in type(v).__mro__:
        if b.__name__ == "Enum" and b.__module__ == "enum":
            return True
    return False


def _parse_json(text):
    # pydantic's validate_json (jiter): None when it is not JSON
    try:
        return True, pydantic._jparse(text)
    except Exception:
        return False, None


# ---------------------------------------------------------------- the wire shapes (generated tables)


def _table(text):
    out = {}
    for name, pyname, base, extra, fields in json.loads(text):
        fs = []
        for f in fields:
            fs.append((f[0], f[1] if f[1] is not None else f[0], _tup(f[2]), f[3][0] == "r", f[3]))
        out[name] = (fs, extra)
    return out


def _tup(d):
    if isinstance(d, list):
        return tuple(_tup(x) for x in d)
    return d


_WH = None
_WM = None
_SURF = None


def _wire(era):
    global _WH, _WM, _SURF
    if _SURF is None:
        _WH = _table(_W.WIRE_H)
        _WM = _table(_W.WIRE_M)
        _SURF = json.loads(_W.SURFACE)
    return _WH if era == "h" else _WM


def _surface(kind):
    _wire("h")
    return _SURF[kind]


class _Bad(Exception):
    pass


def _fits(d, v, tab):
    # whether pydantic (lax, python input) would validate v as d
    if d is None:
        return True
    if type(d) is str:
        if d == "str":
            return isinstance(v, str)
        if d == "int":
            return (isinstance(v, int) and not isinstance(v, bool)) or (isinstance(v, float) and v == int(v)) if not isinstance(v, bool) else True
        if d == "float":
            return isinstance(v, (int, float))
        if d == "bool":
            return isinstance(v, bool) or (isinstance(v, int) and v in (0, 1))
        if d == "none":
            return v is None
        if d == "bytes":
            return isinstance(v, (str, bytes))
        return True
    k = d[0]
    if k == "Lit":
        for x in d[1]:
            if v == x and type(v) is type(x):
                return True
        return False
    if k == "L":
        if not isinstance(v, (list, tuple)):
            return False
        for x in v:
            if not _fits(d[1], x, tab):
                return False
        return True
    if k == "D":
        if not isinstance(v, dict):
            return False
        for x in v.values():
            if not _fits(d[1], x, tab):
                return False
        return True
    if k == "U":
        for x in d[1]:
            if _fits(x, v, tab):
                return True
        return False
    if k == "M":
        if not isinstance(v, dict):
            return False
        fs, extra = tab[d[1]]
        for name, key, fd, req, dflt in fs:
            if key in v:
                if not _fits(fd, v[key], tab):
                    return False
            elif req:
                return False
        if extra == "forbid":
            keys = {f[1] for f in fs}
            for k2 in v:
                if k2 not in keys:
                    return False
        return True
    return True


def _pick(ds, v, tab):
    # pydantic's smart-mode union: an exact fit first; of models, the one the most fields of v fit
    best = None
    best_n = -1
    for x in ds:
        if not _fits(x, v, tab):
            continue
        if type(x) is tuple and x[0] == "M":
            fs = tab[x[1]][0]
            keys = {f[1] for f in fs}
            n = 0
            for key in v:
                if key in keys:
                    n += 1
            if n > best_n:
                best, best_n = x, n
        elif best is None:
            if type(x) is str and x in ("int", "float", "str", "bool", "none"):
                if _exact(x, v):
                    return x
                if best is None:
                    best = x
                continue
            return x
    if best is None:
        raise _Bad()
    if type(best) is str:
        for x in ds:
            if type(x) is str and _exact(x, v):
                return x
    return best


def _exact(x, v):
    if x == "int":
        return type(v) is int
    if x == "float":
        return type(v) is float
    if x == "str":
        return type(v) is str
    if x == "bool":
        return type(v) is bool
    if x == "none":
        return v is None
    return False


def _shape(d, v, tab):
    # v, validated as d and dumped (by alias, mode json, exclude_none)
    if d is None:
        return v
    if type(d) is str:
        if d == "float" and type(v) is int:
            return float(v)
        if d == "int" and type(v) is float:
            return int(v)
        if d == "bool" and type(v) is int:
            return bool(v)
        return v
    k = d[0]
    if k == "L":
        return [_shape(d[1], x, tab) for x in v]
    if k == "D":
        return {key: _shape(d[1], x, tab) for key, x in v.items()}
    if k == "U":
        return _shape(_pick(d[1], v, tab), v, tab)
    if k == "M":
        fs, extra = tab[d[1]]
        out = {}
        known = set()
        for name, key, fd, req, dflt in fs:
            known.add(key)
            if key in v:
                x = v[key]
                if x is None:
                    continue
                y = _shape(fd, x, tab)
                if y is not None:
                    out[key] = y
            elif dflt[0] == "v" and dflt[1] is not None:
                out[key] = dflt[1]
        if extra == "allow":
            for key, x in v.items():
                if key not in known and x is not None:
                    out[key] = x
        return out
    return v


def _sieve(method, version, data):
    # methods.serialize_server_result: the result as the version's wire model has it
    surf = _surface("results").get(method)
    if surf is None or version not in surf:
        raise KeyError(method)
    name = surf[version]
    era = "m" if version == "2026-07-28" else "h"
    tab = _wire(era)
    if isinstance(name, list):
        d = ("U", tuple(("M", n) for n in name), None)
    else:
        d = ("M", name)
    if not _fits(d, data, tab):
        raise _Bad()
    return _shape(d, data, tab)


def _why_bad(method, version, data):
    # the pydantic error CPython logs for a result _sieve refused: the result model's own, when it has one
    name = _surface("results").get(method, {}).get(version)
    m = getattr(T, name, None) if isinstance(name, str) else None
    if m is None:
        return None
    try:
        m.model_validate(data)
    except ValidationError as e:
        return e
    return None


def _request_ok(method, version, params):
    # methods.validate_client_request: KeyError when the method is not in the version; False when the params do
    # not fit its wire request
    surf = _surface("requests").get(method)
    if surf is None or version not in surf:
        raise KeyError(method)
    era = "m" if version == "2026-07-28" else "h"
    tab = _wire(era)
    body = {"jsonrpc": "2.0", "id": 0, "method": method}
    if params is not None:
        body["params"] = params
    return _fits(("M", surf[version]), body, tab)


_SPEC_METHODS = None


def _spec_methods():
    global _SPEC_METHODS
    if _SPEC_METHODS is None:
        _SPEC_METHODS = set(_surface("requests").keys())
    return _SPEC_METHODS


def _dump(m):
    return m.model_dump(by_alias=True, mode="json", exclude_none=True)


# ---------------------------------------------------------------- JSON schemas, as pydantic writes them
# Descriptors are Brevi's pydantic's (see src/dynpy/py/pydantic.py: ('int',), ('list', d, 'list', bare), ...).


def _field_title(name):
    # pydantic's default field title
    return name.title().replace("_", " ")


def _sort_schema(v, parent=None):
    if isinstance(v, dict):
        keys = list(v.keys())
        if parent not in ("properties", "default"):
            keys = sorted(keys)
        out = {}
        for k in keys:
            out[k] = _sort_schema(v[k], k)
        return out
    if isinstance(v, list):
        return [_sort_schema(x, parent) for x in v]
    return v


class _Defs:
    def __init__(self):
        self.defs = {}
        self.building = set()


def _cons_of(metas):
    # the constraints and field info in Annotated metadata / Field(...)
    cons = {}
    info = {}
    for m in metas:
        if isinstance(m, pydantic.FieldInfo):
            for k in ("title", "description", "examples", "default", "default_factory", "alias", "deprecated", "json_schema_extra"):
                v = getattr(m, k, None)
                if k == "default":
                    if v is not pydantic.PydanticUndefined:
                        info.setdefault("default", v)
                    continue
                if v is not None and k not in info:
                    info[k] = v
            for mm in m.metadata:
                _meta_into(mm, cons)
        else:
            _meta_into(m, cons)
    return cons, info


def _meta_into(m, cons):
    if isinstance(m, pydantic._Meta):
        cons[m.key] = m.value
    elif isinstance(m, pydantic._General):
        for k, v in m.items:
            cons[k] = v


_NUM_KEYS = {"gt": "exclusiveMinimum", "ge": "minimum", "lt": "exclusiveMaximum", "le": "maximum", "multiple_of": "multipleOf"}


def _apply_cons(s, cons, kind):
    for k, v in cons.items():
        if k in _NUM_KEYS and kind in ("int", "float"):
            s[_NUM_KEYS[k]] = v
        elif k == "min_length":
            s["minLength" if kind in ("str", "bytes") else ("minProperties" if kind == "dict" else "minItems")] = v
        elif k == "max_length":
            s["maxLength" if kind in ("str", "bytes") else ("maxProperties" if kind == "dict" else "maxItems")] = v
        elif k == "pattern" and kind == "str":
            s["pattern"] = v
        elif k == "allow_inf_nan" or k == "strict":
            pass
    return s


def _schema(d, cx, metas=()):
    t = d[0]
    if t == "ann":
        return _schema(d[1], cx, tuple(d[2]) + tuple(metas))
    cons, info = _cons_of(metas)
    if t == "cls":
        v = d[1]
        if isinstance(v, tuple):
            return _schema(v, cx, metas)
        return _cls_schema(v, cx, cons)
    if t == "lazy":
        return _cls_schema(d[1](), cx, cons)
    if t in ("int", "float", "str", "bool", "bytes", "none"):
        s = {"int": {"type": "integer"}, "float": {"type": "number"}, "str": {"type": "string"}, "bool": {"type": "boolean"},
             "bytes": {"format": "binary", "type": "string"}, "none": {"type": "null"}}[t]
        return _apply_cons(dict(s), cons, t)
    if t == "any":
        return {}
    if t in ("list", "set", "frozenset", "tuplevar"):
        s = {"items": _schema(d[1], cx), "type": "array"}
        if t in ("set", "frozenset"):
            s["uniqueItems"] = True
        return _apply_cons(s, cons, "list")
    if t == "tuple":
        n = len(d[1])
        s = {"maxItems": n, "minItems": n, "type": "array"}
        if n:
            s["prefixItems"] = [_schema(x, cx) for x in d[1]]
        return s
    if t == "dict":
        if d[4]:
            s = {"additionalProperties": True, "type": "object"}
        else:
            s = {"additionalProperties": _schema(d[2], cx), "type": "object"}
            if d[1][0] not in ("str", "any", "int", "float"):
                pass
        return _apply_cons(s, cons, "dict")
    if t == "union":
        arms = []
        for x in d[1]:
            sx = _schema(x, cx)
            if sx not in arms:
                arms.append(sx)
        if len(arms) == 1:
            return arms[0]
        return {"anyOf": arms}
    if t == "literal":
        vals = [(v.value if _is_enum(v) else v) for v in d[1]]
        types = set()
        for v in vals:
            if v is None:
                types.add("null")
            elif isinstance(v, bool):
                types.add("boolean")
            elif isinstance(v, int):
                types.add("integer")
            elif isinstance(v, float):
                types.add("number")
            elif isinstance(v, str):
                types.add("string")
            else:
                types.add("?")
        s = {}
        if len(vals) == 1:
            s["const"] = vals[0]
        else:
            s["enum"] = vals
        if len(types) == 1 and "?" not in types:
            s["type"] = list(types)[0]
        return s
    raise InvalidSignature("no JSON schema for %r" % (d,))


def _cls_schema(c, cx, cons):
    if c is int:
        return _apply_cons({"type": "integer"}, cons, "int")
    if c is float:
        return _apply_cons({"type": "number"}, cons, "float")
    if c is str:
        return _apply_cons({"type": "string"}, cons, "str")
    if c is bool:
        return {"type": "boolean"}
    if c is type(None):
        return {"type": "null"}
    if c is object:
        return {}
    if isinstance(c, type) and issubclass(c, BaseModel):
        name = c.__name__
        if name not in cx.defs and name not in cx.building:
            cx.building.add(name)
            cx.defs[name] = _model_schema(c, cx)
            cx.building.discard(name)
        return {"$ref": "#/$defs/" + name}
    if isinstance(c, type) and _is_enum_cls(c):
        name = c.__name__
        if name not in cx.defs:
            vals = [m.value for m in c]
            s = {"enum": vals, "title": name}
            ts = set()
            for v in vals:
                if isinstance(v, bool):
                    ts.add("boolean")
                elif isinstance(v, int):
                    ts.add("integer")
                elif isinstance(v, float):
                    ts.add("number")
                elif isinstance(v, str):
                    ts.add("string")
                else:
                    ts.add("?")
            if len(ts) == 1 and "?" not in ts:
                s["type"] = list(ts)[0]
            doc = c.__dict__.get("__doc__")
            if doc and doc != "An enumeration.":
                s["description"] = _clean_doc(doc)
            cx.defs[name] = s
        return {"$ref": "#/$defs/" + name}
    # a constrained alias of Brevi's pydantic (PositiveInt, ...)
    d = getattr(c, "_brevi_desc", None)
    if d is not None:
        return _schema(d, cx)
    raise InvalidSignature("no JSON schema for %r" % (c,))


def _is_enum_cls(c):
    for b in c.__mro__:
        if b.__name__ == "Enum" and b.__module__ == "enum":
            return True
    return False


def _clean_doc(doc):
    # inspect.cleandoc, as pydantic reads a model's docstring
    lines = doc.expandtabs().split("\n")
    margin = None
    for l in lines[1:]:
        content = len(l.lstrip())
        if content:
            ind = len(l) - content
            margin = ind if margin is None else min(margin, ind)
    out = [lines[0].lstrip()]
    for l in lines[1:]:
        out.append(l[margin:] if margin is not None else l)
    while out and not out[-1]:
        out.pop()
    while out and not out[0]:
        out.pop(0)
    return "\n".join(out)


def _model_schema(cls, cx):
    pydantic._schema(cls)
    info = pydantic._INFO[cls]
    props = {}
    req = []
    for name, f in info.fields.items():
        key = f.alias if f.alias is not None else name
        props[key] = _prop_schema(name, f._desc, f, cx)
        if f.default is pydantic.PydanticUndefined and f.default_factory is None:
            req.append(key)
    s = {"properties": props}
    if req:
        s["required"] = req
    s["title"] = info.cfg.get("title") or cls.__name__
    s["type"] = "object"
    doc = cls.__dict__.get("__doc__")
    if doc:
        s["description"] = _clean_doc(doc)
    if info.cfg.get("extra") == "forbid":
        s["additionalProperties"] = False
    return s


def _prop_schema(name, d, f, cx):
    # one field: its type's schema with the field's title, default, description, ...
    metas = list(f.metadata) if f is not None else []
    s = _schema(d, cx, metas)
    s = dict(s)
    # pydantic sets no title on a field whose schema is a model or enum ($ref), or such a type or None
    any_of = s.get("anyOf")
    is_ref = "$ref" in s or (isinstance(any_of, list) and len(any_of) == 2 and {"type": "null"} in any_of
                             and any(isinstance(x, dict) and list(x) == ["$ref"] for x in any_of))
    title = getattr(f, "title", None) if f is not None else None
    if title is not None:
        s["title"] = title
    elif not is_ref:
        s["title"] = _field_title(name)
    if f is not None:
        if f.description is not None:
            s["description"] = f.description
        if f.default is not pydantic.PydanticUndefined:
            s["default"] = _jsonable(f.default)
        if getattr(f, "examples", None) is not None:
            s["examples"] = _jsonable(f.examples)
        if getattr(f, "deprecated", None):
            s["deprecated"] = True
        extra = getattr(f, "json_schema_extra", None)
        if isinstance(extra, dict):
            s.update(extra)
    return s


# ---------------------------------------------------------------- function signatures
# fn.__brevi_sig__ (set by the compiler): (params, ret, has_ret, is_async, doc), params as
# (name, descriptor or None, kind, has_default, default) with kind 0 positional-or-keyword, 1 *args,
# 2 keyword-only, 3 **kwargs. _sig gives params as (name, descriptor or None, default or _NO, kind).


class _NoDefault:
    def __repr__(self):
        return "<empty>"


_NO = _NoDefault()


def _sig(fn):
    s = getattr(fn, "__brevi_sig__", None)
    if s is None:
        f = getattr(fn, "__func__", None)
        if f is not None:
            s = getattr(f, "__brevi_sig__", None)
            if s is not None:
                s = _norm_sig(s)
                return (s[0][1:],) + tuple(s[1:])
        w = getattr(fn, "__wrapped__", None)
        if w is not None:
            return _sig(w)
        if hasattr(fn, "__call__") and not hasattr(fn, "__name__"):
            c = getattr(type(fn), "__call__", None)
            cs = getattr(c, "__brevi_sig__", None) if c is not None else None
            if cs is not None:
                cs = _norm_sig(cs)
                return (cs[0][1:],) + tuple(cs[1:])
        raise InvalidSignature("Brevi: no signature for %r" % (fn,))
    return _norm_sig(s)


def _norm_sig(s):
    ps = s[0]
    if ps and len(ps[0]) == 5:
        ps = [(p[0], p[1], p[4] if p[3] else _NO, p[2]) for p in ps]
        return (ps,) + tuple(s[1:])
    return s


def _doc(fn):
    # the function's docstring: the compiler's copy in its signature, else __doc__
    try:
        d = _sig(fn)[4]
    except InvalidSignature:
        d = None
    if d is None:
        d = getattr(fn, "__doc__", None)
    return d


def _is_async(fn):
    try:
        return bool(_sig(fn)[3])
    except InvalidSignature:
        return False


def _desc_cls(d):
    if d is None:
        return None
    while d[0] == "ann":
        d = d[1]
    if d[0] == "cls" and not isinstance(d[1], tuple):
        return d[1]
    if d[0] == "lazy":
        try:
            return d[1]()
        except NameError:
            return None
    return None


def find_context_parameter(fn):
    try:
        params = _sig(fn)[0]
    except InvalidSignature:
        return None
    for name, d, dflt, kind in params:
        if d is None:
            continue
        x = d
        while x[0] == "ann":
            x = x[1]
        arms = x[1] if x[0] == "union" else (x,)
        for a in arms:
            c = _desc_cls(a)
            if isinstance(c, type) and issubclass(c, Context):
                return name
    return None


class ArgModelBase(BaseModel):
    model_config = {"arbitrary_types_allowed": True}

    def model_dump_one_level(self):
        kw = {}
        for name, f in type(self).model_fields.items():
            kw[f.alias if f.alias else name] = getattr(self, name)
        return kw


ArgModelBase.__module__ = "mcp.server.mcpserver.utilities.func_metadata"

_BASEMODEL_CALLABLES = ("copy", "dict", "json", "model_construct", "model_copy", "model_dump", "model_dump_json", "model_json_schema",
                        "model_parametrized_name", "model_post_init", "model_rebuild", "model_validate", "model_validate_json",
                        "model_validate_strings", "parse_file", "parse_obj", "parse_raw", "schema", "schema_json", "update_forward_refs",
                        "validate", "from_orm", "construct")


def _make_model(name, fields, base=None):
    # pydantic.create_model with (name, descriptor, FieldInfo) fields, through Brevi's pydantic
    anns = []
    ns = {}
    for fname, d, info in fields:
        anns.append((fname, d))
        ns[fname] = info
    ns["__annotations__"] = tuple(anns)
    ns["__module__"] = "pydantic.main"
    return type(name, (base or BaseModel,), ns)


class FuncMetadata:
    def __init__(self, arg_model, arg_fields, output_schema=None, output_model=None, wrap_output=False, out_desc=None):
        self.arg_model = arg_model
        self.arg_fields = arg_fields
        self.output_schema = output_schema
        self.output_model = output_model
        self.wrap_output = wrap_output
        self.out_desc = out_desc

    def pre_parse_json(self, data):
        new = dict(data)
        keys = {}
        for name, f in self.arg_model.model_fields.items():
            keys[name] = f
            if f.alias:
                keys[f.alias] = f
        for k, v in data.items():
            f = keys.get(k)
            if f is None:
                continue
            if isinstance(v, str) and not _is_str_desc(f._desc):
                ok, parsed = _parse_json(v)
                if not ok:
                    continue
                if isinstance(parsed, (str, int, float)):
                    continue
                new[k] = parsed
        return new

    def validate_arguments(self, arguments):
        pre = self.pre_parse_json(arguments)
        m = self.arg_model.model_validate(pre)
        return m.model_dump_one_level()

    async def call_fn(self, fn, fn_is_async, arguments, direct=None):
        kw = dict(arguments)
        if direct:
            kw.update(direct)
        r = fn(**kw)
        if _brevi_async.iscoroutine(r):
            r = await r
        return r

    def convert_result(self, result):
        if isinstance(result, T.InputRequiredResult):
            return result
        output_model = self.output_model if self.output_schema is not None else None
        if isinstance(result, T.CallToolResult):
            if output_model is not None and not result.is_error:
                _validate_output(self, result.structured_content)
            return result
        content = _convert_to_content(result)
        if output_model is None:
            return T.CallToolResult(content=content)
        if self.wrap_output:
            result = {"result": result}
        structured = _validate_output(self, result)
        return T.CallToolResult(content=content, structured_content=structured)


def _is_str_desc(d):
    # field_info.annotation is str (Annotated[str, ...] keeps str as the annotation)
    return d[0] == "str" or (d[0] == "cls" and d[1] is str)


def _validate_output(meta, result):
    m = meta.output_model
    if isinstance(m, type) and issubclass(m, BaseModel):
        if isinstance(result, m):
            v = result
        elif isinstance(result, BaseModel):
            v = m.model_validate(result.model_dump())
        else:
            v = m.model_validate(result)
        return v.model_dump(mode="json", by_alias=True)
    # dict[str, T] (a TypeAdapter over the annotation): validated through a one-field model
    w = meta.dict_model
    v = w.model_validate({"root": result})
    return v.model_dump(mode="json", by_alias=True)["root"]


def _sig_params(fn):
    return _sig(fn)[0]


def func_metadata(fn, skip_names=(), structured_output=None):
    s = _sig(fn)
    params, ret, has_ret = s[0], s[1], s[2]
    fname = getattr(fn, "__name__", type(fn).__name__)
    fields = []
    arg_fields = []
    for name, d, dflt, kind in params:
        if name.startswith("_"):
            raise InvalidSignature("Parameter %s of %s cannot start with '_'" % (name, fname))
        if name in skip_names:
            continue
        no_ann = d is None
        if no_ann:
            d = ("any", "Any")
        info = pydantic.Field(dflt) if dflt is not _NO else pydantic.Field()
        field_name = name
        if name in _BASEMODEL_CALLABLES:
            info = pydantic.Field(dflt, alias=name) if dflt is not _NO else pydantic.Field(alias=name)
            field_name = "field_" + name
        info._brevi_noann = no_ann
        fields.append((field_name, d, info))
        arg_fields.append((field_name, name, d, info, no_ann))
    model = _make_model(fname + "Arguments", fields, ArgModelBase)
    meta = FuncMetadata(model, arg_fields)
    if structured_output is False:
        return meta
    if not has_ret:
        if structured_output is True:
            raise InvalidSignature("Function %s: return annotation required for structured output" % fname)
        # no annotation: inspect's `empty`, which is a class -> the "other class" case with no type hints
        return meta
    rd = ret
    while rd[0] == "ann":
        rd = rd[1]
    while rd[0] == "cls" and isinstance(rd[1], tuple):
        rd = rd[1]
    rc = _desc_cls(rd)
    if rd[0] == "union":
        arms = [a for a in rd[1] if not _is_input_required(_desc_cls(a))]
        if not arms:
            return meta
        if len(arms) != len(rd[1]):
            rd = arms[0] if len(arms) == 1 else ("union", tuple(arms))
            ret = rd
            rc = _desc_cls(rd)
        if len(arms) > 1:
            for a in arms:
                c = _desc_cls(a)
                if isinstance(c, type) and issubclass(c, T.CallToolResult):
                    raise InvalidSignature("Function %s: CallToolResult cannot be used in Union or Optional types. To return empty results, use: CallToolResult(content=[])" % fname)
    if _is_input_required(rc):
        return meta
    if isinstance(rc, type) and issubclass(rc, T.CallToolResult):
        anns = _ann_metas(ret)
        if anns:
            rd = anns[0]
            if not isinstance(rd, tuple):
                rd = ("cls", rd)
            ret = rd
            rc = _desc_cls(rd)
        else:
            return meta
    if structured_output is None and _returns_content(rd):
        return meta
    model, wrap, dict_model = _output_model(ret, rd, rc, fname)
    if model is not None:
        try:
            cx = _Defs()
            if wrap:
                schema = _top_schema(model, cx)
            elif dict_model is not None:
                schema = _schema(rd, cx)
                schema = dict(schema)
                schema["title"] = fname + "DictOutput"
                if cx.defs:
                    schema["$defs"] = cx.defs
                schema = _sort_schema(schema)
            else:
                schema = _top_schema(model, cx)
            meta.output_schema = schema
            meta.output_model = model
            meta.wrap_output = wrap
            meta.dict_model = dict_model
            meta.out_desc = ret
            return meta
        except InvalidSignature as e:
            _log(20, "Cannot create schema for type %s in %s: %s: %s" % (_show(rd), fname, "PydanticSchemaGenerationError", e))
    if structured_output is True:
        raise InvalidSignature("Function %s: return type %s is not serializable for structured output" % (fname, _show(rd)))
    return meta


def _show(d):
    try:
        return pydantic._show(d)
    except Exception:
        return repr(d)


def _ann_metas(d):
    if d[0] == "ann":
        return list(d[2])
    return []


def _is_input_required(c):
    return isinstance(c, type) and issubclass(c, T.InputRequiredResult)


_CONTENT = None


def _content_types():
    global _CONTENT
    if _CONTENT is None:
        _CONTENT = T.ContentBlock + (Image, Audio)
    return _CONTENT


def _returns_content(d):
    while d[0] == "ann":
        d = d[1]
    if d[0] in ("union", ):
        return any(_returns_content(a) for a in d[1])
    if d[0] in ("list", "tuplevar"):
        return _returns_content(d[1])
    if d[0] == "tuple":
        return any(_returns_content(a) for a in d[1])
    c = _desc_cls(d)
    if isinstance(c, tuple):
        return True
    return isinstance(c, type) and issubclass(c, _content_types())


def _output_model(ret, rd, rc, fname):
    t = rd[0]
    if t == "none" or rc is type(None):
        return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    # a bare builtin class (list, dict, ...) is a class without type hints: no structured output; typing's
    # forms (List, Dict[str, int], ...) are not types.GenericAlias and are wrapped
    if t in ("list", "set", "frozenset", "tuplevar"):
        if rd[2] in ("list", "set", "frozenset", "tuple") and rd[3]:
            return None, False, None
        return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    if t in ("tuple", "union", "literal"):
        return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    if t == "dict":
        if rd[3] == "dict" and rd[4]:
            return None, False, None
        if rd[3] == "dict" and rd[1][0] == "str":
            return "dict", False, _make_model(fname + "DictOutput", [("root", ret, pydantic.Field())])
        return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    if t in ("int", "float", "str", "bool", "bytes"):
        return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    if t == "any":
        if rd[1] in ("Any", "object"):
            return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
    if isinstance(rc, type):
        if issubclass(rc, BaseModel):
            return rc, False, None
        if rc in (str, int, float, bool, bytes, type(None)):
            return _make_model(fname + "Output", [("result", ret, pydantic.Field())]), True, None
        if t == "any" and rc is not object:
            return None, False, None
        # a dataclass or another class with annotations: a model of its fields
        fs = getattr(rc, "__dataclass_fields__", None)
        if fs:
            raise InvalidSignature("class return types")
        return None, False, None
    return None, False, None


def _top_schema(model, cx):
    s = _model_schema(model, cx)
    if cx.defs:
        s["$defs"] = cx.defs
    return _sort_schema(s)


def _args_schema(meta):
    # arg_model.model_json_schema(by_alias=True)
    cx = _Defs()
    model = meta.arg_model
    pydantic._schema(model)
    info = pydantic._INFO[model]
    props = {}
    req = []
    for field_name, name, d, f, no_ann in meta.arg_fields:
        fi = info.fields[field_name]
        key = fi.alias if fi.alias is not None else field_name
        if no_ann:
            s = {"title": name, "type": "string"}
            if fi.default is not pydantic.PydanticUndefined:
                s["default"] = _jsonable(fi.default)
            props[key] = s
        else:
            props[key] = _prop_schema(field_name, fi._desc, fi, cx)
        if fi.default is pydantic.PydanticUndefined and fi.default_factory is None:
            req.append(key)
    s = {"properties": props}
    if req:
        s["required"] = req
    s["title"] = model.__name__
    s["type"] = "object"
    if cx.defs:
        s["$defs"] = cx.defs
    return _sort_schema(s)


def _convert_to_content(result):
    if result is None:
        return []
    if isinstance(result, T.ContentBlock):
        return [result]
    if isinstance(result, Image):
        return [result.to_image_content()]
    if isinstance(result, Audio):
        return [result.to_audio_content()]
    if isinstance(result, (list, tuple)):
        out = []
        for x in result:
            out.extend(_convert_to_content(x))
        return out
    if not isinstance(result, str):
        result = _to_json_text(result, 2)
    return [T.TextContent(type="text", text=result)]


# ---------------------------------------------------------------- Image and Audio


class Image:
    def __init__(self, path=None, data=None, format=None):
        if path is None and data is None:
            raise ValueError("Either path or data must be provided")
        if path is not None and data is not None:
            raise ValueError("Only one of path or data can be provided")
        self.path = path
        self.data = data
        self._format = format
        self._mime_type = self._get_mime_type()

    def _get_mime_type(self):
        if self._format:
            return "image/" + self._format.lower()
        if self.path:
            suffix = _suffix(str(self.path))
            return {".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".gif": "image/gif", ".webp": "image/webp"}.get(suffix, "application/octet-stream")
        return "image/png"

    def to_image_content(self, annotations=None):
        if self.path:
            # (a file: refused when the program is built, src/caps.rs)
            raise NotImplementedError("brevi: Image(path=...) / Audio(path=...) are not in Brevi's mcp")
        elif self.data is not None:
            data = base64.b64encode(self.data).decode()
        else:
            raise ValueError("No image data available")
        return T.ImageContent(type="image", data=data, mime_type=self._mime_type, annotations=annotations)


class Audio:
    def __init__(self, path=None, data=None, format=None):
        if not bool(path) ^ bool(data):
            raise ValueError("Either path or data can be provided")
        self.path = path
        self.data = data
        self._format = format
        self._mime_type = self._get_mime_type()

    def _get_mime_type(self):
        if self._format:
            return "audio/" + self._format.lower()
        if self.path:
            suffix = _suffix(str(self.path))
            return {".wav": "audio/wav", ".mp3": "audio/mpeg", ".ogg": "audio/ogg", ".flac": "audio/flac", ".aac": "audio/aac", ".m4a": "audio/mp4"}.get(suffix, "application/octet-stream")
        return "audio/wav"

    def to_audio_content(self, annotations=None):
        if self.path:
            # (a file: refused when the program is built, src/caps.rs)
            raise NotImplementedError("brevi: Image(path=...) / Audio(path=...) are not in Brevi's mcp")
        elif self.data is not None:
            data = base64.b64encode(self.data).decode()
        else:
            raise ValueError("No audio data available")
        return T.AudioContent(type="audio", data=data, mime_type=self._mime_type, annotations=annotations)


Image.__module__ = "mcp.server.mcpserver.utilities.types"
Audio.__module__ = "mcp.server.mcpserver.utilities.types"


def _suffix(p):
    name = p.rsplit("/", 1)[-1]
    if "." not in name.strip("."):
        return ""
    return "." + name.rsplit(".", 1)[1].lower()


# ---------------------------------------------------------------- validate_call (prompts and resource functions)


class _Validated:
    # pydantic.validate_call(fn): the arguments validated against the signature, then fn called
    def __init__(self, fn):
        self.raw = fn
        self.__name__ = getattr(fn, "__name__", "fn")
        self.__doc__ = _doc(fn)
        self.model = None

    def _model(self):
        if self.model is None:
            fields = []
            for name, d, dflt, kind in _sig(self.raw)[0]:
                if d is None:
                    d = ("any", "Any")
                fields.append((name, d, pydantic.Field(dflt) if dflt is not _NO else pydantic.Field()))
            self.model = _make_model(self.__name__, fields, _CallModel)
        return self.model

    def __call__(self, **kw):
        m = self._model()
        ctxs = {}
        data = {}
        for k, v in kw.items():
            if isinstance(v, Context):
                ctxs[k] = v
            else:
                data[k] = v
        r = m.model_validate(data)
        args = {}
        for name in type(r).model_fields:
            if name in data or name in ctxs or True:
                args[name] = getattr(r, name)
        for k in list(args):
            if k not in data and k not in ctxs and type(r).model_fields[k].default is pydantic.PydanticUndefined:
                del args[k]
        args.update(ctxs)
        return self.raw(**args)


class _CallModel(BaseModel):
    model_config = {"arbitrary_types_allowed": True}


def validate_call(fn):
    return _Validated(fn)


def _await_maybe(r):
    return r


# ---------------------------------------------------------------- tools


_TOOL_NAME = re.compile(r"^[A-Za-z0-9._-]{1,128}$")


def _validate_tool_name(name):
    # mcp.shared.tool_name_validation: warnings only
    if not name:
        _log(30, "Tool name validation warning for '': Tool name cannot be empty")
        return
    if _TOOL_NAME.match(name):
        return
    warns = []
    if len(name) > 128:
        warns.append("Tool name exceeds maximum length of 128 characters (current: %d)" % len(name))
    if " " in name:
        warns.append("Tool name contains spaces, which may cause parsing issues")
    if "," in name:
        warns.append("Tool name contains commas, which may cause parsing issues")
    if name.startswith("-") or name.endswith("-"):
        warns.append("Tool name starts or ends with a dash, which may cause parsing issues in some contexts")
    if name.startswith(".") or name.endswith("."):
        warns.append("Tool name starts or ends with a dot, which may cause parsing issues in some contexts")
    bad = []
    for c in name:
        if not (c.isascii() and (c.isalnum() or c in "._-")) and c not in bad:
            bad.append(c)
    if bad:
        warns.append("Tool name contains invalid characters: %s" % ", ".join('"%s"' % c for c in bad))
        warns.append("Allowed characters are: A-Z, a-z, 0-9, underscore (_), dash (-), and dot (.)")
    if warns:
        _log(30, "Tool name validation warning for %r:" % name)
        for w in warns:
            _log(30, "  - %s" % w)
        _log(30, "Tool registration will proceed, but this may cause compatibility issues.")
        _log(30, "Consider updating the tool name to conform to the MCP tool naming standard.")
        _log(30, "See SEP-986: https://github.com/modelcontextprotocol/modelcontextprotocol/issues/986")


class Tool:
    def __init__(self, fn, name, title, description, parameters, fn_metadata, is_async, context_kwarg, annotations=None, icons=None, meta=None):
        self.fn = fn
        self.name = name
        self.title = title
        self.description = description
        self.parameters = parameters
        self.fn_metadata = fn_metadata
        self.is_async = is_async
        self.context_kwarg = context_kwarg
        self.annotations = annotations
        self.icons = icons
        self.meta = meta

    @property
    def output_schema(self):
        return self.fn_metadata.output_schema

    @classmethod
    def from_function(cls, fn, name=None, title=None, description=None, context_kwarg=None, annotations=None, icons=None, meta=None,
                      structured_output=None):
        func_name = name or fn.__name__
        _validate_tool_name(func_name)
        if func_name == "<lambda>":
            raise ValueError("You must provide a name for lambda functions")
        doc = description or _doc(fn) or ""
        is_async = _is_async(fn)
        if context_kwarg is None:
            context_kwarg = find_context_parameter(fn)
        skip = [context_kwarg] if context_kwarg is not None else []
        md = func_metadata(fn, skip_names=skip, structured_output=structured_output)
        params = _args_schema(md)
        return cls(fn, func_name, title, doc, params, md, is_async, context_kwarg, annotations, icons, meta)

    async def run(self, arguments, context, convert_result=False):
        try:
            validated = self.fn_metadata.validate_arguments(arguments)
        except ValidationError as exc:
            raise ToolError("Error executing tool %s: %s" % (self.name, exc)) from exc
        except MCPError:
            raise
        except Exception as exc:
            raise UnexpectedToolError("Error executing tool %s" % self.name) from exc
        try:
            direct = {}
            if self.context_kwarg is not None:
                direct[self.context_kwarg] = context
            result = await self.fn_metadata.call_fn(self.fn, self.is_async, validated, direct)
            if convert_result:
                result = self.fn_metadata.convert_result(result)
            return result
        except MCPError:
            raise
        except (UnexpectedToolError, UnexpectedResourceError) as exc:
            raise UnexpectedToolError("Error executing tool %s: %s" % (self.name, exc)) from exc
        except (ToolError, ResourceError) as exc:
            raise ToolError("Error executing tool %s: %s" % (self.name, exc)) from exc
        except Exception as exc:
            raise UnexpectedToolError("Error executing tool %s" % self.name) from exc


Tool.__module__ = "mcp.server.mcpserver.tools.base"


class ToolManager:
    def __init__(self, warn_on_duplicate_tools=True, *, tools=None):
        self._tools = {}
        for t in tools or ():
            if warn_on_duplicate_tools and t.name in self._tools:
                _log(30, "Tool already exists: %s" % t.name)
            self._tools[t.name] = t
        self.warn_on_duplicate_tools = warn_on_duplicate_tools

    def get_tool(self, name):
        return self._tools.get(name)

    def list_tools(self):
        return list(self._tools.values())

    def add_tool(self, fn, name=None, title=None, description=None, annotations=None, icons=None, meta=None, structured_output=None):
        tool = Tool.from_function(fn, name=name, title=title, description=description, annotations=annotations, icons=icons, meta=meta,
                                  structured_output=structured_output)
        existing = self._tools.get(tool.name)
        if existing:
            if self.warn_on_duplicate_tools:
                _log(30, "Tool already exists: %s" % tool.name)
            return existing
        self._tools[tool.name] = tool
        return tool

    def remove_tool(self, name):
        if name not in self._tools:
            raise ToolError("Unknown tool: %s" % name)
        del self._tools[name]

    async def call_tool(self, name, arguments, context, convert_result=False):
        tool = self.get_tool(name)
        if not tool:
            raise ToolError("Unknown tool: %s" % name)
        return await tool.run(arguments, context, convert_result=convert_result)


# ---------------------------------------------------------------- URI templates (the subset of RFC 6570 the SDK matches)


_VARNAME = re.compile(r"^[A-Za-z0-9_]([A-Za-z0-9_.]|%[0-9A-Fa-f]{2})*$")


def _unquote(s):
    # urllib.parse.unquote (UTF-8, errors replaced)
    if "%" not in s:
        return s
    out = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c == "%" and i + 2 < n + 1 and _ishex(s[i + 1:i + 3]):
            out.append(int(s[i + 1:i + 3], 16))
            i += 3
        else:
            out.extend(c.encode("utf-8"))
            i += 1
    return bytes(out).decode("utf-8", "replace")


def _ishex(t):
    return len(t) == 2 and all(c in "0123456789abcdefABCDEF" for c in t)


class UriTemplate:
    def __init__(self, template, parts):
        self.template = template
        self._parts = parts
        names = []
        q = []
        for p in parts:
            if isinstance(p, tuple):
                for v in p[1]:
                    names.append(v)
                    if p[0] in ("?", "&"):
                        q.append(v)
        self.variable_names = names
        self.query_variable_names = frozenset(q)

    @classmethod
    def parse(cls, template):
        parts = []
        i = 0
        lit = []
        seen = set()
        n = len(template)
        while i < n:
            c = template[i]
            if c == "{":
                j = template.find("}", i)
                if j < 0:
                    raise InvalidUriTemplate("Unclosed expression at position %d" % i, template=template, position=i)
                body = template[i + 1:j]
                op = ""
                if body and body[0] in "+#./;?&":
                    op = body[0]
                    body = body[1:]
                if body and body[0] in "=,!@|":
                    raise InvalidUriTemplate("Reserved operator %r at position %d is not supported" % (body[0], i), template=template, position=i)
                vs = []
                for v in body.split(","):
                    if ":" in v:
                        raise InvalidUriTemplate("Prefix modifier {var:N} is not supported (in %r at position %d)" % (v, i), template=template, position=i)
                    if v.endswith("*"):
                        raise InvalidUriTemplate("Explode modifier on {%s} is not supported for matching" % (op + v), template=template, position=i)
                    if not _VARNAME.match(v):
                        raise InvalidUriTemplate("Invalid variable name %r at position %d" % (v, i), template=template, position=i)
                    if v in seen:
                        raise InvalidUriTemplate("Variable %r appears more than once; repeated variables are not supported" % v, template=template, position=i)
                    seen.add(v)
                    vs.append(v)
                if lit:
                    parts.append("".join(lit))
                    lit = []
                parts.append((op, vs))
                i = j + 1
            elif c == "}":
                raise InvalidUriTemplate("Unmatched '}' at position %d" % i, template=template, position=i)
            else:
                lit.append(c)
                i += 1
        if lit:
            parts.append("".join(lit))
        return cls(template, parts)

    def __str__(self):
        return self.template

    def match(self, uri):
        # the path part as a regular expression; {?a,b} / {&a} as the query's named parameters
        qparts = [p for p in self._parts if isinstance(p, tuple) and p[0] in ("?", "&")]
        path_parts = [p for p in self._parts if not (isinstance(p, tuple) and p[0] in ("?", "&"))]
        rx = ["^"]
        names = []
        for p in path_parts:
            if isinstance(p, str):
                rx.append(re.escape(p))
                continue
            op, vs = p
            for k, v in enumerate(vs):
                names.append(v)
                if op == "":
                    rx.append("([^/?#]*)" if k == 0 else ",([^/?#,]*)")
                elif op == "+":
                    rx.append("([^?#]*)" if k == 0 else ",([^?#,]*)")
                elif op == "#":
                    rx.append("#(.*)" if k == 0 else ",(.*)")
                elif op == ".":
                    rx.append("\\.([^/?#.]*)")
                elif op == "/":
                    rx.append("/([^/?#]*)")
                elif op == ";":
                    rx.append(";" + re.escape(v) + "(?:=([^;/?#]*))?")
        query = None
        text = uri
        if qparts:
            k = uri.find("?")
            if k >= 0:
                text, query = uri[:k], uri[k + 1:]
            rx.append("$")
        else:
            rx.append("$")
        m = re.match("".join(rx), text)
        if m is None:
            return None
        out = {}
        for k in range(len(names) - 1, -1, -1):
            g = m.group(k + 1)
            out[names[k]] = _unquote(g if g is not None else "")
        if qparts:
            want = []
            for p in qparts:
                want.extend(p[1])
            if query is not None:
                got = {}
                for pair in query.split("&"):
                    if "=" in pair:
                        a, b = pair.split("=", 1)
                    else:
                        a, b = pair, ""
                    a = _unquote(a)
                    if a in want and a not in got:
                        got[a] = _unquote(b)
                for v in want:
                    if v in got:
                        out[v] = got[v]
        return out


UriTemplate.__module__ = "mcp.shared.uri_template"


def _contains_traversal(v):
    for seg in v.replace("\\", "/").split("/"):
        if seg == "..":
            return True
    return False


def _is_absolute(v):
    if v.startswith("/") or v.startswith("\\"):
        return True
    if len(v) >= 2 and v[1] == ":" and v[0].isalpha():
        return True
    return False


class ResourceSecurity:
    def __init__(self, reject_path_traversal=True, reject_absolute_paths=True, reject_null_bytes=True, exempt_params=frozenset()):
        self.reject_path_traversal = reject_path_traversal
        self.reject_absolute_paths = reject_absolute_paths
        self.reject_null_bytes = reject_null_bytes
        self.exempt_params = frozenset(exempt_params)

    def validate(self, params):
        for name, value in params.items():
            if name in self.exempt_params:
                continue
            vals = value if isinstance(value, list) else [value]
            for v in vals:
                if self.reject_null_bytes and "\0" in v:
                    return name
                if self.reject_path_traversal and _contains_traversal(v):
                    return name
                if self.reject_absolute_paths and _is_absolute(v):
                    return name
        return None

    def __repr__(self):
        return "ResourceSecurity(reject_path_traversal=%r, reject_absolute_paths=%r, reject_null_bytes=%r, exempt_params=%r)" % (
            self.reject_path_traversal, self.reject_absolute_paths, self.reject_null_bytes, self.exempt_params)


ResourceSecurity.__module__ = "mcp.server.mcpserver.resources.templates"
DEFAULT_RESOURCE_SECURITY = ResourceSecurity()


class ResourceSecurityError(ValueError):
    def __init__(self, template, param):
        super().__init__("Parameter %r of template %r failed security validation" % (param, template))
        self.template = template
        self.param = param


# ---------------------------------------------------------------- resources


class Resource:
    def __init__(self, uri, name=None, title=None, description=None, mime_type="text/plain", icons=None, annotations=None, meta=None):
        self.uri = str(uri)
        if not name:
            name = self.uri
        self.name = name
        self.title = title
        self.description = description
        self.mime_type = mime_type
        self.icons = icons
        self.annotations = annotations
        self.meta = meta

    async def read(self):
        raise NotImplementedError


Resource.__module__ = "mcp.server.mcpserver.resources.base"


async def _call(fn, kw=None):
    r = fn(**(kw or {}))
    if _brevi_async.iscoroutine(r):
        r = await r
    return r


class FunctionResource(Resource):
    def __init__(self, uri, fn, name=None, title=None, description=None, mime_type="text/plain", icons=None, annotations=None, meta=None):
        Resource.__init__(self, uri, name, title, description, mime_type, icons, annotations, meta)
        self.fn = fn

    async def read(self):
        result = await _call(self.fn)
        if isinstance(result, T.InputRequiredResult):
            raise ValueError("static resources cannot return InputRequiredResult; only resource template functions participate in the multi-round-trip flow")
        if isinstance(result, Resource):
            return await result.read()
        if isinstance(result, bytes):
            return result
        if isinstance(result, str):
            return result
        return _to_json_text(result, 2)

    @classmethod
    def from_function(cls, fn, uri, name=None, title=None, description=None, mime_type=None, icons=None, annotations=None, meta=None):
        func_name = name or fn.__name__
        if func_name == "<lambda>":
            raise ValueError("You must provide a name for lambda functions")
        v = validate_call(fn)
        return cls(uri, v, name=func_name, title=title, description=description or _doc(fn) or "", mime_type=mime_type or "text/plain",
                   icons=icons, annotations=annotations, meta=meta)


FunctionResource.__module__ = "mcp.server.mcpserver.resources.types"


class TextResource(Resource):
    def __init__(self, uri, text, name=None, title=None, description=None, mime_type="text/plain", icons=None, annotations=None, meta=None):
        Resource.__init__(self, uri, name, title, description, mime_type, icons, annotations, meta)
        self.text = text

    async def read(self):
        return self.text


class BinaryResource(Resource):
    def __init__(self, uri, data, name=None, title=None, description=None, mime_type="text/plain", icons=None, annotations=None, meta=None):
        Resource.__init__(self, uri, name, title, description, mime_type, icons, annotations, meta)
        self.data = data

    async def read(self):
        return self.data


for _c in (TextResource, BinaryResource):
    _c.__module__ = "mcp.server.mcpserver.resources.types"


class ResourceTemplate:
    def __init__(self, uri_template, name, title, description, mime_type, icons, annotations, meta, fn, parameters, context_kwarg, parsed, security):
        self.uri_template = uri_template
        self.name = name
        self.title = title
        self.description = description
        self.mime_type = mime_type
        self.icons = icons
        self.annotations = annotations
        self.meta = meta
        self.fn = fn
        self.parameters = parameters
        self.context_kwarg = context_kwarg
        self.parsed_template = parsed
        self.security = security

    @classmethod
    def from_function(cls, fn, uri_template, name=None, title=None, description=None, mime_type=None, icons=None, annotations=None, meta=None,
                      context_kwarg=None, security=DEFAULT_RESOURCE_SECURITY):
        func_name = name or fn.__name__
        if func_name == "<lambda>":
            raise ValueError("You must provide a name for lambda functions")
        parsed = UriTemplate.parse(uri_template)
        if context_kwarg is None:
            context_kwarg = find_context_parameter(fn)
        md = func_metadata(fn, skip_names=[context_kwarg] if context_kwarg is not None else [], structured_output=False)
        params = _args_schema(md)
        v = validate_call(fn)
        return cls(uri_template, func_name, title, description or _doc(fn) or "", mime_type or "text/plain", icons, annotations, meta, v,
                   params, context_kwarg, parsed, security)

    def matches(self, uri):
        params = self.parsed_template.match(uri)
        if params is None:
            return None
        failed = self.security.validate(params)
        if failed is not None:
            raise ResourceSecurityError(self.uri_template, failed)
        return params

    async def create_resource(self, uri, params, context):
        try:
            kw = dict(params)
            if self.context_kwarg is not None and context is not None:
                kw[self.context_kwarg] = context
            result = await _call(self.fn, kw)
            if isinstance(result, T.InputRequiredResult):
                return result
            return FunctionResource(uri, lambda: result, name=self.name, title=self.title, description=self.description, mime_type=self.mime_type,
                                    icons=self.icons, annotations=self.annotations, meta=self.meta)
        except (ResourceError, MCPError):
            raise
        except Exception as exc:
            raise UnexpectedResourceError("Error creating resource from template %s" % uri) from exc


ResourceTemplate.__module__ = "mcp.server.mcpserver.resources.templates"


class ResourceManager:
    def __init__(self, warn_on_duplicate_resources=True, *, resources=None):
        self._resources = {}
        self._templates = {}
        self.warn_on_duplicate_resources = warn_on_duplicate_resources
        for r in resources or ():
            self.add_resource(r)

    def add_resource(self, resource):
        existing = self._resources.get(str(resource.uri))
        if existing:
            if self.warn_on_duplicate_resources:
                _log(30, "Resource already exists: %s" % resource.uri)
            return existing
        self._resources[str(resource.uri)] = resource
        return resource

    def add_template(self, fn, uri_template, name=None, title=None, description=None, mime_type=None, icons=None, annotations=None, meta=None,
                     security=DEFAULT_RESOURCE_SECURITY):
        t = ResourceTemplate.from_function(fn, uri_template=uri_template, name=name, title=title, description=description, mime_type=mime_type,
                                           icons=icons, annotations=annotations, meta=meta, security=security)
        self._templates[t.uri_template] = t
        return t

    async def get_resource(self, uri, context):
        uri_str = str(uri)
        r = self._resources.get(uri_str)
        if r:
            return r
        for t in self._templates.values():
            try:
                params = t.matches(uri_str)
            except ResourceSecurityError as e:
                raise ResourceNotFoundError("Unknown resource: %s" % uri) from e
            if params is not None:
                return await t.create_resource(uri_str, params, context=context)
        raise ResourceNotFoundError("Unknown resource: %s" % uri)

    def list_resources(self):
        return list(self._resources.values())

    def list_templates(self):
        return list(self._templates.values())


class ReadResourceContents:
    def __init__(self, content, mime_type=None, meta=None):
        self.content = content
        self.mime_type = mime_type
        self.meta = meta


ReadResourceContents.__module__ = "mcp.server.lowlevel.helper_types"


# ---------------------------------------------------------------- prompts


class Message(BaseModel):
    def __init__(self, content, **kw):
        if isinstance(content, str):
            content = T.TextContent(type="text", text=content)
        elif isinstance(content, Image):
            content = content.to_image_content()
        elif isinstance(content, Audio):
            content = content.to_audio_content()
        super().__init__(content=content, **kw)

    role = pydantic.Field()
    content = pydantic.Field()
    __annotations__ = (("role", ("literal", ("user", "assistant"))),
                       ("content", ("union", tuple(("cls", c) for c in T.ContentBlock))))


class UserMessage(Message):
    role = pydantic.Field("user")
    __annotations__ = (("role", ("literal", ("user", "assistant"))),)


class AssistantMessage(Message):
    role = pydantic.Field("assistant")
    __annotations__ = (("role", ("literal", ("user", "assistant"))),)


for _c in (Message, UserMessage, AssistantMessage):
    _c.__module__ = "mcp.server.mcpserver.prompts.base"


def _message_from_dict(d):
    # TypeAdapter(UserMessage | AssistantMessage, left_to_right).validate_python: the classes have their own
    # __init__, which pydantic calls with the dict's items
    try:
        return UserMessage(**d)
    except ValidationError as e1:
        try:
            return AssistantMessage(**d)
        except ValidationError:
            raise e1


class PromptArgument:
    def __init__(self, name, description=None, required=False):
        self.name = name
        self.description = description
        self.required = required


class Prompt:
    def __init__(self, name, title=None, description=None, arguments=None, fn=None, icons=None, context_kwarg=None):
        self.name = name
        self.title = title
        self.description = description
        self.arguments = arguments
        self.fn = fn
        self.icons = icons
        self.context_kwarg = context_kwarg

    @classmethod
    def from_function(cls, fn, name=None, title=None, description=None, icons=None, context_kwarg=None):
        func_name = name or fn.__name__
        if func_name == "<lambda>":
            raise ValueError("You must provide a name for lambda functions")
        if context_kwarg is None:
            context_kwarg = find_context_parameter(fn)
        md = func_metadata(fn, skip_names=[context_kwarg] if context_kwarg is not None else [], structured_output=False)
        params = _args_schema_plain(md)
        args = []
        if "properties" in params:
            for pn, p in params["properties"].items():
                args.append(PromptArgument(pn, p.get("description"), pn in params.get("required", [])))
        v = validate_call(fn)
        return cls(func_name, title, description or _doc(fn) or "", args, v, icons, context_kwarg)

    async def render(self, arguments, context):
        if self.arguments:
            required = {a.name for a in self.arguments if a.required}
            provided = set(arguments or {})
            missing = required - provided
            if missing:
                raise ValueError("Missing required arguments: %s" % (_set_repr(missing),))
        try:
            kw = dict(arguments or {})
            if self.context_kwarg is not None and context is not None:
                kw[self.context_kwarg] = context
            result = await _call(self.fn, kw)
            if isinstance(result, T.InputRequiredResult):
                return result
            if not isinstance(result, (list, tuple)):
                result = [result]
            msgs = []
            for m in result:
                if isinstance(m, Message):
                    msgs.append(m)
                elif isinstance(m, dict):
                    msgs.append(_message_from_dict(m))
                elif isinstance(m, str) or isinstance(m, T.ContentBlock) or isinstance(m, (Image, Audio)):
                    msgs.append(UserMessage(m))
                else:
                    msgs.append(Message(role="user", content=_to_json_text(m, 2)))
            return msgs
        except MCPError:
            raise
        except Exception as exc:
            raise ValueError("Error rendering prompt %s" % self.name) from exc


Prompt.__module__ = "mcp.server.mcpserver.prompts.base"
PromptArgument.__module__ = "mcp.server.mcpserver.prompts.base"


def _set_repr(s):
    # a set's repr in CPython's order: by hash (strings hash differently per run; sorted is as good as any)
    items = sorted(s)
    return "{" + ", ".join(repr(x) for x in items) + "}"


def _args_schema_plain(md):
    return _args_schema(md)


class PromptManager:
    def __init__(self, warn_on_duplicate_prompts=True):
        self._prompts = {}
        self.warn_on_duplicate_prompts = warn_on_duplicate_prompts

    def get_prompt(self, name):
        return self._prompts.get(name)

    def list_prompts(self):
        return list(self._prompts.values())

    def add_prompt(self, prompt):
        existing = self._prompts.get(prompt.name)
        if existing:
            if self.warn_on_duplicate_prompts:
                _log(30, "Prompt already exists: %s" % prompt.name)
            return existing
        self._prompts[prompt.name] = prompt
        return prompt

    def remove_prompt(self, name):
        if name not in self._prompts:
            raise ValueError("Unknown prompt: %s" % name)
        del self._prompts[name]


# ---------------------------------------------------------------- versions and keys (mcp_types.version, mcp_types)


HANDSHAKE_PROTOCOL_VERSIONS = ("2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25")
MODERN_PROTOCOL_VERSIONS = ("2026-07-28",)
KNOWN_PROTOCOL_VERSIONS = HANDSHAKE_PROTOCOL_VERSIONS + MODERN_PROTOCOL_VERSIONS
LATEST_HANDSHAKE_VERSION = "2025-11-25"
LATEST_MODERN_VERSION = "2026-07-28"
LATEST_PROTOCOL_VERSION = LATEST_HANDSHAKE_VERSION
PROTOCOL_VERSION_META_KEY = "io.modelcontextprotocol/protocolVersion"
CLIENT_INFO_META_KEY = "io.modelcontextprotocol/clientInfo"
CLIENT_CAPABILITIES_META_KEY = "io.modelcontextprotocol/clientCapabilities"
SERVER_INFO_META_KEY = "io.modelcontextprotocol/serverInfo"
LOG_LEVEL_META_KEY = "io.modelcontextprotocol/logLevel"
CONNECTION_CLOSED = -32000
REQUEST_TIMEOUT = -32001
HEADER_MISMATCH = -32020
MISSING_REQUIRED_CLIENT_CAPABILITY = -32021
UNSUPPORTED_PROTOCOL_VERSION = -32022
URL_ELICITATION_REQUIRED = -32042
CORE_RESULT_TYPES = ("complete", "input_required")
_LOG_LEVELS = ("debug", "info", "notice", "warning", "error", "critical", "alert", "emergency")
_INIT_EXEMPT = ("ping",)


class _InvalidParams(Exception):
    # a request's params that do not fit the version's wire request (pydantic's ValidationError in the SDK)
    pass


def _error_of(exc):
    # jsonrpc_dispatcher.handler_exception_to_error_data: the reply for a handler's exception, None for others
    if isinstance(exc, MCPError):
        return exc.error
    if isinstance(exc, (ValidationError, _InvalidParams)):
        return T.ErrorData(code=INVALID_PARAMS, message="Invalid request parameters", data="")
    return None


def _error_wire(err):
    out = {"code": err.code, "message": err.message}
    if "data" in err.model_fields_set:
        out["data"] = _jsonable(err.data)
    return out


def _allowed_log_levels(version, meta):
    if version not in MODERN_PROTOCOL_VERSIONS:
        return _LOG_LEVELS
    req = (meta or {}).get(LOG_LEVEL_META_KEY)
    if req not in _LOG_LEVELS:
        return ()
    return _LOG_LEVELS[_LOG_LEVELS.index(req):]


def _dump_result(result):
    if result is None:
        return {}
    if isinstance(result, T.ErrorData):
        raise MCPError.from_error_data(result)
    if isinstance(result, BaseModel):
        return _dump(result)
    if isinstance(result, dict):
        return dict(result)
    raise TypeError("handler returned %s; expected BaseModel, dict, or None" % type(result).__name__)


def _extract_meta(params):
    if not params or "_meta" not in params:
        return None
    m = params.get("_meta")
    return m if isinstance(m, dict) else None


# ---------------------------------------------------------------- the connection, the session and the request context


class _Event:
    def __init__(self):
        self._set = False

    def set(self):
        self._set = True

    def is_set(self):
        return self._set


class Connection:
    def __init__(self, outbound, *, protocol_version, session_id=None, client_params=None):
        self.outbound = outbound
        self.protocol_version = protocol_version
        self.session_id = session_id
        self.client_capabilities = None
        self._client_params = None
        self.client_params = client_params
        self.initialized = _Event()
        self.state = {}

    @property
    def client_params(self):
        return self._client_params

    @client_params.setter
    def client_params(self, value):
        self._client_params = value
        if value is not None:
            self.client_capabilities = value.capabilities

    @property
    def initialize_accepted(self):
        return self._client_params is not None or self.initialized.is_set()

    @classmethod
    def from_envelope(cls, protocol_version, client_info, client_capabilities, outbound=None):
        info = _typed(T.Implementation, client_info)
        caps = _typed(T.ClientCapabilities, client_capabilities)
        cp = None
        if info is not None and caps is not None:
            cp = T.InitializeRequestParams(protocol_version=protocol_version, capabilities=caps, client_info=info)
        c = cls(outbound, protocol_version=protocol_version, client_params=cp)
        c.client_capabilities = caps
        c.initialized.set()
        return c

    async def notify(self, method, params):
        if self.outbound is not None:
            await self.outbound.notify(method, params, None)

    def check_capability(self, capability):
        have = self.client_capabilities
        if have is None:
            return False
        if capability.roots is not None:
            if have.roots is None:
                return False
            if capability.roots.list_changed and not have.roots.list_changed:
                return False
        if capability.sampling is not None:
            if have.sampling is None:
                return False
            if capability.sampling.context is not None and have.sampling.context is None:
                return False
            if capability.sampling.tools is not None and have.sampling.tools is None:
                return False
        if capability.elicitation is not None and have.elicitation is None:
            return False
        if capability.experimental is not None:
            if have.experimental is None:
                return False
            for k, v in capability.experimental.items():
                if k not in have.experimental or have.experimental[k] != v:
                    return False
        if capability.extensions is not None:
            if have.extensions is None:
                return False
            for k in capability.extensions:
                if k not in have.extensions:
                    return False
        return True


def _typed(model, raw):
    try:
        return model.model_validate(raw)
    except ValidationError:
        return None


class ServerSession:
    def __init__(self, request_outbound, connection, request_meta=None):
        self._request_outbound = request_outbound
        self._connection = connection
        self._log_is_request_scoped = connection.protocol_version in MODERN_PROTOCOL_VERSIONS
        self._allowed_log_levels = _allowed_log_levels(connection.protocol_version, request_meta)

    @property
    def client_params(self):
        return self._connection.client_params

    @property
    def client_capabilities(self):
        return self._connection.client_capabilities

    @property
    def protocol_version(self):
        return self._connection.protocol_version

    @property
    def can_send_request(self):
        return False

    async def send_notification(self, notification, related_request_id=None):
        await self._notify(notification, related_request_id is not None)

    async def _notify(self, notification, request_scoped):
        data = _dump(notification)
        if request_scoped:
            await self._request_outbound.notify(data["method"], data.get("params"))
        else:
            await self._connection.notify(data["method"], data.get("params"))

    def check_client_capability(self, capability):
        return self._connection.check_capability(capability)

    async def send_log_message(self, level, data, logger=None, related_request_id=None):
        if level not in self._allowed_log_levels:
            return
        n = T.LoggingMessageNotification(params=T.LoggingMessageNotificationParams(level=level, data=data, logger=logger))
        await self._notify(n, self._log_is_request_scoped or related_request_id is not None)

    async def send_resource_updated(self, uri):
        await self.send_notification(T.ResourceUpdatedNotification(params=T.ResourceUpdatedNotificationParams(uri=str(uri))))

    async def report_progress(self, progress, total=None, message=None):
        await self._request_outbound.progress(progress, total, message)

    async def send_progress_notification(self, progress_token, progress, total=None, message=None, related_request_id=None):
        await self.send_notification(T.ProgressNotification(params=T.ProgressNotificationParams(progress_token=progress_token, progress=progress,
                                                                                               total=total, message=message)), related_request_id)

    async def send_resource_list_changed(self):
        await self.send_notification(T.ResourceListChangedNotification())

    async def send_tool_list_changed(self):
        await self.send_notification(T.ToolListChangedNotification())

    async def send_prompt_list_changed(self):
        await self.send_notification(T.PromptListChangedNotification())


ServerSession.__module__ = "mcp.server.session"


class ServerRequestContext:
    def __init__(self, session, lifespan_context, method, params, request_id, meta, protocol_version, request=None):
        self.session = session
        self.lifespan_context = lifespan_context
        self.method = method
        self.params = params
        self.request_id = request_id
        self.meta = meta
        self.protocol_version = protocol_version
        self.request = request
        self.close_sse_stream = None
        self.close_standalone_sse_stream = None


ServerRequestContext.__module__ = "mcp.server.context"


def _ctx_class_getitem(cls, item):
    return cls


# ServerRequestContext[Any, Any] in an annotation is the class itself
ServerRequestContext.__class_getitem__ = classmethod(_ctx_class_getitem)
# mcp.server.context's typing aliases, for annotations: a middleware's result and its call_next
HandlerResult = object
CallNext = object


class Context:
    # mcp.server.mcpserver.Context: what a tool, resource template or prompt function asks for by annotating a
    # parameter with it
    def __init__(self, *, request_context=None, mcp_server=None, input_params=None, subscriptions=None, **kw):
        self._request_context = request_context
        self._mcp_server = mcp_server
        self._input_params = input_params

    def __class_getitem__(cls, item):
        return cls

    def __repr__(self):
        return "Context()"

    @property
    def mcp_server(self):
        if self._mcp_server is None:
            raise ValueError("Context is not available outside of a request")
        return self._mcp_server

    @property
    def request_context(self):
        if self._request_context is None:
            raise ValueError("Context is not available outside of a request")
        return self._request_context

    def _nested(self):
        return Context(request_context=self._request_context, mcp_server=self._mcp_server)

    async def report_progress(self, progress, total=None, message=None):
        await self.request_context.session.report_progress(progress, total, message)

    async def read_resource(self, uri):
        assert self._mcp_server is not None, "Context is not available outside of a request"
        r = await self._mcp_server.read_resource(uri, self._nested())
        if isinstance(r, T.InputRequiredResult):
            raise RuntimeError("Resource returned InputRequiredResult; ctx.read_resource() only returns content — use MCPServer.read_resource(uri, context) to receive and forward it.")
        return r

    async def log(self, level, data, *, logger_name=None):
        await self.request_context.session.send_log_message(level=level, data=data, logger=logger_name, related_request_id=self.request_id)

    async def debug(self, data, *, logger_name=None):
        await self.log("debug", data, logger_name=logger_name)

    async def info(self, data, *, logger_name=None):
        await self.log("info", data, logger_name=logger_name)

    async def warning(self, data, *, logger_name=None):
        await self.log("warning", data, logger_name=logger_name)

    async def error(self, data, *, logger_name=None):
        await self.log("error", data, logger_name=logger_name)

    @property
    def headers(self):
        return getattr(self.request_context.request, "headers", None)

    @property
    def request_id(self):
        return str(self.request_context.request_id)

    @property
    def protocol_version(self):
        return self._request_context.protocol_version if self._request_context is not None else None

    @property
    def input_responses(self):
        return self._input_params.input_responses if self._input_params else None

    @property
    def request_state(self):
        return self._input_params.request_state if self._input_params else None

    @property
    def client_capabilities(self):
        return self.request_context.session.client_capabilities

    @property
    def session(self):
        return self.request_context.session

    async def close_sse_stream(self):
        return None

    async def close_standalone_sse_stream(self):
        return None


Context.__module__ = "mcp.server.mcpserver.context"


# ---------------------------------------------------------------- the low-level Server


class _Entry:
    def __init__(self, params_type, handler):
        self.params_type = params_type
        self.handler = handler


class NotificationOptions:
    def __init__(self, prompts_changed=False, resources_changed=False, tools_changed=False):
        self.prompts_changed = prompts_changed
        self.resources_changed = resources_changed
        self.tools_changed = tools_changed


NotificationOptions.__module__ = "mcp.server.lowlevel.server"


class InitializationOptions:
    def __init__(self, server_name, server_version, capabilities, title=None, description=None, instructions=None, website_url=None, icons=None):
        self.server_name = server_name
        self.server_version = server_version
        self.title = title
        self.description = description
        self.capabilities = capabilities
        self.instructions = instructions
        self.website_url = website_url
        self.icons = icons


InitializationOptions.__module__ = "mcp.server.models"


class _Lifespan:
    # the default lifespan: an async context manager that gives {}
    def __init__(self, server):
        pass

    async def __aenter__(self):
        return {}

    async def __aexit__(self, a, b, c):
        return False


async def _ping_handler(ctx, params):
    return T.EmptyResult()


class Server:
    def __init__(self, name, *, version="", title=None, description=None, instructions=None, website_url=None, icons=None, cache_hints=None,
                 lifespan=None, get_tool_input_schema=None, on_list_tools=None, on_call_tool=None, on_list_resources=None,
                 on_list_resource_templates=None, on_read_resource=None, on_subscribe_resource=None, on_unsubscribe_resource=None,
                 on_subscriptions_listen=None, on_list_prompts=None, on_get_prompt=None, on_completion=None, on_set_logging_level=None,
                 on_ping=_ping_handler, on_roots_list_changed=None, on_progress=None):
        if on_set_logging_level is not None:
            _warn_dep("The logging capability is deprecated as of 2026-07-28 (SEP-2577).")
        if on_roots_list_changed is not None:
            _warn_dep("The roots capability is deprecated as of 2026-07-28 (SEP-2577).")
        if on_progress is not None:
            _warn_dep("Client-to-server progress is deprecated as of 2026-07-28.")
        if cache_hints:
            raise NotImplementedError("brevi: Server(cache_hints=...) is not in Brevi's mcp")
        self.name = name
        self.version = version
        self.title = title
        self.description = description
        self.instructions = instructions
        self.website_url = website_url
        self.icons = icons
        self.cache_hints = {}
        self.lifespan = lifespan if lifespan is not None else _Lifespan
        self.get_tool_input_schema = get_tool_input_schema
        self._request_handlers = {}
        self._notification_handlers = {}
        self.middleware = []
        self.extensions = {}
        spec = [
            ("ping", T.RequestParams, on_ping),
            ("server/discover", T.RequestParams, self._handle_discover),
            ("prompts/list", T.PaginatedRequestParams, on_list_prompts),
            ("prompts/get", T.GetPromptRequestParams, on_get_prompt),
            ("resources/list", T.PaginatedRequestParams, on_list_resources),
            ("resources/templates/list", T.PaginatedRequestParams, on_list_resource_templates),
            ("resources/read", T.ReadResourceRequestParams, on_read_resource),
            ("resources/subscribe", T.SubscribeRequestParams, on_subscribe_resource),
            ("resources/unsubscribe", T.UnsubscribeRequestParams, on_unsubscribe_resource),
            ("subscriptions/listen", T.SubscriptionsListenRequestParams, on_subscriptions_listen),
            ("tools/list", T.PaginatedRequestParams, on_list_tools),
            ("tools/call", T.CallToolRequestParams, on_call_tool),
            ("logging/setLevel", T.SetLevelRequestParams, on_set_logging_level),
            ("completion/complete", T.CompleteRequestParams, on_completion),
        ]
        for m, pt, h in spec:
            if h is not None:
                self._request_handlers[m] = _Entry(pt, h)
        for m, pt, h in (("notifications/roots/list_changed", T.NotificationParams, on_roots_list_changed),
                         ("notifications/progress", T.ProgressNotificationParams, on_progress)):
            if h is not None:
                self._notification_handlers[m] = _Entry(pt, h)
        self._stamp = None

    def add_request_handler(self, method, params_type, handler):
        if method == "initialize":
            raise ValueError("'initialize' is handled by the server runner and cannot be overridden; use Server.middleware to observe or wrap initialization")
        self._request_handlers[method] = _Entry(params_type, handler)

    def add_notification_handler(self, method, params_type, handler):
        self._notification_handlers[method] = _Entry(params_type, handler)

    def get_request_handler(self, method):
        return self._request_handlers.get(method)

    def get_notification_handler(self, method):
        return self._notification_handlers.get(method)

    def create_initialization_options(self, notification_options=None, experimental_capabilities=None, extensions=None):
        return InitializationOptions(server_name=self.name, server_version=self.version, title=self.title, description=self.description,
                                     capabilities=self.get_capabilities(notification_options or NotificationOptions(), experimental_capabilities,
                                                                        extensions if extensions is not None else self.extensions),
                                     instructions=self.instructions, website_url=self.website_url, icons=self.icons)

    def get_capabilities(self, notification_options=None, experimental_capabilities=None, extensions=None, *, protocol_version=None):
        no = notification_options or NotificationOptions()
        prompts = resources = tools = logging_ = completions = None
        if protocol_version in MODERN_PROTOCOL_VERSIONS:
            listen = "subscriptions/listen" in self._request_handlers
            pc = tc = rc = sub = listen
        else:
            pc = no.prompts_changed
            tc = no.tools_changed
            rc = no.resources_changed
            sub = "resources/subscribe" in self._request_handlers
        if "prompts/list" in self._request_handlers:
            prompts = T.PromptsCapability(list_changed=pc)
        if "resources/list" in self._request_handlers:
            resources = T.ResourcesCapability(subscribe=sub, list_changed=rc)
        if "tools/list" in self._request_handlers:
            tools = T.ToolsCapability(list_changed=tc)
        if "logging/setLevel" in self._request_handlers:
            logging_ = T.LoggingCapability()
        if "completion/complete" in self._request_handlers:
            completions = T.CompletionsCapability()
        return T.ServerCapabilities(prompts=prompts, resources=resources, tools=tools, logging=logging_, experimental=experimental_capabilities,
                                    extensions=extensions if extensions is not None else (self.extensions or None), completions=completions)

    @property
    def server_info(self):
        return T.Implementation(name=self.name, version=self.version, title=self.title, description=self.description, website_url=self.website_url,
                                icons=self.icons)

    @property
    def server_info_stamp(self):
        if self._stamp is None:
            self._stamp = _dump(self.server_info)
        return json.loads(json.dumps(self._stamp))

    async def _handle_discover(self, ctx, params):
        return T.DiscoverResult(supported_versions=list(MODERN_PROTOCOL_VERSIONS), capabilities=self.get_capabilities(protocol_version=ctx.protocol_version),
                                instructions=self.instructions)

    async def run(self, read_stream, write_stream, initialization_options=None, raise_exceptions=False):
        async with self.lifespan(self) as state:
            await _serve_dual_era(self, read_stream, write_stream, state, initialization_options, raise_exceptions)


Server.__module__ = "mcp.server.lowlevel.server"


# ---------------------------------------------------------------- the runner (mcp.server.runner)


def _bind_middleware(mw, call_next):
    # one middleware in the CallNext shape: ctx taken at call time, so a rewritten context flows down the chain
    def call(ctx):
        return mw(ctx, call_next)
    return call


class _Runner:
    def __init__(self, server, connection, lifespan_state, init_options=None):
        self.server = server
        self.connection = connection
        self.lifespan_state = lifespan_state
        self.init_options = init_options

    async def on_request(self, dctx, method, params):
        meta = _extract_meta(params)
        version = self.connection.protocol_version
        ctx = self._make_context(dctx, method, params, meta, version)

        async def inner(ctx):
            # method and params off ctx, so a middleware's rewrite reaches lookup and the handler
            method, params = ctx.method, ctx.params
            if method in _spec_methods():
                try:
                    ok = _request_ok(method, version, params)
                except KeyError:
                    raise MCPError(METHOD_NOT_FOUND, "Method not found", method)
                if not ok:
                    raise _InvalidParams()
            if method == "initialize":
                return self._serialize(method, version, self._handle_initialize(params))
            entry = self.server.get_request_handler(method)
            if entry is None:
                raise MCPError(METHOD_NOT_FOUND, "Method not found", method)
            if not self.connection.initialize_accepted and method not in _INIT_EXEMPT:
                raise MCPError(INVALID_PARAMS, "Invalid request parameters", "")
            typed = entry.params_type.model_validate({} if params is None else params)
            r = await entry.handler(ctx, typed)
            if isinstance(r, T.ErrorData):
                raise MCPError.from_error_data(r)
            return self._serialize(method, version, r)

        result = _dump_result(await self._chain(inner)(ctx))
        if method == "initialize":
            self.connection.client_params, self.connection.protocol_version = self._negotiate(params)
        return result

    def _chain(self, inner):
        # Server.middleware around inner, outermost first (ServerRunner._compose_server_middleware)
        call = inner
        for mw in reversed(self.server.middleware):
            call = _bind_middleware(mw, call)
        return call

    async def on_notify(self, dctx, method, params):
        meta = _extract_meta(params)
        version = self.connection.protocol_version
        ctx = self._make_context(dctx, method, params, meta, version)

        async def inner(ctx):
            method, params = ctx.method, ctx.params
            surf = _surface("client_notifications").get(method)
            if surf is not None:
                if version not in surf:
                    return None
                era = "m" if version in MODERN_PROTOCOL_VERSIONS else "h"
                body = {"jsonrpc": "2.0", "method": method}
                if params is not None:
                    body["params"] = params
                if not _fits(("M", surf[version]), body, _wire(era)):
                    _log(30, "dropped %r: malformed params", method)
                    return None
            if method == "notifications/initialized":
                self.connection.initialized.set()
            elif not self.connection.initialize_accepted:
                return None
            entry = self.server.get_notification_handler(method)
            if entry is None:
                return None
            try:
                typed = entry.params_type.model_validate({} if params is None else params)
            except ValidationError:
                _log(30, "dropped %r: malformed params", method)
                return None
            await entry.handler(ctx, typed)
            return None

        try:
            await self._chain(inner)(ctx)
        except Exception as e:
            _log_exc(e, "notification handler for %r raised", method)

    def _make_context(self, dctx, method, params, meta, version):
        session = ServerSession(dctx, self.connection, meta if dctx.request_id is not None else None)
        return ServerRequestContext(session, self.lifespan_state, method, params, dctx.request_id, meta, version)

    def _serialize(self, method, version, result):
        dumped = _dump_result(result)
        rt = dumped.get("resultType")
        core = version not in MODERN_PROTOCOL_VERSIONS or not isinstance(rt, str) or rt in CORE_RESULT_TYPES
        if method in _spec_methods() and core:
            try:
                dumped = _sieve(method, version, dumped)
            except _Bad:
                _log_exc(_why_bad(method, version, dumped), "handler for %r returned an invalid result", method)
                raise MCPError(INTERNAL_ERROR, "Handler returned an invalid result")
        if version in MODERN_PROTOCOL_VERSIONS and dumped.get("resultType") is None:
            dumped["resultType"] = "complete"
        if version in MODERN_PROTOCOL_VERSIONS:
            m = dumped.get("_meta")
            if m is None:
                dumped["_meta"] = {SERVER_INFO_META_KEY: self.server.server_info_stamp}
            elif isinstance(m, dict) and m.get(SERVER_INFO_META_KEY) is None:
                m2 = dict(m)
                m2[SERVER_INFO_META_KEY] = self.server.server_info_stamp
                dumped["_meta"] = m2
        return dumped

    @staticmethod
    def _negotiate(params):
        init = T.InitializeRequestParams.model_validate(params or {})
        req = init.protocol_version
        return init, (req if req in HANDSHAKE_PROTOCOL_VERSIONS else LATEST_HANDSHAKE_VERSION)

    def _handle_initialize(self, params):
        _, negotiated = self._negotiate(params)
        o = self.init_options if self.init_options is not None else self.server.create_initialization_options()
        return T.InitializeResult(protocol_version=negotiated, capabilities=o.capabilities,
                                  server_info=T.Implementation(name=o.server_name, title=o.title, description=o.description, version=o.server_version,
                                                               website_url=o.website_url, icons=o.icons),
                                  instructions=o.instructions)


# ---------------------------------------------------------------- the JSON-RPC dispatcher (mcp.shared.jsonrpc_dispatcher)


class _Msg:
    # a message off the wire: kind "request", "notification", "response" or "error"
    def __init__(self, kind, id, method, params, result=None, error=None):
        self.kind = kind
        self.id = id
        self.method = method
        self.params = params
        self.result = result
        self.error = error


def _is_rid(v):
    # RequestId = Annotated[int, Field(strict=True)] | str
    return (type(v) is int) or isinstance(v, str)


def _classify(v):
    # jsonrpc_message_adapter.validate_json: the union of request, notification, response and error, as
    # pydantic's smart mode picks among them (the one most fields fit; the first of a tie); None when none fits
    if not isinstance(v, dict) or v.get("jsonrpc") != "2.0" or not isinstance(v.get("jsonrpc"), str):
        return None
    params = v.get("params")
    params_ok = params is None or isinstance(params, dict)
    method = v.get("method")
    has_id = "id" in v
    cands = []
    if has_id and _is_rid(v["id"]) and isinstance(method, str) and params_ok:
        cands.append((3 + ("params" in v), 0, _Msg("request", v["id"], method, params)))
    if isinstance(method, str) and params_ok:
        cands.append((2 + ("params" in v), 1, _Msg("notification", None, method, params)))
    if has_id and _is_rid(v["id"]) and isinstance(v.get("result"), dict):
        cands.append((3, 2, _Msg("response", v["id"], None, None, result=v["result"])))
    if has_id and (v["id"] is None or _is_rid(v["id"])) and "error" in v:
        e = v["error"]
        if isinstance(e, dict) and _err_ok(e):
            cands.append((3, 3, _Msg("error", v["id"], None, None, error=e)))
    if not cands:
        return None
    best = cands[0]
    for c in cands[1:]:
        if c[0] > best[0]:
            best = c
    return best[2]


def _err_ok(e):
    c = e.get("code")
    if isinstance(c, bool):
        return False
    if isinstance(c, float):
        if c != int(c):
            return False
    elif not isinstance(c, int):
        if not (isinstance(c, str) and re.fullmatch(r"\s*[+-]?\d+\s*", c)):
            return False
    return isinstance(e.get("message"), str)


class _DispatchContext:
    def __init__(self, dispatcher, request_id, progress_token=None):
        self._dispatcher = dispatcher
        self._request_id = request_id
        self._progress_token = progress_token
        self._closed = False
        self.cancel_requested = _Event()
        self.message_metadata = None

    @property
    def request_id(self):
        return self._request_id

    @property
    def can_send_request(self):
        return False

    async def notify(self, method, params, opts=None):
        if self._closed:
            return
        await self._dispatcher.notify(method, params)

    async def progress(self, progress, total=None, message=None):
        if self._progress_token is None:
            return
        p = {"progressToken": self._progress_token, "progress": progress}
        if total is not None:
            p["total"] = total
        if message is not None:
            p["message"] = message
        await self.notify("notifications/progress", p)

    def close(self):
        self._closed = True


def _progress_token(params):
    if isinstance(params, dict):
        m = params.get("_meta")
        if isinstance(m, dict):
            t = m.get("progressToken")
            if (isinstance(t, str) or isinstance(t, int)) and not isinstance(t, bool):
                return t
    return None


def _rkey(rid):
    # coerce_request_id: an int and its decimal string are one request
    if isinstance(rid, str) and re.fullmatch(r"-?[0-9]+", rid):
        try:
            return int(rid)
        except ValueError:
            return rid
    return rid


class _Dispatcher:
    def __init__(self, read_stream, write_stream, inline_methods=(), raise_exceptions=False):
        self._read = read_stream
        self._write = write_stream
        self._inline = inline_methods
        self._raise = raise_exceptions
        self._in_flight = {}
        self._tasks = set()
        self._closed = False

    async def notify(self, method, params):
        if self._closed:
            return
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = dict(params)
        await self._write.send(msg)

    async def run(self, on_request, on_notify):
        try:
            async for item in self._read:
                await self._dispatch(item, on_request, on_notify)
        finally:
            self._closed = True
            for t in list(self._tasks):
                t.cancel()
            for t in list(self._tasks):
                try:
                    await t
                except BaseException:
                    pass

    async def _dispatch(self, item, on_request, on_notify):
        if not isinstance(item, _Msg):
            return
        if item.kind == "request":
            dctx = _DispatchContext(self, item.id, _progress_token(item.params))
            if item.method in self._inline:
                self._in_flight[_rkey(item.id)] = dctx
                await self._handle(item, dctx, on_request)
            else:
                self._in_flight[_rkey(item.id)] = dctx
                self._spawn(self._handle(item, dctx, on_request))
        elif item.kind == "notification":
            if item.method == "notifications/cancelled":
                rid = (item.params or {}).get("requestId")
                if rid is not None and _is_rid(rid):
                    d = self._in_flight.get(_rkey(rid))
                    if d is not None:
                        d.cancel_requested.set()
                        t = getattr(d, "_task", None)
                        if t is not None:
                            t.cancel()
            dctx = _DispatchContext(self, None)
            self._spawn(on_notify(dctx, item.method, item.params))

    def _spawn(self, coro):
        t = asyncio.ensure_future(coro)
        self._tasks.add(t)
        t.add_done_callback(self._tasks.discard)
        return t

    async def _handle(self, req, dctx, on_request):
        dctx._task = asyncio.current_task()
        try:
            try:
                result = await on_request(dctx, req.method, req.params)
            finally:
                dctx.close()
                k = _rkey(req.id)
                if self._in_flight.get(k) is dctx:
                    del self._in_flight[k]
            if not dctx.cancel_requested.is_set():
                await self._write.send({"jsonrpc": "2.0", "id": req.id, "result": result})
        except asyncio.CancelledError:
            # cancelled by the peer: never answered; by the connection closing: "Connection closed"
            if not dctx.cancel_requested.is_set():
                await self._write.send({"jsonrpc": "2.0", "id": req.id, "error": {"code": CONNECTION_CLOSED, "message": "Connection closed"}})
            raise
        except Exception as e:
            try:
                err = _error_wire(_error_of(e) or self._unexpected(e, req.method))
            except Exception as e2:
                # a fault in this copy, never in the SDK: still answer, so the client is not left waiting
                err = {"code": 0, "message": "brevi: %s: %s" % (type(e2).__name__, e2)}
            if not dctx.cancel_requested.is_set():
                await self._write.send({"jsonrpc": "2.0", "id": req.id, "error": err})

    def _unexpected(self, e, method):
        _log_exc(e, "handler for %r raised", method)
        return T.ErrorData(code=0, message=str(e))


def _has_modern_envelope(params):
    if not params:
        return False
    m = params.get("_meta")
    return isinstance(m, dict) and PROTOCOL_VERSION_META_KEY in m


class _Replay:
    # the read stream again, from its first frames (the opening request and up to 8 frames before it)
    def __init__(self, lead, rest):
        self._lead = lead
        self._rest = rest

    def __aiter__(self):
        return self

    async def __anext__(self):
        if self._lead:
            return self._lead.pop(0)
        if self._rest is None:
            raise StopAsyncIteration
        return await self._rest.__anext__()


async def _serve_dual_era(server, read_stream, write_stream, state, init_options, raise_exceptions):
    try:
        it = read_stream.__aiter__()
        lead = []
        opening = None
        while True:
            try:
                item = await it.__anext__()
            except StopAsyncIteration:
                it = None
                break
            if isinstance(item, _Msg) and item.kind == "request":
                opening = item
            elif len(lead) >= 8:
                continue
            lead.append(item)
            if opening is not None:
                break
        replayed = _Replay(lead, it)
        if opening is not None and opening.method != "initialize" and _has_modern_envelope(opening.params):
            await _serve_modern(server, replayed, write_stream, state, raise_exceptions)
        else:
            await _serve_legacy(server, replayed, write_stream, state, init_options, raise_exceptions)
    finally:
        await write_stream.aclose()


async def _serve_legacy(server, read_stream, write_stream, state, init_options, raise_exceptions):
    d = _Dispatcher(read_stream, write_stream, ("initialize",), raise_exceptions)
    connection = Connection(d, protocol_version=LATEST_HANDSHAKE_VERSION)
    runner = _Runner(server, connection, state, init_options)

    async def on_request(dctx, method, params):
        if method != "initialize" and _has_modern_envelope(params):
            raise MCPError(INVALID_REQUEST, "this connection serves the handshake protocol era; requests carrying the 2026-07-28 envelope are not accepted on it")
        return await runner.on_request(dctx, method, params)

    await d.run(on_request, runner.on_notify)


class _NotifyOnly:
    def __init__(self, d):
        self._d = d

    async def notify(self, method, params, opts=None):
        if method in ("notifications/tools/list_changed", "notifications/prompts/list_changed", "notifications/resources/list_changed",
                      "notifications/resources/updated"):
            return
        await self._d.notify(method, params)


def _classify_inbound(method, params):
    meta = params.get("_meta") if isinstance(params, dict) else None
    if not isinstance(meta, dict):
        return MCPError(INVALID_PARAMS, "params._meta must be an object carrying the required %r and %r envelope keys" % (PROTOCOL_VERSION_META_KEY, CLIENT_CAPABILITIES_META_KEY))
    missing = [k for k in (PROTOCOL_VERSION_META_KEY, CLIENT_CAPABILITIES_META_KEY) if k not in meta]
    if missing:
        return MCPError(INVALID_PARAMS, "params._meta is missing the required envelope key(s): %s" % ", ".join(missing))
    pv = meta[PROTOCOL_VERSION_META_KEY]
    if not isinstance(pv, str):
        return MCPError(INVALID_PARAMS, "the protocol-version envelope value must be a string")
    if pv not in MODERN_PROTOCOL_VERSIONS:
        return MCPError(UNSUPPORTED_PROTOCOL_VERSION, "Unsupported protocol version", {"supported": list(MODERN_PROTOCOL_VERSIONS), "requested": pv})
    return (pv, meta.get(CLIENT_INFO_META_KEY), meta[CLIENT_CAPABILITIES_META_KEY])


async def _serve_modern(server, read_stream, write_stream, state, raise_exceptions):
    d = _Dispatcher(read_stream, write_stream, (), raise_exceptions)
    outbound = _NotifyOnly(d)

    async def on_request(dctx, method, params):
        if method == "initialize":
            req = (params or {}).get("protocolVersion")
            data = {"supported": list(MODERN_PROTOCOL_VERSIONS)}
            if isinstance(req, str):
                data = {"supported": list(MODERN_PROTOCOL_VERSIONS), "requested": req}
            raise MCPError(UNSUPPORTED_PROTOCOL_VERSION, "connection is serving the 2026-07-28 protocol; the initialize handshake is not accepted", data)
        route = _classify_inbound(method, params)
        if isinstance(route, MCPError):
            raise route
        connection = Connection.from_envelope(route[0], route[1], route[2], outbound)
        runner = _Runner(server, connection, state)
        try:
            return await runner.on_request(dctx, method, params)
        except (MCPError, ValidationError, _InvalidParams):
            raise
        except Exception as exc:
            if raise_exceptions:
                raise
            err = _error_of(exc)
            if err is None:
                _log_exc(exc, "modern request handler raised")
                err = T.ErrorData(code=INTERNAL_ERROR, message="Internal server error")
            raise MCPError(err.code, err.message, err.data)

    async def on_notify(dctx, method, params):
        connection = Connection.from_envelope(LATEST_MODERN_VERSION, None, None, outbound)
        await _Runner(server, connection, state).on_notify(dctx, method, params)

    await d.run(on_request, on_notify)


# ---------------------------------------------------------------- the stdio transport (mcp.server.stdio)


class _StdinStream:
    # lines off the claimed stdin, read as TextIOWrapper(encoding="utf-8", errors="replace") reads them
    # (universal newlines), each validated as a JSON-RPC message; an invalid line is dropped
    def __init__(self, fd):
        self._fd = fd
        self._buf = b""
        self._eof = False
        self._closed = False

    def __aiter__(self):
        return self

    async def _line(self):
        while True:
            b = self._buf
            i = 0
            n = len(b)
            while i < n:
                c = b[i]
                if c == 10:
                    self._buf = b[i + 1:]
                    return b[:i + 1]
                if c == 13:
                    if i + 1 < n:
                        if b[i + 1] == 10:
                            self._buf = b[i + 2:]
                        else:
                            self._buf = b[i + 1:]
                        return b[:i] + b"\n"
                    if self._eof:
                        self._buf = b""
                        return b[:i] + b"\n"
                    break
                i += 1
            if self._eof:
                if self._buf:
                    rest = self._buf
                    self._buf = b""
                    return rest
                return None
            await _stdio_wait(self._fd)
            data = _brevi_stdio.read(self._fd, 65536)
            if not data:
                self._eof = True
            else:
                self._buf = self._buf + data

    async def __anext__(self):
        while True:
            if self._closed:
                raise StopAsyncIteration
            raw = await self._line()
            if raw is None:
                raise StopAsyncIteration
            line = raw.decode("utf-8", "replace")
            try:
                v = pydantic._jparse(line)
            except Exception:
                continue
            m = _classify(v)
            if m is None:
                continue
            return m

    async def aclose(self):
        self._closed = True


class _StdoutStream:
    def __init__(self, fd):
        self._fd = fd
        self._closed = False

    async def send(self, msg):
        if self._closed:
            return
        _brevi_stdio.write(self._fd, (_jdump(msg) + "\n").encode("utf-8"))

    async def aclose(self):
        self._closed = True


def _stdio_wait(fd):
    loop = asyncio.get_running_loop()
    if loop._io_poll is None:
        loop._io_poll = _brevi_stdio.poll
    return loop._brevi_wait_fd(fd, 1)


class stdio_server:
    # async with stdio_server() as (read_stream, write_stream): fd 0 and 1 are the server's; print() goes to
    # stderr while it runs
    def __init__(self, stdin=None, stdout=None):
        if stdin is not None or stdout is not None:
            raise NotImplementedError("brevi: stdio_server(stdin=..., stdout=...) is not in Brevi's mcp")
        self._fds = None

    async def __aenter__(self):
        self._fds = _brevi_stdio.claim()
        return (_StdinStream(self._fds[0]), _StdoutStream(self._fds[1]))

    async def __aexit__(self, a, b, c):
        _brevi_stdio.release()
        return False


stdio_server.__module__ = "mcp.server.stdio"


# ---------------------------------------------------------------- MCPServer (mcp.server.mcpserver.server)


class _Settings:
    def __init__(self, **kw):
        for k, v in kw.items():
            setattr(self, k, v)


class _LifespanWrap:
    def __init__(self, app, fn):
        self._app = app
        self._fn = fn
        self._cm = None

    def __call__(self, server):
        return _LifespanWrap(self._app, self._fn)

    async def __aenter__(self):
        self._cm = self._fn(self._app)
        return await self._cm.__aenter__()

    async def __aexit__(self, a, b, c):
        return await self._cm.__aexit__(a, b, c)


_TRANSPORTS = ("stdio", "sse", "streamable-http")


class MCPServer:
    def __init__(self, name=None, title=None, description=None, instructions=None, website_url=None, icons=None, version="",
                 auth_server_provider=None, token_verifier=None, *, tools=None, resources=None, extensions=None, debug=False, log_level="INFO",
                 warn_on_duplicate_resources=True, warn_on_duplicate_tools=True, warn_on_duplicate_prompts=True, dependencies=None, lifespan=None,
                 auth=None, resource_security=DEFAULT_RESOURCE_SECURITY, request_state_security=None, cache_hints=None, subscriptions=None,
                 middleware=None):
        for what, v in (("auth_server_provider", auth_server_provider), ("token_verifier", token_verifier), ("extensions", extensions), ("auth", auth),
                        ("request_state_security", request_state_security), ("cache_hints", cache_hints)):
            if v:
                raise NotImplementedError("brevi: MCPServer(%s=...) is not in Brevi's mcp" % what)
        if subscriptions not in (None, False):
            raise NotImplementedError("brevi: MCPServer(subscriptions=...) is not in Brevi's mcp")
        if log_level not in ("DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"):
            raise _settings_error("log_level", log_level)
        self._resource_security = resource_security
        self.settings = _Settings(debug=debug, log_level=log_level, warn_on_duplicate_resources=warn_on_duplicate_resources,
                                  warn_on_duplicate_tools=warn_on_duplicate_tools, warn_on_duplicate_prompts=warn_on_duplicate_prompts,
                                  dependencies=list(dependencies or []), lifespan=lifespan, auth=None)
        self.dependencies = self.settings.dependencies
        self._tool_manager = ToolManager(tools=tools, warn_on_duplicate_tools=warn_on_duplicate_tools)
        self._resource_manager = ResourceManager(resources=resources, warn_on_duplicate_resources=warn_on_duplicate_resources)
        self._prompt_manager = PromptManager(warn_on_duplicate_prompts=warn_on_duplicate_prompts)
        self._lowlevel_server = Server(name or "mcp-server", title=title, description=description, instructions=instructions, website_url=website_url,
                                       icons=icons, version=version, get_tool_input_schema=self._tool_input_schema, on_list_tools=self._handle_list_tools,
                                       on_call_tool=self._handle_call_tool, on_list_resources=self._handle_list_resources,
                                       on_read_resource=self._handle_read_resource, on_list_resource_templates=self._handle_list_resource_templates,
                                       on_list_prompts=self._handle_list_prompts, on_get_prompt=self._handle_get_prompt,
                                       lifespan=_LifespanWrap(self, lifespan) if lifespan else None)
        self._lowlevel_server.middleware.extend(middleware or ())
        self._custom_starlette_routes = []
        configure_logging(log_level)

    @property
    def name(self):
        return self._lowlevel_server.name

    @property
    def middleware(self):
        return self._lowlevel_server.middleware

    @property
    def title(self):
        return self._lowlevel_server.title

    @property
    def description(self):
        return self._lowlevel_server.description

    @property
    def instructions(self):
        return self._lowlevel_server.instructions

    @property
    def website_url(self):
        return self._lowlevel_server.website_url

    @property
    def icons(self):
        return self._lowlevel_server.icons

    @property
    def version(self):
        return self._lowlevel_server.version

    def run(self, transport="stdio", **kwargs):
        if transport not in _TRANSPORTS:
            raise ValueError("Unknown transport: %s" % transport)
        if transport == "stdio":
            asyncio.run(self.run_stdio_async())
        elif transport == "sse":
            asyncio.run(self.run_sse_async(**kwargs))
        else:
            asyncio.run(self.run_streamable_http_async(**kwargs))

    async def run_stdio_async(self):
        async with stdio_server() as (r, w):
            await self._lowlevel_server.run(r, w, self._lowlevel_server.create_initialization_options())

    async def run_sse_async(self, **kw):
        await _run_http(self, "sse", kw)

    async def run_streamable_http_async(self, **kw):
        await _run_http(self, "streamable-http", kw)

    async def _handle_list_tools(self, ctx, params):
        return T.ListToolsResult(tools=await self.list_tools())

    def _tool_input_schema(self, name):
        t = self._tool_manager.get_tool(name)
        return None if t is None else t.parameters

    async def _handle_call_tool(self, ctx, params):
        context = Context(request_context=ctx, mcp_server=self, input_params=params)
        try:
            return await self.call_tool(params.name, params.arguments or {}, context)
        except MCPError:
            raise
        except Exception as exc:
            if isinstance(exc, ToolError) and not isinstance(exc, UnexpectedToolError):
                if isinstance(exc.__cause__, ValidationError):
                    fields = sorted({".".join(str(p) for p in err["loc"]) for err in exc.__cause__.errors()})
                    _log(20, "Tool %r rejected arguments: %r", params.name, fields)
                else:
                    _log(20, "Tool %r failed: %r", params.name, str(exc))
            else:
                _log_exc(exc, "Tool %r raised an unexpected exception", params.name)
            return T.CallToolResult(content=[T.TextContent(type="text", text=str(exc))], is_error=True)

    async def _handle_list_resources(self, ctx, params):
        return T.ListResourcesResult(resources=await self.list_resources())

    async def _handle_read_resource(self, ctx, params):
        context = Context(request_context=ctx, mcp_server=self, input_params=params)
        try:
            results = await self.read_resource(params.uri, context)
        except ResourceError as err:
            if isinstance(err, UnexpectedResourceError):
                _log_exc(err, "Resource %r raised an unexpected exception", str(params.uri))
            else:
                _log(20, "Resource %r failed: %r", str(params.uri), str(err))
            code = INVALID_PARAMS if isinstance(err, ResourceNotFoundError) else INTERNAL_ERROR
            raise MCPError(code, str(err), {"uri": str(params.uri)})
        if isinstance(results, T.InputRequiredResult):
            return results
        contents = []
        for item in results:
            if isinstance(item.content, bytes):
                contents.append(T.BlobResourceContents(uri=params.uri, blob=base64.b64encode(item.content).decode(),
                                                       mime_type=item.mime_type or "application/octet-stream", meta=item.meta))
            else:
                contents.append(T.TextResourceContents(uri=params.uri, text=item.content, mime_type=item.mime_type or "text/plain", meta=item.meta))
        return T.ReadResourceResult(contents=contents)

    async def _handle_list_resource_templates(self, ctx, params):
        return T.ListResourceTemplatesResult(resource_templates=await self.list_resource_templates())

    async def _handle_list_prompts(self, ctx, params):
        return T.ListPromptsResult(prompts=await self.list_prompts())

    async def _handle_get_prompt(self, ctx, params):
        context = Context(request_context=ctx, mcp_server=self, input_params=params)
        return await self.get_prompt(params.name, params.arguments, context)

    async def list_tools(self):
        return [T.Tool(name=i.name, title=i.title, description=i.description, input_schema=i.parameters, output_schema=i.output_schema,
                       annotations=i.annotations, icons=i.icons, meta=i.meta) for i in self._tool_manager.list_tools()]

    async def call_tool(self, name, arguments, context=None):
        if context is None:
            context = Context(mcp_server=self)
        return await self._tool_manager.call_tool(name, arguments, context, convert_result=True)

    async def list_resources(self):
        return [T.Resource(uri=r.uri, name=r.name or "", title=r.title, description=r.description, mime_type=r.mime_type, icons=r.icons,
                           annotations=r.annotations, meta=r.meta) for r in self._resource_manager.list_resources()]

    async def list_resource_templates(self):
        return [T.ResourceTemplate(uri_template=t.uri_template, name=t.name, title=t.title, description=t.description, mime_type=t.mime_type,
                                   icons=t.icons, annotations=t.annotations, meta=t.meta) for t in self._resource_manager.list_templates()]

    async def read_resource(self, uri, context=None):
        if context is None:
            context = Context(mcp_server=self)
        try:
            resource = await self._resource_manager.get_resource(uri, context)
            if isinstance(resource, T.InputRequiredResult):
                return resource
            content = await resource.read()
            if not isinstance(content, (str, bytes)):
                raise TypeError("Resource.read() must return str or bytes, not %s" % type(content).__name__)
            return [ReadResourceContents(content=content, mime_type=resource.mime_type, meta=resource.meta)]
        except (MCPError, ResourceError):
            raise
        except Exception as exc:
            raise UnexpectedResourceError("Error reading resource %s" % uri) from exc

    def add_tool(self, fn, name=None, title=None, description=None, annotations=None, icons=None, meta=None, structured_output=None):
        self._tool_manager.add_tool(fn, name=name, title=title, description=description, annotations=annotations, icons=icons, meta=meta,
                                    structured_output=structured_output)

    def remove_tool(self, name):
        self._tool_manager.remove_tool(name)

    def tool(self, name=None, title=None, description=None, annotations=None, icons=None, meta=None, structured_output=None):
        if callable(name):
            raise TypeError("The @tool decorator was used incorrectly. Did you forget to call it? Use @tool() instead of @tool")

        def decorator(fn):
            self.add_tool(fn, name=name, title=title, description=description, annotations=annotations, icons=icons, meta=meta,
                          structured_output=structured_output)
            return fn

        return decorator

    def completion(self):
        def decorator(func):
            async def handler(ctx, params):
                try:
                    result = await func(params.ref, params.argument, params.context)
                    return T.CompleteResult(completion=result if result is not None else T.Completion(values=[], total=None, has_more=None))
                except MCPError:
                    raise
                except Exception as exc:
                    _log_exc(exc, "Completion for argument %r raised an unexpected exception", params.argument.name)
                    raise MCPError(INTERNAL_ERROR, "Error completing argument %s" % params.argument.name) from exc

            self._lowlevel_server.add_request_handler("completion/complete", T.CompleteRequestParams, handler)
            return func

        return decorator

    def add_resource(self, resource):
        self._resource_manager.add_resource(resource)

    def resource(self, uri, *, name=None, title=None, description=None, mime_type=None, icons=None, annotations=None, meta=None, security=None):
        if callable(uri):
            raise TypeError("The @resource decorator was used incorrectly. Did you forget to call it? Use @resource('uri') instead of @resource")
        parsed = UriTemplate.parse(uri)
        uri_params = set(parsed.variable_names)

        def decorator(fn):
            params = _sig(fn)[0]
            context_param = find_context_parameter(fn)
            func_params = {p[0] for p in params if p[0] != context_param}
            if uri_params:
                if uri_params != func_params:
                    raise ValueError("Mismatch between URI parameters %s and function parameters %s" % (_set_repr(uri_params), _set_repr(func_params)))
                missing = sorted(n for n in parsed.query_variable_names if _param_default(params, n) is _NO)
                if missing:
                    raise ValueError("Resource %r: query parameter(s) %r have no default value. A client may omit a {?...}/{&...} query parameter, "
                                     "so the matching handler parameter must declare a default." % (uri, missing))
                self._resource_manager.add_template(fn=fn, uri_template=uri, name=name, title=title, description=description, mime_type=mime_type,
                                                    icons=icons, annotations=annotations, security=security if security is not None else self._resource_security,
                                                    meta=meta)
            else:
                if func_params:
                    raise ValueError("Resource %r has no URI template variables, but the handler declares parameters %s. Add matching {...} variables "
                                     "to the URI or remove the parameters." % (uri, _set_repr(func_params)))
                if context_param is not None:
                    raise ValueError("Resource %r has no URI template variables, but the handler declares a Context parameter. Context injection for "
                                     "static resources is not supported. Add a template variable to the URI or remove the Context parameter." % uri)
                self.add_resource(FunctionResource.from_function(fn=fn, uri=uri, name=name, title=title, description=description, mime_type=mime_type,
                                                                 icons=icons, annotations=annotations, meta=meta))
            return fn

        return decorator

    def add_prompt(self, prompt):
        self._prompt_manager.add_prompt(prompt)

    def remove_prompt(self, name):
        self._prompt_manager.remove_prompt(name)

    def prompt(self, name=None, title=None, description=None, icons=None):
        if callable(name):
            raise TypeError("The @prompt decorator was used incorrectly. Did you forget to call it? Use @prompt() instead of @prompt")

        def decorator(func):
            self.add_prompt(Prompt.from_function(func, name=name, title=title, description=description, icons=icons))
            return func

        return decorator

    async def list_prompts(self):
        return [T.Prompt(name=p.name, title=p.title, description=p.description,
                         arguments=[T.PromptArgument(name=a.name, description=a.description, required=a.required) for a in (p.arguments or [])],
                         icons=p.icons) for p in self._prompt_manager.list_prompts()]

    async def get_prompt(self, name, arguments=None, context=None):
        if context is None:
            context = Context(mcp_server=self)
        try:
            prompt = self._prompt_manager.get_prompt(name)
            if not prompt:
                raise ValueError("Unknown prompt: %s" % name)
            rendered = await prompt.render(arguments, context)
            if isinstance(rendered, T.InputRequiredResult):
                return rendered
            return T.GetPromptResult(description=prompt.description, messages=[_dump_msg(m) for m in rendered])
        except MCPError:
            raise
        except Exception as e:
            raise ValueError(str(e)) from e


MCPServer.__module__ = "mcp.server.mcpserver.server"


def _dump_msg(m):
    # pydantic_core.to_jsonable_python(message): by field name, None kept
    return m.model_dump(mode="json")


def _param_default(params, name):
    for p in params:
        if p[0] == name:
            return p[2]
    return _NO


def _settings_error(field, value):
    return ValueError("1 validation error for Settings\n%s\n  Input should be 'DEBUG', 'INFO', 'WARNING', 'ERROR' or 'CRITICAL' [type=literal_error, input_value=%r, input_type=%s]"
                      % (field, value, type(value).__name__))


async def _run_http(app, kind, kw):
    raise NotImplementedError("brevi: the %s transport is not in Brevi's mcp yet" % kind)
