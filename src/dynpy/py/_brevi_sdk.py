# What Brevi's anthropic and openai copies share: both SDKs are written by Stainless and work alike. Here are
# their response models (BaseModel: built from the tables tools/gen_llm_types.py writes, with the SDKs' repr,
# equality, model_dump / model_dump_json / to_dict / to_json and the way they turn JSON into objects), request
# bodies as the SDKs send them, Server-Sent Events, the retry loop with its backoff, streams and pages, all over
# Brevi's own httpx (sync and async). Each SDK's copy adds its clients, resources, errors and stream helpers.
import asyncio
import datetime as _dt
import json as _json
import random as _random
import time as _time

import httpx


class NotGiven:
    def __bool__(self):
        return False

    def __repr__(self):
        return "NOT_GIVEN"


NOT_GIVEN = NotGiven()
not_given = NOT_GIVEN


class Omit:
    def __bool__(self):
        return False

    def __repr__(self):
        return "Omit()"


omit = Omit()


def is_given(v):
    return not isinstance(v, (NotGiven, Omit))


# ---------- models ----------

_CLS = {}
_REQ = ("r",)


def _default(d):
    k = d[0]
    if k == "v":
        return d[1]
    if k == "f":
        return [] if d[1] == "list" else {}
    return None


class BaseModel:
    _F = ()
    _FN = {}
    _RN = None
    # fields left out of dumps (pydantic's Field(exclude=True))
    _EX = ()

    def __init__(self, **data):
        _fill(self, data)

    @classmethod
    def construct(cls, _fields_set=None, **values):
        m = object.__new__(cls)
        _fill(m, values)
        return m

    @classmethod
    def model_construct(cls, _fields_set=None, **values):
        m = object.__new__(cls)
        _fill(m, values)
        return m

    def __getattr__(self, name):
        if name.startswith("_bv_") or name.startswith("__"):
            raise AttributeError("%r object has no attribute %r" % (type(self).__name__, name))
        ex = self._bv_extra
        if name in ex:
            return ex[name]
        raise AttributeError("%r object has no attribute %r" % (type(self).__name__, name))

    def __repr__(self):
        return _rname(self) + "(" + ", ".join(k + "=" + repr(v) for k, v in _items(self)) + ")"

    def __str__(self):
        return _rname(self) + "(" + ", ".join(k + "=" + repr(v) for k, v in _items(self)) + ")"

    def __eq__(self, other):
        if not isinstance(other, BaseModel):
            return NotImplemented
        if type(self) is not type(other):
            return False
        for f in type(self)._F:
            if getattr(self, f[0]) != getattr(other, f[0]):
                return False
        return self._bv_extra == other._bv_extra and getattr(self, "_request_id", None) == getattr(other, "_request_id", None)

    def __ne__(self, other):
        r = self.__eq__(other)
        if r is NotImplemented:
            return r
        return not r

    def __iter__(self):
        for k, v in _items(self):
            yield (k, v)

    @property
    def model_fields_set(self):
        return set(self._bv_fs)

    @property
    def model_extra(self):
        return self._bv_extra

    def model_dump(self, *, mode="python", include=None, exclude=None, context=None, by_alias=None, exclude_unset=False, exclude_defaults=False,
                   exclude_none=False, exclude_computed_fields=False, round_trip=False, warnings=True, fallback=None, serialize_as_any=False):
        return _dump(self, mode == "json", bool(by_alias), exclude_unset, exclude_defaults, exclude_none, include, exclude)

    def model_dump_json(self, *, indent=None, ensure_ascii=False, include=None, exclude=None, context=None, by_alias=None, exclude_unset=False,
                        exclude_defaults=False, exclude_none=False, exclude_computed_fields=False, round_trip=False, warnings=True, fallback=None,
                        serialize_as_any=False):
        return _jdump(_dump(self, True, bool(by_alias), exclude_unset, exclude_defaults, exclude_none, include, exclude), indent)

    def to_dict(self, *, mode="python", use_api_names=True, exclude_unset=True, exclude_defaults=False, exclude_none=False, warnings=True):
        return _dump(self, mode == "json", use_api_names, exclude_unset, exclude_defaults, exclude_none, None, None)

    def to_json(self, *, indent=2, use_api_names=True, exclude_unset=True, exclude_defaults=False, exclude_none=False, warnings=True):
        return _jdump(_dump(self, True, use_api_names, exclude_unset, exclude_defaults, exclude_none, None, None), indent)

    def model_copy(self, *, update=None, deep=False):
        m = object.__new__(type(self))
        for f in type(self)._F:
            v = getattr(self, f[0])
            setattr(m, f[0], _deep(v) if deep else v)
        m._bv_fs = list(self._bv_fs)
        m._bv_extra = dict(self._bv_extra)
        for k, v in (update or {}).items():
            if k in type(self)._FN:
                setattr(m, k, v)
                if k not in m._bv_fs:
                    m._bv_fs.append(k)
            else:
                m._bv_extra[k] = v
        return m

    def __copy__(self):
        return self.model_copy()

    def __deepcopy__(self, memo=None):
        return self.model_copy(deep=True)


def _deep(v):
    if isinstance(v, BaseModel):
        return v.model_copy(deep=True)
    if isinstance(v, list):
        return [_deep(x) for x in v]
    if isinstance(v, dict):
        return {k: _deep(x) for k, x in v.items()}
    return v


def _rname(m):
    n = type(m)._RN
    return n if n is not None else type(m).__name__


def _items(m):
    out = []
    for f in type(m)._F:
        out.append((f[0], getattr(m, f[0])))
    for k, v in m._bv_extra.items():
        out.append((k, v))
    return out


def _fill(m, values):
    cls = type(m)
    fs = []
    used = set()
    for name, alias, d, dflt in cls._F:
        key = alias if alias is not None else name
        used.add(key)
        if key in values:
            v = values[key]
            if v is None:
                v = _default(dflt)
            else:
                v = _cv(d, v)
            fs.append(name)
        else:
            v = _default(dflt)
        setattr(m, name, v)
    ex = {}
    for k in values:
        if k not in used and k not in cls._FN:
            ex[k] = values[k]
    m._bv_fs = fs
    m._bv_extra = ex


def _new(cls, values):
    m = object.__new__(cls)
    _fill(m, values)
    return m


def _cv(d, v):
    # construct_type: loose coercion to the type, building nested models, the value as it is when it does not fit
    if d is None:
        return v
    if type(d) is str:
        if d == "float":
            if type(v) is int:
                f = float(v)
                return f if f == v else v
            return v
        if d == "dt":
            try:
                return parse_datetime(v)
            except Exception:
                return v
        if d == "date":
            try:
                return parse_date(v)
            except Exception:
                return v
        return v
    k = d[0]
    if k == "M":
        cls = _CLS[d[1]]
        if isinstance(v, dict):
            return _new(cls, v)
        if isinstance(v, list):
            return [_new(cls, e) if isinstance(e, dict) else e for e in v]
        return v
    if k == "L":
        if not isinstance(v, list):
            return v
        e = d[1]
        return [_cv(e, x) for x in v]
    if k == "D":
        if not isinstance(v, dict):
            return v
        e = d[1]
        return {key: _cv(e, x) for key, x in v.items()}
    if k == "U":
        return _cu(d, v)
    return v


def _cu(d, v):
    ds = d[1]
    best = _pick(ds, v)
    if best is not None:
        return _cv(best, v)
    disc = d[2]
    if disc is not None and isinstance(v, dict):
        t = v.get(disc[0])
        if t and isinstance(t, str):
            c = disc[1].get(t)
            if c:
                return _new(_CLS[c], v)
    for x in ds:
        try:
            return _cv(x, v)
        except Exception:
            continue
    raise RuntimeError("Could not convert data into a valid instance of the union")


def _pick(ds, v):
    # what pydantic's smart-mode union validation would choose: the first variant the value fits, or of models the
    # one with the most fields set
    best = None
    best_n = -1
    for x in ds:
        if _match(x, v):
            if type(x) is tuple and x[0] == "M":
                fn = _CLS[x[1]]._FN
                n = 0
                for key in v:
                    if key in fn:
                        n += 1
                if n > best_n:
                    best, best_n = x, n
            elif best is None:
                return x
    return best


def _match(d, v):
    if d is None:
        return True
    if type(d) is str:
        if d == "str":
            return isinstance(v, str)
        if d == "int":
            return isinstance(v, int) and not isinstance(v, bool)
        if d == "float":
            return isinstance(v, (int, float)) and not isinstance(v, bool)
        if d == "bool":
            return isinstance(v, bool)
        if d == "none":
            return v is None
        if d in ("dt", "date"):
            return isinstance(v, (str, _dt.datetime, _dt.date))
        return True
    k = d[0]
    if k == "Lit":
        for x in d[1]:
            if v == x and type(v) is type(x):
                return True
        return False
    if k == "L":
        if not isinstance(v, list):
            return False
        for x in v:
            if not _match(d[1], x):
                return False
        return True
    if k == "D":
        if not isinstance(v, dict):
            return False
        for x in v.values():
            if not _match(d[1], x):
                return False
        return True
    if k == "U":
        for x in d[1]:
            if _match(x, v):
                return True
        return False
    if k == "M":
        if not isinstance(v, dict):
            return False
        for name, alias, fd, dflt in _CLS[d[1]]._F:
            key = alias if alias is not None else name
            if key in v:
                if not _match(fd, v[key]):
                    return False
            elif dflt == _REQ:
                return False
        return True
    return True


def _build(text):
    # the classes of a generated table: [name, __name__, module, base, fields]
    out = {}
    for name, pyname, module, base, fields in _json.loads(text):
        b = out[base] if base is not None else BaseModel
        fs = tuple((f[0], f[1], _tup(f[2]), tuple(f[3])) for f in fields)
        fn = {}
        for f in fs:
            fn[f[0]] = f
        cls = type(pyname, (b,), {"_F": fs, "_FN": fn, "__module__": module})
        out[name] = cls
        _CLS[name] = cls
    return out


def _tup(d):
    if isinstance(d, list):
        if d and d[0] == "U":
            disc = d[2]
            return ("U", tuple(_tup(x) for x in d[1]), None if disc is None else (disc[0], disc[1]))
        if d and d[0] == "Lit":
            return ("Lit", tuple(d[1]))
        return tuple(_tup(x) for x in d)
    return d


def model_class(name, pyname, fields, base=BaseModel, module=None, rname=None):
    # a model written by hand in a copy (pages, stream events): fields as (name, descriptor, default)
    fs = tuple((f[0], None, f[1], f[2]) for f in fields)
    fn = {}
    for f in fs:
        fn[f[0]] = f
    attrs = {"_F": fs, "_FN": fn}
    if module is not None:
        attrs["__module__"] = module
    if rname is not None:
        attrs["_RN"] = rname
    cls = type(pyname, (base,), attrs)
    _CLS[name] = cls
    return cls


def _cast(cast_to, data):
    # a response as the type a resource asked for: a model class, a descriptor, or object / dict (as it is)
    if isinstance(cast_to, type):
        if issubclass(cast_to, BaseModel):
            return _new(cast_to, data) if isinstance(data, dict) else data
        return data
    return _cv(cast_to, data)


def construct_type(value, d):
    return _cv(d, value)


def build(cls, **values):
    return _new(cls, values)


def parse_datetime(v):
    if isinstance(v, _dt.datetime):
        return v
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return _dt.datetime.fromtimestamp(v, _dt.timezone.utc)
    s = v.decode() if isinstance(v, bytes) else v
    if s.endswith("Z") or s.endswith("z"):
        s = s[:-1] + "+00:00"
    d = _dt.datetime.fromisoformat(s)
    if d.tzinfo is not None and d.utcoffset() == _dt.timedelta(0):
        d = d.replace(tzinfo=_dt.timezone.utc)
    return d


def parse_date(v):
    if isinstance(v, _dt.date):
        return v
    return _dt.date.fromisoformat(v)


# ---------- dumping ----------

def _dump(m, js, by_alias, unset, defaults, none, include, exclude):
    out = {}
    fs = m._bv_fs
    ex = type(m)._EX
    for name, alias, d, dflt in type(m)._F:
        if name in ex:
            continue
        if unset and name not in fs:
            continue
        if include is not None and name not in include:
            continue
        if exclude is not None and name in exclude:
            continue
        v = getattr(m, name)
        if none and v is None:
            continue
        if defaults and dflt[0] != "r" and v == _default(dflt):
            continue
        out[alias if by_alias and alias is not None else name] = _dv(v, js, by_alias, unset, defaults, none)
    for k, v in m._bv_extra.items():
        if none and v is None:
            continue
        if exclude is not None and k in exclude:
            continue
        out[k] = _dv(v, js, by_alias, unset, defaults, none)
    return out


def _dv(v, js, by_alias, unset, defaults, none):
    if isinstance(v, BaseModel):
        return _dump(v, js, by_alias, unset, defaults, none, None, None)
    if isinstance(v, list):
        return [_dv(x, js, by_alias, unset, defaults, none) for x in v]
    if isinstance(v, tuple):
        t = [_dv(x, js, by_alias, unset, defaults, none) for x in v]
        return t if js else tuple(t)
    if isinstance(v, dict):
        return {k: _dv(x, js, by_alias, unset, defaults, none) for k, x in v.items()}
    if not isinstance(v, (str, int, float, bytes, _dt.datetime, _dt.date)) and v is not None and hasattr(v, "model_dump") and hasattr(v, "model_fields"):
        # a pydantic model of the program (a parsed structured output)
        return v.model_dump(mode="json" if js else "python", by_alias=by_alias, exclude_unset=unset, exclude_defaults=defaults, exclude_none=none)
    if js:
        if isinstance(v, _dt.datetime):
            return _iso_pyd(v)
        if isinstance(v, _dt.date):
            return v.isoformat()
        if isinstance(v, bytes):
            return v.decode("utf-8")
    return v


def _iso_pyd(d):
    # a datetime as pydantic-core writes it: Z for UTC
    s = d.isoformat()
    if s.endswith("+00:00"):
        s = s[:-6] + "Z"
    return s


def _jfloat(f):
    # a float as pydantic-core's JSON writes it
    if f != f or f in (float("inf"), float("-inf")):
        return "null"
    r = repr(f)
    if "e" not in r and "E" not in r:
        return r
    m, e = r.split("e")
    ex = int(e)
    neg = m.startswith("-")
    if neg:
        m = m[1:]
    digits = m.replace(".", "")
    point = len(m.split(".")[0]) + ex
    if -5 < point <= 0:
        return ("-" if neg else "") + "0." + "0" * (-point) + digits
    if 0 < point <= 16:
        if point >= len(digits):
            s = digits + "0" * (point - len(digits)) + ".0"
        else:
            s = digits[:point] + "." + digits[point:]
        return ("-" if neg else "") + s
    mant = digits[0] + ("." + digits[1:] if len(digits) > 1 else "")
    ex2 = point - 1
    return ("-" if neg else "") + mant + "e" + ("+" if ex2 > 0 else "") + str(ex2)


def _jstr(s):
    return _json.dumps(s, ensure_ascii=False)


def _jdump(v, indent=None, level=0):
    if v is None:
        return "null"
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, str):
        return _jstr(str(v))
    if isinstance(v, float):
        return _jfloat(float(v))
    if isinstance(v, int):
        return str(int(v))
    if isinstance(v, dict):
        if not v:
            return "{}"
        if indent is None:
            return "{" + ",".join(_jstr(str(k)) + ":" + _jdump(e) for k, e in v.items()) + "}"
        pad = "\n" + " " * (indent * (level + 1))
        return "{" + ",".join(pad + _jstr(str(k)) + ": " + _jdump(e, indent, level + 1) for k, e in v.items()) + "\n" + " " * (indent * level) + "}"
    if isinstance(v, (list, tuple)):
        if not v:
            return "[]"
        if indent is None:
            return "[" + ",".join(_jdump(e) for e in v) + "]"
        pad = "\n" + " " * (indent * (level + 1))
        return "[" + ",".join(pad + _jdump(e, indent, level + 1) for e in v) + "\n" + " " * (indent * level) + "]"
    if isinstance(v, _dt.datetime):
        return _jstr(_iso_pyd(v))
    if isinstance(v, _dt.date):
        return _jstr(v.isoformat())
    return _jstr(str(v))


# ---------- request bodies ----------

def prepare(v):
    # the request data as the SDKs send it: NotGiven / Omit entries dropped, sequences as lists, models dumped
    if isinstance(v, dict):
        out = {}
        for k, x in v.items():
            if isinstance(x, (NotGiven, Omit)):
                continue
            out[k] = prepare(x)
        return out
    if isinstance(v, (list, tuple, set, frozenset)):
        return [prepare(x) for x in v]
    if isinstance(v, BaseModel):
        return prepare(_dump(v, True, True, True, False, False, None, getattr(v, "__api_exclude__", None)))
    if isinstance(v, (str, bytes, int, float, type(None))):
        return v
    if isinstance(v, (_dt.datetime, _dt.date)):
        return v.isoformat()
    if hasattr(v, "__next__"):
        return [prepare(x) for x in v]
    if hasattr(v, "items") and hasattr(v, "keys"):
        return prepare(dict(v.items()))
    return v


def dumps(data):
    return _json.dumps(data, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")


_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-~!$&'()*+,;=:@"


def quote_segment(v):
    # urllib.parse.quote(v, safe="!$&'()*+,;=:@"): a value in a URL path segment
    out = []
    for b in str(v).encode("utf-8"):
        c = chr(b)
        out.append(c if c in _SAFE else "%%%02X" % b)
    return "".join(out)


def strip_not_given(d):
    return {k: v for k, v in d.items() if is_given(v)}


# ---------- Server-Sent Events ----------

class ServerSentEvent:
    def __init__(self, *, event=None, data=None, id=None, retry=None, raw=None):
        self._event = event
        self._data = data if data is not None else ""
        self._id = id
        self._retry = retry
        self._raw = raw if raw is not None else []

    @property
    def event(self):
        return self._event

    @property
    def id(self):
        return self._id

    @property
    def retry(self):
        return self._retry

    @property
    def data(self):
        return self._data

    @property
    def raw(self):
        return self._raw

    def json(self):
        return _json.loads(self._data)

    def __repr__(self):
        return "ServerSentEvent(event=%s, data=%s, id=%s, retry=%s)" % (self._event, self._data, self._id, self._retry)


def _lines(b, keep):
    # bytes.splitlines(keepends=keep) for \r, \n and \r\n
    out = []
    i = 0
    n = len(b)
    start = 0
    while i < n:
        c = b[i]
        if c == 10 or c == 13:
            end = i
            if c == 13 and i + 1 < n and b[i + 1] == 10:
                i += 1
            i += 1
            out.append(b[start:i] if keep else b[start:end])
            start = i
        else:
            i += 1
    if start < n:
        out.append(b[start:])
    return out


class SSEDecoder:
    def __init__(self):
        self._event = None
        self._data = []
        self._last_event_id = None
        self._retry = None
        self._raw = []

    def iter_bytes(self, iterator):
        for chunk in self._iter_chunks(iterator):
            for raw_line in _lines(chunk, False):
                sse = self.decode(raw_line.decode("utf-8"))
                if sse:
                    yield sse

    def _iter_chunks(self, iterator):
        data = b""
        for chunk in iterator:
            for line in _lines(chunk, True):
                data += line
                if data.endswith(b"\r\r") or data.endswith(b"\n\n") or data.endswith(b"\r\n\r\n"):
                    yield data
                    data = b""
        if data:
            yield data

    async def aiter_bytes(self, iterator):
        data = b""
        async for chunk in iterator:
            for line in _lines(chunk, True):
                data += line
                if data.endswith(b"\r\r") or data.endswith(b"\n\n") or data.endswith(b"\r\n\r\n"):
                    for raw_line in _lines(data, False):
                        sse = self.decode(raw_line.decode("utf-8"))
                        if sse:
                            yield sse
                    data = b""
        if data:
            for raw_line in _lines(data, False):
                sse = self.decode(raw_line.decode("utf-8"))
                if sse:
                    yield sse

    def decode(self, line):
        if not line:
            if not self._event and not self._data and not self._last_event_id and self._retry is None:
                self._raw = []
                return None
            sse = ServerSentEvent(event=self._event, data="\n".join(self._data), id=self._last_event_id, retry=self._retry, raw=self._raw)
            self._event = None
            self._data = []
            self._retry = None
            self._raw = []
            return sse
        self._raw.append(line)
        if line.startswith(":"):
            return None
        fieldname, _, value = line.partition(":")
        if value.startswith(" "):
            value = value[1:]
        if fieldname == "event":
            self._event = value
        elif fieldname == "data":
            self._data.append(value)
        elif fieldname == "id":
            if "\0" not in value:
                self._last_event_id = value
        elif fieldname == "retry":
            try:
                self._retry = int(value)
            except (TypeError, ValueError):
                pass
        return None


class Stream:
    def __init__(self, *, cast_to, response, client, options=None):
        self.response = response
        self._cast_to = cast_to
        self._client = client
        self._options = options
        self._decoder = SSEDecoder()
        self._it = None

    def _iterator(self):
        if self._it is None:
            self._it = self._client._stream_items(self)
        return self._it

    def __next__(self):
        return next(self._iterator())

    def __iter__(self):
        for item in self._iterator():
            yield item

    def _iter_events(self):
        return self._decoder.iter_bytes(self.response.iter_bytes())

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def close(self):
        self.response.close()


class AsyncStream:
    def __init__(self, *, cast_to, response, client, options=None):
        self.response = response
        self._cast_to = cast_to
        self._client = client
        self._options = options
        self._decoder = SSEDecoder()
        self._it = None

    def _iterator(self):
        if self._it is None:
            self._it = self._client._stream_items(self)
        return self._it

    async def __anext__(self):
        return await self._iterator().__anext__()

    async def __aiter__(self):
        async for item in self._iterator():
            yield item

    def _iter_events(self):
        return self._decoder.aiter_bytes(self.response.aiter_bytes())

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        await self.close()

    async def close(self):
        await self.response.aclose()


# ---------- requests ----------

INITIAL_RETRY_DELAY = 0.5
MAX_RETRY_DELAY = 8.0


class Options:
    def __init__(self, method, url, *, json_data=None, params=None, headers=None, max_retries=NOT_GIVEN, timeout=NOT_GIVEN, extra_json=None,
                 idempotency_key=None, post_parser=None):
        self.method = method
        self.url = url
        self.json_data = json_data
        self.params = params if params is not None else {}
        self.headers = headers
        self.max_retries = max_retries
        self.timeout = timeout
        self.extra_json = extra_json
        self.idempotency_key = idempotency_key
        self.post_parser = post_parser

    def copy(self):
        o = Options(self.method, self.url, json_data=self.json_data, params=dict(self.params), headers=self.headers, max_retries=self.max_retries,
                    timeout=self.timeout, extra_json=self.extra_json, idempotency_key=self.idempotency_key, post_parser=self.post_parser)
        return o

    def get_max_retries(self, n):
        return self.max_retries if is_given(self.max_retries) else n


def request_options(*, query=None, extra_headers=None, extra_query=None, extra_body=None, timeout=NOT_GIVEN, post_parser=None, idempotency_key=None):
    o = {}
    if extra_headers is not None:
        o["headers"] = extra_headers
    if extra_body is not None:
        o["extra_json"] = extra_body
    if query is not None or extra_query is not None:
        q = dict(query or {})
        for k, v in (extra_query or {}).items():
            q[k] = v
        o["params"] = q
    if is_given(timeout):
        o["timeout"] = timeout
    if post_parser is not None:
        o["post_parser"] = post_parser
    if idempotency_key is not None:
        o["idempotency_key"] = idempotency_key
    return o


def _merge_headers(*layers):
    # later layers win; Omit removes a header
    out = {}
    low = {}
    for layer in layers:
        for k, v in layer.items():
            lk = k.lower()
            if lk in low:
                del out[low[lk]]
                del low[lk]
            if isinstance(v, (Omit, NotGiven)):
                continue
            out[k] = v
            low[lk] = k
    return out


def _qs(params, array_format):
    # the SDKs' query strings: nested dicts as a[b]=, lists as repeated keys (comma / brackets per SDK)
    out = []
    for k, v in params.items():
        _qs_item(out, k, v, array_format)
    return out


def _qs_item(out, key, v, fmt):
    if isinstance(v, (NotGiven, Omit)):
        return
    if isinstance(v, dict):
        for k2, v2 in v.items():
            _qs_item(out, "%s[%s]" % (key, k2), v2, fmt)
        return
    if isinstance(v, (list, tuple)):
        if fmt == "comma":
            items = [_qs_prim(x) for x in v if x is not None]
            if items:
                out.append((key, ",".join(items)))
            return
        for x in v:
            _qs_item(out, key + "[]" if fmt == "brackets" else key, x, fmt)
        return
    if v is None:
        return
    out.append((key, _qs_prim(v)))


def _qs_prim(v):
    if v is True:
        return "true"
    if v is False:
        return "false"
    return str(v)


class _ClientBase:
    _array_format = "repeat"
    _sdk_error = Exception

    def _setup(self, *, version, base_url, max_retries, timeout, custom_headers, custom_query, default_timeout):
        self._version = version
        self._base_url = _slash(httpx.URL(str(base_url)))
        self.max_retries = max_retries
        self.timeout = timeout if is_given(timeout) else default_timeout
        self._custom_headers = custom_headers or {}
        self._custom_query = custom_query or {}
        self._default_timeout = default_timeout
        if max_retries is None:
            raise TypeError(self._none_retries_msg)

    @property
    def base_url(self):
        return self._base_url

    @base_url.setter
    def base_url(self, url):
        self._base_url = _slash(url if isinstance(url, httpx.URL) else httpx.URL(url))

    @property
    def user_agent(self):
        return "%s/Python %s" % (type(self).__name__, self._version)

    def platform_headers(self):
        return {
            "X-Stainless-Lang": "python",
            "X-Stainless-Package-Version": self._version,
            "X-Stainless-OS": _platform(),
            "X-Stainless-Arch": "unknown",
            "X-Stainless-Runtime": "Brevi",
            "X-Stainless-Runtime-Version": "3.13",
        }

    @property
    def auth_headers(self):
        return {}

    @property
    def default_headers(self):
        return self._base_headers()

    def _base_headers(self):
        h = {"Accept": "application/json", "Content-Type": "application/json", "User-Agent": self.user_agent}
        for k, v in self.platform_headers().items():
            h[k] = v
        for k, v in self.auth_headers.items():
            h[k] = v
        for k, v in self._custom_headers.items():
            h[k] = v
        return h

    @property
    def default_query(self):
        return dict(self._custom_query)

    def _validate_headers(self, headers):
        pass

    def _build_headers(self, options, retries_taken):
        t = options.timeout if is_given(options.timeout) else self.timeout
        first = {
            "x-stainless-timeout": str(options.timeout.read) if isinstance(options.timeout, httpx.Timeout) else str(options.timeout),
            "x-stainless-retry-count": str(retries_taken),
        }
        read = t.read if isinstance(t, httpx.Timeout) else t
        if read is not None:
            first["x-stainless-read-timeout"] = str(read)
        if self._idempotency_header and options.idempotency_key:
            first[self._idempotency_header] = options.idempotency_key
        for k, v in self.default_headers.items():
            first[k] = v
        h = _merge_headers(first, self._custom_headers, options.headers or {})
        self._validate_headers(h)
        return h

    _idempotency_header = None

    def _prepare_url(self, url):
        u = httpx.URL(url)
        if u.is_relative_url:
            return self._base_url.copy_with(raw_path=self._base_url.raw_path + u.raw_path.lstrip(b"/"))
        return u

    def _build_request(self, options, retries_taken):
        json_data = options.json_data
        if options.extra_json is not None:
            if json_data is None:
                json_data = options.extra_json
            elif isinstance(json_data, dict):
                json_data = dict(json_data)
                for k, v in options.extra_json.items():
                    json_data[k] = v
        headers = self._build_headers(options, retries_taken)
        params = dict(self.default_query)
        for k, v in (options.params or {}).items():
            params[k] = v
        url = self._prepare_url(options.url)
        kw = {}
        if options.method.lower() != "get":
            body = prepare(json_data) if json_data is not None else None
            kw["content"] = dumps(body) if body is not None else None
        else:
            for k in list(headers):
                if k.lower() == "content-type":
                    del headers[k]
        q = _qs(prepare(params), self._array_format) if params else None
        return self._client.build_request(options.method.upper(), url, headers=headers, params=q,
                                          timeout=self.timeout if not is_given(options.timeout) else options.timeout, **kw)

    def _parse_retry_after(self, headers):
        if headers is None:
            return None
        try:
            return float(headers.get("retry-after-ms", None)) / 1000
        except (TypeError, ValueError):
            pass
        r = headers.get("retry-after")
        try:
            return float(r)
        except (TypeError, ValueError):
            pass
        return None

    def _retry_timeout(self, remaining, options, headers):
        max_retries = options.get_max_retries(self.max_retries)
        ra = self._parse_retry_after(headers)
        if ra is not None and 0 < ra <= self._max_retry_after:
            return ra
        n = min(max_retries - remaining, 1000)
        s = min(INITIAL_RETRY_DELAY * pow(2.0, n), MAX_RETRY_DELAY)
        t = s * (1 - 0.25 * _random.random())
        return t if t >= 0 else 0

    _max_retry_after = 4294967.0

    def _should_retry(self, response):
        h = response.headers.get("x-should-retry")
        if h == "true":
            return True
        if h == "false":
            return False
        c = response.status_code
        return c == 408 or c == 409 or c == 429 or c >= 500

    def _status_error_from_response(self, response):
        if response.is_closed and not response.is_stream_consumed:
            body = None
            msg = "Error code: %d" % response.status_code
        else:
            text = response.text.strip()
            body = text
            try:
                body = _json.loads(text)
                msg = "Error code: %d - %s" % (response.status_code, body)
            except Exception:
                msg = text or "Error code: %d" % response.status_code
        return self._make_status_error(msg, body=body, response=response)

    def _process_response_data(self, *, data, cast_to, response):
        if cast_to is None:
            return data
        return _cast(cast_to, data)

    def _parse(self, cast_to, response, options, stream, stream_cls):
        if stream:
            # stream_cls is (the stream class, what each event is built as)
            return stream_cls[0](cast_to=stream_cls[1], response=response, client=self, options=options)
        if cast_to is None:
            return None
        if cast_to is str:
            return response.text
        data = response.json()
        v = _cast(cast_to, data)
        if isinstance(v, BaseModel):
            self._add_ids(v, response)
        if options.post_parser is not None:
            v = options.post_parser(v)
        return v

    def _add_ids(self, v, response):
        v._request_id = response.headers.get("x-request-id")

    def _list_page(self, page_cls, item, options, response):
        page = _new(page_cls, response.json())
        self._add_ids(page, response)
        page._bv_client = self
        page._bv_options = options
        return page


def _slash(u):
    if u.raw_path.endswith(b"/"):
        return u
    return u.copy_with(raw_path=u.raw_path + b"/")


def _platform():
    import sys
    p = sys.platform
    if p.startswith("linux"):
        return "Linux"
    if p == "darwin":
        return "MacOS"
    return "Other:" + p


class SyncClient(_ClientBase):
    def _init_http(self, http_client):
        self._client = http_client or httpx.Client(base_url=self._base_url, timeout=self.timeout, follow_redirects=True)
        self._own_http = http_client is None

    def close(self):
        if hasattr(self, "_client"):
            self._client.close()

    def is_closed(self):
        return self._client.is_closed

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def request(self, cast_to, options, *, stream=False, stream_cls=None):
        input_options = options.copy()
        if input_options.idempotency_key is None and input_options.method.lower() != "get" and self._idempotency_header:
            input_options.idempotency_key = self._idempotency_key()
        max_retries = input_options.get_max_retries(self.max_retries)
        retries_taken = 0
        while True:
            remaining = max_retries - retries_taken
            request = self._build_request(input_options, retries_taken)
            try:
                response = self._client.send(request, stream=stream)
            except httpx.TimeoutException as err:
                if remaining > 0:
                    self._sleep(retries_taken, max_retries, input_options, None)
                    retries_taken += 1
                    continue
                raise self._timeout_error(request) from err
            except Exception as err:
                if isinstance(err, self._sdk_error):
                    raise
                if remaining > 0:
                    self._sleep(retries_taken, max_retries, input_options, None)
                    retries_taken += 1
                    continue
                raise self._connection_error(request) from err
            if response.is_success:
                return self._parse(cast_to, response, input_options, stream, stream_cls)
            if remaining > 0 and self._should_retry(response):
                response.close()
                self._sleep(retries_taken, max_retries, input_options, response)
                retries_taken += 1
                continue
            if not response.is_closed:
                response.read()
            raise self._status_error_from_response(response) from None

    def _sleep(self, retries_taken, max_retries, options, response):
        _time.sleep(self._retry_timeout(max_retries - retries_taken, options, response.headers if response is not None else None))

    def get(self, path, *, cast_to, options=None, stream=False, stream_cls=None):
        return self.request(cast_to, Options("get", path, **(options or {})), stream=stream, stream_cls=stream_cls)

    def post(self, path, *, cast_to, body=None, options=None, stream=False, stream_cls=None):
        return self.request(cast_to, Options("post", path, json_data=body, **(options or {})), stream=stream, stream_cls=stream_cls)

    def delete(self, path, *, cast_to, body=None, options=None):
        return self.request(cast_to, Options("delete", path, json_data=body, **(options or {})))

    def get_api_list(self, path, *, page, model, options=None, method="get", body=None):
        o = Options(method, path, json_data=body, **(options or {}))
        return self._get_page(page, model, o)

    def _get_page(self, page, model, options):
        def parser(resp):
            return resp
        return self.request(("Page", page, model), options)

    def _parse(self, cast_to, response, options, stream, stream_cls):
        if type(cast_to) is tuple and cast_to[0] == "Page":
            return self._list_page(cast_to[1], cast_to[2], options, response)
        return _ClientBase._parse(self, cast_to, response, options, stream, stream_cls)


class AsyncClientBase(_ClientBase):
    def _init_http(self, http_client):
        self._client = http_client or httpx.AsyncClient(base_url=self._base_url, timeout=self.timeout, follow_redirects=True)
        self._own_http = http_client is None

    async def close(self):
        await self._client.aclose()

    def is_closed(self):
        return self._client.is_closed

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        await self.close()

    async def request(self, cast_to, options, *, stream=False, stream_cls=None):
        input_options = options.copy()
        if input_options.idempotency_key is None and input_options.method.lower() != "get" and self._idempotency_header:
            input_options.idempotency_key = self._idempotency_key()
        max_retries = input_options.get_max_retries(self.max_retries)
        retries_taken = 0
        while True:
            remaining = max_retries - retries_taken
            request = self._build_request(input_options, retries_taken)
            try:
                response = await self._client.send(request, stream=stream)
            except httpx.TimeoutException as err:
                if remaining > 0:
                    await self._sleep(retries_taken, max_retries, input_options, None)
                    retries_taken += 1
                    continue
                raise self._timeout_error(request) from err
            except Exception as err:
                if isinstance(err, self._sdk_error):
                    raise
                if remaining > 0:
                    await self._sleep(retries_taken, max_retries, input_options, None)
                    retries_taken += 1
                    continue
                raise self._connection_error(request) from err
            if response.is_success:
                if type(cast_to) is tuple and cast_to[0] == "Page":
                    await response.aread()
                    return self._list_page(cast_to[1], cast_to[2], input_options, response)
                if not stream:
                    await response.aread()
                return self._parse(cast_to, response, input_options, stream, stream_cls)
            if remaining > 0 and self._should_retry(response):
                await response.aclose()
                await self._sleep(retries_taken, max_retries, input_options, response)
                retries_taken += 1
                continue
            if not response.is_closed:
                await response.aread()
            raise self._status_error_from_response(response) from None

    async def _sleep(self, retries_taken, max_retries, options, response):
        await asyncio.sleep(self._retry_timeout(max_retries - retries_taken, options, response.headers if response is not None else None))

    async def get(self, path, *, cast_to, options=None, stream=False, stream_cls=None):
        return await self.request(cast_to, Options("get", path, **(options or {})), stream=stream, stream_cls=stream_cls)

    async def post(self, path, *, cast_to, body=None, options=None, stream=False, stream_cls=None):
        return await self.request(cast_to, Options("post", path, json_data=body, **(options or {})), stream=stream, stream_cls=stream_cls)

    async def delete(self, path, *, cast_to, body=None, options=None):
        return await self.request(cast_to, Options("delete", path, json_data=body, **(options or {})))

    def get_api_list(self, path, *, page, model, options=None, method="get", body=None):
        o = Options(method, path, json_data=body, **(options or {}))
        return AsyncPaginator(self, page, model, o)


# ---------- pages ----------

class BasePage(BaseModel):
    def _get_page_items(self):
        return self.data or []

    def has_next_page(self):
        if not self._get_page_items():
            return False
        return self.next_page_info() is not None

    def next_page_info(self):
        return None

    def _next_options(self, info):
        o = self._bv_options.copy()
        p = dict(o.params)
        for k, v in info.items():
            p[k] = v
        o.params = p
        return o

    def __iter__(self):
        for page in self.iter_pages():
            for item in page._get_page_items():
                yield item

    def iter_pages(self):
        page = self
        while True:
            yield page
            if page.has_next_page():
                page = page.get_next_page()
            else:
                return

    def get_next_page(self):
        info = self.next_page_info()
        if not info:
            raise RuntimeError("No next page expected; please check `.has_next_page()` before calling `.get_next_page()`.")
        return self._bv_client.request(("Page", type(self), None), self._next_options(info))


class AsyncBasePage(BasePage):
    async def __aiter__(self):
        page = self
        while True:
            for item in page._get_page_items():
                yield item
            if page.has_next_page():
                page = await page.get_next_page()
            else:
                return

    async def iter_pages(self):
        page = self
        while True:
            yield page
            if page.has_next_page():
                page = await page.get_next_page()
            else:
                return

    async def get_next_page(self):
        info = self.next_page_info()
        if not info:
            raise RuntimeError("No next page expected; please check `.has_next_page()` before calling `.get_next_page()`.")
        return await self._bv_client.request(("Page", type(self), None), self._next_options(info))


class AsyncPaginator:
    def __init__(self, client, page_cls, model, options):
        self._client = client
        self._page_cls = page_cls
        self._model = model
        self._options = options

    def __await__(self):
        # (a task: Brevi's coroutines have no __await__ of their own)
        return asyncio.ensure_future(self._get_page()).__await__()

    async def _get_page(self):
        return await self._client.request(("Page", self._page_cls, self._model), self._options)

    async def __aiter__(self):
        page = await self._get_page()
        async for item in page:
            yield item


def page_class(base, pyname, item, fields, module):
    # SyncPage[ModelInfo] and the like: a class of that __name__, its data built as the item model
    fs = [("data", ("L", ("M", item)), ("r",))] + list(fields)
    return model_class(pyname, pyname, fs, base=base, module=module)
